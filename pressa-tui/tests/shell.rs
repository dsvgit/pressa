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
use pressa_tui::tui::{AppState, Command, Effect, Load, PAGE, Route, update};

use support::{example_schema, records, schema_from_yaml};

/// An effect runner for a test that triggers none. Panics rather than returning
/// nothing, so a test that silently grew an effect is a failure and not a pass.
fn no_effects(effects: Vec<Effect>) -> Vec<Command> {
    assert!(effects.is_empty(), "unexpected effects: {effects:?}");
    Vec::new()
}

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
        // Cloned because `Command` is no longer `Copy`.
        let first = update(&mut once, command.clone());
        let second = update(&mut twice, command);
        assert_eq!(
            first, second,
            "the same command asked for different effects"
        );
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
    assert_eq!(
        effects,
        vec![Effect::LoadRecords {
            collection: "posts".to_string()
        }],
        "opening a collection loads it"
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

    pressa_tui::tui::drive(&mut state, &mut terminal, &mut events, &mut no_effects)
        .expect("the loop exits cleanly");
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

    pressa_tui::tui::drive(&mut state, &mut terminal, &mut events, &mut no_effects)
        .expect("the loop exits cleanly");
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

    pressa_tui::tui::drive(&mut state, &mut terminal, &mut events, &mut no_effects)
        .expect("the loop exits cleanly");

    let log = std::fs::read_to_string(pressa_tui::logging::log_path(tree.root()))
        .expect("the log file exists");
    assert!(log.contains("tui started"), "log was: {log}");
    assert!(log.contains("tui exited"), "log was: {log}");
}

// ---------------------------------------------------------------------------
// The list: navigation (SPEC-007 "Navigation")
// ---------------------------------------------------------------------------

/// Two collections, both with records, so a test can watch one selection move
/// while the other stays put.
const TWO: &str = "\
project:
  name: blog-cms

collections:
  posts:
    list_columns: [title]
    fields:
      - name: title
        type: text
  authors:
    list_columns: [name]
    fields:
      - name: name
        type: text
";

/// `examples/blog`'s `posts`, opened, with `n` records loaded.
fn list_of(n: usize) -> AppState {
    let schema = example_schema();
    // Built through the service layer, so no test constructs a `Record`.
    let documents: Vec<String> = (1..=n)
        .map(|i| {
            format!(
                r#"{{"title":"Post {i:02}","slug":"post-{i:02}","status":"draft","views":{i}}}"#
            )
        })
        .collect();
    let borrowed: Vec<&str> = documents.iter().map(String::as_str).collect();
    let loaded = records(&schema, "posts", &borrowed);

    let mut state = AppState::new(schema);
    update(&mut state, Command::Select);
    update(&mut state, Command::RecordsLoaded(loaded));
    state
}

