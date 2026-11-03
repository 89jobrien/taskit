# taskit-crux

`taskit-crux` is the workspace's prospective embedded Crux adapter. It defines
`EmbeddedCruxRunner`, an implementation of the `taskit-core` `PipelineRunner` port.

## Workspace role

The intended boundary is:

```text
taskit-core::PipelineRunner
             ^
             |
taskit-crux::EmbeddedCruxRunner
```

The crate depends on `taskit-core` and `taskit-types`. It has no Cargo feature flags. Although the
package description says the adapter is behind a feature flag, no workspace feature currently
enables or disables it.

## API

Construct a runner with the Cruxfile path and invoke it through the port:

```rust
use std::path::{Path, PathBuf};

use taskit_core::pipeline_runner::PipelineRunner;
use taskit_crux::EmbeddedCruxRunner;

let runner = EmbeddedCruxRunner::new(PathBuf::from("Cruxfile"));
let outcome = runner.run_pipeline(Path::new("taskit.toml"), false)?;
assert!(outcome.passed);
# Ok::<(), taskit_types::error::TaskitError>(())
```

## Current implementation status

The adapter is a stub, not an embedded Crux runtime. `run_pipeline` currently:

1. checks whether the path passed to `EmbeddedCruxRunner::new` exists;
2. returns `TaskitError::Io` when it does not; and
3. otherwise returns one synthetic passing step named `crux-embedded`.

It does not read, parse, validate, or execute the Cruxfile. It also ignores the `config_path` and
`fail_fast` arguments supplied to `PipelineRunner::run_pipeline`.

The root `taskit` package does not depend on `taskit-crux`, and the production `taskit check ci`
path does not construct this adapter. `taskit-engine` also has a separate
`SubprocessCruxRunner`, but configured `[ci].cruxfile` values are not currently used to select
either runner. Use taskit's built-in CI path for current production behavior.

## Testing and development

The unit tests verify missing-path errors and apply the shared `taskit-core` conformance helpers to
the synthetic successful outcome. `taskit-testing` is a development dependency but is not used by
the current tests.

```bash
cargo check -p taskit-crux
cargo clippy -p taskit-crux --all-targets -- -D warnings
cargo nextest run -p taskit-crux
```

A real implementation must replace the synthetic result, preserve `PipelineOutcome` invariants,
and define how Crux failures, diagnostics, gates, and fail-fast behavior map into taskit contracts.
