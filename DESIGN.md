# taskit Design Document

**Date:** 2026-05-13
**Status:** Approved

---

## Goal

Publish the current maestro-internal `xtask` crate as `taskit` — a config-driven, standalone
Rust binary that works out of the box for any Rust workspace, similar in spirit to `cargo-make`
but narrower in scope: focused on the `cargo xtask` pattern with first-class support for
affected-crate detection, protocol-drift tracking, and CI pipeline orchestration.

---

## Crate Name

**`taskit`** (crates.io verified available as of 2026-05-13)

Binary name: `taskit`
Install: `cargo install taskit`
Usage: `taskit fmt`, `taskit lint`, `taskit ci`, etc.

`cargoflow` and `rundown` are also available. `taskit` is preferred for being short, memorable,
and clearly in the "task orchestration" namespace without implying it is cargo-make or a general
shell runner.

---

## Architecture

### Crate structure

Single crate, single binary. No workspace split at this stage (YAGNI — a `taskit-core` lib crate
can be extracted later if embedding use-cases emerge).

```
taskit/
  Cargo.toml          # name = "taskit", publish = true
  taskit.toml          # default workspace config (bundled as example/docs)
  src/
    main.rs           # CLI entry point — reads taskit.toml, dispatches commands
    config.rs         # Config types: WorkspaceConfig, CrateEntry, SurfaceEntry
    pipeline.rs       # Pipeline / PipelineStep / StepResult (from step.rs, no changes)
    runner.rs         # xrun / xrun_ok / dry-run flag (from runner.rs, no changes)
    affected.rs       # Affected-crate detector — caller-supplied crate list (refactored)
    protocol/
      mod.rs
      drift.rs        # Hash/lockfile/compare engine — surfaces supplied from config
      contract_hash.rs
      sites.rs
    commands/
      mod.rs
      fmt.rs
      lint.rs
      test.rs
      ci.rs
      quick.rs
      audit.rs
      clean.rs
      hooks.rs
      version.rs
      dev_setup.rs
      coverage.rs
      check_deps.rs
      check_freshness.rs
      protocol_drift.rs
      update_claude.rs
    progress.rs
    cache/
      mod.rs
    util.rs
```

### What is removed vs. what stays

| Module                   | Disposition       | Reason                                                 |
| ------------------------ | ----------------- | ------------------------------------------------------ |
| `testing/k8s.rs`         | Removed           | Maestro-specific K8s integration                       |
| `testing/docker.rs`      | Removed           | Maestro-specific Docker integration                    |
| `testing/smolvm.rs`      | Removed           | Maestro-specific SmolVM                                |
| `testing/smoke.rs`       | Removed           | Maestro-specific staging/prod endpoints                |
| `testing/conformance.rs` | Removed           | Maestro-specific contract tests                        |
| `testing/run.rs`         | Kept, generalized | Nextest runner — works for any workspace               |
| `testing/coverage.rs`    | Kept, generalized | llvm-cov — works for any workspace                     |
| `testing/compile.rs`     | Kept              | Generic compile-test-binaries step                     |
| `testing/bench.rs`       | Kept              | Criterion runner — generic                             |
| `testing/proptest.rs`    | Kept              | Generic                                                |
| `testing/fuzz.rs`        | Kept              | Generic                                                |
| `testing/report.rs`      | Kept              | Generic                                                |
| `testing/snapshot.rs`    | Kept              | Insta — generic                                        |
| `testing/self_test.rs`   | Kept              | Tests taskit itself                                    |
| `schema.rs`              | Removed           | GraphQL schema dump is maestro-specific                |
| `update_claude.rs`       | Kept              | Generic enough — finds/updates version pins            |
| `affected.rs`            | Refactored        | Remove hardcoded constants; read from config           |
| `protocol/drift.rs`      | Refactored        | Remove hardcoded SURFACES; read from config            |
| `main.rs`                | Replaced          | New generic main; remove K8s/Docker/SmolVM/Schema cmds |

### Maestro's xtask after extraction

Maestro keeps a thin `xtask/src/main.rs` that:

1. Calls `taskit` as a library (if/when a lib surface is added) OR
2. Keeps its own binary that delegates generic steps to the taskit binary and adds
   maestro-specific subcommands (`schema`, `test-k8s`, `test-docker`, `smoke-test`, etc.)

