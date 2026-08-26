# Idea: VM/Container-Based Cross-Platform Test Execution

## Effort

Large (> 3 days)

## Source

Grounded in `minibox/xtask/src/{setup_test_vm,test_in_vm,test_linux,test_image}.rs`.

## Supersedes / Reconcile With

`docs/ideas/2026-08-09-windows-target-support.md` — that doc identified the
Windows-target gap and flagged `EmbeddedCruxRunner`'s stub status
(`docs/ideas/2026-08-09-real-crux-script-engine.md`) as a likely blocker. This
idea supplies the *mechanism* minibox uses to solve an analogous
cross-platform testing problem: don't require native runners for every
target, run tests inside a VM/container instead. **Read the Windows-target
idea doc first** — this is a proposed implementation path for it, not a
separate need.

## Problem

taskit's CI test job runs on `ubuntu-latest` only (per the earlier repo scan:
"no macOS/cross-platform matrix for the actual test suite, only for release
builds"). Getting genuine Windows or additional Linux-variant test coverage
without paying for/maintaining multiple native GitHub-hosted runner types is
an open problem.

## Evidence

- `minibox/xtask/src/test_image.rs`: builds an OCI test image (Alpine-based).
- `minibox/xtask/src/test_linux.rs` / `test_in_vm.rs`: runs the Linux test
  suite inside that image or a VM.
- `minibox/xtask/src/setup_test_vm.rs`: provisions a `smolvm` VM (per
  `xconfig.toml`'s `[vm]` section: VM name + smolfile paths for
  setup/CI-gate) — this is minibox's actual mechanism for testing
  platform-specific (Linux daemon/cgroup) behavior from a macOS dev machine
  without needing native Linux CI minutes for every run.
- `taskit`'s CLAUDE.md notes "Tests gated on `cfg(target_os = "linux")` are
  silently skipped on macOS — expected, not a failure" — i.e. taskit already
  has platform-gated tests that currently just don't run locally, the same
  gap minibox's VM infra exists to close.

## Proposed Direction

This is a much heavier lift for taskit than for minibox — minibox's VM
infrastructure exists because it manages actual Linux VMs/containers as its
core product; taskit would be adopting VM tooling purely for test coverage.
Before committing to this:

1. Confirm actual demand for Windows/cross-platform taskit usage (per the
   open question already in `2026-08-09-windows-target-support.md`).
2. If real, evaluate reusing `minibox`'s own VM tooling (`mbx`/`minibox` per
   the `~/dev` CLAUDE.md project index — "Linux VM/VPS manager") as an
   external dependency rather than reimplementing VM provisioning inside
   taskit's own xtask-equivalent, since minibox already solves this problem
   as its primary purpose.
3. Scope down to just Linux-variant testing (Alpine via OCI image, no VM)
   first — much cheaper than full VM provisioning, and directly reusable via
   `docker run` in CI without new infrastructure.

## Open Questions

- Is there really Windows demand, or does "cross-platform" for taskit's
  actual use case just mean "more Linux distro coverage" (musl vs glibc,
  already partially covered by the release build matrix)?
- Would using `minibox`/`mbx` directly (as a sibling project in this
  workspace) as taskit's VM test backend create an unwanted dependency
  between two otherwise-independent projects?
