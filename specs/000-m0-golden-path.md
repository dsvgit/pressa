# SPEC-000: M0 Golden Path

Status: **Approved** · Task: T12 · Milestone: M0 · Crates: all
· ADRs: [0004](../docs/adr/0004-schema-driven-ui.md),
[0005](../docs/adr/0005-command-and-keymap-architecture.md),
[0007](../docs/adr/0007-vertical-slice-first.md)

## Problem

M0 needs one unambiguous definition of "done" that cannot be satisfied by
partially working layers. Without it, progress is measured in components built
rather than in the product working.

## Goal

One end-to-end scenario that exercises every layer — config, validation,
storage, services, CLI, TUI — and proves data survives a restart. When this
passes, M0 is complete.

## Non-goals

**Coverage.** This scenario is deliberately narrow: one collection, a handful of
fields, the happy path plus one validation failure. Breadth lives in the
individual task specs. Specifically, a reader may reasonably expect the
following here and will not get it:

| Not here | Where it lives instead |
|---|---|
| Every field type *edited*. The scenario types only into `text`, `select` and `number`; `textarea`, `boolean`, `datetime` and `json` are rendered but never modified | per-widget snapshots in [008](008-record-editor.md) |
| Search (`/`) and the help overlay (`?`) | T11, spec 010 |
| A second collection, and switching between them. `examples/blog` was trimmed to `posts` alone so that the example and this scenario cannot drift apart | a second collection returns when there is a feature that needs one |
| Sorting, filtering, pagination, bulk operations | M1, [roadmap](../docs/roadmap.md) §3 |
| A pty or terminal-emulator test of step 12. The automated test drives `update()` and a `TestBackend`, never a real terminal | `TerminalGuard` panic-restore test in [006](006-tui-shell.md) |
| `Ctrl+C`, `--log-level`, and every CLI flag the scenario does not press | [005](005-cli.md) |
| Uniqueness rejection. `slug` is declared `unique: true`, but the scenario never creates a second record, so the rule is never triggered | [004](004-app-services.md) |
| Terminal sizes other than 80×24, performance, dataset size | not in M0 |

**And: no new behaviour.** Every step below must already be implemented by
T1–T11. T12 wires up `examples/blog`, the README and this test; it is where the
slice is *proven*, not where it is built. If a step turns out to need
production code that does not exist, that code belongs to the task that owns
that screen — stop and say so rather than adding it here.

## User stories

- As a user, I want to declare a collection in `pressa.yaml` and immediately get
  a working list and editor for it, so that I never write UI code for my own
  data.
- As a user, I want a record I saved to still be there after I quit and reopen
  the app, so that I can trust it with real content.
- As a user, I want a save that would store an invalid record to be refused with
  the reason under the field it belongs to, so that I can fix it without
  guessing.
- As the coordinator, I want one scenario whose pass or fail decides whether M0
  is done, so that "done" is not a judgement call about how many components
  exist.

## The scenario

Given a fresh directory and this `pressa.yaml` — the same file as
`examples/blog/pressa.yaml`:

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

