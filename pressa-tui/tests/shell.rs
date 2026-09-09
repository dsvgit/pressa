//! `update`, breadcrumbs and the loop — [SPEC-006](../../specs/006-tui-shell.md).
//!
//! None of these opens a terminal or a database. They live here rather than
//! beside their modules only because building a [`Schema`] means going through
//! the loader, and the loader needs a file (see `support::schema_from_yaml`).

mod support;

use std::io;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use pressa_tui::tui::state::collection_label;
use pressa_tui::tui::view::breadcrumbs;
use pressa_tui::tui::{AppState, Command, Route, update};

use support::{example_schema, schema_from_yaml};

/// Three collections, labels defaulted by the loader: Posts, Authors,
/// Categories.
const THREE: &str = "\
project:
  name: blog-cms

collections:
  posts:
    fields:
      - name: title
        type: text
  authors:
    fields:
      - name: name
        type: text
  categories:
    fields:
      - name: name
        type: text
";

/// Deliberately not alphabetical: the sidebar order must be the file's.
const UNSORTED: &str = "\
project:
  name: zoo

collections:
  zebras:
    fields:
      - name: name
        type: text
  apples:
    fields:
      - name: name
        type: text
  mangoes:
    fields:
      - name: name
        type: text
";

fn three() -> AppState {
    AppState::new(schema_from_yaml("three", THREE))
}

// ---------------------------------------------------------------------------
// update
// ---------------------------------------------------------------------------

#[test]
fn the_same_state_and_command_always_produce_the_same_state() {
    // Purity, as far as a test can see it: two runs from equal starting points
    // end equal, and neither asks the world for anything.
    let mut once = three();
    let mut twice = three();

    for command in [Command::MoveDown, Command::MoveDown, Command::Select] {
        let first = update(&mut once, command);
        let second = update(&mut twice, command);
        assert!(first.is_empty(), "T7 has no effects to run");
        assert!(second.is_empty(), "T7 has no effects to run");
    }

    assert_eq!(once, twice);
}

#[test]
fn moving_down_stops_at_the_last_collection() {
    let mut state = three();

    update(&mut state, Command::MoveDown);
    update(&mut state, Command::MoveDown);
    assert_eq!(state.sidebar.selected, 2);

    // A third press changes nothing at all — clamped, never wrapping.
    let before = state.clone();
    let effects = update(&mut state, Command::MoveDown);
    assert_eq!(state.sidebar.selected, 2);
    assert_eq!(state, before);
    assert!(effects.is_empty());
}

#[test]
fn moving_up_stops_at_the_first_collection() {
    let mut state = three();

    update(&mut state, Command::MoveUp);
    assert_eq!(state.sidebar.selected, 0);
}

#[test]
fn select_routes_to_whichever_collection_is_selected() {
    // Asserted twice, at two indices, so no code path can be naming a slug.
    let mut state = three();
    let effects = update(&mut state, Command::Select);
    assert_eq!(
        state.route,
        Route::List {
            collection: "posts".to_string()
        }
    );
    assert!(
        effects.is_empty(),
        "opening a collection loads nothing in T7"
    );

    let mut state = three();
    update(&mut state, Command::MoveDown);
    update(&mut state, Command::MoveDown);
    update(&mut state, Command::Select);
    assert_eq!(
        state.route,
        Route::List {
            collection: "categories".to_string()
        }
    );
}

#[test]
fn back_returns_home_with_the_selection_untouched() {
    let mut state = three();
    update(&mut state, Command::MoveDown);
    update(&mut state, Command::Select);

    let effects = update(&mut state, Command::Back);
    assert_eq!(state.route, Route::Home);
    assert_eq!(state.sidebar.selected, 1, "coming back keeps the selection");
    assert!(effects.is_empty());
}

#[test]
fn quit_asks_the_loop_to_stop() {
    let mut state = three();
    assert!(!state.should_quit);

    let effects = update(&mut state, Command::Quit);
    assert!(state.should_quit);
    assert!(effects.is_empty());
}

