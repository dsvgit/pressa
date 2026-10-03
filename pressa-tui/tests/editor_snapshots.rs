//! Every frame of [SPEC-008](../../specs/008-record-editor.md), rendered into a
//! `TestBackend` and captured with `insta` (ADR-0011), plus what a text
//! snapshot cannot show — dim, red, reverse video — asserted on cell styles.
//!
//! No service is in scope when `view` runs: states are reached through
//! `update`, and records arrive through `support::records`, which creates them
//! through `RecordService` before the drawing starts.

mod support;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

use pressa_app::domain::{ErrorCode, FieldError, Schema};

use pressa_tui::tui::keymap::{context_for, hints};
use pressa_tui::tui::view::form::value_column;
use pressa_tui::tui::{AppState, Command, update, view};

use support::{example_schema, records, schema_from_yaml};

/// The hint bar joins its entries with three spaces (SPEC-006 "Hints").
const HINT_SEPARATOR: &str = "   ";

/// The main panel at 80x24: columns 22 to 78, rows 2 to 18.
const PANEL_X: u16 = 22;
const PANEL_WIDTH: u16 = 57;
const FIRST_ROW: u16 = 2;
const LAST_ROW: u16 = 18;
const STATUS_ROW: u16 = 20;

/// The record SPEC-000 step 8 stores: saved from a blank form, so every key is
/// there and `featured` is `false` rather than absent.
const HELLO: &str = r#"{"title":"Hello world","slug":"hello-world","status":"published",
    "content":null,"views":42,"featured":false,"published_at":null,"metadata":null}"#;

/// Frame H's collection: one field of each type, under a slug and field names
/// that are not `posts`' — only the labels are shared, so the frames line up.
const SEVEN_TYPES: &str = "\
project:
  name: blog-cms

collections:
  entries:
    list_columns: [headline]
    fields:
      - name: headline
        label: Title
        type: text
        required: true
      - name: state
        label: Status
        type: select
        required: true
        options: [draft, published]
      - name: hits
        label: Views
        type: number
      - name: pinned
        label: Featured
        type: boolean
      - name: released_at
        label: Published at
        type: datetime
      - name: extra
        label: Metadata
        type: json
      - name: notes
        label: Notes
        type: textarea
";

const RELEASE_NOTES: &str = r#"{"headline":"Release notes","state":"published","hits":1024,
    "pinned":true,"released_at":"2026-09-06T12:00:00Z",
    "extra":{"tags":["rust"],"pinned":true}}"#;

// ---------------------------------------------------------------------------
// Drawing helpers
// ---------------------------------------------------------------------------

fn draw(width: u16, height: u16, state: &AppState) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| view(state, frame))
        .expect("the test backend never fails to draw");
    terminal
}

/// Row `y` of the buffer as a string.
fn row(buffer: &Buffer, y: u16) -> String {
    (buffer.area.left()..buffer.area.right())
        .map(|x| buffer.cell((x, y)).map_or(" ", |cell| cell.symbol()))
        .collect()
}

/// Row `y` of the main panel at 80x24, by column and not by byte.
fn panel_row(buffer: &Buffer, y: u16) -> String {
    (PANEL_X..PANEL_X + PANEL_WIDTH)
        .map(|x| buffer.cell((x, y)).map_or(" ", |cell| cell.symbol()))
        .collect()
}

/// The panel column where `needle` starts on row `y`, counted in characters.
fn column_of(buffer: &Buffer, y: u16, needle: &str) -> Option<u16> {
    let line = panel_row(buffer, y);
    let byte = line.find(needle)?;
    // Characters before the match, so a multi-byte `▸` counts as one column.
    Some(PANEL_X + line[..byte].chars().count() as u16)
}

fn has(buffer: &Buffer, x: u16, y: u16, modifier: Modifier) -> bool {
    buffer
        .cell((x, y))
        .is_some_and(|cell| cell.style().add_modifier.contains(modifier))
}

// ---------------------------------------------------------------------------
// States, reached through the commands
// ---------------------------------------------------------------------------

