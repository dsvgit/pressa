//! The one form. Takes a `&Collection` and edits whatever it describes: no
//! branch on a collection slug or a field name, ever
//! ([ADR-0004](../../../../docs/adr/0004-schema-driven-ui.md)).
//!
//! The layout is SPEC-008 "The form, stated once". [`display`], [`editable`],
//! [`parse`], [`value_column`], [`block_height`] and [`first_visible`] are pure
//! and unit-tested below; [`render`] is what the snapshots cover.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use pressa_app::domain::{Collection, ErrorCode, Field, FieldError, FieldType, Json};

use crate::tui::state::{AppState, EditorState};
use crate::tui::view::table::Cell;
use crate::tui::view::truncate;

/// Two columns before every label: a gutter and the focus marker's column.
const INDENT: usize = 2;
/// A stacked value sits under its label, indented this far.
const STACK_INDENT: usize = 4;
/// The value column is never left of this, so `posts` lines up at 18 (Q9).
const VALUE_FLOOR: usize = 18;
/// Columns between the longest label and the value column.
const VALUE_GAP: usize = 2;
/// The focus bar starts this far in, and stops this far from the right edge.
const BAR_MARGIN: usize = 2;
/// A `Textarea` or `Json` field being typed into shows this many rows (Q16).
const MULTILINE_ROWS: usize = 5;
/// The scroll summaries keep this many columns clear of the right border.
const SUMMARY_GUTTER: u16 = 2;

const ABSENT: &str = "—";
const MARKER: &str = "▸";
const CURSOR: &str = "▌";
const ELLIPSIS: char = '…';
const REQUIRED: &str = " *";
const WARNING: &str = "⚠ ";

/// The messages SPEC-002 gives the three faults the editor finds itself.
const NOT_A_NUMBER: &str = "must be a number";
const NOT_A_BOOLEAN: &str = "must be true or false";
const NOT_A_DATE: &str = "must be a date like 2026-09-06T12:00:00Z";
const NOT_JSON: &str = "not valid JSON";

// ---------------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------------

/// How one field's value reads, and whether it is dim: the editable text, a
/// dim `—` for absent, dim JSON for a value the field's type forbids.
///
/// Deliberately not `table::cell`: a form shows what you would be editing, so
/// a value does not change format when it is focused (Q7).
pub fn display(kind: &FieldType, value: Option<&Json>) -> Cell {
    // Absent and null are one case; `filter` folds null into `None`.
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Cell {
            text: ABSENT.to_string(),
            dim: true,
        };
    };

    Cell {
        text: editable(kind, Some(value)),
        dim: !allowed(kind, value),
    }
}

/// The text `BeginEdit` seeds the input buffer with: a string as itself,
/// anything else as its JSON text, and nothing for an absent value.
pub fn editable(kind: &FieldType, value: Option<&Json>) -> String {
    match (kind, value) {
        (_, None | Some(Json::Null)) => String::new(),
        // The three string-shaped types show the string, not its quotes.
        (
            FieldType::Text | FieldType::Textarea | FieldType::DateTime | FieldType::Select { .. },
            Some(Json::String(text)),
        ) => text.clone(),
        // A number's own text (`42`, not `42.0`), a boolean's `true` /
        // `false`, a `Json` field's JSON, and a forbidden value's JSON — all
        // are what `to_string` writes.
        (_, Some(other)) => other.to_string(),
    }
}

