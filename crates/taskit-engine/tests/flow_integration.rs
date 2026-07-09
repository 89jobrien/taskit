use taskit_core::conflict_resolver::ConflictResolver;
use taskit_engine::ctx::Ctx;
use taskit_engine::flow::{self, merge_with_resolution};
use taskit_types::config::{Config, FlowConfig};
use taskit_types::conflict::{ConflictFile, ResolvedFile};
use taskit_types::error::{FlowError, TaskitError};
use taskit_types::output_format::OutputFormat;
use xshell::{Shell, cmd};

#[allow(dead_code)]
struct NoOpResolver;
impl ConflictResolver for NoOpResolver {
    fn resolve(&self, _files: &[ConflictFile]) -> Result<Vec<ResolvedFile>, TaskitError> {
        Ok(vec![])
    }
}

struct PanicResolver;
impl ConflictResolver for PanicResolver {
    fn resolve(&self, _files: &[ConflictFile]) -> Result<Vec<ResolvedFile>, TaskitError> {
        panic!("resolver should not be called when there are no conflicts");
    }
}

/// Set up a temporary git repo with `main`, `staging`, and `release` branches.
///
/// Returns `(TempDir, Ctx, FlowConfig)`. The context's shell working directory
/// is set to the tempdir root. The caller must keep the `TempDir` alive for the
/// duration of the test.
fn setup_flow_repo() -> (tempfile::TempDir, Ctx, FlowConfig) {
    let dir = tempfile::tempdir().expect("tempdir");
    let sh = Shell::new().expect("shell");
    sh.change_dir(dir.path());

    // Minimal git config so commits work in a clean environment.
    cmd!(sh, "git init -b main").run().expect("git init");
    cmd!(sh, "git config user.email test@example.com")
        .run()
        .expect("git config email");
    cmd!(sh, "git config user.name Test")
        .run()
        .expect("git config name");

    // Initial commit on main.
    sh.write_file("README.md", "# test\n")
        .expect("write README");
    cmd!(sh, "git add README.md").run().expect("git add");
    cmd!(sh, "git commit -m init").run().expect("git commit");

    // Create staging and release from main.
    cmd!(sh, "git branch staging")
        .run()
        .expect("branch staging");
    cmd!(sh, "git branch release")
        .run()
        .expect("branch release");

    let flow = FlowConfig::default();
    let ctx = Ctx::new(
        sh,
        dir.path().to_path_buf(),
        Config::default(),
        false,
        OutputFormat::Human,
    );
    (dir, ctx, flow)
}

/// Helper: write a file, stage it, and commit with a message.
fn commit_file(sh: &Shell, name: &str, content: &str, message: &str) {
    sh.write_file(name, content).expect("write file");
    cmd!(sh, "git add {name}").run().expect("git add");
    cmd!(sh, "git commit -m {message}")
        .run()
        .expect("git commit");
}

/// Helper: extract the head SHA of a branch.
fn branch_sha(sh: &Shell, branch: &str) -> String {
    cmd!(sh, "git rev-parse {branch}")
        .read()
        .expect("rev-parse")
        .trim()
        .to_string()
}

#[test]
fn flow_status_shows_all_branches() {
    let (_dir, ctx, flow) = setup_flow_repo();
    let result = flow::status(&ctx, &flow);
    assert!(result.is_ok(), "flow::status failed: {result:?}");
}

#[test]
fn flow_guard_blocks_on_main() {
    let (_dir, ctx, flow) = setup_flow_repo();
    // We start on main after setup.
    let result = flow::guard(&ctx, &flow);
    match result {
        Err(TaskitError::Flow(FlowError::ProtectedBranch { branch, .. })) => {
            assert_eq!(branch, "main");
        }
        other => panic!("expected ProtectedBranch, got {other:?}"),
    }
}

#[test]
fn flow_guard_allows_staging() {
    let (_dir, ctx, flow) = setup_flow_repo();
    cmd!(ctx.sh, "git checkout staging")
        .run()
        .expect("checkout staging");
    let result = flow::guard(&ctx, &flow);
    assert!(
        result.is_ok(),
        "flow::guard should allow staging: {result:?}"
    );
}

