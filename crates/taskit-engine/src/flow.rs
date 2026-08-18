// TODO(audit): 814 lines — split candidate.
use taskit_core::conflict_resolver::ConflictResolver;
use taskit_types::config::FlowConfig;
use taskit_types::conflict::ConflictFile;
use taskit_types::error::{FlowError, TaskitError};
use xshell::{Shell, cmd};

use crate::ctx::Ctx;

fn current_branch(sh: &Shell) -> Result<String, TaskitError> {
    Ok(cmd!(sh, "git branch --show-current")
        .read()
        .map_err(TaskitError::other)?
        .trim()
        .to_string())
}

fn branch_exists(sh: &Shell, branch: &str) -> Result<bool, TaskitError> {
    // `.ignore_status()` is required: xshell's `.output()` treats a nonzero
    // exit as an `Err` by default, but a nonexistent branch is exactly the
    // (non-error) `false` case this function needs to report.
    let result = cmd!(sh, "git rev-parse --verify --quiet {branch}")
        .quiet()
        .ignore_status()
        .output()
        .map_err(TaskitError::other)?;
    Ok(result.status.success())
}

/// A porcelain status line's path is everything after the 2-character status
/// code and the space that follows it (e.g. `" M .ctx/HANDOFF.foo.yaml"`).
/// For renames this is `"old/path -> new/path"`.
fn porcelain_path(line: &str) -> &str {
    line.get(3..).unwrap_or("")
}

/// Whether a porcelain status line only touches paths under `.ctx/` — safe to
/// ignore in the dirty-worktree check. Renames must have both the old and new
/// path under `.ctx/`, since a rename that moves a file *out* of `.ctx/`
/// changes a real tracked path even though its old path matched.
fn is_ctx_only(line: &str) -> bool {
    let path = porcelain_path(line);
    match path.split_once(" -> ") {
        Some((old, new)) => old.starts_with(".ctx/") && new.starts_with(".ctx/"),
        None => path.starts_with(".ctx/"),
    }
}

fn is_clean(sh: &Shell) -> Result<bool, TaskitError> {
    let output = cmd!(sh, "git status --porcelain")
        .read()
        .map_err(TaskitError::other)?;
    Ok(output.lines().all(is_ctx_only))
}

fn require_clean(sh: &Shell, branch: &str) -> Result<(), TaskitError> {
    if !is_clean(sh)? {
        return Err(FlowError::DirtyWorktree {
            branch: branch.to_string(),
        }
        .into());
    }
    Ok(())
}

fn require_branch(sh: &Shell, expected: &str) -> Result<(), TaskitError> {
    let actual = current_branch(sh)?;
    if actual != expected {
        return Err(FlowError::WrongBranch {
            expected: expected.to_string(),
            actual,
        }
        .into());
    }
    Ok(())
}

fn require_branch_exists(sh: &Shell, branch: &str) -> Result<(), TaskitError> {
    if !branch_exists(sh, branch)? {
        return Err(FlowError::MissingBranch {
            branch: branch.to_string(),
        }
        .into());
    }
    Ok(())
}

fn ahead_behind(sh: &Shell, local: &str, remote: &str) -> Result<(usize, usize), TaskitError> {
    let output = cmd!(sh, "git rev-list --left-right --count {local}...{remote}")
        .read()
        .map_err(TaskitError::other)?;
    let parts: Vec<&str> = output.split_whitespace().collect();
    if parts.len() != 2 {
        return Ok((0, 0));
    }
    let ahead = parts[0].parse().unwrap_or(0);
    let behind = parts[1].parse().unwrap_or(0);
    Ok((ahead, behind))
}

/// Parse `git status --porcelain` output for conflict markers (UU, AA, DD, AU, UA).
pub(crate) fn parse_conflict_paths(porcelain: &str) -> Vec<String> {
    porcelain
        .lines()
        .filter(|l| {
            l.starts_with("UU ")
                || l.starts_with("AA ")
                || l.starts_with("DD ")
                || l.starts_with("AU ")
                || l.starts_with("UA ")
        })
        .map(|l| l[3..].trim().to_string())
        .collect()
}

/// Read both sides of a conflicted file via `git show`.
pub(crate) fn read_conflict_file(sh: &Shell, path: &str) -> Result<ConflictFile, TaskitError> {
    let ours = cmd!(sh, "git show HEAD:{path}")
        .quiet()
        .read()
        .unwrap_or_default();
    let theirs = cmd!(sh, "git show MERGE_HEAD:{path}")
        .quiet()
        .read()
        .unwrap_or_default();
    let base = std::fs::read_to_string(path).ok();
    Ok(ConflictFile::new(path, ours, theirs, base))
}

