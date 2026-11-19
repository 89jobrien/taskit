//! Compiles the Rust examples in each workspace crate's `CLAUDE.md`.
//!
//! Every fenced `rust` block (not tagged `ignore`) becomes a `#[test]` in a
//! generated crate under `target/taskit-doc-examples/`. That crate depends on
//! each owning crate, and on the dependencies that crate declares, under the
//! same keys. An example therefore resolves names the way code inside the
//! crate does. Fragments that are not complete programs should use the
//! rustdoc tag ```` ```rust,ignore ````.

use std::path::{Path, PathBuf};

use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;
use crate::discovery::{CargoMetadataSource, DiscoveredCrate, MetadataSource};

const GENERATED_DIR: &str = "target/taskit-doc-examples";
const DOC_FILE: &str = "CLAUDE.md";
/// Crate names that would shadow the standard library inside the generated crate.
const RESERVED: &[&str] = &["core", "std", "alloc", "proc_macro", "test"];

/// One compilable fence from a `CLAUDE.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Example {
    /// 1-based line of the opening fence in the source doc.
    pub line: usize,
    /// Code between the fences.
    pub code: String,
}

/// The examples from one member crate's `CLAUDE.md`.
struct CrateExamples {
    pkg: String,
    manifest_path: PathBuf,
    examples: Vec<Example>,
}

/// An open fence while scanning a markdown document.
struct Fence {
    line: usize,
    compile: bool,
    code: String,
}

/// Return every `rust` fence not tagged `ignore`.
pub fn extract_examples(markdown: &str) -> Vec<Example> {
    let mut out = Vec::new();
    let mut open: Option<Fence> = None;
    for (idx, line) in markdown.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some(mut fence) = open.take() {
            if trimmed.starts_with("```") {
                if fence.compile {
                    out.push(Example {
                        line: fence.line,
                        code: fence.code,
                    });
                }
            } else {
                fence.code.push_str(line);
                fence.code.push('\n');
                open = Some(fence);
            }
        } else if let Some(info) = trimmed.strip_prefix("```") {
            let tags: Vec<&str> = info
                .split([',', ' '])
                .filter(|tag| !tag.is_empty())
                .collect();
            open = Some(Fence {
                line: idx + 1,
                compile: tags.contains(&"rust") && !tags.contains(&"ignore"),
                code: String::new(),
            });
        }
    }
    out
}

/// Run every compilable `CLAUDE.md` example in the workspace.
pub fn run(ctx: &Ctx) -> Result<(), TaskitError> {
    taskit_output::taskit_progress!("Compiling CLAUDE.md Rust examples...");

    let members = CargoMetadataSource {
        workspace_root: ctx.root.clone(),
    }
    .workspace_members()?;

    let mut crates = Vec::new();
    for member in &members {
        let doc = manifest_dir(member).join(DOC_FILE);
        let Ok(markdown) = std::fs::read_to_string(&doc) else {
            continue;
        };
        let examples = extract_examples(&markdown);
        if examples.is_empty() {
            continue;
        }
        if RESERVED.contains(&module_name(&member.pkg).as_str()) {
            taskit_output::taskit_warn!(
                "doc-examples: `{}` has a reserved crate name; its examples are not compiled",
                member.pkg
            );
            continue;
        }
        crates.push(CrateExamples {
            pkg: member.pkg.clone(),
            manifest_path: member.manifest_path.clone(),
            examples,
        });
    }

    let total: usize = crates.iter().map(|c| c.examples.len()).sum();
    if total == 0 {
        taskit_output::taskit_skip!("doc-examples: no compilable rust examples in CLAUDE.md files");
        return Ok(());
    }

    let root_manifest = read_manifest(&ctx.root.join("Cargo.toml"))?;
    let workspace_deps = root_manifest
        .get("workspace")
        .and_then(toml::Value::as_table)
        .and_then(|ws| ws.get("dependencies"))
        .and_then(toml::Value::as_table)
        .cloned()
        .unwrap_or_default();

    let mut deps = toml::Table::new();
    for krate in &crates {
        collect_deps(krate, &ctx.root, &workspace_deps, &mut deps)?;
    }

    let gen_dir = ctx.root.join(GENERATED_DIR);
    if !ctx.dry_run {
        write_generated(&gen_dir, &deps, &crates)?;
    }
    taskit_output::taskit_progress!(
        "Compiling {total} example(s) from {} crate(s)",
        crates.len()
    );
    let manifest = gen_dir.join("Cargo.toml").display().to_string();
    ctx.run(cmd!(
        ctx.sh,
        "cargo test --manifest-path {manifest} --quiet"
    ))
}

