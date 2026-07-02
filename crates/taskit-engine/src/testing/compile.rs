use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

const CACHE_DIR: &str = ".taskit-cache";
const CACHE_FILE: &str = ".taskit-cache/compile-cache.json";

// ── cache schema ─────────────────────────────────────────────────────────────

/// Persisted compile cache. Mirrors the three-level hash tree so each level
/// can be checked independently: repo → crate → module.
#[derive(Serialize, Deserialize, Default)]
struct CompileCache {
    /// SHA-256 of `Cargo.lock`. Change busts all crate entries.
    cargo_lock_hash: String,
    /// Per-crate entry keyed by package name.
    crates: BTreeMap<String, CrateEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct CrateEntry {
    /// SHA-256 over all module hashes + `Cargo.toml` hash (sorted).
    hash: String,
    /// Module-level hashes: absolute path → SHA-256(contents).
    modules: BTreeMap<String, String>,
}

// ── public entry point ───────────────────────────────────────────────────────

/// Compile all test binaries, skipping crates whose sources are unchanged.
/// Recompiles only the crates where the module-level hash tree drifted.
pub fn run(ctx: &Ctx) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let current = snapshot(Path::new("."))?;
    let cached = load_cache();
    let changed = stale_crates(&current, &cached);

    if changed.is_empty() {
        taskit_output::taskit_skip!(
            "compile-tests: all {} crates up to date",
            current.crates.len()
        );
        return Ok(());
    }

    taskit_output::taskit_progress!(
        "compile-tests: recompiling {}/{} crates: {}",
        changed.len(),
        current.crates.len(),
        changed.join(", ")
    );

    for name in &changed {
        ctx.run(cmd!(sh, "cargo nextest run --locked -p {name} --no-run"))?;
    }

    if !ctx.dry_run {
        // Merge: keep unchanged entries from old cache, replace changed ones.
        let mut merged = CompileCache {
            cargo_lock_hash: current.cargo_lock_hash.clone(),
            crates: BTreeMap::new(),
        };
        for (name, entry) in &cached.crates {
            if !changed.contains(name) {
                merged.crates.insert(name.clone(), entry.clone());
            }
        }
        for name in &changed {
            if let Some(entry) = current.crates.get(name) {
                merged.crates.insert(name.clone(), entry.clone());
            }
        }
        write_cache(&merged)?;
        crate::cache::update()?;
    }

    Ok(())
}

// ── staleness logic ───────────────────────────────────────────────────────────

/// Return the names of all crates in `current` that are absent from or have
/// drifted in `cached`. A changed `cargo_lock_hash` busts every crate.
fn stale_crates(current: &CompileCache, cached: &CompileCache) -> Vec<String> {
    let lock_changed = current.cargo_lock_hash != cached.cargo_lock_hash;
    current
        .crates
        .keys()
        .filter(|name| {
            lock_changed
                || cached
                    .crates
                    .get(*name)
                    .is_none_or(|c| c.hash != current.crates[*name].hash)
        })
        .cloned()
        .collect()
}

// ── snapshot ─────────────────────────────────────────────────────────────────

fn snapshot(root: &Path) -> Result<CompileCache, TaskitError> {
    let lock_hash = file_hash(&root.join("Cargo.lock")).unwrap_or_default();

    let mut crate_roots: Vec<(String, PathBuf)> = Vec::new();
    collect_crate_roots(root, &mut crate_roots)?;

    let mut crates: BTreeMap<String, CrateEntry> = BTreeMap::new();
    for (name, crate_root) in &crate_roots {
        let entry = crate_entry(crate_root)?;
        crates.insert(name.clone(), entry);
    }

    Ok(CompileCache {
        cargo_lock_hash: lock_hash,
        crates,
    })
}

