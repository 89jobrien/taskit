use taskit_types::error::TaskitError;

use crate::ctx::Ctx;

/// Set up a workspace for development: install git hooks and dev tools in
/// one step. (Formerly `taskit install` — renamed because "install" reads as
/// installing the `taskit` binary itself, which is what `taskit install` now
/// does; see `install.rs`.)
pub fn run(ctx: &Ctx) -> Result<(), TaskitError> {
    crate::hooks::install_hooks(ctx)?;
    crate::dev_setup::setup(ctx)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_dry_run_does_not_panic() {
        let ctx = Ctx::test();
        // dry_run mode: no filesystem writes, must not panic
        let result = run(&ctx);
        // dev_setup::run may fail if tools are missing; we only assert no panic
        let _ = result;
    }
}
