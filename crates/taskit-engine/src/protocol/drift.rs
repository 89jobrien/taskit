use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::config::{ProtocolConfig, SurfaceEntry};

const DEFAULT_LOCK_PATH: &str = "taskit-protocol.lock";
const ALGORITHM: &str = "sha256-normalized-core-contract-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Lockfile {
    version: u8,
    algorithm: String,
    surfaces: Vec<SurfaceHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SurfaceHash {
    name: String,
    path: String,
    hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Drift {
    name: String,
    path: String,
    expected: Option<String>,
    actual: Option<String>,
}

impl Drift {
    fn from_surface(s: &SurfaceHash, expected: Option<String>, actual: Option<String>) -> Self {
        Self {
            name: s.name.clone(),
            path: s.path.clone(),
            expected,
            actual,
        }
    }
}

/// Run the protocol-drift check.
///
/// `config` is the `[protocol]` section from `taskit.toml`, or `None` in
/// zero-config mode. When `config` is `None` or contains no surfaces the
/// check is skipped silently (nothing to track).
pub fn run(
    root: &Path,
    config: Option<&ProtocolConfig>,
    update: bool,
    warn_only: bool,
    hook: bool,
) -> Result<()> {
    let surfaces: &[SurfaceEntry] = config.map(|c| c.surfaces.as_slice()).unwrap_or(&[]);
    let lock_rel = config
        .map(|c| c.lockfile_path())
        .unwrap_or(DEFAULT_LOCK_PATH);

    if surfaces.is_empty() {
        if !hook {
            eprintln!("protocol-drift: no surfaces configured, skipping");
        }
        return Ok(());
    }

    let hook_path = if hook {
        hook_input::read_file_path()?
    } else {
        None
    };

    if let Some(path) = &hook_path {
        if !is_tracked_surface_path(root, path, surfaces) {
            return Ok(());
        }
        eprintln!(
            "[protocol-drift] {} is a hash-tracked core contract surface",
            display_relative(root, path).display()
        );
    }

    let current = calculate_lockfile(root, surfaces)?;
    let lock_path = root.join(lock_rel);

    if update {
        if crate::runner::is_dry_run() {
            eprintln!("dry-run: write {lock_rel}");
        } else {
            write_lockfile(&lock_path, &current)?;
            eprintln!(
                "protocol-drift: updated {} with {} surface hash(es)",
                lock_rel,
                current.surfaces.len()
            );
        }
        return Ok(());
    }

    let expected = read_lockfile(&lock_path)?;
    let drift = compare_lockfiles(&expected, &current);

    if drift.is_empty() {
        if !hook {
            eprintln!(
                "protocol-drift: OK ({} core contract surface hash(es) match)",
                current.surfaces.len()
            );
        }
        return Ok(());
    }

    report_drift(&drift);
    eprintln!(
        "protocol-drift: if this change is intentional, \
         run `cargo xtask check-protocol-drift --update`"
    );

    if hook || warn_only {
        return Ok(());
    }

    bail!("core contract drift detected");
}

fn calculate_lockfile(root: &Path, surfaces: &[SurfaceEntry]) -> Result<Lockfile> {
    let mut hashes = Vec::with_capacity(surfaces.len());
    for surface in surfaces {
        let path = root.join(&surface.path);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let normalized = super::contract_hash::normalize(&content);
        hashes.push(SurfaceHash {
            name: surface.name.clone(),
            path: surface.path.clone(),
            hash: super::contract_hash::hash(&normalized),
        });
    }
    Ok(Lockfile {
        version: 1,
        algorithm: ALGORITHM.to_string(),
        surfaces: hashes,
    })
}

fn read_lockfile(path: &Path) -> Result<Lockfile> {
    let content = fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read {}; run `cargo xtask check-protocol-drift --update` to create it",
            path.display()
        )
    })?;
    let lockfile: Lockfile = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if lockfile.version != 1 {
        bail!(
            "unsupported protocol drift lockfile version {} in {}",
            lockfile.version,
            path.display()
        );
    }
    if lockfile.algorithm != ALGORITHM {
        bail!(
            "unsupported protocol drift algorithm {} in {} (expected {ALGORITHM})",
            lockfile.algorithm,
            path.display()
        );
    }
    Ok(lockfile)
}

