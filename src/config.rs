use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const CONFIG_FILE: &str = "taskit.toml";

// ---------------------------------------------------------------------------
// Public config types (mirrors DESIGN.md §Config Model)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub workspace: WorkspaceConfig,
    pub protocol: Option<ProtocolConfig>,
    pub ci: Option<CiConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub struct WorkspaceConfig {
    /// Override workspace root (default: discovered automatically).
    pub root: Option<PathBuf>,
    #[serde(default)]
    pub crates: Vec<CrateEntry>,
    #[serde(default)]
    pub propagation: Vec<PropagationEntry>,
}

#[derive(Debug, Deserialize)]
pub struct CrateEntry {
    pub dir: String,
    /// Cargo package name. Defaults to `dir` when absent.
    pub pkg: Option<String>,
}

impl CrateEntry {
    pub fn pkg_name(&self) -> &str {
        self.pkg.as_deref().unwrap_or(&self.dir)
    }
}

#[derive(Debug, Deserialize)]
pub struct PropagationEntry {
    pub source: String,
    pub dependents: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProtocolConfig {
    #[serde(default)]
    pub surfaces: Vec<SurfaceEntry>,
    /// Path to the lockfile, relative to workspace root. Default: `taskit-protocol.lock`.
    pub lockfile: Option<String>,
}

impl ProtocolConfig {
    pub fn lockfile_path(&self) -> &str {
        self.lockfile.as_deref().unwrap_or("taskit-protocol.lock")
    }
}

#[derive(Debug, Deserialize)]
pub struct SurfaceEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct CiConfig {
    #[serde(default)]
    pub steps: Vec<CiStep>,
}

#[derive(Debug, Deserialize)]
pub struct CiStep {
    pub name: String,
    pub cmd: String,
    #[serde(default)]
    pub gate: bool,
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Resolved workspace root and parsed config (or zero-config defaults).
#[derive(Debug)]
pub struct Workspace {
    /// Absolute path to the workspace root directory.
    pub root: PathBuf,
    pub config: Config,
}

/// Find the workspace root and load `taskit.toml` if present.
///
/// Resolution order:
/// 1. Walk up from `$PWD` looking for `taskit.toml`. Its directory is the root.
/// 2. Fall back to `cargo metadata --no-deps` to locate the Cargo workspace root
///    and return a zero-config `Config`.
pub fn load() -> Result<Workspace> {
    let cwd = env::current_dir().context("failed to read current directory")?;

    if let Some(config_path) = find_config_file(&cwd) {
        let root = config_path
            .parent()
            .expect("config file always has a parent directory")
            .to_path_buf();
        let config = parse_config(&config_path)?;
        // If [workspace].root is set, resolve it relative to the config file's directory.
        let root = match &config.workspace.root {
            Some(override_root) => {
                let resolved = root.join(override_root);
                resolved.canonicalize().with_context(|| {
                    format!("failed to resolve workspace.root = {}", resolved.display())
                })?
            }
            None => root,
        };
        return Ok(Workspace { root, config });
    }

    // No taskit.toml found — fall back to cargo metadata.
    let root = cargo_workspace_root().context(
        "no taskit.toml found and `cargo metadata` failed; \
         run taskit from inside a Cargo workspace",
    )?;
    Ok(Workspace {
        root,
        config: Config::default(),
    })
}

/// Walk up the directory tree from `start`, returning the path to the first
/// `taskit.toml` found, or `None` if the filesystem root is reached.
fn find_config_file(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join(CONFIG_FILE);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn parse_config(path: &Path) -> Result<Config> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
}

/// Ask Cargo for the workspace root via `cargo metadata`.
fn cargo_workspace_root() -> Result<PathBuf> {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .context("failed to run `cargo metadata`")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("`cargo metadata` failed: {stderr}");
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .context("failed to parse `cargo metadata` output")?;

    let root = json["workspace_root"]
        .as_str()
        .context("`cargo metadata` output missing `workspace_root` field")?;

    Ok(PathBuf::from(root))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_temp_config(content: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(CONFIG_FILE);
        fs::write(&path, content).unwrap();
        (dir, path)
    }

    // --- find_config_file ---

    #[test]
    fn find_config_file_returns_none_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        assert!(find_config_file(dir.path()).is_none());
    }

    #[test]
    fn find_config_file_finds_file_in_start_dir() {
        let (dir, path) = write_temp_config("");
        assert_eq!(find_config_file(dir.path()), Some(path));
    }

