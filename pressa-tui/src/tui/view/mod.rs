//! Drawing. Pure: reads the state, mutates nothing, performs no I/O.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use pressa_app::domain::Schema;

use crate::counted;
use crate::tui::keymap::context_for;
use crate::tui::state::{AppState, Route, collection_label};

pub mod chrome;
pub mod sidebar;

/// The smallest terminal that still gets a layout; below it, screen G.
pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 16;
/// Fixed: with a 60-column floor the sidebar never has to shrink, so
/// `docs/tui.md` §4's minimum of 14 is never reached (SPEC-006 "Non-goals").
pub const SIDEBAR_WIDTH: u16 = 20;
/// The root of every breadcrumb trail.
const ROOT_CRUMB: &str = "pressa";

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
pub fn breadcrumbs(route: &Route, schema: &Schema) -> Vec<String> {
    match route {
        Route::Home => vec![ROOT_CRUMB.to_string()],
        Route::List { collection } => vec![
            ROOT_CRUMB.to_string(),
            collection_label(schema, collection).to_string(),
        ],
    }
}

/// Draws the whole screen.
pub fn view(state: &AppState, frame: &mut Frame) {
    let area = frame.area();

    // Too small to lay out: say so rather than draw something broken.
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        chrome::too_small(frame, area);
        return;
    }

    let title = format!(" {} ", breadcrumbs(&state.route, &state.schema).join(" › "));
    chrome::shell(frame, area, &title);

    let panels = layout(area);
    sidebar::render(frame, panels.sidebar, state);
    main_panel(frame, panels.main, state);
    chrome::status(frame, panels.status, state.status.as_ref());
    chrome::hint_bar(frame, panels.hints, context_for(&state.route));
}

/// The panel every later screen fills. T7 draws the empty state and, on a
/// route T8 owns, says whose it is.
fn main_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let lines = match state.route {
        Route::Home => vec![
            Line::from("Select a collection to begin."),
            Line::from(""),
            Line::from(format!(
                "{} · {}",
                state.schema.project.name,
                counted(state.schema.collections.len(), "collection")
            )),
        ],
        Route::List { .. } => vec![Line::from("The list view arrives in T8.")],
    };

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
