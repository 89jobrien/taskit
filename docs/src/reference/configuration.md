# Configuration

Taskit discovers `taskit.toml` by walking up from the current directory. If none exists, it derives
workspace configuration from Cargo metadata. Every top-level section is optional; absence can
differ from an explicitly empty section.

## Workspace

```toml
[workspace]
root = "."
offline_skip = "not test(/.*network.*/)"

[[workspace.crates]]
dir = "crates/taskit-types"
pkg = "taskit-types"
exclude_from_version_check = false

[[workspace.propagation]]
source = "taskit-types"
dependents = ["taskit-core", "taskit-engine"]
```

| Field          | Absence/default                             |
| -------------- | ------------------------------------------- |
| `root`         | Directory containing the discovered config. |
| `crates`       | Populate from Cargo metadata.               |
| `propagation`  | Infer from local workspace dependencies.    |
| `offline_skip` | No additional nextest expression.           |

For each crate, `dir` is required, `pkg` defaults to `dir`, and
`exclude_from_version_check` defaults to false.

## Protocol

```toml
[protocol]
lockfile = "taskit-protocol.lock"

[[protocol.surfaces]]
name = "pipeline-runner"
path = "crates/taskit-core/src/pipeline_runner.rs"
```

The section is optional. `lockfile` defaults to `taskit-protocol.lock`; every surface requires a
name and path. When the section is absent, discovered trait surfaces are merged into configuration.

## CI

```toml
[ci]
fail_fast = false
cruxfile = "Cruxfile"

[[ci.steps]]
name = "fmt-check"
cmd = "fmt --check"
gate = false
```

| Field       | Default/behavior                                          |
| ----------- | --------------------------------------------------------- |
| `steps`     | Empty when `[ci]` exists.                                 |
| `cruxfile`  | Absent; currently parsed but not used to select a runner. |
| `fail_fast` | `false`.                                                  |

No `[ci]` section selects the built-in pipeline. An explicit empty section runs no steps and
succeeds. Each configured step requires `name` and `cmd`; `gate` defaults to false.

## Coverage

```toml
[coverage]
crate_name = "taskit-engine"
threshold = 80.0
```

`crate_name` is required when this section exists. The threshold defaults to 80 and must be above
zero and at most 100. This configured threshold applies to CI coverage steps; direct
`taskit test coverage` uses its CLI threshold, also defaulting to 80.

## Flow

```toml
[flow]
main = "main"
develop = "develop"
staging = "staging"
release = "release"
conflict_resolver = "baml"
push = false
remote = "origin"
```

All fields use the values shown as defaults. `conflict_resolver` accepts `baml` or `none`. Branch
names must be non-empty and distinct. Push is disabled unless explicitly enabled.

## Release

```toml
[release]
github_repo = "89jobrien/taskit"
publish_order = ["taskit-types", "taskit-core", "taskit-engine", "taskit"]
skip_docs = false
allow_dirty = false
```

The repository may be inferred from a GitHub `origin`. `publish_order` defaults to an empty list;
publishing then uses `taskit-types`, `taskit-testing`, `taskit-macros`, `taskit-output`,
`taskit-core`, `taskit-engine`, `taskit-init`, `taskit-crux`, `taskit-tui`, and `taskit` in that
order. Boolean fields default to false.

## Inspection thresholds

```toml
[inspect]
max_clippy_warnings = 0
max_clippy_errors = 0
max_test_failures = 0
max_todo_fixme = 10
```

All fields are optional. Missing Clippy/test limits use zero at inspection time. A missing TODO
limit disables that check. CLI warning and TODO limits override configuration.

## Cleanup

```toml
[clean]
older_than = "7d"
```

Without `older_than`, cleanup runs `cargo clean`. With it, taskit runs `cargo sweep --time N`.
Values are unsigned integers with an optional lowercase `d`.

## Output

```toml
[output]
default_format = "human"
verbose_on_failure = true
```

`default_format` accepts the formats in [Output Formats](./output-formats.md); invalid values fall
back to human. If `[output]` exists but omits `verbose_on_failure`, deserialization uses true; if
the whole section is absent, the derived config default is false. Current compact rendering always
enables verbose failure expansion regardless of either value.
