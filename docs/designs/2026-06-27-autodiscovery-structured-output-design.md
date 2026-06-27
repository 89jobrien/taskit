# Design: Auto-discovery and Structured Output

## Goal

Make taskit a reusable library for embedding custom xtask binaries, with
auto-discovery of workspace crates/propagation/surfaces from cargo metadata
and conventions, and structured pipeline output in JSON, GitHub Actions, and
JUnit XML formats.

## Approved Approach

"Both, sequenced" -- auto-discovery ships in v0.2, structured output in v0.3,
lib API polish in v0.4. Each release is independently useful. Primary consumer:
maestro's xtask, then personal projects, then public crates.io users.

## Crate Ownership

- **Owner crate**: `taskit` -- single crate, no workspace split
- **Affected crates**: none (maestro's xtask will consume taskit as a
  dependency in a follow-on change, not part of this design)

## Context Map

### Files affected by auto-discovery (v0.2)

| File               | Change                                                        |
| ------------------ | ------------------------------------------------------------- |
| `src/config.rs`    | Add `Config::discover()`, `MetadataSource` trait, merge logic |
| `src/discovery.rs` | **NEW** -- cargo metadata parsing, dep graph, convention scan |
| `Cargo.toml`       | Add `cargo_metadata` dependency                               |

### Files affected by structured output (v0.3)

| File            | Change                                                      |
| --------------- | ----------------------------------------------------------- |
| `src/step.rs`   | `Pipeline::run()` returns `PipelineOutcome` instead of `()` |
| `src/output.rs` | **NEW** -- `OutputFormat`, `OutputFormatter` trait, 4 impls |
| `src/main.rs`   | Add `--output` global flag, format+write outcome after run  |
| `src/ci.rs`     | Propagate `PipelineOutcome` from `run()` callers            |
| `src/quick.rs`  | Same                                                        |
| `src/hooks.rs`  | Same                                                        |
| `Cargo.toml`    | Add `quick-xml` dependency (JUnit)                          |

### Files NOT touched

All step modules (`fmt.rs`, `lint.rs`, `testing/*.rs`, etc.) retain their
current signatures. The output change is at the pipeline level, not the
step level.

---

## Public API

### Traits

```rust
/// Port: abstracts cargo metadata retrieval for testability.
/// src/discovery.rs
pub trait MetadataSource {
    fn workspace_members(&self) -> Result<Vec<DiscoveredCrate>>;
    fn intra_workspace_deps(&self) -> Result<Vec<(String, String)>>;
}

/// Port: formats pipeline results for different output targets.
/// src/output.rs
pub trait OutputFormatter {
    fn render(&self, outcome: &PipelineOutcome) -> String;
}
```

### Types

```rust
/// src/discovery.rs
pub struct DiscoveredCrate {
    pub dir: String,
    pub pkg: String,
    pub manifest_path: PathBuf,
}

/// src/discovery.rs
pub struct DiscoveredSurface {
    pub name: String,
    pub path: String,
}

/// src/discovery.rs -- adapter implementing MetadataSource
pub struct CargoMetadataSource {
    pub workspace_root: PathBuf,
}

/// src/step.rs
pub struct PipelineOutcome {
    pub results: Vec<StepResult>,
    pub total: Duration,
    pub passed: bool,
}

/// src/output.rs
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
    Github,
    Junit,
}

/// src/output.rs
pub struct HumanFormatter;
pub struct JsonFormatter;
pub struct GithubFormatter;
pub struct JunitFormatter {
    pub output_path: PathBuf,
}
```

### Functions

```rust
/// src/config.rs -- build Config from cargo metadata + conventions.
/// Called as fallback when no taskit.toml exists, or to fill gaps.
impl Config {
    pub fn discover(workspace_root: &Path) -> Result<Config>;
    pub fn discover_with(
        workspace_root: &Path,
        source: &dyn MetadataSource,
    ) -> Result<Config>;
}

/// src/discovery.rs -- convention-based surface detection
pub fn scan_surfaces(workspace_root: &Path) -> Result<Vec<DiscoveredSurface>>;

/// src/discovery.rs -- build propagation from dep graph edges
pub fn derive_propagation(
    deps: &[(String, String)],
    known_crates: &[String],
) -> Vec<PropagationEntry>;

/// src/output.rs -- factory
impl OutputFormat {
    pub fn formatter(self) -> Box<dyn OutputFormatter>;
    pub fn formatter_with_path(self, path: PathBuf) -> Box<dyn OutputFormatter>;
}
```

---

## Data Flow

### Auto-discovery (v0.2)

1. **Source**: `CargoMetadataSource` runs `cargo metadata --format-version 1`
   and parses the JSON via the `cargo_metadata` crate.
2. **Transform**: `workspace_members()` extracts crate dir+pkg pairs.
   `intra_workspace_deps()` extracts edges where both endpoints are workspace
   members. `derive_propagation()` converts edges to `PropagationEntry` list.
   `scan_surfaces()` globs for convention patterns and builds
   `DiscoveredSurface` entries.
3. **Sink**: `Config::discover()` assembles a `Config` from the above.
   `config::load()` calls `discover()` when no `taskit.toml` exists. When
   `taskit.toml` is present, explicit sections win entirely (no partial merge).

### Structured output (v0.3)

1. **Source**: `Pipeline::run()` executes steps, collects `Vec<StepResult>`.
2. **Transform**: `Pipeline::run()` returns `PipelineOutcome` (results, total
   duration, passed bool) instead of calling `print_summary()` directly.
3. **Sink**: Caller in `main.rs` passes `PipelineOutcome` to the selected
   `OutputFormatter`. Human goes to stderr, JSON to stdout, GitHub to stderr
   - `$GITHUB_STEP_SUMMARY`, JUnit to file.

