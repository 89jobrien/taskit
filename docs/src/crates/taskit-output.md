# taskit-output

Output formatting crate. Owns the `OutputFormatter` trait and all its implementations.
Depends only on `taskit-types`.

## `OutputFormatter` trait

The engine and binary accept an `&dyn OutputFormatter` to decouple output from logic. All
pipeline progress, step results, and summary tables route through this interface.

## Implementations

| Impl | Format | When |
|------|--------|------|
| `PrettyFormatter` | Human-readable with colour and tables | Default TTY output |
| `JsonFormatter` | Newline-delimited JSON events | `--output json` |
| `MinimalFormatter` | One line per step | `--output minimal` / CI logs |

## Dry-run macro

`taskit_dry!(fmt, ...)` — prints a `[dry-run]`-prefixed line without executing anything.
Used throughout the engine when `ctx.dry_run` is true.
