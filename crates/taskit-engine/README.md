# taskit-engine

`taskit-engine` is taskit's application and orchestration layer. It turns parsed configuration and
command arguments into Cargo, git, release, protocol, test, and health operations. The root binary
is the composition layer; this crate owns the reusable command implementations.

## Workspace role

```text
taskit-types     shared configuration, errors, and outcomes
taskit-core      runner and resolver ports
taskit-output    live messages and final result formatting
       \             |             /
        +------ taskit-engine -----+
                    |
              root taskit CLI
```

The crate has no Cargo feature flags. Its direct taskit dependencies are `taskit-core`,
`taskit-types`, and `taskit-output`.

## Runtime model

`Ctx` is injected into every `Command`. It owns the `xshell::Shell`, resolved workspace root,
parsed `Config`, dry-run state, output format, transient output suppression, and a command log used
for failure provenance.

```rust
use std::path::PathBuf;

use taskit_engine::Ctx;
use taskit_types::config::Config;
use taskit_types::output_format::OutputFormat;
use xshell::Shell;

let ctx = Ctx::new(
    Shell::new()?,
    PathBuf::from("."),
    Config::default(),
    true,
    OutputFormat::Human,
);
assert!(ctx.dry_run);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`Ctx::run` executes a command, records it, and treats non-zero status as an error. In dry-run mode
it records and prints the command without executing it. `run_capture` returns stdout, stderr, and a
success flag even for non-zero exit; it errors only when spawning fails. `with_silent` suppresses
successful child output while retaining failure detail. `pipeline_run_context` gathers best-effort
binary, git, Rust, Cargo, and workspace metadata.

`Workspace` is the crate-root value returned by configuration loading: a resolved root plus parsed
`taskit_types::config::Config`.

## Command port and wrappers

`Command` is the application command port:

```rust
use taskit_engine::{Command, Ctx};
use taskit_types::error::TaskitError;

struct Verify;

impl Command for Verify {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        println!("verifying {}", ctx.root().display());
        Ok(())
    }
}
```

The public structs in `command` carry already-parsed arguments and delegate to engine modules. The
root binary currently maps them into these CLI groups:

| CLI group  | Command wrappers and responsibilities                                     |
| ---------- | ------------------------------------------------------------------------- |
| `check`    | `Fmt`, `Lint`, `Quick`, `Ci`, `CompileTests`, `CheckDeps`, hooks.         |
| `test`     | `Test`, `Coverage`, `Proptest`, `Fuzz`, `Bench`, reports, snapshots.      |
| `health`   | `Health`, `Drift`, `Inspect`, `Version`.                                  |
| `protocol` | Drift/sites checks, TODO sync, freshness, and `Audit`.                    |
| `dev`      | Build, bootstrap, hooks, setup, clean, update, and Claude version update. |
| `release`  | Version bump, crates.io publish, and GitHub release creation.             |
| `flow`     | Status, sync, promote, guard, and resumable automatic promotion.          |
| `self`     | Install, self-test, and required-tool checks.                             |

`Init` is deliberately absent because initialization runs before a config-backed `Ctx` exists.
The interactive dashboard is also wired in the root binary rather than this command module.

## Pipeline builder

`step::Pipeline` executes ordered closures and returns a structured `PipelineOutcome`. A gate
failure always skips subsequent steps. A normal failure skips subsequent steps only when
`fail_fast` is enabled. Skipped steps remain in the outcome with zero duration.

```rust
use taskit_engine::step::Pipeline;
use taskit_types::error::TaskitError;

let outcome = Pipeline::new(true)
    .gate("preflight", || Ok(()))
    .step("fmt", || Ok(()))
    .step("lint", || Err(TaskitError::other("clippy failed")))
    .step("test", || Ok(()))
    .run();

