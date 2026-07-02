use taskit_types::error::TaskitError;

use crate::ctx::Ctx;
use crate::protocol;

pub fn run(ctx: &Ctx) -> Result<(), TaskitError> {
    taskit_output::taskit_progress!("Checking protocol drift...");
    protocol::drift::run(ctx, false, false, false)?;
    taskit_output::taskit_ok!("All freshness checks passed.");
    Ok(())
}
