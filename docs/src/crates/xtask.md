# xtask

`xtask` is a publish-false Rust 2021 workspace package with no Cargo dependencies. It delegates
workspace chores to an installed `taskit` binary, falling back to `cargo run -p taskit --`.

Its current dispatch names are `fmt`, `fmt-check`, `lint`, `test`, `ci`, `pre-commit`, and
`pre-push`. The adapter forwards those legacy flat taskit arguments, so they do not match the
current grouped CLI and fail unless corrected. No `cargo xtask` alias exists in this repository's
`.cargo/config.toml`.

The package is generated integration scaffolding rather than part of taskit's runtime dependency
graph.
