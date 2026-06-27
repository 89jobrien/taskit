use anyhow::Result;
use xshell::{Shell, cmd};

use crate::runner::xrun;

pub fn run(sh: &Shell, crate_name: Option<&str>) -> Result<()> {
    let pkg = crate_name.unwrap_or("maestro-common");
    eprintln!("Running proptests for {pkg}...");
    xrun(cmd!(
        sh,
        "cargo nextest run --locked -p {pkg} --features proptest"
    ))?;
    Ok(())
}
