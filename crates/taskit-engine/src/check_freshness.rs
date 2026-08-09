//! Dependency freshness check: are workspace dependencies stale relative to
//! what's published on the registry?
//!
//! This is distinct from `protocol::drift`, which checks contract-surface
//! hashes (schema/protocol drift), not dependency versions. Shells out to
//! `cargo outdated --workspace --format json` (NDJSON: one object per crate)
//! and reports any dependency with a compatible or newer-major update
//! available. Degrades gracefully if `cargo-outdated` isn't installed.

use serde::Deserialize;
use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;
use crate::util::tool_exists_cmd;

const NOT_AVAILABLE: &str = "---";
const REMOVED: &str = "Removed";

#[derive(Debug, Clone, Deserialize)]
struct OutdatedCrate {
    crate_name: String,
    #[serde(default)]
    dependencies: Vec<OutdatedDependency>,
}

#[derive(Debug, Clone, Deserialize)]
struct OutdatedDependency {
    name: String,
    project: String,
    compat: String,
    latest: String,
}

/// A dependency with an update available, flattened with its owning crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyUpdate {
    pub crate_name: String,
    pub name: String,
    pub project: String,
    pub compat: String,
    pub latest: String,
}

/// True when `cargo-outdated` reports an update is available for this
/// dependency: either a semver-compatible bump (`compat` set) or a newer
/// version outside the current constraint (`latest` set and not "Removed",
/// which means the project version was pulled from the registry entirely,
/// not that it's stale).
fn dependency_is_outdated(dep: &OutdatedDependency) -> bool {
    let has_compat_update = dep.compat != NOT_AVAILABLE;
    let has_latest_update =
        dep.latest != NOT_AVAILABLE && dep.latest != REMOVED && dep.latest != dep.project;
    has_compat_update || has_latest_update
}

/// Parse `cargo outdated --format json` output (newline-delimited JSON, one
/// object per workspace crate) into a flat list of outdated dependencies.
pub fn parse_outdated_json(output: &str) -> Result<Vec<DependencyUpdate>, TaskitError> {
    let mut updates = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let entry: OutdatedCrate = serde_json::from_str(line).map_err(|e| {
            TaskitError::other(format!(
                "failed to parse cargo-outdated output: {e}\nline: {line}"
            ))
        })?;
        for dep in &entry.dependencies {
            if dependency_is_outdated(dep) {
                updates.push(DependencyUpdate {
                    crate_name: entry.crate_name.clone(),
                    name: dep.name.clone(),
                    project: dep.project.clone(),
                    compat: dep.compat.clone(),
                    latest: dep.latest.clone(),
                });
            }
        }
    }
    Ok(updates)
}

pub fn run(ctx: &Ctx, warn_only: bool) -> Result<(), TaskitError> {
    taskit_output::taskit_progress!("Checking dependency freshness...");

    // cargo-outdated is a cargo subcommand plugin; its binary rejects a bare
    // `--version` (mirrors the cargo-llvm-cov handling in dev_setup.rs), so
    // check it via `cargo outdated --version` instead.
    if !tool_exists_cmd("cargo", &["outdated", "--version"]) {
        taskit_output::taskit_skip!(
            "cargo-outdated not installed — skipping dependency freshness check. Install with `cargo install cargo-outdated`."
        );
        return Ok(());
    }

    let sh = &ctx.sh;
    let captured = ctx.run_capture(cmd!(sh, "cargo outdated --workspace --format json"))?;
    let updates = parse_outdated_json(&captured.stdout)?;

    if updates.is_empty() {
        taskit_output::taskit_ok!("All dependencies up to date.");
        return Ok(());
    }

    taskit_output::taskit_progress!("Found {} outdated dependencies:", updates.len());
    for u in &updates {
        taskit_output::taskit_progress!(
            "  {} [{}]: {} -> compat {} / latest {}",
            u.name,
            u.crate_name,
            u.project,
            u.compat,
            u.latest
        );
    }

    if warn_only {
        taskit_output::taskit_warn!(
            "{} outdated dependencies found (warn-only, not failing).",
            updates.len()
        );
        return Ok(());
    }

    Err(TaskitError::other(format!(
        "{} outdated dependencies found. Run `cargo outdated --workspace` for details, or pass --warn-only.",
        updates.len()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_check_freshness() {
        let ctx = Ctx::new(
            xshell::Shell::new().expect("shell"),
            std::path::PathBuf::from("."),
            Default::default(),
            true,
            Default::default(),
        );
        // dry-run skips the actual `cargo outdated` invocation (or, if the
        // tool isn't installed, skips entirely) — either way this succeeds.
        run(&ctx, false).expect("dry-run check-freshness should succeed");
    }

    #[test]
    fn dependency_is_outdated_when_compat_update_available() {
        let dep = OutdatedDependency {
            name: "serde".into(),
            project: "1.0.0".into(),
            compat: "1.0.5".into(),
            latest: "1.0.5".into(),
        };
        assert!(dependency_is_outdated(&dep));
    }

    #[test]
    fn dependency_is_outdated_when_only_latest_beyond_compat() {
        let dep = OutdatedDependency {
            name: "toml".into(),
            project: "0.8.23".into(),
            compat: NOT_AVAILABLE.into(),
            latest: "1.1.4".into(),
        };
        assert!(dependency_is_outdated(&dep));
    }

    #[test]
    fn dependency_is_not_outdated_when_up_to_date() {
        let dep = OutdatedDependency {
            name: "syn".into(),
            project: "2.0.0".into(),
            compat: NOT_AVAILABLE.into(),
            latest: NOT_AVAILABLE.into(),
        };
        assert!(!dependency_is_outdated(&dep));
    }

    #[test]
    fn dependency_is_not_outdated_when_removed_from_registry() {
        // "Removed" means the registry no longer serves this exact version
        // lookup (e.g. yanked/local mirror quirk), not that a newer version
        // exists — don't treat it as an outdated signal on its own.
        let dep = OutdatedDependency {
            name: "hashbrown".into(),
            project: "0.17.1".into(),
            compat: NOT_AVAILABLE.into(),
            latest: REMOVED.into(),
        };
        assert!(!dependency_is_outdated(&dep));
    }

    #[test]
    fn parse_outdated_json_flattens_and_filters() {
        let input = concat!(
            r#"{"crate_name":"a","dependencies":[]}"#,
            "\n",
            r#"{"crate_name":"b","dependencies":[{"name":"serde","project":"1.0.0","compat":"1.0.5","latest":"1.0.5","kind":"Normal","platform":null},{"name":"syn","project":"2.0.0","compat":"---","latest":"---","kind":"Normal","platform":null}]}"#,
        );
        let updates = parse_outdated_json(input).expect("valid json");
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].crate_name, "b");
        assert_eq!(updates[0].name, "serde");
    }

    #[test]
    fn parse_outdated_json_skips_blank_lines() {
        let input = "\n\n{\"crate_name\":\"a\",\"dependencies\":[]}\n\n";
        let updates = parse_outdated_json(input).expect("valid json");
        assert!(updates.is_empty());
    }

    #[test]
    fn parse_outdated_json_errors_on_malformed_line() {
        let input = "not json";
        assert!(parse_outdated_json(input).is_err());
    }

    #[test]
    fn parse_outdated_json_empty_input_is_empty() {
        let updates = parse_outdated_json("").expect("empty is valid");
        assert!(updates.is_empty());
    }
}
