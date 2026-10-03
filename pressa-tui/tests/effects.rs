//! `run_effects` — the one function in the crate that calls a service
//! ([SPEC-007](../../specs/007-list-view.md) "API") — and the `dev` wiring it
//! is reached through.
//!
//! The only tests in the crate that open a repository. They use
//! `MemoryRepository`, so a failing effect test cannot be blamed on SQL; the
//! last one is the exception, and says why.

mod support;

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use pressa_app::RecordService;
use pressa_app::config::load_schema;
use pressa_storage::{MemoryRepository, SqliteRepository};

use pressa_tui::tui::{AppState, Command, Effect, run_effects};

use support::{TempTree, example_schema, example_schema_path};

#[test]
fn load_records_answers_with_the_records_the_service_lists() {
    let schema = example_schema();
    let service = RecordService::new(MemoryRepository::new(), schema.clone());

    // Created in this order, and `ListParams::default()` lists by id, which is
    // chronological: the answer must come back in the same order.
    for (title, slug) in [("Hello", "hello"), ("About", "about"), ("Notes", "notes")] {
        let document = format!(r#"{{"title":"{title}","slug":"{slug}","status":"draft"}}"#);
        service
            .create("posts", document.parse().expect("the document is JSON"))
            .expect("the document satisfies the schema");
    }

    let commands = run_effects(
        &service,
        vec![Effect::LoadRecords {
            collection: "posts".to_string(),
        }],
    );

    let [Command::RecordsLoaded(records)] = commands.as_slice() else {
        panic!("one LoadRecords answers with one RecordsLoaded, got {commands:?}");
    };
    let titles: Vec<&str> = records
        .iter()
        // `as_str` on the borrowed `Json`: the data is a document, not a struct.
        .map(|record| record.data["title"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(titles, ["Hello", "About", "Notes"]);
}

#[test]
fn an_empty_collection_answers_with_an_empty_load_and_not_a_failure() {
    let service = RecordService::new(MemoryRepository::new(), example_schema());

    let commands = run_effects(
        &service,
        vec![Effect::LoadRecords {
            collection: "posts".to_string(),
        }],
    );

    assert_eq!(commands, vec![Command::RecordsLoaded(Vec::new())]);
}

#[test]
fn a_list_that_fails_answers_with_the_errors_own_message() {
    let service = RecordService::new(MemoryRepository::new(), example_schema());

    // A collection the schema does not know: `RecordService::list` refuses it
    // before it reaches the repository, which is a real `AppError` on the real
    // path, needing no fake repository to produce.
    let commands = run_effects(
        &service,
        vec![Effect::LoadRecords {
            collection: "drafts".to_string(),
        }],
    );

    assert_eq!(
        commands,
        vec![Command::OperationFailed(
            "unknown collection: drafts".to_string()
        )],
        "a failure answers with the message and no records"
    );
}

#[test]
fn every_effect_is_run_in_order() {
    let service = RecordService::new(MemoryRepository::new(), example_schema());

    let commands = run_effects(
        &service,
        vec![
            Effect::LoadRecords {
                collection: "drafts".to_string(),
            },
            Effect::LoadRecords {
                collection: "posts".to_string(),
            },
        ],
    );

    assert_eq!(
        commands,
        vec![
            Command::OperationFailed("unknown collection: drafts".to_string()),
            Command::RecordsLoaded(Vec::new()),
        ]
    );
}

#[test]
fn no_effects_ask_for_nothing() {
    let service = RecordService::new(MemoryRepository::new(), example_schema());
    assert!(run_effects(&service, Vec::new()).is_empty());
}

// ---------------------------------------------------------------------------
// The wiring `cli::dev` builds (SPEC-007 "The loop")
// ---------------------------------------------------------------------------

#[test]
fn the_services_dev_builds_load_and_draw_a_collections_records() {
    // The one test that opens SQLite, and the reason is the criterion: `dev`
    // is the composition root, so the proof that a real `pressa dev` draws
    // real records has to use the repository `dev` actually builds.
    // `assert_cmd` cannot see a frame, so the loop is driven here instead.
    let tree = TempTree::new("dev-wiring");
    let schema = load_schema(&example_schema_path()).expect("the example schema is valid");
    let repository = SqliteRepository::open(tree.root().join("data.db")).expect("a fresh database");

    // Exactly `cli::dev`'s own two lines: the schema is cloned because the
    // service owns one, and the service is handed to the loop.
    let records = RecordService::new(repository, schema.clone());
    records
        .create(
            "posts",
            r#"{"title":"Hello world","slug":"hello-world","status":"draft","views":7}"#
                .parse()
                .expect("the document is JSON"),
        )
        .expect("the document satisfies the schema");

    let mut state = AppState::new(schema);
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");

    // `Enter` opens the collection, `q` comes back, `q` quits — the keys a
    // person would press.
    let mut keys = [
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyCode::Char('q'),
        crossterm::event::KeyCode::Char('q'),
    ]
    .into_iter();
    let mut events = move || match keys.next() {
        Some(code) => Ok(crossterm::event::Event::Key(
            crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE),
        )),
        None => Err(std::io::Error::other("the loop read past the quit")),
    };
    // The closure `tui::run` builds, over the real service.
    let mut effects = |effects| run_effects(&records, effects);

    pressa_tui::tui::drive(&mut state, &mut terminal, &mut events, &mut effects)
        .expect("the loop exits cleanly");

    // The record reached the state through the real service and the real
    // effect runner, with no `_records` binding left anywhere.
    assert_eq!(state.list.records.len(), 1);
    assert_eq!(state.list.records[0].data["title"], "Hello world");
}

#[test]
fn the_drawn_frame_holds_the_records_the_database_had() {
    let tree = TempTree::new("dev-frame");
    let schema = load_schema(&example_schema_path()).expect("the example schema is valid");
    let repository = SqliteRepository::open(tree.root().join("data.db")).expect("a fresh database");
    let records = RecordService::new(repository, schema.clone());

    for (title, slug) in [("Hello world", "hello-world"), ("About page", "about-page")] {
        let document = format!(r#"{{"title":"{title}","slug":"{slug}","status":"draft"}}"#);
        records
            .create("posts", document.parse().expect("the document is JSON"))
            .expect("the document satisfies the schema");
    }

    let mut state = AppState::new(schema);
    let loaded = run_effects(
        &records,
        vec![Effect::LoadRecords {
            collection: "posts".to_string(),
        }],
    );

    pressa_tui::tui::update(&mut state, Command::Select);
    for command in loaded {
        pressa_tui::tui::update(&mut state, command);
    }

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");
    terminal
        .draw(|frame| pressa_tui::tui::view(&state, frame))
        .expect("the test backend never fails to draw");

    // The titles are on the screen, out of SQLite, through the table.
    let screen: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(screen.contains("Hello world"), "screen was:\n{screen}");
    assert!(screen.contains("About page"), "screen was:\n{screen}");
    assert!(screen.contains("2 records"), "the title counts them");
}
