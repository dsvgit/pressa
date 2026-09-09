//! Owning the terminal, and giving it back.
//!
//! Raw mode and the alternate screen are entered in one place and left in one
//! place — on return, on `?`, and on panic. A TUI that leaves a shell in raw
//! mode on a crash costs the user a `reset` (`docs/tui.md` §7).

use std::io::{self, IsTerminal, Stdout, Write};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::tui::TuiError;

/// The seam the restore is tested through: a test double records the calls
/// instead of touching a real terminal.
pub trait TerminalOps {
    fn enter(&mut self) -> io::Result<()>;
    fn leave(&mut self) -> io::Result<()>;
}

/// Enters on construction and leaves exactly once, however the scope ends.
pub struct ScreenGuard<O: TerminalOps> {
    ops: O,
    left: bool,
}

impl<O: TerminalOps> ScreenGuard<O> {
    /// Takes the terminal. Nothing is recorded when entering fails, so there
    /// is nothing to give back.
    pub fn enter(mut ops: O) -> io::Result<ScreenGuard<O>> {
        ops.enter()?;
        Ok(ScreenGuard { ops, left: false })
    }

    /// Gives the terminal back now rather than at drop, so a failure can be
    /// reported instead of discarded. Calling it twice restores once.
    pub fn leave(&mut self) -> io::Result<()> {
        if self.left {
            return Ok(());
        }
        // Set before the call, so a failed restore is not retried at drop.
        self.left = true;
        self.ops.leave()
    }
}

impl<O: TerminalOps> Drop for ScreenGuard<O> {
    // Runs on a return, on a `?` and while a panic unwinds — the three ways a
    // scope ends. `Drop` cannot report, so the error is dropped here and only
    // here; `leave` above is the path that reports one.
    fn drop(&mut self) {
        let _ = self.leave();
    }
}

/// Raw mode plus the alternate screen, on the real terminal.
pub struct CrosstermOps;

/// Whether the real terminal is currently ours. The panic hook and `Drop` both
/// restore, and this is what makes the pair restore once.
static ENTERED: AtomicBool = AtomicBool::new(false);

impl TerminalOps for CrosstermOps {
    fn enter(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        crossterm::execute!(stdout, EnterAlternateScreen)?;
        ENTERED.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn leave(&mut self) -> io::Result<()> {
        // `swap` returns the previous value, so only the first caller works.
        if !ENTERED.swap(false, Ordering::SeqCst) {
            return Ok(());
        }
        let mut stdout = io::stdout();
        crossterm::execute!(stdout, LeaveAlternateScreen)?;
        disable_raw_mode()?;
        stdout.flush()
    }
}

/// The terminal for the length of a session: a ratatui `Terminal` and the
/// guard that gives the real screen back.
pub struct TerminalGuard {
    guard: ScreenGuard<CrosstermOps>,
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    /// Raw mode, the alternate screen and the panic hook, in that order.
    ///
    /// Refuses a stdout that is not a terminal: entering raw mode on a pipe
    /// would leave the terminal the process was launched from in raw mode,
    /// which is the one thing this type exists to prevent.
    pub fn enter() -> Result<TerminalGuard, TuiError> {
        if !io::stdout().is_terminal() {
            return Err(TuiError::Enter(io::Error::other(
                "stdout is not a terminal",
            )));
        }

        // Installed before the screen is taken, so a panic between here and the
        // first frame still restores.
        install_panic_hook();
        let guard = ScreenGuard::enter(CrosstermOps).map_err(TuiError::Enter)?;
        let terminal =
            Terminal::new(CrosstermBackend::new(io::stdout())).map_err(TuiError::Enter)?;

        Ok(TerminalGuard { guard, terminal })
    }

    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    pub fn leave(&mut self) -> Result<(), TuiError> {
        self.guard.leave().map_err(TuiError::Leave)
    }
}

/// Installs the process's panic hook, once.
pub fn install_panic_hook() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        chain_panic_hook(|| {
            let _ = CrosstermOps.leave();
        })
    });
}

