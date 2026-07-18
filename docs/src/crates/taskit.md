# taskit (binary)

The root binary crate. Responsible for CLI parsing, adapter wiring, and dispatch to engine
functions. Returns `miette::Result<()>` for rich terminal error rendering.

## `src/main.rs`

Entry point. Parses CLI args with `clap`, loads `Config` via `taskit-engine/config.rs`, then
dispatches to the appropriate engine function. `taskit init` is handled before config load
(the config doesn't exist yet).

## `src/flow_resolver.rs`

`BamlConflictResolver` — the LLM-backed `ConflictResolver` adapter. Wired only at the binary
boundary to keep BAML out of the engine and core crates.

Uses BAML structured output to call an LLM with the ours/theirs/base content of each
conflicted file and returns `Vec<ResolvedFile>`. Unresolvable cases propagate
`FlowError::NeedsHuman`.

## CLI flags (global)

| Flag | Description |
|------|-------------|
| `--dry-run` | Print commands without executing |
| `--output <fmt>` | `pretty` (default) \| `json` \| `minimal` |
| `--config <path>` | Override `taskit.toml` location |

See [Common Subcommands](../README.md#quick-start) for the full command list.
