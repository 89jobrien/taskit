# CI Pipeline

## Running

```sh
taskit ci                    # full pipeline
taskit ci --fail-fast        # stop on first failure
taskit ci --include-network  # include network-dependent steps
taskit quick                 # fast feedback: fmt-check + lint + compile-tests + test
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

[[ci.steps]]
name = "fmt --check"
cmd = "fmt --check"
gate = false

[[ci.steps]]
name = "lint"
cmd = "lint"
gate = false

[[ci.steps]]
name = "test"
cmd = "test"
gate = false
```
Supported config step commands are `fmt`, `lint`, `test`, `coverage`, `compile-tests`,
`check-deps`, `check-protocol-drift`, `self-check`, and `health`.

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
| 1 | One or more steps failed, including gate failures that aborted later steps |
