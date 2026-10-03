//! The only place `AppState` changes.

use crate::tui::command::{Command, Effect};
use crate::tui::state::{AppState, ListState, Load, Route, StatusKind, StatusMessage};

/// How far `PageUp` / `PageDown` move.
///
/// A constant and not the panel's height: the height is known only while
/// drawing and `view` may not write to state, so `update` would have to read a
/// terminal to know it (SPEC-007, Q3). Ten rows is about half the body at the
/// snapshot size, which is what `Ctrl+D` means to a vim user; `view` clamps the
/// offset, so the selection stays visible whatever the real height is.
pub const PAGE: usize = 10;

/// Applies `cmd` to `state` and returns what the application wants from the
/// world. Pure: no I/O, and nothing but `state` is touched
/// ([ADR-0005](../../../docs/adr/0005-command-and-keymap-architecture.md)).
pub fn update(state: &mut AppState, cmd: Command) -> Vec<Effect> {
    match cmd {
        // The two movement commands belong to whichever list has the screen,
        // which is what the route says.
        Command::MoveDown => match state.route {
            Route::Home => {
                // Clamped at the last collection, never wrapping.
                let last = state.schema.collections.len().saturating_sub(1);
                if state.sidebar.selected < last {
                    state.sidebar.selected += 1;
                }
            }
            Route::List { .. } => move_by(state, 1),
        },
        Command::MoveUp => match state.route {
            // `saturating_sub` is the same clamp at the other end: 0 stays 0.
            Route::Home => state.sidebar.selected = state.sidebar.selected.saturating_sub(1),
            Route::List { .. } => move_by(state, -1),
        },
        Command::Select => {
            // The slug is cloned first so the borrow of the schema ends before
            // the route — another field of the same state — is written.
            let slug = state
                .selected_collection()
                .map(|collection| collection.slug.clone());
            if let Some(collection) = slug {
                state.route = Route::List {
                    collection: collection.clone(),
                };
                // Emptied rather than kept: the records on screen must never be
                // another collection's, and entering twice reloads.
                state.list = ListState::default();
                return vec![Effect::LoadRecords { collection }];
            }
        }
        Command::Back => state.route = Route::Home,
        Command::Quit => state.should_quit = true,

        Command::GoToTop => select(state, 0),
        Command::GoToBottom => select(state, state.list.records.len().saturating_sub(1)),
        Command::PageDown => move_by(state, PAGE as isize),
        Command::PageUp => move_by(state, -(PAGE as isize)),

        Command::Refresh => {
            // Only a list has something to reload.
            if let Route::List { collection } = &state.route {
                return vec![Effect::LoadRecords {
                    collection: collection.clone(),
                }];
            }
        }

        Command::RecordsLoaded(records) => {
            state.list.records = records;
            state.list.load = Load::Ok;
            // The selection may be past the end of what arrived — a reload of a
            // collection that shrank.
            select(state, state.list.selected);
            // The cause is gone, so the message about it goes too. An `Info`
            // message is left alone: it is not about this.
            if matches!(
                state.status,
                Some(StatusMessage {
                    kind: StatusKind::Error,
                    ..
                })
            ) {
                state.status = None;
            }
        }

        Command::OperationFailed(message) => {
            state.list.load = Load::Failed;
            // `Failed` with records still in it is a state the code must not
            // produce: an empty table would claim the collection is empty.
            state.list.records.clear();
            state.list.selected = 0;
            state.list.offset = 0;
            // Replaced, not appended: one error message, not a growing list.
            state.status = Some(StatusMessage {
                text: message,
                kind: StatusKind::Error,
            });
        }
    }

    Vec::new()
}

/// Moves the list's selection by `delta`, clamped at both ends.
///
/// `isize` so one function serves `MoveUp`, `MoveDown` and both pages;
/// `saturating_*` is the clamp, and an empty list has nowhere to go.
fn move_by(state: &mut AppState, delta: isize) {
    let target = if delta < 0 {
        // `unsigned_abs` turns the negative delta into the amount to subtract.
        state.list.selected.saturating_sub(delta.unsigned_abs())
    } else {
        state.list.selected.saturating_add(delta as usize)
    };
    select(state, target);
}

/// Puts the selection on `index`, or on the last record when that is past the
/// end. Leaves it at 0 when there are no records at all.
fn select(state: &mut AppState, index: usize) {
    let last = state.list.records.len().saturating_sub(1);
    state.list.selected = if state.list.records.is_empty() {
        0
    } else {
        index.min(last)
    };
}
