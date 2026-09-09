//! The command line: parsing, project resolution, and the three subcommands.
//!
//! Specified by [SPEC-005](../../specs/005-cli.md). Everything here runs
//! *before* the TUI owns the screen, which is why this is the one part of
//! `pressa-tui` allowed to write to stdout and stderr.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pressa_app::config::{
    CONFIG_FILE, ConfigError, ProjectPaths, discover_project, load_schema, project_at,
};
use pressa_app::{AppError, CollectionService, RecordService};
use pressa_storage::SqliteRepository;

/// What `pressa init` writes. `include_str!` rather than a copy: the file the
/// scaffold produces and the Golden Path fixture are then one file and cannot
/// drift apart (SPEC-005 "API").
const TEMPLATE: &str = include_str!("../../examples/blog/pressa.yaml");

/// The parsed command line. `clap`'s derive API builds this from argv.
#[derive(Debug, clap::Parser)]
#[command(name = "pressa", version, about)]
pub struct Cli {
    /// Use this project directory instead of searching upward
    // For `dev` and `validate` it replaces discovery; for `init` it is the
    // directory to scaffold into.
    #[arg(long, global = true, value_name = "DIR")]
    pub project: Option<PathBuf>,

    // Verbosity of `.pressa/pressa.log`. No doc comment: clap already prints
    // the default and the possible values, and screen A shows no help text.
    #[arg(long, global = true, value_name = "LVL", default_value = "info")]
    pub log_level: LogLevel,

