use taskit_types::error::TaskitError;
use xshell::Shell;

use crate::{config::ProtocolConfig, protocol};

pub fn run(_sh: &Shell, proto: Option<&ProtocolConfig>) -> Result<(), TaskitError> {
    eprintln!("Checking protocol drift...");
    let root = std::env::current_dir()?;
    protocol::drift::run(&root, proto, false, false, false)?;
    eprintln!("All freshness checks passed.");
    Ok(())
}
