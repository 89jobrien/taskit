# release-plan

Dry-run preview of what a release would do, without executing.

## Arguments

- `$ARGUMENTS` -- optional bump level: `patch` (default), `minor`, `major`,
  `prerelease`, `release`

## Workflow

1. Parse the bump level from `$ARGUMENTS` (default: `patch`).

2. Show current versions:

```bash
cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | "\(.name) \(.version)"'
```

3. Run the release dry-run for all crates:

```bash
cargo rail release run --all --bump <level> --check
```

4. Show the planned version bump (e.g. `0.4.0 -> 0.4.1`).

5. Show the git tag that would be created (e.g. `v0.4.1`).

6. Remind the user that pushing the tag triggers the GitHub Release workflow
   which builds binaries for linux-musl and macOS targets.

7. Do NOT execute the release. This is preview only.
