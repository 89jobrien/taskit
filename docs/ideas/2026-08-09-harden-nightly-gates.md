# Idea: Harden Nightly CI Gates

## Effort

Quick Win (< 1 day)

## Problem

Every job in `nightly.yml` — `audit`, `deny`, `geiger`, `coverage`, `mutants` — is
marked `continue-on-error: true`. A real CVE (via `cargo audit`) or a newly
introduced `unsafe` block (via `geiger`) would not fail the workflow or otherwise
loudly notify anyone; the job just shows a soft yellow warning in the Actions UI.

## Evidence

- `.github/workflows/nightly.yml`: all five jobs have `continue-on-error: true`.

## Proposed Direction

Split nightly jobs into two tiers:
- **Blocking**: `audit` (cargo-audit, CVEs) and `deny` (cargo-deny — licenses,
  bans, sources) should fail the workflow run on violation.
- **Informational**: `geiger` (unsafe-code counts), `coverage` (trend, not a hard
  gate), and `mutants` (expensive, weekly-only) can stay soft.

Add a Slack/GitHub-issue notification step (or reuse `todo-sync`-style GitHub
issue creation) when a blocking job fails, since nightly runs aren't watched the
way PR checks are.

## Open Questions

- Is there a reason all jobs were made soft originally (e.g. flaky cargo-deny
  advisories)? Check git blame on `nightly.yml` before flipping.
- Should `deny` failures create a GitHub issue automatically, mirroring the
  `todo-sync` pattern already in `taskit-engine/src/todo_sync.rs`?
