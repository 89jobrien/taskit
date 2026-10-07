# taskit-init

`taskit-init` discovers a Cargo workspace and generates taskit configuration, a Crux pipeline, and
optional repository scaffolding. It is separate from `taskit-engine` because initialization must
run before `taskit.toml` can be loaded into an engine `Ctx`.

## Workspace role

The root binary calls `taskit_init::run` for `taskit init`. The crate depends on `taskit-types`,
`cargo_metadata`, and `dialoguer`; it has no Cargo feature flags.

The current renderers use taskit's former flat command spelling in generated CI and Crux content,
for example `taskit fmt --check`, `taskit test`, and `taskit ci`. The current CLI groups these under
`taskit check` and `taskit test`; generated command lines therefore require correction before they
work with the current binary. The same compatibility issue affects generated pre-commit/pre-push
hooks, the GitHub Actions workflow, and the optional xtask adapter.

```bash
taskit init
taskit init --interactive
taskit init --dry-run
taskit init --force
```

`--dry-run` is taskit's global flag. Discovery and existing-file reads still occur, but generated
files are reported instead of written. Without `--force`, an existing `taskit.toml` stops the
entire operation with `InitError::AlreadyExists`. Other scaffold writers generally skip existing
targets unless forced.

## Planning and discovery

`InitPlan` is the intermediate representation consumed by all renderers. It records:

- workspace crates and package-name remaps;
- inferred local dependency propagation;
- detected protocol surfaces;
- coverage, CI, offline-test, flow, and release settings; and
- booleans for hooks, GitHub CI, deny config, context files, mdBook, and xtask scaffolding.

`plan_from_discovery` uses Cargo metadata, detects top-level public trait surfaces, infers local
dependency propagation and publish order, and tries to detect a GitHub repository from git remote
configuration. Its current defaults enable hooks, GitHub CI, `deny.toml`, `.ctx`, and mdBook
scaffolding. It enables the default flow/release plan, leaves coverage/offline filtering unset, and
disables xtask generation.

`plan_interactive` starts with the discovered plan and prompts for optional coverage, offline test
filtering, flow branches, release settings, and scaffold choices.

Inspect discovery without writing files:

```bash
cargo run -p taskit-init --example discover_plan
```

## Renderers

`render_toml(&InitPlan)` produces a full `taskit.toml`. Configured sections are active; available
but unconfigured sections are emitted as commented examples. The renderer covers workspace,
propagation, protocol, coverage, CI, inspect, clean, flow, and release sections.

`render_cruxfile(&InitPlan, project_name)` emits a Crux YAML pipeline. Each CI step becomes a
`shell::exec` step whose command is `taskit <configured cmd>`. Human step names are normalized into
lowercase underscore identifiers. An empty CI plan falls back to one `taskit ci` step.

## Generated files

Depending on `InitPlan`, `run` can create or update:

| Output                               | Writer                                                  |
| ------------------------------------ | ------------------------------------------------------- |
| `taskit.toml`                        | `render_toml` through the crate root orchestration.     |
| `Cruxfile`                           | `render_cruxfile` through the crate root orchestration. |
| `.cargo/config.toml`                 | Adds the `cargo taskit` alias.                          |
| `.githooks/*`                        | `write_git_hooks`.                                      |
| `.github/workflows/ci.yml`           | `write_github_ci`.                                      |
| `deny.toml`                          | `write_deny_toml`.                                      |
| `.ctx/*`                             | `write_ctx_scaffold`.                                   |
| `docs/book.toml` and `docs/src/*`    | `write_mdbook`.                                         |
| `xtask/Cargo.toml` and `xtask/src/*` | `write_xtask`, when explicitly enabled.                 |

The `.cargo/config.toml` writer appends an alias section to existing content when no taskit alias is
found. The xtask writer uses sentinel comments to preserve non-managed code and refuses ambiguous
partial managed sections rather than silently replacing them.

## Public API

| API                                | Purpose                                         |
| ---------------------------------- | ----------------------------------------------- |
| `run(force, interactive, dry_run)` | Execute complete initialization.                |
| `plan_from_discovery`              | Build a non-interactive `InitPlan`.             |
| `plan_interactive`                 | Build a prompted `InitPlan`.                    |
| `InitPlan::default_steps`          | Return the seven built-in CI step descriptions. |
| `render_toml`                      | Render taskit configuration text.               |
| `render_cruxfile`                  | Render Crux YAML text.                          |
| `scaffold::write_*`                | Write individual scaffold groups.               |

```rust
use taskit_init::plan::plan_from_discovery;
use taskit_init::render_toml::render_toml;

let plan = plan_from_discovery()?;
let text = render_toml(&plan);
assert!(text.contains("[workspace]"));
# Ok::<(), taskit_types::error::TaskitError>(())
```

## Development and testing

```bash
cargo check -p taskit-init
cargo clippy -p taskit-init --all-targets -- -D warnings
cargo nextest run -p taskit-init
```

Tests cover discovery, renderer parse round-trips, scaffold skip/force/dry-run behavior, generated
Rust formatting, managed xtask updates, and generated file inventories. Scaffold tests change the
process current directory; keep their isolation requirements in mind when changing test helpers.