assert!(!outcome.passed);
assert_eq!(outcome.results.len(), 4);
```

`with_context` attaches run provenance. `step_with_context_sink` and
`gate_with_context_sink` attach reproduction commands and command logs. `step_with_diagnostics`
adds per-finding records used by SARIF output. `DiagnosticSink` and `StepContextSink` are
`Rc<RefCell<...>>` aliases, so the pipeline builder is synchronous and local-thread oriented.

Run the source-backed example with:

```bash
cargo run -p taskit-engine --example pipeline_builder
```

## CI behavior

`ci::run` chooses between configured and built-in steps:

- non-empty `[[ci.steps]]` entries are dispatched in configuration order;
- an explicit `[ci]` with no steps runs an empty passing pipeline; and
- no `[ci]` section runs the built-in pipeline.

The built-in pipeline runs self-check as a gate, then formatting, lint, test compilation, tests,
unused-dependency checks, protocol drift, and optional configured coverage. CI excludes network
tests by default; the CLI's `--include-network` reverses that behavior. CLI fail-fast or
`[ci].fail_fast = true` enables normal-step short-circuiting.

Configured CI step `cmd` values currently recognize these internal keys:

```text
fmt [--check]
lint
compile-tests
test
coverage
check-deps
check-protocol-drift
self-check
health
health-gate
```

These keys are configuration dispatch identifiers, not the current grouped CLI spelling. Unknown
keys return a failed outcome. Extra trailing arguments are not strictly rejected yet.

`[ci].cruxfile` is parsed but currently ignored by CI dispatch. `BuiltinRunner` and
`SubprocessCruxRunner` implement `taskit_core::pipeline_runner::PipelineRunner`, but
`taskit check ci` calls engine functions directly instead of selecting either adapter.

## Public module areas

| Area            | Modules                                                                        |
| --------------- | ------------------------------------------------------------------------------ |
| Build and setup | `bootstrap`, `build`, `clean`, `dev_setup`, `hooks`, `install`, `update`.      |
| Quality         | `fmt`, `lint`, `quick`, `ci`, `check_deps`, `check_freshness`, `audit`.        |
| Testing         | `testing::{run, compile, coverage, proptest, fuzz, bench, report, snapshot}`.  |
| Governance      | `health`, `inspect`, `drift`, `protocol`, `todo_sync`, `telemetry`.            |
| Release         | `patch`, `publish`, `release`, `changelog`, `version`.                         |
| Git flow        | `flow`, `flow_state_store`.                                                    |
| Infrastructure  | `affected`, `cache`, `config`, `ctx`, `discovery`, `progress`, `step`, `util`. |

Many module-level `run` functions are public because command wrappers and downstream workspace
crates use them directly. Prefer the typed `Command` wrappers when reproducing CLI dispatch; use
module APIs when embedding one focused operation.

## Flow and persistence

Flow operations enforce configured branch names and clean-worktree requirements. Promotion moves
`develop -> staging -> release -> main`, then synchronizes main back into develop where
appropriate. `FlowAction::Auto` receives an injected `ConflictResolver` and CI closure, allowing
tests and the binary composition root to select implementations.

Automatic flow persists resumable state under `target/taskit/state.json`, records telemetry, and
can optionally push configured branches after local success. Integration tests build isolated git
repositories and cover branch guards, promotions, conflict resolution, CI failure, resume state,
and optional push behavior.

## Development and testing

```bash
cargo check -p taskit-engine
cargo clippy -p taskit-engine --all-targets -- -D warnings
cargo nextest run -p taskit-engine
cargo test -p taskit-engine --test flow_integration
```

The crate has extensive module-local unit/property tests and `tests/flow_integration.rs`. Some
commands require external tools such as cargo-nextest, cargo-deny, cargo-llvm-cov, git-cliff,
Criterion tooling, or cargo-fuzz; unit tests generally isolate parsing/orchestration from those
executables. Changes to `command.rs` and `discovery.rs` are tracked protocol surfaces in this
workspace and may require an intentional protocol lockfile update.
