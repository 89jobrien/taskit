use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// RAII guard that sets cwd to a temporary directory and restores
/// the original cwd on drop.
pub struct TempDirGuard {
    _dir: TempDir,
    original: PathBuf,
}

impl TempDirGuard {
    /// Create a new temp directory and set it as the current directory.
    ///
    /// # Panics
    ///
    /// Panics if the temp directory cannot be created or if the current
    /// directory cannot be read or set.
    pub fn new() -> Self {
        let original = std::env::current_dir().expect("failed to read cwd");
        let dir = TempDir::new().expect("failed to create tempdir");
        std::env::set_current_dir(dir.path()).expect("failed to set cwd to tempdir");
        Self {
            _dir: dir,
            original,
        }
    }

    /// Path to the temporary directory.
    pub fn path(&self) -> &Path {
        self._dir.path()
    }
}

impl Default for TempDirGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cwd_changes_to_tempdir() {
        let before = std::env::current_dir().unwrap().canonicalize().unwrap();
        {
            let guard = TempDirGuard::new();
            let during = std::env::current_dir().unwrap().canonicalize().unwrap();
            let guard_path = guard.path().canonicalize().unwrap();
            assert_eq!(during, guard_path);
            assert_ne!(during, before);
        }
        let after = std::env::current_dir().unwrap().canonicalize().unwrap();
        assert_eq!(after, before);
    }

    #[test]
    fn path_accessor_returns_valid_dir() {
        let guard = TempDirGuard::new();
        assert!(guard.path().exists());
        assert!(guard.path().is_dir());
    }

    #[test]
    fn file_isolation_between_guards() {
        let before = std::env::current_dir().unwrap();
        {
            let _g1 = TempDirGuard::new();
            std::fs::write("marker.txt", "hello").unwrap();
        }
        {
            let _g2 = TempDirGuard::new();
            assert!(
                !Path::new("marker.txt").exists(),
                "files from g1 should not leak into g2"
            );
        }
        let _ = std::env::set_current_dir(&before);
    }
}
