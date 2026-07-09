use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};
use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

const CACHE_FILE: &str = ".taskit-cache/self-test.json";
const TASKIT_SRC: &str = "src";
const TASKIT_CARGO_TOML: &str = "Cargo.toml";

#[derive(Serialize, Deserialize, Default, PartialEq, Debug)]
struct SelfTestCache {
    /// SHA-256 over all taskit `.rs` source files + `Cargo.toml` + `Cargo.lock`.
    source_hash: String,
}

pub fn run(ctx: &Ctx) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let current_hash = compute_source_hash()?;
    let cached = load_cache();

    if !cached.source_hash.is_empty() && cached.source_hash == current_hash {
        taskit_output::taskit_skip!("taskit self-tests up to date (source unchanged).");
        return Ok(());
    }

    taskit_output::taskit_progress!("Running taskit self-tests...");
    ctx.run(cmd!(sh, "cargo test --locked -p taskit"))?;

    save_cache(&SelfTestCache {
        source_hash: current_hash,
    })?;
    crate::cache::update()?;
    taskit_output::taskit_ok!("taskit self-tests passed. Cache updated.");
    Ok(())
}

fn load_cache() -> SelfTestCache {
    fs::read_to_string(CACHE_FILE)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_cache(cache: &SelfTestCache) -> Result<(), TaskitError> {
    if let Some(parent) = Path::new(CACHE_FILE).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        CACHE_FILE,
        serde_json::to_string_pretty(cache).map_err(TaskitError::other)?,
    )?;
    Ok(())
}

/// Hash all `.rs` files under `src/` plus `Cargo.toml` and `Cargo.lock`.
fn compute_source_hash() -> Result<String, TaskitError> {
    let mut entries: BTreeMap<String, String> = BTreeMap::new();

    collect_rs_files(Path::new(TASKIT_SRC), &mut entries);

    for extra in &[TASKIT_CARGO_TOML, "Cargo.lock"] {
        if let Some(hash) = file_hash(Path::new(extra)) {
            entries.insert(extra.to_string(), hash);
        }
    }

    let mut hasher = Sha256::new();
    for (path, hash) in &entries {
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
        hasher.update(hash.as_bytes());
        hasher.update(b"\0");
    }
    Ok(hex::encode(hasher.finalize()))
}

fn collect_rs_files(dir: &Path, out: &mut BTreeMap<String, String>) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if entry.metadata().is_ok_and(|m| m.is_dir()) {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs")
            && let Some(hash) = file_hash(&path)
        {
            out.insert(path.to_string_lossy().into_owned(), hash);
        }
    }
}