/// Attempt a `--no-ff` merge; on conflict invoke `resolver`; on escalation return the error.
/// On successful resolution, stages all resolved files and completes the merge via
/// `git commit --no-edit`.
/// Returns the number of conflicted files resolved (0 on the fast,
/// no-conflict path).
pub fn merge_with_resolution(
    ctx: &Ctx,
    branch: &str,
    message: &str,
    resolver: &dyn ConflictResolver,
) -> Result<usize, TaskitError> {
    let sh = &ctx.sh;
    if ctx.dry_run {
        taskit_output::taskit_dry!("git merge --no-ff {branch} -m \"{message}\"");
        return Ok(0);
    }
    let output = cmd!(sh, "git merge --no-ff {branch} -m {message}")
        .quiet()
        .ignore_status()
        .output()
        .map_err(TaskitError::other)?;
    if output.status.success() {
        return Ok(0);
    }
    let porcelain = cmd!(sh, "git status --porcelain")
        .read()
        .map_err(TaskitError::other)?;
    let paths = parse_conflict_paths(&porcelain);
    if paths.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(FlowError::MergeFailed {
            reason: stderr.trim().to_string(),
        }
        .into());
    }
    let files: Vec<ConflictFile> = paths
        .iter()
        .map(|p| read_conflict_file(sh, p))
        .collect::<Result<_, _>>()?;
    let resolved = resolver.resolve(&files)?;
    let conflict_count = resolved.len();
    for r in &resolved {
        let abs_path = ctx.root.join(&r.path);
        std::fs::write(&abs_path, &r.content).map_err(TaskitError::other)?;
        let path = &r.path;
        cmd!(sh, "git add {path}")
            .run()
            .map_err(TaskitError::other)?;
    }
    cmd!(sh, "git commit --no-edit")
        .run()
        .map(|()| conflict_count)
        .map_err(|e| {
            FlowError::MergeFailed {
                reason: e.to_string(),
            }
            .into()
        })
}

fn merge_no_ff(ctx: &Ctx, branch: &str, message: &str) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    if ctx.dry_run {
        taskit_output::taskit_dry!("git merge --no-ff {branch} -m \"{message}\"");
        return Ok(());
    }
    let output = cmd!(sh, "git merge --no-ff {branch} -m {message}")
        .quiet()
        .output()
        .map_err(TaskitError::other)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(FlowError::MergeFailed {
            reason: stderr.trim().to_string(),
        }
        .into());
    }
    Ok(())
}

fn checkout(ctx: &Ctx, branch: &str) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    if ctx.dry_run {
        taskit_output::taskit_dry!("git checkout {branch}");
        return Ok(());
    }
    cmd!(sh, "git checkout {branch}")
        .quiet()
        .run()
        .map_err(TaskitError::other)?;
    Ok(())
}

fn sync_commit_message(main: &str, develop: &str) -> String {
    format!("flow: sync {main} into {develop}")
}

fn push_branches(ctx: &Ctx, remote: &str, branches: &[&str]) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    if ctx.dry_run {
        taskit_output::taskit_dry!("git push {remote} {}", branches.join(" "));
        return Ok(());
    }
    let output = cmd!(sh, "git push {remote} {branches...}")
        .quiet()
        .ignore_status()
        .output()
        .map_err(TaskitError::other)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(FlowError::PushFailed {
            remote: remote.to_string(),
            reason: stderr.trim().to_string(),
        }
        .into());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowHop {
    pub from: String,
    pub to: String,
    pub ahead: usize,
    pub behind: usize,
    pub branches_exist: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowStatusReport {
    pub current_branch: String,
    /// main→develop→staging→release→main, in that order.
    pub hops: Vec<FlowHop>,
}

/// Pure data variant of `status()` — `status()` calls this and prints each
/// hop, with no output change from before this function existed.
pub fn status_report(ctx: &Ctx, flow: &FlowConfig) -> Result<FlowStatusReport, TaskitError> {
    let sh = &ctx.sh;
    let main = flow.main_branch();
    let develop = flow.develop_branch();
    let staging = flow.staging_branch();
    let release = flow.release_branch();
    let current_branch = current_branch(sh)?;

    let mut hops = Vec::with_capacity(4);
    for (from, to) in [
        (main, develop),
        (develop, staging),
        (staging, release),
        (release, main),
    ] {
        if !branch_exists(sh, from)? || !branch_exists(sh, to)? {
            hops.push(FlowHop {
                from: from.to_string(),
                to: to.to_string(),
                ahead: 0,
                behind: 0,
                branches_exist: false,
            });
            continue;
        }
        let (ahead, behind) = ahead_behind(sh, from, to)?;
        hops.push(FlowHop {
            from: from.to_string(),
            to: to.to_string(),
            ahead,
            behind,
            branches_exist: true,
        });
    }

    Ok(FlowStatusReport {
        current_branch,
        hops,
    })
}

pub fn status(ctx: &Ctx, flow: &FlowConfig) -> Result<(), TaskitError> {
    let report = status_report(ctx, flow)?;

    taskit_output::taskit_progress!("Flow status (current branch: {})", report.current_branch);
    taskit_output::taskit_progress!("");

    for hop in &report.hops {
        if !hop.branches_exist {
            taskit_output::taskit_progress!("{} -> {}: (branch missing)", hop.from, hop.to);
        } else {
            taskit_output::taskit_progress!(
                "{} -> {}: {} ahead, {} behind",
                hop.from,
                hop.to,
                hop.ahead,
                hop.behind
            );
        }
    }
    Ok(())
}

/// Merge main into develop, bringing in the latest stable changes.
pub fn sync(ctx: &Ctx, flow: &FlowConfig) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let main = flow.main_branch();
    let develop = flow.develop_branch();

    require_branch(sh, develop)?;
    require_clean(sh, develop)?;
    require_branch_exists(sh, main)?;

    taskit_output::taskit_progress!("Syncing {main} -> {develop}");
    merge_no_ff(ctx, main, &sync_commit_message(main, develop))?;
    taskit_output::taskit_ok!("Done. {develop} is up to date with {main}.");
    Ok(())
}

