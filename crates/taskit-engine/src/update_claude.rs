use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

pub fn run(ctx: &Ctx, version: &str) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    taskit_output::taskit_progress!("Updating Claude Code version to {version}...");
    ctx.run(cmd!(sh, "bash scripts/update-claude-version.sh {version}"))?;
    Ok(())
}
