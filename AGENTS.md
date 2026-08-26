# taskit AGENT Instructions

This guide is for coding agents working in `/Users/joe/dev/taskit`.

## Project Snapshot

- Language: Rust (`edition = 2024`)
- Root binary: `taskit` in `src/main.rs`
- Architecture: multi-crate hexagonal (ports/adapters)
- Shared contracts/errors/types: `crates/taskit-types`
- Core trait ports: `crates/taskit-core`
- Orchestration/commands: `crates/taskit-engine`

## Workspace Layout

```text
taskit/
├── src/                    # CLI entrypoint + adapter wiring
├── crates/taskit-types/    # config, errors, step/output/flow types
├── crates/taskit-core/     # pipeline/conflict ports + conformance
├── crates/taskit-engine/   # CI, flow, protocol, health, test orchestration
├── crates/taskit-init/     # init discovery + taskit.toml/Cruxfile renderers
├── crates/taskit-output/   # output formatter + sink abstractions
├── crates/taskit-tui/      # dashboard app/ui/snapshots
├── crates/taskit-crux/     # Crux integration/stub
├── crates/taskit-testing/  # shared test helpers/macros
├── crates/taskit-macros/   # proc macros
├── taskit.toml             # runtime configuration
└── taskit-protocol.lock    # protocol drift lockfile
```

## Build Commands

```bash
cargo check --workspace
cargo build --workspace
cargo build --release --workspace
```

Run the CLI:

```bash
cargo run -p taskit -- --help
cargo run -p taskit -- check quick
```

## Lint + Format Commands

```bash
cargo fmt --all
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

taskit wrappers:

```bash
taskit check fmt --check
taskit check lint
taskit check quick
taskit check ci --fail-fast
```

## Test Commands

Preferred full test suite:

```bash
cargo nextest run --workspace
```

Run one crate:

```bash
cargo nextest run -p taskit-engine
```

Run a single test by name (nextest filter):

```bash
cargo nextest run -p taskit-engine -E 'test(merge_with_resolution_resolver_resolves_conflict)'
```

Run one integration test file:

```bash
cargo test -p taskit-engine --test flow_integration
```

Run one exact test function with output:

```bash
cargo test -p taskit-engine --test flow_integration \
  merge_with_resolution_resolver_resolves_conflict -- --exact --nocapture
```

taskit test wrappers:

```bash
taskit test run --crate-name taskit-engine
taskit test coverage --crate-name taskit-engine --threshold 80
```

## Recommended Validation Loop

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace
```

## Code Style Guidelines

### Formatting

- Always apply rustfmt output; do not hand-align.
- Keep functions focused; extract helpers for long logic.
- Keep comments sparse; only document non-obvious invariants.

### Imports

- Let rustfmt manage order/grouping.
- Prefer explicit imports over glob imports.
- Use `as` aliases only for collisions or readability.

### Naming

- Types/enums/traits: `PascalCase`
- Functions/modules/fields: `snake_case`
- Constants/statics: `SCREAMING_SNAKE_CASE`
- Tests: behavior-oriented names with clear expected outcomes.

### Types and Boundaries

- Use shared domain/config types from `taskit-types` across crate boundaries.
- Keep trait ports in `taskit-core`; implementations in engine/adapters.
- Prefer `&str`/`&Path` at boundaries unless ownership is required.
- Avoid ad-hoc tuples/strings when a domain type exists.

### Error Handling

- Return `Result<T, TaskitError>` at fallible boundaries.
- Prefer specific variants (`ConfigError`, `FlowError`, `ProtocolError`, etc.).
- Use `?` and `TaskitResultExt` for context mapping.
- Avoid `unwrap`/`expect` in production code.
- `unwrap`/`expect` is acceptable in tests when invariants are explicit.

### Testing Practices

- Keep unit tests near implementation (`#[cfg(test)]`).
- Use integration tests in `crates/*/tests/` for crate-level behavior.
- Cover flow transitions, gate behavior, and typed error paths.

## Architecture Guardrails

- Keep `src/main.rs` focused on CLI parsing and dispatch.
- Put orchestration in `taskit-engine`, not in CLI glue.
- Intentional protocol-surface changes require lockfile updates:

```bash
taskit protocol drift --update
```

## Cursor / Copilot Rules

Repository scan found no editor-agent rule files:

- `.cursor/rules/` not present
- `.cursorrules` not present
- `.github/copilot-instructions.md` not present

If those files are added later, treat them as higher-priority constraints and mirror them here.

## Pointers

- Flow behavior: `crates/taskit-engine/src/flow.rs`
- CLI behavior: `src/main.rs`
- Command dispatch: `crates/taskit-engine/src/command.rs`
