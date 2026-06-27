use anyhow::Result;
use xshell::{Shell, cmd};

use crate::runner::xrun;

pub fn run(sh: &Shell, target: &str, duration: u64) -> Result<()> {
    let dur = duration.to_string();
    eprintln!("Fuzzing {target} for {dur}s...");
    xrun(cmd!(sh, "cargo fuzz run {target} -- -max_total_time={dur}"))?;
    Ok(())
}
