# Idea: `check no-unwrap` Gate

## Effort

Quick Win (< 1 day)

## Source

Grounded in `minibox/xtask` (`xtask check no-unwrap`).

## Problem

`taskit health` already *counts* `.unwrap()`/`.expect()` call sites via
`SafetyCounts` (added this session in `health.rs::count_safety`), and
`taskit health` reports regressions in that count vs. the baseline. But it's
purely informational — there's no hard gate that fails CI when unwrap/expect
usage crosses a threshold or appears in new code, unlike minibox's dedicated
`check no-unwrap` command.

## Evidence

- `crates/taskit-engine/src/health.rs`: `SafetyCounts { unwrap_count,
  warn_count }`, `count_safety`/`count_safety_markers` — counts only, surfaced
  via `health` regression reporting (`print_metric` with
  `Direction::LowerIsBetter`), not a standalone pass/fail gate.
- `minibox/xtask check no-unwrap` — a standalone check subcommand implying
  hard pass/fail semantics independent of the broader health baseline flow.

## Proposed Direction

Two options, worth deciding rather than assuming:

1. **Threshold-based**: extend `[inspect]` config (which already has
   `max_clippy_warnings`, `max_todo_fixme`, etc. per `CLAUDE.md`'s Config
   Reference) with `max_unwrap_expect`, reusing the existing `count_safety`
   logic and `inspect.rs`'s pipeline/threshold pattern rather than adding a
   parallel command.
2. **New-code-only gate**: fail only on `.unwrap()`/`.expect()` introduced in
   the current diff (via `git diff`), which is closer to what "gate on new
   code" tooling usually wants and avoids requiring a repo-wide cleanup before
   the gate can be turned on.

Given `inspect` already exists as taskit's threshold-gate mechanism, option 1
is the smaller, more consistent change.

## Open Questions

- Does minibox's `no-unwrap` check use an absolute-zero threshold, or does it
  allow some baseline count (worth checking `xtask/src/lint_paths.rs` or
  wherever it's implemented, not yet inspected)?
- Should `#[allow]`-style suppression markers be supported for justified
  unwraps (e.g. `// qual:allow` convention referenced in the `rustqual` skill
  used elsewhere in this workspace)?
