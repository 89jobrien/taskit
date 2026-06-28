use taskit_types::error::TaskitError;
use xshell::{Shell, cmd};

use crate::runner::xrun;

pub fn run(sh: &Shell, version: &str) -> Result<(), TaskitError> {
    eprintln!("Updating Claude Code version to {version}...");
    xrun(cmd!(sh, "bash scripts/update-claude-version.sh {version}"))?;
    Ok(())
}
