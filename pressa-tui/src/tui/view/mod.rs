//! Drawing. Pure: reads the state, mutates nothing, performs no I/O.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::counted;
use crate::tui::command::Command;
use crate::tui::keymap::{Context, context_for, key_for};
use crate::tui::state::{AppState, Load, Route, StatusKind, StatusMessage, collection_label};

pub mod chrome;
pub mod form;
pub mod overlay;
pub mod sidebar;
pub mod table;

/// The smallest terminal that still gets a layout; below it, screen G.
pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 16;
/// Fixed: with a 60-column floor the sidebar never has to shrink, so
/// `docs/tui.md` §4's minimum of 14 is never reached (SPEC-006 "Non-goals").
pub const SIDEBAR_WIDTH: u16 = 20;
/// The root of every breadcrumb trail.
const ROOT_CRUMB: &str = "pressa";
/// The editor's third crumb: at most this wide, else the id this short, else
/// this word (SPEC-008 Q8).
const CRUMB_WIDTH: usize = 20;
const SHORT_ID: usize = 6;
const NEW_CRUMB: &str = "New";

/// The first row a windowed list shows.
///
/// `offset` is what the state remembers; a list's height is only known while
/// drawing, so this clamps rather than mutates and `view` stays pure. Shared:
/// the table windows its records the way the sidebar windows its collections.
pub fn window(offset: usize, selected: usize, len: usize, height: usize) -> usize {
    if height == 0 {
        return 0;
    }

    // Never scroll past the point where the last row is on the bottom line, or
    // the list would end in blanks with content hidden above.
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

/// `text` in at most `width` columns, ending in `…` when it does not fit.
///
/// Shared for the same reason as [`window`]: a cell too wide for its column and
/// a label too wide for the sidebar are cut the same way.
pub fn truncate(text: &str, width: usize) -> String {
    // By characters, not bytes: a label or a cell may be more than ASCII.
    if text.chars().count() <= width {
        return text.to_string();
    }

    // One column goes to the ellipsis that says there was more.
    let kept: String = text.chars().take(width.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// The four regions every screen draws into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panels {
    pub sidebar: Rect,
    pub main: Rect,
    pub status: Rect,
    pub hints: Rect,
}

/// `docs/tui.md` §4, in coordinates.
///
/// Seven of the rows are chrome — header, three separators, status, hints and
/// the bottom border — so the body gets what is left. Three of the columns are
/// the two walls and the divider.
pub fn layout(area: Rect) -> Panels {
    let body = area.height.saturating_sub(7);
    let main = area.width.saturating_sub(SIDEBAR_WIDTH + 3);

    Panels {
        sidebar: Rect::new(area.x + 1, area.y + 2, SIDEBAR_WIDTH, body),
        main: Rect::new(area.x + SIDEBAR_WIDTH + 2, area.y + 2, main, body),
        // Counted from the bottom: status, separator, hints, bottom border.
        status: Rect::new(area.x + 1, area.bottom() - 4, area.width - 2, 1),
        hints: Rect::new(area.x + 1, area.bottom() - 2, area.width - 2, 1),
    }
}

/// The header path for a route, derived and never assembled by hand
/// (`docs/tui.md` §2).
///
/// Total: a slug the schema does not know is printed as the slug, so a stale
/// route is visible rather than fatal.
pub fn breadcrumbs(state: &AppState) -> Vec<String> {
    let schema = &state.schema;
    match &state.route {
        Route::Home => vec![ROOT_CRUMB.to_string()],
        Route::List { collection } => vec![
            ROOT_CRUMB.to_string(),
            collection_label(schema, collection).to_string(),
        ],
        Route::New { collection } | Route::Edit { collection, .. } => vec![
            ROOT_CRUMB.to_string(),
            collection_label(schema, collection).to_string(),
            record_crumb(state, collection),
        ],
    }
}

/// The editor's third crumb: the draft's first `list_columns` value through
/// the table's own cell renderer, cut to 20 characters; the shortened id at
/// `Edit` and `New` at `New` when that value is absent (SPEC-008 Q8).
///
/// Read from the draft, so it follows a rename as each field is committed.
fn record_crumb(state: &AppState, slug: &str) -> String {
    let named = state.schema.collections.get(slug).and_then(|collection| {
        // The first list column's field; `find` because columns are names.
        let name = collection.list_columns.first()?;
        let field = collection.fields.iter().find(|field| &field.name == name)?;
        let cell = table::cell(&field.kind, state.editor.draft.get(name.as_str()));
        // The absent dash cannot name a record.
        (cell.text != table::ABSENT).then(|| truncate(&cell.text, CRUMB_WIDTH))
    });

    // `unwrap_or_else` builds the fallback only when the draft named nothing.
    named.unwrap_or_else(|| match state.editor.id {
        Some(id) => {
            let short: String = id.to_string().chars().take(SHORT_ID).collect();
            format!("{short}…")
        }
        None => NEW_CRUMB.to_string(),
    })
}

/// Draws the whole screen.
pub fn view(state: &AppState, frame: &mut Frame) {
    let area = frame.area();

    // Too small to lay out: say so rather than draw something broken.
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        chrome::too_small(frame, area);
        return;
    }

    let title = format!(" {} ", breadcrumbs(state).join(" › "));
    // `as_deref` hands `shell` a `&str` without moving the `String` it owns.
    chrome::shell(frame, area, &title, title_segment(state).as_deref());

    let panels = layout(area);
    sidebar::render(frame, panels.sidebar, state);
    main_panel(frame, panels.main, state);
    // The attention line, when there is one, wins over an older message.
    let attention = attention(state);
    chrome::status(
        frame,
        panels.status,
        attention.as_ref().or(state.status.as_ref()),
    );
    chrome::hint_bar(frame, panels.hints, context_for(state));
}

/// The right-hand segment of the window title, if any: a list's record count,
/// or an editor's unsaved marker.
///
/// The count is omitted unless the list's load succeeded: the title must not
/// report 0 records for a collection whose contents are unknown (frame G). The
/// marker shows only while `draft != original` (SPEC-008 Q5), and names the
/// save key from the keymap rather than spelling it.
fn title_segment(state: &AppState) -> Option<String> {
    match (&state.route, state.list.load) {
        (Route::List { .. }, Load::Ok) => {
            Some(format!(" {} ", counted(state.list.records.len(), "record")))
        }
        (Route::New { .. } | Route::Edit { .. }, _) if state.editor.is_dirty() => {
            key_for(Context::Editor, &Command::Save).map(|key| format!(" ● unsaved · {key} "))
        }
        _ => None,
    }
}

/// `3 fields need attention`, derived from the editor's errors rather than
/// stored (SPEC-008 Q11): it cannot go stale. Info-styled, so no `⚠`.
fn attention(state: &AppState) -> Option<StatusMessage> {
    let n = state.editor.errors.len();
    if state.route.editing().is_none() || n == 0 {
        return None;
    }
    // The verb agrees with the noun as well.
    let verb = if n == 1 { "needs" } else { "need" };
    Some(StatusMessage {
        text: format!("{} {verb} attention", counted(n, "field")),
        kind: StatusKind::Info,
    })
}

/// The panel every screen fills: the empty state at Home, the table in a
/// list, the form in an editor with any overlay on top of it.
fn main_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    // A route naming a collection the schema does not know falls through to
    // the Home body rather than panicking — a stale route is visible, as the
    // breadcrumbs already make it.
    match &state.route {
        Route::List { collection } => {
            if let Some(collection) = state.schema.collections.get(collection) {
                table::render(frame, area, state, collection);
                return;
            }
        }
        Route::New { collection } | Route::Edit { collection, .. } => {
            if let Some(collection) = state.schema.collections.get(collection) {
                form::render(frame, area, state, collection);
                // Drawn last, over the form, which stays visible around it.
                if let Some(overlay) = &state.overlay {
                    overlay::render(frame, area, overlay);
                }
                return;
            }
        }
        Route::Home => {}
    }

    let lines = vec![
        Line::from("Select a collection to begin."),
        Line::from(""),
        Line::from(format!(
            "{} · {}",
            state.schema.project.name,
            counted(state.schema.collections.len(), "collection")
        )),
    ];

    centred(frame, area, lines);
}

