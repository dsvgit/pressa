//! Every frame of [SPEC-006](../../specs/006-tui-shell.md), rendered into a
//! `TestBackend` and captured with `insta` (ADR-0011).
//!
//! Snapshots cover what the screen says; the things a text snapshot cannot
//! show — reverse video, red, dim — are asserted on the buffer's cell styles
//! in the same file.

mod support;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

use pressa_app::domain::Schema;

use pressa_tui::tui::keymap::{Context, hints};
use pressa_tui::tui::{AppState, Command, Route, StatusKind, StatusMessage, update, view};

use support::{example_schema, schema_from_yaml};

/// The hint bar joins its entries with three spaces and leads with one
/// (SPEC-006 "Hints").
const HINT_SEPARATOR: &str = "   ";

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

/// A label of 26 characters, well past the sidebar's 17 usable columns.
const LONG_LABEL: &str = "\
project:
  name: long

collections:
  posts:
    label: Supercalifragilistic Posts
    fields:
      - name: title
        type: text
";

/// `many-collections` with twenty collections, `Coll 01` … `Coll 20`.
fn twenty() -> Schema {
    let mut yaml = String::from("project:\n  name: many-collections\n\ncollections:\n");
    for n in 1..=20 {
        // `c01` … `c20`: a slug must match `^[a-z][a-z0-9_]*$`.
        yaml.push_str(&format!(
            "  c{n:02}:\n    label: Coll {n:02}\n    fields:\n      - name: title\n        type: text\n"
        ));
    }
    schema_from_yaml("twenty", &yaml)
}

fn draw(width: u16, height: u16, state: &AppState) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| view(state, frame))
        .expect("the test backend never fails to draw");
    terminal
}

/// Row `y` of the buffer as a string, so a test can read one line of a frame.
fn row(buffer: &Buffer, y: u16) -> String {
    (buffer.area.left()..buffer.area.right())
        // `cell` returns `None` off the buffer, which cannot happen here.
        .map(|x| buffer.cell((x, y)).map_or(" ", |cell| cell.symbol()))
        .collect()
}

/// Row `y` between the two border columns. By column, not by byte: `│` is
/// three bytes wide and one column wide.
fn inner_row(buffer: &Buffer, y: u16) -> String {
    let width = buffer.area.width as usize;
    row(buffer, y).chars().skip(1).take(width - 2).collect()
}

// ---------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------

