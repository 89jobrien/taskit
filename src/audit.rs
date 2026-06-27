use anyhow::Result;
use xshell::{Shell, cmd};

use crate::runner::xrun;

pub fn run(sh: &Shell) -> Result<()> {
    eprintln!("Running cargo-deny...");
    xrun(cmd!(sh, "cargo deny check"))?;
    Ok(())
}
