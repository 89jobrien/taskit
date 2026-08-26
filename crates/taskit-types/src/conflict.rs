/// A file with merge conflicts, with both sides captured for resolution.
#[derive(Debug)]
#[non_exhaustive]
pub struct ConflictFile {
    /// Path of the conflicted file relative to workspace root.
    pub path: String,
    /// Content from the current branch side of the merge.
    pub ours: String,
    /// Content from the incoming branch side of the merge.
    pub theirs: String,
    /// The raw file content including conflict markers (base context).
    pub base: Option<String>,
}

impl ConflictFile {
    /// Construct a conflict payload for one file.
    pub fn new(
        path: impl Into<String>,
        ours: impl Into<String>,
        theirs: impl Into<String>,
        base: Option<String>,
    ) -> Self {
        Self {
            path: path.into(),
            ours: ours.into(),
            theirs: theirs.into(),
            base,
        }
    }
}

/// A file with its conflict resolved to a final content string.
#[derive(Debug)]
#[non_exhaustive]
pub struct ResolvedFile {
    /// Path of the resolved file relative to workspace root.
    pub path: String,
    /// Final merged content to write to disk.
    pub content: String,
}

impl ResolvedFile {
    /// Construct a resolved file payload.
    pub fn new(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
        }
    }
}
