# Design: Complete mdBook User Guide

## Goal

Turn the existing crate-oriented mdBook into an accurate end-to-end user guide for installing,
configuring, operating, and understanding taskit.

## Approved Approach

Build the approved complete user guide around current behavior only, while correcting the blocking
documentation drift already identified against the CLI and source.

## Context Map

### Navigation and Introduction

| File                  | Change                                                                                           |
| --------------------- | ------------------------------------------------------------------------------------------------ |
| `docs/src/SUMMARY.md` | Replace the sparse navigation with Getting Started, Guides, Reference, Architecture, and Crates. |
| `docs/src/README.md`  | Rewrite the introduction around taskit's current grouped CLI and core workflows.                 |

### New User Guides

| File                                          | Responsibility                                                                  |
| --------------------------------------------- | ------------------------------------------------------------------------------- |
| `docs/src/getting-started/installation.md`    | Installation, prerequisites, and command discovery.                             |
| `docs/src/getting-started/first-workspace.md` | Initialize a workspace and run the first quick/CI checks.                       |
| `docs/src/guides/quality-ci.md`               | Formatting, linting, compilation, quick checks, CI, and hooks.                  |
| `docs/src/guides/testing.md`                  | Nextest, coverage, proptest, fuzz, bench, reports, and snapshots.               |
| `docs/src/guides/health.md`                   | Baseline collection, updates, coverage, gate mode, drift, inspect, and version. |
| `docs/src/guides/protocol.md`                 | Contract drift, construction sites, TODO sync, freshness, and audit.            |
| `docs/src/guides/flow.md`                     | Status, sync, promote, auto, guard, state, push, and conflict resolution.       |
| `docs/src/guides/dashboard.md`                | Four tabs, project context, keyboard controls, and health actions.              |

### Reference

| File                                   | Change                                                                                                 |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `docs/src/reference/cli.md`            | Document every current command group, leaf command, argument, and leaf-specific option from Clap help. |
| `docs/src/reference/configuration.md`  | Cover every current `Config` section and field from `taskit-types`.                                    |
| `docs/src/reference/output-formats.md` | Document all seven output formats and generated report paths.                                          |
| `docs/src/reference/ci-pipeline.md`    | Correct the self-check command and describe the current default pipeline and gate behavior.            |

### Architecture and Crates

| File                                | Change                                                                       |
| ----------------------------------- | ---------------------------------------------------------------------------- |
| `docs/src/architecture/overview.md` | Add taskit-tui, current dependency edges, state flow, and adapter status.    |
| `docs/src/crates/taskit.md`         | Correct source paths and binary responsibilities.                            |
| `docs/src/crates/taskit-core.md`    | Remove the nonexistent Crux feature claim and correct conformance ownership. |
| `docs/src/crates/taskit-engine.md`  | Reflect all public modules and the actual `Ctx` fields.                      |
| `docs/src/crates/taskit-output.md`  | Correct formatter wiring and generated output paths.                         |
| `docs/src/crates/taskit-testing.md` | Replace removed helpers with `TempDirGuard` and exported macros.             |
| `docs/src/crates/taskit-macros.md`  | Document the actual test/config proc macros and current consumers.           |
| `docs/src/crates/taskit-types.md`   | Add current flow configuration and error variants.                           |
| `docs/src/crates/taskit-crux.md`    | Keep the adapter explicitly documented as an unwired stub.                   |
| `docs/src/crates/taskit-init.md`    | Verify and retain the current initialization/scaffolding reference.          |
| `docs/src/crates/taskit-tui.md`     | Retain the reviewed dashboard API reference and link it from the guide.      |

## Source of Truth

1. Command names and options come from `taskit --help`, every group `--help`, and every leaf
   command `--help`.
2. Configuration comes from `crates/taskit-types/src/config.rs` and generated TOML in
   `crates/taskit-init/src/render_toml.rs`.
3. Runtime behavior comes from `src/main.rs` and the owning `taskit-engine` modules.
4. Public crate APIs come from each crate's `Cargo.toml`, `src/lib.rs`, and reachable public items.
5. Dashboard behavior comes from the reviewed `taskit-tui` source and reference page.

## Public API

No Rust API, CLI, configuration, or serialization changes are introduced. This work changes only
Markdown under `docs/`.

## Data Flow

Current source and Clap help feed topic-specific guide/reference pages; `SUMMARY.md` provides one
published navigation tree; `mdbook build docs` verifies all pages and links render together.

## Out of Scope

- Root `README.md`, `CLAUDE.md`, release notes, design archives, plans, ideas, and handoff files.
- New taskit commands, configuration, examples, or behavior.
- Exhaustive rustdoc replacement for every internal public item; crate pages remain architectural
  references, with `taskit-tui` retaining its focused API reference.
- Publishing the generated HTML output.

## Risk

- [ ] Breaking API changes: no.
- [ ] New dependencies: no.
- [ ] Feature flag required: no.
- [ ] More than three files: yes, explicitly approved as a complete guide expansion.
- [ ] Primary risk: stale or invented claims; every page requires source/help cross-check and final
      `godmode:doc-review`.
