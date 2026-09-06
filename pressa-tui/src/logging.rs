//! Tracing setup.
//!
//! The log file is the only place pressa writes diagnostics: the TUI owns
//! stdout and stderr for as long as it runs (`docs/architecture.md` §3.1).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use thiserror::Error;

/// The project-local directory holding everything pressa generates.
const DATA_DIR: &str = ".pressa";
const LOG_FILE: &str = "pressa.log";

#[derive(Debug, Error)]
pub enum LogError {
    #[error("could not create {path}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not open the log file {path}")]
    OpenLog {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("tracing has already been initialised")]
    AlreadyInitialised,
}

/// Where the log for `project_dir` lives: `<project_dir>/.pressa/pressa.log`.
pub fn log_path(project_dir: &Path) -> PathBuf {
    project_dir.join(DATA_DIR).join(LOG_FILE)
}

/// Send `tracing` output to the project's log file, creating `.pressa/` if it
/// is not there yet. Appends, so a run never discards the previous one.
///
/// Fails if called twice: the subscriber is process-global.
pub fn init(project_dir: &Path) -> Result<(), LogError> {
    let dir = project_dir.join(DATA_DIR);
    fs::create_dir_all(&dir).map_err(|source| LogError::CreateDir {
        path: dir.clone(),
        source,
    })?;

    let path = log_path(project_dir);
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|source| LogError::OpenLog {
            path: path.clone(),
            source,
        })?;

    let subscriber = tracing_subscriber::fmt()
        .with_writer(Mutex::new(file))
        .finish();

    tracing::subscriber::set_global_default(subscriber).map_err(|_| LogError::AlreadyInitialised)
}
