//! Every frame of [SPEC-007](../../specs/007-list-view.md), rendered into a
//! `TestBackend` and captured with `insta` (ADR-0011).
//!
//! The snapshots cover what the screen says; the things a text snapshot cannot
//! show — reverse video, dim, red — are asserted on the buffer's cell styles in
//! the same file. No test here opens a terminal, and none opens a database:
//! the records arrive through `Command::RecordsLoaded`, the way the loop
//! delivers them.

mod support;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

use pressa_app::domain::Schema;

use pressa_tui::counted;
use pressa_tui::tui::keymap::{Context, hints};
use pressa_tui::tui::{AppState, Command, update, view};

use support::{example_schema, records, schema_from_yaml, unvalidated_record};

/// The hint bar joins its entries with three spaces and leads with one
/// (SPEC-006 "Hints").
const HINT_SEPARATOR: &str = "   ";

/// The table's header row, its rule, and its first body row, at any size: the
/// main panel starts two rows down and the table fills it from the top.
const HEADER_ROW: u16 = 2;
const RULE_ROW: u16 = 3;
const FIRST_BODY_ROW: u16 = 4;

/// Frame D's collection: one column per cell renderer, and deliberately not
/// `posts` — the same function renders both (ADR-0004).
const FOUR_TYPES: &str = "\
project:
  name: blog-cms

collections:
  entries:
    list_columns: [title, featured, published_at, metadata]
    fields:
      - name: title
        type: text
        required: true
      - name: featured
        type: boolean
      - name: published_at
        type: datetime
      - name: metadata
        type: json
";

/// Frame I's collection: seven columns, more than a 60-column terminal fits.
const SEVEN_COLUMNS: &str = "\
project:
  name: blog-cms

collections:
  entries:
    list_columns: [title, status, content, views, featured, published_at, metadata]
    fields:
      - name: title
        type: text
        required: true
      - name: status
        type: text
      - name: content
        type: textarea
      - name: views
        type: number
      - name: featured
        type: boolean
      - name: published_at
        type: datetime
      - name: metadata
        type: json
";

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

/// Row `y`'s main panel, the sidebar and the divider dropped.
fn panel_row(buffer: &Buffer, y: u16) -> String {
    // `rsplit_once` because the divider is the last `│` before the panel and
    // the right border is the one after it.
    let line = inner_row(buffer, y);
    let (_, panel) = line.split_once('│').expect("the divider is drawn");
    panel.to_string()
}

/// `schema`'s `collection`, opened with `documents` loaded into it.
///
/// Through `Select` and `RecordsLoaded` rather than by assignment: the state a
/// frame is drawn from is one the loop can actually reach.
fn opened(schema: Schema, collection: &str, documents: &[&str]) -> AppState {
    let loaded = records(&schema, collection, documents);
    let mut state = AppState::new(schema);
    update(&mut state, Command::Select);
    update(&mut state, Command::RecordsLoaded(loaded));
    state
}

/// Frames B, C and H: three posts, the last with no `views` key.
fn three_posts() -> AppState {
    opened(
        example_schema(),
        "posts",
        &[
            r#"{"title":"Hello world","slug":"hello-world","status":"draft","views":0}"#,
            r#"{"title":"About page","slug":"about-page","status":"published","views":42}"#,
            r#"{"title":"Release notes","slug":"release-notes","status":"draft"}"#,
        ],
    )
}

/// Frames E and F: twenty posts, more than the fifteen body rows hold.
fn twenty_posts() -> AppState {
    let documents: Vec<String> = (1..=20)
        .map(|i| {
            let status = if i % 2 == 1 { "draft" } else { "published" };
            format!(
                r#"{{"title":"Post {i:02}","slug":"post-{i:02}","status":"{status}","views":{i}}}"#
            )
        })
        .collect();
    let borrowed: Vec<&str> = documents.iter().map(String::as_str).collect();
    opened(example_schema(), "posts", &borrowed)
}

