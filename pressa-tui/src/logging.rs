//! Tracing setup.
//!
//! The log file is the only place pressa writes diagnostics: the TUI owns
//! stdout and stderr for as long as it runs (`docs/architecture.md` §3.1).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use thiserror::Error;

use crate::cli::LogLevel;

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
/// `level` is the subscriber's maximum: an event above it never reaches the
/// file. Fails if called twice: the subscriber is process-global.
pub fn init(project_dir: &Path, level: LogLevel) -> Result<(), LogError> {
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
        // `Mutex` because several threads may log into the one file handle.
        .with_writer(Mutex::new(file))
        .with_max_level(max_level(level))
        .finish();

    tracing::subscriber::set_global_default(subscriber).map_err(|_| LogError::AlreadyInitialised)
}

/// `--log-level` as the subscriber understands it. Kept here so `tracing` stays
/// out of the CLI's public surface (SPEC-005 "Domain model").
fn max_level(level: LogLevel) -> tracing::Level {
    match level {
        LogLevel::Trace => tracing::Level::TRACE,
        LogLevel::Debug => tracing::Level::DEBUG,
        LogLevel::Info => tracing::Level::INFO,
        LogLevel::Warn => tracing::Level::WARN,
        LogLevel::Error => tracing::Level::ERROR,
    }
}
