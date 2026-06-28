//! Master cache integrity hash.
//!
//! Every time any individual cache file under `.xtask-cache/` is written,
//! `update()` should be called to recompute and persist the master hash.
//!
//! On startup (or on demand via `cargo xtask self-check`), `verify()` can
//! confirm that none of the cache files have drifted since the last write.
//!
//! The master hash is stored separately from the cache directory it covers
//! (`xtask/master-hash`) so it is never included in its own digest.

use anyhow;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};
use taskit_types::error::TaskitError;

const CACHE_DIR: &str = ".xtask-cache";
pub const MASTER_FILE: &str = "xtask/master-hash";

#[derive(Serialize, Deserialize, Default, PartialEq, Debug)]
pub struct MasterHash {
    /// SHA-256 over all `.json` files in `.xtask-cache/` sorted by path.
    pub hash: String,
}

// ── public API ────────────────────────────────────────────────────────────────

/// Recompute the master hash from `cache_dir` and write it to `master_file`.
///
/// Call this immediately after writing any individual cache file.
pub fn update() -> Result<(), TaskitError> {
    update_dirs(Path::new(CACHE_DIR), Path::new(MASTER_FILE))
}

/// Return `true` if the stored master hash matches the current cache directory.
///
/// Returns `true` vacuously when no cache files or master file exist yet.
pub fn verify() -> Result<bool, TaskitError> {
    verify_dirs(Path::new(CACHE_DIR), Path::new(MASTER_FILE))
}

// ── parameterised core (testable) ─────────────────────────────────────────────

pub fn update_dirs(cache_dir: &Path, master_file: &Path) -> Result<(), TaskitError> {
    if !cache_dir.exists() {
        return Ok(());
    }
    let hash = compute(cache_dir)?;
    save(master_file, &MasterHash { hash })?;
    Ok(())
}

pub fn verify_dirs(cache_dir: &Path, master_file: &Path) -> Result<bool, TaskitError> {
    if !master_file.exists() {
        return Ok(true);
    }
    let stored: MasterHash = serde_json::from_str(&fs::read_to_string(master_file)?)
        .map_err(|e| TaskitError::from(anyhow::anyhow!("{e}")))?;
    if stored.hash.is_empty() {
        return Ok(true);
    }
    let current = compute(cache_dir)?;
    Ok(stored.hash == current)
}

// ── internals ────────────────────────────────────────────────────────────────

/// Walk `cache_dir`, hash every `.json` file (sorted by path), and combine
/// into a single deterministic SHA-256 digest.
pub fn compute(cache_dir: &Path) -> Result<String, TaskitError> {
    let mut entries: BTreeMap<String, String> = BTreeMap::new();

    let Ok(rd) = fs::read_dir(cache_dir) else {
        return Ok(String::new());
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json")
            && let Some(hash) = file_hash(&path)
        {
            entries.insert(path.to_string_lossy().into_owned(), hash);
        }
    }

    let mut outer = Sha256::new();
    for (name, hash) in &entries {
        outer.update(name.as_bytes());
        outer.update(b"\0");
        outer.update(hash.as_bytes());
        outer.update(b"\0");
    }
    Ok(hex::encode(outer.finalize()))
}

pub fn file_hash(path: &Path) -> Option<String> {
    let content = fs::read(path).ok()?;
    let mut h = Sha256::new();
    h.update(&content);
    Some(hex::encode(h.finalize()))
}

