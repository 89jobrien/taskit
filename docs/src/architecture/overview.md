# Architecture Overview

taskit uses hexagonal architecture across the root binary and supporting crates. The core
direction is from shared types and ports toward adapters; the binary wires adapters together.

## Dependency graph

```
taskit (bin)
  ├── taskit-engine
  │     ├── taskit-core          ← ports only
  │     │     └── taskit-types   ← shared config, errors, outcomes
  │     ├── taskit-output
  │     │     └── taskit-types
  │     └── taskit-types
  ├── taskit-init
  │     └── taskit-types
  ├── taskit-output
  ├── taskit-core
  └── taskit-types

taskit-crux
  └── taskit-core

taskit-testing (dev/test only)
  └── taskit-types

taskit-macros (proc-macro)
  └── syn / quote / proc-macro2
```
`taskit-macros` is available for derive utilities but is not currently a runtime dependency of
`taskit-types`.

## Layers

### Layer 1 — Types (leaf)

`taskit-types` owns shared domain types. It is intentionally low-level and contains no business
logic or I/O-heavy adapters.

Key types: `Config`, `WorkspaceConfig`, `CiConfig`, `FlowConfig`, `TaskitError`, `StepResult`,
`PipelineOutcome`, `ConflictFile`, `OutputFormat`.

### Layer 2 — Ports

`taskit-core` defines the boundary interfaces. It depends only on `taskit-types`.

- `PipelineRunner` — executes a CI pipeline; adapters: `BuiltinRunner`, `SubprocessCruxRunner`,
  `EmbeddedCruxRunner`
- `ConflictResolver` — resolves merge conflicts; adapters: `BamlConflictResolver`, no-op

### Layer 3 — Adapters / Engine

`taskit-engine` wires everything together: config loading, pipeline assembly, step dispatch,
and flow commands. All public functions return `Result<T, TaskitError>`.

`taskit-init` handles discovery and file generation for `taskit init`. It is deliberately
separate from the engine to keep the engine free of generation concerns.

`taskit-output` owns formatting, message sinks, summary tables, and dry-run output through the
`OutputFormatter` trait.

### Layer 4 — Binary

`src/main.rs` parses CLI args (clap), loads config, instantiates adapters, and dispatches to
engine functions. It returns `miette::Result<()>` for rich terminal error rendering.

`src/flow_resolver.rs` houses the `BamlConflictResolver` — the LLM-backed conflict resolution
adapter wired only at the binary boundary.

## Error strategy

All errors are `TaskitError`, a miette `Diagnostic` enum with nested domain variants:

| Variant | Domain |
|---------|--------|
| `Config(ConfigError)` | TOML parse / not-found / invalid |
| `Pipeline(PipelineError)` | Step failures, gate aborts |
| `Protocol(ProtocolError)` | Drift detection, stale lockfile |
| `Init(InitError)` | Scaffold write failures |
| `Flow(FlowError)` | Branch violations, merge conflicts, CI gate |
| `Io(std::io::Error)` | Raw I/O |
| `Other(Box<dyn Error>)` | Escape hatch |

`TaskitResultExt` provides `.err_context("msg")?` as an ergonomic alternative to
`.map_err(|e| TaskitError::other(format!(...)))`.

## Protocol drift

Contract surfaces (public trait signatures, key structs) are hashed and stored in
`taskit-protocol.lock`. `taskit check-protocol-drift` fails CI when the lock diverges from
source, preventing silent API breaks.

## Flow branching model

```
develop → staging → release → main
```

Each promotion is a `--no-ff` merge. LLM conflict resolution (`BamlConflictResolver`) runs on
any merge conflict. Unresolvable conflicts escalate via `FlowError::NeedsHuman`.

`taskit flow auto` is the full pipeline: promote + CI gate + finish, with state persisted to
`.taskit-state.json` for resumption after interruption.