// ---------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------

#[test]
fn frame_a_list_empty() {
    let state = opened(example_schema(), "posts", &[]);
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

#[test]
fn frame_b_three_records() {
    let state = three_posts();
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

#[test]
fn frame_c_after_two_move_downs() {
    let mut state = three_posts();
    // Two presses, not a hand-set index: the marker has to follow the command.
    update(&mut state, Command::MoveDown);
    update(&mut state, Command::MoveDown);
    assert_eq!(state.list.selected, 2);
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

#[test]
fn frame_d_cell_rendering() {
    let state = opened(
        schema_from_yaml("four-types", FOUR_TYPES),
        "entries",
        &[
            r#"{"title":"Release notes","featured":true,
                "published_at":"2026-09-06T12:00:00Z",
                "metadata":{"a":1,"b":2,"c":3}}"#,
            // None of the three set: a boolean has no "unset", so it is false.
            r#"{"title":"Draft idea","featured":false}"#,
        ],
    );
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

#[test]
fn frame_e_twenty_records_at_the_top() {
    let state = twenty_posts();
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

#[test]
fn frame_f_twenty_records_after_go_to_bottom() {
    let mut state = twenty_posts();
    update(&mut state, Command::GoToBottom);
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

#[test]
fn frame_g_load_failed() {
    let mut state = three_posts();
    // The records the load could not replace go away with it: the body must
    // say the load failed, not that the collection is empty.
    update(
        &mut state,
        Command::OperationFailed("database error: disk I/O error".to_string()),
    );
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

#[test]
fn frame_h_at_the_minimum_size() {
    let state = three_posts();
    insta::assert_snapshot!(draw(60, 16, &state).backend());
}

#[test]
fn frame_i_more_columns_than_fit() {
    let state = opened(
        schema_from_yaml("seven-columns", SEVEN_COLUMNS),
        "entries",
        &[r#"{"title":"Release notes","status":"published",
                "content":"Everything about the release","views":1024}"#],
    );
    insta::assert_snapshot!(draw(60, 16, &state).backend());
}

// ---------------------------------------------------------------------------
// The columns are the schema's, and only the schema's
// ---------------------------------------------------------------------------

#[test]
fn the_header_row_is_the_uppercased_labels_in_list_columns_order() {
    let state = three_posts();
    let terminal = draw(80, 24, &state);
    // The marker column is blank on the header row, so the panel's own two
    // leading spaces come before the first label (`docs/tui.md` §5.2).
    let header = panel_row(terminal.backend().buffer(), HEADER_ROW);
    assert!(header.starts_with("  TITLE"), "header was: {header}");

    let words: Vec<&str> = header.split_whitespace().collect();
    assert_eq!(words, ["TITLE", "STATUS", "VIEWS"]);
}

#[test]
fn no_column_is_appended_that_the_schema_did_not_name() {
    // `posts` has eight fields and three `list_columns`: the table shows three.
    let state = three_posts();
    let collection = state
        .schema
        .collections
        .get("posts")
        .expect("the example has posts");
    assert_eq!(collection.fields.len(), 8);

    let terminal = draw(80, 24, &state);
    let header = panel_row(terminal.backend().buffer(), HEADER_ROW);

    assert_eq!(
        header.split_whitespace().count(),
        collection.list_columns.len()
    );
    for absent in ["SLUG", "CONTENT", "FEATURED", "PUBLISHED AT", "METADATA"] {
        assert!(!header.contains(absent), "{absent} is not a list column");
    }
}

#[test]
fn the_widths_come_from_the_records_and_not_from_the_selection() {
    // Frames B and C differ in one character, the marker: the widths are a
    // function of the headers and the loaded records only.
    let mut state = three_posts();
    let before = draw(80, 24, &state);
    let before_header = row(before.backend().buffer(), HEADER_ROW);
    let before_rule = row(before.backend().buffer(), RULE_ROW);

    update(&mut state, Command::MoveDown);
    update(&mut state, Command::MoveDown);
    let after = draw(80, 24, &state);

    assert_eq!(row(after.backend().buffer(), HEADER_ROW), before_header);
    assert_eq!(row(after.backend().buffer(), RULE_ROW), before_rule);
}

#[test]
fn the_widths_do_change_when_the_records_do() {
    // The other half of the rule: a longer value widens its column.
    let narrow = opened(
        example_schema(),
        "posts",
        &[r#"{"title":"Hi","slug":"hi","status":"draft"}"#],
    );
    let wide = opened(
        example_schema(),
        "posts",
        &[r#"{"title":"An altogether considerably longer title","slug":"long","status":"draft"}"#],
    );

    assert_ne!(
        row(draw(80, 24, &narrow).backend().buffer(), HEADER_ROW),
        row(draw(80, 24, &wide).backend().buffer(), HEADER_ROW),
    );
}

#[test]
fn the_dropped_column_count_is_in_the_header_and_in_no_data_row() {
    let state = opened(
        schema_from_yaml("seven-columns", SEVEN_COLUMNS),
        "entries",
        &[r#"{"title":"Release notes","status":"published",
                "content":"Everything about the release","views":1024}"#],
    );
    let terminal = draw(60, 16, &state);
    let buffer = terminal.backend().buffer();

    assert!(
        row(buffer, HEADER_ROW).contains("+3"),
        "the header says how many columns did not fit: {}",
        row(buffer, HEADER_ROW)
    );
    // Every body row, so the marker cannot be hiding one.
    for y in FIRST_BODY_ROW..11 {
        assert!(
            !row(buffer, y).contains("+3"),
            "row {y} carries the dropped-column count: {}",
            row(buffer, y)
        );
    }
}

#[test]
fn a_cell_too_wide_for_its_column_ends_in_an_ellipsis() {
    let state = opened(
        schema_from_yaml("seven-columns", SEVEN_COLUMNS),
        "entries",
        &[r#"{"title":"Release notes","status":"published",
                "content":"Everything about the release","views":1024}"#],
    );
    let terminal = draw(60, 16, &state);
    let data = row(terminal.backend().buffer(), FIRST_BODY_ROW);

    // Six-character columns: five characters and the ellipsis that says there
    // was more. Counted in characters — `…` is three bytes and one column.
    assert!(data.contains("Relea…"), "row was: {data}");
    assert!(data.contains("publi…"), "row was: {data}");
}

// ---------------------------------------------------------------------------
// Bodies that are not a table
// ---------------------------------------------------------------------------

#[test]
fn an_empty_collection_says_so_under_the_rule() {
    let state = opened(example_schema(), "posts", &[]);
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();

    // A blank row, then the sentence, indented three characters — not centred.
    assert_eq!(panel_row(buffer, FIRST_BODY_ROW).trim(), "");
    let sentence = panel_row(buffer, FIRST_BODY_ROW + 1);
    assert_eq!(sentence, format!("{:<57}", "   No records yet."));

    // `n` is T9's: a screen may not name a key that does nothing (SPEC-006 Q4).
    assert!(!sentence.contains("Press n"));
}

#[test]
fn a_load_that_failed_offers_the_key_that_retries() {
    let mut state = three_posts();
    update(
        &mut state,
        Command::OperationFailed("database error: disk I/O error".to_string()),
    );

    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();

    let body = panel_row(buffer, FIRST_BODY_ROW + 1);
    assert_eq!(
        body,
        format!("{:<57}", "   Could not load records.  Press r to retry.")
    );

    // The error itself is the status line's job: one `AppError`, one place.
    assert!(row(buffer, 20).starts_with("│ ⚠ database error: disk I/O error"));
    // And the body does not claim the collection is empty.
    for y in FIRST_BODY_ROW..19 {
        assert!(!row(buffer, y).contains("No records yet"));
    }
}

#[test]
fn a_record_whose_data_is_not_an_object_still_draws_a_row() {
    let schema = example_schema();
    let mut state = AppState::new(schema);
    update(&mut state, Command::Select);
    // Storage stores what it is given, so a row like this can exist.
    update(
        &mut state,
        Command::RecordsLoaded(vec![unvalidated_record("posts", "\"not an object\"")]),
    );

    let terminal = draw(80, 24, &state);
    let data = row(terminal.backend().buffer(), FIRST_BODY_ROW);

    // Every column absent, so every column is a dash — and the row is drawn.
    assert_eq!(
        data.matches('—').count(),
        3,
        "every list column renders as a dash: {data}"
    );
}

#[test]
fn a_newline_in_a_cell_cannot_push_the_rule_off_the_screen() {
    let state = opened(
        example_schema(),
        "posts",
        &[r#"{"title":"first\nsecond","slug":"two-lines","status":"draft"}"#],
    );
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();

    // The rule is still on its own row, and the record is still on one row.
    assert!(row(buffer, RULE_ROW).contains("────"));
    let data = row(buffer, FIRST_BODY_ROW);
    assert!(data.contains("first second"), "row was: {data}");
    // The one record is on one row, so the row under it is empty.
    assert_eq!(panel_row(buffer, FIRST_BODY_ROW + 1).trim(), "");
}

// ---------------------------------------------------------------------------
// The title's record count
// ---------------------------------------------------------------------------

#[test]
fn the_title_counts_the_records_it_loaded() {
    for (n, expected) in [(0, "0 records"), (1, "1 record"), (3, "3 records")] {
        let documents: Vec<String> = (1..=n)
            .map(|i| format!(r#"{{"title":"P{i}","slug":"p{i}","status":"draft"}}"#))
            .collect();
        let borrowed: Vec<&str> = documents.iter().map(String::as_str).collect();
        let state = opened(example_schema(), "posts", &borrowed);

        let terminal = draw(80, 24, &state);
        let title = row(terminal.backend().buffer(), 0);

        // The phrase comes from `counted`, which the CLI shares.
        assert_eq!(expected, counted(n, "record"));
        assert!(title.contains(expected), "title was: {title}");
        // Three `─` from the corner.
        assert!(title.ends_with("───┐"), "title was: {title}");
    }
}

#[test]
fn a_failed_load_leaves_the_count_out_of_the_title() {
    let mut state = three_posts();
    update(
        &mut state,
        Command::OperationFailed("database error: disk I/O error".to_string()),
    );

    let title = row(draw(80, 24, &state).backend().buffer(), 0);
    assert!(
        !title.contains("record"),
        "the title must not report a count it does not have: {title}"
    );
}

#[test]
fn home_has_no_record_count() {
    let state = AppState::new(example_schema());
    let title = row(draw(80, 24, &state).backend().buffer(), 0);
    assert!(!title.contains("record"), "title was: {title}");
}

#[test]
fn a_title_too_long_to_leave_room_drops_the_count_and_not_the_breadcrumbs() {
    // A label long enough to reach the other end of a 60-column top edge.
    let schema = schema_from_yaml(
        "long-label",
        "\
project:
  name: blog-cms

collections:
  entries:
    label: An Extravagantly Long Collection Label Indeed
    list_columns: [title]
    fields:
      - name: title
        type: text
",
    );
    let state = opened(schema, "entries", &[r#"{"title":"Hi"}"#]);

    let title = row(draw(60, 16, &state).backend().buffer(), 0);
    assert!(
        title.contains("An Extravagantly Long"),
        "the breadcrumbs win: {title}"
    );
    assert!(!title.contains("1 record"), "the count gives way: {title}");
}

// ---------------------------------------------------------------------------
// Styles, which a text snapshot cannot show
// ---------------------------------------------------------------------------

/// Whether the table's columns area on row `y` is reverse video. Columns 22
/// onward: the sidebar and the divider are the first 22.
fn reversed(buffer: &Buffer, y: u16) -> bool {
    (22..buffer.area.right() - 1).all(|x| {
        buffer
            .cell((x, y))
            .is_some_and(|cell| cell.style().add_modifier.contains(Modifier::REVERSED))
    })
}

#[test]
fn the_selected_row_is_reverse_video_and_the_others_are_not() {
    let mut state = three_posts();
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();

    assert!(
        row(buffer, FIRST_BODY_ROW).contains('▸'),
        "the marker is text"
    );
    assert!(
        reversed(buffer, FIRST_BODY_ROW),
        "the selected row is a bar"
    );
    assert!(!reversed(buffer, FIRST_BODY_ROW + 1));
    assert!(!reversed(buffer, HEADER_ROW), "the header is not selected");

    // And it follows the selection rather than sitting on the first row.
    update(&mut state, Command::MoveDown);
    update(&mut state, Command::MoveDown);
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();
    assert!(!reversed(buffer, FIRST_BODY_ROW));
    assert!(reversed(buffer, FIRST_BODY_ROW + 2));
    assert!(row(buffer, FIRST_BODY_ROW + 2).contains('▸'));
}

#[test]
fn an_absent_value_is_a_dim_dash() {
    let state = three_posts();
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();

    // The third record has no `views`; find its dash and check the style.
    let y = FIRST_BODY_ROW + 2;
    let line = row(buffer, y);
    let x = line
        .chars()
        .position(|c| c == '—')
        .expect("the absent value is a dash") as u16;
    let dash = buffer.cell((x, y)).expect("the dash is on screen");
    assert!(dash.style().add_modifier.contains(Modifier::DIM));
}

#[test]
fn the_more_marker_is_dim_at_both_ends() {
    let mut state = twenty_posts();

    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();
    // The last body row stands in for everything below it.
    assert!(
        panel_row(buffer, 18).starts_with(" ↓ 6 more"),
        "row was: {}",
        panel_row(buffer, 18)
    );
    let arrow = buffer.cell((23, 18)).expect("the marker is on screen");
    assert!(arrow.style().add_modifier.contains(Modifier::DIM));

    update(&mut state, Command::GoToBottom);
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();
    assert!(
        panel_row(buffer, FIRST_BODY_ROW).starts_with(" ↑ 6 more"),
        "row was: {}",
        panel_row(buffer, FIRST_BODY_ROW)
    );
    let arrow = buffer
        .cell((23, FIRST_BODY_ROW))
        .expect("the marker is on screen");
    assert!(arrow.style().add_modifier.contains(Modifier::DIM));
}

#[test]
fn the_failed_loads_message_is_red_on_the_status_line() {
    let mut state = three_posts();
    update(
        &mut state,
        Command::OperationFailed("database error: disk I/O error".to_string()),
    );

    let terminal = draw(80, 24, &state);
    let warning = terminal
        .backend()
        .buffer()
        .cell((2, 20))
        .expect("the status line is on screen");
    assert_eq!(warning.style().fg, Some(Color::Red));
}

// ---------------------------------------------------------------------------
// The hint bar comes from the table
// ---------------------------------------------------------------------------

#[test]
fn every_frame_carries_the_keymaps_own_hints() {
    // No literal hint text in this file: the bar is computed from `KEYMAP`.
    let expected = format!(" {}", hints(Context::List).join(HINT_SEPARATOR));

    for (width, height, state) in [
        (80, 24, opened(example_schema(), "posts", &[])),
        (80, 24, three_posts()),
        (80, 24, twenty_posts()),
        (60, 16, three_posts()),
    ] {
        let terminal = draw(width, height, &state);
        let bar = inner_row(terminal.backend().buffer(), height - 2);
        assert_eq!(bar.trim_end(), expected.trim_end(), "at {width}x{height}");
    }
}
