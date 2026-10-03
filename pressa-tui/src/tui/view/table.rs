//! The one table. Takes a `&Collection` and renders whatever it describes:
//! no branch on a collection slug, ever
//! ([ADR-0004](../../../../docs/adr/0004-schema-driven-ui.md)).
//!
//! The layout arithmetic is SPEC-007 "The table, stated once", and the per-type
//! cells are its frame D. [`columns`] and [`cell`] are pure and unit-tested
//! below; [`render`] is what the snapshots cover.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use pressa_app::domain::{Collection, Field, FieldType, Json};

use crate::tui::state::{AppState, Load};
use crate::tui::view::{truncate, window};

/// No column is ever narrower than this.
const MIN_WIDTH: usize = 6;
/// Two spaces between columns.
const GAP: usize = 2;
/// Header, then the rule: the rows the body does not get.
const HEADER_ROWS: u16 = 2;
/// A left gutter, the selection marker, and a right gutter.
const CHROME_COLUMNS: u16 = 3;

/// An absent value, a true boolean, a false one, and the selection marker.
const ABSENT: &str = "—";
const YES: &str = "✓";
const NO: &str = "·";
const MARKER: &str = "▸";

/// The two bodies that are not a table.
const EMPTY: &str = "No records yet.";
const FAILED: &str = "Could not load records.  Press r to retry.";
/// How far both of them are indented from the panel's left edge.
const BODY_INDENT: u16 = 3;

/// One cell's text and whether it is dim. Pure, and the whole of ADR-0004's
/// "render in a table cell" question for every field type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub dim: bool,
}

/// How one value reads in a table cell.
///
/// Branches on the field type and on the JSON value, never on a field name or
/// a collection slug. The last arm is what lets the table draw data storage
/// would not have accepted: storage does not validate (SPEC-003), so a value
/// of the wrong type is shown rather than refused.
pub fn cell(kind: &FieldType, value: Option<&Json>) -> Cell {
    // Absent and null are one case: both mean the record has no value here.
    // `filter` on the `Option` collapses them before the match below.
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Cell {
            text: ABSENT.to_string(),
            dim: true,
        };
    };

    match (kind, value) {
        // One arm for the three types whose stored form is a string.
        (FieldType::Text | FieldType::Textarea | FieldType::Select { .. }, Json::String(text)) => {
            plain(collapse(text))
        }
        // The JSON number's own text: `42`, not `42.0`.
        (FieldType::Number, Json::Number(number)) => plain(number.to_string()),
        (FieldType::Boolean, Json::Bool(yes)) => plain(if *yes { YES } else { NO }.to_string()),
        // Not parsed and not converted — see SPEC-007, Q1. A string that is not
        // shaped like one falls through to the invalid-value arm.
        (FieldType::DateTime, Json::String(text)) if is_rfc3339_shaped(text) => {
            plain(to_minutes(text))
        }
        // A `Json` field accepts anything that parsed, so none of these is dim.
        (FieldType::Json, Json::Object(map)) => plain(format!("{{{}}}", map.len())),
        (FieldType::Json, Json::Array(items)) => plain(format!("[{}]", items.len())),
        (FieldType::Json, scalar) => plain(collapse(&scalar.to_string())),
        // Anything the field's type does not allow: its JSON text, dim.
        (_, other) => Cell {
            text: collapse(&other.to_string()),
            dim: true,
        },
    }
}

/// The width of each visible column, and how many did not fit, from the natural
/// widths and the space available. Pure arithmetic, no `Frame`.
pub fn columns(natural: &[usize], area: usize) -> (Vec<usize>, usize) {
    // `let ... else` because an area too small for even one column is a total
    // answer — no columns — rather than an error.
    let Some(visible) = fitting(natural.len(), area) else {
        return (Vec::new(), natural.len());
    };

    let dropped = natural.len() - visible;
    let space = area - GAP * (visible - 1) - reserve(dropped);
    // Only the columns that fit take part in the division.
    let shown = &natural[..visible];
    // `max(1)` guards the division; a natural width is never 0 in practice.
    let total = shown.iter().sum::<usize>().max(1);

    // Proportional and floored, which is what makes frame B's 24 : 16 : 10.
    let mut widths: Vec<usize> = shown.iter().map(|width| space * width / total).collect();

    // The remainder goes out one character at a time from the left.
    let mut left = space.saturating_sub(widths.iter().sum::<usize>());
    while left > 0 {
        for width in widths.iter_mut() {
            if left == 0 {
                break;
            }
            *width += 1;
            left -= 1;
        }
    }

    raise_to_floor(&mut widths);
    (widths, dropped)
}

