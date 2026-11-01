# taskit

`taskit` is a config-driven development and CI runner for Rust workspaces. One grouped CLI covers
formatting, linting, tests, coverage, dependency governance, health baselines, release operations,
Git flow, and a live terminal dashboard.

```bash
taskit check quick
taskit check ci
taskit dashboard
```

Configuration is optional. Without `taskit.toml`, taskit discovers workspace packages with Cargo
metadata and uses its built-in CI pipeline. Adding configuration enables propagation rules,
custom pipeline steps, protocol surfaces, coverage thresholds, flow branches, release settings,
and output defaults.

## Where to start

- [Install taskit](./getting-started/installation.md).
- [Initialize and check a workspace](./getting-started/first-workspace.md).
- Use the [CLI reference](./reference/cli.md) for every current command and option.
- Use the [configuration reference](./reference/configuration.md) for `taskit.toml`.
- Open the [dashboard guide](./guides/dashboard.md) for tabs, keys, and health actions.

## Core workflows

| Goal                         | Command                        |
| ---------------------------- | ------------------------------ |
| Fast affected-crate feedback | `taskit check quick`           |
| Full local CI                | `taskit check ci`              |
| Workspace tests              | `taskit test run`              |
| Update the health baseline   | `taskit health check --update` |
| Check protocol surfaces      | `taskit protocol drift`        |
| Inspect the branch pipeline  | `taskit flow status`           |
| Open the terminal dashboard  | `taskit dashboard`             |

## Design

Taskit uses a multi-crate hexagonal layout. Shared values live in `taskit-types`, ports in
`taskit-core`, orchestration in `taskit-engine`, and outer adapters in the binary, initializer,
output, dashboard, and Crux crates. See the [architecture overview](./architecture/overview.md).
