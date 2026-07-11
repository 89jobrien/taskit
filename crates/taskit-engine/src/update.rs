use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

pub fn run(ctx: &Ctx, aggressive: bool) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    if ctx.dry_run {
        if aggressive {
            taskit_output::taskit_dry!("cargo update --aggressive");
        } else {
            taskit_output::taskit_dry!("cargo update");
        }
        return Ok(());
    }
    if aggressive {
        ctx.run(cmd!(sh, "cargo update --aggressive"))?;
    } else {
        ctx.run(cmd!(sh, "cargo update"))?;
    }
    taskit_output::taskit_ok!("Cargo.lock updated.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_dry_run_returns_ok() {
        let ctx = Ctx::test();
        assert!(run(&ctx, false).is_ok());
        assert!(run(&ctx, true).is_ok());
    }
}
