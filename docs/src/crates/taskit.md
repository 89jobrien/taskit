# taskit (binary)

The root package is the composition root and installed `taskit` binary.

## Entry points

- `src/main.rs` defines the grouped Clap CLI, configuration loading, `Ctx`, output sink, command
  dispatch, completion generation, and dashboard adapter.
- `src/flow_resolver.rs` implements the BAML-backed `ConflictResolver` used by `flow auto`.
- A root-local no-op resolver escalates conflicts when `[flow].conflict_resolver = "none"`.

`taskit init` is dispatched before normal config loading because it may create `taskit.toml`.
Other commands discover config, switch to the resolved workspace root, then dispatch through
`taskit_engine::Command`.

## Global CLI options

```text
--dry-run
--output human|compact|json|github|junit|diagnostic|sarif
```

There is no global `--config` option. See [CLI Commands](../reference/cli.md).

## Direct dependencies

The binary depends on `taskit-types`, `taskit-core`, `taskit-engine`, `taskit-init`,
`taskit-output`, and `taskit-tui`. `taskit-crux` is not linked into the binary.
