//! Reading `pressa.yaml`: finding the project, and turning the file into a
//! validated [`Schema`](pressa_core::schema::Schema).
//!
//! Specified by [SPEC-001](../../../specs/001-schema-config.md). A malformed
//! schema that loaded successfully would produce failures far from their
//! cause, so anything questionable is rejected here, with the YAML path that
//! caused it.

mod discovery;
mod loader;

use std::io;
use std::path::PathBuf;

use thiserror::Error;

pub use discovery::{CONFIG_FILE, ProjectPaths, discover_project, project_at};
pub use loader::load_schema;

#[derive(Debug, Error)]
pub enum ConfigError {
    /// The walk up from `searched_from` reached the filesystem root.
    #[error("no {CONFIG_FILE} found in {} or any parent directory", searched_from.display())]
    NotFound { searched_from: PathBuf },

    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        // `#[source]` keeps the cause chain without repeating it in the message.
        #[source]
        source: io::Error,
    },

    /// `serde_yaml`'s message already ends with "at line L column C".
    #[error("{}: {source}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    /// The file parsed, but says something the schema does not allow.
    /// `path` is the YAML path, e.g. `collections.posts.fields[3].type`.
    #[error("{path}: {problem}")]
    Invalid { path: String, problem: String },
}

impl ConfigError {
    /// Shorthand for the many `Invalid` sites in the loader.
    fn invalid(path: impl Into<String>, problem: impl Into<String>) -> ConfigError {
        ConfigError::Invalid {
            path: path.into(),
            problem: problem.into(),
        }
    }
}
