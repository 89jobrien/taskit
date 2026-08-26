# Idea: Windows Target Support

## Effort

Large (> 3 days) — likely gated on the `crux-script` engine idea

## Problem

No Windows target exists anywhere in the project: the release build matrix only
covers linux musl (x86_64/aarch64 via `cross`) and macOS (x86_64/aarch64
native), and the main CI test job runs on `ubuntu-latest` only. `taskit` is a
general-purpose CI pipeline binary intended for any Rust workspace, but
Windows-based Rust developers currently have no supported path.

## Evidence

- `.github/workflows/release.yml`: 4-target build matrix, no `x86_64-pc-windows-*`
  entry.
- `.github/workflows/ci.yml`: single ubuntu-only job for the actual test suite
  (no cross-platform matrix even for testing, let alone release).

## Proposed Direction

1. **Investigate blockers first** — `SubprocessCruxRunner` and other shell-outs
   (`.githooks/`, `cargo sweep` invocations, path handling in `hooks.rs`) likely
   carry Unix assumptions (shebang scripts, `/`-only path joins, POSIX signal
   handling). Audit for these before attempting a Windows build.
2. If the `EmbeddedCruxRunner` real-implementation idea
   (`docs/ideas/2026-08-09-real-crux-script-engine.md`) lands, it removes one
   subprocess dependency that's likely Unix-coupled.
3. Add `x86_64-pc-windows-msvc` to the release matrix once local `cargo build
   --target x86_64-pc-windows-msvc` (or a Windows CI runner) succeeds without
   patching.
4. Consider whether `taskit init`'s generated `.githooks/` shell scripts need a
   `.ps1`/`.cmd` equivalent for Windows git hook support.

## Open Questions

- Is there actual user demand for Windows support, or is this purely a
  completeness gap? Worth confirming before investing Large-effort work.
- Does `taskit-tui` (built on `ratatui`/`crossterm`) already work on Windows
  terminals, or is that an additional unknown?
