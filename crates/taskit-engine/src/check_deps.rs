use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

pub fn run(ctx: &Ctx) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    taskit_output::taskit_progress!("Checking for unused dependencies...");
    ctx.run(cmd!(sh, "cargo-machete"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_check_deps() {
        let ctx = Ctx::new(
            xshell::Shell::new().expect("shell"),
            std::path::PathBuf::from("."),
            Default::default(),
            true,
            Default::default(),
        );
        run(&ctx).expect("dry-run check-deps should succeed");
    }
}