/// Module name for a package inside the generated crate.
fn module_name(pkg: &str) -> String {
    pkg.replace('-', "_")
}

fn manifest_dir(member: &DiscoveredCrate) -> &Path {
    member.manifest_path.parent().unwrap_or(Path::new("."))
}

fn read_manifest(path: &Path) -> Result<toml::Table, TaskitError> {
    let text = std::fs::read_to_string(path).map_err(TaskitError::from)?;
    toml::from_str(&text)
        .map_err(|e| TaskitError::other(format!("failed to parse {}: {e}", path.display())))
}

/// Add the owning crate and its declared dependencies to `deps`. The first
/// declaration of a key wins, so a later crate cannot change an earlier one.
fn collect_deps(
    krate: &CrateExamples,
    root: &Path,
    workspace_deps: &toml::Table,
    deps: &mut toml::Table,
) -> Result<(), TaskitError> {
    let crate_dir = krate.manifest_path.parent().unwrap_or(Path::new("."));
    let self_key = module_name(&krate.pkg);
    if !deps.contains_key(&self_key) {
        let mut path_dep = toml::Table::new();
        path_dep.insert(
            "path".into(),
            toml::Value::String(crate_dir.display().to_string()),
        );
        deps.insert(self_key, toml::Value::Table(path_dep));
    }

    let manifest = read_manifest(&krate.manifest_path)?;
    for section in ["dependencies", "dev-dependencies"] {
        let Some(table) = manifest.get(section).and_then(toml::Value::as_table) else {
            continue;
        };
        for (key, spec) in table {
            if !deps.contains_key(key) {
                let resolved = resolve_dep(key, spec, workspace_deps, root, crate_dir)?;
                deps.insert(key.clone(), resolved);
            }
        }
    }
    Ok(())
}

/// Turn one `[dependencies]` entry into a dependency the generated crate can
/// resolve: `workspace = true` is replaced by the workspace entry (with the
/// crate's features added), and relative `path`s are made absolute.
fn resolve_dep(
    key: &str,
    spec: &toml::Value,
    workspace: &toml::Table,
    root: &Path,
    crate_dir: &Path,
) -> Result<toml::Value, TaskitError> {
    let Some(local) = spec.as_table() else {
        return Ok(spec.clone());
    };

    let inherits = local.get("workspace").and_then(toml::Value::as_bool) == Some(true);
    let (mut dep, base_dir) = if inherits {
        let entry = workspace.get(key).ok_or_else(|| {
            TaskitError::other(format!(
                "dependency `{key}` inherits from [workspace.dependencies], which has no entry"
            ))
        })?;
        let table = match entry {
            toml::Value::String(version) => {
                let mut table = toml::Table::new();
                table.insert("version".into(), toml::Value::String(version.clone()));
                table
            }
            toml::Value::Table(table) => table.clone(),
            _ => {
                return Err(TaskitError::other(format!(
                    "workspace dependency `{key}` has an unsupported shape"
                )));
            }
        };
        (table, root)
    } else {
        (local.clone(), crate_dir)
    };

    // Features are additive across declarations, so the crate's features are
    // merged into the workspace's rather than replacing them.
    if let Some(features) = local.get("features").and_then(toml::Value::as_array) {
        let mut merged = dep
            .get("features")
            .and_then(toml::Value::as_array)
            .cloned()
            .unwrap_or_default();
        for feature in features {
            if !merged.contains(feature) {
                merged.push(feature.clone());
            }
        }
        dep.insert("features".into(), toml::Value::Array(merged));
    }

    dep.remove("workspace");
    // `optional` only means something inside a feature list; here it would
    // turn the dependency off.
    dep.remove("optional");
    if let Some(path) = dep.get("path").and_then(toml::Value::as_str) {
        let absolute = base_dir.join(path).display().to_string();
        dep.insert("path".into(), toml::Value::String(absolute));
    }
    Ok(toml::Value::Table(dep))
}

