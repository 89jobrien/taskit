use anyhow::Result;
use xshell::{Shell, cmd};

use crate::runner::xrun;

pub fn run(sh: &Shell) -> Result<()> {
    eprintln!("Checking for unused dependencies...");
    xrun(cmd!(sh, "cargo-machete"))?;
    Ok(())
}