/// The header row, the rule, and one row per visible record.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, collection: &Collection) {
    // Too small to hold a header and a rule: draw nothing rather than
    // something broken. The layout floor makes this unreachable.
    if area.height < HEADER_ROWS || area.width <= CHROME_COLUMNS {
        return;
    }

    let fields = list_columns(collection);
    // Every loaded record's cells, computed once: the natural widths are over
    // all of them, so scrolling cannot move the columns.
    let rows: Vec<Vec<Cell>> = state
        .list
        .records
        .iter()
        .map(|record| cells_of(&fields, &record.data))
        .collect();

    let columns_area = (area.width - CHROME_COLUMNS) as usize;
    let (widths, dropped) = columns(&natural_widths(&fields, &rows), columns_area);
    let layout = Layout {
        area,
        widths: &widths,
        columns_area,
    };

    header(frame, &layout, &fields, dropped);
    rule(frame, area);

    // An empty body says *why* it is empty rather than leaving a blank panel.
    if rows.is_empty() {
        notice(frame, area, state.list.load);
    } else {
        body(frame, &layout, state, &rows);
    }
}

// ---------------------------------------------------------------------------
// Cells
// ---------------------------------------------------------------------------

/// A cell that reads as itself.
fn plain(text: String) -> Cell {
    Cell { text, dim: false }
}

/// `text` with every run of control characters — a newline included — as one
/// space, so one record is always one row.
fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    // Tracks whether the previous character was part of a run, so a run of any
    // length collapses to a single space.
    let mut in_run = false;
    for character in text.chars() {
        if character.is_control() {
            if !in_run {
                out.push(' ');
            }
            in_run = true;
        } else {
            out.push(character);
            in_run = false;
        }
    }
    out
}

/// Whether `text` starts with the sixteen characters of an RFC 3339 timestamp.
///
/// A shape test and not a parse: `view` may read no clock and `pressa-tui` may
/// name no date library (SPEC-007, Q1).
fn is_rfc3339_shaped(text: &str) -> bool {
    // On bytes because every character of the shape is ASCII; a multi-byte
    // character lands in a digit slot and fails the test below. The `..` makes
    // the pattern refutable on length, which is what `else` answers.
    let [
        y0,
        y1,
        y2,
        y3,
        b'-',
        m0,
        m1,
        b'-',
        d0,
        d1,
        b'T',
        h0,
        h1,
        b':',
        n0,
        n1,
        ..,
    ] = text.as_bytes()
    else {
        return false;
    };

    [y0, y1, y2, y3, m0, m1, d0, d1, h0, h1, n0, n1]
        .iter()
        .all(|byte| byte.is_ascii_digit())
}

/// The first sixteen characters, with the `T` as a space: `2026-09-06 12:00`.
fn to_minutes(text: &str) -> String {
    text.chars()
        .take(16)
        // By characters so the count is columns and not bytes.
        .map(|character| if character == 'T' { ' ' } else { character })
        .collect()
}

// ---------------------------------------------------------------------------
// Widths
// ---------------------------------------------------------------------------

/// How many digits `k` is written in, so the `+k` can be measured.
fn digits(k: usize) -> usize {
    k.to_string().chars().count()
}

/// The columns held back on the right for the `+k` that says how many columns
/// did not fit: the gap before it, the `+`, and the number.
fn reserve(dropped: usize) -> usize {
    if dropped == 0 {
        0
    } else {
        GAP + 1 + digits(dropped)
    }
}

/// How many of `total` columns can be shown in `area`.
///
/// The widest count that still leaves every shown column six characters, the
/// gaps between them, and room for the `+k`. Trailing columns go first.
fn fitting(total: usize, area: usize) -> Option<usize> {
    // `rev` so the first count that fits is the largest one.
    (1..=total).rev().find(|visible| {
        visible * MIN_WIDTH + GAP * (visible - 1) + reserve(total - visible) <= area
    })
}