#[test]
fn moving_in_a_list_moves_the_list_and_not_the_sidebar() {
    let schema = schema_from_yaml("two", TWO);
    let loaded = records(
        &schema,
        "posts",
        &[r#"{"title":"one"}"#, r#"{"title":"two"}"#],
    );

    let mut state = AppState::new(schema);
    update(&mut state, Command::Select);
    update(&mut state, Command::RecordsLoaded(loaded));

    update(&mut state, Command::MoveDown);
    assert_eq!(state.list.selected, 1);
    assert_eq!(state.sidebar.selected, 0, "the sidebar stayed put");

    // And at Home it is the other way round.
    update(&mut state, Command::Back);
    update(&mut state, Command::MoveDown);
    assert_eq!(state.sidebar.selected, 1);
    assert_eq!(state.list.selected, 1, "the list's selection did not move");
}

#[test]
fn the_list_selection_is_clamped_at_both_ends() {
    let mut state = list_of(3);

    for _ in 0..2 {
        update(&mut state, Command::MoveDown);
    }
    assert_eq!(state.list.selected, 2);

    // Clamped, never wrapping, as the sidebar is.
    let before = state.clone();
    let effects = update(&mut state, Command::MoveDown);
    assert_eq!(state, before);
    assert!(effects.is_empty());

    update(&mut state, Command::GoToTop);
    let before = state.clone();
    let effects = update(&mut state, Command::MoveUp);
    assert_eq!(state, before);
    assert!(effects.is_empty());
}

#[test]
fn g_and_shift_g_go_to_the_first_and_last_record() {
    let mut state = list_of(20);

    update(&mut state, Command::GoToBottom);
    assert_eq!(state.list.selected, 19);

    update(&mut state, Command::GoToTop);
    assert_eq!(state.list.selected, 0);
}

#[test]
fn an_empty_collection_has_nowhere_to_go() {
    let mut state = list_of(0);

    // The case that would index an empty `Vec`.
    for command in [
        Command::GoToBottom,
        Command::PageDown,
        Command::MoveDown,
        Command::GoToTop,
        Command::PageUp,
        Command::MoveUp,
    ] {
        update(&mut state, command);
        assert_eq!(state.list.selected, 0);
    }
}

#[test]
fn a_page_is_ten_rows_and_clamps_at_both_ends() {
    assert_eq!(PAGE, 10, "SPEC-007 Q3 compiles the page size in");

    let mut state = list_of(20);

    update(&mut state, Command::PageDown);
    assert_eq!(state.list.selected, PAGE);

    // The second page would run off the end, so it stops on the last record.
    update(&mut state, Command::PageDown);
    assert_eq!(state.list.selected, 19);

    update(&mut state, Command::PageUp);
    assert_eq!(state.list.selected, 9);
    update(&mut state, Command::PageUp);
    assert_eq!(state.list.selected, 0, "clamped at the top");
}

// ---------------------------------------------------------------------------
// The list: loading and failure (SPEC-007 "Loading and failure")
// ---------------------------------------------------------------------------

#[test]
fn opening_a_collection_asks_for_its_records_and_empties_the_list() {
    let mut state = list_of(3);
    update(&mut state, Command::MoveDown);
    update(&mut state, Command::Back);

    let effects = update(&mut state, Command::Select);

    assert_eq!(
        effects,
        vec![Effect::LoadRecords {
            collection: "posts".to_string()
        }]
    );
    assert!(state.list.records.is_empty(), "the old records are gone");
    assert_eq!(state.list.selected, 0);
    assert_eq!(state.list.offset, 0);
}

#[test]
fn entering_a_collection_twice_reloads_it() {
    let mut state = list_of(3);

    // Back and in again: the records already in `AppState` are not reused.
    update(&mut state, Command::Back);
    let effects = update(&mut state, Command::Select);

    assert_eq!(
        effects,
        vec![Effect::LoadRecords {
            collection: "posts".to_string()
        }],
        "the second Select asks for the records again"
    );
}

#[test]
fn records_loaded_stores_them_and_clears_an_error() {
    let schema = example_schema();
    let loaded = records(
        &schema,
        "posts",
        &[
            r#"{"title":"Hello","slug":"hello","status":"draft"}"#,
            r#"{"title":"About","slug":"about","status":"draft"}"#,
            r#"{"title":"Notes","slug":"notes","status":"draft"}"#,
        ],
    );

    let mut state = AppState::new(schema);
    update(&mut state, Command::Select);
    update(
        &mut state,
        Command::OperationFailed("disk gone".to_string()),
    );
    assert_eq!(state.list.load, Load::Failed);

    // A selection past the end of what arrives, so the clamp is exercised.
    state.list.selected = 7;
    let effects = update(&mut state, Command::RecordsLoaded(loaded));

    assert_eq!(state.list.records.len(), 3);
    assert_eq!(state.list.load, Load::Ok);
    assert_eq!(state.list.selected, 2, "clamped into the new length");
    assert_eq!(state.status, None, "the error went away with the cause");
    assert!(effects.is_empty());
}

#[test]
fn an_operation_that_failed_clears_the_records_and_says_so() {
    let mut state = list_of(3);
    update(&mut state, Command::MoveDown);

    let effects = update(
        &mut state,
        Command::OperationFailed("database error: disk I/O error".to_string()),
    );

    assert_eq!(state.list.load, Load::Failed);
    // `Failed` with records still in it is the state the code must not produce.
    assert!(state.list.records.is_empty());
    assert_eq!(state.list.selected, 0);
    let status = state.status.expect("a failure says so on the status line");
    assert_eq!(status.text, "database error: disk I/O error");
    assert_eq!(status.kind, pressa_tui::tui::StatusKind::Error);
    assert!(effects.is_empty());
}

#[test]
fn failing_twice_leaves_one_message_and_not_a_growing_list() {
    let mut state = list_of(3);

    update(&mut state, Command::OperationFailed("first".to_string()));
    update(&mut state, Command::Refresh);
    update(&mut state, Command::OperationFailed("second".to_string()));

    let status = state.status.expect("still failing");
    assert_eq!(status.text, "second");
}

#[test]
fn refresh_reloads_a_list_and_does_nothing_at_home() {
    let mut state = list_of(3);

    assert_eq!(
        update(&mut state, Command::Refresh),
        vec![Effect::LoadRecords {
            collection: "posts".to_string()
        }]
    );

    update(&mut state, Command::Back);
    assert!(
        update(&mut state, Command::Refresh).is_empty(),
        "there is no list to reload at Home"
    );
}

#[test]
fn every_new_command_is_pure() {
    // The same input twice, from equal states: equal states and equal effects,
    // and no test here opens a repository.
    let schema = example_schema();
    let loaded = records(
        &schema,
        "posts",
        &[r#"{"title":"Hello","slug":"hello","status":"draft"}"#],
    );

    // One state, cloned: `list_of` would mint fresh record ids each call, and
    // two states differing by id would fail for the wrong reason.
    let base = list_of(3);

    for command in [
        Command::GoToTop,
        Command::GoToBottom,
        Command::PageUp,
        Command::PageDown,
        Command::Refresh,
        Command::RecordsLoaded(loaded),
        Command::OperationFailed("boom".to_string()),
    ] {
        let mut once = base.clone();
        let mut twice = base.clone();

        let first = update(&mut once, command.clone());
        let second = update(&mut twice, command);

        assert_eq!(first, second);
        assert_eq!(once, twice);
    }
}

// ---------------------------------------------------------------------------
// The loop drains its effects (SPEC-007 "The loop")
// ---------------------------------------------------------------------------

/// The scripted keys of a loop test, then a source that fails if the loop asks
/// for one more — which proves it stopped rather than blocked.
fn presses(keys: Vec<KeyCode>) -> impl FnMut() -> io::Result<Event> {
    let mut keys = keys.into_iter();
    move || match keys.next() {
        Some(code) => Ok(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))),
        None => Err(io::Error::other("the loop read past the quit")),
    }
}

