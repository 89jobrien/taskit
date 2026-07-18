# taskit-crux

Optional crate providing `EmbeddedCruxRunner` — a `PipelineRunner` implementation that links
`crux` directly into the binary rather than spawning a subprocess.

Gated behind the `crux` feature flag. When the feature is disabled the crate compiles to an
empty stub and the binary falls back to `BuiltinRunner` or `SubprocessCruxRunner`.

## When to use

Enable the `crux` feature when you need lower-latency pipeline execution and have `crux`
available as a library dependency. For most CI use cases `BuiltinRunner` is sufficient.

```toml
# Cargo.toml
[dependencies]
taskit = { version = "...", features = ["crux"] }
```
