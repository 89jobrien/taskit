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

#[cfg(test)]
mod tests {
    use super::*;
    use taskit_types::config::Config;
    use taskit_types::output_format::OutputFormat;

    fn dry_ctx() -> Ctx {
        Ctx::new(
            xshell::Shell::new().expect("shell"),
            std::path::PathBuf::from("."),
            Config::default(),
            true,
            OutputFormat::Human,
        )
    }

    #[test]
    fn dry_run_check_all() {
        let ctx = dry_ctx();
        run(&ctx, true, false).expect("dry-run fmt --check should succeed");
    }

    #[test]
    fn dry_run_format_all() {
        let ctx = dry_ctx();
        run(&ctx, false, false).expect("dry-run fmt should succeed");
    }

    #[test]
    fn dry_run_check_affected() {
        let ctx = dry_ctx();
        run(&ctx, true, true).expect("dry-run fmt --check --affected should succeed");
    }

    #[test]
    fn dry_run_format_affected() {
        let ctx = dry_ctx();
        run(&ctx, false, true).expect("dry-run fmt --affected should succeed");
    }
}
