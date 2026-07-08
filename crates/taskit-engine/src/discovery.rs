use cargo_metadata::MetadataCommand;
use std::path::{Path, PathBuf};
use taskit_types::error::{TaskitError, TaskitResultExt};

use crate::config::PropagationEntry;

/// A discovered workspace crate from cargo metadata.
#[derive(Debug, Clone)]
pub struct DiscoveredCrate {
    pub dir: String,
    pub pkg: String,
    pub manifest_path: PathBuf,
}

/// A discovered protocol surface from convention scanning.
#[derive(Debug, Clone)]
pub struct DiscoveredSurface {
    pub name: String,
    pub path: String,
}

/// Port: abstracts cargo metadata retrieval for testability.
pub trait MetadataSource {
    fn workspace_members(&self) -> Result<Vec<DiscoveredCrate>, TaskitError>;
    fn intra_workspace_deps(&self) -> Result<Vec<(String, String)>, TaskitError>;
}

/// Build propagation rules from intra-workspace dependency edges.
///
/// Each `(source, dependent)` edge where both names appear in
/// `known_crates` produces a propagation entry: if `source` changes,
/// `dependent` is also affected.
pub fn derive_propagation(
    deps: &[(String, String)],
    known_crates: &[String],
) -> Vec<PropagationEntry> {
    let known: std::collections::HashSet<&String> = known_crates.iter().collect();
    let mut map: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for (source, dependent) in deps {
        if known.contains(source) && known.contains(dependent) {
            map.entry(source.clone())
                .or_default()
                .push(dependent.clone());
        }
    }
    map.into_iter()
        .map(|(source, mut dependents)| {
            dependents.sort();
            dependents.dedup();
            PropagationEntry { source, dependents }
        })
        .collect()
}

/// Filename patterns that indicate protocol surfaces.
const SURFACE_PATTERNS: &[(&str, &str)] = &[
    ("types.rs", "types"),
    ("api.rs", "api"),
    ("schema.graphql", "graphql-schema"),
    ("schema.json", "json-schema"),
    ("openapi.yml", "openapi"),
    ("openapi.yaml", "openapi"),
    ("openapi.json", "openapi"),
];

/// Scan workspace for convention-based protocol surface files.
///
/// Walks the directory tree, skipping `target/` and hidden directories.
/// Files matching known patterns or `*.proto` are returned as surfaces.
pub fn scan_surfaces(workspace_root: &Path) -> Result<Vec<DiscoveredSurface>, TaskitError> {
    let mut surfaces = Vec::new();
    walk_for_surfaces(workspace_root, workspace_root, &mut surfaces)?;
    surfaces.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(surfaces)
}

fn walk_for_surfaces(
    root: &Path,
    dir: &Path,
    surfaces: &mut Vec<DiscoveredSurface>,
) -> Result<(), TaskitError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') || name_str == "target" {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            walk_for_surfaces(root, &path, surfaces)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            let file_name = name_str.to_string();

            // Check known filename patterns
            for (pattern, suffix) in SURFACE_PATTERNS {
                if file_name == *pattern {
                    let crate_name = infer_crate_name(&rel);
                    surfaces.push(DiscoveredSurface {
                        name: format!("{crate_name}/{suffix}"),
                        path: rel.clone(),
                    });
                    break;
                }
            }

            // Check .proto files
            if file_name.ends_with(".proto") {
                let crate_name = infer_crate_name(&rel);
                let stem = file_name.trim_end_matches(".proto");
                surfaces.push(DiscoveredSurface {
                    name: format!("{crate_name}/{stem}"),
                    path: rel,
                });
            }
        }
    }
    Ok(())
}

/// Infer the owning crate name from a relative file path.
///
/// Uses the first path component as the crate directory name.
fn infer_crate_name(rel_path: &str) -> &str {
    rel_path.split('/').next().unwrap_or("unknown")
}

