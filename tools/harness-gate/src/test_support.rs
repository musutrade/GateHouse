//! Test-only workspace helpers shared by in-process unit tests.

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Own an isolated temporary project workspace and clean it up on drop.
pub(crate) struct TestWorkspace {
    _temp_dir: TempDir,
    pub(crate) root: PathBuf,
}

impl TestWorkspace {
    /// Create a uniquely named workspace with the supplied diagnostic prefix.
    pub(crate) fn new(prefix: &str) -> Self {
        let temp_dir = tempfile::Builder::new()
            .prefix(&format!("harness-gate-{prefix}-"))
            .tempdir()
            .expect("create test workspace");
        let root = temp_dir.path().to_path_buf();
        Self {
            _temp_dir: temp_dir,
            root,
        }
    }

    /// Create a child directory and return its path.
    pub(crate) fn child(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        std::fs::create_dir_all(&path).expect("create test workspace child");
        path
    }

    /// Initialize a quiet Git repository for tests that exercise Git behavior.
    pub(crate) fn init_git(&self) {
        let output = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&self.root)
            .output()
            .expect("initialize test Git repository");
        assert!(output.status.success(), "git init failed: {output:?}");

        for (key, value) in [
            ("user.name", "Test User"),
            ("user.email", "test@example.com"),
        ] {
            let output = Command::new("git")
                .args(["config", key, value])
                .current_dir(&self.root)
                .output()
                .expect("configure test Git repository");
            assert!(output.status.success(), "git config failed: {output:?}");
        }
    }
}

impl Deref for TestWorkspace {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.root
    }
}
