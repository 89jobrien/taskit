# Quality Checks and CI

## Formatting and linting

```bash
taskit check fmt
taskit check fmt --check
taskit check fmt --affected

taskit check lint
taskit check lint --crate-name taskit-engine
taskit check lint --affected
```

Formatting runs `cargo fmt --all`; `--check` makes it read-only. Affected mode compares
`origin/main...HEAD` and includes dependents from `[[workspace.propagation]]`.

Linting runs Clippy with all targets, the lockfile, and warnings denied. `--crate-name` takes
precedence over `--affected`. Per-crate linting can continue after failures:

```bash
taskit check lint --affected --continue-on-error
```

`--fix` passes Clippy's `--fix --allow-dirty --allow-staged` options and can rewrite source files:

```bash
taskit check lint --fix
```

## Fast feedback

```bash
taskit check quick
```

Quick mode runs a non-fail-fast pipeline:

1. formatting check
2. Clippy
3. cached test-binary compilation
4. nextest in offline mode

Formatting, lint, and tests use affected crates. Compile-tests independently scans all workspace
members and recompiles every stale package in its hash cache. Offline filtering uses
`[workspace].offline_skip` when configured. Quick mode does not run coverage, dependency checks,
or protocol drift.

## Full CI

```bash
taskit check ci
taskit check ci --fail-fast
taskit check ci --include-network
```

When `[ci]` is absent, the built-in pipeline runs:

1. self-check (gate)
2. formatting check
3. lint
4. compile-tests
5. tests
6. unused-dependency check
7. protocol drift
8. coverage when `[coverage]` exists

A gate failure skips all later steps. `--fail-fast` also stops after an ordinary failure. The
effective setting is CLI `--fail-fast` or `[ci].fail_fast = true`.

Configured `[[ci.steps]]` replace the built-in sequence. An explicit `[ci]` with no steps runs an
empty successful pipeline. See [CI Pipeline](../reference/ci-pipeline.md) for supported step names.

Network-dependent tests are excluded only when `[workspace].offline_skip` supplies a nextest
expression. `--include-network` disables that filtering.

## Supporting checks

```bash
taskit check compile
taskit check deps
```

Compile-tests caches successful package hashes in:

```text
target/taskit/cache/compile-cache.json
```

`check deps` runs `cargo-machete`; it checks unused dependencies, not available updates or
license/advisory policy.

## Git hooks

```bash
taskit check pre-commit
taskit check pre-push
```

Pre-commit skips when no staged Rust files exist. Otherwise it checks formatting, affected lint,
and protocol drift. It can apply Clippy fixes, re-stage Rust files, update
`taskit-protocol.lock`, and stage that lockfile.

Pre-push runs affected lint/tests, configured coverage, and protocol drift. If protocol drift is
found, it updates and stages the lockfile, then runs:

```bash
git commit --amend --no-edit
```

Both hook commands cache successful work under `target/taskit/cache/`.