/// Production adapter: reads cargo metadata from the real workspace.
pub struct CargoMetadataSource {
    pub workspace_root: PathBuf,
}

impl MetadataSource for CargoMetadataSource {
    fn workspace_members(&self) -> Result<Vec<DiscoveredCrate>, TaskitError> {
        let metadata = MetadataCommand::new()
            .current_dir(&self.workspace_root)
            .no_deps()
            .exec()
            .err_context("failed to run `cargo metadata`")?;

        let ws_root = metadata.workspace_root.as_std_path();
        let mut crates = Vec::new();
        for pkg_id in &metadata.workspace_members {
            let pkg = metadata
                .packages
                .iter()
                .find(|p| &p.id == pkg_id)
                .ok_or_else(|| TaskitError::other("workspace member not found in packages"))?;
            let manifest_dir = pkg
                .manifest_path
                .parent()
                .ok_or_else(|| TaskitError::other("manifest_path has no parent"))?;
            let dir = manifest_dir
                .strip_prefix(ws_root)
                .unwrap_or(manifest_dir)
                .to_string();
            let dir = if dir == "." || dir.is_empty() {
                pkg.name.clone()
            } else {
                dir
            };
            crates.push(DiscoveredCrate {
                dir,
                pkg: pkg.name.clone(),
                manifest_path: pkg.manifest_path.clone().into(),
            });
        }
        Ok(crates)
    }

    fn intra_workspace_deps(&self) -> Result<Vec<(String, String)>, TaskitError> {
        let metadata = MetadataCommand::new()
            .current_dir(&self.workspace_root)
            .exec()
            .err_context("failed to run `cargo metadata`")?;

        let member_names: std::collections::HashSet<String> = metadata
            .workspace_members
            .iter()
            .filter_map(|id| metadata.packages.iter().find(|p| &p.id == id))
            .map(|p| p.name.clone())
            .collect();

        let mut edges = Vec::new();
        for pkg_id in &metadata.workspace_members {
            let pkg = metadata.packages.iter().find(|p| &p.id == pkg_id).unwrap();
            for dep in &pkg.dependencies {
                if member_names.contains(&dep.name) {
                    edges.push((dep.name.clone(), pkg.name.clone()));
                }
            }
        }
        Ok(edges)
    }
}

#[cfg(test)]
pub(crate) struct FakeMetadataSource {
    pub members: Vec<DiscoveredCrate>,
    pub deps: Vec<(String, String)>,
}

