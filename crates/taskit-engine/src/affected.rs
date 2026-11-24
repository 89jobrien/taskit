//! Detects affected workspace crates from Git changes and configured propagation rules.

use std::collections::BTreeSet;
use taskit_types::error::TaskitError;
use xshell::{Shell, cmd};

use crate::config::WorkspaceConfig;

#[cfg(test)]
use crate::config::{CrateEntry, PropagationEntry};

/// Detect affected crates from every kind of local change.
///
/// Three sources are combined so that work in progress is visible before it
/// is committed:
/// - commits on `HEAD` since `origin/main` (the branch's own history),
/// - tracked edits in the working tree and index, relative to `HEAD`,
/// - untracked files that are not ignored.
///
/// The crate list and propagation table are read from `ws` rather than
/// hardcoded constants, making this workspace-agnostic.
pub fn detect(sh: &Shell, ws: &WorkspaceConfig) -> Result<BTreeSet<String>, TaskitError> {
    let committed = cmd!(sh, "git diff --name-only origin/main...HEAD")
        .read()
        .map_err(|e| {
            TaskitError::other(format!(
                "failed to detect affected crates — ensure 'origin/main' is fetchable: {e}"
            ))
        })?;
    let tracked = cmd!(sh, "git diff --name-only HEAD")
        .read()
        .map_err(|e| TaskitError::other(format!("failed to read working-tree changes: {e}")))?;
    let untracked = cmd!(sh, "git ls-files --others --exclude-standard")
        .read()
        .map_err(|e| TaskitError::other(format!("failed to list untracked files: {e}")))?;

    let changed_files = [committed.as_str(), tracked.as_str(), untracked.as_str()]
        .into_iter()
        .flat_map(str::lines);
    Ok(affected_from_files(changed_files, ws))
}

/// Map changed file paths to crate directories, then apply propagation.
///
/// Pure so the mapping can be tested without a git repository.
fn affected_from_files<'a>(
    files: impl IntoIterator<Item = &'a str>,
    ws: &WorkspaceConfig,
) -> BTreeSet<String> {
    let mut affected = BTreeSet::new();
    for file in files {
        if let Some(dir) = crate_for_file(file, ws) {
            affected.insert(dir.to_string());
        }
    }
    apply_propagation(&mut affected, ws);
    affected
}

/// Find which crate directory a file belongs to, or None.
fn crate_for_file<'a>(file: &str, ws: &'a WorkspaceConfig) -> Option<&'a str> {
    ws.crates
        .iter()
        .find(|e| file.starts_with(&format!("{}/", e.dir)))
        .map(|e| e.dir.as_str())
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
    use proptest::prelude::*;

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
                    exclude_from_version_check: false,
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

    // --- affected_from_files ---

    #[test]
    fn affected_from_files_includes_every_source_of_change() {
        let ws = make_ws(&[("my-lib", None), ("my-cli", None)], &[]);
        // Committed, tracked-but-uncommitted, and untracked files all count.
        let files = ["my-lib/src/lib.rs", "my-cli/src/main.rs"];
        let affected = affected_from_files(files, &ws);
        assert_eq!(
            affected,
            ["my-cli".to_string(), "my-lib".to_string()].into()
        );
    }

    #[test]
    fn affected_from_files_ignores_paths_outside_crates() {
        let ws = make_ws(&[("my-lib", None)], &[]);
        let affected = affected_from_files(["README.md", "Cargo.lock"], &ws);
        assert!(affected.is_empty());
    }

    #[test]
    fn affected_from_files_propagates_to_dependents() {
        let ws = make_ws(&[("common", None), ("api", None)], &[("common", &["api"])]);
        let affected = affected_from_files(["common/src/lib.rs"], &ws);
        assert_eq!(affected, ["api".to_string(), "common".to_string()].into());
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
        assert_eq!(crate_for_file("scripts/build.rs", &ws), None);
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

    // --- properties ---

    proptest! {
        #[test]
        fn affected_only_contains_configured_dirs(
            files in proptest::collection::vec("[a-z/._-]{0,24}", 0..12),
        ) {
            let ws = make_ws(&[("alpha", None), ("beta", None)], &[]);
            let affected = affected_from_files(files.iter().map(String::as_str), &ws);
            for dir in &affected {
                prop_assert!(dir == "alpha" || dir == "beta", "unexpected crate {dir}");
            }
        }

        #[test]
        fn paths_outside_every_crate_dir_affect_nothing(
            files in proptest::collection::vec("[a-z/._-]{0,24}", 0..12),
        ) {
            let ws = make_ws(&[("alpha", None), ("beta", None)], &[]);
            let outside: Vec<&str> = files
                .iter()
                .map(String::as_str)
                .filter(|f| !f.starts_with("alpha/") && !f.starts_with("beta/"))
                .collect();
            prop_assert!(affected_from_files(outside.iter().copied(), &ws).is_empty());
        }

        #[test]
        fn any_file_under_a_crate_dir_affects_that_crate(rest in "[a-z/._-]{0,24}") {
            let ws = make_ws(&[("alpha", None)], &[]);
            let file = format!("alpha/{rest}");
            prop_assert_eq!(
                affected_from_files([file.as_str()], &ws),
                ["alpha".to_string()].into()
            );
        }
    }
}
