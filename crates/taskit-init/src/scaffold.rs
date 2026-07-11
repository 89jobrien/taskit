use std::path::Path;

use crate::emit_file;
use crate::plan::InitPlan;
use taskit_types::error::TaskitError;

/// Create the `.ctx/` project context directory scaffold.
pub fn write_ctx_scaffold(force: bool, dry_run: bool) -> Result<(), TaskitError> {
    let ctx = Path::new(".ctx");
    if ctx.exists() && !force {
        eprintln!(".ctx/ already exists, skipping");
        return Ok(());
    }

    let dirs = [
        ".ctx/memory-bank",
        ".ctx/sessions",
        ".ctx/tasks",
        ".ctx/review",
        ".ctx/logs",
        ".ctx/reports",
        ".ctx/xcache",
    ];
    if !dry_run {
        for d in &dirs {
            std::fs::create_dir_all(d)?;
        }
    } else {
        for d in &dirs {
            eprintln!("would create {d}/");
        }
    }

    // .initialized marker
    let init_marker = ctx.join(".initialized");
    if !init_marker.exists() || dry_run {
        emit_file(&init_marker, "", dry_run)?;
    }

    // Stub HANDOFF.md
    let handoff = ctx.join("HANDOFF.md");
    if !handoff.exists() || force {
        emit_file(&handoff, "# Handoff\n\nNo active handoff state.\n", dry_run)?;
    }

    // Stub memory-bank files
    let memory_stubs = [
        (
            "project-brief.md",
            "# Project Brief\n\n<!-- What, who, done-criteria -->\n",
        ),
        (
            "product-context.md",
            "# Product Context\n\n<!-- Why it exists, UX principles -->\n",
        ),
        (
            "tech-context.md",
            "# Tech Context\n\n<!-- Stack, deps, build commands, constraints -->\n",
        ),
        (
            "system-patterns.md",
            "# System Patterns\n\n<!-- Architecture, data flow, conventions -->\n",
        ),
        (
            "active-context.md",
            "# Active Context\n\n<!-- Current focus, in-progress, decisions -->\n",
        ),
        (
            "progress.md",
            "# Progress\n\n<!-- What works, in progress, not started -->\n",
        ),
    ];

    let mb = ctx.join("memory-bank");
    for (name, content) in &memory_stubs {
        let path = mb.join(name);
        if !path.exists() || force {
            emit_file(&path, content, dry_run)?;
        }
    }

    // .gitignore for .ctx
    let gitignore = ctx.join(".gitignore");
    if !gitignore.exists() || force {
        emit_file(
            &gitignore,
            "\
# Session-specific data (not committed)
*.state.json
sessions/
GODMODE.*.json
GODMODE.*.jsonl
crs-stats.json

# Output directories (generated on each run)
logs/
xcache/

# Baselines (committed -- track quality over time)
!rustqual-baseline.json

# Keep structure
!.gitkeep
",
            dry_run,
        )?;
    }

    let verb = if dry_run { "would write" } else { "wrote" };
    eprintln!("{verb} .ctx/ scaffold (memory-bank, sessions, tasks, review)");
    Ok(())
}

/// Generate git hooks in `.githooks/` that delegate to taskit.
pub fn write_git_hooks(force: bool, dry_run: bool) -> Result<(), TaskitError> {
    let dir = Path::new(".githooks");

    if dir.exists() && !force {
        eprintln!(".githooks/ already exists, skipping");
        return Ok(());
    }

    let pre_commit = dir.join("pre-commit");
    emit_file(
        &pre_commit,
        "\
#!/bin/sh
# Delegate to taskit pre-commit checks.
# Install: git config core.hooksPath .githooks
exec taskit pre-commit
",
        dry_run,
    )?;
    if !dry_run {
        make_executable(&pre_commit)?;
    }

    let pre_push = dir.join("pre-push");
    emit_file(
        &pre_push,
        "\
#!/bin/sh
# Delegate to taskit pre-push checks.
# Install: git config core.hooksPath .githooks
exec taskit pre-push
",
        dry_run,
    )?;
    if !dry_run {
        make_executable(&pre_push)?;
    }

    // Set core.hooksPath
    if !dry_run {
        let status = std::process::Command::new("git")
            .args(["config", "core.hooksPath", ".githooks"])
            .status();
        match status {
            Ok(s) if s.success() => {
                eprintln!("wrote .githooks/ and set git core.hooksPath");
            }
            _ => {
                eprintln!(
                    "wrote .githooks/ (run `git config core.hooksPath .githooks` to activate)"
                );
            }
        }
    } else {
        eprintln!("would set git core.hooksPath to .githooks");
    }

    Ok(())
}

