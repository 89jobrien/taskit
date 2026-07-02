use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

pub fn run(ctx: &Ctx) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    taskit_output::taskit_progress!("Checking for unused dependencies...");
    ctx.run(cmd!(sh, "cargo-machete"))?;
    Ok(())
}
