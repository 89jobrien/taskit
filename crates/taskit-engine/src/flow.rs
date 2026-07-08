use taskit_types::config::FlowConfig;
use taskit_types::error::{FlowError, TaskitError};
use xshell::{Shell, cmd};

use crate::ctx::Ctx;

/// A file with merge conflicts, with both sides captured for resolution.
#[derive(Debug)]
#[non_exhaustive]
pub struct ConflictFile {
    pub path: String,
    pub ours: String,
    pub theirs: String,
    /// The raw file content including conflict markers (base context).
    pub base: Option<String>,
}

/// A file with its conflict resolved to a final content string.
#[derive(Debug)]
#[non_exhaustive]
pub struct ResolvedFile {
    pub path: String,
    pub content: String,
}

/// Port for resolving merge conflicts — implemented by `BamlConflictResolver` in the binary.
pub trait ConflictResolver {
    fn resolve(&self, files: &[ConflictFile]) -> Result<Vec<ResolvedFile>, TaskitError>;
}

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
    Ok(ConflictFile {
        path: path.to_string(),
        ours,
        theirs,
        base,
    })
}

/// Attempt a `--no-ff` merge; on conflict invoke `resolver`; on escalation return the error.
/// On successful resolution, stages all resolved files and completes the merge via
/// `git commit --no-edit`.
pub(crate) fn merge_with_resolution(
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
        std::fs::write(&r.path, &r.content).map_err(TaskitError::other)?;
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
    let staging = flow.staging_branch();
    let release = flow.release_branch();
    let current = current_branch(sh)?;

    taskit_output::taskit_progress!("Flow status (current branch: {current})");
    taskit_output::taskit_progress!("");

    for (from, to) in [(staging, main), (release, staging), (main, release)] {
        if !branch_exists(sh, from)? || !branch_exists(sh, to)? {
            taskit_output::taskit_progress!("{from} -> {to}: (branch missing)");
            continue;
        }
        let (ahead, behind) = ahead_behind(sh, from, to)?;
        taskit_output::taskit_progress!("{from} -> {to}: {ahead} ahead, {behind} behind");
    }
    Ok(())
}

pub fn promote(ctx: &Ctx, flow: &FlowConfig) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let staging = flow.staging_branch();
    let release = flow.release_branch();

    require_branch(sh, staging)?;
    require_clean(sh, staging)?;
    require_branch_exists(sh, release)?;

    taskit_output::taskit_progress!("Promoting {staging} -> {release}");
    checkout(ctx, release)?;
    merge_no_ff(
        ctx,
        staging,
        &format!("flow: promote {staging} into {release}"),
    )?;
    taskit_output::taskit_ok!("Done. Now on {release}. Run `taskit flow finish` when ready.");
    Ok(())
}

pub fn finish(ctx: &Ctx, flow: &FlowConfig) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let main = flow.main_branch();
    let staging = flow.staging_branch();
    let release = flow.release_branch();

    // Auto-switch to release if not already there.
    if current_branch(sh)? != release {
        checkout(ctx, release)?;
    }
    require_clean(sh, release)?;
    require_branch_exists(sh, main)?;
    require_branch_exists(sh, staging)?;

    taskit_output::taskit_progress!(
        "Finishing release: {release} -> {main}, then syncing {main} -> {staging}"
    );

    // Merge release into main
    checkout(ctx, main)?;
    merge_no_ff(ctx, release, &format!("flow: finish {release} into {main}"))?;

    // Sync main back into staging
    checkout(ctx, staging)?;
    merge_no_ff(ctx, main, &format!("flow: sync {main} into {staging}"))?;

    taskit_output::taskit_ok!("Done. Now on {staging}. All branches are in sync.");
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

