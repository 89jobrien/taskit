# Idea: Split the Largest Workspace Modules

## Effort

Medium (1-3 days) — likely a per-module sequence of Quick Wins, but bundled here
since the modules share a common "split candidate" audit marker.

## Problem

Four modules across the workspace are self-flagged via `// TODO(audit): N lines
— split candidate.` header comments, each written by a prior audit pass:

| Module | Lines | Note |
| --- | --- | --- |
| `taskit-output/src/formatter.rs` | 1,251 | largest module in the workspace |
| `crates/taskit-engine/src/health.rs` | 932 | |
| `crates/taskit-engine/src/testing/compile.rs` | 968 | crate-exclusion logic patched 3x (see below) |
| `crates/taskit-init/src/scaffold.rs` | 831 | |
| `crates/taskit-engine/src/flow.rs` | 814 | |

`testing/compile.rs` is the highest-signal one: its header explicitly notes the
crate-exclusion logic "has been patched 3x for different directory types"
(commits `ebe2117`, `bb15aec`, ...) — repeated patching of one code path inside
an oversized module is a correctness smell, not just a style nit.

## Evidence

- Header TODO(audit) comments at line 1 of each file listed above.
- `crates/taskit-engine/src/testing/compile.rs:1-3` names specific commit hashes
  for prior patches to the same logic.

## Proposed Direction

Prioritize `testing/compile.rs` first given its patch history — extract the
crate-exclusion/directory-type logic into its own submodule with dedicated unit
tests covering each directory type that previously required a patch, so future
directory-type additions don't require re-learning the whole 968-line file.

For the other three, a straightforward "extract into `mod.rs` + submodules by
responsibility" refactor (e.g. `health/metrics.rs`, `health/baseline.rs`,
`health/report.rs`) should suffice — no behavior change, just organization.

## Open Questions

- Are these TODO(audit) comments from a specific prior tool run (health-score
  skill? rustqual?) that already has more detailed splitting suggestions
  recorded somewhere (`.ctx/logs/` or `.health-baseline.json` history)?
- Should this be one PR per module to keep diffs reviewable, or a single
  workspace-refactor pass?
