# taskit — Agent Operating Guide

This guide instructs AI coding agents (Claude, Copilot, or any shell-capable LLM)
how to work effectively in the taskit codebase.

## Identity

You are working on **taskit**, a standalone, config-driven Rust binary for running
CI pipelines in any Rust workspace. The codebase uses hexagonal architecture
(ports/adapters) with Rust edition 2024.

## Primary Toolkit: `cargo` Commands

All development workflows use `cargo` and the `taskit` binary directly.

### Build & Quality

| Command                                     | Purpose                                 |
| ------------------------------------------- | --------------------------------------- |
| `cargo build`                               | Build all crates                        |
| `cargo build --release`                     | Build release binaries                  |
| `cargo check`                               | Type-check without compiling            |
| `cargo fmt --check`                         | Check formatting (no modify)            |
| `cargo fmt`                                 | Format all Rust code                    |
| `cargo clippy --all-targets -- -D warnings` | Lint all crates                         |
| `cargo nextest run --workspace`             | Run all tests via nextest               |
| `cargo nextest run -p taskit-engine`        | Test one crate                          |
| `cargo nextest run -E 'test(pipeline)'`     | Filter tests by name                    |
| `cargo test --doc`                          | Run doc tests                           |
| `cargo deny check`                          | Check for license/advisory issues       |
| `taskit pre-commit`                         | Run pre-commit checks                   |

### Testing

Tests are colocated in each module under `#[cfg(test)]` blocks. Run full suite:

```bash
cargo nextest run --workspace
```

Filter by crate:

```bash
cargo nextest run -p taskit-core
cargo nextest run -p taskit-engine
cargo nextest run -p taskit-init
cargo nextest run -p taskit-crux
```

Filter by test name:

```bash
cargo nextest run -E 'test(config_parsing)'
```

## Workspace Layout

```
taskit/
├── Cargo.toml              # Workspace manifest
├── src/                    # Binary entry point (main.rs)
├── crates/
│   ├── taskit-core/        # Shared types: Config, StepResult, PipelineRunner
│   ├── taskit-engine/      # CI pipeline, step execution, output formatters
│   ├── taskit-init/        # InitPlan discovery, TOML/Cruxfile rendering
│   └── taskit-crux/        # EmbeddedCruxRunner stub (feature-gated)
├── taskit-protocol.lock    # Protocol drift tracking (hashes)
├── Cargo.lock              # Reproducible builds
└── README.md
```

## Code Conventions

### Rust (Edition 2024)

- **Line width**: 100 characters
- **Linting**: `cargo clippy --all-targets -- -D warnings` (strict)
- **Error handling**: `anyhow::Result<T>`, propagate with `?`, no `unwrap()` in
  production
- **Naming**: PascalCase structs/enums, snake_case functions, SCREAMING_SNAKE_CASE
  constants
- **Imports**: Group by external crate, then std
- **Tests**: Unit tests in `mod tests {}`, integration tests in `tests/`
- **Test isolation**: Use dependency injection, avoid `set_var` side effects

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

Types: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`

## Crate Responsibilities

| Crate           | Role                                                        |
| --------------- | ----------------------------------------------------------- |
| `taskit`        | Binary entry point; CLI parsing (clap) and dispatch         |
| `taskit-core`   | Shared types: Config, StepResult, PipelineRunner trait      |
| `taskit-engine` | CI pipeline, config loading, output formatters, execution   |
| `taskit-init`   | InitPlan discovery, TOML/Cruxfile rendering, interactive UI |
| `taskit-crux`   | EmbeddedCruxRunner stub (feature-gated, optional)           |

## Workflow: Implement a Feature

1. **Understand the architecture**: Read the relevant crate's module docs and
   existing code. taskit uses hexagonal patterns — ports in traits, adapters in
   implementations.

2. **Write tests first**: Add unit tests in `#[cfg(test)]` blocks, integration
   tests in `tests/` if needed.

3. **Implement the feature**: Follow naming conventions, keep lines under 100
   chars, use `anyhow::Result<T>`.

4. **Validate locally**:

   ```bash
   cargo fmt
   cargo clippy --all-targets -- -D warnings
   cargo nextest run --workspace
   ```

5. **Commit** with a conventional message.

## Workflow: Debug a Test Failure

1. **Identify the failing test**:

   ```bash
   cargo nextest run --workspace
   ```

2. **Run just that test with output**:

   ```bash
   cargo nextest run -E 'test(my_test_name)' --nocapture
   ```

3. **Use `dbg!` macro or print debug info** in the test

4. **Run doc tests** if your change touches documentation:

   ```bash
   cargo test --doc
   ```

## Key Dependencies

- **CLI**: `clap` (derive), **Serialization**: `serde` + `toml`
- **Error handling**: `anyhow`
- **Testing**: `cargo-nextest`

## Environment Variables

| Variable         | Purpose                                |
| ---------------- | -------------------------------------- |
| `RUST_BACKTRACE` | Backtrace on panic (`1` or `full`)     |
| `RUST_LOG`       | Logging filter (if using `env_logger`) |

## Safety & Guardrails

- **No `unwrap()` in production code** — use `?` operator or `anyhow::Context`
- **No hardcoded paths** — inject as function args or struct fields
- **No `unsafe` without clear justification** — document the safety invariant
- **Git**: Never force-push unless explicitly instructed

## When to Ask

- If adding a new crate to the workspace
- If changing the public API of a crate
- If introducing a major architectural change
- If unsure which crate owns a responsibility

Before acting, state:

1. Which crate will be modified
2. The specific files you plan to touch
3. Whether you'll use TDD or direct edit

Wait for confirmation if ambiguous.
