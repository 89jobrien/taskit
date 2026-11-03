# taskit-macros

`taskit-macros` is taskit's procedural-macro crate. It exports a composable test attribute and a
derive for default-value accessors. Workspace crates currently use it as a development dependency;
the taskit production path does not invoke either macro.

## Workspace role

The crate is intentionally limited to token parsing and code generation. Runtime support for
temporary directories comes from `taskit-testing`, and generated configuration methods operate on
the consumer's own types. There are no Cargo feature flags.

## `#[taskit_test]`

The attribute always generates a synchronous `#[test]` and supports two flags:

| Form                               | Generated behavior                            |
| ---------------------------------- | --------------------------------------------- |
| `#[taskit_test]`                   | Ordinary zero-argument test.                  |
| `#[taskit_test(tempdir)]`          | Enter a `TempDirGuard` and bind `dir: &Path`. |
| `#[taskit_test(offline)]`          | Return early when `TASKIT_OFFLINE=1`.         |
| `#[taskit_test(tempdir, offline)]` | Apply both behaviors.                         |

```rust
use std::path::Path;

use taskit_macros::taskit_test;

#[taskit_test(tempdir)]
fn writes_in_an_isolated_directory(dir: &Path) {
    std::fs::write(dir.join("marker.txt"), "ok").unwrap();
    assert!(Path::new("marker.txt").exists());
}
```

The expansion clears the function's declared parameters and supplies only the local `dir` binding
when `tempdir` is enabled. Unknown flags are compile errors. The macro does not support async tests,
custom fixtures, or arbitrary function arguments.

The generated tempdir code refers to `::taskit_testing::TempDirGuard`, so consumers using the
`tempdir` flag must depend on `taskit-testing` under that crate name. `TempDirGuard` changes the
process-wide current directory; such tests must not run concurrently without external
serialization.

`offline` checks for the exact string `TASKIT_OFFLINE=1`. It prints a skip message and returns from
the test body; the test harness therefore records the test as passed rather than as a native
ignored test.

## `#[derive(ConfigDefaults)]`

The derive scans named struct fields carrying `#[default_value = "..."]` and generates public
methods with the same names as those fields:

```rust
use taskit_macros::ConfigDefaults;

#[derive(ConfigDefaults)]
struct Settings {
    #[default_value = "main"]
    branch: Option<String>,
    #[default_value = "80.0"]
    threshold: Option<f64>,
}

let settings = Settings {
    branch: None,
    threshold: Some(92.5),
};
assert_eq!(settings.branch(), "main");
assert_eq!(settings.threshold(), 92.5);
```

Supported input is a struct with named fields. Annotated `Option<f64>` fields generate an `f64`
getter and require a parseable floating-point default. All other annotated fields generate a
`&str` getter using `as_deref`, so in practice they must support the same operations as
`Option<String>`. Unannotated fields do not receive methods. Enums, tuple structs, malformed
defaults, and unsupported shapes produce compile errors or generated-code type errors.

## Testing and development

Integration tests exercise bare, tempdir, offline, combined, and default-accessor expansions:

```bash
cargo check -p taskit-macros
cargo clippy -p taskit-macros --all-targets -- -D warnings
cargo nextest run -p taskit-macros
```

Because this is a proc-macro crate, verify behavior through consuming integration tests rather than
only testing internal token-generation helpers.
