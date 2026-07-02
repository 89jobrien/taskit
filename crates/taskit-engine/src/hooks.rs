use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use taskit_types::error::TaskitError;
use xshell::{Shell, cmd};

use crate::ctx::Ctx;

// ── pre-push hash cache ───────────────────────────────────────────────────────

const PRE_COMMIT_CACHE: &str = ".taskit-cache/pre-commit.json";
const PRE_PUSH_CACHE: &str = ".taskit-cache/pre-push.json";

/// A passing pre-push run is keyed by the HEAD commit SHA plus the sorted
/// list of affected crate names.  If both match on a subsequent push to the
/// same HEAD (e.g. a force-push of the same tree), the checks are skipped.
#[derive(Serialize, Deserialize, Default, PartialEq, Debug)]
struct PrePushCache {
    head_sha: String,
    crates: Vec<String>,
}

fn load_pre_push_cache() -> PrePushCache {
    fs::read_to_string(PRE_PUSH_CACHE)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_pre_push_cache(cache: &PrePushCache) -> Result<(), TaskitError> {
    if let Some(parent) = Path::new(PRE_PUSH_CACHE).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        PRE_PUSH_CACHE,
        serde_json::to_string_pretty(cache).map_err(TaskitError::other)?,
    )?;
    Ok(())
}

fn head_sha(sh: &Shell) -> Result<String, TaskitError> {
    Ok(cmd!(sh, "git rev-parse HEAD")
        .read()
        .map_err(TaskitError::other)?
        .trim()
        .to_string())
}

// ── pre-commit hash cache ─────────────────────────────────────────────────────

/// Keyed on the SHA-256 of all staged `.rs` file blobs (sorted by path).
/// If the staged tree is identical to the last passing pre-commit, skip.
#[derive(Serialize, Deserialize, Default, PartialEq, Debug)]
struct PreCommitCache {
    staged_hash: String,
}

fn load_pre_commit_cache() -> PreCommitCache {
    fs::read_to_string(PRE_COMMIT_CACHE)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_pre_commit_cache(cache: &PreCommitCache) -> Result<(), TaskitError> {
    if let Some(parent) = std::path::Path::new(PRE_COMMIT_CACHE).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        PRE_COMMIT_CACHE,
        serde_json::to_string_pretty(cache).map_err(TaskitError::other)?,
    )?;
    Ok(())
}

/// Extract sorted `.rs` paths from a `git diff --name-only` listing.
fn rs_paths_from_staged(staged: &str) -> Vec<&str> {
    let mut paths: Vec<&str> = staged.lines().filter(|l| l.ends_with(".rs")).collect();
    paths.sort_unstable();
    paths
}

/// Hash the staged blob of each `.rs` file (via `git show :<path>`) and
/// combine them into a single deterministic digest.
fn staged_rs_hash(sh: &Shell, staged: &str) -> Result<String, TaskitError> {
    use sha2::{Digest, Sha256};
    let paths = rs_paths_from_staged(staged);
    let mut outer = Sha256::new();
    for path in paths {
        let blob = cmd!(sh, "git show {path}")
            .read()
            .map_err(TaskitError::other)?;
        let mut inner = Sha256::new();
        inner.update(blob.as_bytes());
        let file_hash = hex::encode(inner.finalize());
        outer.update(path.as_bytes());
        outer.update(b"\0");
        outer.update(file_hash.as_bytes());
        outer.update(b"\0");
    }
    Ok(hex::encode(outer.finalize()))
}

/// Returns true if any line in `staged` is a `.rs` file path.
fn any_rust_file(staged: &str) -> bool {
    staged.lines().any(|l| l.ends_with(".rs"))
}

pub fn pre_commit(ctx: &Ctx) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    taskit_output::taskit_progress!("Running pre-commit checks (Rust only)...");
    let staged = cmd!(sh, "git diff --cached --name-only --diff-filter=d")
        .read()
        .map_err(TaskitError::other)?;
    if !any_rust_file(&staged) {
        taskit_output::taskit_skip!("No Rust files staged, skipping.");
        return Ok(());
    }

    let hash = staged_rs_hash(sh, &staged)?;
    let cached = load_pre_commit_cache();
    if !hash.is_empty() && cached.staged_hash == hash {
        taskit_output::taskit_skip!("pre-commit: staged tree unchanged since last pass. Skipping.");
        return Ok(());
    }

    ctx.run(cmd!(sh, "cargo fmt --check --all"))?;

    if !ctx.dry_run {
        save_pre_commit_cache(&PreCommitCache { staged_hash: hash })?;
        crate::cache::update()?;
    }
    taskit_output::taskit_ok!("Pre-commit checks passed.");
    Ok(())
}

