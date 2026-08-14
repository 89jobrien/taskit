# Idea: Additional Protocol-Drift Structural Checks

## Effort

Medium (1-3 days)

## Source

Grounded in `minibox/xtask/src/protocol_sites.rs`,
`minibox/xtask/src/check_protocol_sites.rs`, and `minibox/xtask/protocol-drift.lock`.

## Problem

taskit's protocol-drift system (`protocol/drift.rs`) hashes tracked contract
surfaces (per `[[protocol.surfaces]]` in `taskit.toml`) and, as of this
session, can `--watch` and auto-remediate. minibox's equivalent system goes
further structurally: it also checks for *unused* protocol variants and
*adapter test coverage*, not just hash drift on tracked files.

## Evidence

- `minibox/xtask/protocol-drift.lock` (111 lines): per-surface sha256 hashes
  across multiple domain files (wire-protocol, domain-capability,
  domain-checkpoint, domain-error, domain-exec, domain-filesystem,
  domain-ids, ...) — comparable granularity to taskit's own lockfile.
- `minibox/xtask/src/protocol_sites.rs` / `check_protocol_sites.rs`: verifies
  `HandlerDependencies`/`DaemonRequest`/`DaemonResponse` construction-site
  counts and flags protocol variants with zero usages outside `protocol.rs`
  (dead protocol surface detection).
- `taskit check-protocol-sites --file F --pattern P --expected N`
  (`CLAUDE.md` command table) is structurally similar — counts construction
  sites for a named pattern — but requires the caller to already know what
  file/pattern/expected-count to check, rather than automatically detecting
  dead variants.
- No `adapter-coverage`-equivalent exists in taskit at all; taskit's
  hexagonal `taskit-core` ports (`PipelineRunner`, `ConflictResolver`) have a
  conformance-test harness in `taskit-testing` per `CLAUDE.md`'s Testing
  section, but nothing automatically checks that every adapter implementing a
  port is covered by that harness.

## Proposed Direction

Two independent, separable additions:

1. **Dead-variant detection**: extend `check-protocol-sites` (or add a
   sibling `check-protocol-variants`) to auto-scan an enum's variants and
   flag ones with zero usages outside their defining file, rather than
   requiring the caller to specify pattern/expected-count manually.
2. **Adapter coverage check**: given `taskit-core`'s ports are a small, known
   set (`PipelineRunner`, `ConflictResolver`), a `check-adapter-coverage`
   command could verify every `impl PipelineRunner for X` / `impl
   ConflictResolver for X` has a corresponding entry exercised by
   `taskit-testing`'s conformance harness.

## Open Questions

- Is dead-variant detection generally useful for taskit's own codebase (which
  is much smaller than minibox's protocol surface), or is this overkill for
  taskit's current scale?
- Adapter coverage is naturally small right now (2 ports, few impls) — worth
  building before it's actually a pain point, or revisit once a third adapter
  is added?
