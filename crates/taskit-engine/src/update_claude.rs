use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

pub fn run(ctx: &Ctx, version: &str) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    taskit_output::taskit_progress!("Updating Claude Code version to {version}...");
    ctx.run(cmd!(sh, "bash scripts/update-claude-version.sh {version}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_update_claude() {
        let ctx = Ctx::new(
            xshell::Shell::new().expect("shell"),
            std::path::PathBuf::from("."),
            Default::default(),
            true,
            Default::default(),
        );
        run(&ctx, "1.0.0").expect("dry-run update-claude should succeed");
    }
}
