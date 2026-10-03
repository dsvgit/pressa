//! What is drawn on top of a route: one dialog in T9, the discard prompt
//! (SPEC-008 frame I). T10's delete dialog and T11's help add variants here.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Clear;

use crate::tui::keymap::{Context, hints};
use crate::tui::state::Overlay;

/// The dialog's size; it shrinks to the panel when the panel is narrower.
const WIDTH: u16 = 42;
const HEIGHT: u16 = 7;
/// The body's indent inside the left border.
const INDENT: usize = 2;
/// The dialog's key line spaces its entries wider than the hint bar does.
const KEY_SEPARATOR: &str = "    ";

/// The discard dialog's wording: it names the changes rather than the record,
/// so it cannot be mistaken for the delete prompt it is shaped like (Q10).
const DISCARD_TITLE: &str = "Discard changes";
const DISCARD_LINES: [&str; 2] = ["Discard unsaved changes?", "This cannot be undone."];

/// Draws `overlay` centred over `area`, which stays drawn around it.
pub fn render(frame: &mut Frame, area: Rect, overlay: &Overlay) {
    let (title, lines, context) = match overlay {
        Overlay::ConfirmDiscard { .. } => (DISCARD_TITLE, DISCARD_LINES, Context::ConfirmDiscard),
    };

    let width = WIDTH.min(area.width);
    let height = HEIGHT.min(area.height);
    // Integer halves: an odd remainder leaves the extra column on the right.
    let dialog = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    // The dialog's rows are cleared across the whole panel, so no half-label
    // shows beside it; the rows above and below keep the form (frame I).
    frame.render_widget(Clear, Rect::new(area.x, dialog.y, area.width, height));

    let inner = width.saturating_sub(2) as usize;
    let keys = hints(context).join(KEY_SEPARATOR);
    let rows = [
        top(title, inner),
        body("", inner),
        body(&format!("{}{}", " ".repeat(INDENT), lines[0]), inner),
        body(&format!("{}{}", " ".repeat(INDENT), lines[1]), inner),
        body("", inner),
        body(&centred(&keys, inner), inner),
        format!("└{}┘", "─".repeat(inner)),
    ];

    let buffer = frame.buffer_mut();
    // `zip` stops at the dialog's height, so a short panel loses rows, not
    // its border.
    for (row, y) in rows.iter().zip(dialog.top()..dialog.bottom()) {
        buffer.set_stringn(dialog.x, y, row, width as usize, Style::new());
    }
}

/// `┌─ Title ───┐` across `inner` columns between the corners.
fn top(title: &str, inner: usize) -> String {
    let label = format!("─ {title} ");
    // In characters: `─` is three bytes and one column.
    let rest = inner.saturating_sub(label.chars().count());
    format!("┌{label}{}┐", "─".repeat(rest))
}

/// `│text      │`, padded to `inner` columns.
fn body(text: &str, inner: usize) -> String {
    let pad = inner.saturating_sub(text.chars().count());
    format!("│{text}{}│", " ".repeat(pad))
}

/// `text` with the room either side of it split, the odd column on the right.
fn centred(text: &str, inner: usize) -> String {
    let left = inner.saturating_sub(text.chars().count()) / 2;
    format!("{}{text}", " ".repeat(left))
}
