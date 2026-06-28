# changelog

Generate or update CHANGELOG.md from git history using git-cliff.

## Arguments

- `$ARGUMENTS` -- optional flags:
  - `unreleased` -- show only unreleased changes (default)
  - `full` -- regenerate entire changelog from all tags
  - `latest` -- show only the latest tagged release
  - `preview` -- print to stdout without writing file

## Workflow

1. Parse mode from `$ARGUMENTS` (default: `unreleased`).

2. Verify `cliff.toml` exists at workspace root. If not, stop and suggest
   running `git-cliff --init`.

3. Generate the changelog:
   - **unreleased** (default):

     ```bash
     git-cliff --unreleased --prepend CHANGELOG.md
     ```

   - **full**:

     ```bash
     git-cliff -o CHANGELOG.md
     ```

   - **latest**:

     ```bash
     git-cliff --latest --prepend CHANGELOG.md
     ```

   - **preview**:
     ```bash
     git-cliff --unreleased
     ```
     Print output and stop. Do not write to file.

4. If not preview mode, show the diff:

   ```bash
   git diff CHANGELOG.md
   ```

5. Do NOT commit automatically. The user decides when to commit, or the
   `/release` command handles it as part of the release flow.
