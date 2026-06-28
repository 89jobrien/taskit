use taskit_types::error::TaskitError;
use xshell::{Shell, cmd};

use crate::runner::xrun;

pub fn run(sh: &Shell) -> Result<(), TaskitError> {
    eprintln!("Running cargo-deny...");
    xrun(cmd!(sh, "cargo deny check"))?;
    Ok(())
}
