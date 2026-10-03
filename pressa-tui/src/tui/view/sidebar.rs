//! The sidebar: the schema's collections, with a selection and a viewport.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::tui::state::{AppState, Route};
use crate::tui::view::{SIDEBAR_WIDTH, truncate, window};

/// One leading space and a two-column marker leave this much for a label.
const LABEL_WIDTH: usize = SIDEBAR_WIDTH as usize - 3;
/// How many rows the title and the blank line under it take.
const HEADER_ROWS: u16 = 2;

/// The title, a blank row, then one row per visible collection.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let buffer = frame.buffer_mut();
    buffer.set_string(area.x, area.y, " Collections", Style::new());

    let rows = area.height.saturating_sub(HEADER_ROWS) as usize;
    let len = state.schema.collections.len();
    let first = window(state.sidebar.offset, state.sidebar.selected, len, rows);
    // Reverse video marks the selection only while the sidebar has focus,
    // which is Home and nowhere else (`docs/tui.md` §6).
    let focused = matches!(state.route, Route::Home);

    for slot in 0..rows {
        let index = first + slot;
        if index >= len {
            break;
        }
        // `as u16` is safe: `slot` is bounded by the area's own height.
        let y = area.y + HEADER_ROWS + slot as u16;

        // Nothing is clipped silently: the top row stands in for everything
        // above it, the bottom row for everything below.
        let hidden_above = (slot == 0 && first > 0).then_some(first + 1);
        let hidden_below =
            (slot + 1 == rows && first + rows < len).then(|| len - (first + rows) + 1);

        let (text, style) = match (hidden_above, hidden_below) {
            (Some(n), _) => (format!(" ↑ {n} more"), dim()),
            (_, Some(n)) => (format!(" ↓ {n} more"), dim()),
            _ => collection_row(state, index, focused),
        };

        // Padded to the full width so a reverse-video row is a bar rather than
        // a word, and so nothing can spill into the divider.
        let padded = format!("{text:<width$}", width = SIDEBAR_WIDTH as usize);
        buffer.set_string(area.x, y, padded, style);
    }
}

/// One collection's row: its marker, its label, and how it is drawn.
fn collection_row(state: &AppState, index: usize, focused: bool) -> (String, Style) {
    let label = state
        .schema
        .collections
        .get_index(index)
        // `index` is inside the map here; an empty label is the harmless
        // fallback rather than a panic.
        .map_or("", |(_, collection)| collection.label.as_str());

    let selected = index == state.sidebar.selected;
    let marker = if selected { "> " } else { "  " };
    let style = if selected && focused {
        Style::new().add_modifier(Modifier::REVERSED)
    } else {
        Style::new()
    };

    (format!(" {marker}{}", truncate(label, LABEL_WIDTH)), style)
}

fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}
