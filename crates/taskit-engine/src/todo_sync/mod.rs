//! Sync TODO/FIXME source markers to GitHub issues.
//!
//! Scans `crates/` and `src/` for TODO/FIXME comments, diffs them against a
//! lockfile of previously-synced markers, and (with `--update`) creates a
//! GitHub issue for each new marker and closes the issue for each marker
//! that has since been removed from source. Mirrors `check-protocol-drift`'s
//! shape: default run is a read-only gate, `--update` mutates.
//!
//! `--update` is idempotent even when the lockfile is missing: before
//! creating anything it reads the repo's existing issues and adopts the one
//! already tracking a marker instead of opening a duplicate.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use taskit_types::error::{TaskitError, TaskitResultExt};
use xshell::cmd;

use crate::ctx::Ctx;
use crate::health::extract_todo_fixme_comment;
use crate::release::gh::resolve_repo;

pub mod dedupe;

const DEFAULT_LOCK_PATH: &str = "taskit-todo-sync.lock";
/// Upper bound on issues fetched when looking for ones to adopt.
const ISSUE_SCAN_LIMIT: u32 = 1000;
/// Stable, line-independent tag embedded in created issue bodies.
const ISSUE_TAG_PREFIX: &str = "taskit-todo-sync";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Lockfile {
    version: u8,
    entries: Vec<SyncedTodo>,
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SyncedTodo {
    file: String,
    text: String,
    issue_number: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TodoMarker {
    file: String,
    line: usize,
    text: String,
}

/// An issue already present in the repo, as returned by `gh issue list`.
#[derive(Debug, Clone, Deserialize)]
struct IssueSummary {
    number: u64,
    title: String,
    #[serde(default)]
    body: String,
    /// `gh` reports this as OPEN/CLOSED. Defaults to empty so partial
    /// payloads still decode.
    #[serde(default)]
    state: String,
}

/// Run the `todo-sync` subcommand.
///
/// Default: read-only gate — errors if there are unsynced markers.
/// `--warn-only`: same diff, but never errors.
/// `--update`: creates/closes GitHub issues via `gh` and writes the lockfile.
pub fn run(ctx: &Ctx, update: bool, warn_only: bool) -> Result<(), TaskitError> {
    let root = ctx.root();
    let lock_path = root.join(DEFAULT_LOCK_PATH);
    let markers = scan_markers(root)?;
    let lockfile = read_lockfile(&lock_path).unwrap_or_default();

    let new_markers: Vec<&TodoMarker> = markers
        .iter()
        .filter(|m| {
            !lockfile
                .entries
                .iter()
                .any(|e| e.file == m.file && e.text == m.text)
        })
        .collect();
    let removed_entries: Vec<&SyncedTodo> = lockfile
        .entries
        .iter()
        .filter(|e| !markers.iter().any(|m| m.file == e.file && m.text == e.text))
        .collect();

    if new_markers.is_empty() && removed_entries.is_empty() {
        taskit_output::taskit_ok!(
            "todo-sync: OK ({} tracked marker(s), no drift)",
            lockfile.entries.len()
        );
        return Ok(());
    }

    report(&new_markers, &removed_entries);

    if !update {
        taskit_output::taskit_err!("todo-sync: {}", unsynced_hint());
        if warn_only {
            return Ok(());
        }
        return Err(TaskitError::other(
            "todo/fixme markers out of sync with tracked issues",
        ));
    }

    let repo = resolve_repo(ctx)?;
    let mut entries: Vec<SyncedTodo> = lockfile
        .entries
        .iter()
        .filter(|e| markers.iter().any(|m| m.file == e.file && m.text == e.text))
        .cloned()
        .collect();

    // A lost or stale lockfile must not produce duplicate issues, so every
    // new marker is reconciled against the issues that already exist.
    let adopted = if ctx.dry_run || new_markers.is_empty() {
        HashMap::new()
    } else {
        adopt_existing_issues(ctx, &repo, &new_markers)?
    };

    for marker in &new_markers {
        let key = issue_key(marker);
        if let Some(issue) = adopted.get(&key) {
            let number = issue.number;
            taskit_output::taskit_ok!(
                "todo-sync: reusing issue #{number} for {}:{}",
                marker.file,
                marker.line
            );
            if is_closed(&issue.state) {
                taskit_output::taskit_warn!(
                    "todo-sync: issue #{number} for {}:{} is closed; reopen it or drop the citation to track it again",
                    marker.file,
                    marker.line
                );
            }
            entries.push(SyncedTodo {
                file: marker.file.clone(),
                text: marker.text.clone(),
                issue_number: number,
            });
            continue;
        }
        if ctx.dry_run {
            taskit_output::taskit_dry!("gh issue create --repo {repo} --title \"{}\"", marker.text);
            continue;
        }
        let number = create_issue(ctx, &repo, marker)?;
        taskit_output::taskit_ok!(
            "todo-sync: created issue #{number} for {}:{}",
            marker.file,
            marker.line
        );
        entries.push(SyncedTodo {
            file: marker.file.clone(),
            text: marker.text.clone(),
            issue_number: number,
        });
    }

    for entry in &removed_entries {
        if ctx.dry_run {
            taskit_output::taskit_dry!("gh issue close {} --repo {repo}", entry.issue_number);
            continue;
        }
        close_issue(ctx, &repo, entry.issue_number)?;
        taskit_output::taskit_ok!("todo-sync: closed issue #{}", entry.issue_number);
    }

    if !ctx.dry_run {
        let count = entries.len();
        write_lockfile(
            &lock_path,
            &Lockfile {
                version: 1,
                entries,
            },
        )?;
        taskit_output::taskit_ok!("todo-sync: lockfile updated ({count} tracked marker(s))");
    }

    Ok(())
}

fn report(new_markers: &[&TodoMarker], removed_entries: &[&SyncedTodo]) {
    if !new_markers.is_empty() {
        taskit_output::taskit_progress!("todo-sync: {} new marker(s):", new_markers.len());
        for marker in new_markers {
            taskit_output::taskit_progress!("+ {}:{} {}", marker.file, marker.line, marker.text);
        }
    }
    if !removed_entries.is_empty() {
        taskit_output::taskit_progress!(
            "todo-sync: {} marker(s) removed from source (issue(s) to close):",
            removed_entries.len()
        );
        for entry in removed_entries {
            taskit_output::taskit_progress!("- {} (issue #{})", entry.file, entry.issue_number);
        }
    }
}

/// Stable, line-independent tag embedded in every issue body this command
/// creates. Line numbers drift as source moves, so matching keys on the file
/// path alone.
fn marker_tag(file: &str) -> String {
    format!("<!-- {ISSUE_TAG_PREFIX}: {file} -->")
}

/// Identity of the issue that tracks a marker: its title plus its file.
fn issue_key(marker: &TodoMarker) -> String {
    format!("{}\u{1}{}", marker.text, marker.file)
}

/// True when an issue body belongs to `marker.file` — either via the stable
/// tag, or via the `\`path:line\`` reference older issues were created with.
fn body_tracks_file(body: &str, file: &str) -> bool {
    body.contains(&marker_tag(file)) || body.contains(&format!("`{file}:"))
}

/// Map each marker to the issue that already tracks it.
///
/// Fails rather than guessing: if the issue list cannot be read, creating is
/// the one outcome guaranteed to be wrong.
fn adopt_existing_issues(
    ctx: &Ctx,
    repo: &str,
    markers: &[&TodoMarker],
) -> Result<HashMap<String, IssueSummary>, TaskitError> {
    let issues = list_issues(ctx, repo)?;
    let mut adopted = HashMap::new();
    for marker in markers {
        match find_existing(&issues, marker) {
            Some(issue) => {
                adopted.insert(issue_key(marker), issue.clone());
            }
            None => {
                if let Some(cited) = cited_issue_number(&marker.text) {
                    taskit_output::taskit_warn!(
                        "todo-sync: {}:{} cites issue #{cited}, which is not in the repo",
                        marker.file,
                        marker.line
                    );
                }
            }
        }
    }
    Ok(adopted)
}

/// Fetch every issue in the repo, open or closed.
fn list_issues(ctx: &Ctx, repo: &str) -> Result<Vec<IssueSummary>, TaskitError> {
    let sh = &ctx.sh;
    let args = vec![
        "issue".to_owned(),
        "list".to_owned(),
        "--repo".to_owned(),
        repo.to_owned(),
        "--state".to_owned(),
        "all".to_owned(),
        "--limit".to_owned(),
        ISSUE_SCAN_LIMIT.to_string(),
        "--json".to_owned(),
        "number,title,body,state".to_owned(),
    ];
    let output = ctx.run_capture(cmd!(sh, "gh {args...}"))?;
    if !output.success {
        return Err(TaskitError::other(format!(
            "failed to list existing issues in {repo}: {}",
            output.stderr.trim()
        )));
    }
    serde_json::from_str(&output.stdout).err_context("failed to parse `gh issue list` output")
}

/// The issue already tracking `marker`, if any.
///
/// A `(#N)` citation in the marker wins whenever it resolves to an issue in
/// the repo — the author named the issue they meant. Citations are often
/// plan-document numbering rather than GitHub numbers, so an unresolved
/// citation falls back to matching on title + file instead of dead-ending.
fn find_existing<'a>(issues: &'a [IssueSummary], marker: &TodoMarker) -> Option<&'a IssueSummary> {
    if let Some(cited) = cited_issue_number(&marker.text)
        && let Some(issue) = issues.iter().find(|issue| issue.number == cited)
    {
        return Some(issue);
    }
    issues
        .iter()
        .filter(|issue| issue.title == marker.text && body_tracks_file(&issue.body, &marker.file))
        .min_by_key(|issue| issue.number)
}

