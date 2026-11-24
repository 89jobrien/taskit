//! Installs cargo crates onto `PATH` via `cargo install --force`.

use std::path::Path;

use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

/// Install a crate from crates.io onto the local `PATH`.
///
/// With no `version`, cargo picks the latest release. With a `version`, that
/// requirement is passed through to `cargo install --version`.
pub fn run_crate(ctx: &Ctx, name: &str, version: Option<&str>) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    taskit_output::taskit_progress!("Installing {name}...");
    match version {
        Some(v) => ctx.run(cmd!(sh, "cargo install {name} --version {v} --force"))?,
        None => ctx.run(cmd!(sh, "cargo install {name} --force"))?,
    }
    taskit_output::taskit_ok!("{name} installed");
    Ok(())
}

/// Install the crate rooted at `dir` onto the local `PATH`.
///
/// Runs `cargo install --path <dir> --force`. Used for local crates, including
/// workspace members under `crates/`.
pub fn run_path(ctx: &Ctx, dir: &Path) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let label = dir.display().to_string();
    taskit_output::taskit_progress!("Installing {label}...");
    ctx.run(cmd!(sh, "cargo install --path {dir} --force"))?;
    taskit_output::taskit_ok!("{label} installed");
    Ok(())
}

/// Install the `taskit` binary itself from this workspace root.
///
/// This is `taskit self install`: `cargo install --path . --force`.
///
/// This is distinct from [`crate::bootstrap::run`], which sets up a
/// *workspace* for development (git hooks + dev tools) rather than
/// installing binaries on `PATH`.
pub fn run_self(ctx: &Ctx) -> Result<(), TaskitError> {
    let root = ctx.root().to_path_buf();
    taskit_output::taskit_progress!("Installing taskit from {}...", root.display());
    run_path(ctx, &root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::path::PathBuf;
    use xshell::Shell;

    /// `Ctx::test()` runs commands for real (dry_run: false) — unacceptable
    /// here, since a real run would `cargo install --force` over whatever
    /// binaries are on the test machine's PATH. Always dry-run these.
    fn dry_run_ctx() -> Ctx {
        Ctx::new(
            Shell::new().expect("create shell"),
            PathBuf::from("."),
            Config::default(),
            true,
            taskit_types::output_format::OutputFormat::Human,
        )
    }

    #[test]
    fn install_self_dry_run_does_not_panic() {
        let ctx = dry_run_ctx();
        assert!(run_self(&ctx).is_ok());
    }

    #[test]
    fn install_crate_dry_run_does_not_panic() {
        let ctx = dry_run_ctx();
        assert!(run_crate(&ctx, "ripgrep", None).is_ok());
        assert!(run_crate(&ctx, "ripgrep", Some("14")).is_ok());
    }

    #[test]
    fn install_path_dry_run_does_not_panic() {
        let ctx = dry_run_ctx();
        assert!(run_path(&ctx, Path::new("crates/taskit-tui")).is_ok());
    }
}
