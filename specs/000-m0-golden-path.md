# SPEC-000: M0 Golden Path

Status: **Approved** · Milestone: M0 · Owner: coordinator

## Problem

M0 needs one unambiguous definition of "done" that cannot be satisfied by
partially working layers. Without it, progress is measured in components built
rather than in the product working.

## Goal

One end-to-end scenario that exercises every layer — config, validation,
storage, services, CLI, TUI — and proves data survives a restart. When this
passes, M0 is complete.

## Non-goals

Coverage. This scenario is deliberately narrow: one collection, a handful of
fields, the happy path plus one validation failure. Breadth lives in the
individual task specs.

## The scenario

Given a fresh directory and this `pressa.yaml`:

```yaml
project:
  name: blog-cms

collections:
  posts:
    label: Posts
    list_columns: [title, status, views]
    fields:
      - name: title
        type: text
        required: true
      - name: slug
        type: text
        required: true
        unique: true
      - name: status
        type: select
        required: true
        options: [draft, published]
      - name: content
        type: textarea
      - name: views
        type: number
      - name: featured
        type: boolean
      - name: published_at
        type: datetime
      - name: metadata
        type: json
```

| # | Step | Expected |
|---|---|---|
| 1 | `pressa init` in an empty directory | `pressa.yaml` with an example collection and `.pressa/` are created |
| 2 | Replace `pressa.yaml` with the schema above; run `pressa validate` | Exit code 0, "schema ok: 1 collection, 8 fields" |
| 3 | `pressa` (or `pressa dev`) | TUI opens; sidebar lists `Posts`; main panel shows the empty state |
| 4 | `Enter` on `Posts` | List view for `posts`, "No records yet. Press n to create the first one." |
| 5 | `n` | Record editor, all 8 fields rendered per their type, `Title` focused |
| 6 | `Ctrl+S` with everything blank | Not saved. Three errors shown under `Title`, `Slug` and `Status`, each reading "required" |
| 7 | Fill `Title` = "Hello world", `Slug` = "hello-world", `Status` = "published", `Views` = "42" | Editor shows the unsaved indicator |
| 8 | `Ctrl+S` | Saved; status line confirms; route returns to the list |
| 9 | — | The list shows one row: `Hello world · published · 42` |
| 10 | `Enter` on the row | Editor opens with the stored values, no unsaved indicator |
| 11 | Change `Title` to "Hello, world!", `Ctrl+S` | Saved; the list shows the new title |
| 12 | `q`, `q` | The TUI exits and the terminal is restored — no raw mode, no leftover alternate screen |
| 13 | `pressa` again, `Enter` on `Posts` | The record is still there with `Hello, world!` |
| 14 | `d`, then `y` | Confirmation appears first; the record is deleted; the empty state returns |
| 15 | `q`, `pressa`, `Enter` on `Posts` | Still empty — the deletion persisted |

## Invariants

- An invalid record is never written to the database.
- Leaving a dirty editor requires confirmation.
- Nothing is written to stdout or stderr while the TUI is running.
- The terminal is restored on exit, on error and on panic.

## Acceptance criteria

- [ ] Every step above behaves as described in a real terminal.
- [ ] An automated end-to-end test drives steps 2–15 through the services and
      `update()` (no pty), asserting state and database contents at each step.
- [ ] `examples/blog/` contains this exact schema and is what `just run` opens.

## Tests

- `tests/golden_path.rs` in the workspace root: a temp directory, a real
  `SqliteRepository`, the real services, and `update()` driven by the same
  `Command` sequence a user's keystrokes would produce. Steps 12–13 are
  modelled by dropping the repository and reopening the database file.
- CLI steps (1, 2) via `assert_cmd`.
- The rendered screens at steps 3, 4, 5, 6, 9 are `insta` snapshots.

## Open questions

None. This spec is frozen for M0; changing it means changing what M0 means.