/// The inverse of [`editable`]: the value `CommitField` writes into the draft,
/// or why the text is not one. Empty text is no value at all.
///
/// The returned error's `field` is empty; the caller knows which field it was
/// and fills it in.
pub fn parse(kind: &FieldType, text: &str) -> Result<Json, FieldError> {
    // Clearing a box clears the field; whitespace around a number or a date
    // is not part of it.
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Json::Null);
    }

    match kind {
        FieldType::Text | FieldType::Textarea => Ok(Json::String(text.to_string())),
        // `parse` through `Json` so `42` becomes the number it reads as; a
        // quoted `"42"` parses too, as a string, and is refused below.
        FieldType::Number => match trimmed.parse::<Json>() {
            Ok(number @ Json::Number(_)) => Ok(number),
            _ => Err(refused(ErrorCode::TypeMismatch, NOT_A_NUMBER)),
        },
        FieldType::Boolean => match trimmed {
            "true" => Ok(Json::Bool(true)),
            "false" => Ok(Json::Bool(false)),
            _ => Err(refused(ErrorCode::TypeMismatch, NOT_A_BOOLEAN)),
        },
        FieldType::DateTime if is_rfc3339(trimmed) => Ok(Json::String(trimmed.to_string())),
        FieldType::DateTime => Err(refused(ErrorCode::InvalidDateTime, NOT_A_DATE)),
        // `any` because `contains` would want a `&String`.
        FieldType::Select { options } if options.iter().any(|option| option == trimmed) => {
            Ok(Json::String(trimmed.to_string()))
        }
        FieldType::Select { options } => Err(refused(
            ErrorCode::NotInOptions,
            &format!("must be one of: {}", options.join(", ")),
        )),
        FieldType::Json => trimmed
            .parse::<Json>()
            // The parser's own message is for developers; SPEC-002's is fixed.
            .map_err(|_| refused(ErrorCode::InvalidJson, NOT_JSON)),
    }
}

/// Whether `value`'s JSON type is one `kind` accepts. Not validation — an
/// option outside `options` is still a string — only "is this the right
/// shape to draw as itself".
fn allowed(kind: &FieldType, value: &Json) -> bool {
    match kind {
        FieldType::Text | FieldType::Textarea | FieldType::DateTime | FieldType::Select { .. } => {
            value.is_string()
        }
        FieldType::Number => value.is_number(),
        FieldType::Boolean => value.is_boolean(),
        // Anything that parsed is a JSON value.
        FieldType::Json => true,
    }
}

fn refused(code: ErrorCode, message: &str) -> FieldError {
    FieldError {
        field: String::new(),
        code,
        message: message.to_string(),
    }
}

/// Whether `text` is an RFC 3339 timestamp: `2026-09-06T12:00:00Z`, with an
/// optional fraction and a `Z` or `±HH:MM` offset, and a day that exists.
///
/// By hand because `pressa-tui` may name no date library (SPEC-008
/// "Non-goals"); `RecordService` checks again with `chrono` on save, so this
/// only has to catch what a person mistypes.
fn is_rfc3339(text: &str) -> bool {
    let bytes = text.as_bytes();
    // `get` rather than indexing, so a short string is `None` and not a panic.
    let number = |from: usize, to: usize| -> Option<u32> {
        let digits = bytes.get(from..to)?;
        // `all` before parsing so a sign or a space cannot sneak through.
        if !digits.iter().all(u8::is_ascii_digit) {
            return None;
        }
        text.get(from..to)?.parse().ok()
    };
    let at = |index: usize| bytes.get(index).copied();

    // `?`-style early exits through `let … else`: any miss is "not a date".
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        number(0, 4),
        number(5, 7),
        number(8, 10),
        number(11, 13),
        number(14, 16),
        number(17, 19),
    ) else {
        return false;
    };
    let separators = at(4) == Some(b'-')
        && at(7) == Some(b'-')
        && matches!(at(10), Some(b'T' | b't'))
        && at(13) == Some(b':')
        && at(16) == Some(b':');
    if !separators || hour > 23 || minute > 59 || second > 60 {
        return false;
    }
    if !(1..=12).contains(&month) || day == 0 || day > days_in(year, month) {
        return false;
    }

    // An optional fraction: a dot and at least one digit.
    let mut rest = 19;
    if at(rest) == Some(b'.') {
        let digits = bytes.get(rest + 1..).map_or(0, |tail| {
            tail.iter().take_while(|b| b.is_ascii_digit()).count()
        });
        if digits == 0 {
            return false;
        }
        rest += 1 + digits;
    }

    match at(rest) {
        Some(b'Z' | b'z') => rest + 1 == bytes.len(),
        Some(b'+' | b'-') => {
            let offset = (number(rest + 1, rest + 3), number(rest + 4, rest + 6));
            matches!(offset, (Some(h), Some(m)) if h <= 23 && m <= 59)
                && at(rest + 3) == Some(b':')
                && rest + 6 == bytes.len()
        }
        _ => false,
    }
}

