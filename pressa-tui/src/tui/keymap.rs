//! The one place a key is named.
//!
//! Every binding the application has is a row of [`KEYMAP`], and both the hint
//! bar and (from T11) the help overlay are generated from it. A key that is not
//! in the table does nothing; a key that is in it is documented by construction
//! ([ADR-0005](../../../docs/adr/0005-command-and-keymap-architecture.md)).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::command::Command;
use crate::tui::state::Route;

/// Where a binding applies. Derived from the state, never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Global,
    Sidebar,
    List,
}

/// What the hint bar prints for a binding, if anything.
///
/// One field rather than a `bool` plus a label, so a hidden binding carrying a
/// label is unrepresentable (`docs/tui.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hint {
    Hidden,
    /// `"<key label> <description>"`.
    Shown,
    /// That label instead of the key's own — `j` and `↓` read as one `↑↓`.
    ShownAs(&'static str),
}

pub struct KeyBinding {
    pub context: Context,
    pub key: KeyEvent,
    pub command: Command,
    pub description: &'static str,
    pub hint: Hint,
}

/// A key with no modifiers. `const` so the table below is a static.
const fn plain(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// `Ctrl` held with a character.
const fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

/// Every binding in the application, in hint-bar order.
///
/// `q` appears twice on purpose: it quits from the sidebar, where there is
/// nowhere left to go back to, and goes back from a list. Two rows rather than
/// a branch inside `update` (`docs/tui.md` §3).
pub static KEYMAP: &[KeyBinding] = &[
    KeyBinding {
        context: Context::Sidebar,
        key: plain(KeyCode::Char('j')),
        command: Command::MoveDown,
        description: "Navigate",
        hint: Hint::ShownAs("↑↓"),
    },
    KeyBinding {
        context: Context::Sidebar,
        key: plain(KeyCode::Down),
        command: Command::MoveDown,
        description: "Navigate",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::Sidebar,
        key: plain(KeyCode::Char('k')),
        command: Command::MoveUp,
        description: "Navigate",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::Sidebar,
        key: plain(KeyCode::Up),
        command: Command::MoveUp,
        description: "Navigate",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::Sidebar,
        key: plain(KeyCode::Enter),
        command: Command::Select,
        description: "Open",
        hint: Hint::Shown,
    },
    KeyBinding {
        context: Context::Sidebar,
        key: plain(KeyCode::Char('l')),
        command: Command::Select,
        description: "Open",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::Sidebar,
        key: plain(KeyCode::Right),
        command: Command::Select,
        description: "Open",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::Sidebar,
        key: plain(KeyCode::Char('q')),
        command: Command::Quit,
        description: "Quit",
        hint: Hint::Shown,
    },
    KeyBinding {
        context: Context::List,
        key: plain(KeyCode::Esc),
        command: Command::Back,
        description: "Back",
        hint: Hint::Shown,
    },
    KeyBinding {
        context: Context::List,
        key: plain(KeyCode::Char('h')),
        command: Command::Back,
        description: "Back",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::List,
        key: plain(KeyCode::Left),
        command: Command::Back,
        description: "Back",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::List,
        key: plain(KeyCode::Char('q')),
        command: Command::Back,
        description: "Back",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::Global,
        key: ctrl('c'),
        command: Command::Quit,
        description: "Quit",
        hint: Hint::Hidden,
    },
];

/// The context a route is in. Derived, never stored, so lookup is unambiguous.
pub fn context_for(route: &Route) -> Context {
    match route {
        Route::Home => Context::Sidebar,
        Route::List { .. } => Context::List,
    }
}

/// The command `key` triggers in `context`, if any.
///
/// The context's own bindings are searched first and `Global` second, so a
/// context can shadow a global key. A key in no binding produces no command,
/// and nothing happens.
pub fn resolve(context: Context, key: KeyEvent) -> Option<Command> {
    // `or_else` runs the second search only when the first found nothing.
    in_context(context, key).or_else(|| in_context(Context::Global, key))
}

/// The one binding of `context` that `key` matches.
fn in_context(context: Context, key: KeyEvent) -> Option<Command> {
    KEYMAP
        .iter()
        .find(|binding| binding.context == context && matches(binding.key, key))
        // `Command` is `Copy`, so the found binding is not borrowed any further.
        .map(|binding| binding.command)
}

/// A key matches a binding when the code and the modifiers are equal.
fn matches(bound: KeyEvent, pressed: KeyEvent) -> bool {
    bound.code == pressed.code && modifiers(bound) == modifiers(pressed)
}

/// The modifiers that count. `SHIFT` on a character is dropped: the character
/// already carries the case, so `Q` and `Shift+q` are the same press.
fn modifiers(key: KeyEvent) -> KeyModifiers {
    match key.code {
        KeyCode::Char(_) => key.modifiers.difference(KeyModifiers::SHIFT),
        _ => key.modifiers,
    }
}

/// One `"<key label> <description>"` per visible binding of `context`, in table
/// order, then the visible global ones.
pub fn hints(context: Context) -> Vec<String> {
    // `Global` is skipped in the second pass when it is already the first, so
    // its own hints are not listed twice.
    let globals = KEYMAP
        .iter()
        .filter(move |binding| context != Context::Global && binding.context == Context::Global);

    KEYMAP
        .iter()
        .filter(|binding| binding.context == context)
        .chain(globals)
        // `filter_map` drops the hidden bindings and keeps the rendered rest.
        .filter_map(hint_of)
        .collect()
}

/// What the hint bar prints for one binding, or `None` when it prints nothing.
fn hint_of(binding: &KeyBinding) -> Option<String> {
    let description = binding.description;
    match binding.hint {
        Hint::Hidden => None,
        Hint::Shown => Some(format!("{} {description}", key_label(binding.key))),
        Hint::ShownAs(label) => Some(format!("{label} {description}")),
    }
}

/// How a key is written in the hint bar.
pub fn key_label(key: KeyEvent) -> String {
    let cap = match key.code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Up => "↑".to_string(),
        KeyCode::Down => "↓".to_string(),
        KeyCode::Left => "←".to_string(),
        KeyCode::Right => "→".to_string(),
        // Nothing else is bound; `Debug` keeps the function total rather than
        // making an unbound key a panic.
        other => format!("{other:?}"),
    };

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        format!("Ctrl+{}", cap.to_uppercase())
    } else {
        cap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plain character press, the shape crossterm delivers.
    fn press(c: char) -> KeyEvent {
        plain(KeyCode::Char(c))
    }

    #[test]
    fn the_sidebar_resolves_every_key_it_offers() {
        for key in [press('j'), plain(KeyCode::Down)] {
            assert_eq!(resolve(Context::Sidebar, key), Some(Command::MoveDown));
        }
        for key in [press('k'), plain(KeyCode::Up)] {
            assert_eq!(resolve(Context::Sidebar, key), Some(Command::MoveUp));
        }
        for key in [plain(KeyCode::Enter), press('l'), plain(KeyCode::Right)] {
            assert_eq!(resolve(Context::Sidebar, key), Some(Command::Select));
        }
        assert_eq!(resolve(Context::Sidebar, press('q')), Some(Command::Quit));
    }

    #[test]
    fn a_list_goes_back_on_the_key_that_quits_at_home() {
        for key in [
            plain(KeyCode::Esc),
            press('h'),
            plain(KeyCode::Left),
            press('q'),
        ] {
            assert_eq!(resolve(Context::List, key), Some(Command::Back));
        }
    }

    #[test]
    fn ctrl_c_quits_from_anywhere() {
        for context in [Context::Sidebar, Context::List] {
            assert_eq!(resolve(context, ctrl('c')), Some(Command::Quit));
        }
    }

    #[test]
    fn a_key_in_no_binding_produces_no_command() {
        // `?` belongs to the help overlay, which is T11's; until then it is a
        // key that does nothing, and the hint bar must not claim otherwise.
        for key in [press('z'), press('?')] {
            assert_eq!(resolve(Context::Sidebar, key), None);
            assert_eq!(resolve(Context::List, key), None);
        }
    }

    #[test]
    fn modifiers_are_part_of_the_match() {
        assert_eq!(resolve(Context::Sidebar, press('c')), None);
        assert_eq!(resolve(Context::Sidebar, ctrl('c')), Some(Command::Quit));
    }

    #[test]
    fn shift_on_a_character_is_ignored_because_the_character_carries_the_case() {
        let shifted = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::SHIFT);
        assert_eq!(resolve(Context::Sidebar, shifted), Some(Command::Quit));
    }

    #[test]
    fn a_context_shadows_a_global_binding() {
        // `q` is Global-free, so the shadowing is shown the other way round:
        // the same key resolves differently in the two contexts.
        assert_eq!(resolve(Context::Sidebar, press('q')), Some(Command::Quit));
        assert_eq!(resolve(Context::List, press('q')), Some(Command::Back));
    }

    #[test]
    fn every_hint_names_a_key_that_does_something() {
        for binding in KEYMAP {
            if binding.hint == Hint::Hidden {
                continue;
            }
            assert_eq!(
                resolve(binding.context, binding.key),
                Some(binding.command),
                "a visible hint names a key that resolves to something else"
            );
        }
    }

    #[test]
    fn the_sidebar_shows_three_hints_and_a_list_shows_one() {
        let sidebar = hints(Context::Sidebar);
        assert_eq!(sidebar.len(), 3, "hints were: {sidebar:?}");
        assert!(sidebar[0].starts_with("↑↓ "));
        assert!(sidebar[1].starts_with("Enter "));
        assert!(sidebar[2].starts_with("q "));

        let list = hints(Context::List);
        assert_eq!(list.len(), 1, "hints were: {list:?}");
        assert!(list[0].starts_with("Esc "));
    }

    #[test]
    fn key_labels_are_what_the_key_cap_says() {
        assert_eq!(key_label(press('q')), "q");
        assert_eq!(key_label(plain(KeyCode::Enter)), "Enter");
        assert_eq!(key_label(plain(KeyCode::Esc)), "Esc");
        assert_eq!(key_label(plain(KeyCode::Tab)), "Tab");
        assert_eq!(key_label(plain(KeyCode::Backspace)), "Backspace");
        assert_eq!(key_label(plain(KeyCode::Up)), "↑");
        assert_eq!(key_label(plain(KeyCode::Down)), "↓");
        assert_eq!(key_label(plain(KeyCode::Left)), "←");
        assert_eq!(key_label(plain(KeyCode::Right)), "→");
        assert_eq!(key_label(ctrl('c')), "Ctrl+C");
    }

    #[test]
    fn the_context_comes_from_the_route() {
        assert_eq!(context_for(&Route::Home), Context::Sidebar);
        assert_eq!(
            context_for(&Route::List {
                collection: "posts".to_string()
            }),
            Context::List
        );
    }
}
