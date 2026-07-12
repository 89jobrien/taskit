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
    let result = cmd!(sh, "git rev-parse --verify --quiet {branch}")
        .quiet()
        .output()
        .map_err(TaskitError::other)?;
    Ok(result.status.success())
}

fn is_clean(sh: &Shell) -> Result<bool, TaskitError> {
    let output = cmd!(sh, "git status --porcelain")
        .read()
        .map_err(TaskitError::other)?;
    Ok(output.trim().is_empty())
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
pub fn merge_with_resolution(
    ctx: &Ctx,
    branch: &str,
    message: &str,
    resolver: &dyn ConflictResolver,
) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    if ctx.dry_run {
        taskit_output::taskit_dry!("git merge --no-ff {branch} -m \"{message}\"");
        return Ok(());
    }
    let output = cmd!(sh, "git merge --no-ff {branch} -m {message}")
        .quiet()
        .ignore_status()
        .output()
        .map_err(TaskitError::other)?;
    if output.status.success() {
        return Ok(());
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
    for r in &resolved {
        let abs_path = ctx.root.join(&r.path);
        std::fs::write(&abs_path, &r.content).map_err(TaskitError::other)?;
        let path = &r.path;
        cmd!(sh, "git add {path}")
            .run()
            .map_err(TaskitError::other)?;
    }
    cmd!(sh, "git commit --no-edit").run().map_err(|e| {
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

pub fn status(ctx: &Ctx, flow: &FlowConfig) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let main = flow.main_branch();
    let develop = flow.develop_branch();
    let staging = flow.staging_branch();
    let release = flow.release_branch();
    let current = current_branch(sh)?;

    taskit_output::taskit_progress!("Flow status (current branch: {current})");
    taskit_output::taskit_progress!("");

    // main → develop → staging → release → main
    for (from, to) in [
        (main, develop),
        (develop, staging),
        (staging, release),
        (release, main),
    ] {
        if !branch_exists(sh, from)? || !branch_exists(sh, to)? {
            taskit_output::taskit_progress!("{from} -> {to}: (branch missing)");
            continue;
        }
        let (ahead, behind) = ahead_behind(sh, from, to)?;
        taskit_output::taskit_progress!("{from} -> {to}: {ahead} ahead, {behind} behind");
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
    merge_no_ff(ctx, main, &format!("flow: sync {main} into {develop}"))?;
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
        merge_no_ff(ctx, main, &format!("flow: sync {main} into {develop}"))?;
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
        merge_with_resolution(
            ctx,
            develop,
            &format!("flow: promote {develop} into {staging}"),
            resolver,
        )?;

        taskit_output::taskit_progress!("auto: staging {staging} → {release}");
        checkout(ctx, release)?;
        merge_with_resolution(
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
    merge_with_resolution(
        ctx,
        release,
        &format!("flow: finish {release} into {main}"),
        resolver,
    )?;

    taskit_output::taskit_progress!("auto: syncing {main} → {develop}");
    checkout(ctx, develop)?;
    merge_with_resolution(
        ctx,
        main,
        &format!("flow: sync {main} into {develop}"),
        resolver,
    )?;

    // Success — clear the state file.
    if !ctx.dry_run {
        crate::flow_state_store::clear(&ctx.root)?;
    }

    taskit_output::taskit_ok!("auto: done. {develop} is in sync with {main}.");
    Ok(())
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
    fn flow_config_partial_override() {
        let cfg: FlowConfig = toml::from_str(r#"develop = "dev""#).unwrap();
        assert_eq!(cfg.main_branch(), "main");
        assert_eq!(cfg.develop_branch(), "dev");
        assert_eq!(cfg.staging_branch(), "staging");
        assert_eq!(cfg.release_branch(), "release");
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
    }
}
