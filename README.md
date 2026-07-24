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
taskit fmt          # format all crates
taskit lint         # clippy on all crates
taskit test         # nextest on all crates
taskit ci           # full local CI pipeline
taskit quick        # fast feedback: fmt-check + lint + test (affected crates, offline)
```

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

| Command                                                                | Description                                         |
| ---------------------------------------------------------------------- | --------------------------------------------------- |
| `fmt [--check] [--affected]`                                           | Format (or check) Rust code                         |
| `lint [--crate-name X] [--affected] [--continue-on-error]`             | Run clippy                                          |
| `test [--crate-name X] [--affected] [--offline] [--continue-on-error]` | Run tests via nextest                               |
| `coverage [--crate-name X] [--threshold N]`                            | Coverage with threshold (default 80%)               |
| `compile-tests`                                                        | Compile test binaries without running them          |
| `check-deps`                                                           | Check for unused dependencies (cargo-udeps)         |
| `check-protocol-drift [--update] [--warn-only] [--hook]`               | Verify tracked file hashes                          |
| `check-protocol-sites --file F --pattern P --expected N`               | Count construction sites for key structs            |
| `check-freshness`                                                      | Verify protocol drift lockfile is up to date        |
| `quick`                                                                | Fast local feedback loop (affected crates, offline) |
| `ci [--fail-fast] [--include-network]`                                 | Full CI pipeline                                    |
| `pre-commit` / `pre-push`                                              | Git hook delegates                                  |
| `install-hooks`                                                        | Install git hooks                                   |
| `audit`                                                                | Run cargo-deny (advisories, licenses, bans)         |
| `clean [--older-than Nd]`                                              | Clean build artifacts                               |
| `health [--update]`                                                    | Measure codebase health, compare to baseline        |
| `inspect [--max-warnings N] [--max-todo N]`                            | Check workspace metrics against thresholds          |
| `publish [--skip-docs] [--allow-dirty]`                                | Generate docs and publish to crates.io              |
| `init [--force] [--interactive]`                                       | Generate taskit.toml, Cruxfile, .cargo/config.toml  |
| `version`                                                              | Show workspace crate versions                       |
| `dev-setup`                                                            | Install development tools                           |
| `self-check`                                                           | Verify required tools are installed                 |
| `self-test`                                                            | Run taskit's own test suite (hash-cached)           |
| `update-claude-version <version>`                                      | Update pinned Claude Code version                   |
| `proptest --crate-name X`                                              | Run property-based tests                            |
| `fuzz <target> [--duration N]`                                         | Run cargo-fuzz on a target                          |
| `bench [--crate-name X] [--save-baseline]`                             | Run criterion benchmarks                            |
| `test-report`                                                          | Generate unified coverage report                    |
| `snapshot-review`                                                      | Review pending insta snapshots                      |
| `flow status`                                                          | Show current branch / staging state                 |
| `flow promote`                                                         | Advance one stage: develop -> staging -> release -> main |
| `flow guard`                                                           | Assert branch invariants                            |
| `flow auto`                                                            | Full promote -> CI -> finish pipeline with LLM      |
|                                                                        | conflict resolution (BamlConflictResolver / BAML);  |
|                                                                        | escalates to human via `FlowError::NeedsHuman`      |

## Affected-crate detection

`taskit lint --affected` and `taskit test --affected` run only on crates changed since
`origin/main`. Add `[[workspace.propagation]]` entries to ensure that changing a shared
crate also triggers its dependents.

## Protocol drift

Track any set of files as contract surfaces. After initial setup:

```bash
taskit check-protocol-drift --update   # generate / update the lockfile
git add taskit-protocol.lock
```

Add `check-protocol-drift` to your CI pipeline. Any subsequent change to a tracked file
will fail CI until the lockfile is updated and committed.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE) at your option.
