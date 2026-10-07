# taskit-testing

`taskit-testing` provides shared test utilities over `taskit-types`.

## `TempDirGuard`

`TempDirGuard::new` creates a temporary directory, switches the process current directory to it,
and restores the original directory on drop. `path()` returns the temporary path.

Because changing the process current directory is global, concurrent tests must serialize uses of
this guard.

## Exported macros

| Macro                  | Purpose                                          |
| ---------------------- | ------------------------------------------------ |
| `in_temp_dir!`         | Run a block with a `TempDirGuard`.               |
| `step_result!`         | Construct a `taskit_types::step::StepResult`.    |
| `__step_result_inner!` | Hidden implementation helper for `step_result!`. |

The crate does not export `PipelineRunnerConformance`, `TempWorkspace`, or fake outcome helpers.
Runner invariants live in `taskit_core::conformance`.
