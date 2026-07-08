use taskit_types::config::FlowConfig;
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
    checkout(ctx, staging)?;
    taskit_output::taskit_ok!(
        "Done. Now on {staging}. Review {release}, then `taskit flow finish`."
    );
    Ok(())
}

pub fn finish(ctx: &Ctx, flow: &FlowConfig) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let main = flow.main_branch();
    let staging = flow.staging_branch();
    let release = flow.release_branch();

    require_branch(sh, release)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn default_flow() -> FlowConfig {
        FlowConfig::default()
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
