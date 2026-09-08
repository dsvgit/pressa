//! A throwaway directory tree for tests that need real files on disk.
//!
//! Hand-rolled rather than `tempfile`: a new dependency needs an ADR
//! (`AGENTS.md`), and this is twenty lines.

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
