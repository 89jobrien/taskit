# Idea: Real `crux-script` Execution Engine

## Effort

Large (> 3 days)

## Problem

`taskit-crux` is documented in `CLAUDE.md` as "EmbeddedCruxRunner stub" and that's
literally accurate: `EmbeddedCruxRunner::run_pipeline`
(`crates/taskit-crux/src/lib.rs:25-57`) checks that the Cruxfile path exists,
starts a timer, then — per its own comment
(`// Stub: crux-script runtime not yet available.`, line 40) — does nothing and
returns a fabricated, hardcoded `Ok(PipelineOutcome)` with a single synthetic
`StepResult { name: "crux-embedded", status: Pass, ... }`. It never parses or
executes an actual Cruxfile. `SubprocessCruxRunner` in
`taskit-engine/src/pipeline_runner.rs` is what's actually wired into CI dispatch
today — it presumably shells out to an external `crux` binary/process instead.

## Evidence

- `crates/taskit-crux/src/lib.rs:14` doc comment: "Currently a stub — full
  implementation requires the `crux-script` crate."
- `crates/taskit-crux/src/lib.rs:40`: `// Stub: crux-script runtime not yet
  available.`
- `crates/taskit-crux/src/lib.rs:8-14`: `// TODO(audit): zero callers outside
  this file — SubprocessCruxRunner in taskit-engine/src/pipeline_runner.rs is
  what's actually wired into CI dispatch. Cross-check
  docs/designs/2026-06-27-init-pipeline-runner-design.md before removing; may
  be intentional forward scaffolding.`
- ~50 lines of conformance tests (lines 60-108) validate only the stub's
  error/success invariants, not real execution semantics.

## Proposed Direction

This is explicitly marked as intentional forward scaffolding pending a
`crux-script` crate that doesn't yet exist in the workspace. Before any
implementation work:

1. Re-read `docs/designs/2026-06-27-init-pipeline-runner-design.md` (referenced
   directly in the TODO) to confirm current intent hasn't changed.
2. Scope what "real" execution means: does `crux-script` need to be built from
   scratch, or does it already exist as a sibling project referenced elsewhere
   in `~/dev`?
3. If built, `EmbeddedCruxRunner` would let taskit run Cruxfiles without
   shelling out to a subprocess — likely the prerequisite for genuine
   cross-platform (Windows) support, since `SubprocessCruxRunner` probably
   assumes a Unix-like environment.

## Open Questions

- Does a `crux-script` crate already exist elsewhere in the workspace (e.g.
  `~/dev/crux`, described in the root `CLAUDE.md` as "Agentic Rust DSL and
  runtime trace model")? If so, this may be an integration task, not a
  from-scratch build.
- Is embedding actually desired, or is subprocess dispatch acceptable
  long-term and this stub should just be removed?
