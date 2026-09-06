# SPEC-008: Record editor

Status: **Draft** · Task: T9 · Crate: `pressa-tui`
· ADRs: [0004](../docs/adr/0004-schema-driven-ui.md), [0005](../docs/adr/0005-command-and-keymap-architecture.md)

> Draft. Finalise immediately before T9 starts. **This task closes the Golden
> Path** — it is the point where the product first works end to end.

## Goal

One form renderer that edits any collection's record, with an editor widget per
field type, dirty tracking, save, and per-field validation errors.

## Scope

- Form generated from `Collection.fields`; `*` marks required.
- Two modes: `Editor` (move between fields) and `EditorInput` (type into one) —
  see [`tui.md`](../docs/tui.md) §3.
- Editors: `Text` single line, `Textarea` 5-row box, `Number` text parsed on
  commit, `Boolean` toggle, `Select` cycler, `DateTime` RFC 3339 text, `Json`
  textarea parsed on commit.
- Dirty indicator in the header; `Esc` while dirty opens `ConfirmDiscard`.
- `Ctrl+S` → `Effect::SaveRecord`; on `SaveFailed`, errors render under their
  fields and the route does not change.
- `Esc` inside `EditorInput` restores the field's original value.
- Both routes (`New` and `Edit`) use the same renderer, differing only in
  whether the draft came from `blank()` or from `get()`.

## States

`Clean → Dirty → Saving → Saved` and `Dirty → ValidationError → Dirty`.

## Invariants

- An invalid record is never sent to the service twice for the same keystroke.
- Leaving a dirty form always requires confirmation.
- Save is all-or-nothing: a failed save leaves the draft exactly as it was.

## Acceptance criteria

To be written. Must include: a snapshot per field type in both focused and
unfocused states; a snapshot of the form with three validation errors; a
pure-`update` test that `Ctrl+S` emits `SaveRecord` with the draft; a test that
`Esc` in input mode restores the original value; and the Golden Path steps 5–11
passing.
