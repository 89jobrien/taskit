# Design: CI, Release, and MSRV Hardening

## Goal

Remove competing publication paths, make security failures visible, and continuously verify the workspace's declared minimum Rust version.

## Approved Approach

Keep manually dispatched `release.yml` as the sole publisher, make audit and deny blocking nightly gates, and verify Rust 1.88 as the locked workspace MSRV.

## Context Map

### Files to Modify

| File                            | Purpose                                             | Changes Needed                                                                                                  |
| ------------------------------- | --------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| `.github/workflows/publish.yml` | Root-only tag publisher                             | Remove after confirming `release.yml` owns tag publication                                                      |
| `.github/workflows/release.yml` | Release gates, artifacts, and workspace publication | Remain the sole publishing workflow                                                                             |
| `.github/workflows/nightly.yml` | Security and exploratory gates                      | Make audit and deny blocking while retaining informational jobs                                                 |
| `.github/workflows/ci.yml`      | Pull-request validation                             | Add an MSRV check job using the committed lockfile                                                              |
| `Cargo.toml`                    | Workspace and root package policy                   | Declare `rust-version` in `[workspace.package]` and add `rust-version.workspace = true` to the root `[package]` |
| `crates/*/Cargo.toml`           | Published package manifests                         | Inherit `rust-version.workspace = true`                                                                         |
| `xtask/Cargo.toml`              | Workspace utility manifest                          | Inherit the workspace Rust version when compatible                                                              |

### Dependencies

The MSRV is Rust 1.88: Rust 2024 requires at least 1.85, while the current locked dependency graph declares requirements up to 1.88. Release publication order remains owned by `taskit release publish`.

### Test Coverage

Workflow syntax validation and the MSRV `cargo check --locked --workspace` job are the acceptance tests. Existing stable-toolchain CI remains unchanged.

### Reference Patterns

Follow the existing stable toolchain and cache setup in `.github/workflows/ci.yml`, and the release gate ordering in `.github/workflows/release.yml`.

## Crate Ownership

This stage changes workspace policy and GitHub Actions only; no Rust crate owns new runtime behavior.

## Public API

No Rust API changes.

## Data Flow

1. Pull requests run stable CI and a separate Rust 1.88 check using `cargo check --locked --workspace`.
2. Nightly audit and deny failures fail the workflow; geiger, coverage, and mutants remain informational.
3. A maintainer manually dispatches `release.yml`; that workflow runs gates, creates the tag, builds artifacts, and publishes the ordered workspace crates.

## Hexagonal Boundaries

Not applicable: this design changes repository automation, not runtime adapters.

## Out of Scope

- Windows support.
- Automated dependency upgrades.
- Changing crate publication order.
- Direct notifications outside GitHub Actions status.

## Risk

- [ ] Breaking API changes: no.
- [ ] New external dependency: no.
- [ ] Workflow permission changes: no additional write permissions.
- [ ] Release behavior change: duplicate root publication is removed.
