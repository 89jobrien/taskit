use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

pub fn run(ctx: &Ctx, crate_name: &str) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    taskit_output::taskit_progress!("Running proptests for {crate_name}...");
    ctx.run(cmd!(
        sh,
        "cargo nextest run --locked -p {crate_name} -E 'test(prop)'"
    ))?;
    Ok(())
}
