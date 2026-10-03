//! The frame every screen lives in: border, separators, status line, hint bar.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::tui::keymap::{Context, hints};
use crate::tui::state::{StatusKind, StatusMessage};
use crate::tui::view::{MIN_HEIGHT, MIN_WIDTH, SIDEBAR_WIDTH, centred};

/// The hint bar joins its entries with three spaces (SPEC-006 "Hints").
const HINT_SEPARATOR: &str = "   ";

/// How many `─` sit between the right-aligned title segment and the corner.
const TITLE_MARGIN: u16 = 3;

/// Border, title and the three separator rules.
///
/// `right` is the segment the list view puts at the other end of the top edge —
/// its record count. It is dropped rather than overlapped when the breadcrumbs
/// reach it: the breadcrumbs say where you are, which matters more than a count
/// (SPEC-007 "The table, stated once").
pub fn shell(frame: &mut Frame, area: Rect, title: &str, right: Option<&str>) {
    // The outer box first: corners, the title on the top edge, and the walls.
    frame.render_widget(Block::bordered().title(Line::from(title.to_string())), area);

    let divider = area.x + 1 + SIDEBAR_WIDTH;
    let buffer = frame.buffer_mut();

    if let Some(segment) = right {
        // Measured in characters, not bytes: `─` is three bytes and one column.
        let width = segment.chars().count() as u16;
        let x = area.right().saturating_sub(1 + TITLE_MARGIN + width);
        // The title starts one column in from the corner; `>` and not `>=` so
        // the two never share a column.
        if x > area.x + 1 + title.chars().count() as u16 {
            buffer.set_string(x, area.y, segment, Style::new());
        }
    }

    // The divider between sidebar and main panel, down the body only.
    for y in (area.y + 2)..(area.bottom() - 5) {
        set(buffer, divider, y, '│');
    }

    // Under the header, over the status line, and over the hint bar. Only the
    // first two meet the divider, so only they carry a junction.
    rule(buffer, area, area.y + 1, Some((divider, '┬')));
    rule(buffer, area, area.bottom() - 5, Some((divider, '┴')));
    rule(buffer, area, area.bottom() - 3, None);
}

/// The status line: nothing, or a message.
pub fn status(frame: &mut Frame, area: Rect, status: Option<&StatusMessage>) {
    // `let … else` because an absent message is the ordinary case, not an error.
    let Some(message) = status else {
        return;
    };

    let (prefix, style) = match message.kind {
        StatusKind::Error => ("⚠ ", Style::new().fg(Color::Red)),
        StatusKind::Info => ("", Style::new()),
    };
    // Two spans so the one leading space stays unstyled and the message alone
    // carries the colour.
    let line = Line::from(vec![
        Span::raw(" "),
        Span::styled(format!("{prefix}{}", message.text), style),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}

/// The hint bar, generated from the keymap for `context`. There is no literal
/// hint text here: every word comes from a binding.
pub fn hint_bar(frame: &mut Frame, area: Rect, context: Context) {
    let text = format!(" {}", hints(context).join(HINT_SEPARATOR));
    frame.render_widget(
        Paragraph::new(text).style(Style::new().add_modifier(Modifier::DIM)),
        area,
    );
}

/// Screen G: two centred lines and nothing else, redrawn on every resize until
/// the terminal is big enough. `x` rather than `×`, which `docs/tui.md` §6's
/// character set does not allow.
pub fn too_small(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from("terminal too small"),
        Line::from(format!(
            "needs {MIN_WIDTH}x{MIN_HEIGHT}, this is {}x{}",
            area.width, area.height
        )),
    ];

    centred(frame, area, lines);
}

/// `├───┬───┤` across `area` at row `y`, with an optional junction column.
fn rule(buffer: &mut Buffer, area: Rect, y: u16, junction: Option<(u16, char)>) {
    for x in area.left()..area.right() {
        let symbol = match junction {
            // The junction wins wherever it falls, which is never an edge.
            Some((column, joint)) if x == column => joint,
            _ if x == area.left() => '├',
            _ if x == area.right() - 1 => '┤',
            _ => '─',
        };
        set(buffer, x, y, symbol);
    }
}

/// One cell. `cell_mut` returns `None` off the buffer, which is a no-op here
/// rather than a panic.
fn set(buffer: &mut Buffer, x: u16, y: u16, symbol: char) {
    if let Some(cell) = buffer.cell_mut((x, y)) {
        cell.set_char(symbol);
    }
}
