# taskit-types

`taskit-types` is the shared contract crate. It contains no workspace-crate dependencies.

## Public modules

| Module          | Responsibility                                                    |
| --------------- | ----------------------------------------------------------------- |
| `config`        | Full `taskit.toml` schema, defaults, and validation diagnostics.  |
| `conflict`      | `ConflictFile` and `ResolvedFile`.                                |
| `error`         | Typed errors, diagnostics, and `TaskitResultExt`.                 |
| `flow_state`    | Serializable `flow auto` checkpoint state.                        |
| `output_format` | Seven CLI output variants.                                        |
| `step`          | Results, outcomes, diagnostics, command records, and run context. |

Conflict values are re-exported from the crate root.

## Configuration

`Config` contains `workspace`, `protocol`, `ci`, `coverage`, `flow`, `release`, `inspect`, `clean`,
and `output`. Flow includes branch names, `ConflictResolverKind`, optional push, and remote.
See the [Configuration reference](../reference/configuration.md).

## Errors

`TaskitError` wraps configuration, pipeline, protocol, initialization, flow, I/O, and other
errors. Flow errors include wrong/protected branches, `NotAFlowBranch`, dirty worktrees, missing
branches, merge and push failures, CI gate failure, and `NeedsHuman`. Checkout failures currently
map to the general `Other` variant; conflict-resolution commit failures map to `MergeFailed`.

## Pipeline values

`StepResult` records status, duration, gate behavior, errors, diagnostics, and context.
`PipelineOutcome` aggregates results, duration, pass/fail state, and optional run context.
