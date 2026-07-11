# Configuration

## taskit.toml

All configuration lives in `taskit.toml` at the workspace root.

### Sections

| Section | Purpose |
|---------|---------|
| `[workspace]` | Crate list, propagation rules, offline skip |
| `[protocol]` | Contract surface drift detection |
| `[coverage]` | Coverage enforcement |
| `[ci]` | Pipeline steps, fail_fast default |
| `[inspect]` | Metric thresholds (warnings, errors, TODOs) |
| `[clean]` | Artifact retention policy |
| `[flow]` | Git branching workflow |
| `[release]` | Publish order, GitHub repo, skip_docs, allow_dirty |