/// How many days `month` of `year` has, leap years included.
fn days_in(year: u32, month: u32) -> u32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// The panel column inline widgets start at: past the longest label — its
/// ` *` included — and a gap, and never left of 18 (Q9).
///
/// From the schema alone, so widgets line up within a collection and a long
/// label pushes the column out rather than being drawn over.
pub fn value_column(collection: &Collection) -> usize {
    let widest = collection
        .fields
        .iter()
        // In characters, not bytes: a label may be more than ASCII.
        .map(|field| field.label.chars().count() + if field.required { REQUIRED.len() } else { 0 })
        .max()
        .unwrap_or(0);
    (INDENT + widest + VALUE_GAP).max(VALUE_FLOOR)
}

/// How many rows one field's block occupies, before the blank row after it.
///
/// Focus is not an input: moving it never changes a height, so nothing on the
/// form moves when you `Tab` (Q16). Only `typing` into a `Textarea` or `Json`
/// field grows its block, and that happens on a deliberate `Enter`.
pub fn block_height(field: &Field, typing: bool, has_error: bool) -> usize {
    let rows = match (&field.kind, typing) {
        // Inline: the label row is the whole block.
        (FieldType::Boolean | FieldType::Select { .. }, _) => 1,
        // Label and five rows of text while it is being written.
        (FieldType::Textarea | FieldType::Json, true) => 1 + MULTILINE_ROWS,
        // Label and value.
        _ => 2,
    };
    rows + usize::from(has_error)
}

/// The first field the panel draws: the smallest index from which the focused
/// block and its trailing blank row both fit in `rows` — the least scroll that
/// keeps the focused field whole (Q4). A block taller than the panel starts
/// at itself.
pub fn first_visible(focus: usize, heights: &[usize], rows: usize) -> usize {
    // Each block is followed by one blank row, which counts towards the fit:
    // it keeps the focused value off the last row, where `↓ n more` is drawn.
    let Some(focused) = heights.get(focus) else {
        return 0;
    };
    let mut used = focused + 1;
    let mut first = focus;

    // Walk upwards while the block above still fits.
    while let Some(above) = first.checked_sub(1).and_then(|index| heights.get(index)) {
        if used + above + 1 > rows {
            break;
        }
        used += above + 1;
        first -= 1;
    }
    first
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}

fn styled(cell: Cell) -> Span<'static> {
    let style = if cell.dim { dim() } else { Style::new() };
    Span::styled(cell.text, style)
}

