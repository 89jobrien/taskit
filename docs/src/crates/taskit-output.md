# taskit-output

Output formatting crate. Owns the `OutputFormatter` trait and all its implementations.
Depends only on `taskit-types`.

## `OutputFormatter` trait

The engine and binary accept an `&dyn OutputFormatter` to decouple output from logic. All
pipeline progress, step results, and summary tables route through this interface.

## Implementations

| Impl | Format | When |
|------|--------|------|
| `HumanFormatter` | Human-readable summary table | `--output human` / default |
| `CompactFormatter` | One line per step, with failure details | `--output compact` |
| `JsonFormatter` | Structured JSON document | `--output json` |
| `GithubFormatter` | GitHub Actions annotations and summary | `--output github` |
| `JunitFormatter` | JUnit XML written to `target/taskit-results.xml` | `--output junit` |
| `DiagnosticFormatter` | Diagnostic-oriented text output | `--output diagnostic` |
| `SarifFormatter` | SARIF written to `target/taskit-results.sarif` | `--output sarif` |

## Dry-run macro

`taskit_dry!(fmt, ...)` — prints a `[dry-run]`-prefixed line without executing anything.
Used throughout the engine when `ctx.dry_run` is true.