/// `schema`'s `collection` opened as a list with `documents` loaded.
fn opened(schema: Schema, collection: &str, documents: &[&str]) -> AppState {
    let loaded = records(&schema, collection, documents);
    let mut state = AppState::new(schema);
    update(&mut state, Command::Select);
    update(&mut state, Command::RecordsLoaded(loaded));
    state
}

fn new_post() -> AppState {
    let mut state = opened(example_schema(), "posts", &[]);
    update(&mut state, Command::NewRecord);
    state
}

/// `Enter` on the first record of a list holding `documents`, loaded.
fn edit(schema: Schema, collection: &str, documents: &[&str]) -> AppState {
    let mut state = opened(schema, collection, documents);
    let record = state.list.records[0].clone();
    update(&mut state, Command::EditRecord);
    update(&mut state, Command::RecordLoaded(Box::new(record)));
    state
}

fn edit_post() -> AppState {
    edit(example_schema(), "posts", &[HELLO])
}

fn send(state: &mut AppState, commands: &[Command]) {
    for command in commands {
        update(state, command.clone());
    }
}

fn type_text(state: &mut AppState, text: &str) {
    for c in text.chars() {
        update(state, Command::InputChar(c));
    }
}

fn fill(state: &mut AppState, text: &str) {
    update(state, Command::BeginEdit);
    type_text(state, text);
    update(state, Command::CommitField);
}

/// Frame D: SPEC-000 step 7 on a blank post.
fn filled() -> AppState {
    let mut state = new_post();
    fill(&mut state, "Hello world");
    update(&mut state, Command::NextField);
    fill(&mut state, "hello-world");
    update(&mut state, Command::NextField);
    send(&mut state, &[Command::NextOption, Command::NextOption]);
    send(&mut state, &[Command::NextField, Command::NextField]);
    fill(&mut state, "42");
    state
}

fn required(field: &str) -> FieldError {
    FieldError {
        field: field.to_string(),
        code: ErrorCode::Required,
        message: "required".to_string(),
    }
}

/// Frame B: a blank save, refused for the three required fields.
fn refused() -> AppState {
    let mut state = new_post();
    update(
        &mut state,
        Command::SaveFailed(vec![
            required("title"),
            required("slug"),
            required("status"),
        ]),
    );
    state
}

/// Frame G: on the filled form, `Content` typed into over two lines.
fn typing_content() -> AppState {
    let mut state = filled();
    update(&mut state, Command::PrevField);
    update(&mut state, Command::BeginEdit);
    type_text(&mut state, "First line");
    update(&mut state, Command::InsertNewline);
    type_text(&mut state, "second line");
    state
}

/// Frame I: the stored post renamed, then `Esc`.
fn discarding() -> AppState {
    let mut state = edit_post();
    update(&mut state, Command::BeginEdit);
    send(
        &mut state,
        &vec![Command::InputBackspace; "Hello world".len()],
    );
    type_text(&mut state, "Hello, world!");
    update(&mut state, Command::CommitField);
    update(&mut state, Command::Back);
    state
}

// ---------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------

#[test]
fn frame_a_new_record_blank() {
    insta::assert_snapshot!(draw(80, 24, &new_post()).backend());
}

#[test]
fn frame_b_a_save_the_service_refused() {
    insta::assert_snapshot!(draw(80, 24, &refused()).backend());
}

