# CI Pipeline

## Running

```sh
taskit ci                    # full pipeline
taskit ci --fail-fast        # stop on first failure
taskit ci --include-network  # include network-dependent steps
taskit quick                 # fast feedback: fmt-check + lint + test only
```

## Default steps

| Step | Command | Gate |
|------|---------|------|
| Self-check | `taskit self-check` | Yes |
| Format | `taskit fmt --check` | No |
| Lint | `taskit lint` | No |
| Compile tests | `taskit compile-tests` | No |
| Test | `taskit test` | No |
| Deps | `taskit check-deps` | No |
| Protocol drift | `taskit check-protocol-drift` | No |

**Gate** steps abort the pipeline immediately on failure. Non-gate steps report failure and
continue. `--fail-fast` promotes all steps to gate behaviour.

## Customising steps

Override the default step list in `taskit.toml`:

```toml
[ci]
fail_fast = false
steps = ["fmt", "lint", "test", "audit"]
```

Valid step names correspond to taskit subcommands (`fmt`, `lint`, `test`, `coverage`,
`compile-tests`, `check-deps`, `check-protocol-drift`, `audit`).

## Affected-crate mode

Pass `--affected` to limit steps to crates with uncommitted changes plus their dependents
(configured via `[[workspace.propagation]]`):

```sh
taskit lint --affected
taskit test --affected
```

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | All steps passed |
| 1 | One or more steps failed |
| 2 | Gate failed — pipeline aborted early |
