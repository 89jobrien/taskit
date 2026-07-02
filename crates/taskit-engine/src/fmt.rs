use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

pub fn run(ctx: &Ctx, check: bool, affected: bool) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let ws = ctx.ws();
    if affected {
        let crates = crate::affected::detect(sh, ws)?;
        if crates.is_empty() {
            taskit_output::taskit_skip!("No affected crates detected, skipping.");
            return Ok(());
        }
        for crate_dir in &crates {
            let pkg = crate::affected::pkg_name(crate_dir, ws);
            if check {
                ctx.run(cmd!(sh, "cargo fmt -p {pkg} -- --check"))?;
            } else {
                ctx.run(cmd!(sh, "cargo fmt -p {pkg}"))?;
            }
        }
        return Ok(());
    }
    if check {
        ctx.run(cmd!(sh, "cargo fmt --all -- --check"))?;
    } else {
        ctx.run(cmd!(sh, "cargo fmt --all"))?;
    }
    Ok(())
}
