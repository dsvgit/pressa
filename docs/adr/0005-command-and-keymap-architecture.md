# ADR-0005: All user actions go through a Command enum and a keymap table

Status: **Accepted** · Date: 2026-09-06

## Context

The default way TUI input handling grows is `match key { KeyCode::Char('j') =>
… }` scattered across screens. Key handling then lives in a dozen places, the
help text is written separately and drifts, and rebinding is a refactor.

## Decision

- Every user action is a variant of one `Command` enum.
- Key-to-command mapping lives in **one static table** of `KeyBinding
  { context, key, command, description, show_in_hint_bar }`.
- The bottom hint bar and the `?` help overlay are **generated** from that
  table, filtered by the current context.
- `update(&mut AppState, Command) -> Vec<Effect>` is the only place state
  changes, and it never performs I/O.

## Rationale

- The set of things a user can do becomes enumerable and reviewable in one
  place — which is also the answer to "what does this application actually do?"
- Help cannot go stale: a binding that is not in the table does not exist, and
  one that is in the table is documented by construction.
- `update` being pure makes every interaction testable with no terminal and no
  database. This is what makes UI work verifiable by an agent.
- Vim mode, emacs mode and user-configurable bindings later are a different
  table, not a rewrite.

## Consequences

- Adding a keystroke means adding a `Command` variant, a table row and an
  `update` arm. Slightly more ceremony per key, and the ceremony is the point.
- `Context` (Sidebar / List / Editor / EditorInput / Overlay) must be derivable
  from `AppState`, so lookup is unambiguous.
- Effects must cover every I/O the UI can trigger; anything not expressible as
  an `Effect` cannot be done from the UI, by design.
