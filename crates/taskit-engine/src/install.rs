use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

/// Install the `taskit` binary itself via `cargo install --path . --force`.
///
/// This is distinct from [`crate::bootstrap::run`], which sets up a
/// *workspace* for development (git hooks + dev tools) rather than
/// installing taskit as a binary on `PATH`.
pub fn run(ctx: &Ctx) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let root = ctx.root();
    taskit_output::taskit_progress!("Installing taskit from {}...", root.display());
    ctx.run(cmd!(sh, "cargo install --path {root} --force"))?;
    taskit_output::taskit_ok!("taskit installed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::path::PathBuf;
    use xshell::Shell;

    /// `Ctx::test()` runs commands for real (dry_run: false) — unacceptable
    /// here, since a real run would `cargo install --force` over whatever
    /// `taskit` binary is on the test machine's PATH. Always dry-run this one.
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
    fn install_dry_run_does_not_panic() {
        let ctx = dry_run_ctx();
        let result = run(&ctx);
        assert!(result.is_ok());
    }
}
