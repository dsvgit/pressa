# ADR-0001: Use ratatui + crossterm for the terminal UI

Status: **Accepted** · Date: 2026-09-06

## Context

We need a terminal UI capable of a persistent sidebar, tables, forms, overlays
and a context-sensitive hint bar, driven entirely by the keyboard. The UI must
be rendered from a schema at runtime rather than laid out per entity, and it
must be testable without a real terminal, because most of the code will be
written by AI agents that cannot look at a screen.

Options considered: `ratatui` + `crossterm`, `cursive`, `tui-realm` (a
component framework on top of ratatui).

## Decision

Use **ratatui** with the **crossterm** backend.

## Rationale

- Immediate-mode rendering matches the core requirement exactly: rendering is a
  pure function of state, and "draw a table for whatever collection this is"
  is a function call, not a widget hierarchy to construct and keep in sync.
- `ratatui::TestBackend` renders into an inspectable buffer. Combined with
  `insta` this gives a deterministic pass/fail signal for UI work with no
  human in the loop — the single most important property for this project.
- crossterm is cross-platform and is ratatui's default backend; termion is
  Unix-only and less maintained.
- The ecosystem is large (gitui, bottom, zellij), so patterns for scrolling,
  layout and input handling are findable.

`cursive` is more widget-oriented and ships form controls, which would speed up
the editor, but it takes control of the render loop and does not offer an
equivalent snapshot-testing story. That trade goes the wrong way here.

`tui-realm` was rejected for M0: it inserts a component abstraction between us
and the buffer we snapshot-test, and its value appears at a scale we have not
reached.

## Consequences

- Text input widgets are not free. We use `tui-input` for single-line and
  `tui-textarea` for multi-line rather than implementing cursor handling — both
  are additions to be made in T9, under this ADR.
- Scrolling, focus and viewport clamping are ours to write and to test.
- Every screen must be renderable from `AppState` alone, since `view` gets
  nothing else.