/// True when `gh` reported the issue as closed.
fn is_closed(state: &str) -> bool {
    state.eq_ignore_ascii_case("closed")
}

/// Issue body for a newly created marker.
fn issue_body(marker: &TodoMarker) -> String {
    format!(
        "Auto-tracked source marker.\n\n`{}:{}`\n\n{}",
        marker.file,
        marker.line,
        marker_tag(&marker.file)
    )
}

fn create_issue(ctx: &Ctx, repo: &str, marker: &TodoMarker) -> Result<u64, TaskitError> {
    let sh = &ctx.sh;
    let title = marker.text.clone();
    let body = issue_body(marker);
    let args = vec![
        "issue".to_owned(),
        "create".to_owned(),
        "--repo".to_owned(),
        repo.to_owned(),
        "--title".to_owned(),
        title,
        "--body".to_owned(),
        body,
    ];
    let output = cmd!(sh, "gh {args...}")
        .read()
        .map_err(TaskitError::other)?;
    parse_issue_number(&output).ok_or_else(|| {
        TaskitError::other(format!(
            "could not parse issue number from `gh issue create` output: {output}"
        ))
    })
}

fn close_issue(ctx: &Ctx, repo: &str, number: u64) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let number = number.to_string();
    ctx.run(cmd!(sh, "gh issue close {number} --repo {repo}"))
}

