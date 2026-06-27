use anyhow::Result;
use xshell::{Shell, cmd};

use crate::runner::xrun;

pub fn run(sh: &Shell) -> Result<()> {
    eprintln!("Reviewing pending insta snapshots...");
    xrun(cmd!(sh, "cargo insta review"))?;
    Ok(())
}