/// `Enter` to open the selected collection, then `q` to come back out of the
/// loop. `q` is `Back` in a list, so it takes two.
fn open_then_quit() -> impl FnMut() -> io::Result<Event> {
    presses(vec![KeyCode::Enter, KeyCode::Char('q'), KeyCode::Char('q')])
}

#[test]
fn the_loop_runs_its_effects_and_feeds_the_answers_back_before_drawing() {
    let schema = example_schema();
    let loaded = records(
        &schema,
        "posts",
        &[r#"{"title":"Hello","slug":"hello","status":"draft"}"#],
    );

    let mut state = AppState::new(schema);
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");

    // `Enter` opens the collection, which asks for its records; the scripted
    // runner answers. No database and no terminal.
    let mut events = open_then_quit();
    let mut asked = Vec::new();
    let mut effects = |effects: Vec<Effect>| {
        asked.extend(effects);
        vec![Command::RecordsLoaded(loaded.clone())]
    };

    pressa_tui::tui::drive(&mut state, &mut terminal, &mut events, &mut effects)
        .expect("the loop exits cleanly");

    assert_eq!(
        asked,
        vec![Effect::LoadRecords {
            collection: "posts".to_string()
        }]
    );
    assert_eq!(state.list.records.len(), 1, "the answer reached the state");
    assert_eq!(state.list.load, Load::Ok);
}

#[test]
fn the_loop_drains_an_effect_that_leads_to_another() {
    let mut state = AppState::new(example_schema());
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");

    let mut events = open_then_quit();
    // The first effect is answered with `Refresh`, whose `update` returns a
    // second effect. A loop that ran one pass would leave it unserved.
    let mut rounds = 0;
    let mut effects = |_: Vec<Effect>| {
        rounds += 1;
        if rounds == 1 {
            vec![Command::Refresh]
        } else {
            vec![Command::RecordsLoaded(Vec::new())]
        }
    };

    pressa_tui::tui::drive(&mut state, &mut terminal, &mut events, &mut effects)
        .expect("the loop exits cleanly");

    assert_eq!(rounds, 2, "the queue drained rather than stopping at one");
}

#[test]
fn a_failing_effect_runner_cannot_end_the_loop() {
    let mut state = AppState::new(example_schema());
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");

    // `Enter`, then `j`, then twice `q`: the session has to survive the failure
    // in between, because `OperationFailed` is a command like any other.
    let mut events = presses(vec![
        KeyCode::Enter,
        KeyCode::Char('j'),
        KeyCode::Char('q'),
        KeyCode::Char('q'),
    ]);
    let mut effects = |_: Vec<Effect>| {
        vec![Command::OperationFailed(
            "database error: disk I/O error".to_string(),
        )]
    };

    pressa_tui::tui::drive(&mut state, &mut terminal, &mut events, &mut effects)
        .expect("a failed effect is not a failed loop");

    assert!(state.should_quit, "the loop ran to the quit");
    assert_eq!(state.list.load, Load::Failed);
}