fn write_lockfile(path: &Path, lockfile: &Lockfile) -> Result<()> {
    let mut content = serde_json::to_string_pretty(lockfile)?;
    content.push('\n');
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))
}

fn compare_lockfiles(expected: &Lockfile, actual: &Lockfile) -> Vec<Drift> {
    let mut drift = Vec::new();
    for es in &expected.surfaces {
        let actual_surface = actual.surfaces.iter().find(|s| s.name == es.name);
        match actual_surface {
            Some(a) if a.hash == es.hash => {}
            Some(a) => drift.push(Drift::from_surface(
                es,
                Some(es.hash.clone()),
                Some(a.hash.clone()),
            )),
            None => drift.push(Drift::from_surface(es, Some(es.hash.clone()), None)),
        }
    }
    for actual_surface in &actual.surfaces {
        if expected
            .surfaces
            .iter()
            .all(|s| s.name != actual_surface.name)
        {
            drift.push(Drift::from_surface(
                actual_surface,
                None,
                Some(actual_surface.hash.clone()),
            ));
        }
    }
    drift
}

fn report_drift(drift: &[Drift]) {
    eprintln!("protocol-drift: core contract hash mismatch:");
    for item in drift {
        eprintln!("  - {} ({})", item.name, item.path);
        match (&item.expected, &item.actual) {
            (Some(e), Some(a)) => {
                eprintln!("      expected: {e}");
                eprintln!("      actual  : {a}");
            }
            (Some(e), None) => {
                eprintln!("      expected: {e}");
                eprintln!("      actual  : <missing>");
            }
            (None, Some(a)) => {
                eprintln!("      expected: <missing>");
                eprintln!("      actual  : {a}");
            }
            (None, None) => {}
        }
    }
}

fn is_tracked_surface_path(root: &Path, path: &Path, surfaces: &[SurfaceEntry]) -> bool {
    let relative = display_relative(root, path);
    let relative = relative.to_string_lossy();
    surfaces.iter().any(|s| relative == s.path)
}

