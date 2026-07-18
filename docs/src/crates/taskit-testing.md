# taskit-testing

Shared test helpers and the `PipelineRunner` conformance harness. Used only in `dev-dependencies`.

## Conformance harness

`PipelineRunnerConformance` — a parameterized test suite that any `PipelineRunner` implementation
can be run against to verify the five core invariants:

1. A passing pipeline returns `PipelineOutcome { passed: true }`
2. A failing pipeline returns `PipelineOutcome { passed: false }`
3. `fail_fast: true` stops execution after the first failure
4. Gate failures abort the pipeline regardless of `fail_fast`
5. All returned `StepResult` names are non-empty

Run the harness for a new impl:

```rust
#[cfg(test)]
mod conformance {
    use taskit_testing::PipelineRunnerConformance;
    use super::MyRunner;

    #[test]
    fn conforms() {
        PipelineRunnerConformance::new(MyRunner::default()).run_all();
    }
}
```

## Test helpers

- `TempWorkspace` — creates a throwaway Cargo workspace in a tempdir with a valid
  `taskit.toml`; cleaned up on drop
- `fake_step_result(name, passed)` — constructs a minimal `StepResult` for unit tests
- `fake_pipeline_outcome(passed)` — constructs a minimal `PipelineOutcome`