/// Run the full promote → CI gate → finish pipeline with LLM-assisted conflict resolution.
///
/// Sequence:
/// 1. Verify staging is clean and release/main exist.
/// 2. Promote: merge staging → release (with conflict resolution).
/// 3. CI gate: run default pipeline on release; abort if any step fails.
/// 4. Finish: merge release → main, sync main → staging (with conflict resolution).
pub fn auto(
    ctx: &Ctx,
    flow: &FlowConfig,
    resolver: &dyn ConflictResolver,
) -> Result<(), TaskitError> {
    use taskit_types::step::StepStatus;

    let sh = &ctx.sh;
    let staging = flow.staging_branch();
    let release = flow.release_branch();
    let main = flow.main_branch();

    require_branch(sh, staging)?;
    require_clean(sh, staging)?;
    require_branch_exists(sh, release)?;
    require_branch_exists(sh, main)?;

    // 1. Promote staging → release
    taskit_output::taskit_progress!("auto: promoting {staging} → {release}");
    checkout(ctx, release)?;
    merge_with_resolution(
        ctx,
        staging,
        &format!("flow: promote {staging} into {release}"),
        resolver,
    )?;

    // 2. CI gate on release
    taskit_output::taskit_progress!("auto: running CI on {release}");
    let outcome = crate::ci::run_default_internal(ctx, true, false);
    if !outcome.passed {
        let failed: Vec<String> = outcome
            .results
            .iter()
            .filter(|s| s.status == StepStatus::Fail)
            .map(|s| s.name.clone())
            .collect();
        taskit_output::taskit_err!(
            "auto: CI failed on {release} — staying on {release} for investigation"
        );
        return Err(FlowError::CiFailed { failed }.into());
    }
    taskit_output::taskit_ok!("auto: CI passed on {release}");

    // 3. Finish release → main, sync main → staging
    taskit_output::taskit_progress!("auto: finishing {release} → {main}");
    checkout(ctx, main)?;
    merge_with_resolution(
        ctx,
        release,
        &format!("flow: finish {release} into {main}"),
        resolver,
    )?;

    taskit_output::taskit_progress!("auto: syncing {main} → {staging}");
    checkout(ctx, staging)?;
    merge_with_resolution(
        ctx,
        main,
        &format!("flow: sync {main} into {staging}"),
        resolver,
    )?;

    taskit_output::taskit_ok!("auto: done. {staging} is in sync with {main}.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_flow() -> FlowConfig {
        FlowConfig::default()
    }

    struct AlwaysResolve;
    impl ConflictResolver for AlwaysResolve {
        fn resolve(&self, files: &[ConflictFile]) -> Result<Vec<ResolvedFile>, TaskitError> {
            Ok(files
                .iter()
                .map(|f| ResolvedFile {
                    path: f.path.clone(),
                    content: "resolved\n".into(),
                })
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
        let result = AlwaysResolve.resolve(&[ConflictFile {
            path: "src/lib.rs".into(),
            ours: "ours".into(),
            theirs: "theirs".into(),
            base: None,
        }]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap()[0].content, "resolved\n");
    }

    #[test]
    fn conflict_resolver_fake_escalates() {
        let result = AlwaysEscalate.resolve(&[ConflictFile {
            path: "src/lib.rs".into(),
            ours: "a".into(),
            theirs: "b".into(),
            base: None,
        }]);
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
            staging: Some("develop".into()),
            release: Some("rc".into()),
        };
        assert_eq!(f.main_branch(), "production");
        assert_eq!(f.staging_branch(), "develop");
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
staging = "develop"
release = "rc"
"#,
        )
        .unwrap();
        assert_eq!(cfg.main_branch(), "production");
        assert_eq!(cfg.staging_branch(), "develop");
        assert_eq!(cfg.release_branch(), "rc");
    }

    #[test]
    fn flow_config_parses_empty_toml() {
        let cfg: FlowConfig = toml::from_str("").unwrap();
        assert_eq!(cfg.main_branch(), "main");
        assert_eq!(cfg.staging_branch(), "staging");
        assert_eq!(cfg.release_branch(), "release");
    }

    #[test]
    fn flow_config_partial_override() {
        let cfg: FlowConfig = toml::from_str(r#"staging = "dev""#).unwrap();
        assert_eq!(cfg.main_branch(), "main");
        assert_eq!(cfg.staging_branch(), "dev");
        assert_eq!(cfg.release_branch(), "release");
    }
}