/// Extract the trailing issue number from a `gh issue create` URL, e.g.
/// `https://github.com/owner/repo/issues/42`.
fn parse_issue_number(output: &str) -> Option<u64> {
    output.trim().rsplit('/').next()?.parse().ok()
}

/// Trailing `(#47)` citation in a marker, which the author uses to name the
/// issue already tracking it.
fn cited_issue_number(text: &str) -> Option<u64> {
    text.strip_suffix(')')?.rsplit_once("(#")?.1.parse().ok()
}

fn read_lockfile(path: &Path) -> Result<Lockfile, TaskitError> {
    let content = std::fs::read_to_string(path)
        .err_context_with(|| format!("no todo-sync lockfile at {}", path.display()))?;
    serde_json::from_str(&content).err_context("failed to parse todo-sync lockfile")
}

fn write_lockfile(path: &Path, lockfile: &Lockfile) -> Result<(), TaskitError> {
    let mut content = serde_json::to_string_pretty(lockfile).map_err(TaskitError::other)?;
    content.push('\n');
    std::fs::write(path, content).err_context_with(|| format!("failed to write {}", path.display()))
}

fn scan_markers(root: &Path) -> Result<Vec<TodoMarker>, TaskitError> {
    let mut out = Vec::new();
    for child in ["crates", "src"] {
        scan_markers_in_tree(&root.join(child), root, &mut out)?;
    }
    out.sort_by(|a, b| (a.file.as_str(), a.line).cmp(&(b.file.as_str(), b.line)));
    Ok(out)
}

