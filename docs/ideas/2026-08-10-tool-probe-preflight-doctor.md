# Idea: Tool-Probe / Preflight `doctor` System

## Effort

Large (> 3 days)

## Source

Grounded in `minibox/xtask/src/preflight.rs`.

## Problem

taskit shells out to a growing list of external tools (`gh`, `cargo-llvm-cov`,
`cargo-outdated`, `cargo-deny`, `cargo-nextest`, `cargo-machete`, `cargo
sweep`, plus clippy/rustfmt) across many modules, and checks for their
presence ad hoc, inconsistently, per-command. `check_freshness.rs` checks
`cargo outdated --version` with a comment explaining why (`cargo-outdated`
rejects a bare `--version`, "mirrors the cargo-llvm-cov handling in
dev_setup.rs") — i.e. this exact problem (tool detection quirks) has already
been solved twice, separately, in two different files.

## Evidence

- `minibox/xtask/src/preflight.rs`: a `ToolProbe` trait abstraction that
  probes tool availability uniformly, backing a `doctor` diagnostic command.
- `taskit-engine/src/check_freshness.rs`: `tool_exists_cmd("cargo",
  &["outdated", "--version"])` with an explicit comment cross-referencing
  `dev_setup.rs`'s similar handling for `cargo-llvm-cov`.
- `taskit-engine/src/dev_setup.rs`: presumably has its own, separately
  maintained tool-detection logic (per the cross-reference comment above) —
  not yet unified with `check_freshness.rs`'s.
- No `taskit doctor` command exists; a user hitting "cargo-outdated not
  installed" only discovers that mid-run, per-command, one tool at a time.

## Proposed Direction

1. Define a small `ToolProbe` trait in `taskit-engine` (or promote
   `util::tool_exists_cmd` into a richer abstraction) that each tool-dependent
   module (`check_freshness`, `dev_setup`, `testing::coverage`, `todo_sync`'s
   `gh` dependency, `audit`) implements or registers against.
2. Add `taskit doctor` — runs every registered probe and reports install
   status + install-hint (`cargo install cargo-outdated`, etc.) in one pass,
   rather than discovering missing tools one command at a time.
3. Migrate existing ad hoc `tool_exists_cmd` call sites to the unified probe
   list incrementally — this doesn't need to be a big-bang rewrite.

## Open Questions

- Is `dev_setup.rs`'s existing tool-detection logic (referenced but not yet
  inspected in this pass) already close to a `ToolProbe`-shaped abstraction,
  making this smaller than it looks?
- Should `doctor` also validate config (`taskit.toml` presence/validity,
  `taskit-protocol.lock` existence) alongside external tool presence, given
  `init.rs`/`InitPlan` already has related discovery logic?
