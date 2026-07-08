use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

pub fn run(ctx: &Ctx, target: &str, duration: u64) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let dur = duration.to_string();
    taskit_output::taskit_progress!("Fuzzing {target} for {dur}s...");
    ctx.run(cmd!(sh, "cargo fuzz run {target} -- -max_total_time={dur}"))?;
    Ok(())
}