---

## Hexagonal Boundaries

- **Port** (trait): `MetadataSource` in `taskit::discovery`
  - Abstracts cargo metadata so tests can inject fake workspace structures
  - Production adapter: `CargoMetadataSource` (calls real cargo)
  - Test adapter: `FakeMetadataSource` (in-memory, `#[cfg(test)]` only)

- **Port** (trait): `OutputFormatter` in `taskit::output`
  - Abstracts result rendering so new formats can be added without touching
    pipeline logic
  - Four adapters: `HumanFormatter`, `JsonFormatter`, `GithubFormatter`,
    `JunitFormatter`

---

## Merge Semantics (auto-discovery)

When `taskit.toml` exists:

| Config section          | Has entries in toml? | Behavior                 |
| ----------------------- | -------------------- | ------------------------ |
| `workspace.crates`      | yes                  | Use toml, skip discovery |
| `workspace.crates`      | no/empty             | Use discovered crates    |
| `workspace.propagation` | yes                  | Use toml, skip discovery |
| `workspace.propagation` | no/empty             | Use derived propagation  |
| `protocol.surfaces`     | yes                  | Use toml, skip discovery |
| `protocol.surfaces`     | no/empty             | Use convention scan      |
| `ci.steps`              | yes                  | Use toml                 |
| `ci.steps`              | no/empty             | No default (empty)       |

Key rule: no partial merge within a section. If a user defines one crate
manually, discovery is fully disabled for the crate list.

## Convention-based Surface Detection

Default glob patterns for `scan_surfaces()`:

| Pattern             | Derived surface name     |
| ------------------- | ------------------------ |
| `**/types.rs`       | `{crate}/types`          |
| `**/api.rs`         | `{crate}/api`            |
| `**/schema.graphql` | `{crate}/graphql-schema` |
| `**/schema.json`    | `{crate}/json-schema`    |
| `**/openapi.yml`    | `{crate}/openapi`        |
| `**/openapi.yaml`   | `{crate}/openapi`        |
| `**/openapi.json`   | `{crate}/openapi`        |
| `**/*.proto`        | `{crate}/{filename}`     |

Patterns exclude `target/` and hidden directories. Surface name is derived
from the relative path. Users can disable convention scanning entirely by
providing an explicit `[protocol]` section (even with `surfaces = []`).

## JSON Output Schema

```json
{
  "version": 1,
  "pipeline": "ci",
  "steps": [
    {
      "name": "fmt-check",
      "status": "pass",
      "duration_secs": 1.2,
      "error": null,
      "gate": true
    }
  ],
  "total_duration_secs": 22.3,
  "passed": true
}
```

`version: 1` for forward compatibility.

## GitHub Actions Output

- Each failed step emits `::error title={name}::...`
- Each passed step emits `::notice title={name}::...`
- If `$GITHUB_STEP_SUMMARY` env var is set, a markdown summary table is
  appended to that file (same layout as the human-readable table)

## JUnit XML

Standard `<testsuite>` / `<testcase>` structure:

```xml
<testsuites>
  <testsuite name="taskit-ci" tests="6" failures="1" time="22.3">
    <testcase name="fmt-check" time="1.2"/>
    <testcase name="test" time="14.7">
      <failure message="3 tests failed"/>
    </testcase>
  </testsuite>
</testsuites>
```

Default output path: `target/taskit-results.xml`, overridable with
`--output-file`.

---

## Pipeline::run() Change (breaking)

Current signature:

```rust
pub fn run(self) -> anyhow::Result<()>;
```

New signature:

```rust
pub fn run(self) -> PipelineOutcome;
```

`PipelineOutcome.passed` replaces the `Err` return for failures. This is a
breaking change for any lib consumer calling `Pipeline::run()` directly.
Since the lib API is not yet stabilized (v0.1.x), this is acceptable.

The `main.rs` caller changes from:

```rust
pipeline.run()?;
```

to:

```rust
let outcome = pipeline.run();
let fmt = cli.output.formatter();
output::write(&fmt, &outcome);
if !outcome.passed {
    std::process::exit(1);
}
```

---

## StepResult Enhancement

`StepResult` gains two fields for richer output:

```rust
pub struct StepResult {
    pub name: String,
    pub status: StepStatus,
    pub duration: Duration,
    pub error: Option<String>,  // NEW: error message if failed
    pub gate: bool,             // NEW: whether this was a gate step
}
```

These are populated during `Pipeline::run()` from existing data (the error
from `(ps.f)()` and `ps.is_gate`).

---

## Out of Scope

- Async runtime -- all steps remain synchronous
- Plugin system -- no user-supplied Rust step functions
- Windows CI gate -- not tested
- Partial config merge within a section
- Custom shell commands in `[[ci.steps]]` (may come in a future release)
- `taskit-core` crate extraction -- single crate is sufficient
- Maestro xtask migration -- separate follow-on work

## Risk

- [x] Breaking API changes: yes -- `Pipeline::run()` return type changes
      from `Result<()>` to `PipelineOutcome`. Acceptable at v0.x.
- [x] New external dependency: yes -- `cargo_metadata` (v0.2),
      `quick-xml` (v0.3). Both well-maintained, narrow scope.
- [ ] Feature flag required: no

## Semver Plan

| Release   | Contents                                                         |
| --------- | ---------------------------------------------------------------- |
| **0.2.0** | Auto-discovery, `cargo_metadata` dep, convention scanning        |
| **0.3.0** | Structured output, `PipelineOutcome`, `OutputFormat`, formatters |
| **0.4.0** | Lib API polish -- stabilize re-exports, builder ergonomics, docs |
