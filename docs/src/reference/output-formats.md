# Output Formats

Select a format globally:

```bash
taskit --output json health inspect
```

| Format       | Destination | Behavior                                      |
| ------------ | ----------- | --------------------------------------------- |
| `human`      | stderr      | Human-readable status and summaries.          |
| `compact`    | stderr      | One line per step with expanded failures.     |
| `json`       | stdout      | Structured pipeline data for tooling.         |
| `github`     | stderr      | GitHub annotations and optional step summary. |
| `junit`      | file        | JUnit XML report.                             |
| `diagnostic` | stderr      | Rich source-oriented diagnostic text.         |
| `sarif`      | file        | SARIF 2.1 report.                             |

Generated report paths are:

```text
target/taskit/taskit-results.xml
target/taskit/taskit-results.sarif
```

GitHub output appends Markdown to the path in `GITHUB_STEP_SUMMARY` when that environment variable
is set.

Other generated reports and state use separate paths:

| Functionality        | Path                                          |
| -------------------- | --------------------------------------------- |
| HTML coverage report | `target/llvm-cov/html/index.html`             |
| Health baseline      | `.health-baseline.json`                       |
| Daily telemetry      | `.taskit/telemetry/YYYY/MM/DD/history.ndjson` |
| Resumable flow state | `target/taskit/state.json`                    |

Structured formatter selection is used by `check ci`, `health inspect`, and `release publish`.
`check quick` always renders human output, while most other commands write ordinary progress or
subprocess streams. Consult the command-specific guide when scripting output.
