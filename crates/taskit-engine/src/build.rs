use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

/// Build the workspace via `cargo build --workspace`, optionally in release mode.
pub fn run(ctx: &Ctx, release: bool) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    if release {
        taskit_output::taskit_progress!("Building workspace (release)...");
        ctx.run(cmd!(sh, "cargo build --workspace --release"))?;
    } else {
        taskit_output::taskit_progress!("Building workspace...");
        ctx.run(cmd!(sh, "cargo build --workspace"))?;
    }
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
    fn build_dry_run_does_not_panic() {
        let ctx = dry_run_ctx();
        let result = run(&ctx, false);
        assert!(result.is_ok());
    }

    #[test]
    fn build_release_dry_run_does_not_panic() {
        let ctx = dry_run_ctx();
        let result = run(&ctx, true);
        assert!(result.is_ok());
    }
}
