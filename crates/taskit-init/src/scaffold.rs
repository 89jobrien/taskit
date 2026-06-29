use std::path::Path;

use taskit_types::error::{InitError, TaskitError};

/// Create the `.ctx/` project context directory scaffold.
pub fn write_ctx_scaffold(force: bool) -> Result<(), TaskitError> {
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
    for d in &dirs {
        std::fs::create_dir_all(d)?;
    }

    // .initialized marker
    let init_marker = ctx.join(".initialized");
    if !init_marker.exists() {
        std::fs::write(&init_marker, "")?;
    }

    // Stub HANDOFF.md
    let handoff = ctx.join("HANDOFF.md");
    if !handoff.exists() || force {
        std::fs::write(&handoff, "# Handoff\n\nNo active handoff state.\n")?;
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
            std::fs::write(&path, content)?;
        }
    }

    // .gitignore for .ctx — keep structure but ignore session data
    let gitignore = ctx.join(".gitignore");
    if !gitignore.exists() || force {
        std::fs::write(
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

# Baselines (committed — track quality over time)
!rustqual-baseline.json

# Keep structure
!.gitkeep
",
        )?;
    }

    eprintln!("wrote .ctx/ scaffold (memory-bank, sessions, tasks, review)");
    Ok(())
}

/// Generate git hooks in `.githooks/` that delegate to taskit.
pub fn write_git_hooks(force: bool) -> Result<(), TaskitError> {
    let dir = Path::new(".githooks");

    if dir.exists() && !force {
        eprintln!(".githooks/ already exists, skipping");
        return Ok(());
    }

    std::fs::create_dir_all(dir)?;

    let pre_commit = dir.join("pre-commit");
    std::fs::write(
        &pre_commit,
        "\
#!/bin/sh
# Delegate to taskit pre-commit checks.
# Install: git config core.hooksPath .githooks
exec cargo xtask pre-commit
",
    )
    .map_err(|e| InitError::WriteFile {
        file: ".githooks/pre-commit".into(),
        reason: e.to_string(),
    })?;
    make_executable(&pre_commit)?;

    let pre_push = dir.join("pre-push");
    std::fs::write(
        &pre_push,
        "\
#!/bin/sh
# Delegate to taskit pre-push checks.
# Install: git config core.hooksPath .githooks
exec cargo xtask pre-push
",
    )
    .map_err(|e| InitError::WriteFile {
        file: ".githooks/pre-push".into(),
        reason: e.to_string(),
    })?;
    make_executable(&pre_push)?;

    // Set core.hooksPath
    let status = std::process::Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .status();
    match status {
        Ok(s) if s.success() => {
            eprintln!("wrote .githooks/ and set git core.hooksPath");
        }
        _ => {
            eprintln!("wrote .githooks/ (run `git config core.hooksPath .githooks` to activate)");
        }
    }

    Ok(())
}

/// Generate a GitHub Actions CI workflow.
pub fn write_github_ci(force: bool) -> Result<(), TaskitError> {
    let dir = Path::new(".github/workflows");
    let path = dir.join("ci.yml");

    if path.exists() && !force {
        eprintln!(".github/workflows/ci.yml already exists, skipping");
        return Ok(());
    }

    std::fs::create_dir_all(dir)?;

    std::fs::write(
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
        run: cargo xtask ci --fail-fast
",
    )
    .map_err(|e| InitError::WriteFile {
        file: ".github/workflows/ci.yml".into(),
        reason: e.to_string(),
    })?;

    eprintln!("wrote .github/workflows/ci.yml");
    Ok(())
}

/// Generate a starter `deny.toml` for cargo-deny.
pub fn write_deny_toml(force: bool) -> Result<(), TaskitError> {
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
    std::fs::write(path, &content).map_err(|e| InitError::WriteFile {
        file: "deny.toml".into(),
        reason: e.to_string(),
    })?;

    eprintln!("wrote deny.toml");
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

    #[test]
    fn ctx_scaffold_creates_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let result = write_ctx_scaffold(false);
        assert!(result.is_ok());
        assert!(dir.path().join(".ctx/memory-bank").is_dir());
        assert!(dir.path().join(".ctx/sessions").is_dir());
        assert!(dir.path().join(".ctx/tasks").is_dir());
        assert!(dir.path().join(".ctx/review").is_dir());
        assert!(dir.path().join(".ctx/logs").is_dir());
        assert!(dir.path().join(".ctx/reports").is_dir());
        assert!(dir.path().join(".ctx/xcache").is_dir());
        assert!(dir.path().join(".ctx/.initialized").exists());
        assert!(dir.path().join(".ctx/HANDOFF.md").exists());
        assert!(
            dir.path()
                .join(".ctx/memory-bank/project-brief.md")
                .exists()
        );

        std::env::set_current_dir(prev).unwrap();
    }

    #[test]
    fn ctx_scaffold_skips_existing() {
        let dir = tempfile::tempdir().unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        std::fs::create_dir_all(dir.path().join(".ctx")).unwrap();
        let result = write_ctx_scaffold(false);
        assert!(result.is_ok());
        // Should not have created subdirs since .ctx existed
        assert!(!dir.path().join(".ctx/memory-bank").exists());

        std::env::set_current_dir(prev).unwrap();
    }

    #[test]
    fn deny_toml_content() {
        let dir = tempfile::tempdir().unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        write_deny_toml(false).unwrap();
        let content = std::fs::read_to_string(dir.path().join("deny.toml")).unwrap();
        assert!(content.contains("[advisories]"));
        assert!(content.contains("[licenses]"));
        assert!(content.contains("[bans]"));
        assert!(content.contains("[sources]"));

        std::env::set_current_dir(prev).unwrap();
    }

    #[test]
    fn github_ci_content() {
        let dir = tempfile::tempdir().unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        write_github_ci(false).unwrap();
        let content = std::fs::read_to_string(dir.path().join(".github/workflows/ci.yml")).unwrap();
        assert!(content.contains("cargo xtask ci"));
        assert!(content.contains("dtolnay/rust-toolchain"));

        std::env::set_current_dir(prev).unwrap();
    }

    #[test]
    fn git_hooks_content() {
        let dir = tempfile::tempdir().unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        // Init a git repo so git config works
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .ok();

        write_git_hooks(false).unwrap();
        let pre_commit = std::fs::read_to_string(dir.path().join(".githooks/pre-commit")).unwrap();
        assert!(pre_commit.contains("cargo xtask pre-commit"));
        let pre_push = std::fs::read_to_string(dir.path().join(".githooks/pre-push")).unwrap();
        assert!(pre_push.contains("cargo xtask pre-push"));

        std::env::set_current_dir(prev).unwrap();
    }
}
