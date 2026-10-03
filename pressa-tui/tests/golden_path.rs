//! The M0 Golden Path — [SPEC-000](../../specs/000-m0-golden-path.md) — driven
//! through `update` and `run_effects` against a real `SqliteRepository` in a
//! temporary directory.
//!
//! T9 writes the steps it owns: 5 to 11, and 13. T12 completes the file with
//! the rest (SPEC-000 §Tests). Steps 12 and 13's restart is modelled by
//! dropping the repository and opening the same database file again.

mod support;

use std::path::Path;

use pressa_app::RecordService;
use pressa_app::domain::{ErrorCode, Json, Schema};
use pressa_storage::SqliteRepository;

use pressa_tui::tui::{AppState, Command, Route, run_effects, update};

use support::{TempTree, example_schema, schema_from_yaml};

/// Sends `command`, then runs every effect it leads to and feeds the answers
/// back — what `tui::drain` does between two draws.
fn dispatch(state: &mut AppState, service: &RecordService<SqliteRepository>, command: Command) {
    let mut queue = update(state, command);
    while !queue.is_empty() {
        let mut next = Vec::new();
        // `take` hands the queue over and leaves an empty one to refill.
        for answer in run_effects(service, std::mem::take(&mut queue)) {
            next.extend(update(state, answer));
        }
        queue = next;
    }
}

fn send(state: &mut AppState, service: &RecordService<SqliteRepository>, commands: &[Command]) {
    for command in commands {
        dispatch(state, service, command.clone());
    }
}

/// `BeginEdit`, one `InputChar` per character, `CommitField`.
fn fill(state: &mut AppState, service: &RecordService<SqliteRepository>, text: &str) {
    dispatch(state, service, Command::BeginEdit);
    for c in text.chars() {
        dispatch(state, service, Command::InputChar(c));
    }
    dispatch(state, service, Command::CommitField);
}

fn open(path: &Path, schema: &Schema) -> RecordService<SqliteRepository> {
    let repository = SqliteRepository::open(path).expect("the database opens");
    RecordService::new(repository, schema.clone())
}

fn count(service: &RecordService<SqliteRepository>, collection: &str) -> u64 {
    service
        .count(collection, &Default::default())
        .expect("count")
}

fn json(text: &str) -> Json {
    text.parse().expect("the case is JSON")
}

