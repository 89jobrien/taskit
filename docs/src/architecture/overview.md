# Architecture Overview

Taskit uses a multi-crate hexagonal structure. Shared contracts point inward; orchestration and
adapters depend on those contracts; the root binary wires concrete behavior.

## Runtime dependency direction

This diagram excludes dev-dependencies, the workspace-excluded fuzz package, and the publish-false
`xtask` package.

```text
taskit binary
├── taskit-types
├── taskit-core ───────────> taskit-types
├── taskit-output ─────────> taskit-types
├── taskit-engine ─────────> taskit-core + taskit-output + taskit-types
├── taskit-init ───────────> taskit-types
└── taskit-tui ────────────> taskit-engine + taskit-types

taskit-crux ───────────────> taskit-core + taskit-types   (unwired stub)
taskit-testing ────────────> taskit-types                 (test support)
taskit-macros ─────────────> no runtime workspace crates  (proc macro)
```

`taskit-tui` also uses the external `rx-runner` crate for non-blocking dashboard actions.
`taskit-crux` is a workspace member but is not a root-binary dependency.

## Layers

### Shared contracts

`taskit-types` owns configuration, errors, pipeline results, diagnostics, conflict values, output
formats, and persisted flow state.

### Ports

`taskit-core` defines `PipelineRunner`, `ConflictResolver`, and `StepBuilder`, plus reusable
conformance assertions. It depends only on shared types.

### Orchestration and adapters

`taskit-engine` owns command objects, context, configuration loading, pipelines, tests, health,
protocol governance, flow, release operations, and concrete command execution.

`taskit-init`, `taskit-output`, and `taskit-tui` are focused outer adapters for generation,
presentation, and terminal interaction. `taskit-testing` and `taskit-macros` support tests.

### Composition root

`src/main.rs` defines the Clap tree, handles init before config loading, changes to the resolved
workspace root, installs the output sink, builds `Ctx`, selects a conflict resolver, and dispatches
commands. BAML and no-op conflict resolvers are wired only at this boundary.

## Runtime data flow

```text
CLI arguments
  -> Config load/discovery
  -> Ctx (shell, root, config, dry-run, output)
  -> typed Command
  -> engine operation
  -> MessageSink / OutputFormatter
```

CI builds a `Pipeline` of ordinary steps and gates. Gates always stop later work; fail-fast makes
ordinary failures stop later work too. Diagnostics retain reproduction commands and captured child
commands in `PipelineOutcome`.

## Persisted state

| State               | Path                                          |
| ------------------- | --------------------------------------------- |
| Health baseline     | `.health-baseline.json`                       |
| Protocol hashes     | `taskit-protocol.lock` or configured lockfile |
| TODO issue mapping  | `taskit-todo-sync.lock`                       |
| Telemetry           | `.taskit/telemetry/YYYY/MM/DD/history.ndjson` |
| Flow resumption     | `target/taskit/state.json`                    |
| Hook/compile caches | `target/taskit/cache/`                        |

## Adapter status

- `BamlConflictResolver` and the no-op resolver are wired for `flow auto`.
- `BuiltinRunner` and `SubprocessCruxRunner` exist but current CI dispatch calls `ci::run`
  directly.
- `EmbeddedCruxRunner` validates its path and returns a synthetic passing result; it does not
  execute Crux and is not wired into the binary.
- `CiConfig.cruxfile` is parsed but does not currently select a runner.
