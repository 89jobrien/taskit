//! Deduplicate the auto-generated issues `todo-sync` creates.
//!
//! Scope is deliberately narrow: only issues referenced by a lockfile entry
//! are ever considered, so an issue a human opened is never closed. Grouping
//! is derived from `(title, file)` — the same signal `todo_sync` writes into
//! generated issue bodies — rather than from any free-form grouping.

use super::{Ctx, IssueSummary, Lockfile, SyncedTodo};
use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::release::gh::resolve_repo;

/// Close duplicate issues `todo-sync` generated for the same marker, then
/// repoint the lockfile at the survivor.
///
/// Read-only unless `update` is set, mirroring `todo_sync::run`. Never reopens
/// anything: a closed entry is reported so a human decides, because the closure
/// may predate the work.
pub fn run(ctx: &Ctx, update: bool) -> Result<(), TaskitError> {
    let lock_path = ctx.root().join(super::DEFAULT_LOCK_PATH);
    let mut lockfile: Lockfile = super::read_lockfile(&lock_path).unwrap_or_default();
    let repo = resolve_repo(ctx)?;
    let issues = super::list_issues(ctx, &repo)?;

    for entry in lockfile.entries.iter_mut() {
        report_closed_citation(&issues, entry);
        let Some(plan) = plan_dedupe(&issues, entry) else {
            continue;
        };
        if plan.close.is_empty() && plan.repoint_to.is_none() {
            continue;
        }
        apply_plan(ctx, &repo, entry, &plan, update)?;
    }

    if update && !ctx.dry_run {
        let count = lockfile.entries.len();
        super::write_lockfile(
            &lock_path,
            &Lockfile {
                version: 1,
                entries: lockfile.entries,
            },
        )?;
        taskit_output::taskit_ok!("todo-dedupe: lockfile updated ({count} tracked marker(s))");
    }
    Ok(())
}

/// Report an entry whose cited issue is closed. Never reopened here: the closure
/// may predate the work, and only a human can tell.
fn report_closed_citation(issues: &[IssueSummary], entry: &SyncedTodo) {
    let Some(number) = super::cited_issue_number(&entry.text) else {
        return;
    };
    let closed = issues
        .iter()
        .find(|issue| issue.number == number)
        .is_some_and(|issue| super::is_closed(&issue.state));
    if closed {
        taskit_output::taskit_warn!(
            "todo-dedupe: {} cites issue #{number}, which is closed; reopen it or drop the citation",
            entry.file
        );
    }
}

/// Close one entry's duplicates and repoint the lockfile entry at the survivor.
fn apply_plan(
    ctx: &Ctx,
    repo: &str,
    entry: &mut SyncedTodo,
    plan: &DedupePlan,
    update: bool,
) -> Result<(), TaskitError> {
    for number in &plan.close {
        if ctx.dry_run || !update {
            taskit_output::taskit_dry!("gh issue close {number} --repo {repo}");
            continue;
        }
        close_with_comment(ctx, repo, *number, plan.survivor)?;
        taskit_output::taskit_ok!(
            "todo-dedupe: closed duplicate #{number} for {} (tracked by #{})",
            entry.file,
            plan.survivor
        );
    }
    if let Some(survivor) = plan.repoint_to {
        let previous = entry.issue_number;
        entry.issue_number = survivor;
        taskit_output::taskit_ok!(
            "todo-dedupe: repointed {} from issue #{previous} to #{survivor}",
            entry.file
        );
    }
    Ok(())
}

/// Close `number`, leaving a breadcrumb to the issue that survives.
fn close_with_comment(
    ctx: &Ctx,
    repo: &str,
    number: u64,
    survivor: u64,
) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let number = number.to_string();
    let comment = format!(
        "Duplicate of #{survivor}: this marker is tracked by that issue. Closed by `taskit protocol todo-dedupe`."
    );
    ctx.run(cmd!(
        sh,
        "gh issue close {number} --repo {repo} --comment {comment}"
    ))
}

/// What to do about one marker's generated issues.
struct DedupePlan {
    /// Issue number that survives.
    survivor: u64,
    /// Issue numbers to close, survivor excluded.
    close: Vec<u64>,
    /// Set when the lockfile should point somewhere other than its current entry.
    repoint_to: Option<u64>,
}

