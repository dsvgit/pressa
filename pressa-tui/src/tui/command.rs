//! What a user can do, and what the application can ask the world for.

use pressa_app::domain::{FieldError, Json, Record, RecordId};

/// Every user action, as one enum ([ADR-0005](../../../docs/adr/0005-command-and-keymap-architecture.md)).
///
/// The results at the end are not keystrokes: they are what an effect answers
/// with, fed back through `update` like any other command (`docs/tui.md` §1).
/// That is why this enum is not `Copy` — `RecordsLoaded` owns its records.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    MoveUp,
    MoveDown,
    Select,
    Back,
    Quit,
    GoToTop,
    GoToBottom,
    PageUp,
    PageDown,
    Refresh,
    /// Reached from a list: a blank draft, or the selected record's.
    NewRecord,
    EditRecord,
    NextField,
    PrevField,
    /// `Enter` in the editor; `update` dispatches it on the focused field's
    /// type (SPEC-008 Q1).
    BeginEdit,
    CommitField,
    CancelEdit,
    InsertNewline,
    ToggleBoolean,
    NextOption,
    PrevOption,
    InputChar(char),
    InputBackspace,
    Save,
    /// The overlay's two answers.
    Confirm,
    Dismiss,
    RecordsLoaded(Vec<Record>),
    /// Boxed: `Command` is matched by value, and a bare `Record` would make
    /// every variant as large as a document.
    RecordLoaded(Box<Record>),
    RecordSaved(Box<Record>),
    SaveFailed(Vec<FieldError>),
    OperationFailed(String),
}

/// What the application asks the world for. Served by `run_effects`, which is
/// the only function in the crate that calls a service.
///
/// `LoadRecords` carries no `params`: T8 loads with `ListParams::default()`,
/// and T11 adds the field when search gives it a value (SPEC-007 "Domain
/// model"). Each further variant arrives with the runner that serves it —
/// `DeleteRecord` with T10's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    LoadRecords {
        collection: String,
    },
    LoadRecord {
        collection: String,
        id: RecordId,
    },
    /// `id: None` is `RecordService::create`, `Some` is `update`: the only
    /// thing that tells the two apart by the time it reaches `run_effects`.
    SaveRecord {
        collection: String,
        id: Option<RecordId>,
        data: Json,
    },
}
