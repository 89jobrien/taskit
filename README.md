# taskit

Config-driven CI pipeline runner with affected-crate detection, protocol-drift tracking,
and pipeline orchestration for Rust workspaces.

## Install

```bash
cargo install taskit
```

## Quick start

Run from the root of any Rust workspace:

```bash
taskit check fmt    # format all crates
taskit check lint   # clippy on all crates
taskit test run     # nextest on all crates
taskit check ci     # full local CI pipeline
taskit check quick  # fast feedback: fmt-check + lint + test (affected crates, offline)
```

Subcommands are grouped by category: `dev` (build/install/setup), `check` (fmt/lint/CI
gates), `test` (nextest/coverage/proptest/fuzz/bench), `health` (baselines/metrics),
`protocol` (contract drift/TODO sync/dependency audit), `release` (version bumps/publish),
`flow` (git branching), plus top-level `dashboard` and `init`. Run `taskit --help` or
`taskit <category> --help` to see each group.

Add `--dry-run` to any command to print the commands that would run without executing them.

## Configuration

Create `taskit.toml` at the workspace root to customise behaviour. Every field is optional —
taskit works without any configuration by discovering crates via `cargo metadata`.

```toml
[workspace]
# Ordered list of crates for affected-crate detection.
# 'dir' is the directory name relative to workspace root.
# 'pkg' is the Cargo package name (defaults to dir if omitted).
[[workspace.crates]]
dir = "my-common"

[[workspace.crates]]
dir = "my-api"

# If 'source' changes, 'dependents' are also treated as affected.
[[workspace.propagation]]
source = "my-common"
dependents = ["my-api", "my-cli"]

# Protocol-drift: files whose content hash is tracked in a lockfile.
# CI fails when any surface changes without updating the lock.
[[protocol.surfaces]]
name = "api-types"
path = "my-api/src/types.rs"

[[protocol.surfaces]]
name = "cli-commands"
path = "my-cli/src/commands/mod.rs"

[protocol]
lockfile = "taskit-protocol.lock"   # default

# Custom CI pipeline. When omitted, a built-in default pipeline is used.
[[ci.steps]]
name  = "fmt-check"
cmd   = "fmt --check"
gate  = true            # blocks subsequent steps on failure

[[ci.steps]]
name = "lint"
cmd  = "lint"

[[ci.steps]]
name = "test"
cmd  = "test"

[[ci.steps]]
name = "protocol-drift"
cmd  = "check-protocol-drift"
```

## Subcommands

Subcommands are grouped into categories: `dev`, `check`, `test`, `health`, `protocol`,
`release`, `flow`, plus top-level `dashboard` and `init`. Run `taskit <category> --help`
for the full flag list of any group.

### `dev` — build, install, workspace setup

| Command                                       | Description                                          |
| ---------------------------------------------- | ----------------------------------------------------- |
| `dev build [--release]`                        | Build the workspace (cargo build --workspace)        |
| `dev install`                                  | Install the taskit binary (cargo install --path .)   |
| `dev bootstrap`                                | Install git hooks and dev tools (workspace setup)    |
| `dev install-hooks`                            | Install git hooks that delegate to taskit            |
| `dev setup`                                    | Install development tools                            |
| `dev self-check`                               | Verify required tools are installed                  |
| `dev clean [--older-than Nd]`                  | Clean build artifacts                                |
| `dev update [--aggressive]`                    | Update Cargo.lock dependencies                       |
| `dev update-claude-version <version>`          | Update pinned Claude Code version                    |

### `check` — quality gates

| Command                                                                | Description                                         |
| ---------------------------------------------------------------------- | --------------------------------------------------- |
| `check fmt [--check] [--affected]`                                     | Format (or check) Rust code                         |
| `check lint [--crate-name X] [--affected] [--continue-on-error] [--fix]` | Run clippy (`--fix` auto-applies suggestions)      |
| `check quick`                                                          | Fast local feedback loop (affected crates, offline) |
| `check ci [--fail-fast] [--include-network]`                           | Full CI pipeline                                    |
| `check compile`                                                       | Compile test binaries without running them          |
| `check deps`                                                          | Check for unused dependencies (cargo-udeps)         |
| `check pre-commit` / `check pre-push`                                  | Git hook delegates                                  |
| `check self-test`                                                     | Run taskit's own test suite (hash-cached)           |

