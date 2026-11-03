# taskit-output

`taskit-output` owns taskit's result formatting and live message-sink abstractions. It translates
the shared contracts from `taskit-types` into console output, CI artifacts, diagnostics, and
structured events without owning pipeline execution.

## Workspace role

`taskit-engine` emits `Message` values while work is running and produces a `PipelineOutcome` when
the run finishes. This crate provides two distinct paths:

- `MessageSink` implementations and macros for live progress; and
- `OutputFormatter` implementations plus `write_output` for final outcomes.

The crate has no Cargo feature flags.

## Outcome formatters

`OutputFormatter::render` converts a `PipelineOutcome` to a string. `formatter_for` selects the
adapter for an `OutputFormat`:

| Format       | Formatter             | `write_output` destination           |
| ------------ | --------------------- | ------------------------------------ |
| `human`      | `HumanFormatter`      | stderr                               |
| `compact`    | `CompactFormatter`    | stderr                               |
| `json`       | `JsonFormatter`       | stdout                               |
| `github`     | `GithubFormatter`     | stderr and optional step summary     |
| `junit`      | `JunitFormatter`      | `target/taskit/taskit-results.xml`   |
| `diagnostic` | `DiagnosticFormatter` | stderr                               |
| `sarif`      | `SarifFormatter`      | `target/taskit/taskit-results.sarif` |

`CompactFormatter` is public through `taskit_output::formatter`, but unlike the other formatter
types it is not re-exported from the crate root. The factory currently always enables compact
failure details rather than honoring `OutputConfig::verbose_on_failure`.

```rust
use taskit_output::{HumanFormatter, OutputFormatter};
use taskit_types::step::PipelineOutcome;

let rendered = HumanFormatter.render(&PipelineOutcome {
    passed: true,
    ..PipelineOutcome::default()
});
assert!(rendered.contains("Total"));
```

JSON output uses schema version 2 and includes step context and run context when present. GitHub
output emits workflow command annotations and appends its Markdown table to `GITHUB_STEP_SUMMARY`
when that environment variable is set. SARIF creates one run per step and maps structured
diagnostics into rules, results, and source locations.

`write_output` returns `PipelineError` after rendering when the outcome failed. File creation and
writes for JUnit, SARIF, and the GitHub summary are currently best effort: their I/O errors are
discarded, and the function still prints a success-path message for the artifact destination.

## Pipeline errors

`pipeline_error` converts failed steps into a miette-compatible `PipelineError::Failed`, including
the failure count, rendered summary, and related `StepError` values. `DiagnosticFormatter` chooses
a graphical miette report for a terminal stderr and a narratable report otherwise.

## Live messages and sinks

`MessageSink` is a `Send + Sync` port with `emit` and `flush`. The built-in adapters are:

| Sink         | Behavior                                             | Current use                   |
| ------------ | ---------------------------------------------------- | ----------------------------- |
| `StderrSink` | Human-readable progress and diagnostics.             | Default production sink.      |
| `BufferSink` | Thread-safe, cloneable in-memory message collection. | Tests/buffering.              |
| `TeeSink`    | Fans messages and flushes out to child sinks.        | Tested, not production-wired. |

`set_sink` stores one process-global sink in a `OnceLock`. The first call wins permanently;
subsequent calls are ignored. Despite the source module name `thread_sink`, this is process-global,
not thread-local. `sink` returns a static `StderrSink` when no sink was installed.

The exported progress macros format their arguments and emit through the active sink:

```rust
use taskit_output::{BufferSink, set_sink, taskit_ok, taskit_progress};

let buffer = BufferSink::new();
let observer = buffer.clone();
set_sink(Box::new(buffer));
taskit_progress!("running {}", "lint");
taskit_ok!("done");
assert_eq!(observer.len(), 2);
```

Available macros are `taskit_progress!`, `taskit_skip!`, `taskit_dry!`, `taskit_ok!`,
`taskit_err!`, and `taskit_warn!`. Warnings are currently represented as `Message::Progress` with
a `warning:` prefix; there is no separate warning variant.

## Example and development

```bash
cargo run -p taskit-output --example format_outcome
cargo check -p taskit-output
cargo clippy -p taskit-output --all-targets -- -D warnings
cargo nextest run -p taskit-output
```

The checked-in formatter example renders human, JSON, GitHub, JUnit, diagnostic, and SARIF output.
Its introductory comment says "six" formatters and does not demonstrate compact output, while the
crate currently supports seven formats.
