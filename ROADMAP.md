# taskit Roadmap

Taskit is a config-driven CI pipeline runner with affected-crate detection,
protocol-drift tracking, and pipeline orchestration. It currently exposes 11
top-level commands and 51 leaf commands across `dev`, `check`, `test`, `health`,
`protocol`, `release`, `flow`, `self`, `dashboard`, `init`, and `changelog`.

This roadmap covers the gaps identified in a review of workspace-manipulation
and architecture-tooling surface. Items are ordered by dependency: manifest
safety must land before selective version bumping, which must land before
topology and cycle analysis are useful on top of it.

## Phase 1: Manifest Safety

### 1.1 Targeted version rewriting in `patch.rs`

`crates/taskit-engine/src/patch.rs` rewrites manifests with a blanket string
replacement:

```rust
content.replace(&format!("version = \"{old}\""), &format!("version = \"{new}\""))
```

This corrupts any third-party dependency pinned at the same semver. Given:

```toml
[dependencies]
taskit-core = { version = "0.8.0", path = "crates/taskit-core" }
syn = { version = "0.8.0", features = ["full"] }
```

a bump to `0.9.0` rewrites both lines, silently repinning `syn` to a version
that does not exist.

Replace with format-preserving edits via `toml_edit`:

- touch `[package].version` only;
- touch `[workspace.dependencies]` pins only for known own-crate names, matched
  by name rather than by version string;
- leave comments, key ordering, and formatting untouched;
- leave all third-party pins untouched.

Callers pass `own_crates` from the `cargo_metadata` call already present in
`crates/taskit-engine/src/ctx.rs`, so no new discovery path is needed.

Add `toml_edit = "0.25"` to `[workspace.dependencies]`. It brings `indexmap` and
`winnow`, both already in `Cargo.lock`.

Exit condition: the four `replace_version` unit tests at `patch.rs:174-203` are
replaced by equivalent `bump_manifest` tests covering the collision case, and
the three `run` tests at `patch.rs:229-263` still pass.

### 1.2 Discover manifest paths from metadata

`cargo_toml_paths` (`patch.rs:52`) walks a hardcoded `crates/` directory. Any
member elsewhere — `xtask/`, a non-`crates` layout — is never bumped. Derive
paths from `cargo_metadata` workspace members instead.

## Phase 2: Selective Version Bumping

### 2.1 Downstream propagation on bump

`release patch|minor|major` bumps every crate in the workspace. There is no way
to bump one crate and carry the change to its dependents, which means a shared
crate release either over-bumps the leaves or requires manual intervention.

Add crate selection to the release subcommands and propagate through the
existing `[[workspace.propagation]]` tables in `taskit.toml`, reusing the same
data `affected::detect` already reads rather than introducing a second
propagation model.

Depends on 1.1: selective bumping against the current string replacement widens
the corruption surface, because a partial bump will no longer rewrite every
matching line by coincidence.

### 2.2 Fixpoint propagation in `affected::detect`

`apply_propagation` (`crates/taskit-engine/src/affected.rs:44`) performs a
single-pass expansion and documents the limitation: it is correct only when no
source also appears as a dependent. With 1.1 and 2.1 in place, transitive chains
become reachable, so this needs a fixpoint loop and a test covering
`a → b → c`.

## Phase 3: Topology and Cycles

### 3.1 `taskit check topo`

`taskit-init` computes a topological crate ordering at
`crates/taskit-init/src/plan.rs:468` (`topo_sort_members`) but exposes no CLI
surface. Publish order is computed and discarded.

Promote the function out of `taskit-init` into `taskit-engine` and add a
`check topo` subcommand. Flags worth having:

- `--layered` — group crates into dependency layers rather than emitting a flat
  ordering;
- `--crate <name>` — restrict to the subgraph reachable from one crate;
- `--reverse` — emit dependents-first, which is the order you want when
  reasoning about a change's blast radius.

### 3.2 `taskit check cycles`

No cycle detection exists. `petgraph::algo::tarjan_scc` over the internal-crate
dependency graph from `cargo_metadata` detects strongly connected components of
size > 1 (real cycles) or a single node with a self-edge. Fail the gate, or warn
under `--warn-only`.

Add `petgraph = "0.6"` to `[workspace.dependencies]`.

## Phase 4: Architecture Tooling

### 4.1 `taskit check graph`

Architecture layer violations are currently found by ad-hoc analysis outside the
tool. Build the check into taskit so it can run as a CI gate.

Design notes:

- **Rings belong in config, not code.** Declare them as `[workspace.rings]` in
  `taskit.toml`, alongside the existing `[[workspace.propagation]]` tables. A
  project that is not hexagonal should not have to adopt taskit's layer names.
- **An edge is a violation when it skips a ring** — `|to - from| > 1`. Adjacent
  ring edges are legal; the interesting failure is a leaf depending on an
  adapter.
- **Filter to workspace members.** `Package.deps` includes third-party crates;
  `cargo_metadata` gives `workspace_members` to partition on. Honour
  `NodeDep.dep_kinds` so dev-dependencies and build-dependencies are scored
  differently from normal dependencies.
- **Optional `Dot` output** via `petgraph::dot::Dot` for graphviz rendering.

Worth building only if this runs as a gate. Otherwise `cargo tree` plus external
tooling is sufficient, and the config surface is not worth maintaining.

### 4.2 Import ordering

`check fmt` shells out to `cargo fmt`, which does not reorder imports across
groups. Add `group_imports = "StdExternalCrate"` to a workspace `rustfmt.toml`
and a corresponding `imports_granularity` setting.

No new dependency, and no new command — this is configuration plus a
`rustfmt.toml` at the workspace root.

## Dependency Graph

```text
1.1 targeted version rewriting  (toml_edit)
  └─> 1.2 metadata-derived manifest paths
        └─> 2.1 downstream propagation
              └─> 2.2 fixpoint propagation

3.1 check topo        (extract topo_sort_members)
3.2 check cycles      (petgraph)
4.1 check graph       (petgraph + [workspace.rings] config)
4.2 import ordering   (rustfmt.toml, no code)
```

Phases 3 and 4 are independent of each other and of Phases 1 and 2.

## Out of Scope

Deliberately not planned:

- **A dependency on any external workspace-manipulation crate.** Taskit already
  depends on `cargo_metadata` for workspace discovery, which is the actual
  primitive needed here. Adding a facade crate over it would import its
  transitive tree and its maintenance risk without replacing any logic.
- **Prefix-group re-export facades.** Taskit uses `[workspace.dependencies]` and
  `crate_name.workspace = true` rather than umbrella crates that `pub use`
  everything. Adding the facade pattern would be a regression.
- **General file filtering DSLs.** Sufficiently niche for the current surface;
  `check fmt --affected` and `check lint --affected` cover the real cases.
- **Doc generation and category-slug pruning.** Both are cosmetic repository
  hygiene with no gate value.
