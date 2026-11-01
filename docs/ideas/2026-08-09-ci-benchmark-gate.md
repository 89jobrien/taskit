# Idea: CI Benchmark Gate

## Effort

Quick Win (< 1 day)

## Problem

`taskit bench` exists as a CLI subcommand and `testing/bench.rs` implements it, but no
CI workflow ever runs it. Performance regressions in the pipeline engine or hot paths
can land undetected.

## Evidence

- `src/main.rs`: `Cmd::Bench` is a real subcommand, dispatched to `testing::bench`.
- `crates/taskit-engine/src/testing/bench.rs` implements bench execution.
- `.github/workflows/ci.yml` and `.github/workflows/nightly.yml`: no step invokes
  `taskit bench` anywhere.

## Proposed Direction

Add a `bench` job to `nightly.yml` (matching the existing `mutants` job's weekly/
non-blocking cadence) that runs `taskit bench` and uploads results as an artifact.
Consider storing a baseline (similar to `.health-baseline.json`) and failing only on
large regressions, not absolute thresholds.

## Open Questions

- Does `taskit bench` already persist historical results, or would this need new
  baseline-comparison logic (see `telemetry.rs` / `Drift` command for a precedent)?
- Should this block PRs or stay informational (like the other nightly jobs)?