fn file_hash(path: &Path) -> Option<String> {
    let content = fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&content);
    Some(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn tmpdir() -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("taskit-selftest-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).expect("create tmpdir");
        dir
    }

    // --- file_hash ---

    #[test]
    fn file_hash_returns_some_for_existing_file() {
        let dir = tmpdir();
        let p = dir.join("a.rs");
        fs::write(&p, b"fn foo() {}").unwrap();
        assert!(file_hash(&p).is_some());
    }

    #[test]
    fn file_hash_returns_none_for_missing_file() {
        assert!(file_hash(Path::new("/tmp/__taskit_st_missing_xyz__.rs")).is_none());
    }

    #[test]
    fn file_hash_is_deterministic() {
        let dir = tmpdir();
        let p = dir.join("b.rs");
        fs::write(&p, b"pub fn bar() {}").unwrap();
        assert_eq!(file_hash(&p), file_hash(&p));
    }

    #[test]
    fn file_hash_differs_for_different_content() {
        let dir = tmpdir();
        let p1 = dir.join("c1.rs");
        let p2 = dir.join("c2.rs");
        fs::write(&p1, b"fn a() {}").unwrap();
        fs::write(&p2, b"fn b() {}").unwrap();
        assert_ne!(file_hash(&p1), file_hash(&p2));
    }

    #[test]
    fn file_hash_same_content_same_hash() {
        let dir = tmpdir();
        let p1 = dir.join("d1.rs");
        let p2 = dir.join("d2.rs");
        let content = b"fn identical() {}";
        fs::write(&p1, content).unwrap();
        fs::write(&p2, content).unwrap();
        assert_eq!(file_hash(&p1), file_hash(&p2));
    }

    // --- collect_rs_files ---

    #[test]
    fn collect_rs_files_finds_rs_files() {
        let dir = tmpdir();
        fs::write(dir.join("main.rs"), b"fn main() {}").unwrap();
        fs::write(dir.join("lib.rs"), b"pub fn foo() {}").unwrap();
        fs::write(dir.join("build.toml"), b"[build]").unwrap();
        let mut out = BTreeMap::new();
        collect_rs_files(&dir, &mut out);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn collect_rs_files_recurses_into_subdirs() {
        let dir = tmpdir();
        let sub = dir.join("testing");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("k8s.rs"), b"").unwrap();
        let mut out = BTreeMap::new();
        collect_rs_files(&dir, &mut out);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn collect_rs_files_skips_non_rs_files() {
        let dir = tmpdir();
        fs::write(dir.join("Cargo.toml"), b"[package]").unwrap();
        fs::write(dir.join("README.md"), b"# readme").unwrap();
        let mut out = BTreeMap::new();
        collect_rs_files(&dir, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn collect_rs_files_hash_changes_when_file_changes() {
        let dir = tmpdir();
        let p = dir.join("foo.rs");
        fs::write(&p, b"fn v1() {}").unwrap();
        let mut out1 = BTreeMap::new();
        collect_rs_files(&dir, &mut out1);

        fs::write(&p, b"fn v2() {}").unwrap();
        let mut out2 = BTreeMap::new();
        collect_rs_files(&dir, &mut out2);

        assert_ne!(out1, out2);
    }

    // --- SelfTestCache ---

    #[test]
    fn self_test_cache_default_has_empty_hash() {
        let cache = SelfTestCache::default();
        assert!(cache.source_hash.is_empty());
    }

    #[test]
    fn self_test_cache_serializes_and_deserializes() {
        let cache = SelfTestCache {
            source_hash: "abc123".to_string(),
        };
        let json = serde_json::to_string(&cache).unwrap();
        let back: SelfTestCache = serde_json::from_str(&json).unwrap();
        assert_eq!(back.source_hash, "abc123");
    }

    #[test]
    fn self_test_cache_roundtrip_preserves_hash() {
        let original = SelfTestCache {
            source_hash: "deadbeef01234567".to_string(),
        };
        let json = serde_json::to_string_pretty(&original).unwrap();
        let restored: SelfTestCache = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    // --- cache hit/miss semantics ---

    #[test]
    fn empty_cached_hash_is_never_a_hit() {
        // Even if current_hash were somehow empty, an empty cached hash must not skip tests.
        let cached = SelfTestCache {
            source_hash: String::new(),
        };
        let current = String::new();
        // The run() guard: !cached.source_hash.is_empty() && cached == current
        assert!(
            cached.source_hash.is_empty(),
            "guard should reject empty cached hash"
        );
        // So the condition `!cached.source_hash.is_empty()` is false → tests run
        let would_skip = !cached.source_hash.is_empty() && cached.source_hash == current;
        assert!(!would_skip);
    }

    #[test]
    fn matching_nonempty_hash_is_a_hit() {
        let hash = "cafebabe".to_string();
        let cached = SelfTestCache {
            source_hash: hash.clone(),
        };
        let would_skip = !cached.source_hash.is_empty() && cached.source_hash == hash;
        assert!(would_skip);
    }

    #[test]
    fn mismatched_hash_is_not_a_hit() {
        let cached = SelfTestCache {
            source_hash: "old_hash".to_string(),
        };
        let current = "new_hash".to_string();
        let would_skip = !cached.source_hash.is_empty() && cached.source_hash == current;
        assert!(!would_skip);
    }

    // --- master hash integration ---

    fn write_self_test_cache_json(cache_dir: &std::path::Path, source_hash: &str) {
        std::fs::create_dir_all(cache_dir).unwrap();
        let json = serde_json::to_string_pretty(&SelfTestCache {
            source_hash: source_hash.to_string(),
        })
        .unwrap();
        std::fs::write(cache_dir.join("self-test.json"), json).unwrap();
    }

    #[test]
    fn self_test_cache_write_produces_valid_master_hash() {
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        let master = dir.join("master.json");

        write_self_test_cache_json(&cache_dir, "src-hash-abc");
        crate::cache::update_dirs(&cache_dir, &master).unwrap();

        assert!(
            crate::cache::verify_dirs(&cache_dir, &master).unwrap(),
            "master hash must verify immediately after update"
        );
    }

    #[test]
    fn self_test_cache_tamper_detected_by_master() {
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        let master = dir.join("master.json");

        write_self_test_cache_json(&cache_dir, "src-hash-v1");
        crate::cache::update_dirs(&cache_dir, &master).unwrap();

        // Tamper: overwrite with a different source hash
        write_self_test_cache_json(&cache_dir, "src-hash-v2-tampered");

        assert!(
            !crate::cache::verify_dirs(&cache_dir, &master).unwrap(),
            "master hash must fail after self-test cache is tampered with"
        );
    }

    #[test]
    fn self_test_cache_master_hash_stable_across_two_writes() {
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        let master = dir.join("master.json");

        write_self_test_cache_json(&cache_dir, "src-hash-stable");
        crate::cache::update_dirs(&cache_dir, &master).unwrap();
        let h1 = std::fs::read_to_string(&master).unwrap();

        crate::cache::update_dirs(&cache_dir, &master).unwrap();
        let h2 = std::fs::read_to_string(&master).unwrap();

        assert_eq!(h1, h2, "master hash must be idempotent for unchanged cache");
    }
}
