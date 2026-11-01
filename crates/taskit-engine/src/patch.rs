//! Applies semantic version bumps across workspace Cargo manifests.

use std::path::{Path, PathBuf};

use taskit_types::error::TaskitError;
use toml_edit::{DocumentMut, Item, Value};

use crate::ctx::Ctx;

/// A version bump kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpKind {
    /// Increment patch version (`x.y.z -> x.y.(z+1)`).
    Patch,
    /// Increment minor version and reset patch.
    Minor,
    /// Increment major version and reset minor/patch.
    Major,
}

impl std::fmt::Display for BumpKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BumpKind::Patch => write!(f, "patch"),
            BumpKind::Minor => write!(f, "minor"),
            BumpKind::Major => write!(f, "major"),
        }
    }
}

/// Parse a semver string `"X.Y.Z"` into `(major, minor, patch)`.
pub(crate) fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let parts: Vec<&str> = v.trim().splitn(3, '.').collect();
    if parts.len() != 3 {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ))
}

pub(crate) fn bump(major: u64, minor: u64, patch: u64, kind: BumpKind) -> (u64, u64, u64) {
    match kind {
        BumpKind::Patch => (major, minor, patch + 1),
        BumpKind::Minor => (major, minor + 1, 0),
        BumpKind::Major => (major + 1, 0, 0),
    }
}

/// Collect all Cargo.toml paths to rewrite: root + every workspace member dir.
fn cargo_toml_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = vec![root.join("Cargo.toml")];
    let crates_dir = root.join("crates");
    if let Ok(entries) = std::fs::read_dir(&crates_dir) {
        for entry in entries.flatten() {
            let candidate = entry.path().join("Cargo.toml");
            if candidate.exists() {
                paths.push(candidate);
            }
        }
    }
    paths
}

/// Rewrite version fields in a Cargo manifest, touching only first-party crates.
///
/// Edits are surgical:
/// - `[package].version` is always set to `next`;
/// - `[workspace.dependencies]` pins are rewritten only for names in `own_crates`;
/// - every third-party pin, comment, key order, and formatting choice is preserved.
///
/// Matching by name rather than by version string is the point: a manifest can
/// contain `syn = { version = "0.8.0" }` alongside our own `0.8.0` crates, and a
/// blanket `version = "0.8.0"` replace would silently repin the dependency to a
/// release that does not exist.
pub(crate) fn bump_manifest(
    content: &str,
    next: &str,
    own_crates: &[String],
) -> Result<String, TaskitError> {
    let mut doc = content
        .parse::<DocumentMut>()
        .map_err(|e| TaskitError::other(format!("parse Cargo.toml: {e}")))?;

    if doc.get("package").is_some() {
        doc["package"]["version"] = Item::Value(next.into());
    }

    if let Some(deps) = doc
        .get_mut("workspace")
        .and_then(|w| w.get_mut("dependencies"))
        .and_then(|d| d.as_table_like_mut())
    {
        for name in own_crates {
            let Some(item) = deps.get_mut(name.as_str()) else {
                continue;
            };
            if let Item::Value(Value::InlineTable(table)) = item {
                table.insert("version", Value::from(next));
            }
        }
    }

    Ok(doc.to_string())
}

