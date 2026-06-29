// Engine-specific config loading and discovery.
// Type definitions live in taskit_types::config; re-exported here for sibling modules.

pub use taskit_types::config::{
    CiConfig, CiStep, Config, CoverageConfig, CrateEntry, FlowConfig, PropagationEntry,
    ProtocolConfig, SurfaceEntry, WorkspaceConfig,
};

use anyhow::Context;
use std::{
    env, fs,
    path::{Path, PathBuf},
};
use taskit_types::error::TaskitError;

use crate::Workspace;

const CONFIG_FILE: &str = "taskit.toml";

/// Build a Config entirely from cargo metadata + conventions.
pub fn discover(workspace_root: &Path) -> Result<Config, TaskitError> {
    use crate::discovery::CargoMetadataSource;
    let source = CargoMetadataSource {
        workspace_root: workspace_root.to_path_buf(),
    };
    discover_with(workspace_root, &source)
}

/// Build a Config from a given metadata source + conventions.
pub fn discover_with(
    workspace_root: &Path,
    source: &dyn crate::discovery::MetadataSource,
) -> Result<Config, TaskitError> {
    use crate::discovery;

    let members = source.workspace_members()?;
    let deps = source.intra_workspace_deps()?;

    let crates: Vec<CrateEntry> = members
        .iter()
        .map(|m| CrateEntry {
            dir: m.dir.clone(),
            pkg: if m.pkg == m.dir {
                None
            } else {
                Some(m.pkg.clone())
            },
        })
        .collect();

    let known_names: Vec<String> = members.iter().map(|m| m.pkg.clone()).collect();
    let propagation = discovery::derive_propagation(&deps, &known_names);

    let surfaces = discovery::scan_surfaces(workspace_root)?;
    let protocol = if surfaces.is_empty() {
        None
    } else {
        Some(ProtocolConfig {
            surfaces: surfaces
                .into_iter()
                .map(|s| SurfaceEntry {
                    name: s.name,
                    path: s.path,
                })
                .collect(),
            lockfile: None,
        })
    };

    Ok(Config {
        workspace: WorkspaceConfig {
            root: None,
            crates,
            propagation,
            offline_skip: None,
        },
        protocol,
        ci: None,
        coverage: None,
        flow: None,
    })
}

/// Find the workspace root and load `taskit.toml` if present.
pub fn load() -> Result<Workspace, TaskitError> {
    let cwd = env::current_dir().context("failed to read current directory")?;

    if let Some(config_path) = find_config_file(&cwd) {
        let root = config_path
            .parent()
            .expect("config file always has a parent directory")
            .to_path_buf();
        let mut config = parse_config(&config_path)?;
        let root = match &config.workspace.root {
            Some(override_root) => {
                let resolved = root.join(override_root);
                resolved.canonicalize().with_context(|| {
                    format!("failed to resolve workspace.root = {}", resolved.display())
                })?
            }
            None => root,
        };
        if let Ok(discovered) = discover(&root) {
            merge_discovered(&mut config, discovered);
        }
        return Ok(Workspace { root, config });
    }

    let root = cargo_workspace_root().context(
        "no taskit.toml found and `cargo metadata` failed; \
         run taskit from inside a Cargo workspace",
    )?;
    let config = discover(&root).unwrap_or_default();
    Ok(Workspace { root, config })
}

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

fn merge_discovered(config: &mut Config, discovered: Config) {
    if config.workspace.crates.is_empty() {
        config.workspace.crates = discovered.workspace.crates;
    }
    if config.workspace.propagation.is_empty() {
        config.workspace.propagation = discovered.workspace.propagation;
    }
    if config.protocol.is_none() {
        config.protocol = discovered.protocol;
    }
}

fn parse_config(path: &Path) -> Result<Config, TaskitError> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))?)
}

