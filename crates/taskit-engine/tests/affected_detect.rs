//! Integration tests for affected-crate detection against a real git repository.
//!
//! Each test covers one source of change that `detect` must see: commits on
//! the branch, tracked edits not yet committed, and untracked files.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use taskit_engine::affected::detect;
use taskit_engine::config::{CrateEntry, WorkspaceConfig};
use xshell::{Shell, cmd};

const CRATES: [&str; 3] = ["alpha", "beta", "gamma"];

fn workspace() -> WorkspaceConfig {
    WorkspaceConfig {
        crates: CRATES
            .iter()
            .map(|name| CrateEntry {
                dir: format!("crates/{name}"),
                pkg: None,
                exclude_from_version_check: false,
            })
            .collect(),
        ..Default::default()
    }
}

/// Write a file, creating parent directories first.
fn write(root: &Path, rel: &str, contents: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().expect("file has a parent")).expect("create dirs");
    fs::write(path, contents).expect("write file");
}

/// A repo on branch `feature` with `alpha` and `beta` committed on `main`.
///
/// `refs/remotes/origin/main` points at the base commit, so `detect` sees only
/// the work done on `feature`.
struct Repo {
    _dir: tempfile::TempDir,
    root: PathBuf,
    sh: Shell,
}

fn repo() -> Repo {
    let dir = tempfile::tempdir().expect("tempdir");
    let sh = Shell::new().expect("shell");
    sh.change_dir(dir.path());

    cmd!(sh, "git init -b main").run().expect("git init");
    cmd!(sh, "git config user.email test@example.com")
        .run()
        .expect("git config email");
    cmd!(sh, "git config user.name Test")
        .run()
        .expect("git config name");
    cmd!(sh, "git config core.hooksPath /dev/null")
        .run()
        .expect("disable hooks");

    write(dir.path(), "README.md", "# test\n");
    write(dir.path(), "crates/alpha/src/lib.rs", "pub fn a() {}\n");
    write(dir.path(), "crates/beta/src/lib.rs", "pub fn b() {}\n");
    cmd!(sh, "git add .").run().expect("git add");
    cmd!(sh, "git commit -m init").run().expect("git commit");

    // Stand-in for origin/main, which `detect` diffs the branch against.
    cmd!(sh, "git update-ref refs/remotes/origin/main HEAD")
        .run()
        .expect("fake origin/main");
    cmd!(sh, "git checkout -b feature")
        .run()
        .expect("checkout feature");

    Repo {
        root: dir.path().to_path_buf(),
        _dir: dir,
        sh,
    }
}

fn affected(repo: &Repo) -> BTreeSet<String> {
    detect(&repo.sh, &workspace()).expect("detect succeeds in a valid repo")
}

#[test]
fn committed_change_on_branch_is_detected() {
    let repo = repo();
    write(&repo.root, "crates/alpha/src/lib.rs", "pub fn a2() {}\n");
    cmd!(repo.sh, "git commit -am change-alpha")
        .run()
        .expect("commit change");

    assert_eq!(affected(&repo), ["crates/alpha".to_string()].into());
}

#[test]
fn tracked_uncommitted_edit_is_detected() {
    let repo = repo();
    write(&repo.root, "crates/beta/src/lib.rs", "pub fn b2() {}\n");

    assert_eq!(affected(&repo), ["crates/beta".to_string()].into());
}

#[test]
fn untracked_file_is_detected() {
    let repo = repo();
    write(&repo.root, "crates/gamma/src/new.rs", "pub fn g() {}\n");

    assert_eq!(affected(&repo), ["crates/gamma".to_string()].into());
}

#[test]
fn change_outside_any_crate_is_ignored() {
    let repo = repo();
    write(&repo.root, "README.md", "# changed\n");

    assert!(affected(&repo).is_empty());
}
