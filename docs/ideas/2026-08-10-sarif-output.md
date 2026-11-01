# Idea: SARIF Output for Lint / Protocol Findings

## Effort

Medium (1-3 days)

## Source

Grounded in `minibox/xtask/src/clippy_sarif.rs` and `xtask/src/sarif.rs`.

## Problem

`taskit lint` and `taskit inspect` surface findings via `taskit-output`'s
human/structured formats, but neither integrates with GitHub Code Scanning
(SARIF), which is the standard format GitHub Actions consumes for inline PR
annotations and the Security tab.

## Evidence

- `minibox/xtask/src/sarif.rs`: a minimal SARIF 2.1.0 builder.
- `minibox/xtask/src/clippy_sarif.rs`: converts clippy JSON diagnostics to
  SARIF, presumably uploaded via `github/codeql-action/upload-sarif` in CI.
- `taskit-output/src/formatter.rs` (1,251 lines, already flagged as a split
  candidate per `docs/ideas/2026-08-09-split-largest-modules.md`) implements
  the `OutputFormatter` trait and format implementations — no SARIF variant
  currently exists among them.
- `.github/workflows/ci.yml`/`nightly.yml`: no `upload-sarif` step anywhere.

## Proposed Direction

Add a `Sarif` variant to `taskit-types::output_format::OutputFormat`, and a
corresponding formatter in `taskit-output`. `cargo clippy --message-format
json` already emits structured diagnostics `taskit lint` could parse (lint.rs
currently just streams clippy's human output via `ctx.run`) — would need
`lint.rs` to switch to JSON-diagnostic mode when `--format sarif` is
requested, similar to how `coverage.rs` already parses `cargo llvm-cov
--json`.

Note: this overlaps with the module-split work already identified for
`formatter.rs` — worth sequencing after that split rather than adding more
surface area to the largest module in the workspace.

## Open Questions

- Should this cover only `lint`, or also `inspect`/`todo-sync` findings
  (which already resemble SARIF's `results[]` shape — file/line/message)?
- Does the target CI consumer (GitHub Code Scanning) require this to run in
  `ci.yml` itself, or is a nightly/informational job sufficient (matching the
  soft-fail pattern already used for `geiger`/`mutants`)?
