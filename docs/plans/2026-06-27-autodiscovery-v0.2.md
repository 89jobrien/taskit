# Plan: Auto-discovery (v0.2.0)

## Goal

Add automatic workspace discovery from `cargo metadata` so taskit works
out of the box with zero configuration -- crate list, dependency
propagation, and convention-based protocol surface detection are all
derived at runtime.

## Architecture

- Crates affected: `taskit` (single crate)
- New traits/types: `MetadataSource` trait, `CargoMetadataSource` adapter,
  `DiscoveredCrate`, `DiscoveredSurface` in `src/discovery.rs`;
  `Config::discover()` and `Config::discover_with()` in `src/config.rs`
- Data flow: `cargo metadata` JSON -> `CargoMetadataSource` ->
  `Config::discover_with()` -> merged `Config` in `config::load()`

## Tech Stack

- Rust edition 2024
- New dependency: `cargo_metadata = "0.19"` (typed cargo metadata parsing)
- Existing: `anyhow`, `serde`, `toml`

## Tasks

### Task 1: Add `cargo_metadata` dependency

**Crate**: `taskit`
**File(s)**: `Cargo.toml`

1. Add `cargo_metadata` to `[dependencies]`:

   ```toml
   cargo_metadata = "0.19"
   ```

2. Verify:

   ```
   cargo check    -> compiles
   ```

3. Commit: `chore(taskit): add cargo_metadata dependency`

---

### Task 2: Define `MetadataSource` trait and types

**Crate**: `taskit`
**File(s)**: `src/discovery.rs` (new), `src/lib.rs`
**Run**: `cargo test -p taskit -- discovery`