#[test]
fn flow_promote_merges_staging_to_release() {
    let (_dir, ctx, flow) = setup_flow_repo();

    // Add a commit on staging.
    cmd!(ctx.sh, "git checkout staging")
        .run()
        .expect("checkout staging");
    commit_file(
        &ctx.sh,
        "feature.txt",
        "feature content\n",
        "feat: add feature",
    );

    let staging_sha = branch_sha(&ctx.sh, "staging");

    flow::promote(&ctx, &flow).expect("flow::promote");

    // After promote we should be back on staging; verify release has the commit.
    let release_sha = branch_sha(&ctx.sh, "release");
    assert_ne!(
        staging_sha, release_sha,
        "release should have advanced past its original tip"
    );

    // The staging commit must be reachable from release.
    let reachable = cmd!(ctx.sh, "git merge-base --is-ancestor {staging_sha} release")
        .run()
        .is_ok();
    assert!(reachable, "staging commit not reachable from release");
}

#[test]
fn flow_promote_fails_on_wrong_branch() {
    let (_dir, ctx, flow) = setup_flow_repo();
    // We are on main — promote requires staging.
    let result = flow::promote(&ctx, &flow);
    match result {
        Err(TaskitError::Flow(FlowError::WrongBranch { expected, actual })) => {
            assert_eq!(expected, "staging");
            assert_eq!(actual, "main");
        }
        other => panic!("expected WrongBranch, got {other:?}"),
    }
}

#[test]
fn flow_finish_merges_release_to_main_and_syncs_staging() {
    let (_dir, ctx, flow) = setup_flow_repo();

    // Add a commit directly on release.
    cmd!(ctx.sh, "git checkout release")
        .run()
        .expect("checkout release");
    commit_file(&ctx.sh, "hotfix.txt", "hotfix content\n", "fix: hotfix");

    let release_sha = branch_sha(&ctx.sh, "release");

    flow::finish(&ctx, &flow).expect("flow::finish");

    // After finish we are on staging. Verify main contains the release commit.
    let main_reachable = cmd!(ctx.sh, "git merge-base --is-ancestor {release_sha} main")
        .run()
        .is_ok();
    assert!(main_reachable, "release commit not reachable from main");

    // Verify staging was synced (main commit reachable from staging).
    let main_sha = branch_sha(&ctx.sh, "main");
    let staging_reachable = cmd!(ctx.sh, "git merge-base --is-ancestor {main_sha} staging")
        .run()
        .is_ok();
    assert!(
        staging_reachable,
        "main not yet synced into staging after finish"
    );
}

#[test]
fn flow_promote_leaves_user_on_release() {
    let (_dir, ctx, flow) = setup_flow_repo();

    // Add a commit on staging so promote has something to merge.
    cmd!(ctx.sh, "git checkout staging")
        .run()
        .expect("checkout staging");
    commit_file(&ctx.sh, "feature.txt", "feature\n", "feat: add feature");

    flow::promote(&ctx, &flow).expect("flow::promote");

    let branch = cmd!(ctx.sh, "git branch --show-current")
        .read()
        .expect("branch");
    assert_eq!(
        branch.trim(),
        "release",
        "promote should leave user on release"
    );
}

#[test]
fn flow_finish_auto_switches_from_staging() {
    let (_dir, ctx, flow) = setup_flow_repo();

    // Seed release with a commit.
    cmd!(ctx.sh, "git checkout release")
        .run()
        .expect("checkout release");
    commit_file(&ctx.sh, "hotfix.txt", "hotfix\n", "fix: hotfix");

    // Switch to staging — finish should auto-switch to release.
    cmd!(ctx.sh, "git checkout staging")
        .run()
        .expect("checkout staging");

    flow::finish(&ctx, &flow).expect("flow::finish should succeed from staging");
}

#[test]
fn flow_auto_requires_staging_branch() {
    let (_dir, ctx, flow) = setup_flow_repo();
    // We start on main — auto requires staging.
    let result = flow::auto(&ctx, &flow, &PanicResolver);
    match result {
        Err(TaskitError::Flow(FlowError::WrongBranch { expected, actual })) => {
            assert_eq!(expected, "staging");
            assert_eq!(actual, "main");
        }
        other => panic!("expected WrongBranch, got {other:?}"),
    }
}