/// Advance work from develop to staging.
/// Advance the current branch one step in the pipeline.
///
/// - `develop`  → merges into `staging`
/// - `staging`  → merges into `release`
/// - `release`  → merges into `main`, then syncs `main` back into `develop`
pub fn promote(ctx: &Ctx, flow: &FlowConfig) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let branch = current_branch(sh)?;

    let develop = flow.develop_branch();
    let staging = flow.staging_branch();
    let release = flow.release_branch();
    let main = flow.main_branch();

    if branch == develop {
        require_clean(sh, develop)?;
        require_branch_exists(sh, staging)?;
        taskit_output::taskit_progress!("Promoting {develop} -> {staging}");
        checkout(ctx, staging)?;
        merge_no_ff(
            ctx,
            develop,
            &format!("flow: promote {develop} into {staging}"),
        )?;
        taskit_output::taskit_ok!("Done. Now on {staging}.");
    } else if branch == staging {
        require_clean(sh, staging)?;
        require_branch_exists(sh, release)?;
        taskit_output::taskit_progress!("Promoting {staging} -> {release}");
        checkout(ctx, release)?;
        merge_no_ff(
            ctx,
            staging,
            &format!("flow: promote {staging} into {release}"),
        )?;
        taskit_output::taskit_ok!("Done. Now on {release}.");
    } else if branch == release {
        require_clean(sh, release)?;
        require_branch_exists(sh, main)?;
        require_branch_exists(sh, develop)?;
        taskit_output::taskit_progress!(
            "Promoting {release} -> {main}, then syncing {main} -> {develop}"
        );
        checkout(ctx, main)?;
        merge_no_ff(
            ctx,
            release,
            &format!("flow: promote {release} into {main}"),
        )?;
        checkout(ctx, develop)?;
        merge_no_ff(ctx, main, &sync_commit_message(main, develop))?;
        taskit_output::taskit_ok!("Done. Now on {develop}. All branches are in sync.");
    } else {
        return Err(FlowError::NotAFlowBranch {
            branch,
            develop: develop.to_string(),
            staging: staging.to_string(),
            release: release.to_string(),
        }
        .into());
    }

    Ok(())
}

pub fn guard(ctx: &Ctx, flow: &FlowConfig) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let current = current_branch(sh)?;
    let main = flow.main_branch();
    let release = flow.release_branch();

    if current == main || current == release {
        return Err(FlowError::ProtectedBranch {
            branch: current,
            staging: flow.staging_branch().to_string(),
        }
        .into());
    }
    Ok(())
}

/// Run the full promote → stage → CI gate → finish pipeline with LLM-assisted conflict
/// resolution.
///
/// Sequence:
/// 1. Verify develop is clean and staging/release/main exist.
/// 2. Promote: merge develop → staging (with conflict resolution).
/// 3. Stage: merge staging → release (with conflict resolution).
/// 4. CI gate: run default pipeline on release; abort if any step fails.
/// 5. Finish: merge release → main, sync main → develop (with conflict resolution).
pub fn auto(
    ctx: &Ctx,
    flow: &FlowConfig,
    resolver: &dyn ConflictResolver,
) -> Result<(), TaskitError> {
    auto_with_ci(ctx, flow, resolver, |c| {
        crate::ci::run_default_internal(c, true, false)
    })
}

