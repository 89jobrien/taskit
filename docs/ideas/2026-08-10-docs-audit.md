# Idea: Docs-Truth Auditing (`docs audit`)

## Effort

Medium (1-3 days)

## Source

Grounded in `minibox/xtask/src/docs_audit.rs` and `minibox/xtask/docs-audit-report.json`.

## Problem

`taskit` has no tooling to detect drift between documentation and code facts
(version numbers, crate counts, command lists). This gap is concrete, not
hypothetical: last session's `todo-sync`/`coverage --workspace`/etc. feature
commit bundled manual `CLAUDE.md`/`README.md` edits alongside the code
changes — nothing would have caught it if those docs had been left stale.

## Evidence

- `minibox/xtask/src/docs_audit.rs` + `docs-audit-report.json`: compares
  "fact markers" embedded in docs (e.g. `workspace_version`, `crate_count`,
  `adapter_suites`) against actual code-derived values, flagging
  `MISMATCH`/`pass` per check, plus a `coverage_gaps` report of entities
  (e.g. a crate) missing from specific doc files.
- `taskit`'s `CLAUDE.md` Config Reference table and Common Subcommands table
  are hand-maintained prose with no fact-marker mechanism; nothing verifies
  the subcommand table stays in sync with `src/main.rs`'s actual `Cmd` enum.
- taskit's `doc-sync`/`doc-review` *skills* (godmode plugin, not part of
  taskit itself) cover a similar need at the workspace-tooling layer, but
  operate externally via an LLM pass rather than as a deterministic, code-run
  `taskit` gate.

## Proposed Direction

Start narrow: a `taskit doc-drift` (or `docs audit`, matching minibox's
naming) check that verifies specifically things that are objectively
derivable from code:
- `CLAUDE.md`'s Common Subcommands table lists every `Cmd` variant that
  exists in `src/main.rs` (and no extra/removed ones) — this is directly
  checkable by parsing the clap `Command` tree (see the machine-readable CLI
  schema idea, `docs/ideas/2026-08-10-machine-readable-cli-schema.md` — natural
  prerequisite/synergy).
- Workspace version consistency (already computed internally by
  `health::collect_versions` for `versions_consistent`) cross-checked against
  any version strings mentioned in docs.

This is narrower than minibox's full fact-marker system but requires no new
"marker" annotation convention in docs — it derives ground truth from code
taskit already introspects.

## Open Questions

- Does minibox's fact-marker system require authors to hand-annotate docs
  with markers (e.g. `<!-- fact:workspace_version -->`), or is it fully
  derived? If markers are required, that's a bigger docs-authoring commitment
  than a purely code-derived check.
- Should this be a `taskit` command at all, or does it belong at the
  workspace `~/dev` tooling layer (`doc-sync` skill) since it's about
  cross-referencing docs prose, not something taskit's CI pipeline gates on
  per se?
