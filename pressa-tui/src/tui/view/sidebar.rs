//! The sidebar: the schema's collections, with a selection and a viewport.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::tui::state::{AppState, Route};
use crate::tui::view::SIDEBAR_WIDTH;

/// One leading space and a two-column marker leave this much for a label.
const LABEL_WIDTH: usize = SIDEBAR_WIDTH as usize - 3;
/// How many rows the title and the blank line under it take.
const HEADER_ROWS: u16 = 2;

/// The first collection the list area shows.
///
/// `offset` is what the state remembers; the list's height is only known while
/// drawing, so this clamps rather than mutates and `view` stays pure.
pub fn window(offset: usize, selected: usize, len: usize, height: usize) -> usize {
    if height == 0 {
        return 0;
    }

    // Never scroll past the point where the last collection is on the bottom
    // row, or the list would end in blank rows with content hidden above.
    let furthest = len.saturating_sub(height);
    let mut first = offset.min(furthest);

    if selected < first {
        first = selected; // the selection went off the top
    }
    if selected >= first + height {
        first = selected + 1 - height; // and off the bottom
    }

    first
}

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

/// `label` in at most `width` columns, ending in `…` when it does not fit.
fn truncate(label: &str, width: usize) -> String {
    // By characters, not bytes: a label may be more than ASCII.
    if label.chars().count() <= width {
        return label.to_string();
    }

    // One column goes to the ellipsis that says there was more.
    let kept: String = label.chars().take(width - 1).collect();
    format!("{kept}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_that_fits_never_scrolls() {
        assert_eq!(window(0, 4, 5, 15), 0);
    }

    #[test]
    fn the_window_follows_the_selection_down() {
        // Twenty collections in fifteen rows, selection on the last: the first
        // visible row is 5, leaving six above it to be summarised.
        assert_eq!(window(0, 19, 20, 15), 5);
    }

    #[test]
    fn the_window_follows_the_selection_up() {
        assert_eq!(window(10, 3, 20, 15), 3);
    }

    #[test]
    fn the_window_never_runs_past_the_end() {
        assert_eq!(window(18, 19, 20, 15), 5);
    }

    #[test]
    fn a_zero_height_list_asks_for_nothing() {
        assert_eq!(window(0, 0, 20, 0), 0);
    }
}
