# Architecture Overview

`taskit` uses a hexagonal (ports-and-adapters) architecture across a multi-crate workspace.
Domain types live in the leaf crate; ports (traits) live in `taskit-core`; adapters and
business logic live in `taskit-engine` and the binary.

## Workspace Structure

```
taskit (root bin)
+-- crates/taskit-types    -- leaf: Config, TaskitError, StepResult, ConflictFile
+-- crates/taskit-core     -- ports: PipelineRunner, ConflictResolver traits
+-- crates/taskit-engine   -- CI pipeline, config loading, flow commands
+-- crates/taskit-init     -- `taskit init`: discovery + file generation
+-- crates/taskit-crux     -- EmbeddedCruxRunner (optional, `crux` feature)
+-- crates/taskit-macros   -- proc-macros for derive utilities
+-- crates/taskit-output   -- OutputFormatter trait and format implementations
+-- crates/taskit-testing  -- shared test helpers; conformance harness
```

## Dependency Direction

```
taskit (bin)
  -> taskit-engine, taskit-init, taskit-output, taskit-core, taskit-types
taskit-engine
  -> taskit-core, taskit-types
taskit-core
  -> taskit-types
taskit-types   (no internal deps)
```

No crate below `taskit-engine` imports from the binary or from `taskit-output`.

## Key Ports

| Port                  | Crate          | Implementors                                    |
| --------------------- | -------------- | ----------------------------------------------- |
| `PipelineRunner`      | taskit-core    | `BuiltinRunner`, `SubprocessCruxRunner`         |
| `ConflictResolver`    | taskit-core    | `BamlConflictResolver` (binary adapter)         |

## Flow Commands

`taskit flow` is an agentic branching workflow: `main -> staging -> release -> main`.

| Subcommand | What it does                                                            |
| ---------- | ----------------------------------------------------------------------- |
| `status`   | Print current branch and staging state                                  |
| `promote`  | Merge main into staging                                                 |
| `finish`   | Merge staging into main after CI passes                                 |
| `guard`    | Assert branch invariants (abort if violated)                            |
| `auto`     | Run promote -> CI -> finish end-to-end with LLM conflict resolution     |

`flow auto` uses `BamlConflictResolver` (a BAML-generated LLM adapter) to attempt
automatic conflict resolution. When LLM confidence is below threshold, it escalates
by returning `FlowError::NeedsHuman { path, reason }`.

### FlowError Variants

| Variant                               | Meaning                                                  |
| ------------------------------------- | -------------------------------------------------------- |
| `CiFailed { failed: Vec<String> }`    | One or more CI steps failed; lists step names            |
| `NeedsHuman { path, reason }`         | LLM confidence too low; human must resolve this conflict |
| `ConflictUnresolved { path }`         | Conflict present but no resolver succeeded               |

## ConflictFile / ResolvedFile

`ConflictFile` and `ResolvedFile` are domain types owned by `taskit-types::conflict`.
`ConflictResolver` (the port) lives in `taskit-core::conflict_resolver`. The engine
imports from those layers; the binary provides the `BamlConflictResolver` adapter.
