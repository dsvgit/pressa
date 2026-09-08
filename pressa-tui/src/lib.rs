//! The `pressa` binary's library half: CLI, terminal lifecycle, `AppState`,
//! `Command`, keymap, screens and widgets.
//!
//! It lives as a library so that integration tests can drive `update()`
//! directly — see [SPEC-000](../../specs/000-m0-golden-path.md).

pub mod logging;