    #[test]
    fn find_config_file_finds_file_in_parent() {
        let (dir, config_path) = write_temp_config("");
        let child = dir.path().join("sub");
        fs::create_dir(&child).unwrap();
        assert_eq!(find_config_file(&child), Some(config_path));
    }

    #[test]
    fn find_config_file_finds_file_two_levels_up() {
        let (dir, config_path) = write_temp_config("");
        let grandchild = dir.path().join("a").join("b");
        fs::create_dir_all(&grandchild).unwrap();
        assert_eq!(find_config_file(&grandchild), Some(config_path));
    }

    // --- parse_config ---

    #[test]
    fn parse_config_empty_file_returns_defaults() {
        let (_dir, path) = write_temp_config("");
        let config = parse_config(&path).unwrap();
        assert!(config.workspace.crates.is_empty());
        assert!(config.protocol.is_none());
        assert!(config.ci.is_none());
    }

    #[test]
    fn parse_config_workspace_crates() {
        let (_dir, path) = write_temp_config(
            r#"
[[workspace.crates]]
dir = "my-lib"

[[workspace.crates]]
dir = "my-cli"
pkg = "my-binary"
"#,
        );
        let config = parse_config(&path).unwrap();
        assert_eq!(config.workspace.crates.len(), 2);
        assert_eq!(config.workspace.crates[0].dir, "my-lib");
        assert_eq!(config.workspace.crates[0].pkg_name(), "my-lib");
        assert_eq!(config.workspace.crates[1].dir, "my-cli");
        assert_eq!(config.workspace.crates[1].pkg_name(), "my-binary");
    }

    #[test]
    fn parse_config_propagation() {
        let (_dir, path) = write_temp_config(
            r#"
[[workspace.propagation]]
source = "my-common"
dependents = ["my-api", "my-cli"]
"#,
        );
        let config = parse_config(&path).unwrap();
        assert_eq!(config.workspace.propagation.len(), 1);
        assert_eq!(config.workspace.propagation[0].source, "my-common");
        assert_eq!(
            config.workspace.propagation[0].dependents,
            ["my-api", "my-cli"]
        );
    }

    #[test]
    fn parse_config_protocol_surfaces() {
        let (_dir, path) = write_temp_config(
            r#"
[[protocol.surfaces]]
name = "api-types"
path = "my-api/src/types.rs"

[protocol]
lockfile = "my.lock"
"#,
        );
        let config = parse_config(&path).unwrap();
        let proto = config.protocol.unwrap();
        assert_eq!(proto.surfaces.len(), 1);
        assert_eq!(proto.surfaces[0].name, "api-types");
        assert_eq!(proto.lockfile_path(), "my.lock");
    }

    #[test]
    fn parse_config_protocol_default_lockfile() {
        let (_dir, path) = write_temp_config(
            r#"
[[protocol.surfaces]]
name = "x"
path = "x.rs"
"#,
        );
        let config = parse_config(&path).unwrap();
        assert_eq!(
            config.protocol.unwrap().lockfile_path(),
            "taskit-protocol.lock"
        );
    }

    #[test]
    fn parse_config_ci_steps() {
        let (_dir, path) = write_temp_config(
            r#"
[[ci.steps]]
name = "fmt-check"
cmd = "fmt --check"
gate = true

[[ci.steps]]
name = "lint"
cmd = "lint"
"#,
        );
        let config = parse_config(&path).unwrap();
        let ci = config.ci.unwrap();
        assert_eq!(ci.steps.len(), 2);
        assert_eq!(ci.steps[0].name, "fmt-check");
        assert!(ci.steps[0].gate);
        assert!(!ci.steps[1].gate);
    }

    #[test]
    fn parse_config_rejects_invalid_toml() {
        let (_dir, path) = write_temp_config("[[[ not valid toml");
        assert!(parse_config(&path).is_err());
    }

    // --- cargo_workspace_root ---

    #[test]
    fn cargo_workspace_root_returns_a_directory() {
        let root = cargo_workspace_root().expect("cargo metadata should succeed in a workspace");
        assert!(
            root.is_dir(),
            "workspace root should be an existing directory"
        );
    }

    // --- crate_entry helpers ---

    #[test]
    fn pkg_name_defaults_to_dir() {
        let e = CrateEntry {
            dir: "foo".into(),
            pkg: None,
        };
        assert_eq!(e.pkg_name(), "foo");
    }

    #[test]
    fn pkg_name_uses_explicit_pkg() {
        let e = CrateEntry {
            dir: "foo".into(),
            pkg: Some("bar".into()),
        };
        assert_eq!(e.pkg_name(), "bar");
    }
}
