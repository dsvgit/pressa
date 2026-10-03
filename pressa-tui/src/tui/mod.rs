//! The TUI shell: the terminal, the state, the keymap and the event loop
//! ([SPEC-006](../../specs/006-tui-shell.md)).

pub mod command;
pub mod effects;
pub mod keymap;
pub mod state;
pub mod terminal;
pub mod update;
pub mod view;

use std::io;

use crossterm::event::{Event, KeyEventKind};
use ratatui::Terminal;
use ratatui::backend::Backend;

use pressa_app::RecordService;
use pressa_app::domain::{RecordRepository, Schema};

pub use command::{Command, Effect};
pub use effects::run_effects;
pub use keymap::{Context, Hint, KEYMAP, KeyBinding, hints, key_label, resolve};
pub use state::{AppState, ListState, Load, Route, SidebarState, StatusKind, StatusMessage};
pub use terminal::{TerminalGuard, TerminalOps, install_panic_hook};
pub use update::{PAGE, update};
pub use view::view;

/// Everything that can go wrong once the screen is the output.
#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("the terminal could not be prepared: {0}")]
    Enter(io::Error),
    #[error("the screen could not be drawn: {0}")]
    Draw(io::Error),
    #[error("the terminal stopped sending input: {0}")]
    Input(io::Error),
    #[error("the terminal could not be restored: {0}")]
    Leave(io::Error),
}

/// The event loop. Enters the terminal, draws, reads, updates, and gives the
/// terminal back on every exit path.
///
/// Generic over the repository only to name `records`; the loop itself is not
/// (see [`drive`]), so the screen never sees a storage type.
pub fn run<R: RecordRepository>(schema: Schema, records: RecordService<R>) -> Result<(), TuiError> {
    let mut state = AppState::new(schema);
    let mut guard = TerminalGuard::enter()?;

    let looped = drive(
        &mut state,
        guard.terminal(),
        &mut || crossterm::event::read(),
        // The closure is where the service is captured, so `drive` stays
        // generic over a backend alone and never over a repository.
        &mut |effects| run_effects(&records, effects),
    );

    // The terminal is given back before the loop's error is returned, so the
    // message `main` prints lands on a cooked screen. `Drop` would restore too,
    // but only this path can report a failure to do so.
    let restored = guard.leave();
    looped?;
    restored?;

    Ok(())
}

/// The loop itself, over a terminal, a source of events and a runner for the
/// effects, so it can be driven by a `TestBackend` and two scripted closures
/// instead of a real terminal and a real database.
pub fn drive<B: Backend>(
    state: &mut AppState,
    terminal: &mut Terminal<B>,
    events: &mut dyn FnMut() -> io::Result<Event>,
    effects: &mut dyn FnMut(Vec<Effect>) -> Vec<Command>,
) -> Result<(), TuiError> {
    tracing::info!(target: "pressa", "tui started");

    // One event per pass, and a redraw after it: no tick, no frame rate.
    while !state.should_quit {
        terminal
            .draw(|frame| view(state, frame))
            .map_err(TuiError::Draw)?;

        if let Event::Key(key) = events().map_err(TuiError::Input)? {
            // Windows sends a release for every press; only the press acts.
            if key.kind == KeyEventKind::Press {
                if let Some(command) = resolve(keymap::context_for(&state.route), key) {
                    // Bound first: `state` cannot be borrowed mutably twice
                    // in one call, so `update` has to finish before `drain`.
                    let pending = update(state, command);
                    drain(state, pending, effects);
                }
            }
        }
    }

    tracing::info!(target: "pressa", "tui exited");
    Ok(())
}

/// Runs `pending`, feeds every command it answers with back through `update`,
/// and runs whatever *those* return — until nothing is left, and only then does
/// the caller draw again.
///
/// Written as a queue although T8's one effect settles in a single pass: T9's
/// save answers with a command whose `update` returns another effect.
fn drain(
    state: &mut AppState,
    pending: Vec<Effect>,
    effects: &mut dyn FnMut(Vec<Effect>) -> Vec<Command>,
) {
    let mut queue = pending;
    while !queue.is_empty() {
        // `std::mem::take` hands the queue over and leaves an empty one behind,
        // so the next round collects into a fresh vector rather than appending.
        let mut next = Vec::new();
        for command in effects(std::mem::take(&mut queue)) {
            next.extend(update(state, command));
        }
        queue = next;
    }
}