For the initial release, maestro's xtask stays as-is. The extraction is additive — taskit is
published as a new binary, maestro's xtask is not deleted until taskit is proven in production.

---

## Config Model

### `taskit.toml` (workspace root)

```toml
[workspace]
# Optional: override workspace root detection (default: CARGO_MANIFEST_DIR/../)
# root = "."

# Ordered list of crates for affected-crate detection and pipeline steps.
# 'dir' is the directory name relative to workspace root.
# 'pkg' is the Cargo package name (defaults to 'dir' if omitted).
[[workspace.crates]]
dir = "my-common"
pkg = "my-common"        # optional, defaults to dir

[[workspace.crates]]
dir = "my-api"

# Dependency propagation: if 'source' crate changes, 'dependents' are also affected.
[[workspace.propagation]]
source = "my-common"
dependents = ["my-api", "my-cli"]

# Protocol-drift surfaces: files whose content hash is tracked in a lockfile.
[[protocol.surfaces]]
name = "api-types"
path = "my-api/src/types.rs"

[[protocol.surfaces]]
name = "cli-commands"
path = "my-cli/src/commands/mod.rs"

[protocol]
lockfile = "taskit-protocol.lock"   # default

# CI pipeline: ordered list of named steps with their taskit subcommand.
# 'gate = true' means all subsequent steps are skipped if this one fails.
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

[[ci.steps]]
name = "protocol-drift"
cmd = "check-protocol-drift"
```

### Config type hierarchy (Rust)

```rust
// config.rs
#[derive(Debug, Deserialize)]
pub struct Config {
    pub workspace: WorkspaceConfig,
    pub protocol: Option<ProtocolConfig>,
    pub ci: Option<CiConfig>,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceConfig {
    pub root: Option<PathBuf>,
    pub crates: Vec<CrateEntry>,
    pub propagation: Vec<PropagationEntry>,
}

#[derive(Debug, Deserialize)]
pub struct CrateEntry {
    pub dir: String,
    pub pkg: Option<String>,  // defaults to dir
}

#[derive(Debug, Deserialize)]
pub struct PropagationEntry {
    pub source: String,
    pub dependents: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProtocolConfig {
    pub surfaces: Vec<SurfaceEntry>,
    pub lockfile: Option<String>,  // default: "taskit-protocol.lock"
}

#[derive(Debug, Deserialize)]
pub struct SurfaceEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct CiConfig {
    pub steps: Vec<CiStep>,
}

#[derive(Debug, Deserialize)]
pub struct CiStep {
    pub name: String,
    pub cmd: String,
    pub gate: Option<bool>,
}
```

`config.rs` owns discovery: walk up from `$PWD` looking for `taskit.toml`; fall back to a
zero-config mode where `[[workspace.crates]]` is derived from `cargo metadata`.

---

## CLI Surface

The binary exposes the same subcommands as the current xtask, minus the maestro-specific ones.
All subcommands accept `--dry-run` (global) and `--affected` (where applicable).

**Subcommands kept:**

`fmt`, `lint`, `test`, `coverage`, `check-protocol-drift`, `check-protocol-sites`, `quick`,
`ci`, `compile-tests`, `check-deps`, `check-freshness`, `pre-commit`, `pre-push`,
`install-hooks`, `audit`, `clean`, `version`, `dev-setup`, `self-check`, `self-test`,
`update-claude-version`, `proptest`, `fuzz`, `bench`, `test-report`, `snapshot-review`

**Subcommands removed (maestro-specific):**

`schema`, `smoke-test`, `test-conformance`, `test-docker`, `test-k8s`, `test-smolvm`,
`cleanup-e2e-namespaces`, `test-e2e`

---

## Data Flow

```
taskit ci
  └─ load Config from taskit.toml
  └─ build Pipeline from [[ci.steps]]
        each step calls the corresponding command fn
        command fns receive &Config (for crate list, surface list, etc.)
  └─ Pipeline::run() → summary table → exit code
```

`affected.rs::detect(sh, &config.workspace)` replaces the hardcoded `ALL_CRATES`/`PROPAGATION`
constants with slices built from the config at runtime.

`protocol::drift::run(root, &config.protocol, ...)` receives a `&[SurfaceEntry]` slice built
from the config instead of the hardcoded `SURFACES` constant.