pub fn pre_push(ctx: &Ctx) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let ws = ctx.ws();
    let cov = ctx.cov();
    taskit_output::taskit_progress!("Running pre-push checks...");
    let affected = crate::affected::detect(sh, ws)?;
    if affected.is_empty() {
        taskit_output::taskit_skip!("No affected Rust crates, skipping.");
        return Ok(());
    }

    let mut crate_names: Vec<String> = affected
        .iter()
        .map(|d| crate::affected::pkg_name(d, ws).to_string())
        .collect();
    crate_names.sort();

    let sha = head_sha(sh)?;
    let cached = load_pre_push_cache();
    if !sha.is_empty()
        && cached
            == (PrePushCache {
                head_sha: sha.clone(),
                crates: crate_names.clone(),
            })
    {
        taskit_output::taskit_skip!(
            "pre-push: checks already passed for HEAD {sha:.12}. Skipping."
        );
        return Ok(());
    }

    for pkg in &crate_names {
        taskit_output::taskit_progress!("--- {pkg} ---");
        ctx.run(cmd!(
            sh,
            "cargo clippy --locked --quiet -p {pkg} --all-targets -- -D warnings"
        ))?;
        ctx.run(cmd!(
            sh,
            "cargo nextest run --locked -p {pkg} --lib --no-tests warn --status-level none --final-status-level fail --hide-progress-bar --fail-fast"
        ))?;
        if let Some(c) = cov
            && *pkg == c.crate_name
        {
            crate::testing::coverage::run(ctx, &c.crate_name, c.threshold())?;
        }
    }
    if let Err(e) = crate::protocol::drift::run(ctx, false, true, false) {
        taskit_output::taskit_err!("[protocol-drift] warning: check could not complete: {e:#}");
    }

    if !ctx.dry_run {
        save_pre_push_cache(&PrePushCache {
            head_sha: sha,
            crates: crate_names,
        })?;
        crate::cache::update()?;
    }
    taskit_output::taskit_ok!("Pre-push checks passed.");
    Ok(())
}

const PRE_COMMIT_HOOK: &str = "#!/usr/bin/env bash\n\
                              # Auto-generated by taskit install-hooks\n\
                              # Delegates Rust checks to taskit; non-Rust checks below.\n\n\
                              taskit pre-commit\n\
                              TASKIT_EXIT=$?\n\n\
                              # Run the original hook for non-Rust checks if it exists\n\
                              ORIG_EXIT=0\n\
                              if [ -f hooks/pre-commit ]; then\n\
                              \tbash hooks/pre-commit\n\
                              \tORIG_EXIT=$?\n\
                              fi\n\n\
                              exit $(( TASKIT_EXIT | ORIG_EXIT ))\n";

const PRE_PUSH_HOOK: &str = "#!/usr/bin/env bash\n\
                             # Auto-generated by taskit install-hooks\n\
                             taskit pre-push \"$@\"\n\
                             TASKIT_EXIT=$?\n\n\
                             # Run the original hook for non-Rust checks if it exists\n\
                             ORIG_EXIT=0\n\
                             if [ -f hooks/pre-push ]; then\n\
                             \tbash hooks/pre-push \"$@\"\n\
                             \tORIG_EXIT=$?\n\
                             fi\n\n\
                             exit $(( TASKIT_EXIT | ORIG_EXIT ))\n";

