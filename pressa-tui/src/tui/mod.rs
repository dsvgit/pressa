//! The TUI shell: the terminal, the state, the keymap and the event loop
//! ([SPEC-006](../../specs/006-tui-shell.md)).

pub mod command;
pub mod keymap;
pub mod state;
pub mod terminal;
pub mod update;
pub mod view;

use std::io;

use crossterm::event::{Event, KeyEventKind};
use ratatui::Terminal;
use ratatui::backend::Backend;

use pressa_app::domain::Schema;

pub use command::{Command, Effect};
pub use keymap::{Context, Hint, KEYMAP, KeyBinding, hints, key_label, resolve};
pub use state::{AppState, Route, SidebarState, StatusKind, StatusMessage};
pub use terminal::{TerminalGuard, TerminalOps, install_panic_hook};
pub use update::update;
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
pub fn run(schema: Schema) -> Result<(), TuiError> {
    let mut state = AppState::new(schema);
    let mut guard = TerminalGuard::enter()?;

    let looped = drive(&mut state, guard.terminal(), &mut || {
        crossterm::event::read()
    });

    // The terminal is given back before the loop's error is returned, so the
    // message `main` prints lands on a cooked screen. `Drop` would restore too,
    // but only this path can report a failure to do so.
    let restored = guard.leave();
    looped?;
    restored?;

    Ok(())
}

/// The loop itself, over a terminal and a source of events, so it can be
/// driven by a `TestBackend` and a scripted list instead of a real terminal.
pub fn drive<B: Backend>(
    state: &mut AppState,
    terminal: &mut Terminal<B>,
    events: &mut dyn FnMut() -> io::Result<Event>,
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
                    // Empty by construction in T7: there is no `Effect` to run.
                    let _effects = update(state, command);
                }
            }
        }
    }

    tracing::info!(target: "pressa", "tui exited");
    Ok(())
}