#[test]
fn frame_a_home() {
    let state = AppState::new(example_schema());
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

#[test]
fn frame_b_home_with_three_collections() {
    let state = AppState::new(schema_from_yaml("three", THREE));
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

#[test]
fn frame_c_home_after_two_move_downs() {
    let mut state = AppState::new(schema_from_yaml("three", THREE));
    update(&mut state, Command::MoveDown);
    update(&mut state, Command::MoveDown);
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

#[test]
fn frame_d_home_with_an_error_status() {
    let mut state = AppState::new(example_schema());
    state.status = Some(StatusMessage {
        text: "unknown collection: drafts".to_string(),
        kind: StatusKind::Error,
    });
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

#[test]
fn frame_e_home_at_the_minimum_size() {
    let state = AppState::new(example_schema());
    insta::assert_snapshot!(draw(60, 16, &state).backend());
}

#[test]
fn frame_f_sidebar_scrolled_to_the_last_of_twenty() {
    let mut state = AppState::new(twenty());
    // Nineteen presses, not a hand-set index: the viewport has to follow.
    for _ in 0..19 {
        update(&mut state, Command::MoveDown);
    }
    assert_eq!(state.sidebar.selected, 19);
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

#[test]
fn frame_g_terminal_too_small() {
    let state = AppState::new(example_schema());
    insta::assert_snapshot!("frame_g_at_40x10", draw(40, 10, &state).backend());
    insta::assert_snapshot!("frame_g_at_80x15", draw(80, 15, &state).backend());
}

#[test]
fn frame_h_list_route() {
    let mut state = AppState::new(example_schema());
    state.route = Route::List {
        collection: "posts".to_string(),
    };
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

// ---------------------------------------------------------------------------
// The too-small boundary
// ---------------------------------------------------------------------------

#[test]
fn the_too_small_screen_starts_one_column_and_one_row_short() {
    let state = AppState::new(example_schema());

    // One short on each axis. SPEC-006's criterion says 79x24 for the width
    // case, which cannot be right: its own frame E draws the whole layout at
    // 60 columns, so the column that breaks the floor is 59, not 79.
    for (width, height) in [(59, 24), (80, 15)] {
        let terminal = draw(width, height, &state);
        assert!(
            row(terminal.backend().buffer(), height / 2 - 1).contains("terminal too small"),
            "{width}x{height} is below the floor on one axis"
        );
    }

    // The floor itself draws the layout: a border, not a message. Asserted on
    // both axes, so neither `<` is quietly a `<=`.
    for (width, height) in [(60, 16), (80, 16)] {
        let terminal = draw(width, height, &state);
        assert!(
            row(terminal.backend().buffer(), 0).starts_with("┌ pressa "),
            "{width}x{height} is the floor, not below it"
        );
    }
}

#[test]
fn the_too_small_screen_names_the_size_it_has() {
    let state = AppState::new(example_schema());
    let terminal = draw(40, 10, &state);

    assert_eq!(
        row(terminal.backend().buffer(), 5).trim(),
        "needs 60x16, this is 40x10"
    );
}

// ---------------------------------------------------------------------------
// Styles, which a text snapshot cannot show
// ---------------------------------------------------------------------------

/// The sidebar's first list row: header, blank, then the collections.
const FIRST_LIST_ROW: u16 = 4;

#[test]
fn the_selected_row_is_reverse_video_only_while_the_sidebar_has_focus() {
    let mut state = AppState::new(example_schema());

    let home = draw(80, 24, &state);
    let marker = row(home.backend().buffer(), FIRST_LIST_ROW);
    assert!(marker.starts_with("│ > Posts"), "row was: {marker}");
    assert!(
        reversed(home.backend().buffer(), FIRST_LIST_ROW),
        "the sidebar has focus at Home"
    );

    state.route = Route::List {
        collection: "posts".to_string(),
    };
    let list = draw(80, 24, &state);
    let marker = row(list.backend().buffer(), FIRST_LIST_ROW);
    assert!(
        marker.starts_with("│ > Posts"),
        "the marker stays: {marker}"
    );
    assert!(
        !reversed(list.backend().buffer(), FIRST_LIST_ROW),
        "the sidebar has lost focus"
    );
}

/// Whether the sidebar's twenty columns on row `y` are reverse video.
fn reversed(buffer: &Buffer, y: u16) -> bool {
    (1..21).all(|x| {
        buffer
            .cell((x, y))
            .is_some_and(|cell| cell.style().add_modifier.contains(Modifier::REVERSED))
    })
}

#[test]
fn an_error_status_is_red_and_warned_and_an_info_status_is_neither() {
    let mut state = AppState::new(example_schema());
    state.status = Some(StatusMessage {
        text: "unknown collection: drafts".to_string(),
        kind: StatusKind::Error,
    });

    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();
    assert!(row(buffer, 20).starts_with("│ ⚠ unknown collection: drafts"));
    let warning = buffer.cell((2, 20)).expect("the status line is on screen");
    assert_eq!(warning.style().fg, Some(Color::Red));

    state.status = Some(StatusMessage {
        text: "unknown collection: drafts".to_string(),
        kind: StatusKind::Info,
    });
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();
    assert!(row(buffer, 20).starts_with("│ unknown collection: drafts"));
    let first = buffer.cell((2, 20)).expect("the status line is on screen");
    assert_ne!(first.style().fg, Some(Color::Red));
}

#[test]
fn the_more_marker_is_dim() {
    let mut state = AppState::new(twenty());
    for _ in 0..19 {
        update(&mut state, Command::MoveDown);
    }

    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();
    assert!(row(buffer, FIRST_LIST_ROW).starts_with("│ ↑ 6 more"));
    let arrow = buffer
        .cell((2, FIRST_LIST_ROW))
        .expect("the marker is on screen");
    assert!(arrow.style().add_modifier.contains(Modifier::DIM));
}

// ---------------------------------------------------------------------------
// The sidebar's own rules
// ---------------------------------------------------------------------------

#[test]
fn a_label_wider_than_the_sidebar_is_truncated_and_never_reaches_the_divider() {
    let state = AppState::new(schema_from_yaml("long", LONG_LABEL));
    let terminal = draw(80, 24, &state);
    let line = row(terminal.backend().buffer(), FIRST_LIST_ROW);

    assert!(line.starts_with("│ > Supercalifragili…"), "row was: {line}");
    // Column 21 is the divider; the label must not have written over it.
    assert_eq!(
        terminal
            .backend()
            .buffer()
            .cell((21, FIRST_LIST_ROW))
            .map(|cell| cell.symbol().to_string()),
        Some("│".to_string())
    );
}

#[test]
fn the_sidebar_lists_collections_in_the_schemas_order() {
    let state = AppState::new(schema_from_yaml(
        "unsorted",
        "\
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
",
    ));
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();

    assert!(row(buffer, FIRST_LIST_ROW).starts_with("│ > Zebras"));
    assert!(row(buffer, FIRST_LIST_ROW + 1).starts_with("│   Apples"));
}

// ---------------------------------------------------------------------------
// The hint bar comes from the table
// ---------------------------------------------------------------------------

#[test]
fn the_hint_bar_is_the_keymap_for_the_current_context() {
    let mut state = AppState::new(example_schema());

    let home = draw(80, 24, &state);
    assert_eq!(
        inner_row(home.backend().buffer(), 22).trim_end(),
        format!(" {}", hints(Context::Sidebar).join(HINT_SEPARATOR)).trim_end()
    );

    state.route = Route::List {
        collection: "posts".to_string(),
    };
    let list = draw(80, 24, &state);
    assert_eq!(
        inner_row(list.backend().buffer(), 22).trim_end(),
        format!(" {}", hints(Context::List).join(HINT_SEPARATOR)).trim_end()
    );
}
