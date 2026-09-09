//! The only place `AppState` changes.

use crate::tui::command::{Command, Effect};
use crate::tui::state::{AppState, Route};

/// Applies `cmd` to `state` and returns what the application wants from the
/// world. Pure: no I/O, and nothing but `state` is touched
/// ([ADR-0005](../../../docs/adr/0005-command-and-keymap-architecture.md)).
///
/// The vector is always empty in T7 — [`Effect`] has no variants, so one
/// cannot be built (SPEC-006 "Non-goals").
pub fn update(state: &mut AppState, cmd: Command) -> Vec<Effect> {
    match cmd {
        Command::MoveDown => {
            // Clamped at the last collection, never wrapping.
            let last = state.schema.collections.len().saturating_sub(1);
            if state.sidebar.selected < last {
                state.sidebar.selected += 1;
            }
        }
        // `saturating_sub` is the same clamp at the other end: 0 stays 0.
        Command::MoveUp => state.sidebar.selected = state.sidebar.selected.saturating_sub(1),
        Command::Select => {
            // The slug is cloned first so the borrow of the schema ends before
            // the route — another field of the same state — is written.
            let slug = state
                .selected_collection()
                .map(|collection| collection.slug.clone());
            if let Some(collection) = slug {
                state.route = Route::List { collection };
            }
        }
        Command::Back => state.route = Route::Home,
        Command::Quit => state.should_quit = true,
    }

    Vec::new()
}
