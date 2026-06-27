use anyhow::{Context, Result};
use std::collections::BTreeSet;
use xshell::{Shell, cmd};

use crate::config::WorkspaceConfig;

#[cfg(test)]
use crate::config::{CrateEntry, PropagationEntry};

/// Detect affected crates from git diff against `origin/main`.
///
/// The crate list and propagation table are read from `ws` rather than
/// hardcoded constants, making this workspace-agnostic.
pub fn detect(sh: &Shell, ws: &WorkspaceConfig) -> Result<BTreeSet<String>> {
    let output = cmd!(sh, "git diff --name-only origin/main...HEAD")
        .read()
        .context("failed to detect affected crates — ensure 'origin/main' is fetchable")?;
    let changed_files: Vec<&str> = output.lines().collect();

    let mut affected = BTreeSet::new();
    for file in &changed_files {
        for entry in &ws.crates {
            if file.starts_with(&format!("{}/", entry.dir)) {
                affected.insert(entry.dir.clone());
            }
        }
    }

    apply_propagation(&mut affected, ws);
    Ok(affected)
}

/// Expand `affected` in-place by following the propagation table in `ws`.
///
/// Single-pass expansion — correct when no source also appears as a dependent
/// (no transitive chains). Replace with a fixpoint loop if transitive
/// relationships are ever added to the config.
fn apply_propagation(affected: &mut BTreeSet<String>, ws: &WorkspaceConfig) {
    let direct: Vec<String> = affected.iter().cloned().collect();
    for crate_name in &direct {
        for entry in &ws.propagation {
            if crate_name == &entry.source {
                for dep in &entry.dependents {
                    affected.insert(dep.clone());
                }
            }
        }
    }
}

/// Map a crate directory name to its Cargo package name.
///
/// Looks up the `pkg` field on the matching `CrateEntry`; falls back to
/// `crate_dir` if not found or no override is set.
pub fn pkg_name<'a>(crate_dir: &'a str, ws: &'a WorkspaceConfig) -> &'a str {
    ws.crates
        .iter()
        .find(|e| e.dir == crate_dir)
        .map(|e| e.pkg_name())
        .unwrap_or(crate_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ws(
        crates: &[(&str, Option<&str>)],
        propagation: &[(&str, &[&str])],
    ) -> WorkspaceConfig {
        WorkspaceConfig {
            root: None,
            crates: crates
                .iter()
                .map(|(dir, pkg)| CrateEntry {
                    dir: dir.to_string(),
                    pkg: pkg.map(|s| s.to_string()),
                })
                .collect(),
            propagation: propagation
                .iter()
                .map(|(source, deps)| PropagationEntry {
                    source: source.to_string(),
                    dependents: deps.iter().map(|s| s.to_string()).collect(),
                })
                .collect(),
            offline_skip: None,
        }
    }

    /// Find which crate directory a file belongs to, or None.
    fn crate_for_file<'a>(file: &str, ws: &'a WorkspaceConfig) -> Option<&'a str> {
        ws.crates
            .iter()
            .find(|e| file.starts_with(&format!("{}/", e.dir)))
            .map(|e| e.dir.as_str())
    }

    // --- pkg_name ---

    #[test]
    fn pkg_name_defaults_to_dir_when_no_pkg() {
        let ws = make_ws(&[("my-lib", None)], &[]);
        assert_eq!(pkg_name("my-lib", &ws), "my-lib");
    }

    #[test]
    fn pkg_name_uses_pkg_field_when_set() {
        let ws = make_ws(&[("my-cli", Some("my-binary"))], &[]);
        assert_eq!(pkg_name("my-cli", &ws), "my-binary");
    }

    #[test]
    fn pkg_name_falls_back_to_dir_for_unknown_crate() {
        let ws = make_ws(&[], &[]);
        assert_eq!(pkg_name("unknown", &ws), "unknown");
    }

    // --- crate_for_file ---

    #[test]
    fn crate_for_file_matches_known_crate() {
        let ws = make_ws(&[("my-lib", None), ("my-cli", None)], &[]);
        assert_eq!(crate_for_file("my-lib/src/lib.rs", &ws), Some("my-lib"));
        assert_eq!(crate_for_file("my-cli/src/main.rs", &ws), Some("my-cli"));
    }

    #[test]
    fn crate_for_file_returns_none_for_unknown_path() {
        let ws = make_ws(&[("my-lib", None)], &[]);
        assert_eq!(crate_for_file("xtask/src/main.rs", &ws), None);
        assert_eq!(crate_for_file("README.md", &ws), None);
        assert_eq!(crate_for_file("Cargo.toml", &ws), None);
    }

    #[test]
    fn crate_for_file_requires_trailing_slash_prefix() {
        let ws = make_ws(&[("my-lib", None)], &[]);
        assert_eq!(crate_for_file("my-lib", &ws), None);
    }

    // --- apply_propagation ---

    #[test]
    fn apply_propagation_empty_set_stays_empty() {
        let ws = make_ws(&[], &[]);
        let mut affected = BTreeSet::new();
        apply_propagation(&mut affected, &ws);
        assert!(affected.is_empty());
    }

    #[test]
    fn apply_propagation_expands_source_to_declared_dependents() {
        let ws = make_ws(
            &[("common", None), ("api", None), ("cli", None)],
            &[("common", &["api", "cli"])],
        );
        let mut affected: BTreeSet<String> = ["common".to_string()].into();
        apply_propagation(&mut affected, &ws);
        assert!(affected.contains("api"));
        assert!(affected.contains("cli"));
    }

    #[test]
    fn apply_propagation_unrelated_crate_does_not_expand() {
        let ws = make_ws(&[("common", None), ("api", None)], &[("common", &["api"])]);
        // "api" is a dependent, not a source — nothing should be added.
        let mut affected: BTreeSet<String> = ["api".to_string()].into();
        apply_propagation(&mut affected, &ws);
        assert_eq!(affected.len(), 1);
    }

    #[test]
    fn apply_propagation_source_with_multiple_dependents() {
        let ws = make_ws(
            &[("k8s", None), ("cli", None), ("api", None)],
            &[("k8s", &["cli", "api"])],
        );
        let mut affected: BTreeSet<String> = ["k8s".to_string()].into();
        apply_propagation(&mut affected, &ws);
        assert!(affected.contains("cli"));
        assert!(affected.contains("api"));
    }

    #[test]
    fn apply_propagation_source_with_single_dependent() {
        let ws = make_ws(&[("docker", None), ("cli", None)], &[("docker", &["cli"])]);
        let mut affected: BTreeSet<String> = ["docker".to_string()].into();
        apply_propagation(&mut affected, &ws);
        assert!(affected.contains("cli"));
        assert!(!affected.contains("api"), "api is not a declared dependent");
    }

    #[test]
    fn apply_propagation_is_idempotent() {
        let ws = make_ws(
            &[("common", None), ("api", None), ("cli", None)],
            &[("common", &["api", "cli"])],
        );
        let mut first: BTreeSet<String> = ["common".to_string()].into();
        apply_propagation(&mut first, &ws);
        let mut second = first.clone();
        apply_propagation(&mut second, &ws);
        assert_eq!(first, second);
    }
}