/// Moves characters from the widest column to any column under the floor.
///
/// Proportional division alone can miss the floor when one natural width is
/// tiny beside a very wide one (`[6, 100]`). [`fitting`] guarantees the area
/// holds `MIN_WIDTH` for every shown column, so this always converges — and it
/// preserves the total, so the widths still fill the area exactly.
fn raise_to_floor(widths: &mut [usize]) {
    loop {
        let thin = widths.iter().position(|width| *width < MIN_WIDTH);
        // `max_by_key` on the last of equal maxima; any of them will do.
        let wide = widths
            .iter()
            .enumerate()
            .max_by_key(|(_, width)| **width)
            .map(|(index, _)| index);

        // Nothing under the floor, or nothing above it to take from.
        let (Some(thin), Some(wide)) = (thin, wide) else {
            return;
        };
        if thin == wide || widths.get(wide).is_none_or(|width| *width <= MIN_WIDTH) {
            return;
        }

        // `get_mut` twice rather than indexing: a stored index goes through
        // `get` (`AGENTS.md`), and the two writes cannot be held at once.
        if let Some(width) = widths.get_mut(thin) {
            *width += 1;
        }
        if let Some(width) = widths.get_mut(wide) {
            *width -= 1;
        }
    }
}

/// The fields `list_columns` names, in that order.
///
/// `filter_map` drops a name the collection has no field for. SPEC-001 rejects
/// such a schema at load time, so this is a backstop and not a case: the
/// renderer may treat every column as resolvable.
fn list_columns(collection: &Collection) -> Vec<&Field> {
    collection
        .list_columns
        .iter()
        .filter_map(|name| collection.fields.iter().find(|field| &field.name == name))
        .collect()
}

/// One record's cells, in column order.
fn cells_of(fields: &[&Field], data: &Json) -> Vec<Cell> {
    fields
        .iter()
        // `data.get` is `None` when the key is absent *and* when `data` is not
        // an object at all, so a record whose document is a string renders
        // every column as a dash.
        .map(|field| cell(&field.kind, data.get(field.name.as_str())))
        .collect()
}

/// The natural width of every column: the widest of its header and its cells
/// over all loaded records, never less than [`MIN_WIDTH`].
fn natural_widths(fields: &[&Field], rows: &[Vec<Cell>]) -> Vec<usize> {
    fields
        .iter()
        .enumerate()
        .map(|(column, field)| {
            let widest = rows
                .iter()
                // `get` because a short row would otherwise index out of range.
                .filter_map(|row| row.get(column))
                .map(|cell| cell.text.chars().count())
                .max()
                .unwrap_or(0);
            // In characters, not bytes: `✓` and `—` are one column each.
            widest.max(header_of(field).chars().count()).max(MIN_WIDTH)
        })
        .collect()
}

/// A column's header: the field's label, uppercased.
fn header_of(field: &Field) -> String {
    field.label.to_uppercase()
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}

/// What every row of one table shares: where the panel is, how wide each
/// visible column came out, and how much room the columns have in total.
///
/// Grouped because these three are the same for the header and for every record
/// row, and passing them separately made the row function unreadable.
struct Layout<'a> {
    area: Rect,
    widths: &'a [usize],
    columns_area: usize,
}

