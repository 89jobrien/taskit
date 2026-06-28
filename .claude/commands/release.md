# release

Execute a full release of the taskit workspace.

## Arguments

- `$ARGUMENTS` -- optional bump level: `patch` (default), `minor`, `major`,
  `prerelease`, `release`

## Workflow

### 1. Pre-flight

Run `/release-check` first. If any check fails, stop.

### 2. Confirm

Show the user:

- Current version (from root Cargo.toml)
- Target version after bump
- Tag that will be created
- Crates that will be published to crates.io

Ask for explicit confirmation before proceeding.

### 3. Changelog

Generate changelog for the release:

```bash
git-cliff --unreleased --prepend CHANGELOG.md
```

Show the generated changelog section to the user. Stage the file:

```bash
git add CHANGELOG.md
```

### 4. Execute

Parse bump level from `$ARGUMENTS` (default: `patch`).

```bash
cargo rail release run --all --bump <level>
```

If `cargo rail` is not configured, fall back to manual lockstep bump:

```bash
# bump all Cargo.toml versions
# git add -A
# git commit -m "chore(release): taskit v<new_version>"
# git tag v<new_version>
```

### 5. Push

After successful release commit and tag:

```bash
git push
git push --tags
```

This triggers `.github/workflows/release.yml` which builds binaries for:

- x86_64-unknown-linux-musl
- aarch64-unknown-linux-musl
- x86_64-apple-darwin
- aarch64-apple-darwin

### 6. Verify

```bash
gh run list --workflow=release.yml --limit 1
```

Report the workflow run URL so the user can monitor it.

### 7. Post-release

- Do NOT skip `--tags` on push
- Do NOT use `--no-verify` on any git operation
- Never force-push tags
