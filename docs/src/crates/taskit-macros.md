# taskit-macros

`taskit-macros` is a proc-macro crate with two exports.

## `#[taskit_test]`

The attribute supports bare tests plus `tempdir`, `offline`, or both. `tempdir` wraps the test with
`taskit_testing::TempDirGuard`; `offline` returns early when `TASKIT_OFFLINE=1`.

## `#[derive(ConfigDefaults)]`

The derive generates accessors for struct fields annotated with `#[default_value = "..."]`.

Both macros are currently exercised by this crate's integration tests. `taskit-engine` and
`taskit-init` list the crate only under dev-dependencies; production source does not invoke it.
