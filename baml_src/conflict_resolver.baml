/// A single file with merge conflicts awaiting resolution.
class ConflictInput {
    path string @description("Relative file path")
    ours string @description("Content of the file on the current branch (HEAD)")
    theirs string @description("Content of the file on the branch being merged in (MERGE_HEAD)")
    base string? @description("Raw file content including conflict markers, if available")
}

/// A file with its conflict fully resolved.
class ResolvedConflict {
    path string @description("Relative file path — must match the input path exactly")
    content string @description("The fully resolved file content, with no conflict markers remaining")
    confidence float @description("0–1 confidence that the resolution is correct")
    rationale string @description("One sentence explaining the resolution strategy")
}

/// Outcome for a batch of conflicts.
class ConflictResolution {
    resolved ResolvedConflict[] @description("Files that were successfully resolved")
    needs_human string[] @description("Paths that could not be resolved confidently (confidence < 0.7) — escalate these")
}

function ResolveConflict(files: ConflictInput[]) -> ConflictResolution {
    client ConflictResolver
    prompt #"
        You are an expert software engineer resolving Git merge conflicts.

        For each file in the input:
        - Study `ours` (current branch) and `theirs` (incoming branch) carefully.
        - Produce a merged result that preserves the intent of BOTH sides where possible.
        - If the changes are incompatible and you cannot resolve with >= 0.7 confidence,
          add the path to `needs_human` instead.
        - Never output conflict markers (<<<<<<<, =======, >>>>>>>).
        - Set `content` to the complete, valid file content after resolution.

        Files to resolve:
        {{ files }}

        {{ ctx.output_format }}
    "#
}
