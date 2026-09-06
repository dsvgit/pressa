# SPEC-006: TUI shell

Status: **Draft** · Task: T7 · Crate: `pressa-tui`
· ADR: [0005](../docs/adr/0005-command-and-keymap-architecture.md)

> Draft. Finalise immediately before T7 starts.

## Goal

The frame everything else renders inside: terminal lifecycle, layout, sidebar,
derived breadcrumbs, status line, generated hint bar, and the keymap table that
the hint bar and help are built from.

## Scope

- `TerminalGuard`: raw mode + alternate screen on enter, restored on drop, on
  error and on panic (panic hook installed).
- Layout per [`tui.md`](../docs/tui.md) §4; "terminal too small" below 60×16.
- `AppState`, `Route`, `Command`, `Effect`, `update()` skeleton.
- Sidebar listing collections in schema order, with selection.
- Breadcrumbs derived from `Route`, never hand-assembled.
- The static keymap table; hint bar generated from it by context.
- `q` at Home quits; `Ctrl+C` always quits.

Out of scope: the record table (T8), the editor (T9), overlays (T10–T11).

## Acceptance criteria

To be written. Must include: an 80×24 `insta` snapshot of Home; a test that
panicking inside the guard still restores the terminal; a test that the hint
bar text is derived from the keymap rather than a literal.
