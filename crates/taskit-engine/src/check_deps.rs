use taskit_types::error::TaskitError;
use xshell::{Shell, cmd};

use crate::runner::xrun;

pub fn run(sh: &Shell) -> Result<(), TaskitError> {
    eprintln!("Checking for unused dependencies...");
    xrun(cmd!(sh, "cargo-machete"))?;
    Ok(())
}
