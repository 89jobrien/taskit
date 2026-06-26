use anyhow::Result;
use xshell::Shell;

use crate::{config::ProtocolConfig, protocol, schema};

pub fn run(sh: &Shell, proto: Option<&ProtocolConfig>) -> Result<()> {
    eprintln!("Checking schema freshness...");
    schema::run(sh, true)?;
    eprintln!("Checking protocol drift...");
    let root = std::env::current_dir()?;
    protocol::drift::run(&root, proto, false, false, false)?;
    eprintln!("All freshness checks passed.");
    Ok(())
}