/// Restores, then calls the hook that was already installed, so the panic
/// message lands on a cooked screen instead of being swallowed.
fn chain_panic_hook<F: Fn() + Send + Sync + 'static>(restore: F) {
    // Taken rather than replaced: this is the hook that prints the message,
    // and the new one has to call it.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{self, AssertUnwindSafe};
    use std::sync::{Arc, Mutex};

    /// The panic hook is process-global, so the tests that replace one take
    /// this first and cannot interleave.
    static HOOK_LOCK: Mutex<()> = Mutex::new(());

    type Calls = Arc<Mutex<Vec<&'static str>>>;

    /// A terminal that only writes down what was asked of it.
    struct Recorder {
        calls: Calls,
    }

    impl Recorder {
        fn new() -> (Recorder, Calls) {
            let calls: Calls = Arc::new(Mutex::new(Vec::new()));
            // Both halves share the same list; `clone` on an `Arc` is a second
            // handle to it, not a copy of it.
            (
                Recorder {
                    calls: Arc::clone(&calls),
                },
                calls,
            )
        }
    }

    impl TerminalOps for Recorder {
        fn enter(&mut self) -> io::Result<()> {
            record(&self.calls, "enter");
            Ok(())
        }

        fn leave(&mut self) -> io::Result<()> {
            record(&self.calls, "leave");
            Ok(())
        }
    }

    fn record(calls: &Calls, what: &'static str) {
        // `into_inner` on a poisoned lock: a panicking test is the point here.
        let mut calls = calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        calls.push(what);
    }

    fn seen(calls: &Calls) -> Vec<&'static str> {
        calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    #[test]
    fn entering_and_dropping_take_the_terminal_and_give_it_back() {
        let (recorder, calls) = Recorder::new();
        {
            let _guard = ScreenGuard::enter(recorder).expect("the double never fails");
            assert_eq!(seen(&calls), ["enter"], "raw mode is on inside the scope");
        }
        assert_eq!(seen(&calls), ["enter", "leave"], "and off after it");
    }

    #[test]
    fn leaving_by_hand_and_then_dropping_restores_once() {
        let (recorder, calls) = Recorder::new();
        let mut guard = ScreenGuard::enter(recorder).expect("the double never fails");

        guard.leave().expect("the double never fails");
        drop(guard);

        assert_eq!(seen(&calls), ["enter", "leave"]);
    }

    #[test]
    fn a_panic_inside_the_scope_restores_exactly_once() {
        let _serial = HOOK_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (recorder, calls) = Recorder::new();

        // A silent hook, so a deliberate panic does not print a backtrace over
        // the test output. Put back below.
        let previous = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));
        // `AssertUnwindSafe`: the recorder is behind a `Mutex`, so a half-done
        // mutation cannot be observed.
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            let _guard = ScreenGuard::enter(recorder).expect("the double never fails");
            panic!("the loop fell over");
        }));
        panic::set_hook(previous);

        assert!(result.is_err(), "the panic was not swallowed");
        assert_eq!(seen(&calls), ["enter", "leave"]);
    }

    #[test]
    fn the_panic_hook_restores_before_it_calls_the_hook_it_replaced() {
        let _serial = HOOK_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));

        let original = panic::take_hook();
        let theirs = Arc::clone(&calls);
        // The hook standing in for whatever was installed before ours — the
        // one that prints the panic message.
        panic::set_hook(Box::new(move |_| record(&theirs, "previous")));

        let ours = Arc::clone(&calls);
        chain_panic_hook(move || record(&ours, "restore"));

        let _ = panic::catch_unwind(|| panic!("the loop fell over"));
        panic::set_hook(original);

        assert_eq!(
            seen(&calls),
            ["restore", "previous"],
            "the terminal is cooked before the message is printed"
        );
    }
}
