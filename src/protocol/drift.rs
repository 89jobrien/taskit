use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const LOCK_PATH: &str = "xtask/protocol-drift.lock";
const ALGORITHM: &str = "sha256-normalized-core-contract-v1";

const SURFACES: &[Surface] = &[
    Surface {
        name: "graphql-schema",
        path: "maestro-api/src/graphql/mod.rs",
    },
    Surface {
        name: "k8s-session-spec",
        path: "maestro-k8s/src/session_spec.rs",
    },
    Surface {
        name: "session-types",
        path: "maestro-common/src/session.rs",
    },
    Surface {
        name: "config-types",
        path: "maestro-common/src/maestro_config.rs",
    },
    Surface {
        name: "cli-commands",
        path: "maestro-cli/src/commands/mod.rs",
    },
    Surface {
        name: "runtime-api",
        path: "maestro-runtime/src/lib.rs",
    },
];

#[derive(Debug, Clone, Copy)]
struct Surface {
    name: &'static str,
    path: &'static str,
}

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

pub fn run(root: &Path, update: bool, warn_only: bool, hook: bool) -> Result<()> {
    let hook_path = if hook {
        hook_input::read_file_path()?
    } else {
        None
    };

    if let Some(path) = &hook_path {
        if !is_tracked_surface_path(root, path) {
            return Ok(());
        }
        eprintln!(
            "[protocol-drift] {} is a hash-tracked core contract surface",
            display_relative(root, path).display()
        );
    }

    let current = calculate_lockfile(root)?;
    let lock_path = root.join(LOCK_PATH);

    if update {
        if crate::runner::is_dry_run() {
            eprintln!("dry-run: write {LOCK_PATH}");
        } else {
            write_lockfile(&lock_path, &current)?;
            eprintln!(
                "protocol-drift: updated {} with {} surface hash(es)",
                LOCK_PATH,
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

fn calculate_lockfile(root: &Path) -> Result<Lockfile> {
    let mut surfaces = Vec::with_capacity(SURFACES.len());
    for surface in SURFACES {
        let path = root.join(surface.path);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let normalized = super::contract_hash::normalize(&content);
        surfaces.push(SurfaceHash {
            name: surface.name.to_string(),
            path: surface.path.to_string(),
            hash: super::contract_hash::hash(&normalized),
        });
    }
    Ok(Lockfile {
        version: 1,
        algorithm: ALGORITHM.to_string(),
        surfaces,
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

fn is_tracked_surface_path(root: &Path, path: &Path) -> bool {
    let relative = display_relative(root, path);
    let relative = relative.to_string_lossy();
    SURFACES.iter().any(|s| relative == s.path)
}

fn display_relative(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires maestro workspace surface files; run from within maestro"]
    fn calculate_lockfile_hashes_all_surfaces() {
        // Use the actual workspace root so all surface files exist.
        // CARGO_MANIFEST_DIR is xtask/; its parent is the workspace root.
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = manifest.parent().expect("xtask must be inside workspace");
        let lockfile = calculate_lockfile(root).expect("calculate_lockfile should succeed");
        assert_eq!(
            lockfile.surfaces.len(),
            SURFACES.len(),
            "lockfile surface count should match SURFACES constant"
        );
        for surface in &lockfile.surfaces {
            assert!(
                !surface.hash.is_empty(),
                "surface {} should have a non-empty hash",
                surface.name
            );
        }
    }

    #[test]
    fn compare_lockfiles_reports_hash_mismatch() {
        let expected = Lockfile {
            version: 1,
            algorithm: ALGORITHM.to_string(),
            surfaces: vec![SurfaceHash {
                name: "graphql-schema".to_string(),
                path: "maestro-api/src/graphql/mod.rs".to_string(),
                hash: "old".to_string(),
            }],
        };
        let actual = Lockfile {
            version: 1,
            algorithm: ALGORITHM.to_string(),
            surfaces: vec![SurfaceHash {
                name: "graphql-schema".to_string(),
                path: "maestro-api/src/graphql/mod.rs".to_string(),
                hash: "new".to_string(),
            }],
        };
        let drift = compare_lockfiles(&expected, &actual);
        assert_eq!(drift.len(), 1);
        assert_eq!(drift[0].name, "graphql-schema");
    }

    #[test]
    fn compare_lockfiles_no_drift_when_hashes_match() {
        let lf = Lockfile {
            version: 1,
            algorithm: ALGORITHM.to_string(),
            surfaces: vec![SurfaceHash {
                name: "session-types".to_string(),
                path: "maestro-common/src/session.rs".to_string(),
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
                path: "maestro-runtime/src/lib.rs".to_string(),
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
                path: "maestro-runtime/src/lib.rs".to_string(),
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

    #[test]
    fn is_tracked_surface_path_returns_true_for_known_surface() {
        let root = std::path::Path::new("/workspace");
        let path = root.join("maestro-common/src/session.rs");
        assert!(is_tracked_surface_path(root, &path));
    }

    #[test]
    fn is_tracked_surface_path_returns_false_for_unknown_file() {
        let root = std::path::Path::new("/workspace");
        let path = root.join("maestro-cli/src/commands/start.rs");
        assert!(!is_tracked_surface_path(root, &path));
    }

    #[test]
    fn is_tracked_surface_path_returns_false_for_root_relative_mismatch() {
        // Path without the root prefix should still not match an arbitrary file.
        let root = std::path::Path::new("/workspace");
        let path = std::path::Path::new("/other/maestro-common/src/session.rs");
        assert!(!is_tracked_surface_path(root, path));
    }

    // --- read_lockfile validation ---

    fn write_temp_lockfile(content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "xtask-test-lockfile-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn read_lockfile_rejects_unsupported_version() {
        let content = serde_json::json!({
            "version": 2,
            "algorithm": ALGORITHM,
            "surfaces": []
        })
        .to_string();
        let path = write_temp_lockfile(&content);
        let result = read_lockfile(&path);
        let _ = std::fs::remove_file(&path);
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
        let content = serde_json::json!({
            "version": 1,
            "algorithm": "sha256-unknown-algo",
            "surfaces": []
        })
        .to_string();
        let path = write_temp_lockfile(&content);
        let result = read_lockfile(&path);
        let _ = std::fs::remove_file(&path);
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
        let path = write_temp_lockfile("not json at all {{{");
        let result = read_lockfile(&path);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_err());
    }

    #[test]
    fn read_lockfile_accepts_valid_lockfile() {
        let content = serde_json::json!({
            "version": 1,
            "algorithm": ALGORITHM,
            "surfaces": []
        })
        .to_string();
        let path = write_temp_lockfile(&content);
        let result = read_lockfile(&path);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().surfaces.len(), 0);
    }
}
