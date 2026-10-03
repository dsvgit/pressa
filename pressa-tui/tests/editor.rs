//! `update` for the record editor — [SPEC-008](../../specs/008-record-editor.md)
//! "Routes", "Editing", "Dirty state and leaving" and the state half of
//! "Saving".
//!
//! No test here opens a repository or a terminal: a state is built, a command
//! is sent, and the state and the effects are asserted. Records arrive through
//! `support::records`, which goes through `RecordService::create` against a
//! `MemoryRepository`, so no test constructs a `Record` by hand.

mod support;

use pressa_app::domain::{ErrorCode, FieldError, Json, Record};

use pressa_tui::tui::keymap::KEYMAP;
use pressa_tui::tui::{
    AppState, Command, Effect, Load, Overlay, Route, StatusKind, StatusMessage, update,
};

use support::{example_schema, records};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const HELLO: &str =
    r#"{"title":"Hello world","slug":"hello-world","status":"published","views":42}"#;

/// The posts list with `documents` loaded, reached through the commands.
fn posts_list(documents: &[&str]) -> AppState {
    let schema = example_schema();
    let loaded = records(&schema, "posts", documents);
    let mut state = AppState::new(schema);
    update(&mut state, Command::Select);
    update(&mut state, Command::RecordsLoaded(loaded));
    state
}

/// `n` on an empty posts list.
fn new_post() -> AppState {
    let mut state = posts_list(&[]);
    update(&mut state, Command::NewRecord);
    state
}

/// `Enter` on a list holding `document`, and the load answered.
fn edit_post(document: &str) -> (AppState, Record) {
    let mut state = posts_list(&[document]);
    let record = state.list.records[0].clone();
    update(&mut state, Command::EditRecord);
    update(&mut state, Command::RecordLoaded(Box::new(record.clone())));
    (state, record)
}

fn send(state: &mut AppState, commands: &[Command]) -> Vec<Effect> {
    let mut effects = Vec::new();
    for command in commands {
        effects.extend(update(state, command.clone()));
    }
    effects
}

/// One `InputChar` per character of `text`.
fn typed(text: &str) -> Vec<Command> {
    text.chars().map(Command::InputChar).collect()
}

/// `BeginEdit`, the text, `CommitField` — filling the focused field.
fn fill(state: &mut AppState, text: &str) {
    update(state, Command::BeginEdit);
    send(state, &typed(text));
    update(state, Command::CommitField);
}

fn posts() -> Route {
    Route::List {
        collection: "posts".to_string(),
    }
}

fn json(text: &str) -> Json {
    text.parse().expect("the case is JSON")
}

/// Step 7 of SPEC-000, as the commands its keystrokes resolve to.
fn step_seven(state: &mut AppState) {
    fill(state, "Hello world");
    update(state, Command::NextField);
    fill(state, "hello-world");
    update(state, Command::NextField);
    send(state, &[Command::NextOption, Command::NextOption]);
    send(state, &[Command::NextField, Command::NextField]);
    fill(state, "42");
}

// ---------------------------------------------------------------------------
// Routes and reaching the form
// ---------------------------------------------------------------------------

