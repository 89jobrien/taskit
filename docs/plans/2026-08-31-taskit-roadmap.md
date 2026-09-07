# Taskit Roadmap (2026-08-31)

## Goal

Evolve `taskit` from a local CI/task runner into a workspace control plane with:

- stronger diagnosis and recovery,
- better release and policy intelligence,
- extensibility for team-specific workflows.

## Scope

This roadmap tracks the twelve roadmap TODOs added to
`crates/taskit-engine/src/command.rs`.

## Phase 1: Quick Wins (1-2 weeks)

### 1) `taskit doctor`

- Validate runtime/toolchain setup, hook installation, and common env failures.
- Emit actionable fixes per failed check.

### 2) `taskit plan`

- Print resolved command plan before execution.
- Include affected crates, skipped gates, and estimated runtime bands.

### 3) `taskit verify-docs`

- Detect CLI/config/docs drift (flags, command names, config keys).
- Fail with precise file-level mismatch diagnostics.

### 4) `taskit trend`

- Show local metric trends from telemetry (build duration, pass rate, warnings).
- Support a default 30-day window with compact terminal output.

## Phase 2: Reliability + Governance (2-4 weeks)

### 5) `taskit policy`

- Introduce `taskit-policy.toml` for gate policy.
- Support thresholds and deny rules (coverage floors, unsafe usage caps, banned deps).

### 6) `taskit release-impact`

- Analyze changed crates and dependency graph.
- Suggest semver bumps and highlight downstream compatibility risk.

### 7) `taskit quarantine`

- Detect flaky tests from historical runs.
- Mark/route flaky tests into quarantine reporting instead of hard pipeline failure.

### 8) `taskit fix`

- Optional safe auto-remediation pass.
- First targets: formatting, straightforward clippy fixes, lockfile/check metadata normalization.

## Phase 3: Platform Expansion (4-8 weeks)

### 9) `taskit plugin`

- Define stable plugin contract for custom commands/gates.
- Load team-specific extensions without forking core.

### 10) `taskit multi`

- Orchestrate workflows across multiple repositories.
- Coordinate shared release and compatibility checks.

### 11) `taskit attestation`

- Emit signed/provenance-friendly build and test metadata.
- Target release/compliance auditability.

### 12) `taskit bisect`

- Automate regression-cause commit identification.
- Integrate with failing test/gate replay logic.

## Execution Order

1. Build diagnosis + observability primitives (`doctor`, `plan`, `trend`).
2. Add governance and release intelligence (`policy`, `release-impact`).
3. Add failure-management and remediation (`quarantine`, `fix`, `bisect`).
4. Open extension and federation surfaces (`plugin`, `multi`, `attestation`).

## Definition of Done (per item)

- Command is documented in README and `taskit --help` output.
- Command has unit tests plus at least one integration test path.
- Dry-run behavior is explicit and validated.
- Failure output uses typed `TaskitError` variants where applicable.