| # | Step | Expected | Screen |
|---|---|---|---|
| 1 | `pressa init` in an empty directory | `pressa.yaml` with an example collection and `.pressa/` are created | — |
| 2 | Replace `pressa.yaml` with the schema above; run `pressa validate` | Exit code 0, "schema ok: 1 collection, 8 fields" | — |
| 3 | `pressa` (or `pressa dev`) | TUI opens; sidebar lists `Posts`; main panel shows the empty state | [A](#a--home-step-3) |
| 4 | `Enter` on `Posts` | List view for `posts`, "No records yet. Press n to create the first one." | [B](#b--list-empty-step-4) |
| 5 | `n` | Record editor, all 8 fields rendered per their type, `Title` focused | [C](#c--new-record-blank-step-5) |
| 6 | `Ctrl+S` with everything blank | Not saved. Three errors shown under `Title`, `Slug` and `Status`, each reading "required" | [D](#d--save-rejected-step-6) |
| 7 | Fill `Title` = "Hello world", `Slug` = "hello-world", `Status` = "published", `Views` = "42" | Editor shows the unsaved indicator | [E](#e--filled-and-dirty-step-7) |
| 8 | `Ctrl+S` | Saved; status line confirms; route returns to the list | [F](#f--list-after-save-steps-89) |
| 9 | — | The list shows one row: `Hello world · published · 42` | [F](#f--list-after-save-steps-89) |
| 10 | `Enter` on the row | Editor opens with the stored values, no unsaved indicator | [G](#g--editing-a-stored-record-step-10) |
| 11 | Change `Title` to "Hello, world!", `Ctrl+S` | Saved; the list shows the new title | [H](#h--list-after-rename-steps-11-and-13) |
| 12 | `q`, `q` | The TUI exits and the terminal is restored — no raw mode, no leftover alternate screen | — |
| 13 | `pressa` again, `Enter` on `Posts` | The record is still there with `Hello, world!` | [H](#h--list-after-rename-steps-11-and-13) |
| 14 | `d`, then `y` | Confirmation appears first; the record is deleted; the empty state returns | [I](#i--confirm-delete-step-14) |
| 15 | `q`, `q`, `pressa`, `Enter` on `Posts` | Still empty — the deletion persisted | [J](#j--empty-after-delete-step-15) |

`q` never quits from a list: per the keymap ([`tui.md`](../docs/tui.md) §3) it is
`Back`, and only `Back` at Home quits. Steps 12 and 15 therefore both press it
twice.

### Command sequence

The automated test drives `update()` with the commands these keystrokes resolve
to. Effects are executed against the real services and their results fed back
as commands.

```
step 4   Select
step 5   NewRecord
step 6   Save                       -> SaveRecord -> SaveFailed(errors)
step 7   BeginEdit, InputChar x11, CommitField               (title)
         NextField, BeginEdit, InputChar x11, CommitField    (slug)
         NextField, NextOption x2                            (status: - > draft > published)
         NextField x2, BeginEdit, InputChar x2, CommitField  (views)
step 8   Save                       -> SaveRecord -> RecordSaved -> LoadRecords
step 10  EditRecord                 -> LoadRecord -> RecordLoaded
step 11  BeginEdit, InputChar/InputBackspace..., CommitField, Save
step 12  Back, Quit
step 13  re-open the repository, LoadRecords
step 14  DeleteRecord, Confirm      -> DeleteRecord -> RecordDeleted -> LoadRecords
step 15  Back, Quit, re-open the repository, LoadRecords
```

`NextOption` is pressed twice at step 7 because `blank()` leaves a required
`Select` empty ([004](004-app-services.md)); the first press selects `draft`,
the second `published`.

## UX

Every screen the scenario reaches, at exactly 80×24 — the size the `insta`
snapshots are taken at. The frames are normative for layout, wording and field
order. Two details in them are drawn from behaviour that spec
[008](008-record-editor.md) owns and must match whatever 008 settles on: the
`↑ n more` / `↓ n more` scroll markers, and how an empty `Select` renders
(drawn here as `‹ — ›`, following the dim `—` used for absent values in
[`tui.md`](../docs/tui.md) §5.2).

### A — Home (step 3)

```text
┌ pressa ──────────────────────────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │                                                         │
│                    │                                                         │
│ > Posts            │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │              Select a collection to begin.              │
│                    │                                                         │
│                    │                 blog-cms · 1 collection                 │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
├────────────────────┴─────────────────────────────────────────────────────────┤
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Navigate   Enter Open   ? Help   q Quit                                   │
└──────────────────────────────────────────────────────────────────────────────┘
```

Wording and singular/plural forms of `blog-cms · 1 collection` belong to
[006](006-tui-shell.md).

### B — List, empty (step 4)

```text
┌ pressa › Posts ──────────────────────────────────────────────── 0 records ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  TITLE                  STATUS         VIEWS            │
│                    │ ─────────────────────────────────────────────────────── │
│ > Posts            │                                                         │
│                    │   No records yet.  Press n to create the first one.     │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
├────────────────────┴─────────────────────────────────────────────────────────┤
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Navigate  Enter Edit  n New  d Delete  / Search  Esc Back  ? Help         │
└──────────────────────────────────────────────────────────────────────────────┘
```

Three columns, because `list_columns` names three. The table appends no
implicit column of its own ([`tui.md`](../docs/tui.md) §5.2).

### C — New record, blank (step 5)

```text
┌ pressa › Posts › New ────────────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  Title *                                                │
│                    │  ┌───────────────────────────────────────────────────┐  │
│ > Posts            │  │ ▌                                                │   │
│                    │  └───────────────────────────────────────────────────┘  │
│                    │                                                         │
│                    │  Slug *                                                 │
│                    │    —                                                    │
│                    │                                                         │
│                    │  Status *        ‹ — ›                                  │
│                    │                                                         │
│                    │  Content                                                │
│                    │    —                                                    │
│                    │                                                         │
│                    │  Views                                                  │
│                    │    —                                                    │
│                    │                                                         │
│                    │  Featured        [ ]                          ↓ 2 more  │
├────────────────────┴─────────────────────────────────────────────────────────┤
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ Tab Next  Enter Edit  Ctrl+S Save  Esc Back  ? Help                          │
└──────────────────────────────────────────────────────────────────────────────┘
```

The focused field is boxed; unfocused fields are drawn as plain values, so a
`Textarea` occupies one line until it is focused ([`tui.md`](../docs/tui.md)
§5.3). Even so the form is ~24 rows tall and the body is 17, so `Featured`,
`Published at` and `Metadata` start below the fold and the form scrolls to
follow focus.

`Status` is empty rather than pre-filled with `draft`: `RecordService::blank`
leaves a required `Select` unset ([004](004-app-services.md)), so the user
chooses it and step 6 can report it as missing.

### D — Save rejected (step 6)

```text
┌ pressa › Posts › New ────────────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  Title *                                                │
│                    │  ┌───────────────────────────────────────────────────┐  │
│ > Posts            │  │ ▌                                                │   │
│                    │  └───────────────────────────────────────────────────┘  │
│                    │  ⚠ required                                             │
│                    │                                                         │
│                    │  Slug *                                                 │
│                    │    —                                                    │
│                    │  ⚠ required                                             │
│                    │                                                         │
│                    │  Status *        ‹ — ›                                  │
│                    │  ⚠ required                                             │
│                    │                                                         │
│                    │  Content                                                │
│                    │    —                                                    │
│                    │                                                         │
│                    │  Views                                        ↓ 3 more  │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ 3 fields need attention                                                      │
├──────────────────────────────────────────────────────────────────────────────┤
│ Tab Next  Enter Edit  Ctrl+S Save  Esc Back  ? Help                          │
└──────────────────────────────────────────────────────────────────────────────┘
```

Three errors, one per required field, because nothing in the blank draft
satisfies `required`. The route does not change and the draft is untouched.

### E — Filled and dirty (step 7)

```text
┌ pressa › Posts › New ───────────────────────────────── ● unsaved · Ctrl+S ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  Status *        ‹ published ›                ↑ 2 more  │
│                    │                                                         │
│ > Posts            │  Content                                                │
│                    │    —                                                    │
│                    │                                                         │
│                    │  Views                                                  │
│                    │  ┌───────────────────────────────────────────────────┐  │
│                    │  │ 42▌                                              │   │
│                    │  └───────────────────────────────────────────────────┘  │
│                    │                                                         │
│                    │  Featured        [ ]                                    │
│                    │                                                         │
│                    │  Published at                                           │
│                    │    —                                                    │
│                    │                                                         │
│                    │  Metadata                                               │
│                    │    —                                                    │
├────────────────────┴─────────────────────────────────────────────────────────┤
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ Tab Next  Enter Edit  Ctrl+S Save  Esc Back  ? Help                          │
└──────────────────────────────────────────────────────────────────────────────┘
```

The viewport has followed focus down to `Views`. The header carries the unsaved
indicator; the hint bar is the `Editor` context, because `CommitField` has
returned from `EditorInput`.

### F — List after save (steps 8–9)

```text
┌ pressa › Posts ───────────────────────────────────────────────── 1 record ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  TITLE                  STATUS         VIEWS            │
│                    │ ─────────────────────────────────────────────────────── │
│ > Posts            │ ▸Hello world           published         42             │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ Record saved.                                                                │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Navigate  Enter Edit  n New  d Delete  / Search  Esc Back  ? Help         │
└──────────────────────────────────────────────────────────────────────────────┘
```

### G — Editing a stored record (step 10)

```text
┌ pressa › Posts › Edit ───────────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  Title *                                                │
│                    │  ┌───────────────────────────────────────────────────┐  │
│ > Posts            │  │ Hello world▌                                     │   │
│                    │  └───────────────────────────────────────────────────┘  │
│                    │                                                         │
│                    │  Slug *                                                 │
│                    │    hello-world                                          │
│                    │                                                         │
│                    │  Status *        ‹ published ›                          │
│                    │                                                         │
│                    │  Content                                                │
│                    │    —                                                    │
│                    │                                                         │
│                    │  Views                                                  │
│                    │    42                                                   │
│                    │                                                         │
│                    │  Featured        [ ]                          ↓ 2 more  │
├────────────────────┴─────────────────────────────────────────────────────────┤
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ Tab Next  Enter Edit  Ctrl+S Save  Esc Back  ? Help                          │
└──────────────────────────────────────────────────────────────────────────────┘
```

No unsaved indicator in the header: a draft loaded from storage starts clean.

### H — List after rename (steps 11 and 13)

```text
┌ pressa › Posts ───────────────────────────────────────────────── 1 record ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  TITLE                  STATUS         VIEWS            │
│                    │ ─────────────────────────────────────────────────────── │
│ > Posts            │ ▸Hello, world!         published         42             │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ Record saved.                                                                │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Navigate  Enter Edit  n New  d Delete  / Search  Esc Back  ? Help         │
└──────────────────────────────────────────────────────────────────────────────┘
```

Step 13 reaches the same screen after a full restart. The status line is empty
on the restart path, since no action has been taken yet in that session.

### I — Confirm delete (step 14)

```text
┌ pressa › Posts ───────────────────────────────────────────────── 1 record ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  TITLE                  STATUS         VIEWS            │
│                    │ ─────────────────────────────────────────────────────── │
│ > Posts            │ ▸Hello, world!         published         42             │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │       ┌─ Delete record ────────────────────────┐        │
│                    │       │                                        │        │
│                    │       │  Delete "Hello, world!" from Posts?    │        │
│                    │       │  This cannot be undone.                │        │
│                    │       │                                        │        │
│                    │       │           y Delete    n Cancel         │        │
│                    │       └────────────────────────────────────────┘        │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
├────────────────────┴─────────────────────────────────────────────────────────┤
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ y Delete   n Cancel                                                          │
└──────────────────────────────────────────────────────────────────────────────┘
```

The dialog quotes the record's first `list_columns` value, rendered by the same
cell renderer the table uses. The fallback when that value is absent or is not
text belongs to spec 009 (T10), which owns this overlay.

### J — Empty after delete (step 15)

```text
┌ pressa › Posts ──────────────────────────────────────────────── 0 records ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  TITLE                  STATUS         VIEWS            │
│                    │ ─────────────────────────────────────────────────────── │
│ > Posts            │                                                         │
│                    │   No records yet.  Press n to create the first one.     │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ Record deleted.                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Navigate  Enter Edit  n New  d Delete  / Search  Esc Back  ? Help         │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Domain model

**No types are added or changed.** This spec is an integration spec; it
exercises types that T1–T11 already own:

`Schema`, `Collection`, `Field`, `FieldType`, `Record`, `RecordId`, `Value`,
`FieldError`, `ErrorCode`, `ListParams`, `RecordRepository`, `AppError`,
`AppState`, `Route`, `Overlay`, `Command`, `Effect`.

If implementing this task requires a new type, that is the signal that a lower
task is incomplete — the stop rule applies ([`AGENTS.md`](../AGENTS.md)).

## API

**No new public API.** The end-to-end test is written against the surface that
already exists:

```rust
// pressa-app
discover_project(&Path) -> Result<ProjectPaths, ConfigError>
load_schema(&Path)      -> Result<Schema, ConfigError>
RecordService::{list, count, get, create, update, delete, blank}
CollectionService::{all, get}

// pressa-storage
SqliteRepository::open(&Path) -> Result<SqliteRepository, StorageError>

// pressa-tui
update(&mut AppState, Command) -> Vec<Effect>
view(&AppState, &mut Frame)
```

## Invariants

- An invalid record is never written to the database.
- Leaving a dirty editor requires confirmation.
- Nothing is written to stdout or stderr while the TUI is running.
- The terminal is restored on exit, on error and on panic.
- A successful save routes to `Route::List` and reloads the list, from both
  `New` and `Edit`, so steps 9 and 11 see fresh data.
- No step depends on the collection being called `posts`. The same command
  sequence against an identically shaped collection under any other slug
  produces the same state transitions ([ADR-0004](../docs/adr/0004-schema-driven-ui.md)).
- Every step's expectation is a function of the schema and the stored data
  only — nothing in the scenario depends on wall-clock time or on the order
  tests run in.

## Error cases

| Input | Result |
|---|---|
| Step 6: `Ctrl+S` on the blank form | `AppError::Validation` carrying one `FieldError { code: Required, message: "required" }` for each of `title`, `slug` and `status`, in schema field order. The repository is not called; `count("posts")` is still 0. Route stays `New`. |
| Step 2 variant: `pressa validate` against a schema with `type: relation` | Exit code 1; the message names the YAML path, per [001](001-schema-config.md) |
| Step 1 variant: `pressa init` where `pressa.yaml` already exists | Refuses and exits non-zero, per [005](005-cli.md) |
| Step 13 variant: the database file is missing at startup | It is created and migrated; an existing file from a newer `schema_version` fails with a clear error, per [003](003-storage-repository.md) |

Failure modes the scenario declares but never triggers — `NotUnique` on `slug`,
`Corrupt` rows, `NotFound` — are covered by 002, 003 and 004 and are not
re-tested here.

## Acceptance criteria

- [ ] `examples/blog/pressa.yaml` declares exactly one collection, `posts`, with
      the 8 fields above in that order, with those types, `required`, `unique`
      and `options` values, and `list_columns: [title, status, views]`.
- [ ] `just run` opens `examples/blog` and reaches screen A.
- [ ] `pressa init` in an empty temp directory exits 0 and creates both
      `pressa.yaml` and `.pressa/`.
- [ ] `pressa validate` on the scenario schema exits 0 and prints a summary
      naming the collection count and the field count.
- [ ] `pressa validate` on a schema with an unknown field type exits non-zero
      and prints the offending YAML path.
- [ ] One end-to-end test at `pressa-tui/tests/golden_path.rs` drives steps 2–15
      with no pty: a `TempDir`, a real `SqliteRepository`, the real services, and
      `update()` fed the command sequence above.
- [ ] Step 6: the save produces `AppError::Validation`, and `count("posts")` is
      0 immediately afterwards.
- [ ] Step 6: exactly three `FieldError`s come back — `title`, `slug`, `status`
      — each with `code == ErrorCode::Required` and `message == "required"`, in
      schema field order.
- [ ] Step 6: the route is still `Route::New { collection: "posts" }` after the
      rejected save, and the draft is unchanged.
- [ ] Step 7: after the four field commits the editor reports dirty, and `views`
      in the draft is the JSON number `42`, not the string `"42"`.
- [ ] Step 8: `Ctrl+S` emits exactly one `Effect::SaveRecord`; after
      `RecordSaved` the route is `Route::List { collection: "posts" }` and the
      list has been reloaded.
- [ ] Step 9: the row rendered for the saved record shows `Hello world`,
      `published` and `42`, and no fourth column.
- [ ] Step 10: `Enter` on the row routes to `Route::Edit`, the draft equals the
      stored record's `data`, and the editor is clean.
- [ ] Step 11: saving the edited title updates in place — same `id`, same
      `created_at`, strictly later `updated_at`.
- [ ] Steps 12–13: after dropping the repository and reopening the same database
      file, `posts` contains exactly one record whose `title` is
      `"Hello, world!"`.
- [ ] Step 14: `d` opens `Overlay::ConfirmDelete` and emits no effect;
      `Effect::DeleteRecord` is emitted only after `Command::Confirm`.
- [ ] Step 14: `Command::Dismiss` on that overlay closes it and leaves the record
      in the database.
- [ ] Step 15: after a second drop-and-reopen, `count("posts")` is 0 and the
      empty state renders.
- [ ] 80×24 `insta` snapshots exist for steps 3, 4, 5, 6 and 9 and match
      mockups A, B, C, D and F.
- [ ] The same command sequence, run against a schema whose collection slug is
      `notes` with identically shaped fields, produces the same state
      transitions and the same stored data.
- [ ] Every row of the scenario table is listed in §Tests as either automated or
      manual; none is unaccounted for.
- [ ] Manual: steps 1–15 performed once in a real terminal, and after step 12 the
      shell is usable — no raw mode, no leftover alternate screen. Recorded as a
      checked box in the PR body.

## Tests

`pressa-tui/tests/golden_path.rs` is the home of the end-to-end test:
`pressa-tui` is the only crate that can see both `update()` and
`SqliteRepository`, and putting it there needs no fifth crate and no change to
[ADR-0006](../docs/adr/0006-workspace-of-four-crates.md).

| Step | Mechanism |
|---|---|
| 1, 2 | `assert_cmd` against the `pressa` binary in a `TempDir` |
| 3, 4, 5, 6, 9 | `insta` snapshots via `ratatui::TestBackend` at 80×24 |
| 4–11, 14 | `update()` + real services + `SqliteRepository`, asserting state, emitted effects and database contents after each step |
| 12, 15 (exit) | Modelled by dropping the repository and reopening the database file; the terminal half is covered by [006](006-tui-shell.md) |
| 13, 15 (persistence) | Reopen, `list`, assert contents |
| 12 (terminal restore) | Manual, plus the panic-restore test in [006](006-tui-shell.md) |

The test asserts database contents through `RecordService`, never by reading
SQL directly: a golden-path test that bypasses the service layer would keep
passing after the service layer broke.

## Open questions

None.

### Decisions taken at approval

Nine questions were open in Draft. Recorded here because six of them changed
another document, and a reviewer will otherwise read those changes as drift.

| # | Question | Decision | Also changed |
|---|---|---|---|
| 1 | `examples/blog` had two collections; the scenario expects one | Trim to `posts` alone | `examples/blog/pressa.yaml` |
| 2 | `blank()` seeded a required `Select`, so step 6 would report two errors, not three | `blank()` leaves required selects empty; the user chooses | [004](004-app-services.md) |
| 3 | Did the table append an implicit `UPDATED` column? | No. The table shows exactly `list_columns` | [`tui.md`](../docs/tui.md) §5.2 |
| 4 | Editor form taller than the screen | Scrolls to follow focus; markers as drawn. Owned by [008](008-record-editor.md) | — |
| 5 | Which field titles the delete dialog | First `list_columns` value via the table cell renderer; fallback owned by spec 009 | — |
| 6 | Exact strings and pluralisation | Owned by 005–008; this spec only references them | — |
| 7 | Where the end-to-end test lives | `pressa-tui/tests/golden_path.rs`; no fifth crate, ADR-0006 untouched | — |
| 8 | Steps 12 and 15 disagreed on the exit keystrokes | Both are `q`, `q`; step 15 corrected | this spec |
| 9 | What happens to the list after a save | Route to `List` and reload, from both `New` and `Edit` | this spec, §Invariants |