    /// Absent means `Dev` — `pressa` and `pressa dev` are the same command.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// `PartialEq` so a test can assert that `pressa` parses to the same command
/// as `pressa dev`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::Subcommand)]
pub enum Command {
    /// Open the TUI for this project [default]
    Dev,
    /// Scaffold pressa.yaml and .pressa/ in this directory
    Init,
    /// Load and check the schema, then report
    Validate,
}

/// Mirrors `tracing::Level` so that `clap` can parse it and the CLI does not
/// make a `tracing` type part of its public surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Everything that can go wrong before the TUI owns the screen. Rendered as
/// one line after `pressa: `; `thiserror`, because this is library code.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Log(#[from] crate::logging::LogError),
    /// Carried as `AppError` rather than as
    /// `pressa_core::repository::StorageError`: naming that type would add a
    /// `pressa-core` dependency edge to `pressa-tui`, which ADR-0006 makes an
    /// ADR-sized decision and SPEC-005 forbids ("clap as its only new runtime
    /// dependency"). `AppError` is transparent over it, so the message the user
    /// reads is the storage error's own.
    #[error(transparent)]
    Storage(#[from] AppError),
    #[error("pressa.yaml already exists in {}", .0.display())]
    AlreadyInitialised(PathBuf),
    #[error("could not create {}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The T6 stub for a screen T7 owns. Removed when the event loop lands.
    #[error("the TUI arrives in T7")]
    NotImplementedYet,
}

/// Runs the parsed command against `cwd`. Returns the process exit code, so
/// `main` is a shim and every path through the CLI is testable in-process.
pub fn run(cli: Cli, cwd: &Path) -> ExitCode {
    let level = cli.log_level;
    // Borrowed as a `&Path` because both `init` and `resolve_project` only read it.
    let project = cli.project.as_deref();

    // `Some(text)` is a line for stdout; `None` means the command prints nothing.
    let result: Result<Option<String>, CliError> = match cli.command.unwrap_or(Command::Dev) {
        Command::Init => {
            // `--project` names the directory to scaffold into; relative to
            // `cwd` rather than to the process's own working directory.
            let dir = project.map_or_else(|| cwd.to_path_buf(), |dir| cwd.join(dir));
            init(&dir, level).map(|()| Some(scaffold_report(project)))
        }
        // `and_then` so the project is resolved and logging started before the
        // subcommand runs, and neither runs if that failed.
        Command::Validate => {
            start(project, cwd, level).and_then(|paths| validate(&paths).map(Some))
        }
        Command::Dev => start(project, cwd, level).and_then(|paths| dev(&paths).map(|()| None)),
    };

    match result {
        Ok(Some(text)) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::SUCCESS,
        Err(error) => {
            // One line, plain text, on stderr: the TUI does not own the screen
            // yet, and a piped run stays readable (SPEC-005 "Invariants").
            eprintln!("pressa: {error}");
            ExitCode::FAILURE
        }
    }
}

/// `pressa init`: scaffold into `dir`, creating `dir` itself if needed.
/// Fails if `dir/pressa.yaml` exists, before writing anything.
pub fn init(dir: &Path, level: LogLevel) -> Result<(), CliError> {
    let config = dir.join(CONFIG_FILE);
    // Decided before anything is created, so a refusal leaves the filesystem
    // exactly as it was — no directory, no log (screen D).
    if config.exists() {
        return Err(CliError::AlreadyInitialised(absolute_or_given(dir)));
    }

    fs::create_dir_all(dir).map_err(|source| CliError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    fs::write(&config, TEMPLATE).map_err(|source| CliError::Io {
        path: config.clone(),
        source,
    })?;

    // Only now is this directory known to be a project, which is when a
    // `.pressa/` may appear in it.
    start_logging(dir, level)?;
    tracing::info!(target: "pressa", root = %absolute_or_given(dir).display(), "project scaffolded");
    Ok(())
}

/// `pressa validate`: loads the schema at `paths.config` and returns the
/// summary line for screen E. Printing is the caller's job.
pub fn validate(paths: &ProjectPaths) -> Result<String, CliError> {
    let schema = load_schema(&paths.config)?;
    // `values()` borrows each collection; `sum` totals the fields across all of
    // them, which is what the second noun counts.
    let fields: usize = schema
        .collections
        .values()
        .map(|collection| collection.fields.len())
        .sum();

    let summary = format!(
        "schema ok: {}, {}",
        counted(schema.collections.len(), "collection"),
        counted(fields, "field")
    );
    tracing::info!(target: "pressa", collections = schema.collections.len(), fields, "schema validated");
    Ok(summary)
}

/// `pressa dev`: the whole startup sequence, up to the point where T7 takes
/// over. Returns `CliError::NotImplementedYet` on success until then.
///
/// This is the composition root: the one place `pressa-storage` is named
/// outside tests, where a `SqliteRepository` is built and handed to the
/// services (ADR-0006). Nothing above it sees a storage type.
pub fn dev(paths: &ProjectPaths) -> Result<(), CliError> {
    // The schema is loaded first, so a bad one leaves no `data.db` behind.
    let schema = load_schema(&paths.config)?;

    // `map_err` because `StorageError` is not nameable here; `AppError` is
    // transparent over it, so the message is unchanged (see `CliError`).
    let repository = SqliteRepository::open(&paths.database).map_err(AppError::from)?;

    // The schema is cloned because both services own one: they never re-read
    // `pressa.yaml` (SPEC-004).
    let collections = CollectionService::new(schema.clone());
    // Held until T7 gives it an event loop to serve; built here so that a
    // failure to construct it surfaces at startup rather than on first keypress.
    let _records = RecordService::new(repository, schema);

    tracing::info!(target: "pressa", collections = collections.all().count(), "services ready");
    tracing::warn!(target: "pressa", "the TUI arrives in T7");
    Err(CliError::NotImplementedYet)
}

/// Resolves the project: `project_at` when `--project` was given,
/// `discover_project` from `cwd` otherwise. Not used by `init`.
pub fn resolve_project(project: Option<&Path>, cwd: &Path) -> Result<ProjectPaths, CliError> {
    let paths = match project {
        // `join` leaves an absolute `--project` alone and resolves a relative
        // one against `cwd`, never against the process's working directory.
        Some(dir) => project_at(&cwd.join(dir))?,
        None => discover_project(cwd)?,
    };
    Ok(paths)
}

/// Resolve the project, then start its log — in that order, so a directory
/// that is not a project never gets a `.pressa/`.
fn start(project: Option<&Path>, cwd: &Path, level: LogLevel) -> Result<ProjectPaths, CliError> {
    let paths = resolve_project(project, cwd)?;
    start_logging(&paths.root, level)?;
    tracing::info!(target: "pressa", root = %paths.root.display(), "project resolved");
    Ok(paths)
}

/// Point `tracing` at `<dir>/.pressa/pressa.log` and record that the process
/// started. The first line every successful run leaves behind.
fn start_logging(dir: &Path, level: LogLevel) -> Result<(), CliError> {
    crate::logging::init(dir, level)?;
    tracing::info!(target: "pressa", version = %env!("CARGO_PKG_VERSION"), "pressa started");
    Ok(())
}

/// `1 collection`, `3 collections` — each noun pluralises on its own count.
fn counted(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{n} {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// The absolute form of `dir` for a message, falling back to what was given:
/// a path that cannot be made absolute is still worth naming.
fn absolute_or_given(dir: &Path) -> PathBuf {
    std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf())
}

/// What `init` prints on success (screens B and C). The directory is named the
/// way the user typed it, minus a leading `./`.
fn scaffold_report(project: Option<&Path>) -> String {
    match project {
        None => "created pressa.yaml\ncreated .pressa/\nrun `pressa` to open it".to_string(),
        Some(dir) => {
            let dir = dir.display().to_string();
            let dir = dir.strip_prefix("./").unwrap_or(&dir);
            format!(
                "created {dir}/pressa.yaml\ncreated {dir}/.pressa/\n\
                 run `pressa --project {dir}` to open it"
            )
        }
    }
}
