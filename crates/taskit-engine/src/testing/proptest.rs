use anyhow::Result;
use xshell::{Shell, cmd};

use crate::runner::xrun;

pub fn run(sh: &Shell, crate_name: &str) -> Result<()> {
    eprintln!("Running proptests for {crate_name}...");
    xrun(cmd!(
        sh,
        "cargo nextest run --locked -p {crate_name} --features proptest"
    ))?;
    Ok(())
}