fn cargo_workspace_root() -> Result<PathBuf, TaskitError> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .no_deps()
        .exec()
        .context("failed to run `cargo metadata`")?;
    Ok(metadata.workspace_root.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::fs;

    proptest! {
        #[test]
        fn prop_parse_config_never_panics(
            content in "[a-z_\\[\\]=\"\n ]{0,120}",
        ) {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(CONFIG_FILE);
            fs::write(&path, &content).unwrap();
            // Must not panic — either Ok or Err is acceptable.
            let _ = parse_config(&path);
        }
    }

    fn write_temp_config(content: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(CONFIG_FILE);
        fs::write(&path, content).unwrap();
        (dir, path)
    }

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

    #[test]
    fn cargo_workspace_root_returns_a_directory() {
        let root = cargo_workspace_root().expect("cargo metadata should succeed in a workspace");
        assert!(
            root.is_dir(),
            "workspace root should be an existing directory"
        );
    }

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

    #[test]
    fn merge_fills_empty_crates_from_discovery() {
        let mut config = Config::default();
        let discovered = Config {
            workspace: WorkspaceConfig {
                root: None,
                crates: vec![CrateEntry {
                    dir: "discovered".into(),
                    pkg: None,
                }],
                propagation: vec![],
                offline_skip: None,
            },
            ..Config::default()
        };
        merge_discovered(&mut config, discovered);
        assert_eq!(config.workspace.crates.len(), 1);
        assert_eq!(config.workspace.crates[0].dir, "discovered");
    }

    #[test]
    fn merge_keeps_explicit_crates_over_discovery() {
        let mut config = Config {
            workspace: WorkspaceConfig {
                root: None,
                crates: vec![CrateEntry {
                    dir: "explicit".into(),
                    pkg: None,
                }],
                propagation: vec![],
                offline_skip: None,
            },
            ..Config::default()
        };
        let discovered = Config {
            workspace: WorkspaceConfig {
                root: None,
                crates: vec![CrateEntry {
                    dir: "discovered".into(),
                    pkg: None,
                }],
                propagation: vec![],
                offline_skip: None,
            },
            ..Config::default()
        };
        merge_discovered(&mut config, discovered);
        assert_eq!(config.workspace.crates.len(), 1);
        assert_eq!(config.workspace.crates[0].dir, "explicit");
    }

    #[test]
    fn merge_fills_empty_propagation_from_discovery() {
        let mut config = Config::default();
        let discovered = Config {
            workspace: WorkspaceConfig {
                root: None,
                crates: vec![],
                propagation: vec![PropagationEntry {
                    source: "common".into(),
                    dependents: vec!["api".into()],
                }],
                offline_skip: None,
            },
            ..Config::default()
        };
        merge_discovered(&mut config, discovered);
        assert_eq!(config.workspace.propagation.len(), 1);
    }

    #[test]
    fn merge_fills_protocol_when_none() {
        let mut config = Config::default();
        let discovered = Config {
            protocol: Some(ProtocolConfig {
                surfaces: vec![SurfaceEntry {
                    name: "found".into(),
                    path: "x.rs".into(),
                }],
                lockfile: None,
            }),
            ..Config::default()
        };
        merge_discovered(&mut config, discovered);
        assert!(config.protocol.is_some());
    }

    #[test]
    fn merge_keeps_explicit_protocol() {
        let mut config = Config {
            protocol: Some(ProtocolConfig {
                surfaces: vec![],
                lockfile: Some("my.lock".into()),
            }),
            ..Config::default()
        };
        let discovered = Config {
            protocol: Some(ProtocolConfig {
                surfaces: vec![SurfaceEntry {
                    name: "found".into(),
                    path: "x.rs".into(),
                }],
                lockfile: None,
            }),
            ..Config::default()
        };
        merge_discovered(&mut config, discovered);
        assert_eq!(config.protocol.as_ref().unwrap().lockfile_path(), "my.lock");
    }

    #[test]
    fn load_returns_workspace_from_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_content = r#"
[[workspace.crates]]
dir = "my-lib"
"#;
        fs::write(dir.path().join(CONFIG_FILE), config_content).unwrap();
        // Create a minimal Cargo.toml so cargo metadata can find the workspace
        fs::write(dir.path().join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();

        // parse_config + find_config_file are the core of load(); test them
        // directly since load() uses env::current_dir() which we can't control
        let path = find_config_file(dir.path()).unwrap();
        let config = parse_config(&path).unwrap();
        assert_eq!(config.workspace.crates.len(), 1);
        assert_eq!(config.workspace.crates[0].dir, "my-lib");
    }

    use crate::discovery::{DiscoveredCrate, FakeMetadataSource};

    #[test]
    fn discover_with_populates_crates_from_source() {
        let source = FakeMetadataSource {
            members: vec![DiscoveredCrate {
                dir: "my-lib".into(),
                pkg: "my-lib".into(),
                manifest_path: PathBuf::from("/ws/my-lib/Cargo.toml"),
            }],
            deps: vec![],
        };
        let dir = tempfile::tempdir().unwrap();
        let config = discover_with(dir.path(), &source).unwrap();
        assert_eq!(config.workspace.crates.len(), 1);
        assert_eq!(config.workspace.crates[0].dir, "my-lib");
    }

    #[test]
    fn discover_with_derives_propagation() {
        let source = FakeMetadataSource {
            members: vec![
                DiscoveredCrate {
                    dir: "common".into(),
                    pkg: "common".into(),
                    manifest_path: PathBuf::from("/ws/common/Cargo.toml"),
                },
                DiscoveredCrate {
                    dir: "api".into(),
                    pkg: "api".into(),
                    manifest_path: PathBuf::from("/ws/api/Cargo.toml"),
                },
            ],
            deps: vec![("common".into(), "api".into())],
        };
        let dir = tempfile::tempdir().unwrap();
        let config = discover_with(dir.path(), &source).unwrap();
        assert_eq!(config.workspace.propagation.len(), 1);
        assert_eq!(config.workspace.propagation[0].source, "common");
    }

    #[test]
    fn discover_with_no_members_returns_empty_config() {
        let source = FakeMetadataSource {
            members: vec![],
            deps: vec![],
        };
        let dir = tempfile::tempdir().unwrap();
        let config = discover_with(dir.path(), &source).unwrap();
        assert!(config.workspace.crates.is_empty());
        assert!(config.workspace.propagation.is_empty());
    }

    #[test]
    fn discover_with_sets_pkg_when_different_from_dir() {
        let source = FakeMetadataSource {
            members: vec![DiscoveredCrate {
                dir: "my-cli".into(),
                pkg: "my-binary".into(),
                manifest_path: PathBuf::from("/ws/my-cli/Cargo.toml"),
            }],
            deps: vec![],
        };
        let dir = tempfile::tempdir().unwrap();
        let config = discover_with(dir.path(), &source).unwrap();
        assert_eq!(config.workspace.crates[0].pkg_name(), "my-binary");
    }
}
