# Idea: TUI Coverage / Audit Surfacing

## Effort

Medium (1-3 days)

## Problem

`coverage --workspace` (added this session) and `audit` (cargo-deny wrapper) are
both top-level CLI subcommands with no TUI visibility at all — a user has to
shell out to see workspace coverage % or the latest audit status.

## Evidence

- `src/main.rs`: `Cmd::Coverage { crate_name, threshold, workspace }` and
  `Cmd::Audit` both dispatch to real engine logic
  (`testing::coverage::run_workspace`, `audit::run`).
- `crates/taskit-tui/src/app.rs`: `Tab::ALL` has no Coverage or Audit
  representation; Overview only surfaces CI duration drift + protocol drift.

## Proposed Direction

Rather than two more top-level tabs, fold both into the proposed Health tab
(`docs/ideas/2026-08-09-tui-health-tab.md`) as additional panels/rows:
- Coverage %: last workspace coverage run result + threshold pass/fail.
- Audit: last `cargo-deny` status (advisories/bans/licenses/sources), pulled
  from whatever `audit.rs` currently returns or logs.

This keeps `Tab::ALL` from growing unbounded while still giving the two new-ish
CLI capabilities (`--workspace`, and audit generally) a home in the TUI.

## Open Questions

- Does `audit::run` currently persist its last result anywhere the TUI could
  read without re-running cargo-deny on every snapshot refresh?
- Is workspace coverage measurement cheap enough to auto-refresh, or should it
  be manual-trigger-only in the TUI (same question as the Health tab idea)?
