# taskit-testing

`taskit-testing` provides small shared test utilities for taskit crates: a temporary-directory
guard and declarative macros for temporary working directories and `StepResult` construction.

## Workspace role

This is a development-support crate over `taskit-types`; it does not provide production pipeline
execution. Runner conformance helpers live in `taskit_core::conformance`, not here. The crate has no
Cargo feature flags.

## `TempDirGuard`

`TempDirGuard::new` creates a `tempfile::TempDir`, records the current process working directory,
and changes the process working directory to the temporary directory. Dropping the guard attempts
to restore the original directory. `path` exposes the temporary path, and `Default` calls `new`.

```rust
use taskit_testing::TempDirGuard;

let original = std::env::current_dir()?;
{
    let guard = TempDirGuard::new();
    std::fs::write(guard.path().join("marker.txt"), "isolated")?;
}
assert_eq!(std::env::current_dir()?, original);
# Ok::<(), std::io::Error>(())
```

### Process-wide current-directory warning

Changing the current directory affects the entire process, not one test or thread. Concurrent uses
can race, observe another test's directory, or restore directories in the wrong order. Tests that
use `TempDirGuard`, `in_temp_dir!`, or `#[taskit_test(tempdir)]` must be serialized until the
tracked process-wide isolation issue is resolved. The constructor also intentionally panics if it
cannot create the directory or read/set the current directory; drop restoration is best effort.

## Exported macros

### `in_temp_dir!`

Run a block with a live guard. The second form binds the temporary path:

```rust
use taskit_testing::in_temp_dir;

let exists = in_temp_dir! { dir =>
    std::fs::write(dir.join("file.txt"), "data").unwrap();
    dir.join("file.txt").exists()
};
assert!(exists);
```

### `step_result!`

Construct `taskit_types::step::StepResult` values with empty diagnostics and default context:

```rust
use std::time::Duration;

use taskit_testing::step_result;

let passed = step_result!("fmt", Pass);
let failed = step_result!(
    "test",
    Fail,
    error: "assertion failed",
    gate: true,
    duration: Duration::from_secs(2)
);

assert_eq!(passed.name, "fmt");
assert!(failed.gate);
```

Supported forms are status only, or status with `error`, `gate`, `duration`, `error + gate`, or
`error + gate + duration` in the exact order defined by the macro. Because expansion refers to
`::taskit_types`, downstream crates using this macro must make `taskit-types` available under that
crate name. `__step_result_inner!` is exported for macro expansion but is hidden from docs and is
not intended as the user-facing entry point.

## Deliberate non-API

`src/helpers.rs` currently contains only crate-internal tests; it exports no helper functions.
This crate does not export `TempWorkspace`, fake runners, fake outcomes, or a conformance suite.

## Development

```bash
cargo check -p taskit-testing
cargo clippy -p taskit-testing --all-targets -- -D warnings
cargo nextest run -p taskit-testing
```

When running the crate's own current-directory tests in parallel, use nextest's test isolation
configuration or a single test thread if failures indicate process-wide CWD interference.