/// Cargo manifest for the generated crate. It declares its own empty
/// `[workspace]` so it is not treated as a member of the enclosing workspace.
fn render_manifest(deps: &toml::Table) -> Result<String, TaskitError> {
    let mut package = toml::Table::new();
    package.insert("name".into(), "taskit-doc-examples".into());
    package.insert("version".into(), "0.0.0".into());
    package.insert("edition".into(), "2021".into());
    package.insert("publish".into(), toml::Value::Boolean(false));

    let mut manifest = toml::Table::new();
    manifest.insert("package".into(), toml::Value::Table(package));
    manifest.insert("workspace".into(), toml::Value::Table(toml::Table::new()));
    manifest.insert("dependencies".into(), toml::Value::Table(deps.clone()));
    toml::to_string(&manifest)
        .map_err(|e| TaskitError::other(format!("failed to render generated Cargo.toml: {e}")))
}

/// Source for the generated crate: one module per owning crate, one `#[test]`
/// per example.
fn render_lib(crates: &[CrateExamples]) -> String {
    let mut out = String::from(
        "//! Generated by `taskit check doc-examples`. Regenerated on every run.\n\
         #![allow(unused, clippy::all)]\n",
    );
    for krate in crates {
        out.push_str(&format!("\npub mod doc_{} {{\n", module_name(&krate.pkg)));
        for example in &krate.examples {
            out.push_str(&format!(
                "    #[test]\n    fn line_{}() -> Result<(), Box<dyn std::error::Error>> {{\n{}    Ok(())\n    }}\n",
                example.line, example.code
            ));
        }
        out.push_str("}\n");
    }
    out
}

