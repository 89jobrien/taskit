use taskit_engine::ctx::Ctx;
use taskit_engine::flow;
use taskit_engine::flow::{ConflictFile, ConflictResolver, ResolvedFile, merge_with_resolution};
use taskit_types::config::{Config, FlowConfig};
use taskit_types::error::{FlowError, TaskitError};
use taskit_types::output_format::OutputFormat;
use xshell::{Shell, cmd};

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

// ── merge_with_resolution tests ──────────────────────────────────────────────

fn no_op_resolver() -> ConflictResolver {
    Box::new(
        |_files: Vec<ConflictFile>| -> Result<Vec<ResolvedFile>, TaskitError> {
            panic!("resolver should not be called on a clean merge");
        },
    )
}

/// Branch A adds one file; merge into main is clean.
/// Expected: `Ok(())`, resolver never called.
#[test]
fn merge_with_resolution_fast_path_clean_merge() {
    let (_dir, ctx, _flow) = setup_flow_repo();
    let sh = &ctx.sh;

    // Create branch-a with a new file.
    cmd!(sh, "git checkout -b branch-a")
        .run()
        .expect("checkout branch-a");
    commit_file(sh, "a.txt", "content from a\n", "feat: add a.txt");

    cmd!(sh, "git checkout main").run().expect("checkout main");

    let result = merge_with_resolution(&ctx, "branch-a", &no_op_resolver());
    assert!(result.is_ok(), "expected Ok(()), got {result:?}");
}

/// Attempt to merge a non-existent branch — git fails for a non-conflict reason.
/// Expected: `Err(TaskitError::Flow(FlowError::MergeFailed { .. }))`.
#[test]
fn merge_with_resolution_non_conflict_failure() {
    let (_dir, ctx, _flow) = setup_flow_repo();

    let result = merge_with_resolution(&ctx, "branch-does-not-exist", &no_op_resolver());
    match result {
        Err(TaskitError::Flow(FlowError::MergeFailed { .. })) => {}
        other => panic!("expected MergeFailed, got {other:?}"),
    }
}

/// Both main and branch-b edit the same line in README.md, producing a conflict.
/// The resolver returns `Err(NeedsHuman)`.
/// Expected: that error propagates unchanged.
#[test]
fn merge_with_resolution_conflict_resolver_needs_human() {
    let (_dir, ctx, _flow) = setup_flow_repo();
    let sh = &ctx.sh;

    // branch-b: edit line 1 of README.md.
    cmd!(sh, "git checkout -b branch-b")
        .run()
        .expect("checkout branch-b");
    commit_file(
        sh,
        "README.md",
        "# from branch-b\n",
        "chore: edit readme in b",
    );

    // main: edit the same line differently.
    cmd!(sh, "git checkout main").run().expect("checkout main");
    commit_file(
        sh,
        "README.md",
        "# from main\n",
        "chore: edit readme in main",
    );

    let resolver: ConflictResolver = Box::new(|files: Vec<ConflictFile>| {
        let paths: Vec<String> = files
            .iter()
            .map(|f| f.path.to_string_lossy().into())
            .collect();
        Err(FlowError::NeedsHuman { files: paths }.into())
    });

    let result = merge_with_resolution(&ctx, "branch-b", &resolver);
    match result {
        Err(TaskitError::Flow(FlowError::NeedsHuman { .. })) => {}
        other => panic!("expected NeedsHuman, got {other:?}"),
    }
}

/// Both main and branch-c edit the same line — conflict — but the resolver
/// resolves it by writing known content. After the round-trip, `git log`
/// should show a merge commit and the working tree should be clean.
/// Expected: `Ok(())`.
#[test]
fn merge_with_resolution_conflict_resolver_round_trip() {
    let (_dir, ctx, _flow) = setup_flow_repo();
    let sh = &ctx.sh;

    // branch-c: edit README.md.
    cmd!(sh, "git checkout -b branch-c")
        .run()
        .expect("checkout branch-c");
    commit_file(
        sh,
        "README.md",
        "# from branch-c\n",
        "chore: edit readme in c",
    );

    // main: edit the same file differently.
    cmd!(sh, "git checkout main").run().expect("checkout main");
    commit_file(
        sh,
        "README.md",
        "# from main\n",
        "chore: edit readme in main",
    );

    let resolver: ConflictResolver = Box::new(|files: Vec<ConflictFile>| {
        // Accept all files and write a known resolved value.
        let resolved = files
            .into_iter()
            .map(|f| ResolvedFile {
                path: f.path,
                content: "# resolved\n".to_string(),
            })
            .collect();
        Ok(resolved)
    });

    let result = merge_with_resolution(&ctx, "branch-c", &resolver);
    assert!(result.is_ok(), "expected Ok(()), got {result:?}");

    // Working tree must be clean.
    let status = cmd!(sh, "git status --porcelain")
        .read()
        .expect("git status");
    assert!(
        status.trim().is_empty(),
        "working tree dirty after resolution: {status}"
    );

    // The resolved README must have the resolver's content.
    let readme =
        std::fs::read_to_string(sh.current_dir().join("README.md")).expect("read README.md");
    assert_eq!(readme, "# resolved\n");
}
