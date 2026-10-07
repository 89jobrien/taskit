# taskit-core

`taskit-core` is taskit's ports layer. It defines the narrow traits that pipeline and conflict
resolution adapters implement, plus shared builders and conformance assertions. It depends only on
`taskit-types`, keeping orchestration and process execution out of the core boundary.

## Workspace role

```text
taskit-types  <-  taskit-core  <-  taskit-engine / taskit-crux / taskit
 contracts         ports             adapters and composition root
```

The crate has no feature flags. Its `conformance` module is always public despite an outdated
module comment that refers to a `test-support` feature.

## Public API

| Module              | API                 | Contract                                                   |
| ------------------- | ------------------- | ---------------------------------------------------------- |
| `pipeline_runner`   | `PipelineRunner`    | Run a pipeline and return a `PipelineOutcome`.             |
| `conflict_resolver` | `ConflictResolver`  | Resolve conflicted files into final file content.          |
| `step_builder`      | `StepBuilder`       | Construct `StepResult` values with test-friendly defaults. |
| `conformance`       | Assertion functions | Check common `PipelineRunner` outcome invariants.          |

`ConflictResolver` is also re-exported as `taskit_core::ConflictResolver`. `PipelineRunner` is
currently accessed through `taskit_core::pipeline_runner::PipelineRunner`.

### `PipelineRunner`

```rust
use std::path::Path;

use taskit_core::pipeline_runner::PipelineRunner;
use taskit_types::error::TaskitError;
use taskit_types::step::PipelineOutcome;

struct Runner;

impl PipelineRunner for Runner {
    fn run_pipeline(
        &self,
        config_path: &Path,
        fail_fast: bool,
    ) -> Result<PipelineOutcome, TaskitError> {
        let _ = (config_path, fail_fast);
        Ok(PipelineOutcome {
            passed: true,
            ..PipelineOutcome::default()
        })
    }
}
```

The port currently passes a config path and a `fail_fast` boolean separately. Implementations may
not use both parameters; the source tracks replacing these inconsistent arguments with an explicit
pipeline request.

Known adapters are:

| Adapter                | Crate           | Current status                                           |
| ---------------------- | --------------- | -------------------------------------------------------- |
| `BuiltinRunner`        | `taskit-engine` | Executes configured or built-in taskit steps.            |
| `SubprocessCruxRunner` | `taskit-engine` | Runs `crux run <Cruxfile>` as a subprocess.              |
| `EmbeddedCruxRunner`   | `taskit-crux`   | Stub that returns a synthetic pass for an existing file. |

The current `taskit check ci` path calls engine CI functions directly; it does not select these
runner adapters from `[ci].cruxfile`.

### `ConflictResolver`

The resolver receives `ConflictFile` values containing relative paths and the ours/theirs/base
content. It returns one `ResolvedFile` per resolved path or a `TaskitError`. The taskit binary wires
either its BAML resolver or a resolver that reports automatic resolution as disabled.

### `StepBuilder`

`StepBuilder::new` defaults to a passing, non-gate, zero-duration result with no error,
diagnostics, or diagnostic context. Chain `status`, `fail`, `skip`, `duration`, `duration_ms`,
`error`, `gate`, and `diagnostic` before `build`:

```rust
use taskit_core::step_builder::StepBuilder;

let result = StepBuilder::new("coverage")
    .fail()
    .duration_ms(125)
    .error("below threshold")
    .gate()
    .diagnostic("coverage", "78% is below 80%")
    .build();

assert!(!result.error.as_deref().unwrap_or_default().is_empty());
assert!(result.gate);
```

`diagnostic` currently creates warning-level diagnostics without file or position metadata.

## Conformance contracts

The public helpers assert that:

- nonexistent runner input returns an error;
- successful outcomes are marked passed, contain results, and contain only passing steps;
- failed outcomes are marked failed and contain at least one failed step;
- total duration is at least the sum of step durations, with a 1 ms tolerance; and
- step names are non-empty.

`assert_pipeline_runner_contract` currently checks only nonexistent-path behavior. Adapter tests
must call the outcome assertions separately for passing and failing runs.

## Examples and development

Run the checked-in custom runner example:

```bash
cargo run -p taskit-core --example custom_runner
```

Validate the crate with:

```bash
cargo check -p taskit-core
cargo clippy -p taskit-core --all-targets -- -D warnings
cargo nextest run -p taskit-core
```

When changing either port, update the corresponding protocol surface in
`taskit-protocol.lock` through the supported taskit protocol-drift workflow.
