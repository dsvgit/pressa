//! The `pressa` binary and the workspace's composition root.
//!
//! The clap CLI arrives in T6 ([SPEC-005](../../specs/005-cli.md)) and the
//! event loop in T7. What the skeleton owes is the one thing everything else
//! depends on: logging, started before anything can fail.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    // Upward discovery of `pressa.yaml` lands in T2; until then the project is
    // the working directory.
    let project_dir = Path::new(".");

    if let Err(error) = pressa_tui::logging::init(project_dir) {
        eprintln!("pressa: {error}");
        return ExitCode::FAILURE;
    }

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "pressa started");
    ExitCode::SUCCESS
}