---

## Tech Decisions

| Decision             | Choice                                   | Rationale                                          |
| -------------------- | ---------------------------------------- | -------------------------------------------------- |
| Config format        | TOML via `toml` crate                    | Consistent with Cargo; already in Rust ecosystem   |
| Config discovery     | Walk up from `$PWD`                      | Standard convention (same as `.cargo/config.toml`) |
| Zero-config fallback | `cargo metadata` for crate list          | Lowers friction for first-time users               |
| Affected detection   | `git diff origin/main...HEAD`            | No change from current; works universally          |
| Pipeline             | Current `Pipeline` / `step.rs` unchanged | Already correct and well-tested                    |
| Shell execution      | `xshell` unchanged                       | Portable, quoted-arg safe                          |
| New dependency       | `toml = "0.8"`                           | Only new crate added                               |
| Maestro xtask        | Stays as-is, not deleted                 | Additive migration; no regression risk             |

---

## Migration Plan

1. Rename crate: `name = "taskit"`, `publish = true` in `Cargo.toml`.
2. Refactor `affected.rs`: remove `ALL_CRATES`/`PROPAGATION` constants; accept
   `&WorkspaceConfig` parameter in `detect()` and `apply_propagation()`.
3. Refactor `protocol/drift.rs`: remove `SURFACES` constant; accept `&[SurfaceEntry]`
   parameter in `calculate_lockfile()` and `run()`.
4. Add `config.rs` with `Config` types and `toml` dependency.
5. Add `taskit.toml` loader in `main.rs`: find config, deserialize, pass to all commands.
6. Delete `testing/{k8s,docker,smolvm,smoke,conformance}.rs` and `schema.rs`.
7. Remove the corresponding CLI subcommands from `main.rs`.
8. Update `Cargo.toml`: `publish = true`, add `toml` dep, add description/license/repository.
9. Write `README.md` with install + quick-start + `taskit.toml` reference.
10. Publish `0.1.0` to crates.io.

Maestro's xtask is updated in a follow-on step: add a `taskit.toml` at the maestro workspace
root encoding the existing hardcoded crate list and surfaces, then point maestro's xtask at the
`taskit` binary for generic steps.

---

## Out of Scope

- Library (`taskit-core`) extraction — not needed until an embedding use-case exists.
- Plugin system — no user-supplied Rust step functions; config-driven only.
- Remote task execution — local workspace only.
- Windows support — not tested; no explicit CI gate added (can be added later).
- Replacing maestro's xtask in this PR — additive only; maestro xtask stays intact.
- Async runtime — all steps are synchronous shell invocations via `xshell`.
- Custom step commands beyond the built-in set — `[[ci.steps]]` maps to built-in subcommands
  only; arbitrary shell commands are out of scope for v0.1.

---

## Directory Layout After Refactor

```
/Users/joe/dev/taskx/
  Cargo.toml            # name = "taskit", publish = true
  DESIGN.md             # this file
  README.md             # install + quick-start
  taskit.toml            # example/self-hosting config for taskit's own CI
  protocol-drift.lock   # renamed: taskit-protocol.lock
  src/
    main.rs
    config.rs           # NEW
    pipeline.rs         # renamed from step.rs
    runner.rs
    affected.rs         # refactored (no hardcoded crates)
    progress.rs
    util.rs
    cache/mod.rs
    protocol/
      mod.rs
      drift.rs          # refactored (no hardcoded surfaces)
      contract_hash.rs
      sites.rs
    commands/           # renamed from individual top-level modules
      mod.rs
      fmt.rs
      lint.rs
      test.rs           # from testing/run.rs
      coverage.rs       # from testing/coverage.rs
      ci.rs
      quick.rs
      audit.rs
      clean.rs
      hooks.rs
      version.rs
      dev_setup.rs
      check_deps.rs
      check_freshness.rs
      protocol_drift.rs # from main.rs dispatch + protocol/drift.rs
      update_claude.rs
      bench.rs          # from testing/bench.rs
      proptest.rs       # from testing/proptest.rs
      fuzz.rs           # from testing/fuzz.rs
      report.rs         # from testing/report.rs
      snapshot.rs       # from testing/snapshot.rs
      compile.rs        # from testing/compile.rs
      self_test.rs      # from testing/self_test.rs
```