/// Every *open* generated issue tracking `entry`, lowest number first.
///
/// Closed issues are excluded because they are already resolved: re-planning
/// them would make every run report the same phantom work and ask a human to
/// close the same issues forever. A closed issue that the lockfile still
/// points at is left to `report_closed_citation`, since only a human can tell
/// whether the work is genuinely done.
///
/// Returns `None` when no open issue matches, which is a missing-entry problem
/// for `todo-sync --update` to repair, not a duplication problem.
fn plan_dedupe(issues: &[IssueSummary], entry: &SyncedTodo) -> Option<DedupePlan> {
    let mut open: Vec<u64> = issues
        .iter()
        .filter(|issue| {
            issue.title == entry.text
                && super::body_tracks_file(&issue.body, &entry.file)
                && !super::is_closed(&issue.state)
        })
        .map(|issue| issue.number)
        .collect();
    open.sort_unstable();

    let survivor = *open.first()?;
    Some(DedupePlan {
        survivor,
        close: open.iter().copied().filter(|n| *n != survivor).collect(),
        repoint_to: (survivor != entry.issue_number).then_some(survivor),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::todo_sync::marker_tag;

    fn entry_for(file: &str) -> SyncedTodo {
        SyncedTodo {
            file: file.to_string(),
            text: "TODO(concurrent-state-updates): add a locked update API".to_string(),
            issue_number: 83,
        }
    }

    fn generated(issue: u64, entry: &SyncedTodo) -> IssueSummary {
        IssueSummary {
            number: issue,
            title: entry.text.clone(),
            body: marker_tag(&entry.file),
            state: "OPEN".to_string(),
        }
    }

    #[test]
    fn plan_keeps_the_lowest_number_and_repoints_the_entry() {
        let entry = entry_for("crates/taskit-engine/src/store.rs");
        let issues = vec![generated(83, &entry), generated(42, &entry)];
        let plan = plan_dedupe(&issues, &entry).expect("duplicate pair should plan");
        assert_eq!(plan.survivor, 42);
        assert_eq!(plan.close, vec![83]);
        assert_eq!(plan.repoint_to, Some(42));
    }

    #[test]
    fn a_single_issue_needs_no_action() {
        let entry = entry_for("crates/taskit-engine/src/store.rs");
        let plan = plan_dedupe(&[generated(83, &entry)], &entry).expect("one match should plan");
        assert_eq!(plan.survivor, 83);
        assert!(plan.close.is_empty());
        assert_eq!(plan.repoint_to, None);
    }

    #[test]
    fn an_issue_absent_from_the_lockfile_is_never_closed() {
        let entry = entry_for("crates/taskit-engine/src/store.rs");
        let human = IssueSummary {
            number: 9,
            title: entry.text.clone(),
            body: "written by a human".to_string(),
            state: "OPEN".to_string(),
        };
        assert!(plan_dedupe(&[human], &entry).is_none());
    }

    #[test]
    fn an_issue_for_a_different_file_is_never_closed() {
        let entry = entry_for("crates/taskit-engine/src/store.rs");
        let other = IssueSummary {
            number: 12,
            title: entry.text.clone(),
            body: marker_tag("crates/taskit-engine/src/other.rs"),
            state: "OPEN".to_string(),
        };
        assert!(plan_dedupe(&[other], &entry).is_none());
    }

    #[test]
    fn three_duplicates_close_all_but_the_lowest() {
        let entry = entry_for("crates/taskit-engine/src/store.rs");
        let issues = vec![
            generated(91, &entry),
            generated(12, &entry),
            generated(58, &entry),
        ];
        let plan = plan_dedupe(&issues, &entry).expect("three matches should plan");
        assert_eq!(plan.survivor, 12);
        assert_eq!(plan.close, vec![58, 91]);
    }

    fn closed(issue: u64, entry: &SyncedTodo) -> IssueSummary {
        IssueSummary {
            state: "CLOSED".to_string(),
            ..generated(issue, entry)
        }
    }

    #[test]
    fn an_already_closed_duplicate_is_never_closed_again() {
        // Regression shape: dedupe kept planning the same 41 closes on every
        // run, so a read-only run always looked like it had work to do.
        let mut entry = entry_for("crates/taskit-engine/src/store.rs");
        entry.issue_number = 42;
        let issues = vec![generated(42, &entry), closed(83, &entry)];
        let plan = plan_dedupe(&issues, &entry).expect("the open survivor still plans");
        assert_eq!(plan.survivor, 42);
        assert!(plan.close.is_empty(), "83 is already closed");
        assert_eq!(plan.repoint_to, None);
    }

    #[test]
    fn a_closed_survivor_repoints_onto_the_open_duplicate() {
        let mut entry = entry_for("crates/taskit-engine/src/store.rs");
        entry.issue_number = 42;
        let issues = vec![closed(42, &entry), generated(83, &entry)];
        let plan = plan_dedupe(&issues, &entry).expect("the open duplicate plans");
        assert_eq!(plan.survivor, 83);
        assert!(plan.close.is_empty());
        assert_eq!(plan.repoint_to, Some(83));
    }

    #[test]
    fn an_entry_with_only_closed_issues_plans_nothing() {
        let mut entry = entry_for("crates/taskit-engine/src/store.rs");
        entry.issue_number = 42;
        assert!(plan_dedupe(&[closed(42, &entry)], &entry).is_none());
    }
}
