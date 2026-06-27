use anyhow::Result;
use xshell::{Shell, cmd};

use crate::runner::is_dry_run;

const VERSION_MEMBERS: &[(&str, &str)] = &[
    ("maestro-api", "maestro-api"),
    ("maestro-cli", "maestro"),
    ("maestro-common", "maestro-common"),
    ("maestro-runtime", "maestro-runtime"),
    ("maestro-k8s", "maestro-k8s"),
];

/// Look up `name` in a cargo metadata `packages` array and return its version string,
/// or `"unknown"` if not found.
fn find_package_version<'a>(packages: &'a [serde_json::Value], name: &str) -> &'a str {
    packages
        .iter()
        .find(|p| p["name"].as_str() == Some(name))
        .and_then(|p| p["version"].as_str())
        .unwrap_or("unknown")
}

pub fn run(sh: &Shell) -> Result<()> {
    if is_dry_run() {
        eprintln!("dry-run: cargo metadata --no-deps --format-version 1");
        eprintln!("dry-run: rustc --version");
        return Ok(());
    }

    let meta_json = cmd!(sh, "cargo metadata --no-deps --format-version 1").read()?;
    let meta: serde_json::Value = serde_json::from_str(&meta_json)?;
    let packages = meta["packages"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("no packages in cargo metadata output"))?;

    eprintln!("Maestro Versions:");
    for (label, pkg_name) in VERSION_MEMBERS {
        eprintln!("  {label}: {}", find_package_version(packages, pkg_name));
    }
    eprintln!();
    let rustc = cmd!(sh, "rustc --version").read()?;
    eprintln!("  rustc: {rustc}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, version: &str) -> serde_json::Value {
        serde_json::json!({ "name": name, "version": version })
    }

    #[test]
    fn find_package_version_returns_version_when_found() {
        let pkgs = vec![pkg("maestro-api", "1.2.3")];
        assert_eq!(find_package_version(&pkgs, "maestro-api"), "1.2.3");
    }

    #[test]
    fn find_package_version_returns_unknown_when_not_found() {
        let pkgs = vec![pkg("other-crate", "0.1.0")];
        assert_eq!(find_package_version(&pkgs, "maestro-api"), "unknown");
    }

    #[test]
    fn find_package_version_returns_unknown_for_empty_list() {
        assert_eq!(find_package_version(&[], "maestro-api"), "unknown");
    }

    #[test]
    fn find_package_version_returns_first_match_when_multiple() {
        let pkgs = vec![pkg("maestro-api", "1.0.0"), pkg("maestro-api", "2.0.0")];
        assert_eq!(find_package_version(&pkgs, "maestro-api"), "1.0.0");
    }

    #[test]
    fn find_package_version_returns_unknown_when_version_field_missing() {
        let pkgs = vec![serde_json::json!({ "name": "maestro-api" })];
        assert_eq!(find_package_version(&pkgs, "maestro-api"), "unknown");
    }

    #[test]
    fn version_members_are_all_nonempty() {
        for (label, pkg_name) in VERSION_MEMBERS {
            assert!(!label.is_empty(), "empty label in VERSION_MEMBERS");
            assert!(!pkg_name.is_empty(), "empty pkg_name in VERSION_MEMBERS");
        }
    }

    #[test]
    fn version_members_has_no_duplicates() {
        let mut labels = std::collections::HashSet::new();
        for (label, _) in VERSION_MEMBERS {
            assert!(labels.insert(*label), "duplicate label: {label}");
        }
    }
}
