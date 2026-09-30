# Git Flow

Taskit's default branch pipeline is:

```text
main -> develop -> staging -> release -> main
```

Configure different names, conflict handling, and optional push behavior in `[flow]`.

## Inspect and synchronize

```bash
taskit flow status
taskit flow sync
```

Status is read-only and reports the current branch plus ahead/behind counts for each configured
hop. Sync requires the configured develop branch, a clean worktree except `.ctx/` changes, and a
local main branch. It creates a `--no-ff` merge from main into develop.

## Promote one stage

```bash
taskit flow promote
```

Promote advances the current branch:

- develop to staging
- staging to release
- release to main, then main back to develop

It changes branches and creates merge commits. Target branches must already exist locally.

## Run the complete flow

```bash
taskit flow auto
```

Fresh runs start on develop with a clean worktree and existing staging, release, and main branches:

1. merge develop into staging
2. merge staging into release
3. run the built-in CI pipeline on release with fail-fast
4. merge release into main
5. merge main back into develop
6. optionally push all configured branches

`flow auto` uses the built-in pipeline rather than configured `[[ci.steps]]`.

## Resumption and conflicts

Progress is stored atomically in:

```text
target/taskit/state.json
```

CI failure leaves the checkout on release and preserves failed-step state. Success clears the file
and finishes on develop.

With `conflict_resolver = "baml"`, conflicted files are passed to the BAML resolver, then written,
staged, and committed with `git commit --no-edit`. `none` requests manual resolution.

### Provider chain

The client chain is declared in `baml_src/clients.baml` and tried in order:

1. `Anthropic_Sonnet` (`ANTHROPIC_API_KEY`)
2. `OpenAI_GPT4o` (`OPENAI_API_KEY`)
3. `Ollama_Llama` — local daemon at `http://localhost:11434`, model `llama3.2`

BAML advances to the next client when a call fails, so a rejected or expired credential degrades to
the local daemon instead of failing the flow. A credential that is not set at all is different:
BAML cannot construct that client and aborts the chain
([boundaryml/baml#2055](https://github.com/BoundaryML/baml/issues/2055)), so when neither
`ANTHROPIC_API_KEY` nor `OPENAI_API_KEY` is set, `taskit` pins the call to `Ollama_Llama`
directly. `ollama pull llama3.2` and a running daemon are required for that path; if none is
reachable, the call fails with the connection error and the flow escalates.

## Optional push

```toml
[flow]
push = true
remote = "origin"
```

Push is disabled by default. When enabled, auto pushes main, develop, staging, and release to the
configured remote. Push failure preserves resumable finishing state.

Global dry-run performs read-only validation and prints Git and push commands without mutating
branches, flow state, or telemetry. It does not print state save/advance/clear intentions.

## Protected-branch guard

```bash
taskit flow guard
```

Guard fails on configured main or release and succeeds on other branches.