### `test` — extended testing

| Command                                                                | Description                                         |
| ---------------------------------------------------------------------- | --------------------------------------------------- |
| `test run [--crate-name X] [--affected] [--offline] [--continue-on-error]` | Run tests via nextest                          |
| `test coverage [--crate-name X] [--threshold N] [--workspace]`        | Coverage with threshold (default 80%)               |
| `test proptest --crate-name X`                                        | Run property-based tests                            |
| `test fuzz <target> [--duration N]`                                   | Run cargo-fuzz on a target                          |
| `test bench [--crate-name X] [--save-baseline]`                       | Run criterion benchmarks                            |
| `test report`                                                         | Generate unified coverage report                    |
| `test snapshots`                                                      | Review pending insta snapshots                      |

### `health` — codebase health and metrics

| Command                                                | Description                                   |
| ------------------------------------------------------- | ---------------------------------------------- |
| `health check [--update] [--with-coverage] [--gate]`    | Measure codebase health, compare to baseline  |
| `health drift --metric M [--window N]`                  | Compare a telemetry metric vs. its baseline   |
| `health inspect [--max-warnings N] [--max-todo N]`      | Check workspace metrics against thresholds    |
| `health version`                                        | Show workspace crate versions                 |

### `protocol` — contract drift, TODO sync, governance

| Command                                                                    | Description                                    |
| ---------------------------------------------------------------------------- | ------------------------------------------- |
| `protocol drift [--update] [--warn-only] [--hook] [--watch [--interval SECS]]` | Verify tracked contract-surface hashes    |
| `protocol sites --file F --pattern P --expected N`                          | Count construction sites for key structs    |
| `protocol todo-sync [--update] [--warn-only]`                               | Scan TODO/FIXME markers, sync to GitHub issues |
| `protocol freshness [--warn-only]`                                          | Check workspace dependency freshness (cargo-outdated) |
| `protocol audit`                                                            | Run cargo-deny (advisories, licenses, bans) |

### `release` — version bumps and publishing

| Command                                       | Description                                          |
| ---------------------------------------------- | ----------------------------------------------------- |
| `release patch` / `minor` / `major`            | Bump version across all workspace Cargo.toml files    |
| `release publish [--skip-docs] [--allow-dirty]` | Generate docs and publish to crates.io               |
| `release create <tag> [--notes-file F]`        | Create a GitHub release for a tagged version           |

### `flow` — git branching workflow

| Command        | Description                                                   |
| -------------- | --------------------------------------------------------------- |
| `flow status`  | Show current branch / staging state                            |
| `flow promote` | Advance one stage: develop -> staging -> release -> main       |
| `flow guard`   | Assert branch invariants                                       |
| `flow auto`    | Full promote -> CI -> finish pipeline with LLM conflict resolution (BamlConflictResolver / BAML); escalates to human via `FlowError::NeedsHuman` |

### top-level

| Command                                | Description                                          |
| ---------------------------------------- | ----------------------------------------------------- |
| `init [--force] [--interactive]`         | Generate taskit.toml, Cruxfile, .cargo/config.toml    |
| `dashboard`                              | Live terminal dashboard: health, CI telemetry, drift  |

## Affected-crate detection

`taskit check lint --affected` and `taskit test run --affected` run only on crates changed since
`origin/main`. Add `[[workspace.propagation]]` entries to ensure that changing a shared
crate also triggers its dependents.

## Protocol drift

Track any set of files as contract surfaces. After initial setup:

```bash
taskit protocol drift --update   # generate / update the lockfile
git add taskit-protocol.lock
```

Add `check-protocol-drift` to your CI pipeline. Any subsequent change to a tracked file
will fail CI until the lockfile is updated and committed.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE) at your option.
