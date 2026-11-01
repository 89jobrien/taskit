# taskit-core

`taskit-core` is the ports layer. It depends only on `taskit-types`.

## Public modules

| Module              | Public surface                                 |
| ------------------- | ---------------------------------------------- |
| `conflict_resolver` | `ConflictResolver`                             |
| `pipeline_runner`   | `PipelineRunner`                               |
| `step_builder`      | `StepBuilder`                                  |
| `conformance`       | Reusable `PipelineRunner` invariant assertions |

`ConflictResolver` is re-exported from the crate root. `PipelineRunner` remains available through
`taskit_core::pipeline_runner`. The conformance module is exported unconditionally; no
`test-support` feature exists.

## Pipeline adapters

| Adapter                | Location        | Current status                                           |
| ---------------------- | --------------- | -------------------------------------------------------- |
| `BuiltinRunner`        | `taskit-engine` | Implemented; not used by current production CI dispatch. |
| `SubprocessCruxRunner` | `taskit-engine` | Publicly constructible; current callers are tests.       |
| `EmbeddedCruxRunner`   | `taskit-crux`   | Synthetic stub; not wired into the binary.               |

## Conflict adapters

The root binary selects either `BamlConflictResolver` or its no-op resolver for `flow auto`.