1. Write failing test in `src/discovery.rs`:

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       struct FakeSource {
           members: Vec<DiscoveredCrate>,
           deps: Vec<(String, String)>,
       }

       impl MetadataSource for FakeSource {
           fn workspace_members(&self) -> Result<Vec<DiscoveredCrate>> {
               Ok(self.members.clone())
           }
           fn intra_workspace_deps(&self) -> Result<Vec<(String, String)>> {
               Ok(self.deps.clone())
           }
       }

       #[test]
       fn fake_source_returns_members() {
           let source = FakeSource {
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
   }
   ```

   Run: `cargo test -p taskit -- discovery`
   Expected: FAIL (module does not exist)

2. Create `src/discovery.rs` with trait and types:

   ```rust
   use anyhow::Result;
   use std::path::PathBuf;

   use crate::config::{CrateEntry, PropagationEntry};

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
       fn workspace_members(&self) -> Result<Vec<DiscoveredCrate>>;
       fn intra_workspace_deps(&self) -> Result<Vec<(String, String)>>;
   }
   ```

3. Add `pub mod discovery;` to `src/lib.rs` (insert after `pub mod dev_setup;`
   at line 11).

4. Verify:

   ```
   cargo test -p taskit -- discovery       -> 1 test passes
   cargo clippy -p taskit -- -D warnings   -> zero warnings
   ```

5. Commit: `feat(taskit): add MetadataSource trait and discovery types`

---

### Task 3: Implement `CargoMetadataSource`

**Crate**: `taskit`
**File(s)**: `src/discovery.rs`
**Run**: `cargo test -p taskit -- cargo_metadata_source`

1. Write failing test:

   ```rust
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
   ```

   Run: `cargo test -p taskit -- cargo_metadata_source`
   Expected: FAIL (struct does not exist)

2. Implement `CargoMetadataSource`:

   ```rust
   use cargo_metadata::MetadataCommand;

   /// Production adapter: reads cargo metadata from the real workspace.
   pub struct CargoMetadataSource {
       pub workspace_root: PathBuf,
   }

   impl MetadataSource for CargoMetadataSource {
       fn workspace_members(&self) -> Result<Vec<DiscoveredCrate>> {
           let metadata = MetadataCommand::new()
               .current_dir(&self.workspace_root)
               .no_deps()
               .exec()
               .context("failed to run `cargo metadata`")?;

           let ws_root = metadata.workspace_root.as_std_path();
           let mut crates = Vec::new();
           for pkg_id in &metadata.workspace_members {
               let pkg = metadata
                   .packages
                   .iter()
                   .find(|p| &p.id == pkg_id)
                   .context("workspace member not found in packages")?;
               let manifest_dir = pkg
                   .manifest_path
                   .parent()
                   .context("manifest_path has no parent")?;
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

       fn intra_workspace_deps(&self) -> Result<Vec<(String, String)>> {
           let metadata = MetadataCommand::new()
               .current_dir(&self.workspace_root)
               .exec()
               .context("failed to run `cargo metadata`")?;

           let member_names: std::collections::HashSet<String> = metadata
               .workspace_members
               .iter()
               .filter_map(|id| {
                   metadata.packages.iter().find(|p| &p.id == id)
               })
               .map(|p| p.name.clone())
               .collect();

           let mut edges = Vec::new();
           for pkg_id in &metadata.workspace_members {
               let pkg = metadata
                   .packages
                   .iter()
                   .find(|p| &p.id == pkg_id)
                   .unwrap();
               for dep in &pkg.dependencies {
                   if member_names.contains(&dep.name) {
                       edges.push((dep.name.clone(), pkg.name.clone()));
                   }
               }
           }
           Ok(edges)
       }
   }
   ```

3. Verify:

   ```
   cargo test -p taskit -- cargo_metadata_source  -> passes
   cargo clippy -p taskit -- -D warnings          -> zero warnings
   ```

4. Commit: `feat(taskit): implement CargoMetadataSource adapter`

---

### Task 4: Implement `derive_propagation()`

**Crate**: `taskit`
**File(s)**: `src/discovery.rs`
**Run**: `cargo test -p taskit -- derive_propagation`

1. Write failing tests:

   ```rust
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
       let known = vec![
           "common".into(),
           "utils".into(),
           "api".into(),
           "cli".into(),
       ];
       let result = derive_propagation(&deps, &known);
       assert_eq!(result.len(), 2);
   }
   ```

   Run: `cargo test -p taskit -- derive_propagation`
   Expected: FAIL (function does not exist)

2. Implement:

   ```rust
   use std::collections::BTreeMap;

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
       let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
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
   ```

3. Verify:

   ```
   cargo test -p taskit -- derive_propagation  -> all pass
   cargo clippy -p taskit -- -D warnings       -> zero warnings
   ```

4. Commit: `feat(taskit): implement derive_propagation from dep graph`

---

### Task 5: Implement `scan_surfaces()`

**Crate**: `taskit`
**File(s)**: `src/discovery.rs`
**Run**: `cargo test -p taskit -- scan_surfaces`

1. Write failing tests:

   ```rust
   #[test]
   fn scan_surfaces_finds_types_rs() {
       let dir = tempfile::tempdir().unwrap();
       let crate_dir = dir.path().join("my-api/src");
       fs::create_dir_all(&crate_dir).unwrap();
       fs::write(crate_dir.join("types.rs"), "pub struct Foo;").unwrap();
       let surfaces = scan_surfaces(dir.path()).unwrap();
       assert_eq!(surfaces.len(), 1);
       assert_eq!(surfaces[0].name, "my-api/types");
       assert_eq!(surfaces[0].path, "my-api/src/types.rs");
   }

   #[test]
   fn scan_surfaces_skips_target_dir() {
       let dir = tempfile::tempdir().unwrap();
       let target = dir.path().join("target/debug/build/my-api/src");
       fs::create_dir_all(&target).unwrap();
       fs::write(target.join("types.rs"), "generated").unwrap();
       let surfaces = scan_surfaces(dir.path()).unwrap();
       assert!(surfaces.is_empty());
   }

   #[test]
   fn scan_surfaces_finds_graphql_schema() {
       let dir = tempfile::tempdir().unwrap();
       let crate_dir = dir.path().join("my-api");
       fs::create_dir_all(&crate_dir).unwrap();
       fs::write(crate_dir.join("schema.graphql"), "type Q{}").unwrap();
       let surfaces = scan_surfaces(dir.path()).unwrap();
       assert_eq!(surfaces.len(), 1);
       assert!(surfaces[0].name.contains("graphql-schema"));
   }

   #[test]
   fn scan_surfaces_finds_proto_files() {
       let dir = tempfile::tempdir().unwrap();
       let proto_dir = dir.path().join("my-svc/proto");
       fs::create_dir_all(&proto_dir).unwrap();
       fs::write(proto_dir.join("service.proto"), "syntax='proto3';")
           .unwrap();
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
       fs::create_dir_all(&hidden).unwrap();
       fs::write(hidden.join("types.rs"), "struct X;").unwrap();
       let surfaces = scan_surfaces(dir.path()).unwrap();
       assert!(surfaces.is_empty());
   }
   ```

   Run: `cargo test -p taskit -- scan_surfaces`
   Expected: FAIL

2. Implement:

   ```rust
   use std::fs;

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
   pub fn scan_surfaces(workspace_root: &Path) -> Result<Vec<DiscoveredSurface>> {
       let mut surfaces = Vec::new();
       walk_for_surfaces(workspace_root, workspace_root, &mut surfaces)?;
       surfaces.sort_by(|a, b| a.path.cmp(&b.path));
       Ok(surfaces)
   }

   fn walk_for_surfaces(
       root: &Path,
       dir: &Path,
       surfaces: &mut Vec<DiscoveredSurface>,
   ) -> Result<()> {
       let entries = match fs::read_dir(dir) {
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
   ```

3. Verify:

   ```
   cargo test -p taskit -- scan_surfaces   -> all pass
   cargo clippy -p taskit -- -D warnings   -> zero warnings
   ```

4. Commit: `feat(taskit): implement convention-based surface scanning`

---

### Task 6: Implement `Config::discover()` and `Config::discover_with()`

**Crate**: `taskit`
**File(s)**: `src/config.rs`
**Run**: `cargo test -p taskit -- config::tests::discover`

1. Write failing tests in `src/config.rs`:

   ```rust
   // Inside existing #[cfg(test)] mod tests { ... }

   use crate::discovery::{
       DiscoveredCrate, DiscoveredSurface, FakeMetadataSource,
       MetadataSource,
   };

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
       let config = Config::discover_with(dir.path(), &source).unwrap();
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
       let config = Config::discover_with(dir.path(), &source).unwrap();
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
       let config = Config::discover_with(dir.path(), &source).unwrap();
       assert!(config.workspace.crates.is_empty());
       assert!(config.workspace.propagation.is_empty());
   }
   ```

   Note: `FakeMetadataSource` must be made `pub(crate)` in `discovery.rs`
   (gated behind `#[cfg(test)]`) so `config.rs` tests can use it.

   Run: `cargo test -p taskit -- config::tests::discover`
   Expected: FAIL

2. Move the existing `FakeSource` test helper in `discovery.rs` to a
   `pub(crate)` position (still `#[cfg(test)]`):

   ```rust
   // At bottom of src/discovery.rs, before the tests module:
   #[cfg(test)]
   pub(crate) struct FakeMetadataSource {
       pub members: Vec<DiscoveredCrate>,
       pub deps: Vec<(String, String)>,
   }

   #[cfg(test)]
   impl MetadataSource for FakeMetadataSource {
       fn workspace_members(&self) -> Result<Vec<DiscoveredCrate>> {
           Ok(self.members.clone())
       }
       fn intra_workspace_deps(&self) -> Result<Vec<(String, String)>> {
           Ok(self.deps.clone())
       }
   }
   ```

3. Implement in `src/config.rs`:

   ```rust
   use crate::discovery::{
       self, CargoMetadataSource, DiscoveredSurface, MetadataSource,
   };

   impl Config {
       /// Build a Config entirely from cargo metadata + conventions.
       pub fn discover(workspace_root: &Path) -> Result<Config> {
           let source = CargoMetadataSource {
               workspace_root: workspace_root.to_path_buf(),
           };
           Self::discover_with(workspace_root, &source)
       }

       /// Build a Config from a given metadata source + conventions.
       /// Primarily exists for testability.
       pub fn discover_with(
           workspace_root: &Path,
           source: &dyn MetadataSource,
       ) -> Result<Config> {
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

           let known_names: Vec<String> =
               members.iter().map(|m| m.pkg.clone()).collect();
           let propagation = discovery::derive_propagation(&deps, &known_names);

           let surfaces: Vec<DiscoveredSurface> =
               discovery::scan_surfaces(workspace_root)?;
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
           })
       }
   }
   ```

4. Verify:

   ```
   cargo test -p taskit -- config::tests::discover  -> all pass
   cargo clippy -p taskit -- -D warnings             -> zero warnings
   ```

5. Commit: `feat(taskit): implement Config::discover and discover_with`

---

### Task 7: Integrate discovery into `config::load()` merge logic

**Crate**: `taskit`
**File(s)**: `src/config.rs`
**Run**: `cargo test -p taskit -- config::tests`

1. Write failing tests:

   ```rust
   #[test]
   fn load_merge_explicit_crates_wins_over_discovery() {
       let dir = tempfile::tempdir().unwrap();
       let toml = r#"
   [[workspace.crates]]
   dir = "explicit-crate"
   "#;
       fs::write(dir.path().join("taskit.toml"), toml).unwrap();
       // Create a Cargo.toml so cargo metadata would find something
       fs::write(
           dir.path().join("Cargo.toml"),
           "[workspace]\nmembers = []\n\n[package]\nname = \"dummy\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
       )
       .unwrap();
       let config_path = dir.path().join("taskit.toml");
       let config = parse_config(&config_path).unwrap();
       // Explicit crates present -> discovery should be skipped
       assert_eq!(config.workspace.crates.len(), 1);
       assert_eq!(config.workspace.crates[0].dir, "explicit-crate");
   }

   #[test]
   fn merge_fills_empty_crates_from_discovery() {
       let source = FakeMetadataSource {
           members: vec![DiscoveredCrate {
               dir: "discovered".into(),
               pkg: "discovered".into(),
               manifest_path: PathBuf::from("/ws/discovered/Cargo.toml"),
           }],
           deps: vec![],
       };
       let dir = tempfile::tempdir().unwrap();
       // Empty taskit.toml -- no crates section
       fs::write(dir.path().join("taskit.toml"), "").unwrap();
       let mut config = parse_config(&dir.path().join("taskit.toml")).unwrap();
       let discovered = Config::discover_with(dir.path(), &source).unwrap();
       merge_discovered(&mut config, discovered);
       assert_eq!(config.workspace.crates.len(), 1);
       assert_eq!(config.workspace.crates[0].dir, "discovered");
   }
   ```

   Run: `cargo test -p taskit -- config::tests`
   Expected: FAIL (`merge_discovered` does not exist)

2. Implement `merge_discovered` in `src/config.rs`:

   ```rust
   /// Merge discovered config into an explicit config.
   ///
   /// Per-section rule: if the explicit config has any entries in a
   /// section, the entire section comes from config. If empty, the
   /// section is filled from discovery.
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
       // ci and coverage are never discovered -- config only
   }
   ```

3. Update `load()` (lines 129-160) to call discovery when sections are empty:

   ```rust
   pub fn load() -> Result<Workspace> {
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
                       format!(
                           "failed to resolve workspace.root = {}",
                           resolved.display()
                       )
                   })?
               }
               None => root,
           };
           // Fill empty sections from discovery
           if let Ok(discovered) = Config::discover(&root) {
               merge_discovered(&mut config, discovered);
           }
           return Ok(Workspace { root, config });
       }

       // No taskit.toml found -- full discovery
       let root = cargo_workspace_root().context(
           "no taskit.toml found and `cargo metadata` failed; \
            run taskit from inside a Cargo workspace",
       )?;
       let config = Config::discover(&root).unwrap_or_default();
       Ok(Workspace { root, config })
   }
   ```

4. Verify:

   ```
   cargo test -p taskit -- config::tests   -> all pass
   cargo clippy -p taskit -- -D warnings   -> zero warnings
   ```

5. Commit: `feat(taskit): integrate discovery merge into config::load`

---

### Task 8: Replace raw `cargo metadata` call with `CargoMetadataSource`

**Crate**: `taskit`
**File(s)**: `src/config.rs`
**Run**: `cargo test -p taskit -- config::tests::cargo_workspace`

1. Delete the `cargo_workspace_root()` function (lines 184-203) and replace
   its call site in `load()` with `CargoMetadataSource`:

   ```rust
   // In load(), the fallback branch becomes:
   let source = CargoMetadataSource {
       workspace_root: cwd.clone(),
   };
   let members = source
       .workspace_members()
       .context(
           "no taskit.toml found and `cargo metadata` failed; \
            run taskit from inside a Cargo workspace",
       )?;
   // The workspace root is the manifest dir's parent for the first member,
   // or we can get it from cargo_metadata directly:
   let metadata = cargo_metadata::MetadataCommand::new()
       .current_dir(&cwd)
       .no_deps()
       .exec()
       .context("no taskit.toml found and `cargo metadata` failed")?;
   let root = PathBuf::from(metadata.workspace_root.as_std_path());
   let config = Config::discover(&root).unwrap_or_default();
   Ok(Workspace { root, config })
   ```

2. Remove the now-unused `use std::process::Command` import and the
   `cargo_workspace_root` test.

3. Verify:

   ```
   cargo test -p taskit                    -> all pass
   cargo clippy -p taskit -- -D warnings   -> zero warnings
   ```

4. Commit: `refactor(taskit): replace raw cargo metadata with CargoMetadataSource`

---

### Task 9: Integration test -- zero-config in taskit's own workspace

**Crate**: `taskit`
**File(s)**: `src/discovery.rs`
**Run**: `cargo test -p taskit -- integration_discover_taskit`

1. Write test:

   ```rust
   #[test]
   fn integration_discover_taskit_workspace() {
       let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
       let config = crate::config::Config::discover(&root).unwrap();
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
   ```

2. Verify:

   ```
   cargo test -p taskit -- integration_discover  -> passes
   ```

3. Commit: `test(taskit): add integration test for zero-config discovery`

---

### Task 10: Version bump to 0.2.0

**Crate**: `taskit`
**File(s)**: `Cargo.toml`

1. Update version in `Cargo.toml` line 3:

   ```toml
   version = "0.2.0"
   ```

2. Verify:

   ```
   cargo check                             -> compiles
   cargo test -p taskit                     -> all pass
   cargo clippy -p taskit -- -D warnings    -> zero warnings
   ```

3. Commit: `chore(release): taskit v0.2.0`