#[test]
fn flow_auto_requires_clean_staging() {
    let (_dir, ctx, flow) = setup_flow_repo();
    cmd!(ctx.sh, "git checkout staging")
        .run()
        .expect("checkout staging");
    // Create an uncommitted file.
    ctx.sh
        .write_file("dirty.txt", "untracked\n")
        .expect("write dirty");
    cmd!(ctx.sh, "git add dirty.txt").run().expect("git add");
    // staged but not committed
    let result = flow::auto(&ctx, &flow, &PanicResolver);
    match result {
        Err(TaskitError::Flow(FlowError::DirtyWorktree { branch })) => {
            assert_eq!(branch, "staging");
        }
        other => panic!("expected DirtyWorktree, got {other:?}"),
    }
}

#[test]
fn flow_auto_no_conflict_happy_path_ends_on_staging() {
    let (_dir, ctx, flow) = setup_flow_repo();

    // Add a commit on staging.
    cmd!(ctx.sh, "git checkout staging")
        .run()
        .expect("checkout staging");
    commit_file(&ctx.sh, "feature.txt", "new feature\n", "feat: add feature");

    // `auto` will run CI internally. We need a minimal taskit.toml in the tempdir
    // so that run_default_internal can load config. Use dry_run=true via a new Ctx
    // so ci steps don't actually execute.
    let dry_ctx = Ctx::new(
        xshell::Shell::new().expect("shell"),
        ctx.root.clone(),
        Config::default(),
        true, // dry_run
        OutputFormat::Human,
    );
    // The dry_run ctx needs its shell in the tempdir.
    dry_ctx.sh.change_dir(&ctx.root);

    let result = flow::auto(&dry_ctx, &flow, &PanicResolver);
    // In dry_run mode all git ops are printed but not run — the outcome is Ok.
    assert!(result.is_ok(), "flow::auto dry-run failed: {result:?}");
}

// ── merge_with_resolution integration tests ───────────────────────────────────

#[test]
fn merge_with_resolution_fast_path_no_conflict() {
    let (_dir, ctx, _flow) = setup_flow_repo();

    // Add a commit on staging that doesn't exist on release.
    cmd!(ctx.sh, "git checkout staging")
        .run()
        .expect("checkout staging");
    commit_file(&ctx.sh, "feature.txt", "feature\n", "feat: add feature");

    // Merge staging into release — no conflict, resolver must not be called.
    cmd!(ctx.sh, "git checkout release")
        .run()
        .expect("checkout release");
    let result = merge_with_resolution(
        &ctx,
        "staging",
        "flow: merge staging into release",
        &PanicResolver,
    );
    assert!(result.is_ok(), "clean merge failed: {result:?}");
    // release should now contain the staging commit.
    let log = cmd!(ctx.sh, "git log --oneline -1").read().expect("log");
    assert!(log.contains("merge staging into release"), "got: {log}");
}

#[test]
fn merge_with_resolution_needs_human_escalates() {
    // Build a repo with a true content conflict from scratch.
    let dir = tempfile::tempdir().expect("tempdir");
    let sh = xshell::Shell::new().expect("shell");
    sh.change_dir(dir.path());
    cmd!(sh, "git init -b main").run().expect("git init");
    cmd!(sh, "git config user.email test@example.com")
        .run()
        .expect("email");
    cmd!(sh, "git config user.name Test").run().expect("name");
    cmd!(sh, "git config core.hooksPath /dev/null")
        .run()
        .expect("disable hooks");
    // Shared base commit.
    sh.write_file("shared.txt", "base content\n")
        .expect("write");
    cmd!(sh, "git add shared.txt").run().expect("add");
    cmd!(sh, "git commit -m base").run().expect("commit");
    // Create staging and release from the same base, diverge shared.txt.
    cmd!(sh, "git branch staging")
        .run()
        .expect("branch staging");
    cmd!(sh, "git checkout staging")
        .run()
        .expect("checkout staging");
    sh.write_file("shared.txt", "staging version\n")
        .expect("write");
    cmd!(sh, "git add shared.txt").run().expect("add");
    cmd!(sh, "git commit -m staging-change")
        .run()
        .expect("commit staging");
    cmd!(sh, "git checkout main").run().expect("checkout main");
    cmd!(sh, "git branch release")
        .run()
        .expect("branch release");
    cmd!(sh, "git checkout release")
        .run()
        .expect("checkout release");
    sh.write_file("shared.txt", "release version\n")
        .expect("write");
    cmd!(sh, "git add shared.txt").run().expect("add");
    cmd!(sh, "git commit -m release-change")
        .run()
        .expect("commit release");

    let ctx = Ctx::new(
        sh,
        dir.path().to_path_buf(),
        Config::default(),
        false,
        OutputFormat::Human,
    );

    struct EscalateResolver;
    impl ConflictResolver for EscalateResolver {
        fn resolve(&self, files: &[ConflictFile]) -> Result<Vec<ResolvedFile>, TaskitError> {
            Err(FlowError::NeedsHuman {
                path: files.first().map(|f| f.path.clone()).unwrap_or_default(),
                reason: "too complex".into(),
            }
            .into())
        }
    }

    let result = merge_with_resolution(
        &ctx,
        "staging",
        "flow: merge staging into release",
        &EscalateResolver,
    );
    match result {
        Err(TaskitError::Flow(FlowError::NeedsHuman { path, .. })) => {
            assert_eq!(path, "shared.txt");
        }
        other => panic!("expected NeedsHuman, got {other:?}"),
    }
    // Abort the in-progress merge so the repo is clean for teardown.
    let _ = cmd!(ctx.sh, "git merge --abort").run();
}

