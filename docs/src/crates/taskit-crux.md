# taskit-crux

`taskit-crux` contains `EmbeddedCruxRunner`, an implementation of the `PipelineRunner` port.

## Current status

The adapter is a stub. It verifies that the configured Cruxfile path exists, then returns a
synthetic passing `crux-embedded` step. It does not parse or execute the file.

The root taskit package does not depend on this crate, no feature enables it, and current CI does
not construct it. Use taskit's built-in CI path for production behavior.

The crate remains forward scaffolding for a future embedded Crux runtime.
