//! Sync TODO/FIXME source markers to GitHub issues.
//!
//! Scans `crates/` and `src/` for TODO/FIXME comments, diffs them against a
//! lockfile of previously-synced markers, and (with `--update`) creates a
//! GitHub issue for each new marker and closes the issue for each marker
//! that has since been removed from source. Mirrors `check-protocol-drift`'s
//! shape: default run is a read-only gate, `--update` mutates.

use serde::{Deserialize, Serialize};
use std::path::Path;
use taskit_types::error::{TaskitError, TaskitResultExt};
use xshell::cmd;

use crate::ctx::Ctx;
use crate::health::extract_todo_fixme_comment;
use crate::release::gh::resolve_repo;

const DEFAULT_LOCK_PATH: &str = "taskit-todo-sync.lock";

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
        taskit_output::taskit_err!("todo-sync: run `taskit todo-sync --update` to sync issues");
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

    for marker in &new_markers {
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

fn create_issue(ctx: &Ctx, repo: &str, marker: &TodoMarker) -> Result<u64, TaskitError> {
    let sh = &ctx.sh;
    let title = marker.text.clone();
    let body = format!(
        "Auto-tracked source marker.\n\n`{}:{}`",
        marker.file, marker.line
    );
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
}
