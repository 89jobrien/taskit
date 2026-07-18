# taskit-types

Leaf crate. Owns all shared domain types. No business logic, no I/O, no dependencies
outside the standard library and derive crates (`serde`, `thiserror`, `miette`).

## Modules

### `config.rs`

`Config` — top-level deserialized from `taskit.toml`.

```
Config
  workspace: WorkspaceConfig   crates, propagation rules, offline_skip
  ci:        CiConfig          steps, cruxfile path, fail_fast
  inspect:   InspectConfig     max_clippy_warnings, max_todo_fixme, ...
  clean:     CleanConfig       older_than: Option<String>
  flow:      FlowConfig        main/develop/staging/release names, conflict_resolver
  release:   ReleaseConfig     github_repo, publish_order, skip_docs, allow_dirty
  protocol:  ProtocolConfig    surfaces: Vec<ProtocolSurface>
  coverage:  CoverageConfig    threshold, exclude
```

### `error.rs`

`TaskitError` — top-level `miette::Diagnostic` enum; all variants are transparent:

| Variant | When raised |
|---------|-------------|
| `Config(ConfigError)` | TOML not found, parse failure, invalid value |
| `Pipeline(PipelineError)` | Step failure, gate abort |
| `Protocol(ProtocolError)` | Drift detected, lockfile missing or stale |
| `Init(InitError)` | Scaffold write failure, cargo metadata error |
| `Flow(FlowError)` | Wrong branch, dirty worktree, merge failure, NeedsHuman |
| `Io(std::io::Error)` | Raw I/O errors |
| `Other(Box<dyn Error>)` | Escape hatch — use `TaskitError::other(msg)` |

`TaskitResultExt` adds `.err_context("msg")?` on any `Result<T, E: Display>`.

### `step.rs`

- `StepResult` — name, `StepStatus`, duration, optional error string, gate flag
- `StepStatus` — `Pass | Fail | Skip`
- `PipelineOutcome` — all results, total duration, `passed` bool, optional context

### `conflict.rs`

- `ConflictFile` — ours/theirs/base content for one conflicted path
- `ResolvedFile` — resolver output: resolved content + explanation string

### `output_format.rs`

`OutputFormat` — `Pretty | Json | Minimal`

### `flow_state.rs`

`FlowState` — serialised checkpoint for `flow auto` resumption; written to
`.taskit-state.json` between promotion steps.
