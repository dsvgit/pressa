# SPEC-007: List view

Status: **Draft** · Task: T8 · Crate: `pressa-tui`
· ADR: [0004](../docs/adr/0004-schema-driven-ui.md)

> Draft. Finalise immediately before T8 starts.

## Goal

One table renderer that displays any collection's records using its
`list_columns`. No collection-specific code paths — see
[ADR-0004](../docs/adr/0004-schema-driven-ui.md).

## Scope

- Headers from `list_columns`; proportional widths, minimum 6, ellipsis on
  overflow.
- Cell rendering per field type: `Boolean` as `✓`/`·`, `Json` as `{n}`, absent
  as a dim `—`, `DateTime` as a relative time.
- Navigation: `j`/`k`, `g`/`G`, `Ctrl+D`/`Ctrl+U`, with viewport scrolling and
  selection clamped to the row count.
- `Enter` → `Command::EditRecord`, `n` → `NewRecord`, `r` → `Refresh`.
- Empty state; loading state.
- Record count in the header.

Out of scope: sorting UI, filtering (T11), pagination controls, bulk selection.

## Acceptance criteria

To be written. Must include: snapshots of the initial render, after two `j`,
and of the empty state; a pure-`update` test that `Enter` on row 1 emits
`EditRecord` with that row's id; a test that a schema with a different
collection renders through the same function.
