//! The one place a key is named.
//!
//! Every binding the application has is a row of [`KEYMAP`], and both the hint
//! bar and (from T11) the help overlay are generated from it. A key that is not
//! in the table does nothing; a key that is in it is documented by construction
//! ([ADR-0005](../../../docs/adr/0005-command-and-keymap-architecture.md)).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use pressa_app::domain::FieldType;

use crate::tui::command::Command;
use crate::tui::state::{AppState, Route};

/// Where a binding applies. Derived from the state, never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Global,
    Sidebar,
    List,
    /// `New` or `Edit`, moving between fields.
    Editor,
    /// Typing into one of the five single-line field types.
    EditorInput,
    /// Typing into a `Textarea` or a `Json` field, where `Enter` is a newline.
    EditorText,
    /// The discard dialog. T10 adds `ConfirmDelete` beside it (SPEC-008 Q10).
    ConfirmDiscard,
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

/// Stands for "any printable character" in the two input contexts. A table
/// cannot hold a row per character, so this one row is matched by
/// [`resolve`] against whatever was typed, and its `InputChar` placeholder is
/// replaced by that character. `Null` because no terminal sends it as a key.
const PRINTABLE: KeyEvent = plain(KeyCode::Null);

/// The `char` in the `PRINTABLE` rows' command, replaced on every match.
const ANY: char = ' ';

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
        key: plain(KeyCode::Char('j')),
        command: Command::MoveDown,
        description: "Navigate",
        hint: Hint::ShownAs("↑↓"),
    },
    KeyBinding {
        context: Context::List,
        key: plain(KeyCode::Down),
        command: Command::MoveDown,
        description: "Navigate",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::List,
        key: plain(KeyCode::Char('k')),
        command: Command::MoveUp,
        description: "Navigate",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::List,
        key: plain(KeyCode::Up),
        command: Command::MoveUp,
        description: "Navigate",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::List,
        key: plain(KeyCode::Char('g')),
        command: Command::GoToTop,
        description: "Top",
        hint: Hint::Hidden,
    },
    // `G` and not `Shift+g`: `modifiers` drops SHIFT on a character, because
    // the character already carries the case.
    KeyBinding {
        context: Context::List,
        key: plain(KeyCode::Char('G')),
        command: Command::GoToBottom,
        description: "Bottom",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::List,
        key: ctrl('d'),
        command: Command::PageDown,
        description: "Page down",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::List,
        key: ctrl('u'),
        command: Command::PageUp,
        description: "Page up",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::List,
        key: plain(KeyCode::Char('r')),
        command: Command::Refresh,
        description: "Reload",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::List,
        key: plain(KeyCode::Enter),
        command: Command::EditRecord,
        description: "Edit",
        hint: Hint::Shown,
    },
    KeyBinding {
        context: Context::List,
        key: plain(KeyCode::Char('n')),
        command: Command::NewRecord,
        description: "New",
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
    // `Enter` is one command in the editor; `update` decides by the focused
    // field's type what it does (SPEC-008 Q1). `Space`, `l` and `h` reach the
    // per-type commands directly and stay out of the bar.
    KeyBinding {
        context: Context::Editor,
        key: plain(KeyCode::Tab),
        command: Command::NextField,
        description: "Next",
        hint: Hint::Shown,
    },
    KeyBinding {
        context: Context::Editor,
        key: plain(KeyCode::Char('j')),
        command: Command::NextField,
        description: "Next",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::Editor,
        key: plain(KeyCode::BackTab),
        command: Command::PrevField,
        description: "Previous",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::Editor,
        key: plain(KeyCode::Char('k')),
        command: Command::PrevField,
        description: "Previous",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::Editor,
        key: plain(KeyCode::Enter),
        command: Command::BeginEdit,
        description: "Edit",
        hint: Hint::Shown,
    },
    KeyBinding {
        context: Context::Editor,
        key: plain(KeyCode::Char(' ')),
        command: Command::ToggleBoolean,
        description: "Toggle",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::Editor,
        key: plain(KeyCode::Char('l')),
        command: Command::NextOption,
        description: "Next option",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::Editor,
        key: plain(KeyCode::Right),
        command: Command::NextOption,
        description: "Next option",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::Editor,
        key: plain(KeyCode::Char('h')),
        command: Command::PrevOption,
        description: "Previous option",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::Editor,
        key: plain(KeyCode::Left),
        command: Command::PrevOption,
        description: "Previous option",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::Editor,
        key: ctrl('s'),
        command: Command::Save,
        description: "Save",
        hint: Hint::Shown,
    },
    KeyBinding {
        context: Context::Editor,
        key: plain(KeyCode::Esc),
        command: Command::Back,
        description: "Back",
        hint: Hint::Shown,
    },
    // The two input contexts differ only in `Enter` and `Tab`, which swap
    // roles: a single-line field commits on `Enter`, a multi-line one inserts
    // a newline with it and commits on `Tab` (SPEC-008 Q3b).
    KeyBinding {
        context: Context::EditorInput,
        key: PRINTABLE,
        command: Command::InputChar(ANY),
        description: "Type",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::EditorInput,
        key: plain(KeyCode::Backspace),
        command: Command::InputBackspace,
        description: "Delete",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::EditorInput,
        key: plain(KeyCode::Enter),
        command: Command::CommitField,
        description: "Done",
        hint: Hint::Shown,
    },
    KeyBinding {
        context: Context::EditorInput,
        key: plain(KeyCode::Esc),
        command: Command::CancelEdit,
        description: "Cancel",
        hint: Hint::Shown,
    },
    KeyBinding {
        context: Context::EditorText,
        key: PRINTABLE,
        command: Command::InputChar(ANY),
        description: "Type",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::EditorText,
        key: plain(KeyCode::Backspace),
        command: Command::InputBackspace,
        description: "Delete",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::EditorText,
        key: plain(KeyCode::Tab),
        command: Command::CommitField,
        description: "Done",
        hint: Hint::Shown,
    },
    KeyBinding {
        context: Context::EditorText,
        key: plain(KeyCode::Enter),
        command: Command::InsertNewline,
        description: "Newline",
        hint: Hint::Shown,
    },
    KeyBinding {
        context: Context::EditorText,
        key: plain(KeyCode::Esc),
        command: Command::CancelEdit,
        description: "Cancel",
        hint: Hint::Shown,
    },
    // One context per dialog, so `y Discard` and T10's `y Delete` each come
    // from their own row (SPEC-008 Q10).
    KeyBinding {
        context: Context::ConfirmDiscard,
        key: plain(KeyCode::Char('y')),
        command: Command::Confirm,
        description: "Discard",
        hint: Hint::Shown,
    },
    KeyBinding {
        context: Context::ConfirmDiscard,
        key: plain(KeyCode::Enter),
        command: Command::Confirm,
        description: "Discard",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::ConfirmDiscard,
        key: plain(KeyCode::Char('n')),
        command: Command::Dismiss,
        description: "Cancel",
        hint: Hint::Shown,
    },
    KeyBinding {
        context: Context::ConfirmDiscard,
        key: plain(KeyCode::Esc),
        command: Command::Dismiss,
        description: "Cancel",
        hint: Hint::Hidden,
    },
    KeyBinding {
        context: Context::ConfirmDiscard,
        key: plain(KeyCode::Char('q')),
        command: Command::Dismiss,
        description: "Cancel",
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

/// The context the state is in. Derived, never stored, so lookup is unambiguous.
///
/// An overlay first, then a field being typed into, then the route: an
/// overlay and an input buffer are state, not route (SPEC-008 "Domain model").
pub fn context_for(state: &AppState) -> Context {
    if state.overlay.is_some() {
        return Context::ConfirmDiscard;
    }
    if state.editor.input.is_some() {
        return if focused_is_multiline(state) {
            Context::EditorText
        } else {
            Context::EditorInput
        };
    }
    match state.route {
        Route::Home => Context::Sidebar,
        Route::List { .. } => Context::List,
        Route::New { .. } | Route::Edit { .. } => Context::Editor,
    }
}

/// Whether the focused field is a `Textarea` or a `Json` one — the two where
/// `Enter` is a newline. A field the schema does not have is single-line.
fn focused_is_multiline(state: &AppState) -> bool {
    // `and_then` chains the lookups; any `None` falls through to `false`.
    state
        .route
        .editing()
        .and_then(|slug| state.schema.collections.get(slug))
        .and_then(|collection| collection.fields.get(state.editor.focus))
        .is_some_and(|field| matches!(field.kind, FieldType::Textarea | FieldType::Json))
}

/// The command `key` triggers in `context`, if any.
///
/// The context's own bindings are searched first and `Global` second, so a
/// context can shadow a global key. A key in no binding produces no command,
/// and nothing happens.
pub fn resolve(context: Context, key: KeyEvent) -> Option<Command> {
    // `or_else` runs each later search only when the one before found nothing:
    // an exact key first, so `Enter` and `Esc` beat the printable row.
    in_context(context, key)
        .or_else(|| printable_in(context, key))
        .or_else(|| in_context(Context::Global, key))
}

/// The one binding of `context` that `key` matches.
fn in_context(context: Context, key: KeyEvent) -> Option<Command> {
    KEYMAP
        .iter()
        .find(|binding| binding.context == context && matches(binding.key, key))
        // Cloned because `Command` is not `Copy` (`RecordsLoaded` owns a
        // `Vec`). Every key in the table is a small variant, so this is cheap.
        .map(|binding| binding.command.clone())
}

/// `InputChar` with the typed character, when `context` has a `PRINTABLE` row
/// and `key` is a character with no modifier but `Shift`.
fn printable_in(context: Context, key: KeyEvent) -> Option<Command> {
    // `let … else`: anything but a plain or shifted character is not text.
    let KeyCode::Char(c) = key.code else {
        return None;
    };
    if !modifiers(key).is_empty() || c.is_control() {
        return None;
    }
    // `any` asks only whether the row exists; its placeholder is not reused.
    let typed = KEYMAP
        .iter()
        .any(|binding| binding.context == context && binding.key == PRINTABLE);
    typed.then_some(Command::InputChar(c))
}

/// A key matches a binding when the code and the modifiers are equal.
fn matches(bound: KeyEvent, pressed: KeyEvent) -> bool {
    bound.code == pressed.code && modifiers(bound) == modifiers(pressed)
}

/// The modifiers that count. `SHIFT` on a character is dropped: the character
/// already carries the case, so `Q` and `Shift+q` are the same press.
///
/// `BackTab` is the same: it *is* `Shift+Tab`, and terminals disagree on
/// whether they also report the `SHIFT` that produced it.
fn modifiers(key: KeyEvent) -> KeyModifiers {
    match key.code {
        KeyCode::Char(_) | KeyCode::BackTab => key.modifiers.difference(KeyModifiers::SHIFT),
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

/// How the key bound to `command` in `context` is written, if one is.
///
/// For text that names a key outside the hint bar — the header's
/// `● unsaved · Ctrl+S` — so it too comes from the table.
pub fn key_for(context: Context, command: &Command) -> Option<String> {
    KEYMAP
        .iter()
        .find(|binding| binding.context == context && &binding.command == command)
        .map(|binding| key_label(binding.key))
}

/// How a key is written in the hint bar.
pub fn key_label(key: KeyEvent) -> String {
    let cap = match key.code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "Shift+Tab".to_string(),
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
    fn the_list_resolves_every_key_it_offers() {
        for key in [press('j'), plain(KeyCode::Down)] {
            assert_eq!(resolve(Context::List, key), Some(Command::MoveDown));
        }
        for key in [press('k'), plain(KeyCode::Up)] {
            assert_eq!(resolve(Context::List, key), Some(Command::MoveUp));
        }
        assert_eq!(resolve(Context::List, press('g')), Some(Command::GoToTop));
        assert_eq!(
            resolve(Context::List, press('G')),
            Some(Command::GoToBottom)
        );
        assert_eq!(resolve(Context::List, ctrl('d')), Some(Command::PageDown));
        assert_eq!(resolve(Context::List, ctrl('u')), Some(Command::PageUp));
        assert_eq!(resolve(Context::List, press('r')), Some(Command::Refresh));
    }

    #[test]
    fn a_lowercase_g_and_an_uppercase_g_are_different_keys() {
        // `modifiers` drops SHIFT on a character, so `Shift+g` has to arrive as
        // the character `G` and resolve to the other end of the list.
        let shifted = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
        assert_eq!(resolve(Context::List, shifted), Some(Command::GoToBottom));
        assert_eq!(resolve(Context::List, press('g')), Some(Command::GoToTop));
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
                // Cloned: `Command` is no longer `Copy`.
                Some(binding.command.clone()),
                "a visible hint names a key that resolves to something else"
            );
        }
    }

    #[test]
    fn the_sidebar_shows_three_hints_and_a_list_shows_four() {
        let sidebar = hints(Context::Sidebar);
        assert_eq!(sidebar.len(), 3, "hints were: {sidebar:?}");
        assert!(sidebar[0].starts_with("↑↓ "));
        assert!(sidebar[1].starts_with("Enter "));
        assert!(sidebar[2].starts_with("q "));

        // The list's navigation keys read as one entry, and `g`, `G`, `Ctrl+D`,
        // `Ctrl+U` and `r` are `Hidden`: four entries, not nine.
        let list = hints(Context::List);
        assert_eq!(
            list,
            ["↑↓ Navigate", "Enter Edit", "n New", "Esc Back"],
            "hints were: {list:?}"
        );
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

    /// The example schema, so a context can be derived from a real state.
    fn example() -> AppState {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/blog/pressa.yaml");
        AppState::new(pressa_app::config::load_schema(&path).expect("the example loads"))
    }

    /// The example at `Route::New` for posts, focused on field `focus`.
    fn editing(focus: usize, input: Option<&str>) -> AppState {
        let mut state = example();
        state.route = crate::tui::state::Route::New {
            collection: "posts".to_string(),
        };
        state.editor.focus = focus;
        state.editor.input = input.map(str::to_string);
        state
    }

    #[test]
    fn the_context_comes_from_the_overlay_then_the_input_then_the_route() {
        use crate::tui::state::{Overlay, Route};

        let mut state = example();
        assert_eq!(context_for(&state), Context::Sidebar);
        state.route = Route::List {
            collection: "posts".to_string(),
        };
        assert_eq!(context_for(&state), Context::List);

        // posts: title, slug, status, content, views, featured, published_at,
        // metadata — content (3) and metadata (7) are the multi-line two.
        for focus in 0..8 {
            assert_eq!(context_for(&editing(focus, None)), Context::Editor);
        }
        for focus in [0, 1, 4, 6] {
            assert_eq!(
                context_for(&editing(focus, Some("x"))),
                Context::EditorInput,
                "focus {focus}"
            );
        }
        for focus in [3, 7] {
            assert_eq!(
                context_for(&editing(focus, Some("x"))),
                Context::EditorText,
                "focus {focus}"
            );
        }

        // Overlay first, even over a field being typed into.
        let mut state = editing(0, Some("x"));
        state.overlay = Some(Overlay::ConfirmDiscard {
            next: Box::new(Route::Home),
        });
        assert_eq!(context_for(&state), Context::ConfirmDiscard);

        // And at Edit as at New.
        let mut state = editing(0, None);
        state.route = Route::Edit {
            collection: "posts".to_string(),
            id: pressa_app::domain::RecordId::new(),
        };
        assert_eq!(context_for(&state), Context::Editor);
    }

    // -----------------------------------------------------------------------
    // SPEC-008: the editor's contexts
    // -----------------------------------------------------------------------

    fn shift_tab() -> KeyEvent {
        KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)
    }

    #[test]
    fn every_new_row_resolves_in_its_context() {
        let rows: Vec<(Context, KeyEvent, Command)> = vec![
            (Context::List, plain(KeyCode::Enter), Command::EditRecord),
            (Context::List, press('n'), Command::NewRecord),
            (Context::Editor, plain(KeyCode::Tab), Command::NextField),
            (Context::Editor, press('j'), Command::NextField),
            (Context::Editor, shift_tab(), Command::PrevField),
            (Context::Editor, plain(KeyCode::BackTab), Command::PrevField),
            (Context::Editor, press('k'), Command::PrevField),
            (Context::Editor, plain(KeyCode::Enter), Command::BeginEdit),
            (Context::Editor, press(' '), Command::ToggleBoolean),
            (Context::Editor, press('l'), Command::NextOption),
            (Context::Editor, plain(KeyCode::Right), Command::NextOption),
            (Context::Editor, press('h'), Command::PrevOption),
            (Context::Editor, plain(KeyCode::Left), Command::PrevOption),
            (Context::Editor, ctrl('s'), Command::Save),
            (Context::Editor, plain(KeyCode::Esc), Command::Back),
            (
                Context::EditorInput,
                plain(KeyCode::Backspace),
                Command::InputBackspace,
            ),
            (
                Context::EditorInput,
                plain(KeyCode::Enter),
                Command::CommitField,
            ),
            (
                Context::EditorInput,
                plain(KeyCode::Esc),
                Command::CancelEdit,
            ),
            (
                Context::EditorText,
                plain(KeyCode::Backspace),
                Command::InputBackspace,
            ),
            (
                Context::EditorText,
                plain(KeyCode::Tab),
                Command::CommitField,
            ),
            (
                Context::EditorText,
                plain(KeyCode::Enter),
                Command::InsertNewline,
            ),
            (
                Context::EditorText,
                plain(KeyCode::Esc),
                Command::CancelEdit,
            ),
            (Context::ConfirmDiscard, press('y'), Command::Confirm),
            (
                Context::ConfirmDiscard,
                plain(KeyCode::Enter),
                Command::Confirm,
            ),
            (Context::ConfirmDiscard, press('n'), Command::Dismiss),
            (
                Context::ConfirmDiscard,
                plain(KeyCode::Esc),
                Command::Dismiss,
            ),
            (Context::ConfirmDiscard, press('q'), Command::Dismiss),
        ];
        for (context, key, command) in rows {
            assert_eq!(
                resolve(context, key),
                Some(command.clone()),
                "{context:?} {key:?}"
            );
            // And nowhere else: the row is the only one for its key in its
            // context, and the key does not reach the command globally — a
            // context's binding never leaks out of it.
            let rows = KEYMAP
                .iter()
                .filter(|binding| binding.context == context && matches(binding.key, key))
                .count();
            assert_eq!(rows, 1, "{context:?} {key:?} is bound {rows} times");
            assert_ne!(in_context(Context::Global, key), Some(command));
        }
    }

    #[test]
    fn the_input_contexts_type_every_printable_character() {
        for context in [Context::EditorInput, Context::EditorText] {
            for c in (' '..='~').chain(['é', 'ü', '✓', '—']) {
                assert_eq!(
                    resolve(context, press(c)),
                    Some(Command::InputChar(c)),
                    "{context:?} on {c:?}"
                );
            }
            // Shift on a character is the character's own case.
            let shifted = KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::SHIFT);
            assert_eq!(resolve(context, shifted), Some(Command::InputChar('Q')));
            // The keys that move or save in `Editor` are text here, or nothing.
            assert_eq!(resolve(context, press('j')), Some(Command::InputChar('j')));
            assert_eq!(resolve(context, ctrl('s')), None, "{context:?}");
        }
        assert_eq!(resolve(Context::EditorInput, plain(KeyCode::Tab)), None);
    }

    #[test]
    fn enter_and_tab_swap_roles_between_the_input_contexts() {
        assert_eq!(
            resolve(Context::EditorText, plain(KeyCode::Enter)),
            Some(Command::InsertNewline)
        );
        assert_eq!(
            resolve(Context::EditorText, plain(KeyCode::Tab)),
            Some(Command::CommitField)
        );
        assert_eq!(
            resolve(Context::EditorInput, plain(KeyCode::Enter)),
            Some(Command::CommitField)
        );
        // `InsertNewline` resolves only in `EditorText`.
        for binding in KEYMAP {
            if binding.command == Command::InsertNewline {
                assert_eq!(binding.context, Context::EditorText);
            }
        }
    }

    #[test]
    fn the_discard_dialog_binds_its_five_keys_and_nothing_else() {
        assert_eq!(resolve(Context::ConfirmDiscard, ctrl('s')), None);
        assert_eq!(resolve(Context::ConfirmDiscard, plain(KeyCode::Tab)), None);
        assert_eq!(resolve(Context::ConfirmDiscard, press('j')), None);
    }

    #[test]
    fn ctrl_c_still_quits_from_every_editor_context() {
        for context in [
            Context::Editor,
            Context::EditorInput,
            Context::EditorText,
            Context::ConfirmDiscard,
        ] {
            assert_eq!(
                resolve(context, ctrl('c')),
                Some(Command::Quit),
                "{context:?}"
            );
        }
    }

    #[test]
    fn question_mark_does_nothing_until_t11_except_where_it_is_text() {
        for context in [
            Context::Global,
            Context::Sidebar,
            Context::List,
            Context::Editor,
            Context::ConfirmDiscard,
        ] {
            assert_eq!(resolve(context, press('?')), None, "{context:?}");
        }
        // In a field being typed into, `?` is a character like any other: a
        // title may end in a question mark.
        for context in [Context::EditorInput, Context::EditorText] {
            assert_eq!(resolve(context, press('?')), Some(Command::InputChar('?')));
        }
    }

    #[test]
    fn the_editor_contexts_show_spec_008s_four_bars() {
        // Compared word by word with the spec's table, so this is the one
        // place the bars are written down; every renderer computes them.
        assert_eq!(
            hints(Context::Editor),
            ["Tab Next", "Enter Edit", "Ctrl+S Save", "Esc Back"]
        );
        assert_eq!(hints(Context::EditorInput), ["Enter Done", "Esc Cancel"]);
        assert_eq!(
            hints(Context::EditorText),
            ["Tab Done", "Enter Newline", "Esc Cancel"]
        );
        assert_eq!(hints(Context::ConfirmDiscard), ["y Discard", "n Cancel"]);
        assert_eq!(
            hints(Context::List),
            ["↑↓ Navigate", "Enter Edit", "n New", "Esc Back"]
        );
    }

    #[test]
    fn the_save_key_is_named_from_the_table() {
        assert_eq!(
            key_for(Context::Editor, &Command::Save).as_deref(),
            Some("Ctrl+S")
        );
        assert_eq!(key_for(Context::Sidebar, &Command::Save), None);
    }
}