#[test]
fn merge_with_resolution_resolver_resolves_conflict() {
    // Build a repo with a true content conflict from scratch.
    let dir = tempfile::tempdir().expect("tempdir");
    let sh = xshell::Shell::new().expect("shell");
    sh.change_dir(dir.path());
    cmd!(sh, "git init -b main").run().expect("git init");
    cmd!(sh, "git config user.email test@example.com")
        .run()
        .expect("email");
    cmd!(sh, "git config user.name Test").run().expect("name");
    cmd!(sh, "git config core.hooksPath /dev/null")
        .run()
        .expect("disable hooks");
    sh.write_file("shared.txt", "base content\n")
        .expect("write");
    cmd!(sh, "git add shared.txt").run().expect("add");
    cmd!(sh, "git commit -m base").run().expect("commit");
    cmd!(sh, "git branch staging")
        .run()
        .expect("branch staging");
    cmd!(sh, "git checkout staging")
        .run()
        .expect("checkout staging");
    sh.write_file("shared.txt", "staging version\n")
        .expect("write");
    cmd!(sh, "git add shared.txt").run().expect("add");
    cmd!(sh, "git commit -m staging-change")
        .run()
        .expect("commit staging");
    cmd!(sh, "git checkout main").run().expect("checkout main");
    cmd!(sh, "git branch release")
        .run()
        .expect("branch release");
    cmd!(sh, "git checkout release")
        .run()
        .expect("checkout release");
    sh.write_file("shared.txt", "release version\n")
        .expect("write");
    cmd!(sh, "git add shared.txt").run().expect("add");
    cmd!(sh, "git commit -m release-change")
        .run()
        .expect("commit release");

    let ctx = Ctx::new(
        sh,
        dir.path().to_path_buf(),
        Config::default(),
        false,
        OutputFormat::Human,
    );

    struct PickOurs;
    impl ConflictResolver for PickOurs {
        fn resolve(&self, files: &[ConflictFile]) -> Result<Vec<ResolvedFile>, TaskitError> {
            Ok(files
                .iter()
                .map(|f| ResolvedFile::new(f.path.clone(), "resolved content\n"))
                .collect())
        }
    }

    let result = merge_with_resolution(
        &ctx,
        "staging",
        "flow: merge staging into release",
        &PickOurs,
    );
    assert!(result.is_ok(), "resolution failed: {result:?}");

    // The resolved content should be on disk.
    let content = std::fs::read_to_string(ctx.root.join("shared.txt")).expect("read");
    assert_eq!(content, "resolved content\n");
}

#[test]
fn merge_with_resolution_nothing_to_merge_returns_merge_failed() {
    let (_dir, ctx, _flow) = setup_flow_repo();

    // release and staging are identical — merge produces "Already up to date."
    // which git exits 0 for. Force a failure by merging a non-existent branch.
    cmd!(ctx.sh, "git checkout release")
        .run()
        .expect("checkout release");
    let result = merge_with_resolution(
        &ctx,
        "nonexistent-branch",
        "flow: merge nonexistent",
        &PanicResolver,
    );
    assert!(
        result.is_err(),
        "expected error merging nonexistent branch, got Ok"
    );
}