/// Internal entry point for `flow auto`, injectable CI function for testing.
///
/// Resumes from `.taskit-state.json` if present, skipping phases already completed.
pub fn auto_with_ci(
    ctx: &Ctx,
    flow: &FlowConfig,
    resolver: &dyn ConflictResolver,
    run_ci: impl Fn(&Ctx) -> taskit_types::step::PipelineOutcome,
) -> Result<(), TaskitError> {
    use taskit_types::flow_state::{FlowPhase, FlowState};
    use taskit_types::step::StepStatus;

    let start = std::time::Instant::now();
    let mut conflicts_resolved: usize = 0;

    let sh = &ctx.sh;
    let develop = flow.develop_branch();
    let staging = flow.staging_branch();
    let release = flow.release_branch();
    let main = flow.main_branch();

    // Load any persisted state from a prior interrupted run.
    let saved = crate::flow_state_store::load(&ctx.root);

    let resume_phase = saved.as_ref().map(|s| &s.phase);
    if let Some(phase) = resume_phase {
        taskit_output::taskit_progress!("auto: resuming from {phase:?}");
    }

    // ── Phase 1: Promoting ────────────────────────────────────────────────

    if resume_phase.is_none() || resume_phase == Some(&FlowPhase::Promoting) {
        if resume_phase.is_none() {
            // Fresh run — validate preconditions.
            require_branch(sh, develop)?;
            require_clean(sh, develop)?;
            require_branch_exists(sh, staging)?;
            require_branch_exists(sh, release)?;
            require_branch_exists(sh, main)?;

            let state = FlowState::promoting(staging, release, main);
            if !ctx.dry_run {
                crate::flow_state_store::save(&ctx.root, &state)?;
            }
        }

        taskit_output::taskit_progress!("auto: promoting {develop} → {staging}");
        checkout(ctx, staging)?;
        conflicts_resolved += merge_with_resolution(
            ctx,
            develop,
            &format!("flow: promote {develop} into {staging}"),
            resolver,
        )?;

        taskit_output::taskit_progress!("auto: staging {staging} → {release}");
        checkout(ctx, release)?;
        conflicts_resolved += merge_with_resolution(
            ctx,
            staging,
            &format!("flow: stage {staging} into {release}"),
            resolver,
        )?;

        // Advance state to CiGate.
        if !ctx.dry_run {
            let state = FlowState {
                phase: FlowPhase::CiGate,
                staging: staging.to_string(),
                release: release.to_string(),
                main: main.to_string(),
                merge_sha: None,
                failed_steps: vec![],
            };
            crate::flow_state_store::save(&ctx.root, &state)?;
        }
    }

    // ── Phase 2: CI gate ─────────────────────────────────────────────────

    if resume_phase.is_none()
        || resume_phase == Some(&FlowPhase::Promoting)
        || resume_phase == Some(&FlowPhase::CiGate)
    {
        taskit_output::taskit_progress!("auto: running CI on {release}");
        let outcome = run_ci(ctx);
        if !outcome.passed {
            let failed: Vec<String> = outcome
                .results
                .iter()
                .filter(|s| s.status == StepStatus::Fail)
                .map(|s| s.name.clone())
                .collect();
            // Persist failure state so the user can re-run after fixing CI.
            if !ctx.dry_run {
                let state = FlowState {
                    phase: FlowPhase::CiGate,
                    staging: staging.to_string(),
                    release: release.to_string(),
                    main: main.to_string(),
                    merge_sha: None,
                    failed_steps: failed.clone(),
                };
                crate::flow_state_store::save(&ctx.root, &state)?;
            }
            taskit_output::taskit_err!(
                "auto: CI failed on {release} — staying on {release} for investigation"
            );
            let _ = crate::telemetry::record(
                ctx,
                &[
                    ("flow_auto_duration_ms", start.elapsed().as_millis() as f64),
                    ("flow_auto_result", 0.0),
                    ("flow_auto_conflicts", conflicts_resolved as f64),
                ],
            );
            return Err(FlowError::CiFailed { failed }.into());
        }
        taskit_output::taskit_ok!("auto: CI passed on {release}");

        // Advance to Finishing.
        if !ctx.dry_run {
            let state = FlowState {
                phase: FlowPhase::Finishing,
                staging: staging.to_string(),
                release: release.to_string(),
                main: main.to_string(),
                merge_sha: None,
                failed_steps: vec![],
            };
            crate::flow_state_store::save(&ctx.root, &state)?;
        }
    }

    // ── Phase 3: Finishing ────────────────────────────────────────────────

    taskit_output::taskit_progress!("auto: finishing {release} → {main}");
    checkout(ctx, main)?;
    conflicts_resolved += merge_with_resolution(
        ctx,
        release,
        &format!("flow: finish {release} into {main}"),
        resolver,
    )?;

    taskit_output::taskit_progress!("auto: syncing {main} → {develop}");
    checkout(ctx, develop)?;
    conflicts_resolved +=
        merge_with_resolution(ctx, main, &sync_commit_message(main, develop), resolver)?;

    // Push before clearing state: a failed push leaves the run resumable at
    // Finishing, where the merges no-op and the push is retried.
    if flow.push_enabled() {
        let remote = flow.push_remote();
        taskit_output::taskit_progress!(
            "auto: pushing {main}, {develop}, {staging}, {release} → {remote}"
        );
        push_branches(ctx, remote, &[main, develop, staging, release])?;
    }

    // Success — clear the state file.
    if !ctx.dry_run {
        crate::flow_state_store::clear(&ctx.root)?;
    }

    let _ = crate::telemetry::record(
        ctx,
        &[
            ("flow_auto_duration_ms", start.elapsed().as_millis() as f64),
            ("flow_auto_result", 1.0),
            ("flow_auto_conflicts", conflicts_resolved as f64),
        ],
    );

    taskit_output::taskit_ok!("auto: done. {develop} is in sync with {main}.");
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests_support {
    use super::*;
    use taskit_types::config::Config;
    use taskit_types::output_format::OutputFormat;

    /// Minimal repo with `main`/`staging`/`release`, `staging` one commit
    /// ahead — enough for a fast-path (no-conflict) merge.
    pub(crate) fn merge_test_repo() -> (tempfile::TempDir, Ctx, FlowConfig) {
        let dir = tempfile::tempdir().expect("tempdir");
        let sh = Shell::new().expect("shell");
        sh.change_dir(dir.path());
        cmd!(sh, "git init -b main").run().expect("git init");
        cmd!(sh, "git config user.email test@example.com")
            .run()
            .expect("email");
        cmd!(sh, "git config user.name Test").run().expect("name");
        sh.write_file("README.md", "# test\n").expect("write");
        cmd!(sh, "git add README.md").run().expect("add");
        cmd!(sh, "git commit -m init").run().expect("commit");
        cmd!(sh, "git branch staging")
            .run()
            .expect("branch staging");
        cmd!(sh, "git branch release")
            .run()
            .expect("branch release");
        cmd!(sh, "git checkout staging")
            .run()
            .expect("checkout staging");
        sh.write_file("feature.txt", "feature\n").expect("write");
        cmd!(sh, "git add feature.txt").run().expect("add");
        cmd!(sh, "git commit -m feat").run().expect("commit");
        cmd!(sh, "git checkout release")
            .run()
            .expect("checkout release");

        let ctx = Ctx::new(
            sh,
            dir.path().to_path_buf(),
            Config::default(),
            false,
            OutputFormat::Human,
        );
        (dir, ctx, FlowConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taskit_types::conflict::ResolvedFile;

    fn default_flow() -> FlowConfig {
        FlowConfig::default()
    }

    struct AlwaysResolve;
    impl ConflictResolver for AlwaysResolve {
        fn resolve(&self, files: &[ConflictFile]) -> Result<Vec<ResolvedFile>, TaskitError> {
            Ok(files
                .iter()
                .map(|f| ResolvedFile::new(f.path.clone(), "resolved\n"))
                .collect())
        }
    }

    struct AlwaysEscalate;
    impl ConflictResolver for AlwaysEscalate {
        fn resolve(&self, files: &[ConflictFile]) -> Result<Vec<ResolvedFile>, TaskitError> {
            Err(FlowError::NeedsHuman {
                path: files.first().map(|f| f.path.clone()).unwrap_or_default(),
                reason: "too complex".into(),
            }
            .into())
        }
    }

    #[test]
    fn porcelain_path_strips_status_code() {
        assert_eq!(
            porcelain_path(" M .ctx/HANDOFF.foo.yaml"),
            ".ctx/HANDOFF.foo.yaml"
        );
        assert_eq!(porcelain_path("?? src/lib.rs"), "src/lib.rs");
        assert_eq!(porcelain_path(""), "");
    }

    #[test]
    fn is_ctx_only_true_for_plain_ctx_path() {
        assert!(is_ctx_only(" M .ctx/HANDOFF.foo.yaml"));
        assert!(is_ctx_only("?? .ctx/scratch.txt"));
    }

    #[test]
    fn is_ctx_only_false_for_non_ctx_path() {
        assert!(!is_ctx_only(" M src/lib.rs"));
        assert!(!is_ctx_only("?? new.txt"));
    }

    #[test]
    fn is_ctx_only_true_for_rename_within_ctx() {
        assert!(is_ctx_only(
            "R  .ctx/HANDOFF.old.yaml -> .ctx/HANDOFF.new.yaml"
        ));
    }

    #[test]
    fn is_ctx_only_false_for_rename_out_of_ctx() {
        // Old path matched .ctx/, but the rename moves it to a real tracked
        // path — must not be silently ignored.
        assert!(!is_ctx_only("R  .ctx/HANDOFF.foo.yaml -> src/handoff.yaml"));
    }

    #[test]
    fn is_ctx_only_false_for_rename_into_ctx() {
        assert!(!is_ctx_only("R  src/handoff.yaml -> .ctx/HANDOFF.foo.yaml"));
    }

    #[test]
    fn parse_conflict_paths_empty_on_clean() {
        assert!(parse_conflict_paths("").is_empty());
        assert!(parse_conflict_paths("M  src/lib.rs\n?? new.txt\n").is_empty());
    }

    #[test]
    fn parse_conflict_paths_detects_uu_aa_dd() {
        let porcelain = "UU src/lib.rs\nAA Cargo.toml\nDD old.rs\nM  clean.rs\n";
        let paths = parse_conflict_paths(porcelain);
        assert_eq!(paths, vec!["src/lib.rs", "Cargo.toml", "old.rs"]);
    }

    #[test]
    fn parse_conflict_paths_detects_au_ua() {
        let porcelain = "AU src/main.rs\nUA Cargo.lock\nM  clean.rs\n";
        let paths = parse_conflict_paths(porcelain);
        assert_eq!(paths, vec!["src/main.rs", "Cargo.lock"]);
    }

    #[test]
    fn conflict_resolver_fake_resolves() {
        let result =
            AlwaysResolve.resolve(&[ConflictFile::new("src/lib.rs", "ours", "theirs", None)]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap()[0].content, "resolved\n");
    }

    #[test]
    fn conflict_resolver_fake_escalates() {
        let result = AlwaysEscalate.resolve(&[ConflictFile::new("src/lib.rs", "a", "b", None)]);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("src/lib.rs"), "got: {msg}");
    }

    #[test]
    fn merge_with_resolution_fast_path_returns_zero_conflicts() {
        let (_dir, ctx, _flow) = tests_support::merge_test_repo();
        // No conflict: `staging` merges cleanly into `release`, so the
        // resolver must not be called.
        let count = merge_with_resolution(&ctx, "staging", "flow: fast path", &AlwaysResolve)
            .expect("fast-path merge should succeed");
        assert_eq!(count, 0);
    }

    #[test]
    fn default_branch_names() {
        let f = default_flow();
        assert_eq!(f.main_branch(), "main");
        assert_eq!(f.staging_branch(), "staging");
        assert_eq!(f.release_branch(), "release");
    }

    #[test]
    fn custom_branch_names() {
        let f = FlowConfig {
            main: Some("production".into()),
            develop: Some("work".into()),
            staging: Some("int".into()),
            release: Some("rc".into()),
            ..Default::default()
        };
        assert_eq!(f.main_branch(), "production");
        assert_eq!(f.develop_branch(), "work");
        assert_eq!(f.staging_branch(), "int");
        assert_eq!(f.release_branch(), "rc");
    }

    #[test]
    fn wrong_branch_error_display() {
        let err = FlowError::WrongBranch {
            expected: "staging".into(),
            actual: "main".into(),
        };
        assert!(err.to_string().contains("expected 'staging'"));
        assert!(err.to_string().contains("got 'main'"));
    }

    #[test]
    fn wrong_branch_diagnostic_code() {
        use miette::Diagnostic;
        let err = FlowError::WrongBranch {
            expected: "staging".into(),
            actual: "main".into(),
        };
        let code = err.code().expect("should have code");
        assert_eq!(code.to_string(), "taskit::flow::wrong_branch");
    }

    #[test]
    fn protected_branch_error_display() {
        let err = FlowError::ProtectedBranch {
            branch: "main".into(),
            staging: "staging".into(),
        };
        assert!(err.to_string().contains("protected"));
        assert!(err.to_string().contains("main"));
    }

    #[test]
    fn protected_branch_diagnostic_code() {
        use miette::Diagnostic;
        let err = FlowError::ProtectedBranch {
            branch: "main".into(),
            staging: "staging".into(),
        };
        let code = err.code().expect("should have code");
        assert_eq!(code.to_string(), "taskit::flow::protected");
    }

    #[test]
    fn missing_branch_error_display() {
        let err = FlowError::MissingBranch {
            branch: "release".into(),
        };
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn dirty_worktree_error_display() {
        let err = FlowError::DirtyWorktree {
            branch: "staging".into(),
        };
        assert!(err.to_string().contains("uncommitted changes"));
    }

    #[test]
    fn merge_failed_error_display() {
        let err = FlowError::MergeFailed {
            reason: "conflict in Cargo.toml".into(),
        };
        assert!(err.to_string().contains("merge failed"));
        assert!(err.to_string().contains("conflict"));
    }

    #[test]
    fn merge_failed_diagnostic_code() {
        use miette::Diagnostic;
        let err = FlowError::MergeFailed {
            reason: "conflict".into(),
        };
        let code = err.code().expect("should have code");
        assert_eq!(code.to_string(), "taskit::flow::merge_failed");
    }

    #[test]
    fn flow_config_parses_from_toml() {
        let cfg: FlowConfig = toml::from_str(
            r#"
main = "production"
develop = "work"
staging = "int"
release = "rc"
"#,
        )
        .unwrap();
        assert_eq!(cfg.main_branch(), "production");
        assert_eq!(cfg.develop_branch(), "work");
        assert_eq!(cfg.staging_branch(), "int");
        assert_eq!(cfg.release_branch(), "rc");
    }

    #[test]
    fn flow_config_parses_empty_toml() {
        let cfg: FlowConfig = toml::from_str("").unwrap();
        assert_eq!(cfg.main_branch(), "main");
        assert_eq!(cfg.develop_branch(), "develop");
        assert_eq!(cfg.staging_branch(), "staging");
        assert_eq!(cfg.release_branch(), "release");
    }

    #[test]
    fn flow_config_push_defaults_off() {
        let cfg: FlowConfig = toml::from_str("").unwrap();
        assert!(!cfg.push_enabled());
        assert_eq!(cfg.push_remote(), "origin");
    }

    #[test]
    fn flow_config_push_parses() {
        let cfg: FlowConfig = toml::from_str(
            r#"
push = true
remote = "github"
"#,
        )
        .unwrap();
        assert!(cfg.push_enabled());
        assert_eq!(cfg.push_remote(), "github");
    }

    #[test]
    fn push_failed_error_display() {
        let err = FlowError::PushFailed {
            remote: "origin".into(),
            reason: "connection refused".into(),
        };
        assert!(err.to_string().contains("push to 'origin' failed"));
        assert!(err.to_string().contains("connection refused"));
    }

    #[test]
    fn flow_config_partial_override() {
        let cfg: FlowConfig = toml::from_str(r#"develop = "dev""#).unwrap();
        assert_eq!(cfg.main_branch(), "main");
        assert_eq!(cfg.develop_branch(), "dev");
        assert_eq!(cfg.staging_branch(), "staging");
        assert_eq!(cfg.release_branch(), "release");
    }

    #[test]
    fn status_report_flags_missing_branches_and_computes_ahead_behind() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sh = xshell::Shell::new().expect("shell");
        sh.change_dir(dir.path());
        cmd!(sh, "git init -b main").run().expect("git init");
        cmd!(sh, "git config user.email test@example.com")
            .run()
            .expect("email");
        cmd!(sh, "git config user.name Test").run().expect("name");
        sh.write_file("README.md", "# test\n").expect("write");
        cmd!(sh, "git add README.md").run().expect("add");
        cmd!(sh, "git commit -m init").run().expect("commit");
        cmd!(sh, "git branch develop")
            .run()
            .expect("branch develop");
        // staging/release intentionally not created.

        let ctx = Ctx::new(
            sh,
            dir.path().to_path_buf(),
            taskit_types::config::Config::default(),
            false,
            taskit_types::output_format::OutputFormat::Human,
        );
        let flow = default_flow();

        let report = status_report(&ctx, &flow).expect("status_report should succeed");
        assert_eq!(report.current_branch, "main");
        assert_eq!(report.hops.len(), 4);

        let main_to_develop = &report.hops[0];
        assert_eq!(main_to_develop.from, "main");
        assert_eq!(main_to_develop.to, "develop");
        assert!(main_to_develop.branches_exist);
        assert_eq!(main_to_develop.ahead, 0);
        assert_eq!(main_to_develop.behind, 0);

        let develop_to_staging = &report.hops[1];
        assert!(
            !develop_to_staging.branches_exist,
            "staging doesn't exist yet"
        );
    }

    // ── auto_with_ci tests (CI gate path) ─────────────────────────────────────

    fn setup_auto_repo() -> (tempfile::TempDir, Ctx, FlowConfig) {
        use taskit_types::config::Config;
        use taskit_types::output_format::OutputFormat;
        let dir = tempfile::tempdir().expect("tempdir");
        let sh = xshell::Shell::new().expect("shell");
        sh.change_dir(dir.path());
        cmd!(sh, "git init -b main").run().expect("git init");
        cmd!(sh, "git config user.email test@example.com")
            .run()
            .expect("email");
        cmd!(sh, "git config user.name Test").run().expect("name");
        cmd!(sh, "git config core.hooksPath /dev/null")
            .run()
            .expect("disable hooks");
        sh.write_file("README.md", "# test\n").expect("write");
        cmd!(sh, "git add README.md").run().expect("add");
        cmd!(sh, "git commit -m init").run().expect("commit");
        cmd!(sh, "git branch develop")
            .run()
            .expect("branch develop");
        cmd!(sh, "git branch staging")
            .run()
            .expect("branch staging");
        cmd!(sh, "git branch release")
            .run()
            .expect("branch release");
        cmd!(sh, "git checkout develop")
            .run()
            .expect("checkout develop");
        // Add a commit so promote has something to merge.
        sh.write_file("feature.txt", "feature\n").expect("write");
        cmd!(sh, "git add feature.txt").run().expect("add");
        cmd!(sh, "git commit -m feature").run().expect("commit");
        let flow = FlowConfig::default();
        let ctx = Ctx::new(
            sh,
            dir.path().to_path_buf(),
            Config::default(),
            false,
            OutputFormat::Human,
        );
        (dir, ctx, flow)
    }

    #[test]
    fn auto_ci_failure_returns_ci_failed_and_stays_on_release() {
        use taskit_types::error::FlowError;
        use taskit_types::step::{PipelineOutcome, StepResult, StepStatus};

        let (_dir, ctx, flow) = setup_auto_repo();

        // Inject a CI function that always reports a failed step.
        let failing_ci = |_: &Ctx| PipelineOutcome {
            results: vec![StepResult {
                name: "fmt".into(),
                status: StepStatus::Fail,
                duration: std::time::Duration::ZERO,
                error: Some("formatting errors".into()),
                gate: false,
                diagnostics: vec![],
                context: taskit_types::step::StepDiagnosticContext::default(),
            }],
            ..Default::default()
        };

        let result = auto_with_ci(&ctx, &flow, &AlwaysResolve, failing_ci);

        match result {
            Err(taskit_types::error::TaskitError::Flow(FlowError::CiFailed { failed })) => {
                assert_eq!(failed, vec!["fmt".to_string()]);
            }
            other => panic!("expected CiFailed, got {other:?}"),
        }

        // After CI failure, we should still be on release (not main).
        let branch = cmd!(ctx.sh, "git branch --show-current")
            .read()
            .expect("branch");
        assert_eq!(
            branch.trim(),
            "release",
            "should stay on release after CI failure"
        );

        use crate::telemetry::TelemetryStore;
        let store = crate::telemetry::NdjsonStore::new(ctx.root.clone());
        let records = store.load_window(7).expect("load telemetry window");
        let last = records
            .last()
            .expect("a flow_auto telemetry record should have been written");
        let metric = |name: &str| {
            last.metrics
                .iter()
                .find(|m| m.name == name)
                .unwrap_or_else(|| panic!("missing metric {name}"))
                .value
        };
        assert_eq!(metric("flow_auto_result"), 0.0);
        assert!(metric("flow_auto_duration_ms") >= 0.0);
    }

    fn passing_ci_fn() -> impl Fn(&Ctx) -> taskit_types::step::PipelineOutcome {
        use taskit_types::step::{PipelineOutcome, StepResult, StepStatus};
        |_: &Ctx| PipelineOutcome {
            results: vec![StepResult {
                name: "fmt".into(),
                status: StepStatus::Pass,
                duration: std::time::Duration::ZERO,
                error: None,
                gate: false,
                diagnostics: vec![],
                context: taskit_types::step::StepDiagnosticContext::default(),
            }],
            passed: true,
            ..Default::default()
        }
    }

    #[test]
    fn auto_with_push_updates_remote_branches() {
        let (_dir, ctx, mut flow) = setup_auto_repo();
        let bare = tempfile::tempdir().expect("bare tempdir");
        let bare_path = bare.path().to_str().expect("utf8 path").to_string();
        cmd!(ctx.sh, "git init --bare {bare_path}")
            .run()
            .expect("init bare");
        cmd!(ctx.sh, "git remote add origin {bare_path}")
            .run()
            .expect("add remote");
        flow.push = Some(true);

        auto_with_ci(&ctx, &flow, &AlwaysResolve, passing_ci_fn())
            .expect("auto with push should succeed");

        let refs = cmd!(ctx.sh, "git ls-remote --heads origin")
            .read()
            .expect("ls-remote");
        for branch in ["main", "develop", "staging", "release"] {
            assert!(
                refs.contains(&format!("refs/heads/{branch}")),
                "remote should have {branch}, got:\n{refs}"
            );
        }
    }

    #[test]
    fn auto_push_failure_returns_push_failed_and_keeps_state() {
        use taskit_types::error::FlowError;

        let (_dir, ctx, mut flow) = setup_auto_repo();
        // No `origin` remote exists — the push must fail after local merges.
        flow.push = Some(true);

        let result = auto_with_ci(&ctx, &flow, &AlwaysResolve, passing_ci_fn());
        match result {
            Err(taskit_types::error::TaskitError::Flow(FlowError::PushFailed { .. })) => {}
            other => panic!("expected PushFailed, got {other:?}"),
        }
        assert!(
            crate::flow_state_store::load(&ctx.root).is_some(),
            "state file should survive a failed push so the run is resumable"
        );
    }

    #[test]
    fn auto_ci_pass_completes_finish() {
        use taskit_types::step::{PipelineOutcome, StepResult, StepStatus};

        let (_dir, ctx, flow) = setup_auto_repo();

        let passing_ci = |_: &Ctx| PipelineOutcome {
            results: vec![StepResult {
                name: "fmt".into(),
                status: StepStatus::Pass,
                duration: std::time::Duration::ZERO,
                error: None,
                gate: false,
                diagnostics: vec![],
                context: taskit_types::step::StepDiagnosticContext::default(),
            }],
            passed: true,
            ..Default::default()
        };

        let result = auto_with_ci(&ctx, &flow, &AlwaysResolve, passing_ci);
        assert!(
            result.is_ok(),
            "auto should complete when CI passes: {result:?}"
        );

        // After success we should be on develop.
        let branch = cmd!(ctx.sh, "git branch --show-current")
            .read()
            .expect("branch");
        assert_eq!(
            branch.trim(),
            "develop",
            "should land on develop after auto completes"
        );

        use crate::telemetry::TelemetryStore;
        let store = crate::telemetry::NdjsonStore::new(ctx.root.clone());
        let records = store.load_window(7).expect("load telemetry window");
        let last = records
            .last()
            .expect("a flow_auto telemetry record should have been written");
        let metric = |name: &str| {
            last.metrics
                .iter()
                .find(|m| m.name == name)
                .unwrap_or_else(|| panic!("missing metric {name}"))
                .value
        };
        assert_eq!(metric("flow_auto_result"), 1.0);
        assert!(metric("flow_auto_duration_ms") >= 0.0);
        assert_eq!(metric("flow_auto_conflicts"), 0.0);
    }
}
