//! The only place `AppState` changes.

use pressa_app::domain::{Collection, Field, FieldType, Json, blank_document};

use crate::tui::command::{Command, Effect};
use crate::tui::state::{
    AppState, EditorState, ListState, Load, Overlay, Route, StatusKind, StatusMessage,
};
use crate::tui::view::form::{editable, parse};

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
            Route::New { .. } | Route::Edit { .. } => {}
        },
        Command::MoveUp => match state.route {
            // `saturating_sub` is the same clamp at the other end: 0 stays 0.
            Route::Home => state.sidebar.selected = state.sidebar.selected.saturating_sub(1),
            Route::List { .. } => move_by(state, -1),
            Route::New { .. } | Route::Edit { .. } => {}
        },
        // Only the sidebar opens a collection; elsewhere `Select` is inert, so
        // it can never move the route off an editor.
        Command::Select if state.route != Route::Home => {}
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
        Command::Back => back(state),
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

        // In the editor a failure is a save's, or an `Edit`'s load. A load
        // that found nothing leaves the list provably stale (Q14).
        Command::OperationFailed(message) if state.route.editing().is_some() => {
            state.status = Some(StatusMessage {
                text: message,
                kind: StatusKind::Error,
            });
            if !state.editor.is_loaded() {
                if let Some(collection) = state.route.editing().map(str::to_string) {
                    leave_editor(
                        state,
                        Route::List {
                            collection: collection.clone(),
                        },
                    );
                    return vec![Effect::LoadRecords { collection }];
                }
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
        Command::NewRecord => {
            if let Route::List { collection } = &state.route {
                // `blank_document` is pure, so no effect is needed (Q2).
                if let Some(found) = state.schema.collections.get(collection) {
                    state.editor = EditorState::over(blank_document(found), None);
                    state.route = Route::New {
                        collection: collection.clone(),
                    };
                }
            }
        }
        Command::EditRecord => {
            // `get` rather than indexing: an empty or failed list has no row.
            let target = match &state.route {
                Route::List { collection } => state
                    .list
                    .records
                    .get(state.list.selected)
                    .map(|record| (collection.clone(), record.id)),
                _ => None,
            };
            if let Some((collection, id)) = target {
                state.editor = EditorState {
                    id: Some(id),
                    ..EditorState::default()
                };
                state.route = Route::Edit {
                    collection: collection.clone(),
                    id,
                };
                return vec![Effect::LoadRecord { collection, id }];
            }
        }
        Command::RecordLoaded(record) => {
            if matches!(state.route, Route::Edit { id, .. } if id == record.id) {
                state.editor = EditorState::over(record.data, Some(record.id));
            }
        }

        Command::NextField | Command::PrevField if state.editor.input.is_none() => {
            let last = field_count(state).saturating_sub(1);
            state.editor.focus = if cmd == Command::NextField {
                (state.editor.focus + 1).min(last)
            } else {
                state.editor.focus.saturating_sub(1)
            };
        }
        Command::BeginEdit if state.editor.input.is_none() => {
            // `cloned` so the schema borrow ends before the editor is written.
            if let Some(field) = focused(state).cloned() {
                match field.kind {
                    FieldType::Boolean => toggle(state, &field),
                    FieldType::Select { .. } => cycle(state, &field, 1),
                    FieldType::Text
                    | FieldType::Textarea
                    | FieldType::Number
                    | FieldType::DateTime
                    | FieldType::Json => {
                        let value = state.editor.draft.get(field.name.as_str());
                        state.editor.input = Some(editable(&field.kind, value));
                    }
                }
            }
        }
        Command::ToggleBoolean if state.editor.input.is_none() => {
            if let Some(field) = focused(state).cloned() {
                toggle(state, &field);
            }
        }
        Command::NextOption | Command::PrevOption if state.editor.input.is_none() => {
            if let Some(field) = focused(state).cloned() {
                let step = if cmd == Command::NextOption { 1 } else { -1 };
                cycle(state, &field, step);
            }
        }
        Command::InputChar(c) => {
            // `as_mut` edits the buffer in place; `None` means not typing.
            if let Some(input) = state.editor.input.as_mut() {
                input.push(c);
            }
        }
        Command::InsertNewline => {
            if let Some(input) = state.editor.input.as_mut() {
                input.push('\n');
            }
        }
        Command::InputBackspace => {
            if let Some(input) = state.editor.input.as_mut() {
                input.pop();
            }
        }
        Command::CancelEdit => cancel(state),
        Command::CommitField => commit(state),

        Command::Save => {
            let target = match &state.route {
                Route::New { collection } => Some((collection.clone(), None)),
                Route::Edit { collection, id } => Some((collection.clone(), Some(*id))),
                _ => None,
            };
            if let Some((collection, id)) = target.filter(|_| state.editor.is_loaded()) {
                return vec![Effect::SaveRecord {
                    collection,
                    id,
                    data: state.editor.draft.clone(),
                }];
            }
        }
        Command::SaveFailed(errors) => {
            // Replaced, not appended, and one entry per field: the first in the
            // service's order wins.
            state.editor.errors.clear();
            for error in errors {
                if !state
                    .editor
                    .errors
                    .iter()
                    .any(|kept| kept.field == error.field)
                {
                    state.editor.errors.push(error);
                }
            }
        }
        Command::RecordSaved(_) => {
            if let Some(collection) = state.route.editing().map(str::to_string) {
                leave_editor(
                    state,
                    Route::List {
                        collection: collection.clone(),
                    },
                );
                state.status = Some(StatusMessage {
                    text: SAVED.to_string(),
                    kind: StatusKind::Info,
                });
                return vec![Effect::LoadRecords { collection }];
            }
        }

        Command::Confirm => {
            // `take` empties the overlay and hands over where it was going.
            if let Some(Overlay::ConfirmDiscard { next }) = state.overlay.take() {
                leave_editor(state, *next);
            }
        }
        Command::Dismiss => state.overlay = None,

        // Typing: navigation and per-type commands wait for the field to close.
        Command::NextField
        | Command::PrevField
        | Command::BeginEdit
        | Command::ToggleBoolean
        | Command::NextOption
        | Command::PrevOption => {}
    }

    Vec::new()
}

