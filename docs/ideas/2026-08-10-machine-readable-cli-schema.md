# Idea: Machine-Readable CLI Schema

## Effort

Quick Win (< 1 day)

## Source

Grounded in `minibox/xtask/schema/cli.schema.json`.

## Problem

`taskit`'s full CLI surface (the `Cmd` enum in `src/main.rs`, ~40 subcommands
and growing — `todo-sync`, `--fix`, `--watch`, `--workspace` flags all added
this week alone) exists only as clap derive macros. There's no
machine-readable description of it for external tooling: shell completion
generation beyond clap's built-in, docs generation, or agent tool-calling
(relevant since `taskit` is frequently invoked by Claude Code sessions in
this workspace).

## Evidence

- `minibox/xtask/schema/cli.schema.json` (458 lines): a hand-maintained JSON
  Schema (draft 2020-12) describing every `cargo xtask <command>` invocation
  as a `oneOf` of `$defs/commands/*`. Its header explicitly states: "Generated
  by hand... regenerate manually when main.rs's match arms change; there is
  no build-time codegen."
- `taskit`'s `src/main.rs` `Cmd` enum has no schema export; clap's own
  `clap_complete`/`clap_complete_nushell` (already workspace deps per
  `Cargo.toml`) only generate shell completions, not a general-purpose schema.

## Proposed Direction

Rather than hand-maintaining a schema (which minibox's own header comment
flags as a manual-sync burden), generate it from the `Cmd` enum directly:
clap's `CommandFactory`/`Command::get_subcommands()` API can walk the parsed
`clap::Command` tree at runtime and emit JSON Schema without needing a
separate hand-written source of truth. Add a hidden `taskit --schema`
(or `taskit init --emit-schema`) flag that dumps it.

## Open Questions

- Is there an existing `clap` ecosystem crate for this (e.g. `clap_json` or
  similar) rather than writing a custom walker?
- What's the actual downstream consumer — shell completion, docs generation,
  or agent tool definitions? The consumer should drive the schema's shape.
