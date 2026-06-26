use anyhow::Result;
use xshell::{Shell, cmd};

use crate::{config::WorkspaceConfig, runner::xrun};

pub fn run(sh: &Shell, ws: &WorkspaceConfig, check: bool, affected: bool) -> Result<()> {
    if affected {
        let crates = crate::affected::detect(sh, ws)?;
        if crates.is_empty() {
            eprintln!("No affected crates detected, skipping.");
            return Ok(());
        }
        for crate_dir in &crates {
            let pkg = crate::affected::pkg_name(crate_dir, ws);
            if check {
                xrun(cmd!(sh, "cargo fmt -p {pkg} -- --check"))?;
            } else {
                xrun(cmd!(sh, "cargo fmt -p {pkg}"))?;
            }
        }
        return Ok(());
    }
    if check {
        xrun(cmd!(sh, "cargo fmt --all -- --check"))?;
    } else {
        xrun(cmd!(sh, "cargo fmt --all"))?;
    }
    Ok(())
}
