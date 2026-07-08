use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

pub fn run(ctx: &Ctx) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    taskit_output::taskit_progress!("Generating unified coverage report...");
    let output = cmd!(
        sh,
        "cargo llvm-cov --locked --all-features --workspace --html"
    )
    .ignore_status()
    .output()
    .map_err(|e| TaskitError::other(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        taskit_output::taskit_err!("some tests failed during coverage run");
        if !stderr.is_empty() {
            eprintln!("{stderr}");
        }
    }

    taskit_output::taskit_ok!("Report: target/llvm-cov/html/index.html");
    Ok(())
}