/// A block of lines, centred horizontally and vertically in `area`.
pub fn centred(frame: &mut Frame, area: Rect, lines: Vec<Line>) {
    // `as u16` is safe: these blocks are two or three lines long.
    let height = lines.len() as u16;
    if area.height < height {
        return;
    }

    let block = Rect::new(
        area.x,
        area.y + (area.height - height) / 2,
        area.width,
        height,
    );
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), block);
}

#[cfg(test)]
mod tests {
    use super::*;

    // `window`'s own tests, moved here with it from `view::sidebar`: the table
    // and the sidebar now share the function, so they share these.

    #[test]
    fn a_list_that_fits_never_scrolls() {
        assert_eq!(window(0, 4, 5, 15), 0);
    }

    #[test]
    fn the_window_follows_the_selection_down() {
        // Twenty rows in fifteen, selection on the last: the first visible row
        // is 5, leaving six above it to be summarised.
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

    #[test]
    fn text_that_fits_is_left_alone_and_text_that_does_not_ends_in_an_ellipsis() {
        assert_eq!(truncate("draft", 6), "draft");
        assert_eq!(truncate("published", 6), "publi…");
        // By characters, not bytes: `…` is three bytes and one column.
        assert_eq!(truncate("ünïcödé", 4).chars().count(), 4);
    }
}