pub fn install_hooks(ctx: &Ctx) -> Result<(), TaskitError> {
    let hooks_dir = ".git/hooks";

    let pre_commit = PRE_COMMIT_HOOK;
    let pre_push = PRE_PUSH_HOOK;

    if ctx.dry_run {
        taskit_output::taskit_dry!("create_dir_all {hooks_dir}");
        taskit_output::taskit_dry!("write {hooks_dir}/pre-commit");
        taskit_output::taskit_dry!("write {hooks_dir}/pre-push");
        return Ok(());
    }

    fs::create_dir_all(hooks_dir)?;
    fs::write(format!("{hooks_dir}/pre-commit"), pre_commit)?;
    make_executable(&format!("{hooks_dir}/pre-commit"))?;
    fs::write(format!("{hooks_dir}/pre-push"), pre_push)?;
    make_executable(&format!("{hooks_dir}/pre-push"))?;

    taskit_output::taskit_ok!("Git hooks installed:");
    taskit_output::taskit_ok!(".git/hooks/pre-commit");
    taskit_output::taskit_ok!(".git/hooks/pre-push");
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &str) -> Result<(), TaskitError> {
    use std::os::unix::fs::PermissionsExt;
    let perms = fs::Permissions::from_mode(0o755);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &str) -> Result<(), TaskitError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- any_rust_file ---

    // --- PreCommitCache ---

    #[test]
    fn pre_commit_cache_default_has_empty_hash() {
        let c = PreCommitCache::default();
        assert!(c.staged_hash.is_empty());
    }

    #[test]
    fn pre_commit_cache_roundtrip() {
        let orig = PreCommitCache {
            staged_hash: "abc123".to_string(),
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: PreCommitCache = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn empty_staged_hash_never_hits() {
        let cached = PreCommitCache::default();
        let hash = String::new();
        let would_skip = !hash.is_empty() && cached.staged_hash == hash;
        assert!(!would_skip);
    }

    #[test]
    fn matching_staged_hash_is_a_hit() {
        let hash = "cafebabe".to_string();
        let cached = PreCommitCache {
            staged_hash: hash.clone(),
        };
        let would_skip = !hash.is_empty() && cached.staged_hash == hash;
        assert!(would_skip);
    }

    #[test]
    fn different_staged_hash_is_not_a_hit() {
        let cached = PreCommitCache {
            staged_hash: "old".to_string(),
        };
        let hash = "new".to_string();
        let would_skip = !hash.is_empty() && cached.staged_hash == hash;
        assert!(!would_skip);
    }

    #[test]
    fn pre_commit_cache_pretty_json_has_staged_hash_key() {
        let c = PreCommitCache {
            staged_hash: "x".to_string(),
        };
        let json = serde_json::to_string_pretty(&c).unwrap();
        assert!(json.contains("staged_hash"));
    }

    // --- PrePushCache ---

    #[test]
    fn pre_push_cache_default_has_empty_sha() {
        let c = PrePushCache::default();
        assert!(c.head_sha.is_empty());
        assert!(c.crates.is_empty());
    }

    #[test]
    fn pre_push_cache_roundtrip() {
        let orig = PrePushCache {
            head_sha: "abc123".to_string(),
            crates: vec!["my-api".to_string(), "my-cli".to_string()],
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: PrePushCache = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn empty_sha_cache_never_hits() {
        let cached = PrePushCache::default();
        let sha = String::new();
        let would_skip = !sha.is_empty()
            && cached
                == (PrePushCache {
                    head_sha: sha.clone(),
                    crates: vec![],
                });
        assert!(!would_skip);
    }

    #[test]
    fn matching_sha_and_crates_is_a_hit() {
        let sha = "deadbeef".to_string();
        let crates = vec!["my-api".to_string()];
        let cached = PrePushCache {
            head_sha: sha.clone(),
            crates: crates.clone(),
        };
        let would_skip = !sha.is_empty()
            && cached
                == (PrePushCache {
                    head_sha: sha,
                    crates,
                });
        assert!(would_skip);
    }

    #[test]
    fn different_sha_is_not_a_hit() {
        let cached = PrePushCache {
            head_sha: "old".to_string(),
            crates: vec!["my-api".to_string()],
        };
        let sha = "new".to_string();
        let would_skip = !sha.is_empty()
            && cached
                == (PrePushCache {
                    head_sha: sha,
                    crates: vec!["my-api".to_string()],
                });
        assert!(!would_skip);
    }

    #[test]
    fn different_crates_is_not_a_hit() {
        let sha = "abc".to_string();
        let cached = PrePushCache {
            head_sha: sha.clone(),
            crates: vec!["my-api".to_string()],
        };
        let would_skip = !sha.is_empty()
            && cached
                == (PrePushCache {
                    head_sha: sha,
                    crates: vec!["my-cli".to_string()],
                });
        assert!(!would_skip);
    }

    // --- rs_paths_from_staged ---

    #[test]
    fn rs_paths_returns_only_rs_files() {
        let staged = "src/main.rs\nCargo.toml\nREADME.md\nsrc/lib.rs";
        let paths = rs_paths_from_staged(staged);
        assert_eq!(paths, vec!["src/lib.rs", "src/main.rs"]);
    }

    #[test]
    fn rs_paths_returns_empty_for_no_rs_files() {
        let paths = rs_paths_from_staged("Cargo.toml\nREADME.md");
        assert!(paths.is_empty());
    }

    #[test]
    fn rs_paths_are_sorted() {
        let staged = "z/z.rs\na/a.rs\nm/m.rs";
        let paths = rs_paths_from_staged(staged);
        assert_eq!(paths, vec!["a/a.rs", "m/m.rs", "z/z.rs"]);
    }

    #[test]
    fn rs_paths_excludes_rsc_extension() {
        assert!(rs_paths_from_staged("src/foo.rsc").is_empty());
    }

    #[test]
    fn rs_paths_handles_empty_input() {
        assert!(rs_paths_from_staged("").is_empty());
    }

    #[test]
    fn rs_paths_deduplication_not_needed_but_order_stable() {
        // Two identical paths appear twice — both are included (git won't produce
        // duplicates but the function doesn't need to deduplicate).
        let staged = "src/a.rs\nsrc/a.rs";
        let paths = rs_paths_from_staged(staged);
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn rs_paths_handles_nested_paths() {
        let paths = rs_paths_from_staged("my-api/src/graphql/schema.rs");
        assert_eq!(paths, vec!["my-api/src/graphql/schema.rs"]);
    }

    // --- any_rust_file ---

    #[test]
    fn any_rust_file_true_for_single_rs_file() {
        assert!(any_rust_file("src/main.rs"));
    }

    #[test]
    fn any_rust_file_true_when_rs_file_among_others() {
        assert!(any_rust_file("Makefile\nsrc/lib.rs\nCargo.toml"));
    }

    #[test]
    fn any_rust_file_false_for_no_rs_files() {
        assert!(!any_rust_file("Makefile\nCargo.toml\nREADME.md"));
    }

    #[test]
    fn any_rust_file_false_for_empty_input() {
        assert!(!any_rust_file(""));
    }

    #[test]
    fn any_rust_file_requires_rs_extension_not_substring() {
        // "rusty_file.rsc" or "not_rs" should not match
        assert!(!any_rust_file("not_rs\nsrc/foo.rsc\n"));
    }

    #[test]
    fn any_rust_file_matches_nested_path() {
        assert!(any_rust_file("my-api/src/graphql/schema.rs"));
    }

    // --- hook content conformance ---

    #[test]
    fn pre_commit_hook_has_bash_shebang() {
        assert!(PRE_COMMIT_HOOK.starts_with("#!/usr/bin/env bash"));
    }

    #[test]
    fn pre_commit_hook_delegates_to_taskit_pre_commit() {
        assert!(PRE_COMMIT_HOOK.contains("taskit pre-commit"));
    }

    #[test]
    fn pre_commit_hook_delegates_to_original_hook_when_present() {
        assert!(PRE_COMMIT_HOOK.contains("hooks/pre-commit"));
    }

    #[test]
    fn pre_commit_hook_combines_exit_codes() {
        assert!(PRE_COMMIT_HOOK.contains("TASKIT_EXIT"));
        assert!(PRE_COMMIT_HOOK.contains("ORIG_EXIT"));
    }

    #[test]
    fn pre_push_hook_has_bash_shebang() {
        assert!(PRE_PUSH_HOOK.starts_with("#!/usr/bin/env bash"));
    }

    #[test]
    fn pre_push_hook_delegates_to_taskit_pre_push() {
        assert!(PRE_PUSH_HOOK.contains("taskit pre-push"));
    }

    #[test]
    fn pre_push_hook_forwards_args() {
        assert!(PRE_PUSH_HOOK.contains("\"$@\""));
    }

    #[test]
    fn pre_push_hook_delegates_to_original_hook_when_present() {
        assert!(PRE_PUSH_HOOK.contains("hooks/pre-push"));
    }

    // --- master hash integration ---

    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    fn tmpdir() -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("taskit-hooks-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).expect("create tmpdir");
        dir
    }

    #[test]
    fn pre_commit_cache_write_produces_valid_master_hash() {
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        let master = dir.join("master.json");
        fs::create_dir_all(&cache_dir).unwrap();

        let cache_file = cache_dir.join("pre-commit.json");
        let c = PreCommitCache {
            staged_hash: "abc".to_string(),
        };
        fs::write(&cache_file, serde_json::to_string_pretty(&c).unwrap()).unwrap();
        crate::cache::update_dirs(&cache_dir, &master).unwrap();

        assert!(crate::cache::verify_dirs(&cache_dir, &master).unwrap());
    }

    #[test]
    fn pre_commit_cache_tamper_detected_by_master() {
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        let master = dir.join("master.json");
        fs::create_dir_all(&cache_dir).unwrap();

        let cache_file = cache_dir.join("pre-commit.json");
        let c = PreCommitCache {
            staged_hash: "abc".to_string(),
        };
        fs::write(&cache_file, serde_json::to_string_pretty(&c).unwrap()).unwrap();
        crate::cache::update_dirs(&cache_dir, &master).unwrap();

        // Tamper: replace staged_hash without updating master.
        let tampered = PreCommitCache {
            staged_hash: "TAMPERED".to_string(),
        };
        fs::write(
            &cache_file,
            serde_json::to_string_pretty(&tampered).unwrap(),
        )
        .unwrap();

        assert!(!crate::cache::verify_dirs(&cache_dir, &master).unwrap());
    }

    #[test]
    fn pre_push_cache_write_produces_valid_master_hash() {
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        let master = dir.join("master.json");
        fs::create_dir_all(&cache_dir).unwrap();

        let cache_file = cache_dir.join("pre-push.json");
        let c = PrePushCache {
            head_sha: "deadbeef".to_string(),
            crates: vec!["my-api".to_string()],
        };
        fs::write(&cache_file, serde_json::to_string_pretty(&c).unwrap()).unwrap();
        crate::cache::update_dirs(&cache_dir, &master).unwrap();

        assert!(crate::cache::verify_dirs(&cache_dir, &master).unwrap());
    }

    #[test]
    fn pre_push_cache_tamper_detected_by_master() {
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        let master = dir.join("master.json");
        fs::create_dir_all(&cache_dir).unwrap();

        let cache_file = cache_dir.join("pre-push.json");
        let c = PrePushCache {
            head_sha: "abc".to_string(),
            crates: vec!["my-cli".to_string()],
        };
        fs::write(&cache_file, serde_json::to_string_pretty(&c).unwrap()).unwrap();
        crate::cache::update_dirs(&cache_dir, &master).unwrap();

        let tampered = PrePushCache {
            head_sha: "TAMPERED".to_string(),
            crates: vec!["my-cli".to_string()],
        };
        fs::write(
            &cache_file,
            serde_json::to_string_pretty(&tampered).unwrap(),
        )
        .unwrap();

        assert!(!crate::cache::verify_dirs(&cache_dir, &master).unwrap());
    }

    #[test]
    fn both_hook_caches_together_produce_valid_master() {
        let dir = tmpdir();
        let cache_dir = dir.join("cache");
        let master = dir.join("master.json");
        fs::create_dir_all(&cache_dir).unwrap();

        let pc = PreCommitCache {
            staged_hash: "staged".to_string(),
        };
        fs::write(
            cache_dir.join("pre-commit.json"),
            serde_json::to_string_pretty(&pc).unwrap(),
        )
        .unwrap();
        let pp = PrePushCache {
            head_sha: "sha".to_string(),
            crates: vec!["my-api".to_string()],
        };
        fs::write(
            cache_dir.join("pre-push.json"),
            serde_json::to_string_pretty(&pp).unwrap(),
        )
        .unwrap();
        crate::cache::update_dirs(&cache_dir, &master).unwrap();

        assert!(crate::cache::verify_dirs(&cache_dir, &master).unwrap());
    }
}
