# CI Pipeline

## Running

```sh
taskit check ci                    # full pipeline
taskit check ci --fail-fast        # stop on first failure
taskit check ci --include-network  # include network-dependent steps
taskit check quick                 # fast feedback: fmt-check + lint + compile-tests + test
```

## Default steps

| Step | Command | Gate |
|------|---------|------|
| Self-check | `taskit dev self-check` | Yes |
| Format | `taskit check fmt --check` | No |
| Lint | `taskit check lint` | No |
| Compile tests | `taskit check compile` | No |
| Test | `taskit test run` | No |
| Deps | `taskit check deps` | No |
| Protocol drift | `taskit protocol drift` | No |

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
`check-deps`, `check-protocol-drift`, `self-check`, and `health`. These are internal step
identifiers (matched in `taskit-engine::ci::dispatch_cmd`), independent of the CLI's
subcommand paths above — renaming a CLI category (`dev`/`check`/`test`/...) does not affect
these `[[ci.steps]] cmd` values.

## Affected-crate mode

Pass `--affected` to limit steps to crates with uncommitted changes plus their dependents
(configured via `[[workspace.propagation]]`):

```sh
taskit check lint --affected
taskit test run --affected
```

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | All steps passed |
| 1 | One or more steps failed, including gate failures that aborted later steps |