/// One block per field, scrolled to keep the focus whole, with the scroll
/// summaries at the ends.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, collection: &Collection) {
    let editor = &state.editor;
    let fields = &collection.fields;
    let rows = area.height as usize;
    // Clamped rather than trusted: a hand-built state may point past the end.
    let focus = editor.focus.min(fields.len().saturating_sub(1));

    let heights: Vec<usize> = fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            // Only the focused field is ever typed into.
            let typing = index == focus && editor.input.is_some();
            block_height(field, typing, error_of(editor, field).is_some())
        })
        .collect();
    let first = first_visible(focus, &heights, rows);
    let column = value_column(collection);

    let mut y = 0;
    let mut last_drawn = first;
    // `skip` starts at the first visible field; the loop stops at the bottom.
    for (index, field) in fields.iter().enumerate().skip(first) {
        if y >= rows {
            break;
        }
        let block = Block {
            field,
            value: editor.draft.get(field.name.as_str()),
            focused: index == focus,
            // Only the focused field is ever being typed into.
            input: editor.input.as_deref().filter(|_| index == focus),
            error: error_of(editor, field),
            column,
            width: area.width,
        };
        for line in block.lines() {
            if y >= rows {
                break;
            }
            // `as u16` is safe: `y` is bounded by the area's own height.
            let row = Rect::new(area.x, area.y + y as u16, area.width, 1);
            frame.render_widget(Paragraph::new(line), row);
            y += 1;
        }
        last_drawn = index;
        y += 1; // the blank row after every block
    }

    // Nothing is clipped silently: the ends say how many fields are not drawn
    // at all. A block cut off at the bottom has a row drawn, so it is not
    // counted (frame B's `Views`).
    let below = fields.len().saturating_sub(last_drawn + 1);
    if first > 0 {
        summary(frame, area, area.y, &format!("↑ {first} more"));
    }
    if below > 0 && rows > 0 {
        summary(frame, area, area.bottom() - 1, &format!("↓ {below} more"));
    }
}

/// The field's error, if the last save or the last commit reported one. The
/// first in the service's order when there are several (`errors` holds one).
fn error_of<'a>(editor: &'a EditorState, field: &Field) -> Option<&'a FieldError> {
    editor.errors.iter().find(|error| error.field == field.name)
}

/// A dim summary, right-aligned with a gutter, over whatever row `y` holds.
fn summary(frame: &mut Frame, area: Rect, y: u16, text: &str) {
    // In characters: `↑` is three bytes and one column.
    let width = text.chars().count() as u16;
    let x = area.right().saturating_sub(SUMMARY_GUTTER + width);
    frame.buffer_mut().set_string(x.max(area.x), y, text, dim());
}

/// Everything one field's rows are drawn from.
struct Block<'a> {
    field: &'a Field,
    value: Option<&'a Json>,
    focused: bool,
    /// The text being typed, when this field is the one in an input context.
    input: Option<&'a str>,
    error: Option<&'a FieldError>,
    column: usize,
    width: u16,
}

