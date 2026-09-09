//! A throwaway directory tree for tests that need real files on disk.
//!
//! The same helper as `pressa-app/tests/support/mod.rs`: hand-rolled rather
//! than `tempfile`, because a new dependency needs an ADR (`AGENTS.md`) and
//! this is twenty lines. ADR-0010 admits `assert_cmd` on a different ground —
//! what the test prints when it fails — and explicitly leaves that refusal
//! standing.

// Each test binary uses a different subset of these helpers.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// Bumped per tree so two trees in the same process never collide.
static COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct TempTree {
    root: PathBuf,
}

impl TempTree {
    /// Creates `<system temp>/pressa-<label>-<pid>-<n>/` and returns a guard.
    pub fn new(label: &str) -> TempTree {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed); // atomic: tests may run in parallel threads
        let root = std::env::temp_dir().join(format!(
            "pressa-{label}-{}-{n}",
            std::process::id() // separates concurrent `cargo test` processes
        ));
        let _ = fs::remove_dir_all(&root); // ignore "not there yet", which is the normal case
        fs::create_dir_all(&root).expect("create temp tree");
        // Deliberately not canonicalized: discovery only makes paths absolute,
        // so tests compare against the same unresolved form it returns.
        TempTree { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Creates `relative` as a directory, parents included, and returns it.
    pub fn dir(&self, relative: &str) -> PathBuf {
        let path = self.root.join(relative);
        fs::create_dir_all(&path).expect("create dir");
        path
    }

    /// Writes `contents` to `relative`, creating parent directories.
    pub fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(&path, contents).expect("write file");
        path
    }
}

impl Drop for TempTree {
    // Runs even when a test panics, so a failed run leaves no litter behind.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The workspace root, so tests can reach `examples/blog/pressa.yaml`.
pub fn workspace_root() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is `pressa-tui/`; its parent is the workspace.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("pressa-tui sits inside the workspace root")
        .to_path_buf()
}

/// `examples/blog/pressa.yaml` — the good-schema fixture, used rather than
/// copied so the example and the tests cannot drift.
pub fn example_schema_path() -> PathBuf {
    workspace_root()
        .join("examples")
        .join("blog")
        .join("pressa.yaml")
}
