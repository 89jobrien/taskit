# Idea: `info context` — Cold-Start Repo Snapshot

## Effort

Medium (1-3 days)

## Source

Grounded in `minibox/xtask/src/context.rs`.

## Problem

taskit itself is heavily used by AI agent sessions in this workspace
(evidenced by this session's own commits carrying `Claude-Session:` trailers,
and the `.ctx/` directory convention). Every session currently reconstructs
its mental model of a `taskit`-managed workspace by separately reading
`CLAUDE.md`, running `cargo metadata`, checking `git log`, etc. — several
tool calls that a single structured snapshot command could replace.

## Evidence

- `minibox/xtask/src/context.rs` produces a machine-readable repo context
  snapshot (crate graph, adapters, test counts, recent commits) explicitly
  documented as being "for cold-start LLM sessions."
- `taskit`'s closest existing commands are `health` (metrics + trend, not
  structural) and `flow status` (git-flow position only) — neither combines
  crate graph + recent activity + test counts in one machine-readable output.
- `taskit-output::OutputFormat` already supports structured (JSON) output
  across commands, so this would fit the existing output-formatting
  convention rather than needing a new mechanism.

## Proposed Direction

Add `taskit info context` (or fold into `taskit inspect --format json` output
shape) that emits:
- Crate graph (from `cargo_metadata`, already a dependency per
  `check-protocol-drift`/`check-deps` usage).
- `[[workspace.propagation]]` affected-crate relationships (already computed
  internally for `--affected` flag support).
- Recent commit summary (last N commits, `git log --oneline`).
- Current health snapshot (reuse `health::collect`).
- Current flow position (reuse `flow::status_report`, added in the recent
  TUI Flow tab work per `docs/designs/2026-08-08-tui-flow-tab-design.md`).

This is largely composition of existing internal functions rather than new
logic — the value is in the single structured entry point, not new data
collection.

## Open Questions

- Should this live under a new `info` subcommand namespace (matching
  minibox's `info {metrics, context, changes}` grouping) or be a flag on an
  existing command (e.g. `taskit health --context`)?
- Is `--with-coverage`-style expense a concern here, or should this always be
  cheap/fast since it's meant for frequent cold-start use?