/// Build a `CrateEntry` for the crate rooted at `dir`.
fn crate_entry(dir: &Path) -> Result<CrateEntry, TaskitError> {
    let mut modules: BTreeMap<String, String> = BTreeMap::new();

    // Include the crate's own Cargo.toml
    let manifest = dir.join("Cargo.toml");
    if let Some(h) = file_hash(&manifest) {
        modules.insert(manifest.to_string_lossy().into_owned(), h);
    }

    // Collect all .rs files under this crate root (stop at nested crate roots)
    collect_rs_files(dir, dir, &mut modules)?;

    // Crate hash = SHA-256 over sorted "path:hash" entries
    let mut hasher = Sha256::new();
    for (path, hash) in &modules {
        hasher.update(path.as_bytes());
        hasher.update(b":");
        hasher.update(hash.as_bytes());
        hasher.update(b"\n");
    }
    let hash = hex::encode(hasher.finalize());

    Ok(CrateEntry { hash, modules })
}

// ── filesystem walking ────────────────────────────────────────────────────────

/// Find all crate roots (directories with `Cargo.toml` containing `[package]`),
/// excluding ignored directories. Does not recurse into a found crate root.
fn collect_crate_roots(dir: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<(), TaskitError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if matches!(
            name.as_ref(),
            "target" | ".git" | "node_modules" | ".taskit-cache" | "fuzz"
        ) {
            continue;
        }
        let manifest = path.join("Cargo.toml");
        if manifest.exists()
            && let Some(pkg_name) = package_name(&manifest)
        {
            out.push((pkg_name, path));
            // Don't recurse further — nested crates are their own roots
            continue;
        }
        collect_crate_roots(&path, out)?;
    }
    Ok(())
}

/// Recursively collect `.rs` files under `dir`, stopping at subdirectories
/// that are themselves crate roots (have their own `Cargo.toml`).
fn collect_rs_files(
    root: &Path,
    dir: &Path,
    out: &mut BTreeMap<String, String>,
) -> Result<(), TaskitError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            // Stop recursing into nested crate roots
            if path.join("Cargo.toml").exists() && path != root {
                continue;
            }
            collect_rs_files(root, &path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs")
            && let Some(h) = file_hash(&path)
        {
            out.insert(path.to_string_lossy().into_owned(), h);
        }
    }
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn file_hash(path: &Path) -> Option<String> {
    let content = std::fs::read(path).ok()?;
    Some(hex::encode(Sha256::digest(&content)))
}

/// Extract the package name from a `Cargo.toml` without a TOML parser.
/// Returns `None` for workspace manifests (no `[package]` section).
fn package_name(cargo_toml: &Path) -> Option<String> {
    let content = std::fs::read_to_string(cargo_toml).ok()?;
    package_name_from_str(&content)
}

