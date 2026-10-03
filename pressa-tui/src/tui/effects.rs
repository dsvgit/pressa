//! The only place a service is called.
//!
//! `update` is pure and returns [`Effect`]s; this turns each one into the
//! [`Command`] it answers with, and the loop feeds those back through `update`
//! (`docs/tui.md` §1). Keeping the I/O here is what lets every interaction be
//! tested with no database and no terminal.

use pressa_app::domain::{Json, RecordId, RecordRepository};
use pressa_app::{AppError, RecordService};

use crate::tui::command::{Command, Effect};

/// Runs each effect in order and collects what it answers with.
///
/// Generic over the repository rather than over a concrete one, so the same
/// function serves `SqliteRepository` in `pressa dev` and `MemoryRepository` in
/// the tests.
pub fn run_effects<R: RecordRepository>(
    records: &RecordService<R>,
    effects: Vec<Effect>,
) -> Vec<Command> {
    effects
        .into_iter()
        .map(|effect| match effect {
            Effect::LoadRecords { collection } => load(records, &collection),
            Effect::LoadRecord { collection, id } => match records.get(&collection, &id) {
                // Boxed so a `Command` stays small (see `Command::RecordLoaded`).
                Ok(record) => Command::RecordLoaded(Box::new(record)),
                Err(error) => Command::OperationFailed(error.to_string()),
            },
            Effect::SaveRecord {
                collection,
                id,
                data,
            } => save(records, &collection, id, data),
        })
        .collect()
}

/// A collection's records, or the message the failure carries.
fn load<R: RecordRepository>(records: &RecordService<R>, collection: &str) -> Command {
    // `&Default::default()` and not `&ListParams::default()`: T8 loads the
    // whole collection, and naming `ListParams` would mean naming `pressa-core`
    // — a dependency `pressa-tui` does not have (SPEC-007 "Domain model").
    match records.list(collection, &Default::default()) {
        Ok(records) => Command::RecordsLoaded(records),
        // The `AppError`'s own message, rendered in one place: the status line.
        Err(error) => Command::OperationFailed(error.to_string()),
    }
}

/// Creates (`id: None`) or updates (`Some`), and the one place an
/// `AppError::Validation` is taken apart: its field errors go back to the form
/// unchanged, and every other failure goes to the status line.
fn save<R: RecordRepository>(
    records: &RecordService<R>,
    collection: &str,
    id: Option<RecordId>,
    data: Json,
) -> Command {
    let saved = match id {
        None => records.create(collection, data),
        // `&id` because `update` borrows the id rather than taking it.
        Some(id) => records.update(collection, &id, data),
    };
    match saved {
        Ok(record) => Command::RecordSaved(Box::new(record)),
        Err(AppError::Validation(errors)) => Command::SaveFailed(errors),
        Err(error) => Command::OperationFailed(error.to_string()),
    }
}