// ---------------------------------------------------------------------------
// Breadcrumbs
// ---------------------------------------------------------------------------

#[test]
fn breadcrumbs_come_from_the_route_and_the_schema() {
    let schema = example_schema();

    assert_eq!(breadcrumbs(&Route::Home, &schema), ["pressa"]);
    assert_eq!(
        breadcrumbs(
            &Route::List {
                collection: "posts".to_string()
            },
            &schema
        ),
        ["pressa", "Posts"]
    );
}

#[test]
fn breadcrumbs_print_a_slug_the_schema_does_not_know() {
    let schema = example_schema();
    let route = Route::List {
        collection: "drafts".to_string(),
    };

    // Visible rather than fatal: a stale route shows up on screen.
    assert_eq!(breadcrumbs(&route, &schema), ["pressa", "drafts"]);
    assert_eq!(collection_label(&schema, "drafts"), "drafts");
}

// ---------------------------------------------------------------------------
// The loop
// ---------------------------------------------------------------------------

#[test]
fn the_loop_draws_reads_and_exits_on_quit() {
    let mut state = three();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");

    // One scripted keypress, then a source that would fail if the loop asked
    // for a second one — proving it stopped rather than blocked.
    let mut pressed = false;
    let mut events = move || {
        if pressed {
            return Err(io::Error::other("the loop read past the quit"));
        }
        pressed = true;
        Ok(Event::Key(KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
        )))
    };

    pressa_tui::tui::drive(&mut state, &mut terminal, &mut events).expect("the loop exits cleanly");
    assert!(state.should_quit);
    // It drew before it read: the screen is not blank.
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.symbol() != " "),
        "the loop draws before it blocks on input"
    );
}

#[test]
fn a_key_bound_to_nothing_is_ignored() {
    let mut state = three();
    let before = state.clone();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");

    let mut keys = ['z', '?', 'q'].into_iter();
    let mut events = move || match keys.next() {
        Some(c) => Ok(Event::Key(KeyEvent::new(
            KeyCode::Char(c),
            KeyModifiers::NONE,
        ))),
        None => Err(io::Error::other("the loop read past the quit")),
    };

    pressa_tui::tui::drive(&mut state, &mut terminal, &mut events).expect("the loop exits cleanly");
    // `z` and `?` moved nothing; only `q` did anything, and it set the flag.
    assert_eq!(state.route, before.route);
    assert_eq!(state.sidebar, before.sidebar);
    assert_eq!(state.status, before.status);
    assert!(state.should_quit);
}

// ---------------------------------------------------------------------------
// Schema order
// ---------------------------------------------------------------------------

#[test]
fn the_sidebar_selection_walks_the_schemas_own_order() {
    let mut state = AppState::new(schema_from_yaml("unsorted", UNSORTED));

    update(&mut state, Command::Select);
    assert_eq!(
        state.route,
        Route::List {
            collection: "zebras".to_string()
        },
        "the first collection is the file's first, not the alphabet's"
    );
}

#[test]
fn the_loop_leaves_a_line_in_the_log_when_it_starts_and_when_it_exits() {
    // The one test in this binary that initialises logging: the `tracing`
    // subscriber is process-global and refuses a second call.
    let tree = support::TempTree::new("tui-log");
    pressa_tui::logging::init(tree.root(), pressa_tui::cli::LogLevel::Info)
        .expect("logging starts once per test binary");

    let mut state = three();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");
    let mut pressed = false;
    let mut events = move || {
        if pressed {
            return Err(io::Error::other("the loop read past the quit"));
        }
        pressed = true;
        Ok(Event::Key(KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
        )))
    };

    pressa_tui::tui::drive(&mut state, &mut terminal, &mut events).expect("the loop exits cleanly");

    let log = std::fs::read_to_string(pressa_tui::logging::log_path(tree.root()))
        .expect("the log file exists");
    assert!(log.contains("tui started"), "log was: {log}");
    assert!(log.contains("tui exited"), "log was: {log}");
}
