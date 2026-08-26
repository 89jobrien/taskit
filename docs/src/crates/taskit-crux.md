# taskit-crux

Crate providing `EmbeddedCruxRunner` — a `PipelineRunner` implementation intended for an embedded
Cruxfile runtime.

The current implementation is a stub: it checks that the configured Cruxfile path exists and then
returns a synthetic passing `crux-embedded` step. Full embedded execution is blocked on an
available `crux-script` runtime.

## When to use

Use `BuiltinRunner` for normal CI execution today. Use `SubprocessCruxRunner` when you want to
run an external `crux run <path>` process.

```toml
# Cargo.toml
[dependencies]
taskit-crux = { version = "0.8.0", path = "crates/taskit-crux" }
```
