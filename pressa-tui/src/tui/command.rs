//! What a user can do, and what the application can ask the world for.

/// Every user action, as one enum ([ADR-0005](../../../docs/adr/0005-command-and-keymap-architecture.md)).
/// The T7 shell can move the sidebar, open a collection, come back and quit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    MoveUp,
    MoveDown,
    Select,
    Back,
    Quit,
}

/// Uninhabited in T7: the shell asks the world for nothing but the terminal.
///
/// An enum with no variants cannot be constructed, so `Vec<Effect>` is
/// provably empty and the loop has nothing to run. Each variant arrives with
/// the effect runner that serves it (T8 onward).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {}