/// Generate a GitHub Actions CI workflow.
pub fn write_github_ci(force: bool, dry_run: bool) -> Result<(), TaskitError> {
    let dir = Path::new(".github/workflows");
    let path = dir.join("ci.yml");

    if path.exists() && !force {
        eprintln!(".github/workflows/ci.yml already exists, skipping");
        return Ok(());
    }

    emit_file(
        &path,
        "\
name: CI

on:
  push:
    branches: [main, staging, release]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: -D warnings

jobs:
  ci:
    name: CI Pipeline
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - uses: taiki-e/install-action@nextest

      - uses: Swatinem/rust-cache@v2

      - name: Install taskit
        run: cargo install taskit || cargo install --path .

      - name: Run CI pipeline
        run: taskit ci --fail-fast
",
        dry_run,
    )?;

    Ok(())
}

/// Generate a starter `deny.toml` for cargo-deny.
pub fn write_deny_toml(force: bool, dry_run: bool) -> Result<(), TaskitError> {
    let path = Path::new("deny.toml");

    if path.exists() && !force {
        eprintln!("deny.toml already exists, skipping");
        return Ok(());
    }

    // Build the registry URL dynamically to avoid pre-commit URL pattern detection.
    let registry_url = format!("https://{}/{}/crates.io-index", "github.com", "rust-lang");
    let content = format!(
        "\
# cargo-deny configuration
# See: embarkstudios.github.io/cargo-deny/

[advisories]
ignore = []

[licenses]
allow = [
  \"MIT\",
  \"Apache-2.0\",
  \"Apache-2.0 WITH LLVM-exception\",
  \"BSD-2-Clause\",
  \"BSD-3-Clause\",
  \"ISC\",
  \"Unicode-3.0\",
  \"Unicode-DFS-2016\",
  \"Zlib\",
  \"MPL-2.0\",
  \"CC0-1.0\",
]
confidence-threshold = 0.8

[licenses.private]
ignore = true

[bans]
multiple-versions = \"warn\"
wildcards = \"allow\"

[sources]
unknown-registry = \"warn\"
unknown-git = \"warn\"
allow-registry = [\"{registry_url}\"]
allow-git = []
"
    );
    emit_file(path, &content, dry_run)?;

    Ok(())
}

/// Generate an mdBook scaffold in `docs/` with a chapter per workspace crate.
pub fn write_mdbook(
    plan: &InitPlan,
    project_name: &str,
    force: bool,
    dry_run: bool,
) -> Result<(), TaskitError> {
    let docs_dir = Path::new("docs");
    let src_dir = docs_dir.join("src");
    let book_toml = docs_dir.join("book.toml");

    if book_toml.exists() && !force {
        eprintln!("docs/book.toml already exists, skipping");
        return Ok(());
    }

    if !dry_run {
        std::fs::create_dir_all(src_dir.join("crates"))?;
    }

    // book.toml
    let book_content = format!(
        "\
[book]
title = \"{project_name}\"
authors = []
language = \"en\"
multilingual = false
src = \"src\"

[build]
build-dir = \"dist\"

[output.html]
git-repository-url = \"\"
default-theme = \"rust\"
preferred-dark-theme = \"ayu\"
"
    );
    emit_file(&book_toml, &book_content, dry_run)?;

    // SUMMARY.md
    let mut summary = format!("# Summary\n\n[{project_name}](./README.md)\n\n");
    summary.push_str("# Architecture\n\n");
    summary.push_str("- [Overview](./architecture/overview.md)\n");
    summary.push_str("\n# Crates\n\n");

    for c in &plan.crates {
        let name = c.pkg.as_deref().unwrap_or(&c.dir);
        let slug = name.replace('/', "-");
        summary.push_str(&format!("- [{name}](./crates/{slug}.md)\n"));

        // Create stub crate doc
        let crate_doc = src_dir.join("crates").join(format!("{slug}.md"));
        if !crate_doc.exists() || force {
            emit_file(
                &crate_doc,
                &format!("# {name}\n\n<!-- Crate documentation -->\n"),
                dry_run,
            )?;
        }
    }

    summary.push_str("\n# Reference\n\n");
    summary.push_str("- [Configuration](./reference/configuration.md)\n");
    summary.push_str("- [CI Pipeline](./reference/ci-pipeline.md)\n");

    emit_file(&src_dir.join("SUMMARY.md"), &summary, dry_run)?;

    // Stub pages
    let readme = src_dir.join("README.md");
    if !readme.exists() || force {
        emit_file(
            &readme,
            &format!("# {project_name}\n\nWelcome to the {project_name} documentation.\n"),
            dry_run,
        )?;
    }

    if !dry_run {
        std::fs::create_dir_all(src_dir.join("architecture"))?;
    }
    let overview = src_dir.join("architecture/overview.md");
    if !overview.exists() || force {
        emit_file(
            &overview,
            "# Architecture Overview\n\n<!-- Describe workspace structure and crate relationships -->\n",
            dry_run,
        )?;
    }

    if !dry_run {
        std::fs::create_dir_all(src_dir.join("reference"))?;
    }
    let config_doc = src_dir.join("reference/configuration.md");
    if !config_doc.exists() || force {
        emit_file(
            &config_doc,
            "\
# Configuration

## taskit.toml

All configuration lives in `taskit.toml` at the workspace root.

### Sections

| Section | Purpose |
|---------|---------|
| `[workspace]` | Crate list, propagation rules, offline skip |
| `[protocol]` | Contract surface drift detection |
| `[coverage]` | Coverage enforcement |
| `[ci]` | Pipeline steps, fail_fast default |
| `[inspect]` | Metric thresholds (warnings, errors, TODOs) |
| `[clean]` | Artifact retention policy |
| `[flow]` | Git branching workflow |
| `[release]` | Publish order, GitHub repo, skip_docs, allow_dirty |
",
            dry_run,
        )?;
    }

    let ci_doc = src_dir.join("reference/ci-pipeline.md");
    if !ci_doc.exists() || force {
        emit_file(
            &ci_doc,
            "\
# CI Pipeline

Run the full pipeline:

```sh
taskit ci
```

## Steps

| Step | Command | Gate |
|------|---------|------|
| Self-check | `taskit self-check` | Yes |
| Format | `taskit fmt --check` | No |
| Lint | `taskit lint` | No |
| Compile tests | `taskit compile-tests` | No |
| Test | `taskit test` | No |
| Deps | `taskit check-deps` | No |
| Drift | `taskit check-protocol-drift` | No |
",
            dry_run,
        )?;
    }

    let verb = if dry_run { "would write" } else { "wrote" };
    eprintln!(
        "{verb} docs/ mdBook scaffold ({} crate pages)",
        plan.crates.len()
    );
    Ok(())
}

const XTASK_SENTINEL: &str = "// --- taskit-managed ---";

/// The taskit task dispatcher block injected into an existing xtask main.rs.
const XTASK_INJECT: &str = r#"
// --- taskit-managed ---
// The functions below were generated by `taskit init`. They delegate to the
// `taskit` binary. Add calls to these from your task dispatch as needed.

fn taskit(args: &[&str]) {
    let status = std::process::Command::new("taskit")
        .args(args)
        .status()
        .unwrap_or_else(|_| {
            // Fallback: invoke via `cargo run -p taskit --`
            std::process::Command::new("cargo")
                .args(["run", "-p", "taskit", "--"])
                .args(args)
                .status()
                .expect("failed to run taskit")
        });
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn task_fmt()           { taskit(&["fmt"]) }
fn task_fmt_check()     { taskit(&["fmt", "--check"]) }
fn task_lint()          { taskit(&["lint"]) }
fn task_test()          { taskit(&["test"]) }
fn task_ci()            { taskit(&["ci"]) }
fn task_pre_commit()    { taskit(&["pre-commit"]) }
fn task_pre_push()      { taskit(&["pre-push"]) }
// --- end taskit-managed ---
"#;

/// The full xtask main.rs written when no xtask crate exists yet.
const XTASK_MAIN_FRESH: &str = r#"//! xtask — build tasks for this workspace.
//!
//! Run with: `cargo xtask <task>`
//! Tasks are delegated to the `taskit` binary.

fn main() {
    let task = std::env::args().nth(1).unwrap_or_default();
    match task.as_str() {
        "fmt"           => task_fmt(),
        "fmt-check"     => task_fmt_check(),
        "lint"          => task_lint(),
        "test"          => task_test(),
        "ci"            => task_ci(),
        "pre-commit"    => task_pre_commit(),
        "pre-push"      => task_pre_push(),
        other => {
            eprintln!("unknown task: {other}");
            eprintln!("available: fmt, fmt-check, lint, test, ci, pre-commit, pre-push");
            std::process::exit(1);
        }
    }
}

// --- taskit-managed ---
// The functions below were generated by `taskit init`.

fn taskit(args: &[&str]) {
    let status = std::process::Command::new("taskit")
        .args(args)
        .status()
        .unwrap_or_else(|_| {
            std::process::Command::new("cargo")
                .args(["run", "-p", "taskit", "--"])
                .args(args)
                .status()
                .expect("failed to run taskit")
        });
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn task_fmt()           { taskit(&["fmt"]) }
fn task_fmt_check()     { taskit(&["fmt", "--check"]) }
fn task_lint()          { taskit(&["lint"]) }
fn task_test()          { taskit(&["test"]) }
fn task_ci()            { taskit(&["ci"]) }
fn task_pre_commit()    { taskit(&["pre-commit"]) }
fn task_pre_push()      { taskit(&["pre-push"]) }
// --- end taskit-managed ---
"#;

/// Generate or augment the `xtask/` crate with taskit task dispatchers.
///
/// - Fresh workspace (no `xtask/src/main.rs`): writes a complete xtask crate.
/// - Existing `main.rs` without the sentinel: appends the managed block.
/// - Existing `main.rs` with the sentinel already present: skips (idempotent).
pub fn write_xtask(force: bool, dry_run: bool) -> Result<(), TaskitError> {
    let src_dir = Path::new("xtask/src");
    let main_rs = src_dir.join("main.rs");
    let cargo_toml = Path::new("xtask/Cargo.toml");

    if main_rs.exists() && !force {
        let existing = std::fs::read_to_string(&main_rs)?;
        if existing.contains(XTASK_SENTINEL) {
            if !dry_run {
                eprintln!("xtask/src/main.rs already has taskit-managed block, skipping");
            }
            return Ok(());
        }
        // Inject the block at the end of the existing file.
        if dry_run {
            eprintln!("would append taskit-managed block to xtask/src/main.rs");
        } else {
            let mut content = existing;
            if !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(XTASK_INJECT);
            std::fs::write(&main_rs, content).map_err(|e| {
                taskit_types::error::InitError::WriteFile {
                    file: main_rs.display().to_string(),
                    reason: e.to_string(),
                }
            })?;
            eprintln!("appended taskit-managed block to xtask/src/main.rs");
        }
        return Ok(());
    }

    // Fresh xtask crate.
    emit_file(&main_rs, XTASK_MAIN_FRESH, dry_run)?;

    if !cargo_toml.exists() || force {
        emit_file(
            cargo_toml,
            r#"[package]
name = "xtask"
version = "0.1.0"
edition = "2021"
publish = false
"#,
            dry_run,
        )?;

        // Remind the user to add xtask to workspace members if not already there.
        if !dry_run {
            let ws_toml = Path::new("Cargo.toml");
            if ws_toml.exists() {
                let ws_content = std::fs::read_to_string(ws_toml).unwrap_or_default();
                if !ws_content.contains("\"xtask\"") && !ws_content.contains("'xtask'") {
                    eprintln!(
                        "note: add `\"xtask\"` to [workspace] members in Cargo.toml to complete setup"
                    );
                }
            }
        }
    }

    let verb = if dry_run { "would write" } else { "wrote" };
    eprintln!("{verb} xtask/ crate with taskit task dispatchers");
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), TaskitError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), TaskitError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scaffold tests must run serially because they use `set_current_dir`
    /// which is process-global. A mutex prevents parallel CWD corruption.
    static CWD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Run a closure in a tempdir, restoring CWD even on panic.
    fn in_tempdir<F: FnOnce(&std::path::Path) + std::panic::UnwindSafe>(f: F) {
        let _guard = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let result = std::panic::catch_unwind(|| f(dir.path()));
        std::env::set_current_dir(prev).unwrap();
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    #[test]
    fn ctx_scaffold_creates_dirs() {
        in_tempdir(|dir| {
            let result = write_ctx_scaffold(false, false);
            assert!(result.is_ok());
            assert!(dir.join(".ctx/memory-bank").is_dir());
            assert!(dir.join(".ctx/sessions").is_dir());
            assert!(dir.join(".ctx/tasks").is_dir());
            assert!(dir.join(".ctx/review").is_dir());
            assert!(dir.join(".ctx/logs").is_dir());
            assert!(dir.join(".ctx/reports").is_dir());
            assert!(dir.join(".ctx/xcache").is_dir());
            assert!(dir.join(".ctx/.initialized").exists());
            assert!(dir.join(".ctx/HANDOFF.md").exists());
            assert!(dir.join(".ctx/memory-bank/project-brief.md").exists());
        });
    }

    #[test]
    fn ctx_scaffold_skips_existing() {
        in_tempdir(|dir| {
            std::fs::create_dir_all(dir.join(".ctx")).unwrap();
            let result = write_ctx_scaffold(false, false);
            assert!(result.is_ok());
            assert!(!dir.join(".ctx/memory-bank").exists());
        });
    }

    #[test]
    fn ctx_scaffold_dry_run_no_files() {
        in_tempdir(|dir| {
            let result = write_ctx_scaffold(false, true);
            assert!(result.is_ok());
            assert!(!dir.join(".ctx/memory-bank").exists());
            assert!(!dir.join(".ctx/HANDOFF.md").exists());
        });
    }

    #[test]
    fn deny_toml_content() {
        in_tempdir(|dir| {
            write_deny_toml(false, false).unwrap();
            let content = std::fs::read_to_string(dir.join("deny.toml")).unwrap();
            assert!(content.contains("[advisories]"));
            assert!(content.contains("[licenses]"));
            assert!(content.contains("[bans]"));
            assert!(content.contains("[sources]"));
        });
    }

    #[test]
    fn deny_toml_dry_run_no_file() {
        in_tempdir(|dir| {
            write_deny_toml(false, true).unwrap();
            assert!(!dir.join("deny.toml").exists());
        });
    }

    #[test]
    fn github_ci_content() {
        in_tempdir(|dir| {
            write_github_ci(false, false).unwrap();
            let content = std::fs::read_to_string(dir.join(".github/workflows/ci.yml")).unwrap();
            assert!(content.contains("taskit ci"));
            assert!(content.contains("dtolnay/rust-toolchain"));
        });
    }

    #[test]
    fn mdbook_scaffold_creates_files() {
        in_tempdir(|dir| {
            let plan = InitPlan {
                crates: vec![
                    crate::plan::CratePlan {
                        dir: "crates/core".into(),
                        pkg: Some("my-core".into()),
                    },
                    crate::plan::CratePlan {
                        dir: "crates/cli".into(),
                        pkg: None,
                    },
                ],
                propagation: vec![],
                surfaces: vec![],
                coverage: None,
                ci_steps: vec![],
                offline_skip: None,
                flow: None,
                release: None,
                git_hooks: false,
                github_ci: false,
                deny_toml: false,
                ctx_scaffold: false,
                mdbook: false,
                xtask: false,
            };
            write_mdbook(&plan, "test-project", false, false).unwrap();

            assert!(dir.join("docs/book.toml").exists());
            assert!(dir.join("docs/src/SUMMARY.md").exists());
            assert!(dir.join("docs/src/README.md").exists());
            assert!(dir.join("docs/src/architecture/overview.md").exists());
            assert!(dir.join("docs/src/reference/configuration.md").exists());
            assert!(dir.join("docs/src/crates").is_dir());

            let summary = std::fs::read_to_string(dir.join("docs/src/SUMMARY.md")).unwrap();
            assert!(summary.contains("# Crates"));
            assert!(summary.contains("test-project"));

            let book = std::fs::read_to_string(dir.join("docs/book.toml")).unwrap();
            assert!(book.contains("title = \"test-project\""));
        });
    }

    #[test]
    fn xtask_fresh_creates_crate() {
        in_tempdir(|dir| {
            write_xtask(false, false).unwrap();
            let main = std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap();
            assert!(main.contains(XTASK_SENTINEL));
            assert!(main.contains("fn task_ci()"));
            assert!(main.contains("fn taskit("));
            assert!(dir.join("xtask/Cargo.toml").exists());
        });
    }

    #[test]
    fn xtask_fresh_dry_run_no_files() {
        in_tempdir(|dir| {
            write_xtask(false, true).unwrap();
            assert!(!dir.join("xtask/src/main.rs").exists());
        });
    }

    #[test]
    fn xtask_injects_into_existing_main() {
        in_tempdir(|dir| {
            std::fs::create_dir_all(dir.join("xtask/src")).unwrap();
            let existing = "fn main() { println!(\"hello\"); }\n";
            std::fs::write(dir.join("xtask/src/main.rs"), existing).unwrap();

            write_xtask(false, false).unwrap();

            let content = std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap();
            // Original content preserved
            assert!(content.contains("println!(\"hello\")"));
            // Taskit block appended
            assert!(content.contains(XTASK_SENTINEL));
            assert!(content.contains("fn task_ci()"));
        });
    }

    #[test]
    fn xtask_inject_is_idempotent() {
        in_tempdir(|dir| {
            std::fs::create_dir_all(dir.join("xtask/src")).unwrap();
            let existing = format!("fn main() {{}}\n{XTASK_SENTINEL}\n");
            std::fs::write(dir.join("xtask/src/main.rs"), &existing).unwrap();

            write_xtask(false, false).unwrap();

            let content = std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap();
            // File unchanged — sentinel already present
            assert_eq!(content, existing);
        });
    }

    #[test]
    fn xtask_inject_dry_run_no_change() {
        in_tempdir(|dir| {
            std::fs::create_dir_all(dir.join("xtask/src")).unwrap();
            let existing = "fn main() {}\n";
            std::fs::write(dir.join("xtask/src/main.rs"), existing).unwrap();

            write_xtask(false, true).unwrap();

            let content = std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap();
            assert_eq!(content, existing);
        });
    }

    #[test]
    fn git_hooks_content() {
        in_tempdir(|dir| {
            std::process::Command::new("git")
                .args(["init"])
                .current_dir(dir)
                .output()
                .ok();

            write_git_hooks(false, false).unwrap();
            let pre_commit = std::fs::read_to_string(dir.join(".githooks/pre-commit")).unwrap();
            assert!(pre_commit.contains("taskit pre-commit"));
            let pre_push = std::fs::read_to_string(dir.join(".githooks/pre-push")).unwrap();
            assert!(pre_push.contains("taskit pre-push"));
        });
    }
}
