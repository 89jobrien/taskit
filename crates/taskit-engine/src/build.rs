//! Builds cargo packages: the workspace, one workspace member, or a local crate directory.

use std::path::Path;

use taskit_types::error::TaskitError;
use xshell::{Cmd, cmd};

use crate::ctx::Ctx;

/// Build a cargo package in development or release mode.
///
/// With neither `crate_name` nor `path`, builds the whole workspace via
/// `cargo build --workspace`. With a `crate_name`, builds that package with
/// `cargo build -p <name>`. With a `path`, builds the crate rooted at that
/// directory with `cargo build --manifest-path <dir>/Cargo.toml`.
pub fn run(
    ctx: &Ctx,
    crate_name: Option<&str>,
    path: Option<&Path>,
    release: bool,
) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let mut command: Cmd = match (crate_name, path) {
        (Some(name), _) => cmd!(sh, "cargo build -p {name}"),
        (None, Some(dir)) => {
            let manifest = dir.join("Cargo.toml");
            cmd!(sh, "cargo build --manifest-path {manifest}")
        }
        (None, None) => cmd!(sh, "cargo build --workspace"),
    };
    if release {
        command = command.arg("--release");
    }
    let label = crate_name
        .map(str::to_owned)
        .or_else(|| path.map(|p| p.display().to_string()))
        .unwrap_or_else(|| "workspace".to_owned());
    taskit_output::taskit_progress!(
        "Building {label}{}...",
        if release { " (release)" } else { "" }
    );
    ctx.run(command)?;
    taskit_output::taskit_ok!("build complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::path::PathBuf;
    use xshell::Shell;

    /// `Ctx::test()` runs commands for real (dry_run: false) — fine for cheap
    /// commands like `dev_setup`/`hooks`, but `cargo build` is expensive, so
    /// these tests build their own dry-run `Ctx` instead.
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
    fn build_workspace_dry_run_does_not_panic() {
        let ctx = dry_run_ctx();
        assert!(run(&ctx, None, None, false).is_ok());
    }

    #[test]
    fn build_release_dry_run_does_not_panic() {
        let ctx = dry_run_ctx();
        assert!(run(&ctx, None, None, true).is_ok());
    }

    #[test]
    fn build_crate_and_path_dry_run_do_not_panic() {
        let ctx = dry_run_ctx();
        assert!(run(&ctx, Some("taskit-tui"), None, false).is_ok());
        assert!(run(&ctx, None, Some(Path::new("crates/taskit-tui")), true).is_ok());
    }
}