fn write_generated(
    gen_dir: &Path,
    deps: &toml::Table,
    crates: &[CrateExamples],
) -> Result<(), TaskitError> {
    std::fs::create_dir_all(gen_dir.join("src")).map_err(TaskitError::from)?;
    std::fs::write(gen_dir.join("Cargo.toml"), render_manifest(deps)?)
        .map_err(TaskitError::from)?;
    std::fs::write(gen_dir.join("src/lib.rs"), render_lib(crates)).map_err(TaskitError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines_of(examples: &[Example]) -> Vec<usize> {
        examples.iter().map(|e| e.line).collect()
    }

    #[test]
    fn extract_finds_rust_fences_with_line_numbers() {
        let doc = "intro\n```rust\nlet x = 1;\n```\ntext\n```rust\nlet y = 2;\n```\n";
        let examples = extract_examples(doc);
        assert_eq!(lines_of(&examples), vec![2, 6]);
        assert_eq!(examples[0].code, "let x = 1;\n");
        assert_eq!(examples[1].code, "let y = 2;\n");
    }

    #[test]
    fn extract_skips_ignored_and_other_languages() {
        let doc = "```rust,ignore\nfragment\n```\n```bash\ncargo test\n```\n```text\nplain\n```\n";
        assert!(extract_examples(doc).is_empty());
    }

    #[test]
    fn extract_accepts_comma_separated_tags() {
        let doc = "```rust,no_run\nlet x = 1;\n```\n";
        assert_eq!(lines_of(&extract_examples(doc)), vec![1]);
    }

    #[test]
    fn extract_drops_unclosed_fence_at_end_of_file() {
        let doc = "```rust\nlet x = 1;\n";
        assert!(extract_examples(doc).is_empty());
    }

    #[test]
    fn render_lib_wraps_each_example_in_a_test_fn() {
        let krate = CrateExamples {
            pkg: "my-lib".into(),
            manifest_path: PathBuf::from("/ws/crates/my-lib/Cargo.toml"),
            examples: vec![Example {
                line: 3,
                code: "    let x = 1;\n".into(),
            }],
        };
        let lib = render_lib(&[krate]);
        assert!(lib.contains("pub mod doc_my_lib {"));
        assert!(lib.contains("#[test]\n    fn line_3()"));
        assert!(lib.contains("let x = 1;"));
    }

    #[test]
    fn resolve_dep_keeps_plain_version() {
        let spec = toml::Value::String("1.0".into());
        let resolved = resolve_dep(
            "serde",
            &spec,
            &toml::Table::new(),
            Path::new("/ws"),
            Path::new("/ws/crates/a"),
        )
        .unwrap();
        assert_eq!(resolved, spec);
    }

    #[test]
    fn resolve_dep_makes_relative_path_absolute_to_crate() {
        let spec: toml::Value = toml::from_str("path = \"../core\"\npackage = \"core\"").unwrap();
        let resolved = resolve_dep(
            "myrv-core",
            &spec,
            &toml::Table::new(),
            Path::new("/ws"),
            Path::new("/ws/crates/rvc"),
        )
        .unwrap();
        let table = resolved.as_table().unwrap();
        assert_eq!(table["path"].as_str(), Some("/ws/crates/rvc/../core"));
        assert_eq!(table["package"].as_str(), Some("core"));
    }

    #[test]
    fn resolve_dep_inherits_workspace_entry_and_merges_features() {
        let workspace: toml::Table =
            toml::from_str("[serde]\nversion = \"1\"\nfeatures = [\"derive\"]").unwrap();
        let spec: toml::Value =
            toml::from_str("workspace = true\nfeatures = [\"rc\", \"derive\"]").unwrap();
        let resolved = resolve_dep(
            "serde",
            &spec,
            &workspace,
            Path::new("/ws"),
            Path::new("/ws/crates/a"),
        )
        .unwrap();
        let table = resolved.as_table().unwrap();
        assert_eq!(table["version"].as_str(), Some("1"));
        let features: Vec<&str> = table["features"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(toml::Value::as_str)
            .collect();
        assert_eq!(features, vec!["derive", "rc"]);
        assert!(!table.contains_key("workspace"));
    }

    #[test]
    fn resolve_dep_inherited_path_is_relative_to_workspace_root() {
        let workspace: toml::Table = toml::from_str("[shared]\npath = \"crates/shared\"").unwrap();
        let spec: toml::Value = toml::from_str("workspace = true").unwrap();
        let resolved = resolve_dep(
            "shared",
            &spec,
            &workspace,
            Path::new("/ws"),
            Path::new("/ws/crates/a"),
        )
        .unwrap();
        assert_eq!(
            resolved.as_table().unwrap()["path"].as_str(),
            Some("/ws/crates/shared")
        );
    }

    #[test]
    fn resolve_dep_drops_optional_flag() {
        let spec: toml::Value = toml::from_str("version = \"1\"\noptional = true").unwrap();
        let resolved = resolve_dep(
            "x",
            &spec,
            &toml::Table::new(),
            Path::new("/ws"),
            Path::new("/ws/a"),
        )
        .unwrap();
        assert!(!resolved.as_table().unwrap().contains_key("optional"));
    }

    #[test]
    fn render_manifest_declares_empty_workspace() {
        let text = render_manifest(&toml::Table::new()).unwrap();
        assert!(text.contains("[workspace]"));
    }
}