fn scan_markers_in_tree(
    path: &Path,
    root: &Path,
    out: &mut Vec<TodoMarker>,
) -> Result<(), TaskitError> {
    if !path.exists() {
        return Ok(());
    }
    for entry in
        std::fs::read_dir(path).err_context_with(|| format!("failed to read {}", path.display()))?
    {
        let entry =
            entry.err_context_with(|| format!("failed to read entry in {}", path.display()))?;
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .err_context_with(|| format!("failed to inspect {}", entry_path.display()))?;
        if file_type.is_dir() {
            scan_markers_in_tree(&entry_path, root, out)?;
        } else if entry_path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            let content = std::fs::read_to_string(&entry_path)
                .err_context_with(|| format!("failed to read {}", entry_path.display()))?;
            let rel = display_relative(root, &entry_path);
            let mut in_block_comment = false;
            for (i, line) in content.lines().enumerate() {
                if let Some(text) = extract_todo_fixme_comment(line, &mut in_block_comment) {
                    out.push(TodoMarker {
                        file: rel.clone(),
                        line: i + 1,
                        text,
                    });
                }
            }
        }
    }
    Ok(())
}

fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Gate error pointing at the command that repairs it.
///
/// Lives here as a function so a test can assert the path: the CLI was
/// restructured under `protocol` and the stale `taskit todo-sync` form went
/// unnoticed because nothing executed the string.
fn unsynced_hint() -> String {
    "run `taskit protocol todo-sync --update` to sync issues".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // -- unsynced_hint --

    #[test]
    fn error_hint_names_the_real_subcommand_path() {
        let hint = unsynced_hint();
        assert!(
            hint.contains("taskit protocol todo-sync"),
            "hint must name the live path, was: {hint}"
        );
        assert!(
            !hint.contains("taskit todo-sync"),
            "hint must not name the pre-restructure path, was: {hint}"
        );
    }

    fn marker(file: &str, line: usize, text: &str) -> TodoMarker {
        TodoMarker {
            file: file.to_string(),
            line,
            text: text.to_string(),
        }
    }

    fn synced(file: &str, text: &str, issue_number: u64) -> SyncedTodo {
        SyncedTodo {
            file: file.to_string(),
            text: text.to_string(),
            issue_number,
        }
    }

    fn issue(number: u64, title: &str, body: &str) -> IssueSummary {
        IssueSummary {
            number,
            title: title.to_string(),
            body: body.to_string(),
            state: "OPEN".to_string(),
        }
    }

    // -- cited_issue_number --

    #[test]
    fn cited_issue_number_is_read_from_a_trailing_reference() {
        assert_eq!(cited_issue_number("TODO(x): do the thing (#47)"), Some(47));
        assert_eq!(cited_issue_number("TODO: plain"), None);
        assert_eq!(cited_issue_number("TODO: unterminated (#12"), None);
        assert_eq!(cited_issue_number("TODO: not a number (#abc)"), None);
    }

    // -- parse_issue_number --

    #[test]
    fn parse_issue_number_from_url() {
        assert_eq!(
            parse_issue_number("https://github.com/89jobrien/taskit/issues/42\n"),
            Some(42)
        );
    }

    #[test]
    fn parse_issue_number_rejects_non_numeric() {
        assert_eq!(parse_issue_number("not a url"), None);
    }

    // -- IssueSummary --

    #[test]
    fn issue_summary_parses_state() {
        let issue: IssueSummary =
            serde_json::from_str(r#"{"number":7,"title":"t","body":"b","state":"CLOSED"}"#)
                .expect("payload with state should parse");
        assert_eq!(issue.state, "CLOSED");
        // Older partial payloads must still decode.
        let partial: IssueSummary = serde_json::from_str(r#"{"number":8,"title":"t","body":"b"}"#)
            .expect("payload without state should still parse");
        assert_eq!(partial.state, "");
    }

    // -- scan_markers_in_tree --

    #[test]
    fn scan_markers_extracts_file_line_and_text() {
        let dir = TempDir::new().expect("tempdir");
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("main.rs"),
            ["fn main() {}", "// TODO: wire this up", "// FIXME: broken"].join("\n"),
        )
        .unwrap();

        let markers = scan_markers(dir.path()).expect("scan should succeed");
        assert_eq!(markers.len(), 2);
        assert_eq!(markers[0].line, 2);
        assert_eq!(markers[0].text, "TODO: wire this up");
        assert_eq!(markers[1].line, 3);
        assert_eq!(markers[1].text, "FIXME: broken");
    }

    // -- lockfile round trip --

    #[test]
    fn write_and_read_lockfile_round_trips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(DEFAULT_LOCK_PATH);
        let lockfile = Lockfile {
            version: 1,
            entries: vec![synced("src/main.rs", "TODO: x", 7)],
        };
        write_lockfile(&path, &lockfile).unwrap();
        let loaded = read_lockfile(&path).unwrap();
        assert_eq!(loaded, lockfile);
    }

    #[test]
    fn read_missing_lockfile_is_err() {
        let dir = TempDir::new().unwrap();
        assert!(read_lockfile(&dir.path().join(DEFAULT_LOCK_PATH)).is_err());
    }

    // -- diff logic (new/removed) --

    #[test]
    fn new_marker_not_in_lockfile_is_flagged() {
        let markers = [marker("src/a.rs", 1, "TODO: a")];
        let entries: Vec<SyncedTodo> = vec![];
        let new: Vec<&TodoMarker> = markers
            .iter()
            .filter(|m| !entries.iter().any(|e| e.file == m.file && e.text == m.text))
            .collect();
        assert_eq!(new.len(), 1);
    }

    #[test]
    fn removed_marker_still_in_lockfile_is_flagged() {
        let markers: Vec<TodoMarker> = vec![];
        let entries = [synced("src/a.rs", "TODO: a", 3)];
        let removed: Vec<&SyncedTodo> = entries
            .iter()
            .filter(|e| !markers.iter().any(|m| m.file == e.file && m.text == e.text))
            .collect();
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn unchanged_marker_is_neither_new_nor_removed() {
        let markers = [marker("src/a.rs", 5, "TODO: a")];
        let entries = [synced("src/a.rs", "TODO: a", 3)];
        let new: Vec<&TodoMarker> = markers
            .iter()
            .filter(|m| !entries.iter().any(|e| e.file == m.file && e.text == m.text))
            .collect();
        let removed: Vec<&SyncedTodo> = entries
            .iter()
            .filter(|e| !markers.iter().any(|m| m.file == e.file && m.text == e.text))
            .collect();
        assert!(new.is_empty());
        assert!(removed.is_empty());
    }

    // -- existing-issue adoption --

    #[test]
    fn issue_body_carries_the_stable_tag() {
        let body = issue_body(&marker("src/a.rs", 12, "TODO: a"));
        assert!(body.contains("`src/a.rs:12`"));
        assert!(body.contains(&marker_tag("src/a.rs")));
    }

    #[test]
    fn created_issue_is_adopted_by_a_later_run() {
        let marker = marker("src/a.rs", 12, "TODO: a");
        let created = issue(42, "TODO: a", &issue_body(&marker));
        assert_eq!(
            find_existing(&[created], &marker).map(|i| i.number),
            Some(42)
        );
    }

    #[test]
    fn legacy_body_without_tag_is_still_adopted() {
        let issues = [issue(
            42,
            "TODO: a",
            "Auto-tracked source marker.\n\n`src/a.rs:9`",
        )];
        assert_eq!(
            find_existing(&issues, &marker("src/a.rs", 12, "TODO: a")).map(|i| i.number),
            Some(42)
        );
    }

    #[test]
    fn issue_in_a_different_file_is_not_adopted() {
        let issues = [issue(
            42,
            "TODO: a",
            "Auto-tracked source marker.\n\n`src/b.rs:9`",
        )];
        assert!(find_existing(&issues, &marker("src/a.rs", 12, "TODO: a")).is_none());
    }

    #[test]
    fn issue_with_a_different_title_is_not_adopted() {
        let marker = marker("src/a.rs", 12, "TODO: a");
        let unrelated = issue(
            29,
            "fix(crux): resolve unused scaffolding",
            &issue_body(&marker),
        );
        assert!(find_existing(&[unrelated], &marker).is_none());
    }

    #[test]
    fn duplicate_issues_resolve_to_the_lowest_number() {
        let marker = marker("src/a.rs", 12, "TODO: a");
        let issues = [
            issue(76, "TODO: a", &issue_body(&marker)),
            issue(42, "TODO: a", &issue_body(&marker)),
        ];
        assert_eq!(find_existing(&issues, &marker).map(|i| i.number), Some(42));
    }

    // -- cited-issue adoption --

    #[test]
    fn cited_issue_is_adopted_even_when_title_and_file_do_not_match() {
        let marker = marker("src/a.rs", 12, "TODO(feature): add doctor (#47)");
        let cited = issue(47, "totally different title", "and a different body");
        assert_eq!(find_existing(&[cited], &marker).map(|i| i.number), Some(47));
    }

    #[test]
    fn cited_issue_wins_over_a_title_and_file_match() {
        let marker = marker("src/a.rs", 12, "TODO: a (#9)");
        let issues = [
            issue(3, "TODO: a", &issue_body(&marker)),
            issue(9, "TODO: a", &issue_body(&marker)),
        ];
        assert_eq!(find_existing(&issues, &marker).map(|i| i.number), Some(9));
    }

    #[test]
    fn citation_that_is_not_a_github_number_falls_back_to_title_and_file() {
        // Plan documents number their own items; those citations must not
        // stop an existing issue from being reused.
        let marker = marker("src/a.rs", 12, "TODO(feature)(#21): add attestation");
        let tracked = issue(
            59,
            "TODO(feature)(#21): add attestation",
            &issue_body(&marker),
        );
        assert_eq!(
            find_existing(&[tracked], &marker).map(|i| i.number),
            Some(59)
        );
    }

    #[test]
    fn unresolved_citation_without_any_match_adopts_nothing() {
        let marker = marker("src/a.rs", 12, "TODO(feature)(#21): add attestation");
        let unrelated = issue(59, "fix(output): unrelated", "nope");
        assert!(find_existing(&[unrelated], &marker).is_none());
    }

    // -- closed-issue detection --

    #[test]
    fn closed_state_is_detected_case_insensitively() {
        assert!(is_closed("CLOSED"));
        assert!(is_closed("closed"));
        assert!(!is_closed("OPEN"));
        assert!(!is_closed(""));
    }

    #[test]
    fn adopted_closed_issue_is_still_reported_for_reuse() {
        let marker = marker("src/a.rs", 12, "TODO: a");
        let mut closed = issue(47, "TODO: a", &issue_body(&marker));
        closed.state = "CLOSED".to_string();
        let issues = [closed];
        let found = find_existing(&issues, &marker).expect("closed issue still tracks the marker");
        assert_eq!(found.number, 47);
        assert!(is_closed(&found.state), "caller warns on this state");
    }
}
