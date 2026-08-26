use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

/// Supported `git-cliff` changelog workflows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChangelogMode {
    /// Prepend commits since the latest release to `CHANGELOG.md`.
    #[default]
    Unreleased,
    /// Regenerate the complete `CHANGELOG.md` from git history.
    Full,
    /// Prepend the latest tagged release to `CHANGELOG.md`.
    Latest,
    /// Print unreleased changes without writing a file.
    Preview,
}

/// Generate or preview a changelog using `git-cliff`.
pub fn run(ctx: &Ctx, mode: ChangelogMode) -> Result<(), TaskitError> {
    let config_path = ctx.root().join("cliff.toml");
    if !config_path.is_file() {
        return Err(TaskitError::other(format!(
            "{} not found; run `git-cliff --init` to create it",
            config_path.display()
        )));
    }

    let sh = &ctx.sh;
    sh.change_dir(ctx.root());
    match mode {
        ChangelogMode::Unreleased => {
            ctx.run(cmd!(sh, "git-cliff --unreleased --prepend CHANGELOG.md"))?;
        }
        ChangelogMode::Full => {
            ctx.run(cmd!(sh, "git-cliff -o CHANGELOG.md"))?;
        }
        ChangelogMode::Latest => {
            ctx.run(cmd!(sh, "git-cliff --latest --prepend CHANGELOG.md"))?;
        }
        ChangelogMode::Preview => {
            ctx.run(cmd!(sh, "git-cliff --unreleased"))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use taskit_types::config::Config;
    use taskit_types::output_format::OutputFormat;

    fn dry_run_context() -> (tempfile::TempDir, Ctx) {
        let root = tempfile::tempdir().expect("create temporary workspace");
        std::fs::write(root.path().join("cliff.toml"), "").expect("write cliff config");
        let ctx = Ctx::new(
            xshell::Shell::new().expect("create shell"),
            root.path().to_path_buf(),
            Config::default(),
            true,
            OutputFormat::Human,
        );
        (root, ctx)
    }

    #[test]
    fn modes_map_to_expected_git_cliff_commands() {
        let cases = [
            (
                ChangelogMode::Unreleased,
                "git-cliff --unreleased --prepend CHANGELOG.md",
            ),
            (ChangelogMode::Full, "git-cliff -o CHANGELOG.md"),
            (
                ChangelogMode::Latest,
                "git-cliff --latest --prepend CHANGELOG.md",
            ),
            (ChangelogMode::Preview, "git-cliff --unreleased"),
        ];

        for (mode, expected) in cases {
            let (_root, ctx) = dry_run_context();
            let start = ctx.command_capture_start();
            run(&ctx, mode).expect("dry-run changelog should succeed");
            let records = ctx.command_capture_finish(start);
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].command, expected);
        }
    }

    #[test]
    fn missing_cliff_config_returns_error() {
        let root = tempfile::tempdir().expect("create temporary workspace");
        let ctx = Ctx::new(
            xshell::Shell::new().expect("create shell"),
            root.path().to_path_buf(),
            Config::default(),
            true,
            OutputFormat::Human,
        );

        let error = run(&ctx, ChangelogMode::Preview).expect_err("missing config should fail");
        assert!(error.to_string().contains("cliff.toml not found"));
    }

    #[test]
    fn command_runs_from_context_root() {
        let (root, ctx) = dry_run_context();

        run(&ctx, ChangelogMode::Preview).expect("dry-run changelog should succeed");

        assert_eq!(ctx.sh.current_dir(), root.path());
    }
}