#[test]
fn frame_c_typing_into_a_field() {
    let mut state = new_post();
    update(&mut state, Command::BeginEdit);
    type_text(&mut state, "Hello");
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

#[test]
fn frame_d_filled_dirty_focus_moved_on() {
    insta::assert_snapshot!(draw(80, 24, &filled()).backend());
}

#[test]
fn frame_e_editing_a_stored_record() {
    insta::assert_snapshot!(draw(80, 24, &edit_post()).backend());
}

#[test]
fn frame_f_a_save_that_failed_for_another_reason() {
    let mut state = filled();
    send(&mut state, &vec![Command::PrevField; 4]);
    update(
        &mut state,
        Command::OperationFailed("database error: disk I/O error".to_string()),
    );
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

#[test]
fn frame_g_a_focused_textarea_being_typed_into() {
    insta::assert_snapshot!(draw(80, 24, &typing_content()).backend());
}

#[test]
fn frame_h_all_seven_field_types_in_another_collection() {
    let state = edit(
        schema_from_yaml("seven-types", SEVEN_TYPES),
        "entries",
        &[RELEASE_NOTES],
    );
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

#[test]
fn frame_i_confirm_discard() {
    insta::assert_snapshot!(draw(80, 24, &discarding()).backend());
}

#[test]
fn frame_j_the_editor_at_the_minimum_size() {
    insta::assert_snapshot!(draw(60, 16, &new_post()).backend());
}

#[test]
fn frame_k_the_list_once_n_and_enter_are_bound() {
    let state = opened(example_schema(), "posts", &[]);
    insta::assert_snapshot!(draw(80, 24, &state).backend());
}

// ---------------------------------------------------------------------------
// The hint bar comes from the table
// ---------------------------------------------------------------------------

#[test]
fn every_frames_hint_bar_is_its_contexts_hints() {
    let mut typing = new_post();
    update(&mut typing, Command::BeginEdit);

    for (width, height, state) in [
        (80, 24, new_post()),
        (80, 24, refused()),
        (80, 24, typing),
        (80, 24, filled()),
        (80, 24, edit_post()),
        (80, 24, typing_content()),
        (80, 24, discarding()),
        (60, 16, new_post()),
        (80, 24, opened(example_schema(), "posts", &[])),
    ] {
        let terminal = draw(width, height, &state);
        let bar = row(terminal.backend().buffer(), height - 2);
        let expected = format!(" {}", hints(context_for(&state)).join(HINT_SEPARATOR));
        // Between the two walls, by column.
        let inner: String = bar.chars().skip(1).take(width as usize - 2).collect();
        assert_eq!(inner.trim_end(), expected.trim_end());
    }
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

#[test]
fn only_the_focused_label_carries_the_marker_and_it_follows_the_focus() {
    let state = new_post();
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();
    let markers: Vec<u16> = (FIRST_ROW..=LAST_ROW)
        .filter(|y| panel_row(buffer, *y).starts_with(" ▸"))
        .collect();
    assert_eq!(markers, [FIRST_ROW], "Title, and nothing else");

    let state = filled();
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();
    let markers: Vec<String> = (FIRST_ROW..=LAST_ROW)
        .map(|y| panel_row(buffer, y))
        .filter(|line| line.starts_with(" ▸"))
        .collect();
    assert_eq!(markers.len(), 1);
    assert!(markers[0].starts_with(" ▸Views"), "was {markers:?}");
}

#[test]
fn a_required_label_ends_in_a_star_and_an_optional_one_does_not() {
    let terminal = draw(80, 24, &new_post());
    let buffer = terminal.backend().buffer();
    let title = panel_row(buffer, FIRST_ROW);
    assert!(title.trim_end().ends_with("Title *"), "was {title}");
    let content = (FIRST_ROW..=LAST_ROW)
        .map(|y| panel_row(buffer, y))
        .find(|line| line.contains("Content"))
        .expect("Content is on screen");
    assert_eq!(content.trim(), "Content");
}

#[test]
fn a_focused_inline_widget_is_reverse_video_and_an_unfocused_one_is_not() {
    let mut state = new_post();
    state.editor.focus = 2; // Status
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();
    let status_row = (FIRST_ROW..=LAST_ROW)
        .find(|y| panel_row(buffer, *y).contains("Status"))
        .expect("Status is on screen");
    let x = column_of(buffer, status_row, "‹").expect("the cycler is drawn");
    assert!(has(buffer, x, status_row, Modifier::REVERSED));

    let featured_row = (FIRST_ROW..=LAST_ROW)
        .find(|y| panel_row(buffer, *y).contains("Featured"))
        .expect("Featured is on screen");
    let x = column_of(buffer, featured_row, "[ ]").expect("the toggle is drawn");
    assert!(!has(buffer, x, featured_row, Modifier::REVERSED));

    // And the other way round.
    state.editor.focus = 5;
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();
    let featured_row = (FIRST_ROW..=LAST_ROW)
        .find(|y| panel_row(buffer, *y).contains("Featured"))
        .expect("Featured is on screen");
    let x = column_of(buffer, featured_row, "[ ]").expect("the toggle is drawn");
    assert!(has(buffer, x, featured_row, Modifier::REVERSED));
    let status_row = (FIRST_ROW..=LAST_ROW)
        .find(|y| panel_row(buffer, *y).contains("Status"))
        .expect("Status is on screen");
    let x = column_of(buffer, status_row, "‹").expect("the cycler is drawn");
    assert!(!has(buffer, x, status_row, Modifier::REVERSED));
}

#[test]
fn inline_widgets_start_at_the_value_column() {
    let state = new_post();
    let column = value_column(&state.schema.collections["posts"]) as u16;
    assert_eq!(column, 18);
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();
    for label in ["Status", "Featured"] {
        let y = (FIRST_ROW..=LAST_ROW)
            .find(|y| panel_row(buffer, *y).contains(label))
            .expect("the label is on screen");
        let x = (PANEL_X..PANEL_X + PANEL_WIDTH)
            .find(|x| {
                buffer
                    .cell((*x, y))
                    .is_some_and(|c| c.symbol() == "‹" || c.symbol() == "[")
            })
            .expect("a widget is drawn");
        assert_eq!(x - PANEL_X, column, "{label}");
    }
}

#[test]
fn no_inline_widget_is_drawn_over_its_own_label() {
    let schema = schema_from_yaml(
        "long-label",
        "\
project:
  name: blog-cms

collections:
  entries:
    list_columns: [flag]
    fields:
      - name: flag
        label: A twenty char label!
        type: boolean
        required: true
      - name: pick
        label: Short
        type: select
        options: [one, two]
",
    );
    let mut state = opened(schema, "entries", &[]);
    update(&mut state, Command::NewRecord);
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();

    let line = panel_row(buffer, FIRST_ROW);
    assert!(
        line.contains("A twenty char label! *"),
        "the label is whole: {line}"
    );
    assert!(line.contains("[ ]"), "and the toggle is after it: {line}");
    let label_end = line.find(" *").expect("star") + 2;
    let widget = line.find("[ ]").expect("toggle");
    assert!(widget > label_end, "{line}");
}

#[test]
fn an_error_row_sits_under_its_field_and_is_red() {
    let terminal = draw(80, 24, &refused());
    let buffer = terminal.backend().buffer();

    // Title, focused: label, value bar, then the error.
    let error = panel_row(buffer, FIRST_ROW + 2);
    assert!(error.starts_with("  ⚠ required"), "was {error}");
    let x = column_of(buffer, FIRST_ROW + 2, "required").expect("the message");
    assert_eq!(
        buffer.cell((x, FIRST_ROW + 2)).and_then(|c| c.style().fg),
        Some(Color::Red)
    );

    // Slug: label, value, then the error — directly under the value row.
    let slug = (FIRST_ROW..=LAST_ROW)
        .find(|y| panel_row(buffer, *y).contains("Slug"))
        .expect("Slug");
    assert!(panel_row(buffer, slug + 2).starts_with("  ⚠ required"));

    // Status is inline: the error is the row after the label.
    let status = (FIRST_ROW..=LAST_ROW)
        .find(|y| panel_row(buffer, *y).contains("Status"))
        .expect("Status");
    assert!(panel_row(buffer, status + 1).starts_with("  ⚠ required"));
}

#[test]
fn three_errors_make_exactly_three_error_rows_and_none_make_none() {
    let count = |state: &AppState| {
        let terminal = draw(80, 24, state);
        let buffer = terminal.backend().buffer();
        (FIRST_ROW..=LAST_ROW)
            .filter(|y| panel_row(buffer, *y).contains('⚠'))
            .count()
    };
    assert_eq!(count(&refused()), 3);
    assert_eq!(count(&new_post()), 0);
}

#[test]
fn the_scroll_summaries_count_the_fields_no_row_of_which_is_drawn() {
    let last = |state: &AppState| {
        let terminal = draw(80, 24, state);
        panel_row(terminal.backend().buffer(), LAST_ROW)
    };
    // Right-aligned with a two-column gutter, overlaid on the row's own text.
    assert!(
        last(&new_post()).ends_with("↓ 1 more  "),
        "{}",
        last(&new_post())
    );
    assert!(
        last(&new_post()).starts_with("  Published at"),
        "overlaid, not replacing; cut off after its label"
    );
    assert!(last(&refused()).ends_with("↓ 3 more  "));
    assert!(last(&typing_content()).ends_with("↓ 3 more  "));
    assert!(
        last(&typing_content()).starts_with("    42"),
        "Views' value is drawn, so Views is not counted"
    );

    // Scrolled to the bottom, the first row says how many are above.
    let mut state = new_post();
    send(&mut state, &vec![Command::NextField; 7]);
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();
    let first = panel_row(buffer, FIRST_ROW);
    assert!(first.contains(" more  "), "was {first}");
    assert!(
        first.trim_end().starts_with("  "),
        "a label row, not a summary row"
    );
    let n: usize = first
        .split('↑')
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .expect("an up summary");
    // The first label drawn is field n: every field before it is hidden.
    let labels = [
        "Title",
        "Slug",
        "Status",
        "Content",
        "Views",
        "Featured",
        "Published at",
        "Metadata",
    ];
    assert!(
        first.contains(labels[n]),
        "first row {first} should be field {n}"
    );

    // Both summaries are dim.
    let x = column_of(buffer, FIRST_ROW, "↑").expect("arrow");
    assert!(has(buffer, x, FIRST_ROW, Modifier::DIM));
    let terminal = draw(80, 24, &new_post());
    let buffer = terminal.backend().buffer();
    let x = column_of(buffer, LAST_ROW, "↓").expect("arrow");
    assert!(has(buffer, x, LAST_ROW, Modifier::DIM));
}

#[test]
fn a_form_that_fits_carries_no_summary() {
    let schema = schema_from_yaml(
        "short-form",
        "\
project:
  name: blog-cms

collections:
  notes:
    fields:
      - name: body
        type: text
      - name: done
        type: boolean
",
    );
    let mut state = opened(schema, "notes", &[]);
    update(&mut state, Command::NewRecord);
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();
    for y in FIRST_ROW..=LAST_ROW {
        assert!(!panel_row(buffer, y).contains("more"), "row {y}");
    }
}

#[test]
fn the_unsaved_marker_is_in_the_title_only_while_dirty() {
    let title = |state: &AppState| row(draw(80, 24, state).backend().buffer(), 0);
    assert!(
        title(&filled()).contains(" ● unsaved · Ctrl+S "),
        "{}",
        title(&filled())
    );
    assert!(title(&filled()).ends_with("───┐"), "the list count's place");
    assert!(!title(&edit_post()).contains("unsaved"));
    assert!(!title(&new_post()).contains("unsaved"));
}

#[test]
fn the_attention_line_counts_the_errors_with_agreement() {
    let status = |state: &AppState| row(draw(80, 24, state).backend().buffer(), STATUS_ROW);
    assert!(status(&refused()).starts_with("│ 3 fields need attention"));

    let mut one = new_post();
    update(&mut one, Command::SaveFailed(vec![required("title")]));
    assert!(
        status(&one).starts_with("│ 1 field needs attention"),
        "{}",
        status(&one)
    );

    assert_eq!(
        status(&new_post()).trim_matches(|c| c == '│' || c == ' '),
        ""
    );
}

#[test]
fn the_cursor_is_drawn_exactly_while_typing() {
    let cursor = |state: &AppState| {
        let terminal = draw(80, 24, state);
        let buffer = terminal.backend().buffer();
        (FIRST_ROW..=LAST_ROW).any(|y| panel_row(buffer, y).contains('▌'))
    };
    let mut state = new_post();
    assert!(!cursor(&state), "focused, not typing: no cursor");
    update(&mut state, Command::BeginEdit);
    assert!(cursor(&state), "typing: the cursor");
    update(&mut state, Command::CancelEdit);
    assert!(!cursor(&state));
}

#[test]
fn an_absent_value_is_a_dim_dash_even_on_the_focus_bar() {
    let terminal = draw(80, 24, &new_post());
    let buffer = terminal.backend().buffer();
    // Title's value, on the focus bar: dim and reversed at once.
    let x = column_of(buffer, FIRST_ROW + 1, "—").expect("the dash");
    assert!(has(buffer, x, FIRST_ROW + 1, Modifier::DIM));
    assert!(has(buffer, x, FIRST_ROW + 1, Modifier::REVERSED));
    // Slug's unfocused value.
    let slug = (FIRST_ROW..=LAST_ROW)
        .find(|y| panel_row(buffer, *y).contains("Slug"))
        .expect("Slug");
    let x = column_of(buffer, slug + 1, "—").expect("the dash");
    assert!(has(buffer, x, slug + 1, Modifier::DIM));
    // The empty cycler's dash.
    let status = (FIRST_ROW..=LAST_ROW)
        .find(|y| panel_row(buffer, *y).contains("Status"))
        .expect("Status");
    let x = column_of(buffer, status, "—").expect("the dash");
    assert!(has(buffer, x, status, Modifier::DIM));
}

/// The rows `y` on which each label starts, by label, for one frame.
fn label_rows(state: &AppState) -> Vec<(String, u16)> {
    let labels = [
        "Title",
        "Slug",
        "Status",
        "Content",
        "Views",
        "Featured",
        "Published at",
        "Metadata",
    ];
    let terminal = draw(80, 24, state);
    let buffer = terminal.backend().buffer();
    labels
        .iter()
        .filter_map(|label| {
            // A label row starts with the two-column gutter and marker.
            (FIRST_ROW..=LAST_ROW)
                .find(|y| {
                    panel_row(buffer, *y)[..]
                        .trim_start_matches([' ', '▸'])
                        .starts_with(label)
                })
                .map(|y| (label.to_string(), y))
        })
        .collect()
}

#[test]
fn moving_the_focus_moves_no_other_field() {
    // Q16: focus is a marker and a bar, not a box, so no block changes height
    // and every label stays on its row while the focus walks down the form —
    // as far as it can go before the form has to scroll.
    let mut state = new_post();
    let before = label_rows(&state);
    for _ in 0..5 {
        update(&mut state, Command::NextField);
        assert_eq!(label_rows(&state), before, "focus {}", state.editor.focus);
    }
}

#[test]
fn the_focused_value_is_a_bar_and_an_unfocused_one_is_not() {
    let terminal = draw(80, 24, &filled());
    let buffer = terminal.backend().buffer();
    let views = (FIRST_ROW..=LAST_ROW)
        .find(|y| panel_row(buffer, *y).starts_with(" ▸Views"))
        .expect("Views is focused");
    // From the bar's margin to the gutter, every cell is reversed.
    for x in PANEL_X + 2..PANEL_X + PANEL_WIDTH - 2 {
        assert!(has(buffer, x, views + 1, Modifier::REVERSED), "column {x}");
    }
    // The gutters either side are not.
    assert!(!has(buffer, PANEL_X + 1, views + 1, Modifier::REVERSED));
    assert!(!has(
        buffer,
        PANEL_X + PANEL_WIDTH - 1,
        views + 1,
        Modifier::REVERSED
    ));
    // An unfocused value — Slug's — is not a bar.
    let slug = (FIRST_ROW..=LAST_ROW)
        .find(|y| panel_row(buffer, *y).contains("Slug"))
        .expect("Slug");
    assert!(!has(buffer, PANEL_X + 4, slug + 1, Modifier::REVERSED));
}

#[test]
fn a_textarea_and_json_grow_to_five_rows_only_while_typed_into() {
    // The value rows under a label: reversed rows, counted.
    let bar_rows = |state: &AppState| {
        let terminal = draw(80, 24, state);
        let buffer = terminal.backend().buffer();
        (FIRST_ROW..=LAST_ROW)
            .filter(|y| has(buffer, PANEL_X + 3, *y, Modifier::REVERSED))
            .count()
    };
    let mut state = new_post();
    for (focus, typing) in [(0, 1), (1, 1), (3, 5), (4, 1), (6, 1), (7, 5)] {
        state.editor.focus = focus;
        state.editor.input = None;
        assert_eq!(bar_rows(&state), 1, "focus {focus}, not typing");
        update(&mut state, Command::BeginEdit);
        assert_eq!(bar_rows(&state), typing, "focus {focus}, typing");
    }
}

#[test]
fn a_value_wider_than_its_row_shows_its_tail() {
    let mut state = new_post();
    update(&mut state, Command::BeginEdit);
    // Sixty characters, some of them more than one byte.
    let text: String = (0..60)
        .map(|i| if i % 7 == 0 { 'é' } else { 'a' })
        .collect::<String>()
        + "END";
    type_text(&mut state, &text);
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();
    let line = panel_row(buffer, FIRST_ROW + 1);
    // `…` in the value's first column, then the tail ending in the cursor,
    // then the gutter — nothing written past it.
    assert!(line.starts_with("    …"), "was {line}");
    assert!(
        line.ends_with("END▌  "),
        "the tail, up to the gutter: {line}"
    );
    assert_eq!(line.chars().count(), PANEL_WIDTH as usize);
}

#[test]
fn display_values_are_the_editable_text_on_screen() {
    let state = edit(
        schema_from_yaml("seven-types", SEVEN_TYPES),
        "entries",
        &[RELEASE_NOTES],
    );
    let terminal = draw(80, 24, &state);
    let buffer = terminal.backend().buffer();
    let screen: String = (FIRST_ROW..=LAST_ROW)
        .map(|y| panel_row(buffer, y))
        .collect();
    assert!(
        screen.contains("2026-09-06T12:00:00Z"),
        "not the table's minutes"
    );
    assert!(
        screen.contains(r#""tags":["rust"]"#),
        "the JSON text, not {{2}}"
    );
    assert!(screen.contains("[x]"));
    assert!(screen.contains("‹ published ›"));
}

#[test]
fn the_discard_dialog_is_centred_over_the_drawn_panel() {
    let terminal = draw(80, 24, &discarding());
    let buffer = terminal.backend().buffer();
    // The panel is still drawn behind it: its first row is the Title label.
    assert!(panel_row(buffer, FIRST_ROW).starts_with(" ▸Title *"));
    // Seven rows tall in seventeen: five above, five below.
    let top = panel_row(buffer, FIRST_ROW + 5);
    assert!(top.contains("┌─ Discard changes "), "was {top}");
    let bottom = panel_row(buffer, FIRST_ROW + 11);
    assert!(bottom.contains('└'), "was {bottom}");
    // Centred: as many columns either side, give or take one.
    let left = top.chars().take_while(|c| *c == ' ').count();
    let right = top.chars().rev().take_while(|c| *c == ' ').count();
    assert!(left.abs_diff(right) <= 1, "{left} and {right}: {top}");
    // Its key line is the keymap's.
    let keys = panel_row(buffer, FIRST_ROW + 10);
    for hint in hints(context_for(&discarding())) {
        assert!(keys.contains(&hint), "{hint} missing from {keys}");
    }
}

#[test]
fn every_frame_keeps_the_panel_inside_its_borders() {
    let seven = edit(
        schema_from_yaml("seven-types", SEVEN_TYPES),
        "entries",
        &[RELEASE_NOTES],
    );
    for state in [
        new_post(),
        refused(),
        filled(),
        typing_content(),
        discarding(),
        seven,
    ] {
        for (width, height) in [(80, 24), (60, 16), (100, 30)] {
            let terminal = draw(width, height, &state);
            let buffer = terminal.backend().buffer();
            for y in 2..height - 5 {
                assert_eq!(
                    buffer.cell((width - 1, y)).map(|c| c.symbol()),
                    Some("│"),
                    "{width}x{height} row {y}: {}",
                    row(buffer, y)
                );
            }
        }
    }
}
