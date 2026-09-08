//! Finding the project root, the way git finds `.git`.

use std::path::{Path, PathBuf};

use super::ConfigError;

/// The schema file that marks a directory as a pressa project.
pub const CONFIG_FILE: &str = "pressa.yaml";
/// Everything pressa generates lives here, next to the schema file.
const DATA_DIR: &str = ".pressa";
const DATABASE_FILE: &str = "data.db";
const LOG_FILE: &str = "pressa.log";

/// Every path derived from the project root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPaths {
    pub root: PathBuf,
    /// `<root>/pressa.yaml`
    pub config: PathBuf,
    /// `<root>/.pressa`
    pub data_dir: PathBuf,
    pub database: PathBuf,
    pub log: PathBuf,
}

impl ProjectPaths {
    /// Derives the whole set from a project root.
    fn rooted_at(root: PathBuf) -> ProjectPaths {
        let data_dir = root.join(DATA_DIR);
        ProjectPaths {
            config: root.join(CONFIG_FILE),
            database: data_dir.join(DATABASE_FILE),
            log: data_dir.join(LOG_FILE),
            data_dir, // moved last: the joins above borrowed it
            root,
        }
    }
}

/// Walks up from `start` looking for `pressa.yaml`, stopping at the
/// filesystem root.
pub fn discover_project(start: &Path) -> Result<ProjectPaths, ConfigError> {
    // `.` has no parent, so a relative start would end the walk immediately.
    // `absolute` does that lexically — no filesystem access, no symlinks
    // resolved, so the paths returned still look like the ones passed in.
    let start = std::path::absolute(start).map_err(|source| ConfigError::Io {
        path: start.to_path_buf(),
        source,
    })?;

    // `Some(&Path)` walks up one directory per step; `parent()` is None at the
    // root, which is what ends the loop.
    let mut candidate = Some(start.as_path());
    while let Some(dir) = candidate {
        if dir.join(CONFIG_FILE).is_file() {
            return Ok(ProjectPaths::rooted_at(dir.to_path_buf()));
        }
        candidate = dir.parent();
    }

    Err(ConfigError::NotFound {
        searched_from: start,
    })
}

/// `--project <dir>`: the schema must be in *this* directory. No walking up.
pub fn project_at(dir: &Path) -> Result<ProjectPaths, ConfigError> {
    let root = std::path::absolute(dir).map_err(|source| ConfigError::Io {
        path: dir.to_path_buf(),
        source,
    })?;

    if root.join(CONFIG_FILE).is_file() {
        Ok(ProjectPaths::rooted_at(root))
    } else {
        Err(ConfigError::NotFound {
            searched_from: root,
        })
    }
}
