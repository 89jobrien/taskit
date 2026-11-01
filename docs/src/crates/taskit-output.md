# taskit-output

`taskit-output` owns formatting and message sinks. It depends on `taskit-types`.

## Formatter port and adapters

`OutputFormatter::render` converts `PipelineOutcome` into a string. `formatter_for` selects:

| Format     | Adapter               |
| ---------- | --------------------- |
| Human      | `HumanFormatter`      |
| Compact    | `CompactFormatter`    |
| JSON       | `JsonFormatter`       |
| GitHub     | `GithubFormatter`     |
| JUnit      | `JunitFormatter`      |
| Diagnostic | `DiagnosticFormatter` |
| SARIF      | `SarifFormatter`      |

`write_output` sends console formats to stdout/stderr or writes file formats to:

```text
target/taskit/taskit-results.xml
target/taskit/taskit-results.sarif
```

## Message sinks

`MessageSink` is the progress/event output port. Implementations include `StderrSink`,
`BufferSink`, and `TeeSink`; `set_sink` and `sink` manage the process-global selected sink.

Exported macros are `taskit_progress!`, `taskit_skip!`, `taskit_dry!`, `taskit_ok!`,
`taskit_err!`, and `taskit_warn!`.

See [Output Formats](../reference/output-formats.md) for destinations and scripting caveats.
