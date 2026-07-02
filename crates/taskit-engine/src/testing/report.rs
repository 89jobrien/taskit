use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

pub fn run(ctx: &Ctx) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    taskit_output::taskit_progress!("Generating unified coverage report...");
    ctx.run(cmd!(
        sh,
        "cargo llvm-cov --locked --all-features --workspace --html"
    ))?;
    taskit_output::taskit_ok!("Report: target/llvm-cov/html/index.html");
    Ok(())
}
