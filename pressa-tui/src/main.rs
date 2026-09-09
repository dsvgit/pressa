//! The `pressa` binary and the workspace's composition root.
//!
//! A shim: parse argv, find the working directory, hand both to
//! [`pressa_tui::cli::run`] ([SPEC-005](../../specs/005-cli.md)). Everything
//! testable lives in the library half, so `main` has nothing to test.

use std::process::ExitCode;

use clap::Parser;

use pressa_tui::cli::{self, Cli};

fn main() -> ExitCode {
    // `try_parse` rather than `parse`: clap's own `parse` calls
    // `process::exit`, which skips destructors. The exit code is clap's — 2 for
    // a usage error, 0 for `--help` and `--version`.
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            // clap writes help to stdout and usage errors to stderr itself.
            let _ = error.print();
            // `exit_code` is an i32; clap only ever returns 0 or 2 here.
            return ExitCode::from(error.exit_code() as u8);
        }
    };

    // Read once and passed down, so nothing below reaches for process-global
    // state and every function stays testable against a temporary directory.
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            eprintln!("pressa: could not read the working directory: {error}");
            return ExitCode::FAILURE;
        }
    };

    cli::run(cli, &cwd)
}
