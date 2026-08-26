# Idea: Wire Up or Remove `tee`/`buffer` Output Sinks

## Effort

Quick Win (< 1 day)

## Problem

`taskit-output/src/sinks/tee.rs` and `sinks/buffer.rs` both carry a self-documented
`// TODO(audit): only constructed in this crate's own tests — not adopted in any
production output path yet.` They implement the `OutputFormatter`/sink pattern but
nothing in `taskit`'s real command dispatch ever constructs them.

## Evidence

- `crates/taskit-output/src/sinks/tee.rs:4-5` — TODO(audit) comment.
- `crates/taskit-output/src/sinks/buffer.rs:6-7` — same TODO(audit) comment.
- No references to `TeeSink`/`BufferSink` outside their own test modules
  (per Explore agent scan).

## Proposed Direction

Two viable outcomes, either resolves the drift:

1. **Wire it in**: add a `--tee <path>` global flag (or per-command) that mirrors
   stdout output to a log file via `TeeSink` — useful for `ci`/`flow auto` runs
   where output should be both live and archived. `BufferSink` could back an
   in-memory capture used by `flow auto`'s conflict-resolution step for
   structured error context.
2. **Remove it**: if no real use case exists, delete both sinks and their tests
   to stop workspace-refactor/dead-code tooling from flagging them repeatedly.

Given `flow auto` already writes resumable state to `target/taskit/state.json`,
option 1 (tee to a log file during long-running `flow auto`/`ci` runs) seems the
more natural fit — worth validating with the user before deciding.

## Open Questions

- Was this scaffolding intentionally built ahead of a specific consumer (e.g. TUI
  log streaming), or genuinely orphaned?
