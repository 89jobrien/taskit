# taskit-core

Ports-only crate. Defines the boundary interfaces (traits) that adapters implement. Depends
only on `taskit-types`. Contains no I/O and no pipeline logic.

## Traits

### `PipelineRunner`

```rust
pub trait PipelineRunner {
    fn run_pipeline(
        &self,
        config_path: &Path,
        fail_fast: bool,
    ) -> Result<PipelineOutcome, TaskitError>;
}
```

Implemented by:

| Adapter | Crate | Description |
|---------|-------|-------------|
| `BuiltinRunner` | `taskit-engine` | Runs steps in-process via the step engine |
| `SubprocessCruxRunner` | `taskit-engine` | Spawns `crux` as a subprocess |
| `EmbeddedCruxRunner` | `taskit-crux` | Links crux directly (feature-gated) |

### `ConflictResolver`

```rust
pub trait ConflictResolver {
    fn resolve(
        &self,
        conflicts: &[ConflictFile],
    ) -> Result<Vec<ResolvedFile>, TaskitError>;
}
```

Implemented by:

| Adapter | Crate | Description |
|---------|-------|-------------|
| `BamlConflictResolver` | `taskit` (bin) | LLM resolution via BAML structured output |
| No-op impl | inline | Returns `NeedsHuman` for every conflict |

## Design note

Keeping traits in `taskit-core` (separate from `taskit-engine`) allows `taskit-testing` to
import only the port interface for conformance harnesses without pulling in the full engine.
