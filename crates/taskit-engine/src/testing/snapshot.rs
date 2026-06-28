use taskit_types::error::TaskitError;
use xshell::{Shell, cmd};

use crate::runner::xrun;

pub fn run(sh: &Shell) -> Result<(), TaskitError> {
    eprintln!("Reviewing pending insta snapshots...");
    xrun(cmd!(sh, "cargo insta review"))?;
    Ok(())
}
