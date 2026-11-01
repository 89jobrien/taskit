# Installation

## Requirements

Taskit requires Rust and Cargo. The workspace uses Rust edition 2024 and does not declare a
minimum supported Rust version.

```bash
rustc --version
cargo --version
```

## Install taskit

```bash
cargo install taskit
```

To install the current checkout instead:

```bash
taskit self install
```

`self install` runs `cargo install --path . --force`, so use it from the taskit repository.

## Install workflow tools

```bash
taskit dev setup
```

The setup command installs these required tools through `cargo binstall`:

- `cargo-nextest`
- `cargo-llvm-cov`
- `cargo-deny`
- `cargo-machete`

It installs `cargo-binstall` first when necessary. `sccache` is optional, and `cargo-sweep` is
needed only for age-based cleanup. Individual features also require their native tools:

| Feature                                  | Tool         |
| ---------------------------------------- | ------------ |
| Flow and repository discovery            | `git`        |
| GitHub releases and TODO synchronization | `gh`         |
| Fuzz targets                             | `cargo-fuzz` |
| Changelog generation                     | `git-cliff`  |

Verify taskit's own required tools with:

```bash
taskit self check
```

## Discover commands

```bash
taskit --help
taskit check --help
taskit test coverage --help
```

Commands are grouped under `dev`, `check`, `test`, `health`, `protocol`, `release`, `flow`, and
`self`. `dashboard`, `init`, and `changelog` are top-level commands.