/// One row of the panel, from the marker column and the cells.
///
/// Built as spans rather than one string so a dim cell can be dim inside a
/// reverse-video row: the line's style is the row's, and a span's patches it.
fn draw_row(
    frame: &mut Frame,
    layout: &Layout,
    y: u16,
    marker: &str,
    cells: &[Cell],
    row_style: Style,
    tail: Option<String>,
) {
    let mut spans = vec![Span::raw(" "), Span::raw(marker.to_string())];
    let mut drawn = 0;

    // `zip` stops at the shorter of the two, so a dropped column is never
    // drawn: `widths` holds only the visible ones.
    for (cell, width) in cells.iter().zip(layout.widths) {
        if drawn > 0 {
            spans.push(Span::raw(" ".repeat(GAP)));
            drawn += GAP;
        }
        // Truncated to `width - 1` plus `…` when it does not fit, then padded
        // to the column so the next one starts where the header says.
        let text = truncate(&cell.text, *width);
        let padding = width.saturating_sub(text.chars().count());
        spans.push(Span::styled(
            text,
            if cell.dim { dim() } else { Style::new() },
        ));
        spans.push(Span::raw(" ".repeat(padding)));
        drawn += width;
    }

    // The `+k`, on the header row only; every other row leaves the reserve
    // blank, so no data row carries the count.
    let tail = tail.unwrap_or_default();
    let pad = layout
        .columns_area
        .saturating_sub(drawn)
        .saturating_sub(tail.chars().count());
    spans.push(Span::raw(" ".repeat(pad)));
    spans.push(Span::raw(tail));
    // The right gutter, so a reverse-video row reaches the panel's edge.
    spans.push(Span::raw(" "));

    // The spans total the panel's width, so the line covers the row exactly and
    // the row's own style paints all of it.
    let line = Line::from(spans).style(row_style);
    frame.render_widget(
        Paragraph::new(line),
        Rect::new(layout.area.x, y, layout.area.width, 1),
    );
}

/// The header row: the uppercased labels, and the `+k` when columns were
/// dropped. The marker column is blank on it.
fn header(frame: &mut Frame, layout: &Layout, fields: &[&Field], dropped: usize) {
    let cells: Vec<Cell> = fields
        .iter()
        .take(layout.widths.len())
        .map(|field| plain(header_of(field)))
        .collect();
    // `then` builds the string only when something was actually dropped.
    let tail = (dropped > 0).then(|| format!("+{dropped}"));

    draw_row(
        frame,
        layout,
        layout.area.y,
        " ",
        &cells,
        Style::new(),
        tail,
    );
}

/// Row 2: `─` across the marker column and the columns area, one blank gutter
/// each side.
fn rule(frame: &mut Frame, area: Rect) {
    let width = area.width.saturating_sub(2) as usize;
    frame
        .buffer_mut()
        .set_string(area.x + 1, area.y + 1, "─".repeat(width), Style::new());
}

/// The body when there are no records: why there are none, not a blank panel.
///
/// A blank row and then the sentence, indented and directly under the rule
/// rather than centred in the panel (frames A and G).
fn notice(frame: &mut Frame, area: Rect, load: Load) {
    let text = match load {
        Load::Ok => EMPTY,
        Load::Failed => FAILED,
    };
    // Three rows in: the header, the rule, and the blank row under it.
    let y = area.y + HEADER_ROWS + 1;
    if y >= area.bottom() {
        return;
    }

    frame
        .buffer_mut()
        .set_string(area.x + BODY_INDENT, y, text, Style::new());
}

