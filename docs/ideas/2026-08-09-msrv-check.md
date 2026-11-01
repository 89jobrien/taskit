# Idea: Add MSRV Check to CI

## Effort

Quick Win (< 1 day)

## Problem

`taskit` is a published, multi-crate workspace (see `[workspace.package]` and the
`publish.yml` release workflow) but no CI job pins or verifies a Minimum Supported
Rust Version. A contributor could unknowingly rely on a newer Rust feature and
break downstream consumers building on an older toolchain.

## Evidence

- `.github/workflows/ci.yml`: single ubuntu job, no `rust-version` field checked
  or `cargo msrv` step.
- `Cargo.toml` (workspace root): no `rust-version` field observed in the scan.

## Proposed Direction

1. Determine actual MSRV empirically (`cargo msrv find` or bisect against
   edition 2021/2024 features actually used).
2. Set `rust-version = "X.Y"` in `[workspace.package]`.
3. Add a CI job (or step in the existing `ci.yml` job) that installs the pinned
   MSRV toolchain via `dtolnay/rust-toolchain` and runs `cargo check --workspace`.

## Open Questions

- Does the release matrix in `release.yml` (4 targets) already imply a floor
  toolchain version worth reusing here?
- Any dependency (e.g. `ratatui`, `baml`) with a known MSRV that should set the
  floor?
