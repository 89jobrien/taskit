# Idea: `check repo-clean` Subcommand

## Effort

Quick Win (< 1 day)

## Source

Grounded in `minibox/xtask` CLI dispatch (`xtask check repo-clean`).

## Problem

`taskit flow auto` and `taskit release` both implicitly assume a clean
working tree (uncommitted changes could interfere with git-flow branch hops
or release tagging), but there's no standalone, reusable check a user or a
pipeline step can call to assert this before mutating state.

## Evidence

- `minibox/xtask` main.rs dispatch includes `check repo-clean` as one of the
  nested `check` subcommands alongside `stale-names`, `protocol-drift`,
  `protocol-sites`, `protocol-variants`, `adapter-coverage`, `no-unwrap`.
- `taskit`'s `Cmd` enum (`src/main.rs`) has no equivalent; `flow.rs` and
  `release/gh.rs` presumably assume/require clean state without a dedicated,
  independently invokable check.

## Proposed Direction

Add `taskit check-repo-clean` (matching taskit's existing flat naming
convention — `check-deps`, `check-freshness`, `check-protocol-drift`, rather
than minibox's nested `check <subcommand>` style) that runs `git status
--porcelain` and errors (or `--warn-only`, matching the pattern already used
by `check-freshness`/`todo-sync`) if the tree isn't clean. Wire it as an
explicit pre-check inside `flow::guard` and before `release::gh::run`, and
expose it standalone for use in custom pipelines/`taskit.toml` `[ci].steps`.

## Open Questions

- Should this also check for untracked files, or only tracked modifications
  (i.e. does `git status --porcelain` need `--untracked-files=no` to match
  `flow`'s actual safety requirement)?
- Is this better implemented as a reusable internal helper (`ctx` method)
  first, with the CLI subcommand as a thin wrapper, since `flow.rs` likely
  wants to call the same logic without shelling out to itself?
