# taskit Design Document

**Date:** 2026-05-13 (original), 2026-06-28 (revised)
**Status:** Approved

---

## Goal

A config-driven, standalone Rust binary for running CI pipelines in any Rust
workspace. Focused on affected-crate detection, protocol-drift tracking, and
pipeline orchestration. Users invoke `taskit` directly or via
`cargo taskit` (cargo alias).

---

## Crate Name

**`taskit`** (published on crates.io)

Binary name: `taskit`
Install: `cargo install taskit`
Usage: `taskit fmt`, `taskit lint`, `taskit ci`, etc.
Cargo alias: `cargo taskit ci` via `.cargo/config.toml`

---

## Architecture

### Workspace structure

Multi-crate workspace with hexagonal architecture (ports/adapters):

```
taskit/
  Cargo.toml              # workspace manifest
  src/main.rs             # binary entry point (clap CLI + dispatch)
  .cargo/config.toml      # alias: taskit = "run --package taskit --"
  taskit.toml              # self-hosting config for taskit's own CI
  taskit-protocol.lock     # protocol drift lockfile
  crates/
    taskit-types/          # leaf: Config, TaskitError, StepResult, OutputFormat
    taskit-core/           # ports: PipelineRunner trait
    taskit-engine/         # adapters: CI pipeline, config, formatters, steps
    taskit-init/           # `taskit init`: discovery + scaffold generation
    taskit-crux/           # EmbeddedCruxRunner (feature-gated, optional)
```

### Crate responsibilities

| Crate | Role |
|-------|------|
| `taskit` | Binary entry point; CLI parsing (clap) and dispatch |
| `taskit-types` | Leaf crate: Config, TaskitError (miette), StepResult, OutputFormat |
| `taskit-core` | Ports only: PipelineRunner trait |
| `taskit-engine` | CI pipeline, config loading, output formatters, step engine |
| `taskit-init` | InitPlan discovery, TOML/Cruxfile rendering, scaffold generation |
| `taskit-crux` | EmbeddedCruxRunner stub (feature-gated) |

### Dependency direction

```
taskit-types  (leaf, no internal deps)
    ^
taskit-core   (depends on taskit-types)
    ^
taskit-engine (depends on taskit-core + taskit-types)
taskit-init   (depends on taskit-types)
taskit-crux   (depends on taskit-core + taskit-types, feature-gated)
    ^
taskit        (binary, depends on all above)
```

### Hexagonal boundaries

- **Port**: `PipelineRunner` trait in `taskit-core`
- **Adapters**: `BuiltinRunner`, `SubprocessCruxRunner` in `taskit-engine`
- **Error types**: `TaskitError` with nested domain enums (ConfigError,
  PipelineError, ProtocolError, InitError, FlowError, StepError) using
  `miette::Diagnostic` + `thiserror::Error`
- **Internal**: `anyhow` used inside adapters, converted to `TaskitError`
  at public boundaries via `From<anyhow::Error>`

---

## Config Model

### `taskit.toml` (workspace root)

```toml
[workspace]
[[workspace.crates]]
dir = "my-common"
pkg = "my-common"        # optional, defaults to dir

[[workspace.crates]]
dir = "my-api"

[[workspace.propagation]]
source = "my-common"
dependents = ["my-api", "my-cli"]

[[protocol.surfaces]]
name = "api-types"
path = "my-api/src/types.rs"

[protocol]
lockfile = "taskit-protocol.lock"   # default

[[ci.steps]]
name = "fmt-check"
cmd = "fmt --check"
gate = true

[[ci.steps]]
name = "lint"
cmd = "lint"

[[ci.steps]]
name = "test"
cmd = "test"
```

Config discovery walks up from `$PWD` looking for `taskit.toml`. Zero-config
fallback derives `[[workspace.crates]]` from `cargo metadata`.

---

## CLI Surface

All subcommands accept `--dry-run` (global). Affected-crate filtering via
`--affected` where applicable.

**Subcommands:**

`fmt`, `lint`, `test`, `coverage`, `check-protocol-drift`,
`check-protocol-sites`, `quick`, `ci`, `compile-tests`, `check-deps`,
`check-freshness`, `pre-commit`, `pre-push`, `install-hooks`, `audit`,
`clean`, `health`, `inspect`, `publish`, `version`, `dev-setup`,
`self-check`, `self-test`, `update-claude-version`, `proptest`, `fuzz`,
`bench`, `test-report`, `snapshot-review`, `init`, `flow`

---

## Data Flow

```
taskit ci
  -> load Config from taskit.toml (or zero-config fallback)
  -> build Pipeline from [[ci.steps]]
       each step calls the corresponding command fn
       command fns receive &Config
  -> Pipeline::run() -> summary table -> exit code
```

`affected.rs::detect()` reads crate list and propagation from config.
`protocol::drift::run()` receives `&[SurfaceEntry]` from config.

---

## Cache

`.taskit-cache/` stores ephemeral per-step caches (pre-commit, pre-push,
compile, self-test). A master hash file (`.taskit-cache/master-hash`)
tracks SHA-256 integrity over all `.json` cache files. `taskit self-check`
verifies cache integrity. The cache is ephemeral and can be deleted at
any time via `taskit clean`.

---

## Init Scaffold

`taskit init` generates a complete project scaffold:

- `taskit.toml` with all sections (unused ones commented)
- `.cargo/config.toml` with `taskit` alias
- `.githooks/{pre-commit,pre-push}` calling `taskit` directly
- `.github/workflows/ci.yml` using `taskit ci`
- `deny.toml` for cargo-deny
- `docs/` mdBook scaffold with per-crate pages
- `.ctx/` directory structure for memory/sessions/tasks

Smart discovery auto-infers propagation from cargo metadata dep graph
and detects protocol surfaces from `pub trait` files.

---

## Tech Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Config format | TOML | Consistent with Cargo ecosystem |
| Config discovery | Walk up from `$PWD` | Same as `.cargo/config.toml` |
| Zero-config | `cargo metadata` | Low friction for new users |
| Error handling | `TaskitError` (miette) | Rich diagnostics at boundaries |
| Internal errors | `anyhow` in adapters | Ergonomic, converted at API surface |
| CLI | `clap` (derive) | Standard Rust CLI framework |
| Shell execution | `xshell` | Portable, quoted-arg safe |
| Testing | `cargo-nextest` | Fast, filterable test runner |

---

## Out of Scope

- Plugin system (config-driven only, no user Rust step functions)
- Remote task execution (local workspace only)
- Async runtime (all steps are synchronous shell invocations)
- Custom arbitrary shell commands in `[[ci.steps]]` (maps to built-in
  subcommands only)