#[cfg(test)]
impl MetadataSource for FakeMetadataSource {
    fn workspace_members(&self) -> Result<Vec<DiscoveredCrate>, TaskitError> {
        Ok(self.members.clone())
    }
    fn intra_workspace_deps(&self) -> Result<Vec<(String, String)>, TaskitError> {
        Ok(self.deps.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integration_discover_taskit_workspace() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let config = crate::config::discover(&root).unwrap();
        assert!(
            !config.workspace.crates.is_empty(),
            "should discover at least one crate"
        );
        let names: Vec<&str> = config
            .workspace
            .crates
            .iter()
            .map(|c| c.pkg_name())
            .collect();
        assert!(
            names.contains(&"taskit"),
            "should discover taskit itself: {names:?}"
        );
    }

    #[test]
    fn fake_source_returns_members() {
        let source = FakeMetadataSource {
            members: vec![DiscoveredCrate {
                dir: "my-lib".into(),
                pkg: "my-lib".into(),
                manifest_path: PathBuf::from("/ws/my-lib/Cargo.toml"),
            }],
            deps: vec![],
        };
        let members = source.workspace_members().unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].pkg, "my-lib");
    }

    #[test]
    fn scan_surfaces_finds_types_rs() {
        let dir = tempfile::tempdir().unwrap();
        let crate_dir = dir.path().join("my-api/src");
        std::fs::create_dir_all(&crate_dir).unwrap();
        std::fs::write(crate_dir.join("types.rs"), "pub struct Foo;").unwrap();
        let surfaces = scan_surfaces(dir.path()).unwrap();
        assert_eq!(surfaces.len(), 1);
        assert_eq!(surfaces[0].name, "my-api/types");
        assert_eq!(surfaces[0].path, "my-api/src/types.rs");
    }

    #[test]
    fn scan_surfaces_skips_target_dir() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target/debug/build/my-api/src");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("types.rs"), "generated").unwrap();
        let surfaces = scan_surfaces(dir.path()).unwrap();
        assert!(surfaces.is_empty());
    }

    #[test]
    fn scan_surfaces_finds_graphql_schema() {
        let dir = tempfile::tempdir().unwrap();
        let crate_dir = dir.path().join("my-api");
        std::fs::create_dir_all(&crate_dir).unwrap();
        std::fs::write(crate_dir.join("schema.graphql"), "type Q{}").unwrap();
        let surfaces = scan_surfaces(dir.path()).unwrap();
        assert_eq!(surfaces.len(), 1);
        assert!(surfaces[0].name.contains("graphql-schema"));
    }

    #[test]
    fn scan_surfaces_finds_proto_files() {
        let dir = tempfile::tempdir().unwrap();
        let proto_dir = dir.path().join("my-svc/proto");
        std::fs::create_dir_all(&proto_dir).unwrap();
        std::fs::write(proto_dir.join("service.proto"), "syntax='proto3';").unwrap();
        let surfaces = scan_surfaces(dir.path()).unwrap();
        assert_eq!(surfaces.len(), 1);
        assert!(surfaces[0].name.contains("service"));
    }

    #[test]
    fn scan_surfaces_empty_workspace_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let surfaces = scan_surfaces(dir.path()).unwrap();
        assert!(surfaces.is_empty());
    }

    #[test]
    fn scan_surfaces_skips_hidden_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let hidden = dir.path().join(".hidden/src");
        std::fs::create_dir_all(&hidden).unwrap();
        std::fs::write(hidden.join("types.rs"), "struct X;").unwrap();
        let surfaces = scan_surfaces(dir.path()).unwrap();
        assert!(surfaces.is_empty());
    }

    #[test]
    fn derive_propagation_empty_deps() {
        let result = derive_propagation(&[], &["a".into(), "b".into()]);
        assert!(result.is_empty());
    }

    #[test]
    fn derive_propagation_groups_by_source() {
        let deps = vec![
            ("common".into(), "api".into()),
            ("common".into(), "cli".into()),
        ];
        let known = vec!["common".into(), "api".into(), "cli".into()];
        let result = derive_propagation(&deps, &known);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source, "common");
        assert!(result[0].dependents.contains(&"api".to_string()));
        assert!(result[0].dependents.contains(&"cli".to_string()));
    }

    #[test]
    fn derive_propagation_ignores_external_deps() {
        let deps = vec![("serde".into(), "api".into())];
        let known = vec!["api".into()];
        let result = derive_propagation(&deps, &known);
        assert!(result.is_empty());
    }

    #[test]
    fn derive_propagation_multiple_sources() {
        let deps = vec![
            ("common".into(), "api".into()),
            ("utils".into(), "cli".into()),
        ];
        let known = vec!["common".into(), "utils".into(), "api".into(), "cli".into()];
        let result = derive_propagation(&deps, &known);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn cargo_metadata_source_finds_taskit_itself() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let source = CargoMetadataSource {
            workspace_root: root,
        };
        let members = source.workspace_members().unwrap();
        assert!(
            members.iter().any(|c| c.pkg == "taskit"),
            "should discover taskit itself: {members:?}"
        );
    }

    #[test]
    fn fake_source_returns_deps() {
        let source = FakeMetadataSource {
            members: vec![],
            deps: vec![("a".into(), "b".into())],
        };
        let deps = source.intra_workspace_deps().unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0], ("a".to_string(), "b".to_string()));
    }
}
