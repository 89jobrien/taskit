# CLI Commands

## Global form

```text
taskit [OPTIONS] <COMMAND>
```

| Option              | Values    | Purpose                                    |
| ------------------- | --------- | ------------------------------------------ |
| `--dry-run`         | flag      | Request command-specific dry-run behavior. |
| `--output <FORMAT>` | see below | Select report formatting.                  |
| `-h`, `--help`      | flag      | Print command help.                        |

Output values are `human`, `compact`, `json`, `github`, `junit`, `diagnostic`, and `sarif`.
Dry-run support is command-specific. Health still collects and `health check --update` still writes
its baseline; `dev clean` still removes `target/taskit/`; `test report` still generates coverage.

## Development

| Command                     | Specific arguments/options | Purpose                                                      |
| --------------------------- | -------------------------- | ------------------------------------------------------------ |
| `dev build`                 | `--release`                | Build the workspace.                                         |
| `dev bootstrap`             | none                       | Install hooks and development tools.                         |
| `dev install-hooks`         | none                       | Install Git hooks that delegate to taskit.                   |
| `dev setup`                 | none                       | Install required Cargo development tools.                    |
| `dev clean`                 | `--older-than <AGE>`       | Clean artifacts, optionally by age.                          |
| `dev update`                | `--aggressive`             | Update `Cargo.lock`; optionally ignore semver compatibility. |
| `dev update-claude-version` | `<VERSION>`                | Update the pinned Claude Code version.                       |

## Quality checks

| Command            | Specific options                                             | Purpose                                                          |
| ------------------ | ------------------------------------------------------------ | ---------------------------------------------------------------- |
| `check fmt`        | `--check`, `--affected`                                      | Format or verify Rust code.                                      |
| `check lint`       | `--crate-name`, `--affected`, `--continue-on-error`, `--fix` | Run Clippy.                                                      |
| `check quick`      | none                                                         | Run affected formatting, lint, compile-tests, and offline tests. |
| `check ci`         | `--fail-fast`, `--include-network`                           | Run the full local CI pipeline.                                  |
| `check compile`    | none                                                         | Compile test binaries without running them.                      |
| `check deps`       | none                                                         | Run the unused-dependency check.                                 |
| `check pre-commit` | none                                                         | Run the pre-commit hook checks.                                  |
| `check pre-push`   | none                                                         | Run the pre-push affected checks.                                |

## Testing

| Command          | Specific arguments/options                                       | Purpose                           |
| ---------------- | ---------------------------------------------------------------- | --------------------------------- |
| `test run`       | `--crate-name`, `--affected`, `--continue-on-error`, `--offline` | Run nextest.                      |
| `test coverage`  | `--crate-name`, `--threshold 80`, `--workspace`                  | Measure line coverage.            |
| `test proptest`  | required `--crate-name`                                          | Run tests matching `test(prop)`.  |
| `test fuzz`      | `<TARGET>`, `--duration 60`                                      | Run a cargo-fuzz target.          |
| `test bench`     | `--crate-name`, `--save-baseline`                                | Run Criterion benchmarks.         |
| `test report`    | none                                                             | Generate workspace HTML coverage. |
| `test snapshots` | none                                                             | Launch `cargo insta review`.      |

## Health

| Command          | Specific options                        | Purpose                          |
| ---------------- | --------------------------------------- | -------------------------------- |
| `health check`   | `--update`, `--with-coverage`, `--gate` | Collect and compare health.      |
| `health drift`   | required `--metric`, `--window 7`       | Analyze historical metric drift. |
| `health inspect` | `--max-warnings`, `--max-todo`          | Apply workspace thresholds.      |
| `health version` | none                                    | Show package and Rust versions.  |

## Protocol

| Command                | Specific options                                            | Purpose                                       |
| ---------------------- | ----------------------------------------------------------- | --------------------------------------------- |
| `protocol drift`       | mode flags; interval default 5                              | Check contract hashes.                        |
| `protocol sites`       | required `--file`, `--pattern`, `--expected`; `--warn-only` | Count sites.                                  |
| `protocol todo-sync`   | `--update`, `--warn-only`                                   | Reconcile TODO/FIXME issues.                  |
| `protocol todo-dedupe` | `--update`                                                  | Close duplicate marker issues.                |
| `protocol freshness`   | `--warn-only`                                               | Compare dependencies with published versions. |
| `protocol audit`       | none                                                        | Run cargo-deny.                               |

Protocol drift mode flags are `--update`, `--warn-only`, `--hook`, `--watch`, and
`--interval <SECONDS>`.

## Release

| Command           | Specific arguments/options     | Purpose                           |
| ----------------- | ------------------------------ | --------------------------------- |
| `release patch`   | none                           | Bump workspace patch versions.    |
| `release minor`   | none                           | Bump workspace minor versions.    |
| `release major`   | none                           | Bump workspace major versions.    |
| `release publish` | `--skip-docs`, `--allow-dirty` | Generate docs and publish crates. |
| `release create`  | `<TAG>`, `--notes-file <PATH>` | Create a GitHub release.          |

## Flow

| Command        | Purpose                                      |
| -------------- | -------------------------------------------- |
| `flow status`  | Show current branch and ahead/behind counts. |
| `flow sync`    | Merge configured main into develop.          |
| `flow promote` | Advance the current branch one stage.        |
| `flow auto`    | Run all stages with a built-in CI gate.      |
| `flow guard`   | Reject configured protected branches.        |

## Self-management

| Command        | Purpose                                   |
| -------------- | ----------------------------------------- |
| `self install` | Install taskit from the current checkout. |
| `self test`    | Run taskit's hash-cached test suite.      |
| `self check`   | Verify required tools.                    |

## Top-level commands

| Command                | Specific arguments/options | Purpose                                           |
| ---------------------- | -------------------------- | ------------------------------------------------- |
| `dashboard`            | none                       | Open the terminal dashboard.                      |
| `init`                 | `--force`, `--interactive` | Generate workspace configuration and scaffolding. |
| `changelog unreleased` | none                       | Prepend unreleased changes to `CHANGELOG.md`.     |
| `changelog full`       | none                       | Regenerate the complete changelog.                |
| `changelog latest`     | none                       | Prepend the latest tagged release.                |
| `changelog preview`    | none                       | Print unreleased changes without writing.         |

The bare `taskit changelog` form is equivalent to `taskit changelog unreleased`.

Before Clap parsing, `taskit completions` writes Nushell completions to stdout. It is intentionally
absent from `--help` and generated completions.
