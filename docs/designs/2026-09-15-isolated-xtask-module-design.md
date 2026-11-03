# Design: Isolated xtask Module

## Goal

Generate Taskit-owned xtask integration in `xtask/src/taskit/mod.rs` while preserving user-owned xtask files and automatically routing supported tasks.

## Approved Approach

Use a managed module plus a minimal, marker-wrapped bridge in `xtask/src/main.rs`; never replace the xtask directory or a user-owned file.

## Context Map

### Files to Modify

| File                                 | Purpose                                | Changes Needed                                                                     |
| ------------------------------------ | -------------------------------------- | ---------------------------------------------------------------------------------- |
| `crates/taskit-init/src/scaffold.rs` | Generates and tests the xtask scaffold | Render the managed module, insert the minimal bridge, and enforce ownership checks |
| `xtask/src/main.rs`                  | Checked-in generated xtask entry point | Reduce Taskit-owned code to the bridge                                             |
| `xtask/src/taskit/mod.rs`            | Checked-in generated Taskit adapter    | Add the isolated dispatch implementation                                           |

### Dependencies

| File                             | Relationship                                                |
| -------------------------------- | ----------------------------------------------------------- |
| `crates/taskit-init/src/lib.rs`  | Calls `scaffold::write_xtask`; no signature change required |
| `crates/taskit-init/src/plan.rs` | Selects xtask generation; no behavior change required       |

### Test Coverage

| Test location                        | Covers                                                                                                            |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------- |
| `crates/taskit-init/src/scaffold.rs` | Fresh generation, existing-main preservation, idempotency, dry runs, force behavior, and unmanaged path conflicts |

### Reference Patterns

| File                                 | Pattern to Follow                                               |
| ------------------------------------ | --------------------------------------------------------------- |
| `crates/taskit-init/src/scaffold.rs` | Marker-based ownership and idempotent scaffold writes           |
| `crates/taskit-init/src/lib.rs`      | `emit_file` handling for parent-directory creation and dry runs |

### Risk

- [ ] Public API change: no; `write_xtask(force, dry_run)` remains unchanged.
- [ ] Cross-crate dependency change: no.
- [ ] Generated source layout changes: yes; Taskit implementation moves under `xtask/src/taskit/`.
- [ ] Existing custom `main.rs` mutation: limited to a marker-wrapped module declaration and dispatch call.

## Crate Ownership

- **Owner crate**: `taskit-init` owns generated xtask content and ownership checks.
- **Generated adapter**: `xtask/src/taskit/mod.rs` belongs to the consuming workspace's xtask crate.
- **Affected crates**: the root `taskit` binary continues calling `taskit-init`; no new imports are introduced.

## Public API

The Rust library API does not change. The generated module exposes one parent-visible function:

```rust
pub(super) fn dispatch() -> bool;
```

`dispatch` returns `true` after recognizing and running a Taskit-owned xtask command, and `false` when the command belongs to the user's existing dispatcher.

## Data Flow

1. `taskit init` discovers that xtask scaffolding was selected.
2. `write_xtask` validates `xtask/src/main.rs`, `xtask/Cargo.toml`, and `xtask/src/taskit/mod.rs` before mutating anything.
3. The generator creates or updates the marker-owned module and inserts an idempotent bridge into a conventional `fn main() { ... }`.
4. At runtime the bridge asks `taskit::dispatch()` to handle known Taskit tasks, then falls through to the user's dispatcher for unknown tasks.

## Integration Rules

- A missing xtask receives `Cargo.toml`, a minimal `main.rs`, and `taskit/mod.rs`.
- An existing `main.rs` is preserved except for the marker-owned bridge.
- An existing `Cargo.toml` is never rewritten.
- `--force` may refresh only a module containing Taskit's ownership marker; it does not replace `main.rs` or `Cargo.toml`.
- An existing unmarked `xtask/src/taskit/mod.rs` is treated as a conflict and no xtask file is changed.
- An existing `xtask/src/taskit.rs` or unmanaged `mod taskit;` declaration is also treated as a conflict.
- A non-conventional entry point that cannot be bridged safely produces an error before any xtask file is changed.
- Legacy inline Taskit blocks remain untouched during this change to avoid breaking user references to generated helper functions.

## Hexagonal Boundaries

- **Port**: `dispatch() -> bool` is the narrow boundary between a user-owned xtask entry point and Taskit's generated process adapter.
- **Adapter**: `xtask/src/taskit/mod.rs` translates xtask task names into `taskit` process invocations.

## Out of Scope

- Removing legacy inline managed blocks from existing xtasks.
- Changing the set or spelling of generated xtask commands.
- Parsing and rewriting arbitrary Rust syntax with an AST dependency.
- Modifying workspace membership automatically.

## Risk Summary

- [ ] Breaking API changes: no.
- [ ] New external dependency: no.
- [ ] Feature flag required: no.
- [ ] User-file overwrite: no; conservative bridge insertion fails closed.
