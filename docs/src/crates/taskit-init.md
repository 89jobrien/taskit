# taskit-init

Handles `taskit init` — workspace discovery and scaffold file generation. Deliberately
separate from `taskit-engine` to keep the engine free of generation concerns.

## Modules

### `plan.rs`

- `InitPlan` — all decisions made before writing any file (crate list, propagation graph,
  protocol surfaces, flow defaults)
- `plan_from_discovery()` — builds an `InitPlan` by running `cargo metadata` and inspecting
  the workspace
- `plan_interactive()` — prompts the user to confirm or override each plan decision

### `render_toml.rs`

Hand-built TOML renderer for `taskit.toml`. Uses hand-crafted string formatting (not a TOML
serializer) so comments and section ordering are preserved exactly.

### `render_cruxfile.rs`

Generates the `Cruxfile` YAML that drives `crux`-based step execution.

## What `taskit init` generates

| File | Purpose |
|------|---------|
| `taskit.toml` | Full config with unused sections commented out |
| `Cruxfile` | Step definitions for crux runner |
| `xtask/` | Cargo xtask shim |
| `.cargo/config.toml` | `cargo xtask` alias |
| `.githooks/pre-commit` | Pre-commit hook delegate |
| `.githooks/pre-push` | Pre-push hook delegate |
| `.github/workflows/ci.yml` | GitHub Actions CI (nextest, rust-cache) |
| `deny.toml` | cargo-deny: licenses, advisories, bans, sources |
| `docs/` | mdBook skeleton with per-crate pages |
| `.ctx/` | Context directory with sessions, tasks, memory-bank, xcache |

Smart discovery: propagation rules are auto-inferred from the Cargo dep graph; protocol
surfaces are auto-detected from `pub trait` declarations.