/// Run workspace version bump for the selected kind.
pub fn run(ctx: &Ctx, kind: BumpKind) -> Result<(), TaskitError> {
    let root = &ctx.root;

    // Detect current version from the root Cargo.toml.
    let root_toml = std::fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|e| TaskitError::other(format!("cannot read root Cargo.toml: {e}")))?;

    let current = root_toml
        .lines()
        .find_map(|line| {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("version = \"") {
                rest.strip_suffix('"')
            } else {
                None
            }
        })
        .ok_or_else(|| TaskitError::other("no `version = \"...\"` found in root Cargo.toml"))?
        .to_string();

    let (maj, min, pat) = parse_semver(&current)
        .ok_or_else(|| TaskitError::other(format!("cannot parse version {current:?}")))?;
    let (nmaj, nmin, npat) = bump(maj, min, pat, kind);
    let next = format!("{nmaj}.{nmin}.{npat}");

    taskit_output::taskit_progress!("Bumping {kind} version: {current} → {next}");

    let own_crates = ctx.workspace_member_names();
    let paths = cargo_toml_paths(root);
    for path in &paths {
        let content = std::fs::read_to_string(path)
            .map_err(|e| TaskitError::other(format!("read {}: {e}", path.display())))?;
        let updated = bump_manifest(&content, &next, &own_crates)?;
        if updated == content {
            continue;
        }
        if ctx.dry_run {
            taskit_output::taskit_dry!("update version in {}", path.display());
        } else {
            std::fs::write(path, &updated)
                .map_err(|e| TaskitError::other(format!("write {}: {e}", path.display())))?;
            taskit_output::taskit_ok!("updated {}", path.display());
        }
    }

    taskit_output::taskit_ok!("Version bumped to {next}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_semver ---

    #[test]
    fn parse_semver_standard() {
        assert_eq!(parse_semver("1.2.3"), Some((1, 2, 3)));
    }

    #[test]
    fn parse_semver_zeros() {
        assert_eq!(parse_semver("0.0.0"), Some((0, 0, 0)));
    }

    #[test]
    fn parse_semver_large() {
        assert_eq!(parse_semver("10.20.300"), Some((10, 20, 300)));
    }

    #[test]
    fn parse_semver_invalid_returns_none() {
        assert!(parse_semver("1.2").is_none());
        assert!(parse_semver("notaversion").is_none());
        assert!(parse_semver("1.2.x").is_none());
        assert!(parse_semver("").is_none());
    }

    // --- bump ---

    #[test]
    fn bump_patch_increments_patch() {
        assert_eq!(bump(0, 7, 0, BumpKind::Patch), (0, 7, 1));
    }

    #[test]
    fn bump_minor_increments_minor_resets_patch() {
        assert_eq!(bump(0, 7, 3, BumpKind::Minor), (0, 8, 0));
    }

    #[test]
    fn bump_major_increments_major_resets_minor_and_patch() {
        assert_eq!(bump(0, 7, 3, BumpKind::Major), (1, 0, 0));
    }

    // --- bump_manifest ---

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn bump_manifest_updates_package_version() {
        let content = "[package]\nname = \"x\"\nversion = \"0.7.0\"\n";
        let out = bump_manifest(content, "0.7.1", &[]).expect("bump");
        assert!(out.contains("version = \"0.7.1\""), "{out}");
    }

    #[test]
    fn bump_manifest_leaves_third_party_pin_at_same_version_untouched() {
        // Regression: a blanket string replace would repin `syn` to 0.7.1.
        let content = "[dependencies]\nsyn = { version = \"0.7.0\", features = [\"full\"] }\n";
        let out = bump_manifest(content, "0.7.1", &[]).expect("bump");
        assert!(out.contains("syn = { version = \"0.7.0\""), "{out}");
        assert!(!out.contains("0.7.1"), "{out}");
    }

    #[test]
    fn bump_manifest_rewrites_own_workspace_pin() {
        let content = concat!(
            "[workspace.dependencies]\n",
            "taskit-core = { version = \"0.7.0\", path = \"crates/taskit-core\" }\n",
            "syn = { version = \"0.7.0\" }\n",
        );
        let out = bump_manifest(content, "0.7.1", &names(&["taskit-core"])).expect("bump");
        assert!(out.contains("taskit-core = { version = \"0.7.1\""), "{out}");
        assert!(out.contains("syn = { version = \"0.7.0\" }"), "{out}");
    }

    #[test]
    fn bump_manifest_preserves_comments_and_ordering() {
        let content = concat!(
            "# leading comment\n",
            "[package]\n",
            "name = \"x\"\n",
            "# version comment\n",
            "version = \"0.7.0\"\n",
        );
        let out = bump_manifest(content, "0.7.1", &[]).expect("bump");
        assert!(out.contains("# leading comment"), "{out}");
        assert!(out.contains("# version comment"), "{out}");
        assert!(out.contains("name = \"x\""), "{out}");
        assert!(out.contains("version = \"0.7.1\""), "{out}");
    }

    #[test]
    fn bump_manifest_no_op_without_package_section() {
        let content = "[workspace]\nmembers = [\"crates/*\"]\n";
        let out = bump_manifest(content, "0.7.1", &[]).expect("bump");
        assert!(!out.contains("0.7.1"), "{out}");
    }

    #[test]
    fn bump_manifest_errors_on_malformed_toml() {
        assert!(bump_manifest("this is not = = toml", "0.7.1", &[]).is_err());
    }

    // --- run (isolated temp workspace) ---

    fn make_temp_workspace(version: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let toml = format!(
            "[workspace]\nmembers = []\n\n[package]\nname = \"fake\"\nversion = \"{version}\"\n"
        );
        std::fs::write(dir.path().join("Cargo.toml"), toml).unwrap();
        dir
    }

    fn ctx_for_dir(dir: &tempfile::TempDir) -> Ctx {
        use taskit_types::config::Config;
        use taskit_types::output_format::OutputFormat;
        use xshell::Shell;
        Ctx::new(
            Shell::new().unwrap(),
            dir.path().to_path_buf(),
            Config::default(),
            false,
            OutputFormat::Human,
        )
    }

    #[test]
    fn patch_run_bumps_patch_version() {
        let dir = make_temp_workspace("0.7.0");
        let ctx = ctx_for_dir(&dir);
        run(&ctx, BumpKind::Patch).expect("patch run");
        let content = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
        assert!(
            content.contains("version = \"0.7.1\""),
            "expected 0.7.1: {content}"
        );
    }

    #[test]
    fn minor_run_bumps_minor_version() {
        let dir = make_temp_workspace("0.7.0");
        let ctx = ctx_for_dir(&dir);
        run(&ctx, BumpKind::Minor).expect("minor run");
        let content = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
        assert!(
            content.contains("version = \"0.8.0\""),
            "expected 0.8.0: {content}"
        );
    }

    #[test]
    fn major_run_bumps_major_version() {
        let dir = make_temp_workspace("0.7.0");
        let ctx = ctx_for_dir(&dir);
        run(&ctx, BumpKind::Major).expect("major run");
        let content = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
        assert!(
            content.contains("version = \"1.0.0\""),
            "expected 1.0.0: {content}"
        );
    }
}
