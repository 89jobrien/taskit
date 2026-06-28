use taskit_types::error::TaskitError;
use xshell::{Shell, cmd};

use crate::runner::xrun;

pub fn run(sh: &Shell) -> Result<(), TaskitError> {
    eprintln!("Generating unified coverage report...");
    xrun(cmd!(
        sh,
        "cargo llvm-cov --locked --all-features --workspace --html"
    ))?;
    eprintln!("Report: target/llvm-cov/html/index.html");
    Ok(())
}