#[test]
fn enter_on_a_selected_record_routes_to_edit_and_loads_it() {
    let mut state = posts_list(&[HELLO, r#"{"title":"B","slug":"b","status":"draft"}"#]);
    update(&mut state, Command::MoveDown);
    let id = state.list.records[1].id;

    let effects = update(&mut state, Command::EditRecord);

    assert_eq!(
        state.route,
        Route::Edit {
            collection: "posts".to_string(),
            id
        }
    );
    assert_eq!(
        effects,
        vec![Effect::LoadRecord {
            collection: "posts".to_string(),
            id
        }]
    );
}

#[test]
fn enter_on_an_empty_or_failed_list_does_nothing() {
    let empty = posts_list(&[]);
    let mut failed = posts_list(&[HELLO]);
    update(&mut failed, Command::OperationFailed("boom".to_string()));
    assert_eq!(failed.list.load, Load::Failed);

    for before in [empty, failed] {
        let mut state = before.clone();
        let effects = update(&mut state, Command::EditRecord);
        assert!(effects.is_empty());
        assert_eq!(state, before, "no state changed");
    }
}

#[test]
fn n_routes_to_new_with_the_blank_draft_and_no_effect() {
    let schema = example_schema();
    let mut state = posts_list(&[]);

    let effects = update(&mut state, Command::NewRecord);

    assert!(effects.is_empty(), "blank_document is pure, so no effect");
    assert_eq!(
        state.route,
        Route::New {
            collection: "posts".to_string()
        }
    );
    let posts = schema.collections.get("posts").expect("posts");
    assert_eq!(
        state.editor.draft,
        pressa_app::domain::blank_document(posts)
    );
    assert_eq!(state.editor.focus, 0);
}

#[test]
fn n_on_a_collection_whose_load_failed_still_opens_the_editor() {
    let mut state = posts_list(&[]);
    update(&mut state, Command::OperationFailed("boom".to_string()));
    update(&mut state, Command::NewRecord);
    assert!(matches!(state.route, Route::New { .. }));
}

#[test]
fn record_loaded_fills_the_draft_and_the_original_from_the_record() {
    let (state, record) = edit_post(HELLO);

    assert_eq!(state.editor.draft, record.data);
    assert_eq!(state.editor.original, record.data);
    assert_eq!(state.editor.focus, 0);
    assert_eq!(state.editor.offset, 0);
    assert!(state.editor.errors.is_empty());
    assert_eq!(state.editor.id, Some(record.id));
}

#[test]
fn a_loaded_draft_and_a_blank_one_are_both_clean() {
    assert!(!new_post().editor.is_dirty());
    assert!(!edit_post(HELLO).0.editor.is_dirty());
}

// ---------------------------------------------------------------------------
// Editing
// ---------------------------------------------------------------------------

#[test]
fn next_and_prev_field_move_the_focus_and_clamp_at_both_ends() {
    let mut state = new_post();

    assert!(update(&mut state, Command::PrevField).is_empty());
    assert_eq!(state.editor.focus, 0, "clamped at the first");

    assert!(update(&mut state, Command::NextField).is_empty());
    assert_eq!(state.editor.focus, 1);

    for _ in 0..20 {
        update(&mut state, Command::NextField);
    }
    assert_eq!(state.editor.focus, 7, "clamped at the last, never wrapping");

    update(&mut state, Command::PrevField);
    assert_eq!(state.editor.focus, 6);
}

#[test]
fn begin_edit_dispatches_on_the_focused_fields_type() {
    let (mut state, _) = edit_post(HELLO);

    // Text, Textarea, Number, DateTime, Json: the input buffer opens with the
    // editable text. posts: 0 title, 3 content, 4 views, 6 published_at,
    // 7 metadata.
    for (focus, seeded) in [(0, "Hello world"), (3, ""), (4, "42"), (6, ""), (7, "")] {
        state.editor.focus = focus;
        state.editor.input = None;
        update(&mut state, Command::BeginEdit);
        assert_eq!(state.editor.input.as_deref(), Some(seeded), "focus {focus}");
    }
    state.editor.input = None;

    // Boolean: toggled, no input.
    state.editor.focus = 5;
    update(&mut state, Command::BeginEdit);
    assert_eq!(state.editor.draft["featured"], json("true"));
    assert_eq!(state.editor.input, None);

    // Select: the next option, no input.
    state.editor.focus = 2;
    update(&mut state, Command::BeginEdit);
    assert_eq!(state.editor.draft["status"], json(r#""draft""#), "wrapped");
    assert_eq!(state.editor.input, None);
}

#[test]
fn input_char_appends_and_backspace_removes_the_last_character() {
    let mut state = new_post();
    update(&mut state, Command::BeginEdit);
    send(&mut state, &typed("Hé✓"));
    assert_eq!(state.editor.input.as_deref(), Some("Hé✓"));

    update(&mut state, Command::InputBackspace);
    assert_eq!(
        state.editor.input.as_deref(),
        Some("Hé"),
        "a character, not a byte"
    );

    send(
        &mut state,
        &[Command::InputBackspace, Command::InputBackspace],
    );
    assert_eq!(state.editor.input.as_deref(), Some(""));
    update(&mut state, Command::InputBackspace);
    assert_eq!(
        state.editor.input.as_deref(),
        Some(""),
        "a no-op when empty"
    );
}

#[test]
fn insert_newline_appends_a_newline() {
    let mut state = new_post();
    state.editor.focus = 3; // content, a textarea
    update(&mut state, Command::BeginEdit);
    send(&mut state, &typed("a"));
    update(&mut state, Command::InsertNewline);
    send(&mut state, &typed("b"));
    assert_eq!(state.editor.input.as_deref(), Some("a\nb"));
}

#[test]
fn typing_never_touches_the_draft_and_the_state_stays_clean() {
    let mut state = new_post();
    let draft = state.editor.draft.clone();

    update(&mut state, Command::BeginEdit);
    send(&mut state, &typed("abc"));
    update(&mut state, Command::InputBackspace);
    update(&mut state, Command::InsertNewline);

    assert_eq!(state.editor.draft, draft);
    assert!(!state.editor.is_dirty());
}

#[test]
fn commit_field_writes_the_parsed_value_and_keeps_the_focus() {
    let mut state = new_post();
    fill(&mut state, "Hello");

    assert_eq!(state.editor.draft["title"], json(r#""Hello""#));
    assert_eq!(state.editor.input, None);
    assert_eq!(state.editor.focus, 0);
}

#[test]
fn commit_field_on_42_in_a_number_field_stores_the_number() {
    let mut state = new_post();
    state.editor.focus = 4; // views
    fill(&mut state, "42");
    assert_eq!(state.editor.draft["views"], json("42"));
    assert!(state.editor.draft["views"].is_number());
}

#[test]
fn a_commit_that_does_not_parse_keeps_the_text_and_says_why() {
    // views (Number), published_at (DateTime), metadata (Json).
    let cases = [
        (
            4,
            "views",
            "forty",
            ErrorCode::TypeMismatch,
            "must be a number",
        ),
        (
            6,
            "published_at",
            "tuesday",
            ErrorCode::InvalidDateTime,
            "must be a date like 2026-09-06T12:00:00Z",
        ),
        (
            7,
            "metadata",
            "{nope",
            ErrorCode::InvalidJson,
            "not valid JSON",
        ),
    ];
    for (focus, name, text, code, message) in cases {
        let mut state = new_post();
        state.editor.focus = focus;
        let draft = state.editor.draft.clone();

        fill(&mut state, text);

        assert_eq!(state.editor.input.as_deref(), Some(text), "{name}");
        assert_eq!(state.editor.draft, draft, "{name}");
        assert_eq!(
            state.editor.errors,
            vec![FieldError {
                field: name.to_string(),
                code,
                message: message.to_string(),
            }]
        );
    }
}

#[test]
fn a_later_successful_commit_removes_only_its_own_error() {
    let mut state = new_post();
    // A failed save first, so other fields carry errors too.
    update(
        &mut state,
        Command::SaveFailed(vec![
            FieldError {
                field: "title".to_string(),
                code: ErrorCode::Required,
                message: "required".to_string(),
            },
            FieldError {
                field: "slug".to_string(),
                code: ErrorCode::Required,
                message: "required".to_string(),
            },
        ]),
    );
    state.editor.focus = 4;
    fill(&mut state, "nope");
    assert_eq!(state.editor.errors.len(), 3);

    // Fix it: clear the buffer and type a number.
    send(&mut state, &vec![Command::InputBackspace; 4]);
    send(&mut state, &typed("7"));
    update(&mut state, Command::CommitField);

    let fields: Vec<&str> = state
        .editor
        .errors
        .iter()
        .map(|e| e.field.as_str())
        .collect();
    assert_eq!(fields, ["title", "slug"]);
}

#[test]
fn a_parse_error_replaces_the_entry_for_its_own_field() {
    let mut state = new_post();
    state.editor.focus = 4;
    fill(&mut state, "a");
    update(&mut state, Command::InputChar('b'));
    update(&mut state, Command::CommitField);
    assert_eq!(state.editor.errors.len(), 1, "one entry per field");
}

#[test]
fn cancel_edit_leaves_the_draft_exactly_as_it_was() {
    let (mut state, _) = edit_post(HELLO);
    let before = state.editor.draft.to_string();

    update(&mut state, Command::BeginEdit);
    send(&mut state, &typed("xyz"));
    update(&mut state, Command::CancelEdit);
    assert_eq!(state.editor.input, None);
    assert_eq!(state.editor.draft.to_string(), before);

    // And after a failed commit.
    state.editor.focus = 4;
    fill(&mut state, "not a number");
    update(&mut state, Command::CancelEdit);
    assert_eq!(state.editor.input, None);
    assert_eq!(state.editor.draft.to_string(), before);
}

#[test]
fn toggle_boolean_flips_one_key_and_touches_no_other() {
    let mut state = new_post();
    state.editor.focus = 5; // featured
    let before = state.editor.draft.clone();

    update(&mut state, Command::ToggleBoolean);
    assert_eq!(state.editor.draft["featured"], json("true"));
    for (key, value) in before.as_object().expect("an object") {
        if key != "featured" {
            assert_eq!(&state.editor.draft[key], value, "{key} changed");
        }
    }

    update(&mut state, Command::ToggleBoolean);
    assert_eq!(state.editor.draft, before);
}

#[test]
fn options_cycle_and_wrap_in_both_directions() {
    let mut state = new_post();
    state.editor.focus = 2; // status: [draft, published]

    update(&mut state, Command::PrevOption);
    assert_eq!(
        state.editor.draft["status"],
        json(r#""published""#),
        "from none, the last"
    );

    state.editor.draft["status"] = Json::Null;
    update(&mut state, Command::NextOption);
    assert_eq!(
        state.editor.draft["status"],
        json(r#""draft""#),
        "from none, the first"
    );
    update(&mut state, Command::NextOption);
    assert_eq!(state.editor.draft["status"], json(r#""published""#));
    update(&mut state, Command::NextOption);
    assert_eq!(state.editor.draft["status"], json(r#""draft""#), "wrapped");
    update(&mut state, Command::PrevOption);
    assert_eq!(
        state.editor.draft["status"],
        json(r#""published""#),
        "wrapped back"
    );
}

#[test]
fn next_option_twice_from_empty_is_published() {
    let mut state = new_post();
    state.editor.focus = 2;
    send(&mut state, &[Command::NextOption, Command::NextOption]);
    assert_eq!(state.editor.draft["status"], json(r#""published""#));
}

#[test]
fn a_command_aimed_at_the_wrong_type_changes_nothing() {
    for (focus, command) in [
        (0, Command::ToggleBoolean),
        (4, Command::NextOption),
        (4, Command::PrevOption),
        (2, Command::ToggleBoolean),
    ] {
        let mut state = new_post();
        state.editor.focus = focus;
        let before = state.clone();
        assert!(update(&mut state, command.clone()).is_empty());
        assert_eq!(state, before, "{command:?} at {focus}");
    }
}

#[test]
fn the_draft_keeps_exactly_the_schemas_keys_through_step_seven() {
    let mut state = new_post();
    step_seven(&mut state);

    let mut keys: Vec<&String> = state
        .editor
        .draft
        .as_object()
        .expect("an object")
        .keys()
        .collect();
    keys.sort();
    let schema = example_schema();
    let mut names: Vec<&String> = schema.collections["posts"]
        .fields
        .iter()
        .map(|field| &field.name)
        .collect();
    names.sort();
    assert_eq!(keys, names);

    assert_eq!(state.editor.draft["title"], json(r#""Hello world""#));
    assert_eq!(state.editor.draft["slug"], json(r#""hello-world""#));
    assert_eq!(state.editor.draft["status"], json(r#""published""#));
    assert_eq!(state.editor.draft["views"], json("42"));
    assert_eq!(state.editor.focus, 4);
}

// ---------------------------------------------------------------------------
// Dirty state and leaving
// ---------------------------------------------------------------------------

#[test]
fn a_committed_draft_is_dirty_and_one_edited_back_is_clean() {
    let (mut state, _) = edit_post(HELLO);
    state.editor.focus = 4;
    fill(&mut state, "7");
    assert!(state.editor.is_dirty());

    // `BeginEdit` seeded "42", so the commit stored 427; clear it and retype.
    update(&mut state, Command::BeginEdit);
    send(&mut state, &vec![Command::InputBackspace; 3]);
    send(&mut state, &typed("42"));
    update(&mut state, Command::CommitField);
    assert!(!state.editor.is_dirty(), "edited back to itself");
}

#[test]
fn back_from_a_clean_editor_returns_to_the_list() {
    for mut state in [new_post(), edit_post(HELLO).0] {
        let effects = update(&mut state, Command::Back);
        assert!(effects.is_empty(), "no reload: nothing changed");
        assert_eq!(state.route, posts());
        assert_eq!(state.overlay, None);
        assert_eq!(state.editor, Default::default(), "the editor is cleared");
    }
}

#[test]
fn back_from_a_dirty_editor_asks_first() {
    let mut state = new_post();
    fill(&mut state, "Hi");

    let effects = update(&mut state, Command::Back);

    assert!(effects.is_empty());
    assert!(matches!(state.route, Route::New { .. }), "still the editor");
    assert_eq!(
        state.overlay,
        Some(Overlay::ConfirmDiscard {
            next: Box::new(posts())
        })
    );
}

#[test]
fn confirm_leaves_and_clears_the_editor() {
    let mut state = new_post();
    fill(&mut state, "Hi");
    update(&mut state, Command::Back);

    let effects = update(&mut state, Command::Confirm);

    assert!(effects.is_empty());
    assert_eq!(state.route, posts());
    assert_eq!(state.overlay, None);
    assert_eq!(state.editor, Default::default());
}

#[test]
fn dismiss_closes_the_overlay_and_changes_nothing_else() {
    let mut state = new_post();
    fill(&mut state, "Hi");
    update(&mut state, Command::NextField);
    update(&mut state, Command::Back);
    let before = state.clone();

    assert!(update(&mut state, Command::Dismiss).is_empty());

    assert_eq!(state.overlay, None);
    assert_eq!(state.route, before.route);
    assert_eq!(state.editor, before.editor);
}

/// One of every `Command` variant, and a check that it really is every one:
/// `index` is an exhaustive match, so a new variant will not compile until it
/// is given a number, and the assertion below fails until it is listed here.
fn every_command(record: &Record) -> Vec<Command> {
    const COUNT: usize = 30;
    fn index(command: &Command) -> usize {
        match command {
            Command::MoveUp => 0,
            Command::MoveDown => 1,
            Command::Select => 2,
            Command::Back => 3,
            Command::Quit => 4,
            Command::GoToTop => 5,
            Command::GoToBottom => 6,
            Command::PageUp => 7,
            Command::PageDown => 8,
            Command::Refresh => 9,
            Command::NewRecord => 10,
            Command::EditRecord => 11,
            Command::NextField => 12,
            Command::PrevField => 13,
            Command::BeginEdit => 14,
            Command::CommitField => 15,
            Command::CancelEdit => 16,
            Command::InsertNewline => 17,
            Command::ToggleBoolean => 18,
            Command::NextOption => 19,
            Command::PrevOption => 20,
            Command::InputChar(_) => 21,
            Command::InputBackspace => 22,
            Command::Save => 23,
            Command::Confirm => 24,
            Command::Dismiss => 25,
            Command::RecordsLoaded(_) => 26,
            Command::RecordLoaded(_) => 27,
            Command::RecordSaved(_) => 28,
            Command::SaveFailed(_) => 29,
            Command::OperationFailed(_) => 30,
        }
    }

    let all = vec![
        Command::MoveUp,
        Command::MoveDown,
        Command::Select,
        Command::Back,
        Command::Quit,
        Command::GoToTop,
        Command::GoToBottom,
        Command::PageUp,
        Command::PageDown,
        Command::Refresh,
        Command::NewRecord,
        Command::EditRecord,
        Command::NextField,
        Command::PrevField,
        Command::BeginEdit,
        Command::CommitField,
        Command::CancelEdit,
        Command::InsertNewline,
        Command::ToggleBoolean,
        Command::NextOption,
        Command::PrevOption,
        Command::InputChar('x'),
        Command::InputBackspace,
        Command::Save,
        Command::Confirm,
        Command::Dismiss,
        Command::RecordsLoaded(vec![record.clone()]),
        Command::RecordLoaded(Box::new(record.clone())),
        Command::RecordSaved(Box::new(record.clone())),
        Command::SaveFailed(Vec::new()),
        Command::OperationFailed("boom".to_string()),
    ];
    let mut seen: Vec<usize> = all.iter().map(index).collect();
    seen.sort_unstable();
    assert_eq!(seen, (0..=COUNT).collect::<Vec<_>>(), "every variant, once");
    all
}

#[test]
fn only_confirm_and_record_saved_move_the_route_off_a_dirty_editor() {
    let (clean, record) = edit_post(HELLO);

    for command in every_command(&record) {
        // Dirty at Edit and at New, so both routes are held to the rule.
        let mut dirty_edit = clean.clone();
        dirty_edit.editor.focus = 4;
        fill(&mut dirty_edit, "7");
        let mut dirty_new = new_post();
        fill(&mut dirty_new, "Hi");

        for mut state in [dirty_edit, dirty_new] {
            assert!(state.editor.is_dirty());
            let route = state.route.clone();
            // With the overlay open, as `Confirm` needs, and without it.
            let mut asked = state.clone();
            update(&mut asked, Command::Back);

            for state in [&mut state, &mut asked] {
                update(state, command.clone());
                let moved = state.route != route;
                let allowed = matches!(command, Command::Confirm | Command::RecordSaved(_));
                if moved {
                    assert!(allowed, "{command:?} moved the route off a dirty editor");
                }
            }
        }
    }
}

#[test]
fn every_command_variant_is_reachable_from_a_binding_or_an_effect_result() {
    let (_, record) = edit_post(HELLO);
    // What `run_effects` can answer with.
    let results = [
        Command::RecordsLoaded(Vec::new()),
        Command::RecordLoaded(Box::new(record.clone())),
        Command::RecordSaved(Box::new(record.clone())),
        Command::SaveFailed(Vec::new()),
        Command::OperationFailed(String::new()),
    ];

    for command in every_command(&record) {
        // By variant, not by value: `InputChar('x')` is reached by any char.
        let same =
            |other: &Command| std::mem::discriminant(other) == std::mem::discriminant(&command);
        let bound = KEYMAP.iter().any(|binding| same(&binding.command));
        let answered = results.iter().any(same);
        assert!(bound || answered, "nothing can trigger {command:?}");
    }
}

// ---------------------------------------------------------------------------
// Saving: the state half
// ---------------------------------------------------------------------------

#[test]
fn save_at_new_asks_to_create_the_draft() {
    let mut state = new_post();
    fill(&mut state, "Hi");

    let effects = update(&mut state, Command::Save);

    assert_eq!(
        effects,
        vec![Effect::SaveRecord {
            collection: "posts".to_string(),
            id: None,
            data: state.editor.draft.clone(),
        }]
    );
}

#[test]
fn save_at_edit_asks_to_update_the_routes_record() {
    let (mut state, record) = edit_post(HELLO);

    let effects = update(&mut state, Command::Save);

    // A clean draft is saved all the same: an idempotent update is cheaper
    // than a special case.
    assert_eq!(
        effects,
        vec![Effect::SaveRecord {
            collection: "posts".to_string(),
            id: Some(record.id),
            data: record.data.clone(),
        }]
    );
}

#[test]
fn one_save_is_one_effect() {
    let mut state = new_post();
    assert_eq!(update(&mut state, Command::Save).len(), 1);
    assert_eq!(
        send(&mut state, &[Command::Save, Command::Save]).len(),
        2,
        "two saves, two effects"
    );
}

fn required(field: &str) -> FieldError {
    FieldError {
        field: field.to_string(),
        code: ErrorCode::Required,
        message: "required".to_string(),
    }
}

#[test]
fn save_failed_stores_the_errors_and_changes_nothing_else() {
    let mut state = new_post();
    update(&mut state, Command::NextField);
    let before = state.clone();

    let effects = update(
        &mut state,
        Command::SaveFailed(vec![
            required("title"),
            required("slug"),
            required("status"),
        ]),
    );

    assert!(effects.is_empty());
    assert_eq!(state.editor.errors.len(), 3);
    assert_eq!(state.editor.draft, before.editor.draft);
    assert_eq!(state.editor.focus, before.editor.focus);
    assert_eq!(state.editor.offset, before.editor.offset);
    assert_eq!(state.route, before.route);
    // The attention line is derived by `view`; nothing is written here.
    assert_eq!(state.status, before.status);
}

#[test]
fn save_failed_replaces_rather_than_appends() {
    let mut state = new_post();
    update(
        &mut state,
        Command::SaveFailed(vec![required("title"), required("slug")]),
    );
    update(&mut state, Command::SaveFailed(vec![required("status")]));
    assert_eq!(state.editor.errors, vec![required("status")]);
}

#[test]
fn record_saved_returns_to_the_list_and_reloads_it_from_both_routes() {
    let (edit, record) = edit_post(HELLO);
    for mut state in [new_post(), edit] {
        let effects = update(&mut state, Command::RecordSaved(Box::new(record.clone())));

        assert_eq!(state.route, posts());
        assert_eq!(state.editor, Default::default());
        assert_eq!(
            state.status,
            Some(StatusMessage {
                text: "Record saved.".to_string(),
                kind: StatusKind::Info,
            })
        );
        assert_eq!(
            effects,
            vec![Effect::LoadRecords {
                collection: "posts".to_string()
            }]
        );
    }
}

#[test]
fn a_load_that_found_nothing_returns_to_the_list_and_reloads() {
    let mut state = posts_list(&[HELLO]);
    update(&mut state, Command::EditRecord);

    let effects = update(
        &mut state,
        Command::OperationFailed("no record 01J in collection posts".to_string()),
    );

    assert_eq!(state.route, posts());
    assert_eq!(
        effects,
        vec![Effect::LoadRecords {
            collection: "posts".to_string()
        }]
    );
}

#[test]
fn a_save_that_failed_changes_neither_the_route_nor_the_draft() {
    let mut state = new_post();
    fill(&mut state, "Hi");
    let before = state.clone();

    let effects = update(
        &mut state,
        Command::OperationFailed("database error: disk I/O error".to_string()),
    );

    assert!(effects.is_empty());
    assert_eq!(state.route, before.route);
    assert_eq!(state.editor, before.editor);
    assert!(state.editor.is_dirty(), "nothing was stored");
    assert_eq!(
        state.status,
        Some(StatusMessage {
            text: "database error: disk I/O error".to_string(),
            kind: StatusKind::Error,
        })
    );
}

// ---------------------------------------------------------------------------
// Purity
// ---------------------------------------------------------------------------

#[test]
fn every_new_command_is_pure() {
    let (edit, record) = edit_post(HELLO);
    let mut typing = edit.clone();
    update(&mut typing, Command::BeginEdit);
    let mut asked = edit.clone();
    asked.editor.focus = 4;
    fill(&mut asked, "7");
    update(&mut asked, Command::Back);

    for base in [posts_list(&[HELLO]), new_post(), edit, typing, asked] {
        for command in every_command(&record) {
            let mut once = base.clone();
            let mut twice = base.clone();
            assert_eq!(
                update(&mut once, command.clone()),
                update(&mut twice, command.clone()),
                "{command:?}"
            );
            assert_eq!(once, twice, "{command:?}");
        }
    }
}
