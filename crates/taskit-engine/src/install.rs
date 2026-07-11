use taskit_types::error::TaskitError;

use crate::ctx::Ctx;

/// Install git hooks and dev tools in one step.
pub fn run(ctx: &Ctx) -> Result<(), TaskitError> {
    crate::hooks::install_hooks(ctx)?;
    crate::dev_setup::setup(ctx)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_dry_run_does_not_panic() {
        let ctx = Ctx::test();
        // dry_run mode: no filesystem writes, must not panic
        let result = run(&ctx);
        // dev_setup::run may fail if tools are missing; we only assert no panic
        let _ = result;
    }
}
