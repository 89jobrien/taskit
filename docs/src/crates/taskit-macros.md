# taskit-macros

Proc-macro crate. Provides derive utilities used across the taskit workspace.

Proc-macro crates must be a separate crate in Rust — this one exists purely to satisfy that
constraint and keep derive logic isolated from runtime code.

Depended on by `taskit-types` for any derives that require procedural macros beyond what
`serde`, `thiserror`, and `miette` provide out of the box.