impl Block<'_> {
    /// The block's rows, top to bottom, as many as `block_height` says.
    fn lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![self.label()];
        match (&self.field.kind, self.input) {
            // Inline: the label row already carries the value.
            (FieldType::Boolean | FieldType::Select { .. }, _) => {}
            (_, None) => lines.push(self.value_row()),
            (FieldType::Textarea | FieldType::Json, Some(text)) => {
                lines.extend(self.typed(text, MULTILINE_ROWS))
            }
            (_, Some(text)) => lines.extend(self.typed(text, 1)),
        }
        if let Some(error) = self.error {
            lines.push(self.error_row(error));
        }
        lines
    }

    /// The gutter, the focus marker, the label, ` *` when required, and the
    /// inline widget for the two types that have one.
    fn label(&self) -> Line<'static> {
        let marker = if self.focused { MARKER } else { " " };
        let mut text = format!(" {marker}{}", self.field.label);
        if self.field.required {
            text.push_str(REQUIRED);
        }

        let widget = self.widget();
        if widget.is_empty() {
            return Line::from(text);
        }
        // Padded out to the value column; the column is past every label, so
        // the padding is never negative.
        let pad = self.column.saturating_sub(text.chars().count());
        let mut spans = vec![Span::raw(text), Span::raw(" ".repeat(pad))];
        // Reverse video marks a focused widget (Q3a), patched over each span
        // so a dim `—` inside it stays dim.
        for span in widget {
            spans.push(if self.focused {
                span.patch_style(Style::new().add_modifier(Modifier::REVERSED))
            } else {
                span
            });
        }
        Line::from(spans)
    }

    /// `[x]` / `[ ]` or `‹ value ›`, or nothing for the stacked types.
    fn widget(&self) -> Vec<Span<'static>> {
        match (&self.field.kind, self.value) {
            (FieldType::Boolean, Some(Json::Bool(yes))) => {
                vec![Span::raw(if *yes { "[x]" } else { "[ ]" })]
            }
            // Absent, or not a boolean: what `display` says, dim.
            (FieldType::Boolean, value) => vec![styled(display(&self.field.kind, value))],
            (FieldType::Select { .. }, value) => vec![
                Span::raw("‹ "),
                styled(display(&self.field.kind, value)),
                Span::raw(" ›"),
            ],
            _ => Vec::new(),
        }
    }

    /// The columns a value's text gets: from the stack indent to the gutter.
    fn room(&self) -> usize {
        (self.width as usize).saturating_sub(STACK_INDENT + BAR_MARGIN)
    }

    /// The value under its label: one row, control characters as spaces, and
    /// `…` at the end when it does not fit. On the focused field, a bar.
    fn value_row(&self) -> Line<'static> {
        let cell = display(&self.field.kind, self.value);
        let text = truncate(&collapse(&cell.text), self.room());
        self.row(Cell { text, ..cell })
    }

    /// What is being typed, then the cursor (Q15): one row showing the tail
    /// behind `…`, or `rows` wrapped rows for a multi-line field.
    fn typed(&self, text: &str, rows: usize) -> Vec<Line<'static>> {
        // The cursor is part of the text, so it is never pushed off the row.
        let text = format!("{text}{CURSOR}");
        let texts = if rows == 1 {
            vec![tail(&collapse(&text), self.room())]
        } else {
            wrapped(&text, self.room(), rows)
        };
        texts
            .into_iter()
            .map(|text| self.row(Cell { text, dim: false }))
            .collect()
    }

    /// One value row at the stack indent. Focused, it is a reverse-video bar
    /// from the indent to the gutter — the list's selection, so focus reads
    /// the same way in both screens — and the same height as unfocused, which
    /// is why no other field moves (Q16).
    fn row(&self, cell: Cell) -> Line<'static> {
        let margin = " ".repeat(BAR_MARGIN);
        if !self.focused {
            let indent = " ".repeat(STACK_INDENT);
            return Line::from(vec![Span::raw(indent), styled(cell)]);
        }
        let pad = self.room().saturating_sub(cell.text.chars().count());
        let bar = Style::new().add_modifier(Modifier::REVERSED);
        Line::from(vec![
            Span::raw(margin.clone()),
            Span::styled(margin, bar),
            // Patched, so a dim `—` inside the bar stays dim.
            styled(cell).patch_style(bar),
            Span::styled(" ".repeat(pad), bar),
        ])
    }

    /// `  ⚠ message`, in red, cut to the panel.
    fn error_row(&self, error: &FieldError) -> Line<'static> {
        let room = (self.width as usize).saturating_sub(INDENT + BAR_MARGIN);
        let text = truncate(&format!("{WARNING}{}", error.message), room);
        Line::from(vec![
            Span::raw(" ".repeat(INDENT)),
            Span::styled(text, Style::new().fg(Color::Red)),
        ])
    }
}

