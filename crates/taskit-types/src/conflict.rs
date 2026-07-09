/// A file with merge conflicts, with both sides captured for resolution.
#[derive(Debug)]
#[non_exhaustive]
pub struct ConflictFile {
    pub path: String,
    pub ours: String,
    pub theirs: String,
    /// The raw file content including conflict markers (base context).
    pub base: Option<String>,
}

impl ConflictFile {
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
    pub path: String,
    pub content: String,
}

impl ResolvedFile {
    pub fn new(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
        }
    }
}