fn display_relative(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

/// Parses Claude Code hook stdin JSON to extract the edited file path.
mod hook_input {
    use anyhow::{Context, Result};
    use serde::Deserialize;
    use std::{
        io::{self, IsTerminal as _, Read},
        path::PathBuf,
    };

    #[derive(Debug, Deserialize)]
    struct HookInput {
        tool_input: Option<HookToolInput>,
    }

    #[derive(Debug, Deserialize)]
    struct HookToolInput {
        file_path: Option<PathBuf>,
    }

    pub(super) fn read_file_path() -> Result<Option<PathBuf>> {
        if io::stdin().is_terminal() {
            return Ok(None);
        }
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .context("failed to read hook input from stdin")?;
        if input.trim().is_empty() {
            return Ok(None);
        }
        let parsed: HookInput =
            serde_json::from_str(&input).context("failed to parse hook input JSON")?;
        Ok(parsed.tool_input.and_then(|ti| ti.file_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // --- calculate_lockfile ---

    // Self-contained: writes temp files and passes them as surfaces.
    #[test]
    fn calculate_lockfile_hashes_all_surfaces() {
        let dir = TempDir::new().expect("tempdir");
        let file_a = dir.path().join("types.rs");
        let file_b = dir.path().join("commands.rs");
        fs::write(&file_a, "pub struct Foo {}").unwrap();
        fs::write(&file_b, "pub fn bar() {}").unwrap();

        let surfaces = vec![
            SurfaceEntry {
                name: "types".to_string(),
                path: "types.rs".to_string(),
            },
            SurfaceEntry {
                name: "commands".to_string(),
                path: "commands.rs".to_string(),
            },
        ];

        let lockfile =
            calculate_lockfile(dir.path(), &surfaces).expect("calculate_lockfile should succeed");
        assert_eq!(lockfile.surfaces.len(), 2);
        for surface in &lockfile.surfaces {
            assert!(
                !surface.hash.is_empty(),
                "surface {} should have a non-empty hash",
                surface.name
            );
        }
    }

    #[test]
    fn calculate_lockfile_empty_surfaces_produces_empty_lockfile() {
        let dir = TempDir::new().expect("tempdir");
        let lockfile =
            calculate_lockfile(dir.path(), &[]).expect("should succeed with no surfaces");
        assert!(lockfile.surfaces.is_empty());
    }

    #[test]
    fn calculate_lockfile_hash_changes_when_content_changes() {
        let dir = TempDir::new().expect("tempdir");
        let file = dir.path().join("api.rs");
        let surfaces = vec![SurfaceEntry {
            name: "api".to_string(),
            path: "api.rs".to_string(),
        }];

        fs::write(&file, "pub struct A {}").unwrap();
        let lf1 = calculate_lockfile(dir.path(), &surfaces).unwrap();

        fs::write(&file, "pub struct B {}").unwrap();
        let lf2 = calculate_lockfile(dir.path(), &surfaces).unwrap();

        assert_ne!(
            lf1.surfaces[0].hash, lf2.surfaces[0].hash,
            "hash must change when file content changes"
        );
    }

    #[test]
    fn calculate_lockfile_hash_stable_for_unchanged_content() {
        let dir = TempDir::new().expect("tempdir");
        fs::write(dir.path().join("api.rs"), "pub struct A {}").unwrap();
        let surfaces = vec![SurfaceEntry {
            name: "api".to_string(),
            path: "api.rs".to_string(),
        }];

        let lf1 = calculate_lockfile(dir.path(), &surfaces).unwrap();
        let lf2 = calculate_lockfile(dir.path(), &surfaces).unwrap();
        assert_eq!(lf1.surfaces[0].hash, lf2.surfaces[0].hash);
    }

    #[test]
    fn calculate_lockfile_missing_file_returns_error() {
        let dir = TempDir::new().expect("tempdir");
        let surfaces = vec![SurfaceEntry {
            name: "missing".to_string(),
            path: "does_not_exist.rs".to_string(),
        }];
        assert!(calculate_lockfile(dir.path(), &surfaces).is_err());
    }

    // --- compare_lockfiles ---

    #[test]
    fn compare_lockfiles_reports_hash_mismatch() {
        let expected = Lockfile {
            version: 1,
            algorithm: ALGORITHM.to_string(),
            surfaces: vec![SurfaceHash {
                name: "api-types".to_string(),
                path: "src/types.rs".to_string(),
                hash: "old".to_string(),
            }],
        };
        let actual = Lockfile {
            version: 1,
            algorithm: ALGORITHM.to_string(),
            surfaces: vec![SurfaceHash {
                name: "api-types".to_string(),
                path: "src/types.rs".to_string(),
                hash: "new".to_string(),
            }],
        };
        let drift = compare_lockfiles(&expected, &actual);
        assert_eq!(drift.len(), 1);
        assert_eq!(drift[0].name, "api-types");
    }

    #[test]
    fn compare_lockfiles_no_drift_when_hashes_match() {
        let lf = Lockfile {
            version: 1,
            algorithm: ALGORITHM.to_string(),
            surfaces: vec![SurfaceHash {
                name: "session-types".to_string(),
                path: "src/session.rs".to_string(),
                hash: "abc123".to_string(),
            }],
        };
        let drift = compare_lockfiles(&lf, &lf);
        assert!(
            drift.is_empty(),
            "identical lockfiles should produce no drift"
        );
    }

    #[test]
    fn compare_lockfiles_missing_in_actual_reports_drift_with_none_actual() {
        let expected = Lockfile {
            version: 1,
            algorithm: ALGORITHM.to_string(),
            surfaces: vec![SurfaceHash {
                name: "runtime-api".to_string(),
                path: "src/lib.rs".to_string(),
                hash: "abc".to_string(),
            }],
        };
        let actual = Lockfile {
            version: 1,
            algorithm: ALGORITHM.to_string(),
            surfaces: vec![],
        };
        let drift = compare_lockfiles(&expected, &actual);
        assert_eq!(drift.len(), 1);
        assert_eq!(
            drift[0].actual, None,
            "missing surface should have actual=None"
        );
        assert_eq!(drift[0].expected, Some("abc".to_string()));
    }

    #[test]
    fn compare_lockfiles_new_in_actual_reports_drift_with_none_expected() {
        let expected = Lockfile {
            version: 1,
            algorithm: ALGORITHM.to_string(),
            surfaces: vec![],
        };
        let actual = Lockfile {
            version: 1,
            algorithm: ALGORITHM.to_string(),
            surfaces: vec![SurfaceHash {
                name: "runtime-api".to_string(),
                path: "src/lib.rs".to_string(),
                hash: "xyz".to_string(),
            }],
        };
        let drift = compare_lockfiles(&expected, &actual);
        assert_eq!(drift.len(), 1);
        assert_eq!(
            drift[0].expected, None,
            "new surface should have expected=None"
        );
        assert_eq!(drift[0].actual, Some("xyz".to_string()));
    }

    // --- is_tracked_surface_path ---

    fn make_surfaces(paths: &[&str]) -> Vec<SurfaceEntry> {
        paths
            .iter()
            .enumerate()
            .map(|(i, p)| SurfaceEntry {
                name: format!("surface-{i}"),
                path: p.to_string(),
            })
            .collect()
    }

    #[test]
    fn is_tracked_surface_path_returns_true_for_known_surface() {
        let root = Path::new("/workspace");
        let surfaces = make_surfaces(&["my-lib/src/types.rs"]);
        let path = root.join("my-lib/src/types.rs");
        assert!(is_tracked_surface_path(root, &path, &surfaces));
    }

    #[test]
    fn is_tracked_surface_path_returns_false_for_unknown_file() {
        let root = Path::new("/workspace");
        let surfaces = make_surfaces(&["my-lib/src/types.rs"]);
        let path = root.join("my-lib/src/other.rs");
        assert!(!is_tracked_surface_path(root, &path, &surfaces));
    }

    #[test]
    fn is_tracked_surface_path_returns_false_for_root_relative_mismatch() {
        let root = Path::new("/workspace");
        let surfaces = make_surfaces(&["my-lib/src/types.rs"]);
        let path = Path::new("/other/my-lib/src/types.rs");
        assert!(!is_tracked_surface_path(root, path, &surfaces));
    }

    #[test]
    fn is_tracked_surface_path_returns_false_for_empty_surfaces() {
        let root = Path::new("/workspace");
        let path = root.join("anything.rs");
        assert!(!is_tracked_surface_path(root, &path, &[]));
    }

    // --- read_lockfile validation ---

    fn write_temp_lockfile(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn read_lockfile_rejects_unsupported_version() {
        let dir = TempDir::new().unwrap();
        let content = serde_json::json!({
            "version": 2,
            "algorithm": ALGORITHM,
            "surfaces": []
        })
        .to_string();
        let path = write_temp_lockfile(&dir, "lock.json", &content);
        let result = read_lockfile(&path);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unsupported protocol drift lockfile version")
        );
    }

    #[test]
    fn read_lockfile_rejects_unknown_algorithm() {
        let dir = TempDir::new().unwrap();
        let content = serde_json::json!({
            "version": 1,
            "algorithm": "sha256-unknown-algo",
            "surfaces": []
        })
        .to_string();
        let path = write_temp_lockfile(&dir, "lock.json", &content);
        let result = read_lockfile(&path);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unsupported protocol drift algorithm")
        );
    }

    #[test]
    fn read_lockfile_rejects_malformed_json() {
        let dir = TempDir::new().unwrap();
        let path = write_temp_lockfile(&dir, "lock.json", "not json at all {{{");
        assert!(read_lockfile(&path).is_err());
    }

    #[test]
    fn read_lockfile_accepts_valid_lockfile() {
        let dir = TempDir::new().unwrap();
        let content = serde_json::json!({
            "version": 1,
            "algorithm": ALGORITHM,
            "surfaces": []
        })
        .to_string();
        let path = write_temp_lockfile(&dir, "lock.json", &content);
        let result = read_lockfile(&path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().surfaces.len(), 0);
    }
}
