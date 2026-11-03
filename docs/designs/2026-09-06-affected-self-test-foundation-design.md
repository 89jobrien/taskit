# Design: Affected Detection and Self-Test Foundation

## Goal

Make affected-crate selection and cached self-testing correct for the complete workspace before higher-level automation relies on either result.

## Approved Approach

Use the approved foundation-first roadmap: normalize affected results to Cargo package names, compute propagation to a fixed point, and cache only complete workspace test runs.

## Context Map

### Files to Modify

| File                                            | Purpose                                         | Changes Needed                                                                                 |
| ----------------------------------------------- | ----------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `crates/taskit-engine/src/affected.rs`          | Maps changed files to workspace members         | Handle the root package, workspace-wide files, package identifiers, and transitive propagation |
| `crates/taskit-engine/src/util.rs`              | Executes commands per selected package          | Consume canonical package names without directory assumptions                                  |
| `crates/taskit-engine/src/testing/self_test.rs` | Hashes and runs taskit's self-tests             | Hash all workspace inputs and run nextest for the workspace                                    |
| `crates/taskit-types/src/config.rs`             | Defines workspace and propagation configuration | Validate propagation references and duplicates                                                 |
| `taskit.toml`                                   | Declares workspace package mapping              | Remove duplicate dependents and retain canonical package identifiers                           |

### Dependencies

`fmt`, `lint`, test execution, and hooks consume `affected::detect`; cache integrity consumes the self-test cache file. The function signature remains stable, but its result semantics change from directory identifiers to canonical package names, so every workspace caller must migrate in the same stage.

### Test Coverage

Extend inline tests in `affected.rs` for root files, workspace-wide files, package/directory mapping, unknown propagation targets, cycles, and transitive chains. Extend `self_test.rs` tests with a temporary multi-crate workspace fixture.

### Reference Patterns

Follow Cargo metadata discovery in `crates/taskit-engine/src/discovery.rs` and deterministic hashing in `crates/taskit-engine/src/protocol/drift.rs`.

## Crate Ownership

- **Owner crate**: `taskit-engine` — owns affected analysis and self-test orchestration.
- **Contract crate**: `taskit-types` — owns semantic validation of workspace configuration.

## Public API

No new public function is required. Preserve the signature while documenting its new canonical-package result:

```rust
pub fn detect(
    sh: &xshell::Shell,
    ws: &WorkspaceConfig,
) -> Result<BTreeSet<String>, TaskitError>;
```

Add crate-private pure helpers for changed-file classification and fixed-point propagation so behavior can be tested without Git subprocesses. Add `schema_version: u32` to the private `SelfTestCache`; any absent or unsupported version invalidates the cache.

## Data Flow

1. Read changed paths from Git and workspace package metadata from `WorkspaceConfig`.
2. Map paths to canonical package names, treating workspace-level contract files as affecting all packages.
3. Expand configured dependent edges until no package is added.
4. Pass package names to per-crate command execution.
5. Discover package roots through Cargo metadata and hash tracked Rust sources, tests, declared fixtures, manifests, build scripts, and `Cargo.lock`; do not follow symlinks or traverse `.git`, `.taskit`, or `target`.
6. Treat an unreadable discovered input as an error instead of caching an incomplete hash; cache success only after `cargo nextest run --locked --workspace` passes.

## Hexagonal Boundaries

- **Port**: existing Git and command execution boundary through `xshell` and `Ctx`.
- **Pure domain logic**: changed-path mapping and graph expansion remain subprocess-free helpers.

## Out of Scope

- Inferring semver impact.
- Replacing configured propagation with a fully automatic Cargo dependency graph.
- Cross-repository affected analysis.

## Risk

- [ ] Breaking API changes: behavioral change to the public `detect` result; all in-workspace consumers migrate atomically.
- [ ] Serialization format changes: self-test cache contents change and must invalidate old entries safely.
- [ ] New external dependency: no.
- [ ] Protocol lock update: no.
