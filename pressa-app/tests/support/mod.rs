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

// ---------------------------------------------------------------------------
// Service-test fixtures (SPEC-004)
// ---------------------------------------------------------------------------

use pressa_core::record::{Record, RecordId};
use pressa_core::repository::{ListParams, RecordRepository, StorageError};
use pressa_core::schema::Schema;
use serde_json::Value as Json;
use std::sync::{Arc, Mutex};

/// The collection the service tests use: one required unique field, a required
/// `Select`, and one field of every other M0 type.
pub const POSTS: &str = "\
project:
  name: services-test

collections:
  posts:
    label: Posts
    list_columns: [title, slug]
    fields:
      - name: title
        type: text
        required: true
      - name: slug
        type: text
        required: true
        unique: true
      - name: status
        type: select
        required: true
        options: [draft, published]
      - name: views
        type: number
      - name: featured
        type: boolean
      - name: published_at
        type: datetime
      - name: metadata
        type: json
  authors:
    label: Authors
    fields:
      - name: name
        type: text
      # Optional *and* unique: two authors with no email must not collide.
      - name: email
        type: text
        unique: true
";

/// Loads `yaml` through the real loader, so fixtures cannot drift from the
/// config format the rest of the app is built on.
pub fn schema_from(yaml: &str) -> Schema {
    let tree = TempTree::new("services");
    let path = tree.write("pressa.yaml", yaml);
    // `tree` is dropped at the end of this function, after the file is read.
    pressa_app::config::load_schema(&path).expect("fixture schema loads")
}

/// The calls a [`RecordingRepository`] has seen.
///
/// Held separately and cloned so a test can read the log after handing the
/// repository to a service by value — which is why `RecordService` needs no
/// accessor for its repository.
#[derive(Debug, Default, Clone)]
pub struct CallLog(
    // `Arc` so the test and the repository share one log; `Mutex` because the
    // port takes `&self` and adapters own their own locking.
    Arc<Mutex<Vec<String>>>,
);

impl CallLog {
    /// The method names seen so far, in order.
    pub fn calls(&self) -> Vec<String> {
        // Recovered rather than propagated: a poisoned lock here means a test
        // already failed, and its assertion is the more useful message.
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn push(&self, name: &str) {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(name.to_string());
    }
}

/// A repository that records the calls made to it and stores nothing.
///
/// It exists for the "never reaches the repository" criteria of SPEC-004: a
/// service that resolves the collection first must leave this untouched.
#[derive(Debug)]
pub struct RecordingRepository {
    log: CallLog,
}

impl RecordingRepository {
    pub fn new(log: CallLog) -> Self {
        RecordingRepository { log }
    }

    fn record(&self, name: &str) {
        self.log.push(name);
    }

    /// A record to hand back from `create`/`update`, which must return one.
    fn stub(collection: &str, data: Json) -> Record {
        let at = chrono::Utc::now();
        Record {
            id: RecordId::new(),
            collection: collection.to_string(),
            data,
            created_at: at,
            updated_at: at,
        }
    }
}

impl RecordRepository for RecordingRepository {
    fn list(&self, _collection: &str, _params: &ListParams) -> Result<Vec<Record>, StorageError> {
        self.record("list");
        Ok(Vec::new())
    }

    fn count(&self, _collection: &str, _params: &ListParams) -> Result<u64, StorageError> {
        self.record("count");
        Ok(0)
    }

    fn get(&self, _collection: &str, _id: &RecordId) -> Result<Option<Record>, StorageError> {
        self.record("get");
        Ok(None)
    }

    fn create(&self, collection: &str, data: Json) -> Result<Record, StorageError> {
        self.record("create");
        Ok(RecordingRepository::stub(collection, data))
    }

    fn update(&self, collection: &str, _id: &RecordId, data: Json) -> Result<Record, StorageError> {
        self.record("update");
        Ok(RecordingRepository::stub(collection, data))
    }

    fn delete(&self, _collection: &str, _id: &RecordId) -> Result<(), StorageError> {
        self.record("delete");
        Ok(())
    }

    fn find_by_field(
        &self,
        _collection: &str,
        _field: &str,
        _value: &Json,
    ) -> Result<Vec<Record>, StorageError> {
        self.record("find_by_field");
        Ok(Vec::new())
    }
}
