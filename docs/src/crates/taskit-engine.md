# taskit-engine

`taskit-engine` owns taskit's application orchestration and concrete command adapters. It depends
on `taskit-core`, `taskit-types`, and `taskit-output`.

## Public module groups

```text
affected, audit, bootstrap, build, cache, changelog, check_deps,
check_freshness, ci, clean, command, config, ctx, dev_setup, discovery,
drift, flow, flow_state_store, fmt, health, hooks, inspect, install,
lint, patch, pipeline_runner, progress, protocol, publish, quick, release,
step, telemetry, testing, todo_sync, update, update_claude, util, version
```

Nested public modules include all test runners under `testing`, contract hashing/drift/site checks
under `protocol`, and GitHub release handling under `release::gh`.

## Core surfaces

- `Command` is the execution port implemented by typed command structs.
- `Ctx` carries `Shell`, workspace root, parsed `Config`, dry-run state, output format, transient
  output suppression, and command provenance.
- `Workspace` is defined at the crate root and returned by configuration loading.
- `Pipeline` executes ordered steps and gates with diagnostics and aggregate outcomes.

## Responsibilities

- configuration discovery and validation
- affected-crate propagation
- formatting, linting, nextest, coverage, fuzzing, benchmarks, and snapshots
- CI pipeline assembly and telemetry
- health baselines, inspection, and drift
- protocol hashes, site counts, TODO issue synchronization, freshness, and cargo-deny
- branch promotion, conflict resolution, push, and resumable flow state
- bootstrap, hooks, build, install, cleanup, updates, changelog, and release operations

Current CI dispatch directly invokes engine functions. The runner adapters in
`pipeline_runner.rs` are not selected by `[ci].cruxfile`.
