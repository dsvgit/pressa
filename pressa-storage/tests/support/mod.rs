//! A throwaway directory tree for the tests that need a real database file.
//!
//! Hand-rolled rather than `tempfile`: a new dependency needs an ADR
//! (`AGENTS.md`), and `pressa-app/tests/support` already made this call. No
//! test touches a fixed path, which is what `specs/003` actually asks for.

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
        let n = COUNTER.fetch_add(1, Ordering::Relaxed); // atomic: tests run in parallel threads
        let root = std::env::temp_dir().join(format!(
            "pressa-{label}-{}-{n}",
            std::process::id() // separates concurrent `cargo test` processes
        ));
        let _ = fs::remove_dir_all(&root); // ignore "not there yet", the normal case
        fs::create_dir_all(&root).expect("create temp tree");
        TempTree { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The database path the tests use: nested, so opening has to create
    /// `.pressa/` the way a first run does.
    pub fn database(&self) -> PathBuf {
        self.root.join(".pressa").join("data.db")
    }
}

impl Drop for TempTree {
    // Runs even when a test panics, so a failed run leaves no litter behind.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
