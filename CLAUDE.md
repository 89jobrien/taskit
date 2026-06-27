# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Context

`taskit` is being extracted from the `xtask` crate for the **Maestro** workspace into a
standalone, config-driven Rust binary for any Rust workspace. See `DESIGN.md` for the full
plan. During the transition the code still reflects the maestro-internal origin; `DESIGN.md`
is the authoritative source for what the published crate will look like.

## Build & Run

```bash
# Run any xtask subcommand from the workspace root
cargo xtask <subcommand>

# Fast local feedback (fmt-check + lint + compile-tests + test, affected only, offline)
cargo xtask quick

# Full local CI pipeline (mirrors GitHub Actions)
cargo xtask ci

# Fail on first error instead of collecting all results
cargo xtask ci --fail-fast

# Include network-dependent tests (excluded by default)
cargo xtask ci --include-network

# Dry-run any command (prints commands without executing)
cargo xtask --dry-run <subcommand>
```

## Common Subcommands

| Command                                                  | Purpose                                            |
| -------------------------------------------------------- | -------------------------------------------------- |
| `fmt [--check] [--affected]`                             | Format (or check) all Rust code                    |
| `lint [--crate-name X] [--affected]`                     | Run clippy                                         |
| `test [--crate-name X] [--affected] [--offline]`         | Run tests via nextest                              |
| `coverage [--crate-name X]`                              | Coverage with 80% threshold (default: maestro-api) |
| `compile-tests`                                          | Compile test binaries without running              |
| `check-deps`                                             | Check for unused dependencies                      |
| `check-protocol-drift [--update] [--warn-only] [--hook]` | Verify core contract hashes                        |
| `check-freshness`                                        | Verify schema + protocol drift lockfile freshness  |
| `pre-commit` / `pre-push`                                | Git hook delegates                                 |
| `install-hooks`                                          | Install git hooks                                  |
| `audit`                                                  | Run cargo-deny (advisories, licenses, bans)        |
| `schema [--check]`                                       | Dump or verify GraphQL schema                      |
| `self-test`                                              | Run xtask's own test suite (hash-cached)           |

## Architecture

### Module Map

- **`main.rs`** — CLI parsing (clap derive) and dispatch to module functions
- **`runner.rs`** — `xrun()`/`xrun_ok()` wrappers around xshell; global `--dry-run` and
  `--silent` flags via atomics
- **`step.rs`** — `Pipeline` builder: chains steps/gates with fail-fast logic; prints summary
  table. Gates block all subsequent steps on failure regardless of `fail_fast`
- **`affected.rs`** — Detects changed crates from `git diff origin/main...HEAD`; propagates
  changes through a static dependency table (`maestro-common` → all dependents, etc.)
- **`protocol/drift.rs`** — Hashes 6 core contract surfaces (GraphQL schema, session types,
  K8s spec, config types, CLI commands, runtime API) into `xtask/protocol-drift.lock`;
  fails CI on unacknowledged drift. Also runs as a Claude Code PostToolUse hook.
- **`protocol/contract_hash.rs`** — SHA-256 normalization for drift detection
- **`cache/mod.rs`** — Hash-based skip cache (used by `self-test`)
- **`ci.rs`** — Full pipeline: self-check → fmt → lint → compile-tests → test → coverage →
  schema → check-deps → protocol-drift → protocol-sites
- **`quick.rs`** — Reduced pipeline for local feedback with progress spinners (silent mode)
- **`testing/`** — Submodules: `run`, `compile`, `coverage`, `conformance`, `docker`, `k8s`,
  `smolvm`, `proptest`, `fuzz`, `bench`, `report`, `snapshot`, `smoke`, `self_test`

### Affected Crate Detection

`affected.rs` hardcodes all known crates (`ALL_CRATES`) and a propagation table
(`PROPAGATION`). When `maestro-common` changes, all dependents are automatically included.
Single-pass expansion — no transitive chains. If transitive relationships are added, replace
`apply_propagation` with a fixpoint loop.

Crate dir → package name remaps: `maestro-cli` → `maestro`, `e2e` → `maestro-e2e`.

### Protocol Drift

`protocol-drift.lock` at the workspace root tracks SHA-256 hashes of 6 contract surfaces.
When any surface file changes without updating the lock:

- `cargo xtask check-protocol-drift` fails CI
- `cargo xtask check-protocol-drift --update` regenerates the lockfile

The `--hook` flag reads a file path from Claude Code hook stdin JSON to short-circuit when
the edited file is not a tracked surface.

## Testing

```bash
# Run xtask's own unit tests
cargo test -p xtask

# Run a single test by name
cargo nextest run -p xtask --test-threads 1 -E 'test(pipeline_fail_fast)'
```

Tests are colocated in each module under `#[cfg(test)]`. The `self-test` subcommand wraps
this with a hash cache so it skips when xtask source is unchanged.
