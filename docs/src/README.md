# taskit

`taskit` is a config-driven CI pipeline runner for Rust workspaces. It provides a single binary
that orchestrates formatting, linting, testing, coverage, dependency auditing, and git branching
workflows — all driven by a `taskit.toml` at the workspace root.

## Quick start

```bash
taskit init          # scaffold taskit.toml, Cruxfile, hooks, CI, docs
taskit quick         # fast local feedback (fmt + lint + compile-tests + test)
taskit ci            # full CI pipeline
taskit flow auto     # promote develop → staging → release → main with CI gate
```

## Design principles

- **Config-driven** — all pipeline behaviour lives in `taskit.toml`; no magic
- **Hexagonal** — port traits in `taskit-core`, adapters in `taskit-engine`; easy to test
- **Fail-fast** — gates abort the pipeline immediately; non-gates report and continue
- **Rich diagnostics** — `miette`-powered errors with source spans, help text, and codes
- **Resumable** — `flow auto` persists state to `.taskit-state.json` and resumes on re-run

## Workspace layout

```
taskit (root bin)
├── crates/taskit-types    shared types: Config, Error, StepResult, ConflictFile
├── crates/taskit-core     port traits: PipelineRunner, ConflictResolver
├── crates/taskit-engine   pipeline engine, config loading, flow commands
├── crates/taskit-init     `taskit init`: discovery + file generation
├── crates/taskit-crux     EmbeddedCruxRunner (optional, `crux` feature)
├── crates/taskit-macros   proc-macros for derive utilities
├── crates/taskit-output   OutputFormatter trait + implementations
└── crates/taskit-testing  shared test helpers and conformance harness
```

See [Architecture Overview](./architecture/overview.md) for the dependency graph and design
rationale.
