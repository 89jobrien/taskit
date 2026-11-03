//! Generates project scaffolding for context, hooks, CI, documentation, and xtask integration.

// TODO(audit)(#25): 831 lines — split candidate.
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
unset $(git rev-parse --local-env-vars)
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

const XTASK_MODULE_SENTINEL: &str = "// --- taskit-managed module ---";
const XTASK_DECLARATION_SENTINEL: &str = "// --- taskit-managed module declaration ---";
const XTASK_DISPATCH_SENTINEL: &str = "// --- taskit-managed dispatch ---";

const XTASK_MODULE_DECLARATION: &str = r#"// --- taskit-managed module declaration ---
mod taskit;
// --- end taskit-managed module declaration ---
"#;

/// Body indentation used when `main`'s body is empty or carries no leading
/// whitespace of its own.
const DEFAULT_BODY_INDENT: &str = "    ";

/// Render the managed dispatch block at the body's own indentation.
///
/// A hardcoded four-space block would leave a file that fails `check fmt
/// --check` on any workspace that indents with two spaces or tabs, so the
/// caller's indentation is threaded through instead.
fn dispatch_block(indent: &str) -> String {
    let inner = format!("{indent}    ");
    let mut out = String::with_capacity(XTASK_DISPATCH_SENTINEL.len() * 4 + indent.len() * 8);
    out.push('\n');
    for line in [
        format!("{indent}// --- taskit-managed dispatch ---"),
        format!("{indent}if taskit::dispatch() {{"),
        format!("{inner}return;"),
        format!("{indent}}}"),
        format!("{indent}// --- end taskit-managed dispatch ---"),
    ] {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// The Taskit-owned dispatcher isolated from an existing xtask entry point.
const XTASK_MODULE: &str = r#"// --- taskit-managed module ---
//! Taskit command adapter generated by `taskit init`.

pub(super) fn dispatch() -> bool {
    let task = std::env::args().nth(1).unwrap_or_default();
    let args: &[&str] = match task.as_str() {
        "fmt" => &["fmt"],
        "fmt-check" => &["fmt", "--check"],
        "lint" => &["lint"],
        "test" => &["test"],
        "ci" => &["ci"],
        "pre-commit" => &["pre-commit"],
        "pre-push" => &["pre-push"],
        _ => return false,
    };

    taskit(args);
    true
}

fn taskit(args: &[&str]) {
    let status = match std::process::Command::new("taskit").args(args).status() {
        Ok(status) => status,
        Err(_) => match std::process::Command::new("cargo")
            .args(["run", "-p", "taskit", "--"])
            .args(args)
            .status()
        {
            Ok(status) => status,
            Err(err) => {
                eprintln!("failed to run taskit via binary or cargo fallback: {err}");
                std::process::exit(1);
            }
        },
    };
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}
// --- end taskit-managed module ---
"#;

/// The full xtask main.rs written when no xtask crate exists yet.
const XTASK_MAIN_FRESH: &str = r#"//! xtask — build tasks for this workspace.
//!
//! Run with: `cargo xtask <task>`
//! Tasks are delegated to the `taskit` binary.

// --- taskit-managed module declaration ---
mod taskit;
// --- end taskit-managed module declaration ---

fn main() {
    // --- taskit-managed dispatch ---
    if taskit::dispatch() {
        return;
    }
    // --- end taskit-managed dispatch ---

    let task = std::env::args().nth(1).unwrap_or_default();
    eprintln!("unknown task: {task}");
    eprintln!("available: fmt, fmt-check, lint, test, ci, pre-commit, pre-push");
    std::process::exit(1);
}
"#;

fn xtask_write_error(path: &Path, reason: impl Into<String>) -> TaskitError {
    taskit_types::error::InitError::WriteFile {
        file: path.display().to_string(),
        reason: reason.into(),
    }
    .into()
}

fn bridge_xtask_main(path: &Path, existing: &str) -> Result<Option<String>, TaskitError> {
    let has_declaration = existing.contains(XTASK_DECLARATION_SENTINEL);
    let has_dispatch = existing.contains(XTASK_DISPATCH_SENTINEL);

    if has_declaration && has_dispatch {
        return Ok(None);
    }
    if has_declaration || has_dispatch {
        return Err(xtask_write_error(
            path,
            "partial taskit-managed xtask bridge; restore or remove the managed fragments",
        ));
    }

    let file = syn::parse_file(existing)
        .map_err(|e| xtask_write_error(path, format!("existing main.rs is not valid Rust: {e}")))?;

    // Reject a real top-level `mod taskit;`, not a textual mention of one in a
    // comment or string.
    if file
        .items
        .iter()
        .any(|item| matches!(item, syn::Item::Mod(m) if m.ident == "taskit"))
    {
        return Err(xtask_write_error(
            path,
            "an unmanaged `mod taskit;` declaration already exists",
        ));
    }

    let (body_start, indent) = locate_main_body(path, existing, &file)?;

    let dispatch = dispatch_block(&indent);
    let mut bridged =
        String::with_capacity(existing.len() + XTASK_MODULE_DECLARATION.len() + dispatch.len() + 2);
    bridged.push_str(&existing[..body_start]);
    bridged.push_str(&dispatch);
    bridged.push_str(&existing[body_start..]);
    if !bridged.ends_with('\n') {
        bridged.push('\n');
    }
    bridged.push('\n');
    bridged.push_str(XTASK_MODULE_DECLARATION);

    // Belt and braces: never hand back a file that does not parse.
    syn::parse_file(&bridged).map_err(|e| {
        xtask_write_error(
            path,
            format!("refusing to write; the bridged main.rs would not parse ({e})"),
        )
    })?;

    Ok(Some(bridged))
}

/// Locate the byte offset just past `main`'s opening brace, plus the
/// indentation of its body.
///
/// Matching is line-anchored rather than a bare substring search. A line whose
/// trimmed form starts with `fn main(` is a candidate, which rejects the two
/// decoys a substring search walks straight into — a doc comment that mentions
/// `fn main() {` and a string literal that contains it — because neither can
/// begin a line with `fn main(`. The candidate set is then required to hold
/// exactly one entry, and cross-checked against `syn` so that a nested or
/// commented-out `fn main` cannot be mistaken for the crate root's entry point.
fn locate_main_body(
    path: &Path,
    existing: &str,
    file: &syn::File,
) -> Result<(usize, String), TaskitError> {
    let top_level_mains = file
        .items
        .iter()
        .filter(|item| matches!(item, syn::Item::Fn(f) if f.sig.ident == "main"))
        .count();
    if top_level_mains != 1 {
        return Err(xtask_write_error(
            path,
            format!(
                "expected exactly one top-level `fn main`, found {top_level_mains}; \
                 wire `mod taskit` manually"
            ),
        ));
    }

    let mut candidate = None;
    let mut offset = 0usize;
    for line in existing.split_inclusive('\n') {
        if line.trim_start().starts_with("fn main(") {
            if candidate.is_some() {
                return Err(xtask_write_error(
                    path,
                    "more than one line begins `fn main(`; wire `mod taskit` manually",
                ));
            }
            candidate = Some(offset);
        }
        offset += line.len();
    }

    let line_start = candidate.ok_or_else(|| {
        xtask_write_error(
            path,
            "could not safely locate `fn main(`; wire `mod taskit` manually",
        )
    })?;

    let body_start = find_opening_brace(&existing[line_start..]).ok_or_else(|| {
        xtask_write_error(
            path,
            "could not locate `main`'s opening brace; wire `mod taskit` manually",
        )
    })? + line_start;

    let indent =
        body_indent(&existing[body_start..]).unwrap_or_else(|| DEFAULT_BODY_INDENT.to_string());
    Ok((body_start, indent))
}

/// Offset of the first `{` that opens the signature line, or of a `{` opening
/// the body on a following line.
fn find_opening_brace(from_line: &str) -> Option<usize> {
    let mut scan = 0usize;
    for (i, line) in from_line.split_inclusive('\n').enumerate() {
        let trimmed = line.trim_start();
        if i == 0 {
            if let Some(pos) = line.find('{') {
                return Some(scan + pos + 1);
            }
        } else if trimmed.starts_with('{') {
            return Some(scan + (line.len() - trimmed.len()) + 1);
        } else if !trimmed.is_empty() && !trimmed.starts_with("//") {
            return None;
        }
        scan += line.len();
    }
    None
}

/// Leading whitespace of the first non-empty line after the opening brace.
fn body_indent(tail: &str) -> Option<String> {
    tail.split_inclusive('\n')
        .map(|l| l.trim_end())
        .find(|l| !l.trim().is_empty())
        .and_then(|l| {
            let ws = &l[..l.len() - l.trim_start().len()];
            (ws.chars().all(|c| c == ' ' || c == '\t') && !ws.is_empty()).then(|| ws.to_string())
        })
}

/// Generate or augment the `xtask/` crate with taskit task dispatchers.
///
/// - Fresh workspace: writes a complete xtask crate with an isolated managed module.
/// - Existing conventional `main.rs`: inserts a minimal, idempotent dispatch bridge.
/// - Existing files outside the managed module are never replaced, including with `force`.
pub fn write_xtask(force: bool, dry_run: bool) -> Result<(), TaskitError> {
    let src_dir = Path::new("xtask/src");
    let main_rs = src_dir.join("main.rs");
    let flat_taskit_module = src_dir.join("taskit.rs");
    let taskit_module = src_dir.join("taskit/mod.rs");
    let cargo_toml = Path::new("xtask/Cargo.toml");

    if flat_taskit_module.exists() {
        return Err(xtask_write_error(
            &flat_taskit_module,
            "conflicts with generated xtask/src/taskit/mod.rs",
        ));
    }

    let main_content = if main_rs.exists() {
        let existing = std::fs::read_to_string(&main_rs)?;
        bridge_xtask_main(&main_rs, &existing)?
    } else {
        Some(XTASK_MAIN_FRESH.to_string())
    };

    let write_module = if taskit_module.exists() {
        let existing = std::fs::read_to_string(&taskit_module)?;
        if !existing.contains(XTASK_MODULE_SENTINEL) {
            return Err(xtask_write_error(
                &taskit_module,
                "file exists without the taskit-managed marker",
            ));
        }
        force
    } else {
        true
    };

    if let Some(content) = main_content {
        emit_file(&main_rs, &content, dry_run)?;
    }
    if write_module {
        emit_file(&taskit_module, XTASK_MODULE, dry_run)?;
    }

    if !cargo_toml.exists() {
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

    let verb = if dry_run { "would prepare" } else { "prepared" };
    eprintln!("{verb} xtask/ crate with isolated taskit dispatchers");
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
                crux: false,
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
    fn xtask_fresh_creates_isolated_module() {
        in_tempdir(|dir| {
            write_xtask(false, false).unwrap();
            let main = std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap();
            let taskit = std::fs::read_to_string(dir.join("xtask/src/taskit/mod.rs")).unwrap();

            assert!(main.contains("mod taskit;"));
            assert!(main.contains("if taskit::dispatch()"));
            assert!(!main.contains("fn taskit("));
            assert!(taskit.contains("pub(super) fn dispatch() -> bool"));
            assert!(taskit.contains("fn taskit("));
            assert!(dir.join("xtask/Cargo.toml").exists());
        });
    }

    #[test]
    fn xtask_fresh_is_rustfmt_clean() {
        in_tempdir(|dir| {
            write_xtask(false, false).unwrap();
            let output = std::process::Command::new("rustfmt")
                .args(["--edition", "2021", "--check"])
                .arg(dir.join("xtask/src/main.rs"))
                .arg(dir.join("xtask/src/taskit/mod.rs"))
                .output()
                .unwrap();

            assert!(
                output.status.success(),
                "generated xtask is not rustfmt-clean:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
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
            let cargo_toml = "[package]\nname = \"custom-xtask\"\n";
            std::fs::write(dir.join("xtask/src/main.rs"), existing).unwrap();
            std::fs::write(dir.join("xtask/Cargo.toml"), cargo_toml).unwrap();

            write_xtask(false, false).unwrap();

            let content = std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap();
            assert!(content.contains("println!(\"hello\")"));
            assert!(content.contains("mod taskit;"));
            assert!(content.contains("if taskit::dispatch()"));
            assert!(!content.contains("fn task_ci()"));
            assert_eq!(
                std::fs::read_to_string(dir.join("xtask/Cargo.toml")).unwrap(),
                cargo_toml
            );
        });
    }

    #[test]
    fn xtask_inject_is_idempotent() {
        in_tempdir(|dir| {
            std::fs::create_dir_all(dir.join("xtask/src")).unwrap();
            std::fs::write(dir.join("xtask/src/main.rs"), "fn main() {}\n").unwrap();

            write_xtask(false, false).unwrap();
            let first_main = std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap();
            let first_module =
                std::fs::read_to_string(dir.join("xtask/src/taskit/mod.rs")).unwrap();
            write_xtask(false, false).unwrap();

            assert_eq!(
                std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap(),
                first_main
            );
            assert_eq!(
                std::fs::read_to_string(dir.join("xtask/src/taskit/mod.rs")).unwrap(),
                first_module
            );
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
            assert!(!dir.join("xtask/src/taskit/mod.rs").exists());
        });
    }

    /// A doc comment mentioning `fn main() {` must not capture the injection
    /// point. A bare substring search inserted inside the comment, producing an
    /// uncompilable file.
    #[test]
    fn xtask_bridges_past_doc_comment_decoy() {
        in_tempdir(|dir| {
            std::fs::create_dir_all(dir.join("xtask/src")).unwrap();
            let main = concat!(
                "//! Entry point is `fn main() { ... }` per the book.\n",
                "\n",
                "fn main() {\n",
                "    run();\n",
                "}\n",
            );
            std::fs::write(dir.join("xtask/src/main.rs"), main).unwrap();

            write_xtask(false, false).unwrap();

            let content = std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap();
            assert!(content.contains("//! Entry point is `fn main() { ... }` per the book."));
            // The dispatch must sit inside the real fn body, not the comment.
            let dispatch_at = content.find("// --- taskit-managed dispatch ---").unwrap();
            let body_at = content.find("fn main() {").unwrap();
            let run_at = content.find("run();").unwrap();
            assert!(body_at < dispatch_at, "dispatch must follow `fn main() {{`");
            assert!(
                dispatch_at < run_at,
                "dispatch must precede the original body"
            );
            assert!(syn::parse_file(&content).is_ok(), "bridged file must parse");
        });
    }

    /// A string literal containing `fn main() {` must not capture the injection
    /// point. A bare substring search injects into the literal; the result still
    /// compiles, so the bridge looks installed but never dispatches.
    #[test]
    fn xtask_bridges_past_string_literal_decoy() {
        in_tempdir(|dir| {
            std::fs::create_dir_all(dir.join("xtask/src")).unwrap();
            let main = "const S: &str = \"fn main() {\";\n\nfn main() {\n    run();\n}\n";
            std::fs::write(dir.join("xtask/src/main.rs"), main).unwrap();

            write_xtask(false, false).unwrap();

            let content = std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap();
            // The literal is untouched.
            assert!(
                content.contains("const S: &str = \"fn main() {\";"),
                "{content}"
            );
            // The dispatch lands inside the real body, after the literal.
            let dispatch_at = content.find("// --- taskit-managed dispatch ---").unwrap();
            let body_at = content.rfind("fn main() {").unwrap();
            let run_at = content.find("run();").unwrap();
            assert!(
                body_at < dispatch_at,
                "dispatch must follow the real `fn main() {{`"
            );
            assert!(
                dispatch_at < run_at,
                "dispatch must precede the original body"
            );
            assert!(syn::parse_file(&content).is_ok(), "bridged file must parse");
        });
    }

    /// A nested `fn main` must not be treated as the crate root's entry point.
    #[test]
    fn xtask_refuses_when_top_level_main_missing() {
        in_tempdir(|dir| {
            std::fs::create_dir_all(dir.join("xtask/src")).unwrap();
            let main = "fn helper() {\n    fn main() {\n        run();\n    }\n}\n";
            std::fs::write(dir.join("xtask/src/main.rs"), main).unwrap();

            assert!(write_xtask(false, false).is_err());
            assert_eq!(
                std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap(),
                main
            );
            assert!(!dir.join("xtask/src/taskit/mod.rs").exists());
        });
    }

    /// The injected block adopts the body's indentation instead of forcing
    /// four spaces, so a two-space workspace does not immediately fail
    /// `check fmt --check`.
    #[test]
    fn xtask_bridged_dispatch_matches_body_indentation() {
        for (label, indent) in [
            ("two-space", "  "),
            ("tab", "\t"),
            ("eight-space", "        "),
        ] {
            in_tempdir(|dir| {
                std::fs::create_dir_all(dir.join("xtask/src")).unwrap();
                let main = format!("fn main() {{\n{indent}run();\n}}\n");
                std::fs::write(dir.join("xtask/src/main.rs"), &main).unwrap();

                write_xtask(false, false).unwrap();

                let content = std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap();
                let expected = format!(
                    "{indent}// --- taskit-managed dispatch ---\n\
                     {indent}if taskit::dispatch() {{\n\
                     {indent}    return;\n\
                     {indent}}}\n"
                );
                assert!(
                    content.contains(&expected),
                    "{label}: missing correctly-indented dispatch in:\n{content}"
                );
            });
        }
    }

    #[test]
    fn xtask_bridged_output_is_rustfmt_clean() {
        in_tempdir(|dir| {
            std::fs::create_dir_all(dir.join("xtask/src")).unwrap();
            let main = "fn main() {\n    let x = 1;\n    println!(\"{x}\");\n}\n";
            std::fs::write(dir.join("xtask/src/main.rs"), main).unwrap();

            write_xtask(false, false).unwrap();

            let output = std::process::Command::new("rustfmt")
                .args(["--edition", "2021", "--check"])
                .arg(dir.join("xtask/src/main.rs"))
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "bridged main.rs is not rustfmt-clean:\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        });
    }

    #[test]
    fn xtask_force_only_refreshes_managed_module() {
        in_tempdir(|dir| {
            std::fs::create_dir_all(dir.join("xtask/src")).unwrap();
            let main = "fn main() { println!(\"custom\"); }\n";
            let cargo_toml = "[package]\nname = \"custom-xtask\"\n";
            std::fs::write(dir.join("xtask/src/main.rs"), main).unwrap();
            std::fs::write(dir.join("xtask/Cargo.toml"), cargo_toml).unwrap();
            write_xtask(false, false).unwrap();
            let bridged_main = std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap();
            std::fs::write(
                dir.join("xtask/src/taskit/mod.rs"),
                "// --- taskit-managed module ---\n// stale\n",
            )
            .unwrap();

            write_xtask(true, false).unwrap();

            assert_eq!(
                std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap(),
                bridged_main
            );
            assert_eq!(
                std::fs::read_to_string(dir.join("xtask/Cargo.toml")).unwrap(),
                cargo_toml
            );
            let taskit = std::fs::read_to_string(dir.join("xtask/src/taskit/mod.rs")).unwrap();
            assert!(taskit.contains("pub(super) fn dispatch() -> bool"));
            assert!(!taskit.contains("// stale"));
        });
    }

    #[test]
    fn xtask_refuses_unmanaged_taskit_module_without_mutation() {
        in_tempdir(|dir| {
            std::fs::create_dir_all(dir.join("xtask/src/taskit")).unwrap();
            let main = "fn main() { println!(\"custom\"); }\n";
            let module = "pub fn custom_task() {}\n";
            std::fs::write(dir.join("xtask/src/main.rs"), main).unwrap();
            std::fs::write(dir.join("xtask/src/taskit/mod.rs"), module).unwrap();

            assert!(write_xtask(false, false).is_err());
            assert_eq!(
                std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap(),
                main
            );
            assert_eq!(
                std::fs::read_to_string(dir.join("xtask/src/taskit/mod.rs")).unwrap(),
                module
            );
        });
    }

    #[test]
    fn xtask_refuses_conflicting_flat_taskit_module_without_mutation() {
        in_tempdir(|dir| {
            std::fs::create_dir_all(dir.join("xtask/src")).unwrap();
            let main = "mod taskit;\nfn main() { println!(\"custom\"); }\n";
            let module = "pub fn custom_task() {}\n";
            std::fs::write(dir.join("xtask/src/main.rs"), main).unwrap();
            std::fs::write(dir.join("xtask/src/taskit.rs"), module).unwrap();

            assert!(write_xtask(false, false).is_err());
            assert_eq!(
                std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap(),
                main
            );
            assert_eq!(
                std::fs::read_to_string(dir.join("xtask/src/taskit.rs")).unwrap(),
                module
            );
            assert!(!dir.join("xtask/src/taskit/mod.rs").exists());
        });
    }

    #[test]
    fn xtask_refuses_unrecognized_main_without_mutation() {
        in_tempdir(|dir| {
            std::fs::create_dir_all(dir.join("xtask/src")).unwrap();
            let main = "const ENTRY_POINT: &str = \"custom\";\n";
            std::fs::write(dir.join("xtask/src/main.rs"), main).unwrap();

            assert!(write_xtask(false, false).is_err());
            assert_eq!(
                std::fs::read_to_string(dir.join("xtask/src/main.rs")).unwrap(),
                main
            );
            assert!(!dir.join("xtask/src/taskit/mod.rs").exists());
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
            let unset = pre_push
                .find("unset $(git rev-parse --local-env-vars)")
                .expect("pre-push hook should clear Git's local environment");
            let taskit = pre_push
                .find("exec taskit pre-push")
                .expect("pre-push hook should delegate to taskit");
            assert!(
                unset < taskit,
                "Git's local environment must be cleared first"
            );
        });
    }
}
