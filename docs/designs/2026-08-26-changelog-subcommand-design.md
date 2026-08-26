# Design: Changelog Subcommand

## Goal

Add a top-level `taskit changelog` command that provides typed workflows around `git-cliff`.

## Approved Approach

Use typed `unreleased`, `full`, `latest`, and `preview` modes, with `unreleased` as the default.

## Context Map

### Files to Modify

| File                                    | Purpose                        | Changes Needed                                           |
| --------------------------------------- | ------------------------------ | -------------------------------------------------------- |
| `crates/taskit-engine/src/changelog.rs` | Changelog orchestration        | Add mode mapping, config preflight, execution, and tests |
| `crates/taskit-engine/src/lib.rs`       | Engine module exports          | Export the changelog module                              |
| `crates/taskit-engine/src/command.rs`   | Command dispatch port          | Add the changelog command type and implementation        |
| `src/main.rs`                           | Clap parsing and binary wiring | Add the top-level command and map typed CLI modes        |

### Dependencies

| File                               | Relationship                                    |
| ---------------------------------- | ----------------------------------------------- |
| `crates/taskit-engine/src/ctx.rs`  | Existing process execution and dry-run boundary |
| `crates/taskit-types/src/error.rs` | Existing command error type                     |

### Test Coverage

| Test Location                           | Covers                                             |
| --------------------------------------- | -------------------------------------------------- |
| `crates/taskit-engine/src/changelog.rs` | Mode-to-command mapping and missing config failure |
| `src/main.rs`                           | Default and explicit Clap mode parsing             |

### Reference Patterns

| File                                     | Pattern to Follow                              |
| ---------------------------------------- | ---------------------------------------------- |
| `crates/taskit-engine/src/audit.rs`      | Thin external CLI wrapper through `Ctx`        |
| `crates/taskit-engine/src/release/gh.rs` | Typed command arguments and process delegation |
| `.claude/commands/changelog.md`          | Existing changelog mode semantics              |

### Risk

- Additive public engine API only; no existing signatures change.
- CLI surface is additive and has no JSON output contract.
- The wrapper depends on the installed `git-cliff` executable but adds no Cargo dependency.

## Crate Ownership

- **Owner crate**: `taskit-engine` owns changelog orchestration and external command execution.
- **Affected binary**: `taskit` owns Clap-specific parsing and maps it to engine types.

## Public API

### Types

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChangelogMode {
    #[default]
    Unreleased,
    Full,
    Latest,
    Preview,
}

pub struct Changelog {
    pub mode: ChangelogMode,
}
```

### Functions

```rust
pub fn run(ctx: &Ctx, mode: ChangelogMode) -> Result<(), TaskitError>;
```

## Data Flow

1. Clap parses an optional typed mode, defaulting to `unreleased`.
2. Binary dispatch maps the CLI mode to `taskit_engine::changelog::ChangelogMode`.
3. Engine validates `<workspace>/cliff.toml` and selects fixed `git-cliff` arguments.
4. `Ctx::run` executes the process or reports it through the existing dry-run path.

## Hexagonal Boundaries

- **Execution boundary**: Existing `Ctx` command execution methods isolate process invocation and dry-run behavior.
- **Adapter**: `taskit-engine::changelog` translates domain modes into `git-cliff` arguments.
- No new trait is warranted because the established command modules already use the injected `Ctx` boundary.

## Out of Scope

- Forwarding arbitrary `git-cliff` arguments.
- Creating `cliff.toml` automatically.
- Staging, committing, tagging, or publishing changes.
- Adding changelog configuration to `taskit.toml`.

## Risk Summary

- [x] Breaking API changes: no
- [x] New external dependency: no Cargo dependency; `git-cliff` is a runtime prerequisite
- [x] Feature flag required: no