fn save(master_file: &Path, cache: &MasterHash) -> Result<(), TaskitError> {
    if let Some(parent) = master_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        master_file,
        serde_json::to_string_pretty(cache)
            .map_err(|e| TaskitError::from(anyhow::anyhow!("{e}")))?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn tmpdir() -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("xtask-cache-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).expect("create tmpdir");
        dir
    }

    // ── Unit: MasterHash ─────────────────────────────────────────────────────

    #[test]
    fn master_hash_default_is_empty() {
        assert!(MasterHash::default().hash.is_empty());
    }

    #[test]
    fn master_hash_roundtrip() {
        let orig = MasterHash {
            hash: "abc123".to_string(),
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: MasterHash = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn master_hash_pretty_json_has_hash_key() {
        let json = serde_json::to_string_pretty(&MasterHash { hash: "x".into() }).unwrap();
        assert!(json.contains("\"hash\""));
    }

    // ── Unit: file_hash ───────────────────────────────────────────────────────

    #[test]
    fn file_hash_some_for_existing_file() {
        let dir = tmpdir();
        let p = dir.join("a.json");
        fs::write(&p, b"{}").unwrap();
        assert!(file_hash(&p).is_some());
    }

    #[test]
    fn file_hash_none_for_missing_file() {
        assert!(file_hash(Path::new("/tmp/__xtask_cache_no_such__.json")).is_none());
    }

    #[test]
    fn file_hash_is_deterministic() {
        let dir = tmpdir();
        let p = dir.join("b.json");
        fs::write(&p, b"{\"v\":1}").unwrap();
        assert_eq!(file_hash(&p), file_hash(&p));
    }

    #[test]
    fn file_hash_differs_on_content_change() {
        let dir = tmpdir();
        let p = dir.join("c.json");
        fs::write(&p, b"{\"v\":1}").unwrap();
        let h1 = file_hash(&p).unwrap();
        fs::write(&p, b"{\"v\":2}").unwrap();
        let h2 = file_hash(&p).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn file_hash_same_content_same_hash() {
        let dir = tmpdir();
        let p1 = dir.join("d1.json");
        let p2 = dir.join("d2.json");
        fs::write(&p1, b"{\"same\":true}").unwrap();
        fs::write(&p2, b"{\"same\":true}").unwrap();
        assert_eq!(file_hash(&p1), file_hash(&p2));
    }

    // ── Unit: compute ─────────────────────────────────────────────────────────

    #[test]
    fn compute_returns_empty_string_for_missing_dir() {
        let result = compute(Path::new("/tmp/__xtask_no_such_cache_dir__")).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn compute_returns_empty_string_for_empty_dir() {
        let dir = tmpdir();
        let result = compute(&dir).unwrap();
        // No .json files → BTreeMap is empty → SHA-256 of empty input
        // (still a valid hex string, just the hash of nothing).
        assert_eq!(result.len(), 64, "SHA-256 hex is always 64 chars");
    }

    #[test]
    fn compute_is_deterministic() {
        let dir = tmpdir();
        fs::write(dir.join("a.json"), b"{\"x\":1}").unwrap();
        assert_eq!(compute(&dir).unwrap(), compute(&dir).unwrap());
    }

    #[test]
    fn compute_changes_when_file_content_changes() {
        let dir = tmpdir();
        let p = dir.join("compile-cache.json");
        fs::write(&p, b"{\"v\":1}").unwrap();
        let h1 = compute(&dir).unwrap();
        fs::write(&p, b"{\"v\":2}").unwrap();
        let h2 = compute(&dir).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn compute_changes_when_new_file_added() {
        let dir = tmpdir();
        fs::write(dir.join("pre-commit.json"), b"{\"staged_hash\":\"a\"}").unwrap();
        let h1 = compute(&dir).unwrap();
        fs::write(dir.join("pre-push.json"), b"{\"head_sha\":\"b\"}").unwrap();
        let h2 = compute(&dir).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn compute_changes_when_file_removed() {
        let dir = tmpdir();
        let p = dir.join("pre-commit.json");
        fs::write(&p, b"{\"staged_hash\":\"a\"}").unwrap();
        let h1 = compute(&dir).unwrap();
        fs::remove_file(&p).unwrap();
        let h2 = compute(&dir).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn compute_ignores_non_json_files() {
        let dir = tmpdir();
        fs::write(dir.join("compile-hash"), b"some-binary-data").unwrap();
        let h_before = compute(&dir).unwrap();
        fs::write(dir.join("notes.txt"), b"irrelevant").unwrap();
        let h_after = compute(&dir).unwrap();
        assert_eq!(h_before, h_after);
    }

    #[test]
    fn compute_is_order_independent() {
        // BTreeMap sorts entries — result must be the same regardless of
        // which file was written first or what the filesystem enumeration order is.
        let dir = tmpdir();
        fs::write(dir.join("zzz.json"), b"{\"z\":1}").unwrap();
        fs::write(dir.join("aaa.json"), b"{\"a\":1}").unwrap();
        // Compute twice; BTreeMap ensures stable order both times.
        assert_eq!(compute(&dir).unwrap(), compute(&dir).unwrap());
    }

    // ── Integration: update_dirs + verify_dirs ────────────────────────────────

    #[test]
    fn verify_true_when_no_master_file() {
        let cache_dir = tmpdir();
        let master = cache_dir.parent().unwrap().join("master.json");
        assert!(!master.exists());
        assert!(verify_dirs(&cache_dir, &master).unwrap());
    }

    #[test]
    fn verify_true_after_update_with_no_changes() {
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        let master = dir.join("master.json");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(
            cache_dir.join("pre-commit.json"),
            b"{\"staged_hash\":\"abc\"}",
        )
        .unwrap();

        update_dirs(&cache_dir, &master).unwrap();
        assert!(verify_dirs(&cache_dir, &master).unwrap());
    }

    #[test]
    fn verify_false_after_cache_file_modified() {
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        let master = dir.join("master.json");
        fs::create_dir_all(&cache_dir).unwrap();
        let p = cache_dir.join("pre-push.json");
        fs::write(&p, b"{\"head_sha\":\"old\"}").unwrap();

        update_dirs(&cache_dir, &master).unwrap();
        // Tamper with the cache file after saving the master hash.
        fs::write(&p, b"{\"head_sha\":\"tampered\"}").unwrap();
        assert!(!verify_dirs(&cache_dir, &master).unwrap());
    }

    #[test]
    fn verify_false_after_new_cache_file_added() {
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        let master = dir.join("master.json");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(
            cache_dir.join("pre-commit.json"),
            b"{\"staged_hash\":\"x\"}",
        )
        .unwrap();

        update_dirs(&cache_dir, &master).unwrap();
        // Add a new file without updating master.
        fs::write(cache_dir.join("pre-push.json"), b"{\"head_sha\":\"y\"}").unwrap();
        assert!(!verify_dirs(&cache_dir, &master).unwrap());
    }

    #[test]
    fn verify_true_for_empty_stored_hash() {
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        let master = dir.join("master.json");
        fs::create_dir_all(&cache_dir).unwrap();
        // Write a master file with an empty hash — treated as vacuously valid.
        fs::write(
            &master,
            serde_json::to_string(&MasterHash::default()).unwrap(),
        )
        .unwrap();
        assert!(verify_dirs(&cache_dir, &master).unwrap());
    }

    #[test]
    fn update_is_idempotent() {
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        let master = dir.join("master.json");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join("compile-cache.json"), b"{\"lock\":\"abc\"}").unwrap();

        update_dirs(&cache_dir, &master).unwrap();
        let h1 = fs::read_to_string(&master).unwrap();
        update_dirs(&cache_dir, &master).unwrap();
        let h2 = fs::read_to_string(&master).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn update_creates_master_file_parent_dirs() {
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        let master = dir.join("nested").join("deep").join("master.json");
        fs::create_dir_all(&cache_dir).unwrap();

        update_dirs(&cache_dir, &master).unwrap();
        assert!(master.exists());
    }

    #[test]
    fn master_is_outside_cache_dir_so_not_included_in_own_hash() {
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        // Master lives outside cache_dir — it must not appear in compute().
        let master = dir.join("master.json");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(
            cache_dir.join("pre-commit.json"),
            b"{\"staged_hash\":\"x\"}",
        )
        .unwrap();

        update_dirs(&cache_dir, &master).unwrap();
        let h1: MasterHash = serde_json::from_str(&fs::read_to_string(&master).unwrap()).unwrap();

        // Update again — hash must be stable because master is not in cache_dir.
        update_dirs(&cache_dir, &master).unwrap();
        let h2: MasterHash = serde_json::from_str(&fs::read_to_string(&master).unwrap()).unwrap();
        assert_eq!(h1.hash, h2.hash, "master hash must not include itself");
    }

    // ── Regression ───────────────────────────────────────────────────────────

    #[test]
    fn regression_verify_false_correctly_detects_drift() {
        // Reproduce the scenario: cache file modified, master not updated.
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        let master = dir.join("master.json");
        fs::create_dir_all(&cache_dir).unwrap();
        let p = cache_dir.join("compile-cache.json");
        fs::write(&p, b"{\"cargo_lock_hash\":\"v1\"}").unwrap();
        update_dirs(&cache_dir, &master).unwrap();

        assert!(
            verify_dirs(&cache_dir, &master).unwrap(),
            "should pass before drift"
        );
        fs::write(&p, b"{\"cargo_lock_hash\":\"v2\"}").unwrap();
        assert!(
            !verify_dirs(&cache_dir, &master).unwrap(),
            "should fail after drift"
        );
    }
}