/// `text` with every control character — a newline included — as a space, so
/// a one-row value stays one row.
fn collapse(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// The last `room` characters of `text`, with `…` in the first column when
/// the head was cut (Q3c), so the cursor stays in view. By characters.
fn tail(text: &str, room: usize) -> String {
    let count = text.chars().count();
    if count <= room {
        return text.to_string();
    }
    // `skip` drops the head; one column goes to the ellipsis.
    let kept: String = text.chars().skip(count + 1 - room).collect();
    format!("{ELLIPSIS}{kept}")
}

/// `text` wrapped at `room` characters, newlines respected, as exactly `rows`
/// rows. When there are more, the last `rows` are kept and the first of them
/// starts with `…`, as a one-row value's tail does.
fn wrapped(text: &str, room: usize, rows: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let chars: Vec<char> = paragraph.chars().collect();
        if chars.is_empty() {
            lines.push(String::new());
        }
        // `chunks` cuts the paragraph into rows of at most `room` characters;
        // `max(1)` keeps a zero-width box from asking for empty chunks.
        for chunk in chars.chunks(room.max(1)) {
            lines.push(chunk.iter().collect());
        }
    }

    if lines.len() > rows {
        lines.drain(..lines.len() - rows);
        if let Some(first) = lines.first_mut() {
            // Replaces the first character, so the row keeps its width.
            let rest: String = first.chars().skip(1).collect();
            *first = format!("{ELLIPSIS}{rest}");
        }
    }
    lines.resize(rows, String::new());
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    use pressa_app::domain::ErrorCode;

    use crate::tui::view::table;

    /// JSON text as a value, so the cases read like the spec's tables.
    fn json(text: &str) -> Json {
        text.parse().expect("the case is JSON")
    }

    fn select() -> FieldType {
        FieldType::Select {
            options: vec!["draft".to_string(), "published".to_string()],
        }
    }

    /// Every field type, so a case can be asserted "for every field type".
    fn all_types() -> Vec<FieldType> {
        vec![
            FieldType::Text,
            FieldType::Textarea,
            FieldType::Number,
            FieldType::Boolean,
            FieldType::DateTime,
            select(),
            FieldType::Json,
        ]
    }

    fn field(label: &str, kind: FieldType, required: bool) -> Field {
        Field {
            // The name is deliberately not the label: nothing may read it.
            name: format!("f{}", label.len()),
            label: label.to_string(),
            kind,
            required,
            unique: false,
        }
    }

    /// `examples/blog`'s `posts`, the one collection a unit test can load
    /// without a temporary file; reshaped by replacing its fields.
    fn posts() -> Collection {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/blog/pressa.yaml");
        let schema = pressa_app::config::load_schema(&path).expect("the example loads");
        schema
            .collections
            .get("posts")
            .cloned()
            .expect("the example has posts")
    }

    /// The posts collection with `fields` in place of its own.
    fn with_fields(fields: Vec<Field>) -> Collection {
        Collection { fields, ..posts() }
    }

    // -----------------------------------------------------------------------
    // block_height
    // -----------------------------------------------------------------------

    #[test]
    fn block_heights_follow_the_type_the_typing_and_the_error_row() {
        // (type, not typing, typing): focus alone changes nothing (Q16).
        let cases = [
            (FieldType::Text, 2, 2),
            (FieldType::Textarea, 2, 6),
            (FieldType::Number, 2, 2),
            (FieldType::Boolean, 1, 1),
            (FieldType::DateTime, 2, 2),
            (select(), 1, 1),
            (FieldType::Json, 2, 6),
        ];
        for (kind, still, typing) in cases {
            let f = field("Label", kind.clone(), false);
            assert_eq!(block_height(&f, false, false), still, "{kind:?}");
            assert_eq!(block_height(&f, true, false), typing, "{kind:?}");
            // An error row is one more row, typing or not.
            assert_eq!(block_height(&f, false, true), still + 1, "{kind:?}");
            assert_eq!(block_height(&f, true, true), typing + 1, "{kind:?}");
        }
    }

    // -----------------------------------------------------------------------
    // first_visible
    // -----------------------------------------------------------------------

    /// The posts form's block heights, `typing` into field `focus`.
    fn posts_heights(focus: usize, typing: bool) -> Vec<usize> {
        posts()
            .fields
            .iter()
            .enumerate()
            .map(|(index, f)| block_height(f, typing && index == focus, false))
            .collect()
    }

    /// Whether the focused block and its trailing blank row fit from `first`.
    fn fits(first: usize, focus: usize, heights: &[usize], rows: usize) -> bool {
        // Each block is followed by one blank row.
        heights[first..=focus].iter().map(|h| h + 1).sum::<usize>() <= rows
    }

    #[test]
    fn the_window_is_the_least_scroll_that_keeps_the_focused_block_whole() {
        for (rows, typing) in [(17, false), (9, false), (17, true), (9, true)] {
            for focus in 0..8 {
                let heights = posts_heights(focus, typing);
                let first = first_visible(focus, &heights, rows);
                assert!(first <= focus, "focus {focus} at {rows}: first {first}");
                assert!(
                    fits(first, focus, &heights, rows),
                    "focus {focus} at {rows}: the focused block is cut from {first}"
                );
                // The least: one field less of scroll would not fit.
                if first > 0 {
                    assert!(
                        !fits(first - 1, focus, &heights, rows),
                        "focus {focus} at {rows}: {first} scrolls more than it must"
                    );
                }
            }
        }
    }

    #[test]
    fn frame_ds_state_needs_no_scroll() {
        // Focus on `Views`: Title through Views is 14 of the 17 rows.
        assert_eq!(first_visible(4, &posts_heights(4, false), 17), 0);
    }

    #[test]
    fn the_focused_value_is_never_on_the_last_body_row() {
        // The trailing blank row is in the fit, so the focused block ends on
        // the second-last row at the latest; the last stays free for `↓ n more`.
        for (rows, typing) in [(17, false), (9, false), (17, true), (9, true)] {
            for focus in 0..8 {
                let heights = posts_heights(focus, typing);
                let first = first_visible(focus, &heights, rows);
                // Rows up to and including the focused block, without its blank.
                let end: usize = heights[first..=focus].iter().map(|h| h + 1).sum::<usize>() - 1;
                assert!(end < rows, "focus {focus} at {rows}, typing {typing}");
            }
        }
    }

    #[test]
    fn moving_the_focus_never_changes_a_height() {
        // The bug Q16 fixes: a box made the focused block taller, so every
        // `Tab` moved the fields after it.
        for focus in 0..8 {
            assert_eq!(posts_heights(focus, false), posts_heights(0, false));
        }
    }

    #[test]
    fn a_block_taller_than_the_panel_starts_at_itself() {
        assert_eq!(first_visible(2, &[2, 2, 8], 5), 2);
    }

    // -----------------------------------------------------------------------
    // value_column
    // -----------------------------------------------------------------------

    #[test]
    fn the_value_column_is_eighteen_for_posts_and_the_seven_type_shape() {
        assert_eq!(value_column(&posts()), 18);
        let seven = with_fields(vec![
            field("Title", FieldType::Text, true),
            field("Status", select(), true),
            field("Views", FieldType::Number, false),
            field("Featured", FieldType::Boolean, false),
            field("Published at", FieldType::DateTime, false),
            field("Metadata", FieldType::Json, false),
            field("Notes", FieldType::Textarea, false),
        ]);
        assert_eq!(value_column(&seven), 18);
    }

    #[test]
    fn a_long_label_pushes_the_value_column_out() {
        let label = "A twenty char label!";
        assert_eq!(label.chars().count(), 20);
        let collection = with_fields(vec![
            field(label, FieldType::Boolean, true),
            field("Short", select(), false),
        ]);
        let column = value_column(&collection);
        assert!(column > 18, "column was {column}");
        // Two of indent, the label, ` *`, two of gap (SPEC-008 Q9).
        assert_eq!(column, 2 + 20 + 2 + 2);
    }

    // -----------------------------------------------------------------------
    // display
    // -----------------------------------------------------------------------

    #[test]
    fn display_is_the_editable_text_and_not_the_tables() {
        let when = json(r#""2026-09-06T12:00:00Z""#);
        let shown = display(&FieldType::DateTime, Some(&when));
        assert_eq!(shown.text, "2026-09-06T12:00:00Z");
        assert!(!shown.dim);
        assert_ne!(shown, table::cell(&FieldType::DateTime, Some(&when)));

        let object = json(r#"{"a":1,"b":2,"c":3}"#);
        let shown = display(&FieldType::Json, Some(&object));
        assert_eq!(shown.text, r#"{"a":1,"b":2,"c":3}"#);
        assert_ne!(shown, table::cell(&FieldType::Json, Some(&object)));

        assert_eq!(display(&FieldType::Number, Some(&json("42"))).text, "42");
        assert_eq!(
            display(&FieldType::Text, Some(&json(r#""Hello""#))).text,
            "Hello"
        );
        assert_eq!(display(&select(), Some(&json(r#""draft""#))).text, "draft");
    }

    #[test]
    fn an_absent_or_null_value_displays_a_dim_dash_for_every_type() {
        for kind in all_types() {
            for value in [None, Some(Json::Null)] {
                let shown = display(&kind, value.as_ref());
                assert_eq!(shown.text, "—", "{kind:?}");
                assert!(shown.dim, "{kind:?}");
            }
        }
    }

    #[test]
    fn a_value_the_field_does_not_allow_displays_as_dim_json() {
        let number_in_text = display(&FieldType::Text, Some(&json("42")));
        assert_eq!(number_in_text.text, "42");
        assert!(number_in_text.dim);

        let string_in_boolean = display(&FieldType::Boolean, Some(&json(r#""yes""#)));
        assert_eq!(string_in_boolean.text, r#""yes""#);
        assert!(string_in_boolean.dim);
    }

    // -----------------------------------------------------------------------
    // editable and parse
    // -----------------------------------------------------------------------

    #[test]
    fn parse_inverts_editable_for_every_type_and_every_value_it_accepts() {
        let cases: Vec<(FieldType, Vec<&str>)> = vec![
            (
                FieldType::Text,
                vec!["null", r#""Hello world""#, r#""ünï ✓""#],
            ),
            (FieldType::Textarea, vec!["null", r#""one\ntwo""#]),
            (FieldType::Number, vec!["null", "42", "0", "-1.5", "1e3"]),
            (FieldType::Boolean, vec!["null", "true", "false"]),
            (
                FieldType::DateTime,
                vec![
                    "null",
                    r#""2026-09-06T12:00:00Z""#,
                    r#""2026-09-06T12:00:00.5+02:00""#,
                ],
            ),
            (select(), vec!["null", r#""draft""#, r#""published""#]),
            (
                FieldType::Json,
                vec![
                    "null",
                    r#"{"tags":["rust"],"pinned":true}"#,
                    "[1,2]",
                    r#""plain""#,
                    "7",
                    "true",
                ],
            ),
        ];
        for (kind, values) in cases {
            for text in values {
                let value = json(text);
                let round = parse(&kind, &editable(&kind, Some(&value)));
                assert_eq!(round, Ok(value), "{kind:?} on {text}");
            }
        }
    }

    #[test]
    fn a_number_parses_to_a_json_number_and_not_a_string() {
        assert_eq!(parse(&FieldType::Number, "42"), Ok(json("42")));
    }

    #[test]
    fn text_that_does_not_parse_is_refused_with_spec_002s_code_and_message() {
        let cases = [
            (
                FieldType::Number,
                "forty-two",
                ErrorCode::TypeMismatch,
                "must be a number",
            ),
            (
                FieldType::DateTime,
                "last Tuesday",
                ErrorCode::InvalidDateTime,
                "must be a date like 2026-09-06T12:00:00Z",
            ),
            (
                FieldType::DateTime,
                "2026-02-30T12:00:00Z",
                ErrorCode::InvalidDateTime,
                "must be a date like 2026-09-06T12:00:00Z",
            ),
            (
                FieldType::Json,
                "{not json",
                ErrorCode::InvalidJson,
                "not valid JSON",
            ),
        ];
        for (kind, text, code, message) in cases {
            let error = parse(&kind, text).expect_err("the text does not parse");
            assert_eq!(error.code, code, "{kind:?} on {text}");
            assert_eq!(error.message, message, "{kind:?} on {text}");
        }
    }

    #[test]
    fn a_number_string_is_not_a_number_in_a_number_field() {
        // `"42"` with the quotes is JSON text for a string, not a number.
        assert!(parse(&FieldType::Number, r#""42""#).is_err());
    }
}
