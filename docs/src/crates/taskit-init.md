# taskit-init

`taskit-init` owns workspace discovery, configuration rendering, and project scaffolding. It is
separate from `taskit-engine` so generation does not become an engine responsibility.

## Public modules

| Module            | Responsibility                                                             |
| ----------------- | -------------------------------------------------------------------------- |
| `plan`            | `InitPlan`, Cargo metadata discovery, prompts, and inferred relationships. |
| `render_toml`     | New `taskit.toml` text generation from `InitPlan`.                         |
| `render_cruxfile` | `ci.crux` YAML generation (opt-in, gated by `InitPlan.crux`).              |
| `scaffold`        | Hooks, CI, deny config, context, mdBook, and optional xtask files.         |

The crate root also exposes the `run(force, interactive, dry_run)` orchestration function.

## Discovery

Initialization discovers workspace packages, package-directory remapping, local dependency
propagation, top-level `pub trait` protocol surfaces, release order, and a GitHub origin when
available.

## Generated files

Default initialization writes `taskit.toml`, a `cargo taskit` alias, hooks, GitHub CI,
`deny.toml`, `.ctx/`, and an mdBook scaffold. The crux pipeline file (`ci.crux`) is generated only
when crux pipelines are enabled (`InitPlan.crux`); it is disabled by default. Isolated `xtask/`
generation is optional and disabled by default.

See [First Workspace](../getting-started/first-workspace.md) for commands and the current generated
CI spelling caveat.
