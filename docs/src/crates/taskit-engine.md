# taskit-engine

The engine crate wires everything together. All public functions return
`Result<T, TaskitError>`. Depends on `taskit-core`, `taskit-types`, and `taskit-output`.

## Modules

### `config.rs`

- `load(path)` — parse `taskit.toml` into `Config`
- `discover()` — walk up from cwd to find `taskit.toml`

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
| `flow_status` | Print current branch and ahead/behind counts |
| `flow_sync` | Merge main → develop |
| `flow_promote` | develop → staging → release → main (--no-ff merges with LLM conflict resolution) |
| `flow_guard` | Assert branch invariants; fails if violated |
| `flow_auto` | Promote + CI gate + finish; resumes from `.taskit-state.json` |

`merge_with_resolution` handles the merge-conflict loop: on conflict it calls
`ConflictResolver`, stages resolved files, and commits. Unresolvable conflicts raise
`FlowError::NeedsHuman`.

`parse_conflict_paths` parses `git status --porcelain` for `UU`/`AA`/`DD`/`AU`/`UA` markers.

### `ctx.rs`

`Ctx` — shared execution context passed through engine functions: `Shell`, `dry_run` flag,
`OutputFormatter` reference.
