# Idea: Bench Orchestration with Per-Environment Baselines

## Effort

Large (> 3 days)

## Source

Grounded in `minibox/xtask/src/bench.rs` and its README.md Benchmarks section.

## Supersedes / Reconcile With

`docs/ideas/2026-08-09-ci-benchmark-gate.md` — that doc proposed simply
running `taskit bench` in CI. This idea is a superset: don't just run it, add
the baseline/regression infrastructure minibox already has. **Before
implementing either, reconcile the two into one doc** — this one should
likely absorb 2026-08-09's CI-wiring proposal as its "Proposed Direction"
step 3.

## Problem

`taskit bench` (via `testing/bench.rs`) exists but is never run in CI (see
the prior idea doc). Even if wired into CI, taskit has no mechanism to
compare a bench run against a historical baseline — every run is a fresh,
context-free number.

## Evidence

- `minibox/xtask/src/bench.rs`: runs criterion benchmarks, computes
  mean/median/p50/p95/p99, writes JSON/CSV output plus an HTML dashboard with
  history tracking (`perf_dashboard_template.html`).
- `minibox/xtask` README.md Benchmarks section: documents per-environment
  baselines stored at `bench/baseline.{local,selfhosted,hosted}.json`, with
  `--check` (compare against baseline, fail on regression) and
  `--save-baseline` flags.
- `taskit-engine/src/testing/bench.rs` (per prior scan) has no baseline
  comparison logic — compare to how `.health-baseline.json` +
  `health::check`/`print_metric` already implement exactly this
  regression-comparison pattern for health metrics, just not for bench
  timings.

## Proposed Direction

1. Reuse the `.health-baseline.json` comparison pattern
   (`health::check`/`print_metric`/`Direction`) as the template for a
   `.bench-baseline.json`, rather than inventing a new comparison mechanism.
2. Support per-environment baselines (`local`/`ci`) the way minibox does,
   since bench numbers on a laptop vs. a GitHub-hosted runner aren't
   comparable.
3. Wire `taskit bench --check` into `nightly.yml` (per the original Quick Win
   idea) once baseline comparison exists, rather than running raw numbers
   with nothing to compare against.
4. Skip the HTML dashboard initially — JSON/CSV output + a pass/fail gate
   covers the CI use case; the dashboard is a nice-to-have that can follow.

## Open Questions

- Does taskit's `testing/bench.rs` already collect the percentile stats
  minibox does, or just raw timings? (Not yet inspected — check before
  scoping.)
- Where should baseline files live — workspace root (matching
  `.health-baseline.json`) or a `bench/` subdirectory (matching minibox's
  convention)?