/// One row per visible record, with the scroll summaries at the ends.
fn body(frame: &mut Frame, layout: &Layout, state: &AppState, rows: &[Vec<Cell>]) {
    let area = layout.area;
    let height = area.height.saturating_sub(HEADER_ROWS) as usize;
    let len = rows.len();
    // The same window the sidebar uses, now shared (`view::window`).
    let first = window(state.list.offset, state.list.selected, len, height);

    for slot in 0..height {
        let index = first + slot;
        if index >= len {
            break;
        }
        // `as u16` is safe: `slot` is bounded by the area's own height.
        let y = area.y + HEADER_ROWS + slot as u16;

        // Nothing is clipped silently: the top row stands in for everything
        // above it, the bottom row for everything below, counting itself.
        let hidden_above = (slot == 0 && first > 0).then_some(first + 1);
        let hidden_below =
            (slot + 1 == height && first + height < len).then(|| len - (first + height) + 1);

        if let Some(n) = hidden_above.or(hidden_below) {
            let arrow = if hidden_above.is_some() { "↑" } else { "↓" };
            let summary = format!(" {arrow} {n} more");
            frame.buffer_mut().set_string(area.x, y, summary, dim());
            continue;
        }

        let selected = index == state.list.selected;
        let row_style = if selected {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };

        draw_row(
            frame,
            layout,
            y,
            if selected { MARKER } else { " " },
            // `get` rather than indexing: `index` is inside `rows` here, and a
            // missing row is an empty one rather than a panic.
            rows.get(index).map_or(&[][..], Vec::as_slice),
            row_style,
            None,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `cell` for a value written as JSON text, so the cases read like the
    /// spec's table.
    fn rendered(kind: &FieldType, document: &str) -> Cell {
        let value: Json = document.parse().expect("the case is JSON");
        cell(kind, Some(&value))
    }

    /// Every field type, so a case can be asserted "for every field type".
    fn all_types() -> Vec<FieldType> {
        vec![
            FieldType::Text,
            FieldType::Textarea,
            FieldType::Number,
            FieldType::Boolean,
            FieldType::DateTime,
            FieldType::Select {
                options: vec!["draft".to_string()],
            },
            FieldType::Json,
        ]
    }

    // -----------------------------------------------------------------------
    // columns
    // -----------------------------------------------------------------------

    #[test]
    fn the_space_is_split_in_proportion_to_the_natural_widths() {
        // Frame B's own arithmetic: 13 : 9 : 6 over the 54 columns a 80x24
        // main panel leaves.
        let (widths, dropped) = columns(&[13, 9, 6], 54);

        assert_eq!(widths, vec![24, 16, 10]);
        assert_eq!(dropped, 0);
        // Two spaces between columns, so the widths plus the gaps are the area.
        assert_eq!(widths.iter().sum::<usize>() + 2 * (widths.len() - 1), 54);
    }

    #[test]
    fn the_widths_and_the_gaps_always_fill_the_area_exactly() {
        for naturals in [
            vec![6, 6, 6],
            vec![7, 9, 6],
            vec![13, 8, 16, 8],
            vec![6, 100],
            vec![40],
        ] {
            for area in [34, 40, 54, 77] {
                let (widths, dropped) = columns(&naturals, area);
                // Not vacuous: at these areas something always fits.
                assert!(!widths.is_empty(), "{naturals:?} in {area} fitted nothing");
                assert_eq!(widths.len() + dropped, naturals.len());

                let gaps = 2 * (widths.len() - 1);
                assert!(
                    widths.iter().sum::<usize>() + gaps <= area,
                    "{naturals:?} in {area} overflowed: {widths:?}"
                );
            }
        }
    }

    #[test]
    fn every_column_is_at_least_six_wide() {
        // The case proportional division alone gets wrong: one narrow column
        // beside a very wide one would round to nothing.
        for naturals in [
            vec![6, 100],
            vec![6, 6, 200],
            vec![6, 7, 8, 500],
            vec![1000, 6],
        ] {
            for area in [34, 54] {
                let (widths, _) = columns(&naturals, area);
                assert!(!widths.is_empty(), "{naturals:?} in {area} fitted nothing");
                assert!(
                    widths.iter().all(|width| *width >= 6),
                    "{naturals:?} in {area} gave {widths:?}"
                );
            }
        }
    }

    #[test]
    fn trailing_columns_are_dropped_until_the_rest_fit() {
        // Frame I: seven columns in the 34 a 60-column terminal leaves. Four
        // fit at six characters each, and three are reported.
        let (widths, dropped) = columns(&[13, 9, 28, 6, 8, 16, 8], 34);

        assert_eq!(widths, vec![6, 6, 6, 6]);
        assert_eq!(dropped, 3);
    }

    #[test]
    fn an_area_too_small_for_one_column_drops_them_all() {
        // Total rather than a panic: the layout floor makes this unreachable,
        // and a renderer that cannot be called with a bad area is one less
        // thing to reason about.
        let (widths, dropped) = columns(&[6, 6], 4);
        assert!(widths.is_empty());
        assert_eq!(dropped, 2);
    }

    // -----------------------------------------------------------------------
    // cell, one case per row of frame D's table
    // -----------------------------------------------------------------------

    #[test]
    fn text_renders_as_itself() {
        for kind in [
            FieldType::Text,
            FieldType::Textarea,
            FieldType::Select {
                options: vec!["draft".to_string()],
            },
        ] {
            let rendered = rendered(&kind, r#""Hello world""#);
            assert_eq!(rendered.text, "Hello world");
            assert!(!rendered.dim);
        }
    }

    #[test]
    fn a_number_renders_as_the_json_numbers_own_text() {
        assert_eq!(rendered(&FieldType::Number, "42").text, "42");
        // Not `42.0`: the stored text is what is shown.
        assert_eq!(rendered(&FieldType::Number, "0").text, "0");
        assert_eq!(rendered(&FieldType::Number, "-1.5").text, "-1.5");
    }

    #[test]
    fn a_boolean_renders_as_a_tick_or_a_dot() {
        assert_eq!(rendered(&FieldType::Boolean, "true").text, "✓");
        assert_eq!(rendered(&FieldType::Boolean, "false").text, "·");
        assert!(!rendered(&FieldType::Boolean, "false").dim);
    }

    #[test]
    fn a_datetime_renders_as_the_first_sixteen_characters_with_a_space() {
        let rendered = rendered(&FieldType::DateTime, r#""2026-09-06T12:00:00Z""#);
        assert_eq!(rendered.text, "2026-09-06 12:00");
        assert!(!rendered.dim);
    }

    #[test]
    fn a_datetime_that_is_not_shaped_like_one_is_an_invalid_value() {
        // Not parsed, so the check is on the shape; a string that is not one
        // falls to the invalid-value rule rather than being cut to 16 anyway.
        let rendered = rendered(&FieldType::DateTime, r#""last Tuesday""#);
        assert_eq!(rendered.text, r#""last Tuesday""#);
        assert!(rendered.dim);
    }

    #[test]
    fn json_renders_as_its_size() {
        assert_eq!(
            rendered(&FieldType::Json, r#"{"a":1,"b":2,"c":3}"#).text,
            "{3}"
        );
        assert_eq!(rendered(&FieldType::Json, "[1,2]").text, "[2]");
        assert_eq!(rendered(&FieldType::Json, "{}").text, "{0}");
    }

    #[test]
    fn a_scalar_in_a_json_field_renders_as_its_json_text() {
        assert_eq!(rendered(&FieldType::Json, r#""plain""#).text, r#""plain""#);
        assert_eq!(rendered(&FieldType::Json, "7").text, "7");
        assert_eq!(rendered(&FieldType::Json, "true").text, "true");
        // Valid for a `Json` field, so not dim: anything that parsed is a
        // valid JSON value (SPEC-002's type table).
        assert!(!rendered(&FieldType::Json, "7").dim);
    }

    #[test]
    fn an_absent_or_null_value_is_a_dim_dash_for_every_field_type() {
        for kind in all_types() {
            for value in [None, Some(Json::Null)] {
                let rendered = cell(&kind, value.as_ref());
                assert_eq!(rendered.text, "—", "{kind:?} rendered the wrong dash");
                assert!(rendered.dim, "{kind:?} drew an absent value undimmed");
            }
        }
    }

    #[test]
    fn a_value_the_field_does_not_allow_renders_as_its_json_text_dimmed() {
        // Storage does not validate (SPEC-003), so the table must render what
        // is there rather than refuse to draw.
        let number_in_text = rendered(&FieldType::Text, "42");
        assert_eq!(number_in_text.text, "42");
        assert!(number_in_text.dim);

        let string_in_boolean = rendered(&FieldType::Boolean, r#""yes""#);
        assert_eq!(string_in_boolean.text, r#""yes""#);
        assert!(string_in_boolean.dim);
    }

    #[test]
    fn a_run_of_control_characters_becomes_one_space() {
        // One row per record, whatever the data: a newline that reached the
        // buffer would push the table's rule off the screen.
        let rendered = rendered(&FieldType::Text, r#""first\n\nsecond\tthird""#);
        assert_eq!(rendered.text, "first second third");
        assert!(!rendered.text.contains('\n'));
    }

    #[test]
    fn no_value_of_any_type_renders_a_control_character() {
        for kind in all_types() {
            for document in [r#""a\nb""#, r#""a\r\nb""#, "{\"k\":\"v\\nw\"}", "42"] {
                let rendered = rendered(&kind, document);
                assert!(
                    !rendered.text.chars().any(char::is_control),
                    "{kind:?} on {document} kept a control character"
                );
            }
        }
    }
}