fn package_name_from_str(content: &str) -> Option<String> {
    let mut in_package = false;
    for line in content.lines() {
        let t = line.trim();
        if t == "[package]" {
            in_package = true;
        } else if t.starts_with('[') {
            in_package = false;
        } else if in_package
            && let Some(rest) = t.strip_prefix("name")
            && let Some(rest) = rest.trim_start().strip_prefix('=')
        {
            let name = rest.trim().trim_matches('"');
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

// ── cache I/O ─────────────────────────────────────────────────────────────────

fn load_cache() -> CompileCache {
    std::fs::read_to_string(CACHE_FILE)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_cache(cache: &CompileCache) -> Result<(), TaskitError> {
    std::fs::create_dir_all(CACHE_DIR)?;
    let json = serde_json::to_string_pretty(cache).map_err(TaskitError::other)?;
    std::fs::write(CACHE_FILE, json)?;
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    // ── test infrastructure ───────────────────────────────────────────────────

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Isolated temp directory that cleans up on drop.
    struct TempWorkspace {
        pub root: PathBuf,
    }

    impl TempWorkspace {
        fn new() -> Self {
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let root =
                std::env::temp_dir().join(format!("taskit-test-{}-{}", std::process::id(), id));
            std::fs::create_dir_all(&root).expect("create temp workspace");
            Self { root }
        }

        /// Write a file relative to the workspace root, creating parent dirs.
        fn write(&self, rel: &str, content: &str) -> PathBuf {
            let path = self.root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
            path
        }

        /// Write a minimal crate Cargo.toml at `<rel_dir>/Cargo.toml`.
        fn write_crate(&self, dir: &str, name: &str) {
            self.write(
                &format!("{dir}/Cargo.toml"),
                &format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n"),
            );
        }
    }

    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn make_cache(lock_hash: &str, crates: &[(&str, &str)]) -> CompileCache {
        CompileCache {
            cargo_lock_hash: lock_hash.to_string(),
            crates: crates
                .iter()
                .map(|(name, hash)| {
                    (
                        name.to_string(),
                        CrateEntry {
                            hash: hash.to_string(),
                            modules: BTreeMap::new(),
                        },
                    )
                })
                .collect(),
        }
    }

    // ── package_name_from_str ─────────────────────────────────────────────────

    #[test]
    fn package_name_from_str_returns_name_for_crate_manifest() {
        let toml = "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n";
        assert_eq!(package_name_from_str(toml).as_deref(), Some("my-crate"));
    }

    #[test]
    fn package_name_from_str_returns_none_for_workspace_manifest() {
        let toml = "[workspace]\nmembers = [\"crate-a\"]\n";
        assert_eq!(package_name_from_str(toml), None);
    }

    #[test]
    fn package_name_from_str_returns_none_for_empty_input() {
        assert_eq!(package_name_from_str(""), None);
    }

    #[test]
    fn package_name_from_str_ignores_name_outside_package_section() {
        let toml = "[dependencies]\nname = \"not-a-package\"\n\n[package]\nname = \"real-name\"\n";
        assert_eq!(
            package_name_from_str(toml).as_deref(),
            Some("real-name"),
            "name= under [dependencies] must not be mistaken for the package name"
        );
    }

    #[test]
    fn package_name_from_str_returns_none_when_name_not_present() {
        let toml = "[package]\nversion = \"0.1.0\"\n";
        assert_eq!(package_name_from_str(toml), None);
    }

    #[test]
    fn package_name_from_str_stops_at_next_section() {
        // name appears after [lib] which resets in_package
        let toml = "[package]\nversion = \"0.1.0\"\n\n[lib]\nname = \"not-the-pkg\"\n";
        assert_eq!(package_name_from_str(toml), None);
    }

    #[test]
    fn package_name_from_str_handles_whitespace_around_equals() {
        let toml = "[package]\nname   =   \"spaced-name\"\n";
        assert_eq!(package_name_from_str(toml).as_deref(), Some("spaced-name"));
    }

    // ── file_hash ─────────────────────────────────────────────────────────────

    #[test]
    fn file_hash_is_deterministic() {
        let ws = TempWorkspace::new();
        let path = ws.write("file.rs", "fn main() {}");
        assert_eq!(
            file_hash(&path),
            file_hash(&path),
            "same file must produce identical hash on repeated calls"
        );
    }

    #[test]
    fn file_hash_differs_for_different_content() {
        let ws = TempWorkspace::new();
        let a = ws.write("a.rs", "fn foo() {}");
        let b = ws.write("b.rs", "fn bar() {}");
        assert_ne!(
            file_hash(&a),
            file_hash(&b),
            "different content must produce different hashes"
        );
    }

    #[test]
    fn file_hash_same_content_same_hash() {
        let ws = TempWorkspace::new();
        let a = ws.write("a.rs", "identical");
        let b = ws.write("b.rs", "identical");
        assert_eq!(
            file_hash(&a),
            file_hash(&b),
            "identical content must hash identically regardless of path"
        );
    }

    #[test]
    fn file_hash_returns_none_for_missing_file() {
        assert_eq!(file_hash(Path::new("/nonexistent/path/file.rs")), None);
    }

    #[test]
    fn file_hash_changes_when_content_changes() {
        let ws = TempWorkspace::new();
        let path = ws.write("f.rs", "v1");
        let h1 = file_hash(&path).unwrap();
        std::fs::write(&path, "v2").unwrap();
        let h2 = file_hash(&path).unwrap();
        assert_ne!(h1, h2, "hash must change after file content changes");
    }

    // ── stale_crates ──────────────────────────────────────────────────────────

    #[test]
    fn stale_crates_empty_when_all_match() {
        let current = make_cache("lock-abc", &[("crate-a", "hash-1"), ("crate-b", "hash-2")]);
        let cached = make_cache("lock-abc", &[("crate-a", "hash-1"), ("crate-b", "hash-2")]);
        assert!(
            stale_crates(&current, &cached).is_empty(),
            "no crates should be stale when everything matches"
        );
    }

    #[test]
    fn stale_crates_returns_changed_crate() {
        let current = make_cache(
            "lock-abc",
            &[("crate-a", "hash-new"), ("crate-b", "hash-2")],
        );
        let cached = make_cache(
            "lock-abc",
            &[("crate-a", "hash-old"), ("crate-b", "hash-2")],
        );
        let stale = stale_crates(&current, &cached);
        assert_eq!(stale, vec!["crate-a"]);
    }

    #[test]
    fn stale_crates_returns_new_crate_absent_from_cache() {
        let current = make_cache(
            "lock-abc",
            &[("crate-a", "hash-1"), ("new-crate", "hash-x")],
        );
        let cached = make_cache("lock-abc", &[("crate-a", "hash-1")]);
        let stale = stale_crates(&current, &cached);
        assert_eq!(stale, vec!["new-crate"]);
    }

    #[test]
    fn stale_crates_all_stale_when_lock_changes() {
        let current = make_cache("lock-NEW", &[("crate-a", "hash-1"), ("crate-b", "hash-2")]);
        let cached = make_cache("lock-OLD", &[("crate-a", "hash-1"), ("crate-b", "hash-2")]);
        let mut stale = stale_crates(&current, &cached);
        stale.sort();
        assert_eq!(
            stale,
            vec!["crate-a", "crate-b"],
            "Cargo.lock change must bust all crates"
        );
    }

    #[test]
    fn stale_crates_empty_when_cache_is_default_and_current_is_empty() {
        let current = make_cache("", &[]);
        let cached = CompileCache::default();
        assert!(stale_crates(&current, &cached).is_empty());
    }

    #[test]
    fn stale_crates_all_stale_against_empty_cache() {
        let current = make_cache("lock-abc", &[("crate-a", "h1"), ("crate-b", "h2")]);
        let cached = CompileCache::default();
        // lock hash differs (cached default is ""), so all bust
        let mut stale = stale_crates(&current, &cached);
        stale.sort();
        assert_eq!(stale, vec!["crate-a", "crate-b"]);
    }

    // ── collect_crate_roots ───────────────────────────────────────────────────

    #[test]
    fn collect_crate_roots_finds_direct_child_crate() {
        let ws = TempWorkspace::new();
        ws.write_crate("my-crate", "my-crate");
        ws.write("my-crate/src/lib.rs", "");

        let mut roots = Vec::new();
        collect_crate_roots(&ws.root, &mut roots).unwrap();
        let names: Vec<_> = roots.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"my-crate"), "should find the crate");
    }

    #[test]
    fn collect_crate_roots_skips_workspace_root_manifest() {
        // The workspace root has a Cargo.toml but it's a workspace manifest —
        // collect_crate_roots only inspects subdirectories, never root itself.
        let ws = TempWorkspace::new();
        ws.write("Cargo.toml", "[workspace]\nmembers = [\"crate-a\"]\n");
        ws.write_crate("crate-a", "crate-a");

        let mut roots = Vec::new();
        collect_crate_roots(&ws.root, &mut roots).unwrap();
        let names: Vec<_> = roots.iter().map(|(n, _)| n.as_str()).collect();
        assert!(
            !names.contains(&"workspace"),
            "workspace manifest must not appear as a crate"
        );
        assert!(names.contains(&"crate-a"));
    }

    #[test]
    fn collect_crate_roots_skips_target_directory() {
        let ws = TempWorkspace::new();
        ws.write_crate("real-crate", "real-crate");
        // A Cargo.toml inside target/ must be ignored
        ws.write(
            "target/some-dep/Cargo.toml",
            "[package]\nname = \"phantom\"\n",
        );

        let mut roots = Vec::new();
        collect_crate_roots(&ws.root, &mut roots).unwrap();
        let names: Vec<_> = roots.iter().map(|(n, _)| n.as_str()).collect();
        assert!(!names.contains(&"phantom"), "target/ must be excluded");
        assert!(names.contains(&"real-crate"));
    }

    #[test]
    fn collect_crate_roots_stops_at_nested_crate_root() {
        // outer/ is a crate; outer/inner/ is a nested crate.
        // collect_crate_roots should find both as separate roots without
        // recursing into outer/ to re-discover inner/.
        let ws = TempWorkspace::new();
        ws.write_crate("outer", "outer-crate");
        ws.write_crate("outer/inner", "inner-crate");

        let mut roots = Vec::new();
        collect_crate_roots(&ws.root, &mut roots).unwrap();
        let names: Vec<_> = roots.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"outer-crate"));
        assert!(
            !names.contains(&"inner-crate"),
            "nested crate must not be double-counted via recursion into outer/"
        );
    }

    // ── collect_rs_files ─────────────────────────────────────────────────────

    #[test]
    fn collect_rs_files_finds_rs_files_recursively() {
        let ws = TempWorkspace::new();
        ws.write("src/lib.rs", "");
        ws.write("src/sub/mod.rs", "");

        let mut out = BTreeMap::new();
        collect_rs_files(&ws.root, &ws.root, &mut out).unwrap();
        let keys: Vec<_> = out.keys().map(|k| k.as_str()).collect();
        assert!(keys.iter().any(|k| k.ends_with("lib.rs")));
        assert!(keys.iter().any(|k| k.ends_with("mod.rs")));
    }

    #[test]
    fn collect_rs_files_skips_non_rs_files() {
        let ws = TempWorkspace::new();
        ws.write("src/lib.rs", "");
        ws.write("src/README.md", "docs");
        ws.write("src/config.toml", "[section]");

        let mut out = BTreeMap::new();
        collect_rs_files(&ws.root, &ws.root, &mut out).unwrap();
        assert!(
            out.keys().all(|k| k.ends_with(".rs")),
            "only .rs files should be collected"
        );
    }

    #[test]
    fn collect_rs_files_stops_at_nested_crate_boundary() {
        let ws = TempWorkspace::new();
        ws.write("src/lib.rs", "outer");
        // nested/ has its own Cargo.toml — its files must not be included
        ws.write_crate("nested", "nested-crate");
        ws.write("nested/src/lib.rs", "inner — must not appear");

        let mut out = BTreeMap::new();
        collect_rs_files(&ws.root, &ws.root, &mut out).unwrap();
        assert!(
            out.keys().all(|k| !k.contains("nested/src")),
            "rs files inside a nested crate boundary must be excluded"
        );
    }

    // ── crate_entry ───────────────────────────────────────────────────────────

    #[test]
    fn crate_entry_hash_stable_for_unchanged_files() {
        let ws = TempWorkspace::new();
        ws.write_crate(".", "my-crate");
        ws.write("src/lib.rs", "pub fn foo() {}");

        let e1 = crate_entry(&ws.root).unwrap();
        let e2 = crate_entry(&ws.root).unwrap();
        assert_eq!(e1.hash, e2.hash, "hash must be deterministic");
    }

    #[test]
    fn crate_entry_hash_changes_on_module_change() {
        let ws = TempWorkspace::new();
        ws.write_crate(".", "my-crate");
        let src = ws.write("src/lib.rs", "pub fn foo() {}");

        let before = crate_entry(&ws.root).unwrap().hash;
        std::fs::write(&src, "pub fn foo() { /* changed */ }").unwrap();
        let after = crate_entry(&ws.root).unwrap().hash;

        assert_ne!(
            before, after,
            "crate hash must change when a module changes"
        );
    }

    #[test]
    fn crate_entry_hash_changes_on_cargo_toml_change() {
        let ws = TempWorkspace::new();
        ws.write_crate(".", "my-crate");
        ws.write("src/lib.rs", "");

        let before = crate_entry(&ws.root).unwrap().hash;
        ws.write(
            "Cargo.toml",
            "[package]\nname = \"my-crate\"\nversion = \"0.2.0\"\n",
        );
        let after = crate_entry(&ws.root).unwrap().hash;

        assert_ne!(
            before, after,
            "crate hash must change when Cargo.toml changes"
        );
    }

    #[test]
    fn crate_entry_modules_map_includes_rs_files_and_manifest() {
        let ws = TempWorkspace::new();
        ws.write_crate(".", "my-crate");
        ws.write("src/lib.rs", "");
        ws.write("src/util.rs", "");

        let entry = crate_entry(&ws.root).unwrap();
        let keys: Vec<_> = entry.modules.keys().collect();
        assert!(
            keys.iter().any(|k| k.ends_with("lib.rs")),
            "modules map must include lib.rs"
        );
        assert!(
            keys.iter().any(|k| k.ends_with("util.rs")),
            "modules map must include util.rs"
        );
        assert!(
            keys.iter().any(|k| k.ends_with("Cargo.toml")),
            "modules map must include Cargo.toml"
        );
    }

    // ── snapshot ─────────────────────────────────────────────────────────────

    #[test]
    fn snapshot_detects_all_crates() {
        let ws = TempWorkspace::new();
        ws.write("Cargo.lock", "# lock");
        ws.write_crate("crate-a", "crate-a");
        ws.write_crate("crate-b", "crate-b");

        let snap = snapshot(&ws.root).unwrap();
        assert!(snap.crates.contains_key("crate-a"));
        assert!(snap.crates.contains_key("crate-b"));
    }

    #[test]
    fn snapshot_cargo_lock_hash_present_when_file_exists() {
        let ws = TempWorkspace::new();
        ws.write("Cargo.lock", "lock content");

        let snap = snapshot(&ws.root).unwrap();
        assert!(
            !snap.cargo_lock_hash.is_empty(),
            "cargo_lock_hash must be populated when Cargo.lock exists"
        );
    }

    #[test]
    fn snapshot_cargo_lock_hash_empty_when_file_missing() {
        let ws = TempWorkspace::new();
        // No Cargo.lock written
        let snap = snapshot(&ws.root).unwrap();
        assert!(
            snap.cargo_lock_hash.is_empty(),
            "cargo_lock_hash must be empty when Cargo.lock is absent"
        );
    }

    #[test]
    fn snapshot_hash_changes_when_source_changes() {
        let ws = TempWorkspace::new();
        ws.write("Cargo.lock", "lock");
        ws.write_crate("crate-a", "crate-a");
        let src = ws.write("crate-a/src/lib.rs", "v1");

        let snap1 = snapshot(&ws.root).unwrap();
        std::fs::write(&src, "v2").unwrap();
        let snap2 = snapshot(&ws.root).unwrap();

        assert_ne!(
            snap1.crates["crate-a"].hash, snap2.crates["crate-a"].hash,
            "crate hash in snapshot must change when a source file changes"
        );
        assert_eq!(
            snap1.crates["crate-a"].hash, snap1.crates["crate-a"].hash,
            "unchanged crate hash must remain stable"
        );
    }

    // ── regression: edge cases ────────────────────────────────────────────────

    #[test]
    fn regression_package_name_not_confused_by_dependency_named_name() {
        // A Cargo.toml where a dependency happens to be called "name" must not
        // be mistaken for the package name declaration.
        let toml = "[package]\nname = \"correct\"\n\n[dependencies]\nname = \"0.1\"\n";
        assert_eq!(package_name_from_str(toml).as_deref(), Some("correct"));
    }

    #[test]
    fn regression_taskit_cache_dir_excluded_from_crate_roots() {
        // .taskit-cache/ may contain a compile-cache.json that must never be
        // treated as a crate root even if a stray Cargo.toml lands there.
        let ws = TempWorkspace::new();
        ws.write_crate("real-crate", "real");
        ws.write(
            ".taskit-cache/Cargo.toml",
            "[package]\nname = \"phantom\"\n",
        );

        let mut roots = Vec::new();
        collect_crate_roots(&ws.root, &mut roots).unwrap();
        let names: Vec<_> = roots.iter().map(|(n, _)| n.as_str()).collect();
        assert!(
            !names.contains(&"phantom"),
            ".taskit-cache must be excluded from crate roots"
        );
    }

    #[test]
    fn regression_fuzz_dir_excluded_from_crate_roots() {
        // The cargo-fuzz `fuzz/` crate is excluded from the workspace, so
        // compile-tests must not try to `cargo nextest -p taskit-fuzz` it.
        let ws = TempWorkspace::new();
        ws.write_crate("real-crate", "real");
        ws.write("fuzz/Cargo.toml", "[package]\nname = \"taskit-fuzz\"\n");

        let mut roots = Vec::new();
        collect_crate_roots(&ws.root, &mut roots).unwrap();
        let names: Vec<_> = roots.iter().map(|(n, _)| n.as_str()).collect();
        assert!(
            !names.contains(&"taskit-fuzz"),
            "fuzz/ must be excluded from crate roots"
        );
    }

    // ── master hash integration ───────────────────────────────────────────────

    fn write_compile_cache_json(cache_dir: &std::path::Path, cargo_lock_hash: &str) {
        std::fs::create_dir_all(cache_dir).unwrap();
        let json = serde_json::to_string_pretty(&CompileCache {
            cargo_lock_hash: cargo_lock_hash.to_string(),
            crates: BTreeMap::new(),
        })
        .unwrap();
        std::fs::write(cache_dir.join("compile-cache.json"), json).unwrap();
    }

    #[test]
    fn compile_cache_write_produces_valid_master_hash() {
        let ws = TempWorkspace::new();
        let cache_dir = ws.root.join("cache");
        let master = ws.root.join("master.json");

        write_compile_cache_json(&cache_dir, "lock-abc");
        crate::cache::update_dirs(&cache_dir, &master).unwrap();

        assert!(
            crate::cache::verify_dirs(&cache_dir, &master).unwrap(),
            "master hash must verify immediately after update"
        );
    }

    #[test]
    fn compile_cache_tamper_detected_by_master() {
        let ws = TempWorkspace::new();
        let cache_dir = ws.root.join("cache");
        let master = ws.root.join("master.json");
        let cache_file = cache_dir.join("compile-cache.json");

        write_compile_cache_json(&cache_dir, "lock-v1");
        crate::cache::update_dirs(&cache_dir, &master).unwrap();

        // Tamper: write a different lock hash
        write_compile_cache_json(&cache_dir, "lock-v2-tampered");

        assert!(
            !crate::cache::verify_dirs(&cache_dir, &master).unwrap(),
            "master hash must fail after compile cache is tampered with"
        );
        drop(cache_file);
    }

    #[test]
    fn compile_cache_master_hash_stable_across_two_writes() {
        let ws = TempWorkspace::new();
        let cache_dir = ws.root.join("cache");
        let master = ws.root.join("master.json");

        write_compile_cache_json(&cache_dir, "lock-stable");
        crate::cache::update_dirs(&cache_dir, &master).unwrap();
        let h1 = std::fs::read_to_string(&master).unwrap();

        crate::cache::update_dirs(&cache_dir, &master).unwrap();
        let h2 = std::fs::read_to_string(&master).unwrap();

        assert_eq!(h1, h2, "master hash must be idempotent for unchanged cache");
    }
}