/// Everything T9 owns of the scenario, for `collection` in `schema`, returning
/// the state after every step so two runs can be compared.
fn scenario(schema: Schema, collection: &str) -> Vec<(Route, Json)> {
    let tree = TempTree::new("golden-path");
    let database = tree.root().join("data.db");
    let service = open(&database, &schema);
    let mut state = AppState::new(schema.clone());
    let mut trail = Vec::new();
    // The route, and the draft with its volatile parts — none here — kept.
    let mut mark = |state: &AppState| {
        let route = match &state.route {
            // The id is minted per run; compare its shape, not its value.
            Route::Edit { collection, .. } => Route::New {
                collection: format!("edit:{collection}"),
            },
            other => other.clone(),
        };
        trail.push((route, state.editor.draft.clone()));
    };

    // Step 4, so that 5 has a list to start from.
    dispatch(&mut state, &service, Command::Select);
    let list = Route::List {
        collection: collection.to_string(),
    };
    assert_eq!(state.route, list);

    // Step 5: `n`.
    dispatch(&mut state, &service, Command::NewRecord);
    assert_eq!(
        state.route,
        Route::New {
            collection: collection.to_string()
        }
    );
    assert_eq!(state.editor.focus, 0);
    mark(&state);

    // Step 6: a blank save, refused with exactly three reasons.
    dispatch(&mut state, &service, Command::Save);
    let errors: Vec<(&str, ErrorCode, &str)> = state
        .editor
        .errors
        .iter()
        .map(|error| (error.field.as_str(), error.code, error.message.as_str()))
        .collect();
    assert_eq!(
        errors,
        [
            ("title", ErrorCode::Required, "required"),
            ("slug", ErrorCode::Required, "required"),
            ("status", ErrorCode::Required, "required"),
        ]
    );
    assert_eq!(count(&service, collection), 0, "nothing was written");
    assert!(matches!(state.route, Route::New { .. }));
    mark(&state);

    // Step 7: four fields filled.
    fill(&mut state, &service, "Hello world");
    dispatch(&mut state, &service, Command::NextField);
    fill(&mut state, &service, "hello-world");
    dispatch(&mut state, &service, Command::NextField);
    send(
        &mut state,
        &service,
        &[Command::NextOption, Command::NextOption],
    );
    send(
        &mut state,
        &service,
        &[Command::NextField, Command::NextField],
    );
    fill(&mut state, &service, "42");
    assert!(state.editor.is_dirty(), "the unsaved indicator is on");
    assert!(
        state.editor.errors.is_empty(),
        "every refused field was filled, so no `⚠` is left: {:?}",
        state.editor.errors
    );
    mark(&state);

    // Step 8: saved, back at the list, reloaded.
    dispatch(&mut state, &service, Command::Save);
    assert_eq!(state.route, list);
    assert_eq!(
        state.status.as_ref().map(|status| status.text.as_str()),
        Some("Record saved.")
    );
    let stored = service.list(collection, &Default::default()).expect("list");
    assert_eq!(stored.len(), 1);
    let record = &stored[0];
    assert_eq!(record.data["title"], json(r#""Hello world""#));
    assert_eq!(record.data["slug"], json(r#""hello-world""#));
    assert_eq!(record.data["status"], json(r#""published""#));
    assert_eq!(record.data["views"], json("42"));
    mark(&state);

    // Step 9: the list shows the one row.
    assert_eq!(state.list.records.len(), 1);
    assert_eq!(state.list.records[0].data, record.data);

    // Step 10: `Enter` on the row opens it, clean.
    dispatch(&mut state, &service, Command::EditRecord);
    assert_eq!(
        state.route,
        Route::Edit {
            collection: collection.to_string(),
            id: record.id
        }
    );
    assert_eq!(state.editor.draft, record.data);
    assert!(!state.editor.is_dirty());
    mark(&state);

    // Step 11: rename and save; updated in place.
    // Wait out the clock's resolution so `updated_at` can only move forward.
    std::thread::sleep(std::time::Duration::from_millis(5));
    dispatch(&mut state, &service, Command::BeginEdit);
    for _ in "Hello world".chars() {
        dispatch(&mut state, &service, Command::InputBackspace);
    }
    for c in "Hello, world!".chars() {
        dispatch(&mut state, &service, Command::InputChar(c));
    }
    dispatch(&mut state, &service, Command::CommitField);
    dispatch(&mut state, &service, Command::Save);
    assert_eq!(state.route, list);
    let renamed = service.get(collection, &record.id).expect("still there");
    assert_eq!(renamed.data["title"], json(r#""Hello, world!""#));
    assert_eq!(renamed.id, record.id);
    assert_eq!(renamed.created_at, record.created_at);
    assert!(renamed.updated_at > record.updated_at, "strictly later");
    assert_eq!(
        state.list.records[0].data["title"],
        json(r#""Hello, world!""#)
    );
    mark(&state);

    // Steps 12 and 13: quit, drop the repository, open the file again.
    dispatch(&mut state, &service, Command::Back);
    dispatch(&mut state, &service, Command::Quit);
    assert!(state.should_quit);
    drop(service);

    let reopened = open(&database, &schema);
    let mut state = AppState::new(schema);
    dispatch(&mut state, &reopened, Command::Select);
    assert_eq!(state.list.records.len(), 1);
    assert_eq!(state.list.records[0].id, record.id);
    assert_eq!(
        state.list.records[0].data["title"],
        json(r#""Hello, world!""#)
    );
    mark(&state);

    trail
}

#[test]
fn steps_5_to_11_and_13_against_sqlite() {
    scenario(example_schema(), "posts");
}

#[test]
fn the_same_sequence_under_another_slug_gives_the_same_transitions() {
    // The example's fields, under a collection that is not `posts`.
    let yaml = std::fs::read_to_string(support::example_schema_path())
        .expect("the example is readable")
        .replace("  posts:", "  articles:")
        .replace("label: Posts", "label: Articles");
    let other = scenario(schema_from_yaml("articles", &yaml), "articles");
    let posts = scenario(example_schema(), "posts");

    // Route by route and draft by draft, modulo the slug itself.
    assert_eq!(other.len(), posts.len());
    for ((route, draft), (posts_route, posts_draft)) in other.iter().zip(&posts) {
        assert_eq!(draft, posts_draft);
        let renamed = format!("{route:?}").replace("articles", "posts");
        assert_eq!(renamed, format!("{posts_route:?}"));
    }
}
