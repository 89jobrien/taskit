# taskit-engine

The engine crate wires everything together. All public functions return
`Result<T, TaskitError>`. Depends on `taskit-core`, `taskit-types`, and `taskit-output`.

## Modules

### `config.rs`

- `load()` — find `taskit.toml` from cwd, parse it, merge discovered workspace metadata, and
  return a `Workspace`
- `discover(workspace_root)` — build a conventional `Config` from cargo metadata

### `ci.rs`

Pipeline assembly and step dispatch. Reads `CiConfig.steps`, constructs a `Pipeline` via
`taskit-engine/step.rs`, runs it, and returns a `PipelineOutcome`.

### `step.rs`

`Pipeline` builder. Each entry is either a `step` (report and continue on failure) or a
`gate` (abort immediately on failure). `fail_fast` collapses all steps to gate behaviour.

### `pipeline_runner.rs`

- `BuiltinRunner` — runs the step engine in-process
- `SubprocessCruxRunner` — spawns an external `crux` binary and maps its output to
  `PipelineOutcome`

### `flow.rs`

Git branching workflow commands:

| Function | Description |
|----------|-------------|
| `status` | Print current branch and ahead/behind counts |
| `sync` | Merge main → develop |
| `promote` | Advance the current flow branch one step with `--no-ff` merges |
| `guard` | Assert branch invariants; fails if violated |
| `auto` | develop → staging → release → main with CI gate and resumable state |

`merge_with_resolution` handles the `flow auto` merge-conflict loop: on conflict it calls
`ConflictResolver`, stages resolved files, and commits. Unresolvable conflicts raise
`FlowError::NeedsHuman`. The one-step `promote` path uses plain `--no-ff` merges.

`parse_conflict_paths` parses `git status --porcelain` for `UU`/`AA`/`DD`/`AU`/`UA` markers.

### `ctx.rs`

`Ctx` — shared execution context passed through engine functions: `Shell`, `dry_run` flag,
`OutputFormatter` reference.
