//! The `pressa` binary's library half: CLI, terminal lifecycle, `AppState`,
//! `Command`, keymap, screens and widgets.
//!
//! It lives as a library so that integration tests can drive `update()`
//! directly — see [SPEC-000](../../specs/000-m0-golden-path.md).

pub mod cli;
pub mod logging;
pub mod tui;

/// `1 collection`, `3 collections` — each noun pluralises on its own count.
///
/// Shared because `pressa validate` and the shell's empty state print the same
/// phrase, and SPEC-006 asks for the same rule in both.
pub fn counted(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{n} {noun}")
    } else {
        format!("{n} {noun}s")
    }
}
