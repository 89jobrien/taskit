use anyhow::{Context, Result};
use xshell::{Shell, cmd};

use crate::runner;

/// Crates in publish order: dependencies before dependents.
const PUBLISH_ORDER: &[&str] = &[
    "taskit-core",
    "taskit-engine",
    "taskit-init",
    "taskit-crux",
    "taskit",
];

pub fn run(sh: &Shell, skip_docs: bool, allow_dirty: bool) -> Result<()> {
    if !skip_docs {
        generate_docs(sh)?;
    }

    publish_crates(sh, allow_dirty)?;

    eprintln!("publish complete");
    Ok(())
}

fn generate_docs(sh: &Shell) -> Result<()> {
    eprintln!("Generating documentation...");
    let doc_cmd = cmd!(sh, "cargo doc --workspace --no-deps");
    runner::xrun(doc_cmd).context("cargo doc failed")?;
    eprintln!("Documentation generated");
    Ok(())
}

fn publish_crates(sh: &Shell, allow_dirty: bool) -> Result<()> {
    let dry_run = runner::is_dry_run();

    for krate in PUBLISH_ORDER {
        eprintln!("Publishing {krate}...");

        let mut args = vec!["publish", "-p", krate];
        if dry_run {
            args.push("--dry-run");
        }
        if allow_dirty {
            args.push("--allow-dirty");
        }

        // Use cmd! with explicit cargo + args to pass --dry-run to cargo
        // publish itself (separate from taskit's own dry-run which skips
        // execution entirely via xrun).
        let publish_cmd = cmd!(sh, "cargo {args...}");
        runner::xrun(publish_cmd).with_context(|| format!("failed to publish {krate}"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_order_starts_with_core() {
        assert_eq!(PUBLISH_ORDER[0], "taskit-core");
    }

    #[test]
    fn publish_order_ends_with_root() {
        assert_eq!(*PUBLISH_ORDER.last().unwrap(), "taskit");
    }

    #[test]
    fn publish_order_has_all_crates() {
        assert_eq!(PUBLISH_ORDER.len(), 5);
        assert!(PUBLISH_ORDER.contains(&"taskit-core"));
        assert!(PUBLISH_ORDER.contains(&"taskit-engine"));
        assert!(PUBLISH_ORDER.contains(&"taskit-init"));
        assert!(PUBLISH_ORDER.contains(&"taskit-crux"));
        assert!(PUBLISH_ORDER.contains(&"taskit"));
    }

    #[test]
    fn core_before_engine() {
        let core_pos = PUBLISH_ORDER
            .iter()
            .position(|&c| c == "taskit-core")
            .unwrap();
        let engine_pos = PUBLISH_ORDER
            .iter()
            .position(|&c| c == "taskit-engine")
            .unwrap();
        assert!(core_pos < engine_pos);
    }

    #[test]
    fn core_before_init() {
        let core_pos = PUBLISH_ORDER
            .iter()
            .position(|&c| c == "taskit-core")
            .unwrap();
        let init_pos = PUBLISH_ORDER
            .iter()
            .position(|&c| c == "taskit-init")
            .unwrap();
        assert!(core_pos < init_pos);
    }

    #[test]
    fn engine_before_root() {
        let engine_pos = PUBLISH_ORDER
            .iter()
            .position(|&c| c == "taskit-engine")
            .unwrap();
        let root_pos = PUBLISH_ORDER.iter().position(|&c| c == "taskit").unwrap();
        assert!(engine_pos < root_pos);
    }
}
