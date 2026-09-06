//! `tracing` goes to `.pressa/pressa.log` and nowhere else (AGENTS.md,
//! `docs/architecture.md` §3.1).

use std::fs;
use std::path::{Path, PathBuf};

fn temp_project(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp project dir");
    dir
}

#[test]
fn the_log_lives_in_the_projects_dot_pressa_directory() {
    let project = Path::new("/some/project");
    assert_eq!(
        pressa_tui::logging::log_path(project),
        project.join(".pressa").join("pressa.log")
    );
}

#[test]
fn init_creates_the_log_file_and_records_events_in_it() {
    let project = temp_project("logging-init");

    pressa_tui::logging::init(&project).expect("tracing initialises");
    tracing::info!(step = 1, "skeleton is awake");

    let log = fs::read_to_string(pressa_tui::logging::log_path(&project)).expect("log file exists");
    assert!(log.contains("skeleton is awake"), "log was: {log:?}");
    assert!(log.contains("step=1"), "log was: {log:?}");
}
