use taskit_types::error::TaskitError;

use crate::ctx::Ctx;
use crate::protocol;

pub fn run(ctx: &Ctx) -> Result<(), TaskitError> {
    taskit_output::taskit_progress!("Checking protocol drift...");
    protocol::drift::run(ctx, false, false, false)?;
    taskit_output::taskit_ok!("All freshness checks passed.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_check_freshness() {
        let ctx = Ctx::new(
            xshell::Shell::new().expect("shell"),
            std::path::PathBuf::from("."),
            Default::default(),
            true,
            Default::default(),
        );
        // dry-run skips the actual protocol drift check
        run(&ctx).expect("dry-run check-freshness should succeed");
    }
}