/// The status line after a successful save (SPEC-008 "Saving").
const SAVED: &str = "Record saved.";

/// `Back`: up one level, asking first when an editor holds unsaved work.
fn back(state: &mut AppState) {
    match &state.route {
        Route::Home | Route::List { .. } => state.route = Route::Home,
        // A field being typed into closes with `CancelEdit`, never `Back`.
        Route::New { .. } | Route::Edit { .. } if state.editor.input.is_some() => {}
        Route::New { collection } | Route::Edit { collection, .. } => {
            let next = Route::List {
                collection: collection.clone(),
            };
            if state.editor.is_dirty() {
                state.overlay = Some(Overlay::ConfirmDiscard {
                    next: Box::new(next),
                });
            } else {
                leave_editor(state, next);
            }
        }
    }
}

/// Routes to `next` and forgets the editor and any overlay over it.
fn leave_editor(state: &mut AppState, next: Route) {
    state.route = next;
    state.editor = EditorState::default();
    state.overlay = None;
}

/// The collection the editor route names, from the schema.
fn editing(state: &AppState) -> Option<&Collection> {
    state
        .route
        .editing()
        .and_then(|slug| state.schema.collections.get(slug))
}

fn field_count(state: &AppState) -> usize {
    editing(state).map_or(0, |collection| collection.fields.len())
}

/// The focused field, through `get` so a stale index is `None`.
fn focused(state: &AppState) -> Option<&Field> {
    editing(state).and_then(|collection| collection.fields.get(state.editor.focus))
}

/// Writes `value` under `field`'s key. Inserted when a stored record lacked
/// it: the name comes from the schema, so no unknown key can appear.
///
/// The field's error goes with the value it was about: a `Select` cycled or a
/// `Boolean` toggled after a refused save is no longer the value refused, so
/// its `⚠` would be stale (frame D follows step 6's refusal with none).
fn write(state: &mut AppState, field: &Field, value: Json) {
    if let Some(object) = state.editor.draft.as_object_mut() {
        object.insert(field.name.clone(), value);
        drop_error(state, field);
    }
}

/// Removes `field`'s entry from `errors`, if it has one.
fn drop_error(state: &mut AppState, field: &Field) {
    state
        .editor
        .errors
        .retain(|error| error.field != field.name);
}

/// `CancelEdit`: the typed text is thrown away, and so is the parse error it
/// caused — the draft never held that text, so nothing is wrong with it. Any
/// other error, such as a save's `required`, is left under the field.
fn cancel(state: &mut AppState) {
    let (Some(field), Some(text)) = (focused(state).cloned(), state.editor.input.take()) else {
        state.editor.input = None;
        return;
    };
    // The error is the buffer's own when parsing the buffer produces it.
    // `Err(refused)` binds the error `parse` returns, to compare it.
    if let Err(refused) = parse(&field.kind, &text) {
        let caused = state.editor.errors.iter().any(|error| {
            error.field == field.name
                && error.code == refused.code
                && error.message == refused.message
        });
        if caused {
            drop_error(state, &field);
        }
    }
}

/// Flips a `Boolean`; any other type is left alone.
fn toggle(state: &mut AppState, field: &Field) {
    if field.kind != FieldType::Boolean {
        return;
    }
    // Anything but `true` — `false`, null, a stray value — toggles to `true`.
    let on = state.editor.draft.get(field.name.as_str()) == Some(&Json::Bool(true));
    write(state, field, Json::Bool(!on));
}

/// Moves a `Select` by `step` through its options, wrapping; from no value,
/// forwards is the first option and backwards the last.
fn cycle(state: &mut AppState, field: &Field, step: isize) {
    let FieldType::Select { options } = &field.kind else {
        return;
    };
    if options.is_empty() {
        return;
    }
    let len = options.len() as isize;
    let current = state
        .editor
        .draft
        .get(field.name.as_str())
        .and_then(Json::as_str)
        .and_then(|value| options.iter().position(|option| option == value));
    let next = match current {
        // `rem_euclid` wraps both ways, -1 included.
        Some(index) => (index as isize + step).rem_euclid(len),
        None if step > 0 => 0,
        None => len - 1,
    };
    if let Some(option) = options.get(next as usize) {
        write(state, field, Json::String(option.clone()));
    }
}

/// Parses the input for the focused field: written and closed on success,
/// kept open with its error on failure (Q6).
fn commit(state: &mut AppState) {
    let (Some(field), Some(text)) = (focused(state).cloned(), state.editor.input.clone()) else {
        return;
    };
    // Whatever happens, this field's old error is superseded.
    drop_error(state, &field);
    match parse(&field.kind, &text) {
        // Empty text reads as no value, which would turn a stored `""` into
        // `null` — and the form dirty — on an `Enter` that changed nothing.
        // Text that is still what `BeginEdit` seeded leaves the draft alone.
        Ok(Json::Null)
            if editable(&field.kind, state.editor.draft.get(field.name.as_str())) == text =>
        {
            state.editor.input = None;
        }
        Ok(value) => {
            write(state, &field, value);
            state.editor.input = None;
        }
        Err(mut error) => {
            error.field = field.name.clone();
            state.editor.errors.push(error);
        }
    }
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
