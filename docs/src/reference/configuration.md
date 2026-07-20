# Configuration

All configuration lives in `taskit.toml` at the workspace root. Generate a starter file with
`taskit init`; unused sections are commented out by default.

## `[workspace]`

```toml
[workspace]
offline_skip = "test(/.*network.*/)"   # nextest filter expression used by --offline
```

```toml
[[workspace.propagation]]
source = "taskit-types"
dependents = ["taskit-core", "taskit-engine", "taskit-init", "taskit-output"]
```

When `--affected` is passed, changing `taskit-types` automatically includes all listed
dependents.

## `[ci]`

```toml
[ci]
fail_fast = false
cruxfile = "Cruxfile"   # optional; enables crux-based step execution

[[ci.steps]]
name = "fmt --check"
cmd = "fmt --check"
gate = false

[[ci.steps]]
name = "lint"
cmd = "lint"
gate = false
```

## `[inspect]`

```toml
[inspect]
max_clippy_warnings = 0
max_clippy_errors   = 0
max_test_failures   = 0
max_todo_fixme      = 10
```

All fields are optional. Absent fields are not checked.

## `[clean]`

```toml
[clean]
older_than = "7d"   # pass to cargo-sweep; absent = full cargo clean
```

## `[coverage]`

```toml
[coverage]
crate_name = "taskit-engine"
threshold = 80.0        # minimum line coverage percentage
```

## `[flow]`

```toml
[flow]
main    = "main"
develop = "develop"
staging = "staging"
release = "release"
conflict_resolver = "baml"   # "baml" | "none"
```

All branch names are optional and default to the values shown. `conflict_resolver = "none"`
disables LLM resolution and escalates all conflicts directly to `FlowError::NeedsHuman`.

## `[release]`

```toml
[release]
github_repo   = "89jobrien/taskit"
publish_order = ["taskit-types", "taskit-core", "taskit-engine", "taskit"]
skip_docs     = false
allow_dirty   = false
```

## `[[protocol.surfaces]]`

```toml
[[protocol.surfaces]]
name = "pipeline-runner"
path = "crates/taskit-core/src/pipeline_runner.rs"

[[protocol.surfaces]]
name = "conflict-resolver"
path = "crates/taskit-core/src/conflict_resolver.rs"
```

Each surface is SHA-256 hashed and stored in `taskit-protocol.lock`. CI fails when the hash
diverges. Update with `taskit check-protocol-drift --update`.
