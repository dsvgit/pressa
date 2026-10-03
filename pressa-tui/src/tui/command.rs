//! What a user can do, and what the application can ask the world for.

use pressa_app::domain::Record;

/// Every user action, as one enum ([ADR-0005](../../../docs/adr/0005-command-and-keymap-architecture.md)).
///
/// The last two are not keystrokes: they are what an effect answers with, fed
/// back through `update` like any other command (`docs/tui.md` §1). That is why
/// this enum is no longer `Copy` — `RecordsLoaded` owns its records.
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
    RecordsLoaded(Vec<Record>),
    OperationFailed(String),
}

/// What the application asks the world for. Served by `run_effects`, which is
/// the only function in the crate that calls a service.
///
/// `LoadRecords` carries no `params`: T8 loads with `ListParams::default()`,
/// and T11 adds the field when search gives it a value (SPEC-007 "Domain
/// model"). Each further variant arrives with the runner that serves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    LoadRecords { collection: String },
}
