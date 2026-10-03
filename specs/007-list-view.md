# SPEC-007: List view

Status: **Implemented** · Task: T8 · Crates: `pressa-tui`, `pressa-app` (re-exports
only) · ADRs: [0002](../docs/adr/0002-synchronous-rusqlite.md),
[0004](../docs/adr/0004-schema-driven-ui.md),
[0005](../docs/adr/0005-command-and-keymap-architecture.md),
[0011](../docs/adr/0011-insta-for-snapshot-tests.md)

## Problem

Nothing in the application has ever read a record. The storage adapters and
`RecordService` are tested and unreachable from the screen: `cli::dev` builds a
`RecordService` and binds it to `_records` because there is nothing to hand it
to, `Effect` is declared with no variants, `run_effects` does not exist, and
`Route::List` draws the sentence `The list view arrives in T8.`
([SPEC-006](006-tui-shell.md) frame H).

So the product is four tested layers and a frame with nothing in it. A user who
writes a `pressa.yaml`, runs `pressa dev` and presses `Enter` on a collection
learns only that the collection exists. T8 is also the task that first has to
answer the question every later screen asks — how does an `Effect` reach a
service and a `Command` come back — and if T9 answers it instead, it answers it
inside the record editor's diff.

## Goal

One table renderer that shows any collection's records, driven by
`list_columns` and `FieldType` and by nothing else
([ADR-0004](../docs/adr/0004-schema-driven-ui.md)). Entering a collection loads
its records through `RecordService` and draws them: a header row from the named
columns, one row per record, a cell renderer per field type, the record count
in the window title, and a selection the user moves with `j`/`k`, `g`/`G` and
`Ctrl+D`/`Ctrl+U` that scrolls when the records outrun the panel. A collection
with no records says so; a load that fails says that instead, and offers `r`.
The loop gains the half of [`tui.md`](../docs/tui.md) §1 that T7 left out —
`Effect` becomes inhabited, `run_effects` turns effects into commands, and
`update` stays pure and testable without a database.

## Non-goals

T8 fills the main panel on one route. A reader may reasonably expect the
following here and will not get it:

| Not here | Where it lives instead |
|---|---|
| The record editor, and with it `Route::New`, `Route::Edit`, `Context::Editor` and the `Enter` and `n` bindings that reach them. T7 already recorded that `New` and `Edit` arrive with T9 (`tui/state.rs`), and [008](008-record-editor.md) claims both routes; binding a key in T8 would mean inventing a placeholder screen and an escape binding for it, for T9 to delete — see Q2 under "Decisions taken" | [008](008-record-editor.md), T9 |
| Deleting (`d`) and the confirm overlay; creating (`n`) | 009, T10 |
| Search (`/`), the matched-count line and the help overlay (`?`). `ListParams::search` stays `None` in T8, and `Effect::LoadRecords` carries no `params` field until there is something to put in it | 010, T11 |
| Sorting, a sort indicator, a sort UI, filters, pagination controls, bulk selection, `duplicate` | M1, [roadmap](../docs/roadmap.md) §3 |
| A `limit` on the load. `ListParams::default()` fetches the collection, and the whole of it is held in `AppState`. At M0 data sizes `LIKE` is enough for search and a `Vec` is enough for a list ([roadmap](../docs/roadmap.md) §3) | M1, with pagination |
| A loading state. Effects are synchronous ([ADR-0002](../docs/adr/0002-synchronous-rusqlite.md)): the effect queue drains before the next `draw`, so a `Loading` body is unobservable and a state nothing can render is a state nothing should carry. Removed from this spec's earlier draft scope | nowhere; ADR-0002 makes it unreachable |
| Horizontal scrolling, user-resizable columns, remembered widths. Widths are recomputed from the loaded records on every load | not in M0 |
| `Command::FocusSidebar` / `FocusMain`. [`tui.md`](../docs/tui.md) §3 lists `h`/`←`/`Esc` in a list as `FocusSidebar`; T7 resolved them to `Back`, and nothing in M0 needs focus to move without the route moving | superseded by T7 |
| A second query for the record count. The title counts the records already loaded; `RecordService::count` exists for pagination, which M0 does not have | M1 |
| A `Value`-typed cell path. The renderer matches `(&FieldType, Option<&Json>)` directly, because `Value::from_json` *rejects* data that does not fit the field and a table must render bad data rather than refuse to draw ([domain-model](../docs/domain-model.md) §6) | — |
| Any new entry in `pressa-tui/Cargo.toml`. `Record`, `RecordId`, `Field`, `FieldType`, `Json` and `RecordRepository` reach the UI through `pressa_app::domain`, the way [SPEC-006](006-tui-shell.md) Q2 settled | — |

## User stories

- As a user, I want `Enter` on a collection to show me its records in a table,
  so that the data I store is something I can look at.
- As a user, I want the columns to be the ones I named in `list_columns`, so
  that the table shows what matters in my data rather than everything.
- As a user, I want a boolean to read as a tick, an empty field as a dash and a
  JSON blob as its size, so that a row is scannable rather than a wall of JSON.
- As a user with more records than rows, I want to move the selection off the
  bottom and have the table follow, and to be told how many records are off
  screen, so that nothing is hidden silently.
- As a user whose database is unreadable, I want the screen to say the load
  failed and offer me a retry, so that an empty table never lies to me about an
  empty collection.
- As an implementer of T9, I want `Effect`, `run_effects` and the
  command-feedback loop already in place, so that the record editor adds a save
  path rather than an architecture.

## UX

Every frame is exactly 80x24 unless it says otherwise — the size the `insta`
snapshots are taken at. The frames are normative for layout, spacing and
wording, and they were generated from the rules below rather than drawn by
hand, so a disagreement between a frame and a rule is a bug in this spec.

The frames are the approved ones: the five questions this spec was drafted
with were answered on 2026-10-03 and are recorded under "Decisions taken".
Nothing in them is provisional.

### The table, stated once

The main panel is 57x17 at 80x24 and 37x9 at the 60x16 minimum
([SPEC-006](006-tui-shell.md) "UX"). Inside it:

```text
rows                                   columns (panel width P)
  1    column headers                    1      left gutter
  1    rule                              1      selection marker
  H-2  one per record                    P-3    columns
                                         1      right gutter
```

- **Columns.** Exactly the fields `list_columns` names, in that order. The
  header is the field's `label`, uppercased. No column is appended, and the
  marker column is not a column — the header row leaves it blank
  ([`tui.md`](../docs/tui.md) §5.2).
- **Gap.** Two spaces between columns.
- **Natural width** of a column: the widest of its header and of its rendered
  cells over *all loaded records*, in characters, never less than 6. Over all
  records rather than the visible ones, so scrolling does not move the columns.
- **Fitting.** Let `area` be the columns area and `n` the column count. If
  `n` columns cannot each have 6 characters plus the gaps, trailing columns are
  dropped until they can, the right-most 2 + `+k` characters are reserved, and
  the header row ends `+k` where `k` is the number dropped (frame I). Nothing
  is clipped silently ([`tui.md`](../docs/tui.md) §6).
- **Widths.** The visible columns are then widened to fill `area` in proportion
  to their natural widths — `area * natural / sum`, floored, with the remainder
  handed out one character at a time from the left. A cell wider than its
  column is truncated to `width - 1` characters plus `…`.
- **Rule.** Row 2 is `─` across the marker column and the columns area, one
  blank gutter each side.
- **Marker.** `▸` on the selected row, a space on every other, and on the
  header row.
- **Alignment.** Every cell and every header is left-aligned in its column,
  numbers included: one rendering path for all seven field types and no
  `FieldType` branch in the layout code (Q5).
- **Selection** is also reverse video on the whole row, which a text snapshot
  cannot show and which is asserted on cell styles instead
  ([`tui.md`](../docs/tui.md) §6).
- **Scrolling** reuses the sidebar's window: the top row reads `↑ n more` when
  records are hidden above and the bottom row `↓ n more` when they are hidden
  below, counting the row the summary itself occupies (frames E and F). One
  function, now shared (`view::window`).
- **Title.** The window title gains a right-aligned ` <n> record(s) `, three
  `─` from the top-right corner, pluralised by the existing `counted` helper.
  It is omitted when the load failed — the title must not report 0 records for
  a collection whose contents are unknown (frame G) — and when the breadcrumbs
  would reach it, in which case the breadcrumbs win.

### A — List, empty

`examples/blog`, `posts`, no records. Reached by `Enter` on `Posts` at Home.

```text
┌ pressa › Posts ──────────────────────────────────────────────── 0 records ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  TITLE              STATUS             VIEWS            │
│                    │ ─────────────────────────────────────────────────────── │
│ > Posts            │                                                         │
│                    │   No records yet.                                       │
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
│ ↑↓ Navigate   Esc Back                                                       │
└──────────────────────────────────────────────────────────────────────────────┘
```

The body is a blank row and then `No records yet.`, indented three characters,
directly under the rule rather than centred in the panel. The columns keep
their header widths: with no records, the headers are the only content there is
to be proportional to.

### B — Three records, selection on the first

The movement fixture: `posts` with `Hello world` (draft, 0 views),
`About page` (published, 42) and `Release notes` (draft, no `views` key).

```text
┌ pressa › Posts ──────────────────────────────────────────────── 3 records ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  TITLE                     STATUS            VIEWS      │
│                    │ ─────────────────────────────────────────────────────── │
│ > Posts            │ ▸Hello world               draft             0          │
│                    │  About page                published         42         │
│                    │  Release notes             draft             —          │
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
│ ↑↓ Navigate   Esc Back                                                       │
└──────────────────────────────────────────────────────────────────────────────┘
```

`TITLE` is 24 characters wide because `Release notes` is the widest title
loaded and 13 : 9 : 6 is the proportion the 50 characters of column space are
split in. `—` is the absent `views` of the third record.

### C — The same after two `j`

```text
┌ pressa › Posts ──────────────────────────────────────────────── 3 records ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  TITLE                     STATUS            VIEWS      │
│                    │ ─────────────────────────────────────────────────────── │
│ > Posts            │  Hello world               draft             0          │
│                    │  About page                published         42         │
│                    │ ▸Release notes             draft             —          │
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
│ ↑↓ Navigate   Esc Back                                                       │
└──────────────────────────────────────────────────────────────────────────────┘
```

Only the marker moves: the widths come from the records, not from the
selection.

### D — Cell rendering, one column per renderer

A fixture collection whose `list_columns` is
`[title, featured, published_at, metadata]` — text, boolean, datetime and json
— over two records, the second of which has none of the three set.

```text
┌ pressa › Entries ────────────────────────────────────────────── 2 records ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  TITLE           FEATURED   PUBLISHED AT       METADATA │
│                    │ ─────────────────────────────────────────────────────── │
│ > Entries          │ ▸Release notes   ✓          2026-09-06 12:00   {3}      │
│                    │  Draft idea      ·          —                  —        │
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
│ ↑↓ Navigate   Esc Back                                                       │
└──────────────────────────────────────────────────────────────────────────────┘
```

Per field type, where `value` is `Record.data[field.name]`:

| `FieldType` | value | cell |
|---|---|---|
| `Text`, `Textarea`, `Select` | a string | the string; every run of control characters, newline included, becomes one space |
| `Number` | a number | the JSON number's own text — `42`, not `42.0` |
| `Boolean` | `true` / `false` | `✓` / `·` |
| `DateTime` | an RFC 3339 string | its first 16 characters with the `T` replaced by a space: `2026-09-06 12:00`. Not parsed, not converted, not relative — see Q1 under "Decisions taken" |
| `Json` | an object | `{n}`, `n` being the key count |
| `Json` | an array | `[n]`, `n` being the length |
| `Json` | a string, number or boolean | its JSON text |
| any | `null`, or the key is absent | a dim `—` |
| any | anything else the field's type does not allow | the value's JSON text, dim: storage does not validate ([003](003-storage-repository.md)), so the table must render what is there |

### E — Twenty records, at the top

```text
┌ pressa › Posts ─────────────────────────────────────────────── 20 records ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  TITLE             STATUS                 VIEWS         │
│                    │ ─────────────────────────────────────────────────────── │
│ > Posts            │ ▸Post 01           draft                  1             │
│                    │  Post 02           published              2             │
│                    │  Post 03           draft                  3             │
│                    │  Post 04           published              4             │
│                    │  Post 05           draft                  5             │
│                    │  Post 06           published              6             │
│                    │  Post 07           draft                  7             │
│                    │  Post 08           published              8             │
│                    │  Post 09           draft                  9             │
│                    │  Post 10           published              10            │
│                    │  Post 11           draft                  11            │
│                    │  Post 12           published              12            │
│                    │  Post 13           draft                  13            │
│                    │  Post 14           published              14            │
│                    │ ↓ 6 more                                                │
├────────────────────┴─────────────────────────────────────────────────────────┤
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Navigate   Esc Back                                                       │
└──────────────────────────────────────────────────────────────────────────────┘
```

Fifteen body rows and twenty records: the last row is spent saying how many are
below it, so fourteen records and one summary are shown.

### F — The same after `G`

```text
┌ pressa › Posts ─────────────────────────────────────────────── 20 records ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  TITLE             STATUS                 VIEWS         │
│                    │ ─────────────────────────────────────────────────────── │
│ > Posts            │ ↑ 6 more                                                │
│                    │  Post 07           draft                  7             │
│                    │  Post 08           published              8             │
│                    │  Post 09           draft                  9             │
│                    │  Post 10           published              10            │
│                    │  Post 11           draft                  11            │
│                    │  Post 12           published              12            │
│                    │  Post 13           draft                  13            │
│                    │  Post 14           published              14            │
│                    │  Post 15           draft                  15            │
│                    │  Post 16           published              16            │
│                    │  Post 17           draft                  17            │
│                    │  Post 18           published              18            │
│                    │  Post 19           draft                  19            │
│                    │ ▸Post 20           published              20            │
├────────────────────┴─────────────────────────────────────────────────────────┤
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Navigate   Esc Back                                                       │
└──────────────────────────────────────────────────────────────────────────────┘
```

### G — A load that failed

`Select` on a collection whose `list` call returned an `AppError`.

```text
┌ pressa › Posts ──────────────────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  TITLE              STATUS             VIEWS            │
│                    │ ─────────────────────────────────────────────────────── │
│ > Posts            │                                                         │
│                    │   Could not load records.  Press r to retry.            │
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
│ ⚠ database error: disk I/O error                                             │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Navigate   Esc Back                                                       │
└──────────────────────────────────────────────────────────────────────────────┘
```

The body says the load failed rather than that the collection is empty, names
the key that retries, and the error itself is the status line's job — one
`AppError` rendered in one place ([`architecture.md`](../docs/architecture.md)
§4). The title carries no count.

### H — At the 60x16 minimum

```text
┌ pressa › Posts ──────────────────────────── 3 records ───┐
├────────────────────┬─────────────────────────────────────┤
│ Collections        │  TITLE           STATUS      VIEWS  │
│                    │ ─────────────────────────────────── │
│ > Posts            │ ▸Hello world     draft       0      │
│                    │  About page      published   42     │
│                    │  Release notes   draft       —      │
│                    │                                     │
│                    │                                     │
│                    │                                     │
│                    │                                     │
├────────────────────┴─────────────────────────────────────┤
│                                                          │
├──────────────────────────────────────────────────────────┤
│ ↑↓ Navigate   Esc Back                                   │
└──────────────────────────────────────────────────────────┘
```

### I — More columns than fit

A seven-column `list_columns` in the 34 characters a 60-column terminal leaves:
four columns fit at 6 characters each, and the header says so.

```text
┌ pressa › Entries ─────────────────────────── 1 record ───┐
├────────────────────┬─────────────────────────────────────┤
│ Collections        │  TITLE   STATUS  CONTE…  VIEWS   +3 │
│                    │ ─────────────────────────────────── │
│ > Entries          │ ▸Relea…  publi…  Every…  1024       │
│                    │                                     │
│                    │                                     │
│                    │                                     │
│                    │                                     │
│                    │                                     │
│                    │                                     │
├────────────────────┴─────────────────────────────────────┤
│                                                          │
├──────────────────────────────────────────────────────────┤
│ ↑↓ Navigate   Esc Back                                   │
└──────────────────────────────────────────────────────────┘
```

## Domain model

`AppState` gains the `list` field [`tui.md`](../docs/tui.md) §1 reserves for it:

```rust
pub struct AppState {
    pub schema: Schema,
    pub route: Route,
    pub sidebar: SidebarState,
    pub list: ListState,          // new in T8
    pub status: Option<StatusMessage>,
    pub should_quit: bool,
}

/// The records of the collection the route names, and where we are in them.
#[derive(Debug, Clone, PartialEq)]
pub struct ListState {
    pub records: Vec<Record>,
    /// Index into `records`. 0 when there are none.
    pub selected: usize,
    /// The row the table would like to start at; `view` clamps it rather than
    /// mutating it, as the sidebar's already does.
    pub offset: usize,
    pub load: Load,
}

/// Whether the records in `ListState` are the collection's or the remains of a
/// load that failed. Two variants and not three: ADR-0002 makes `Loading`
/// unobservable (see "Non-goals").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Load {
    Ok,
    Failed,
}
```

`Command` gains the list's actions and the two results an effect can produce:

```rust
pub enum Command {
    MoveUp, MoveDown, Select, Back, Quit,       // T7
    GoToTop, GoToBottom, PageUp, PageDown,      // new
    Refresh,                                    // new
    RecordsLoaded(Vec<Record>),                 // new
    OperationFailed(String),                    // new
}
```

`Command` stops being `Copy`, because `RecordsLoaded` owns its records; it
stays `Clone` and `PartialEq`. `keymap::resolve` returns a `Command` by clone
rather than by copy — the keymap's own variants are all unit variants, so the
clone is free.

`Effect` becomes inhabited, with the one variant T8 can serve:

```rust
pub enum Effect {
    LoadRecords { collection: String },
}
```

No `params: ListParams` field: T8 loads with `ListParams::default()` and T11
adds the field when search gives it a value. `ListParams` is therefore not
named in `pressa-tui` at all.

`pressa_app::domain` re-exports what the table reads, extending the list
[SPEC-006](006-tui-shell.md) Q2 started:

```rust
pub use pressa_core::record::{Record, RecordId};
pub use pressa_core::repository::RecordRepository;
pub use pressa_core::schema::{Collection, Field, FieldType, Schema};
/// The stored form of a document, re-exported under the name the UI uses.
pub use serde_json::Value as Json;
```

`RecordId` is re-exported for [008](008-record-editor.md) rather than used
here; `RecordRepository` is needed to write the bound on `run_effects`.

## API

```rust
// tui/update.rs — unchanged signature, new arms
pub fn update(state: &mut AppState, cmd: Command) -> Vec<Effect>;

/// How far `PageUp` / `PageDown` move. A constant and not the panel's height:
/// the height is known only while drawing and `view` may not write to state,
/// so `update` would have to read a terminal to know it (Q3). Ten rows is
/// about half the body at the snapshot size, which is what `Ctrl+D` means to a
/// vim user; `view` clamps the offset, so the selection stays visible whatever
/// the real height is.
pub const PAGE: usize = 10;

// tui/effects.rs — new: the only place a service is called
pub fn run_effects<R: RecordRepository>(
    records: &RecordService<R>,
    effects: Vec<Effect>,
) -> Vec<Command>;

// tui/mod.rs — `drive` gains an effect runner, in the shape its event
// source already has, so the loop stays generic over a backend alone and
// never over a repository
pub fn drive<B: Backend>(
    state: &mut AppState,
    terminal: &mut Terminal<B>,
    events: &mut dyn FnMut() -> io::Result<Event>,
    effects: &mut dyn FnMut(Vec<Effect>) -> Vec<Command>,
) -> Result<(), TuiError>;

// tui/view/table.rs — new
pub fn render(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    collection: &Collection,
);

/// One cell's text and whether it is dim. Pure, and the whole of ADR-0004's
/// "render in a table cell" question for every field type.
pub struct Cell { pub text: String, pub dim: bool }
pub fn cell(kind: &FieldType, value: Option<&Json>) -> Cell;

/// The width of each visible column, and how many did not fit, from the
/// natural widths and the space available. Pure arithmetic, no `Frame`.
pub fn columns(natural: &[usize], area: usize) -> (Vec<usize>, usize);

// tui/view/mod.rs
/// Moves here from `view::sidebar`, unchanged: the table windows its rows the
/// same way the sidebar windows its collections.
pub fn window(offset: usize, selected: usize, len: usize, height: usize)
    -> usize;

// tui/view/chrome.rs — the title gains a right-aligned segment
pub fn shell(frame: &mut Frame, area: Rect, title: &str, right: Option<&str>);
```

`cli::dev` stops binding the service to `_records` and passes it to the loop:
`tui::run(schema, records)`, which builds the closure
`|effects| run_effects(&records, effects)` and hands it to `drive`.

The loop, per pass: draw, read one event, resolve it, `update`, then drain —
run the effects, feed each returned command back through `update`, run what
*those* return — until the queue is empty, and only then draw again. In T8 no
command an effect produces returns an effect, so the queue drains in one pass;
it is written as a queue because T9's save does not.

## Invariants

- `update` performs no I/O and touches nothing but the `AppState` it is given;
  `run_effects` is the only function in the crate that calls a service.
- `view` performs no I/O, reads no clock and never mutates state. Everything it
  draws is already in `AppState`.
- `list.selected < list.records.len()`, or both are 0.
- The number of header cells equals `list_columns.len()` minus the number
  reported as `+k`; the table appends no column of its own.
- `cell` branches on `FieldType` and on the JSON value, never on a field name
  or a collection slug ([ADR-0004](../docs/adr/0004-schema-driven-ui.md)).
- Column widths are a function of the headers and the loaded records only —
  never of the selection or the scroll offset.
- `Load::Failed` and a non-empty `records` cannot both be true: a failed load
  clears the records it could not replace.
- No key is compared to a `KeyCode` outside `keymap.rs`; every hint string
  comes from `KEYMAP`.

## Error cases

| Input | Result |
|---|---|
| `Select` and the `list` call returns `AppError` | frame G: `Load::Failed`, records cleared, status line `⚠ <the AppError's message>`, no count in the title |
| `Refresh` after the cause is fixed | the records appear, `Load::Ok`, the status line clears |
| `Refresh` while still failing | the same frame G; one error message, not a growing list |
| `MoveDown` on the last record, `MoveUp` on the first | the selection does not move and no effect is returned — clamped, never wrapping, as the sidebar is |
| `PageDown` / `GoToBottom` on an empty collection | `selected` stays 0; no panic on an empty `Vec` |
| A cell value of the wrong JSON type for its field | the value's JSON text, dim (frame D's last row of the table above) |
| A `list_columns` entry absent from a record | a dim `—` |
| A record whose `data` is not an object | every column renders `—`; the row is still drawn |
| A title too long to leave room for the count | the count is dropped, the breadcrumbs are not |
| `list_columns` wider than the panel | frame I: trailing columns dropped, `+k` in the header |

A `list_columns` entry naming a field the collection does not have is not an
error case: [SPEC-001](001-schema-config.md) rejects it at load time, so the
renderer may treat every column as resolvable.

## Acceptance criteria

Table layout

- [ ] An 80x24 snapshot of `posts` with three records matches frame B, and the
      snapshot after two `MoveDown` matches frame C.
- [ ] `columns` splits the space in proportion to the natural widths: for
      naturals `[13, 9, 6]` and an area of 54 it returns `[24, 16, 10]`, which
      sums with the gaps to exactly the area.
- [ ] Every column is at least 6 wide, for every natural-width input.
- [ ] A cell wider than its column is truncated to `width - 1` characters plus
      `…`, counted in characters and not bytes.
- [ ] Seven columns in a 60-column terminal render four and a `+3` in the
      header row, and `+3` appears in no data row (frame I).
- [ ] The header row is the uppercased `label` of each `list_columns` field, in
      `list_columns` order, and the marker column is blank on it.
- [ ] Column widths do not change when the selection or the offset changes
      (frames B and C have identical header rows), and do change when the
      records change.

Cell rendering

- [ ] `cell` returns, for each `FieldType` and each row of the table in frame
      D's rules: the string for `Text`/`Textarea`/`Select`, `42` for the number
      `42`, `✓`/`·` for booleans, `2026-09-06 12:00` for
      `"2026-09-06T12:00:00Z"`, `{3}` for a three-key object, `[2]` for a
      two-element array.
- [ ] `None` and `Json::Null` both render a `—` with `dim` set, for every field
      type.
- [ ] A value whose JSON type the field does not allow renders the value's JSON
      text with `dim` set, and does not panic — asserted for a number in a
      `Text` field and a string in a `Boolean` field.
- [ ] A `Text` value containing `\n` renders on one row, the newline replaced
      by a space, and cannot push the table's rule off the screen.
- [ ] A record whose `data` is not a JSON object renders every column as `—`.
- [ ] The 80x24 snapshot of the four-type fixture matches frame D.

Navigation

- [ ] `MoveDown` at `Route::List` moves `list.selected` and leaves
      `sidebar.selected` untouched; at `Route::Home` it does the opposite.
- [ ] `MoveDown` on the last record and `MoveUp` on the first leave the state
      equal to what it was and return no effects.
- [ ] `GoToTop` and `GoToBottom` select the first and last record; on an empty
      collection both leave `selected` at 0.
- [ ] `PageDown` and `PageUp` move the selection by `PAGE` rows and clamp at
      both ends; `PAGE` is 10 and `update` reads no terminal size to get it.
- [ ] Twenty records in fifteen rows: the initial snapshot matches frame E
      (`↓ 6 more` on the last row) and the snapshot after `GoToBottom` matches
      frame F (`↑ 6 more` on the first).
- [ ] The selected row is reverse video, asserted on the buffer's cell styles;
      the `▸` marker is in the text snapshot.
- [ ] `window` is called by both the sidebar and the table — one function, and
      its existing unit tests still pass from its new home.

Loading and failure

- [ ] `Select` at Home returns exactly one `Effect::LoadRecords` naming the
      selected collection, and resets `list` to empty with `selected` and
      `offset` at 0.
- [ ] `run_effects` turns `LoadRecords` into `RecordsLoaded` with the records
      `RecordService::list` returns, in `ListParams::default()` order, against
      a `MemoryRepository`.
- [ ] `run_effects` turns a failing `list` into
      `OperationFailed(error.to_string())` and returns no `RecordsLoaded`.
- [ ] `RecordsLoaded` stores the records, sets `Load::Ok`, clears an error
      status, and clamps `selected` into the new length — asserted with a
      selection of 7 and a reload of 3 records.
- [ ] `OperationFailed` sets `Load::Failed`, clears the records, and sets an
      error status with the message it carried.
- [ ] The 80x24 snapshot of a failed load matches frame G: the retry line in
      the body, the message in the status line, and no count in the title.
- [ ] `Refresh` at `Route::List` returns one `LoadRecords` for the route's
      collection; at `Route::Home` it returns none.
- [ ] The title reads `0 records`, `1 record` and `3 records` from `counted`,
      and the snapshot of an empty collection matches frame A.
- [ ] Entering a collection twice reloads it: the second `Select` returns an
      effect rather than reusing the records in `AppState`.

The loop

- [ ] `drive` runs the effects `update` returns and feeds the resulting
      commands back through `update` before the next draw — asserted with a
      scripted effect runner and a `TestBackend`, no database and no terminal.
- [ ] `drive` drains chained commands: a runner that answers one effect with a
      command whose `update` returns another effect settles before the next
      draw.
- [ ] A failing effect runner cannot end the loop: `OperationFailed` is a
      command like any other and the session continues.
- [ ] `cli::dev` hands the `RecordService` it builds to `tui::run`; the binding
      named `_records` is gone, and `pressa dev` on `examples/blog` loads and
      draws the collection's records.

Keymap and hints

- [ ] `resolve(Context::List, k)` returns `MoveDown` for `j` and `↓`, `MoveUp`
      for `k` and `↑`, `GoToTop` for `g`, `GoToBottom` for `G`, `PageDown` for
      `Ctrl+D`, `PageUp` for `Ctrl+U`, `Refresh` for `r`, and `Back` for `Esc`,
      `h`, `←` and `q`.
- [ ] The `List` hint bar is `↑↓ Navigate   Esc Back` — the navigation keys
      read as one entry and `g`, `G`, `Ctrl+D`, `Ctrl+U` and `r` are `Hidden`.
- [ ] Frames A–I carry hint bars equal to `hints(Context::List)` joined with
      three spaces, computed from `KEYMAP` in the test; no literal hint text
      appears in a renderer or a test.
- [ ] Every visible hint still names a key that resolves to its own command,
      and `?` still resolves to nothing.
- [ ] No `KeyCode::` outside `keymap.rs` — the existing source scan still
      passes.

Purity and boundaries

- [ ] `update` returns the same state and effects for the same input, called
      twice, for every new command; no test of `update` opens a repository.
- [ ] `view` is called in every snapshot test with no services in scope, and
      `cell` takes no clock, no `Schema` and no `AppState`.
- [ ] `pressa-tui/Cargo.toml` gains no dependency, in any section:
      `serde_json`, `chrono` and `pressa-core` are all still absent, and the
      types reach the crate through `pressa_app::domain`.
- [ ] `pressa-tui/tests/architecture.rs` still passes unchanged, including
      `pressa-core` and `rusqlite` on the forbidden list.
- [ ] No `unwrap()`, `expect()` or `panic!()` outside `#[cfg(test)]`; indexing
      a `Vec` by a stored index happens through `get`.
- [ ] No identifier in `pressa-tui` is a collection slug, and the four-type
      fixture is a *different* collection from `posts`, rendered by the same
      function, as proof.

Documents

The approval commit already carried the edits the five answers implied:
[`tui.md`](../docs/tui.md) §1 (`ListState`, `Load`, and the note that
`Effect::LoadRecords` carries no `params` until T11), §3 (the `List` back row,
which T7 resolved to `Back`, plus `FocusSidebar`/`FocusMain` and the page size),
§5.2 (`{n}`, the datetime form, left alignment, the title count, the two empty
bodies, and a pointer here for the width arithmetic), §6 (`·`, `—`, `↑`, `↓`),
and [000](000-m0-golden-path.md)'s frames B, F, H, I and J, whose table rows
were regenerated from this spec's algorithm and whose hint bars were rejoined
with three spaces (Q4). What is left belongs with the code that changes:

- [ ] The `sandbox` recipe and its `README.md` say what `dev` now shows for a
      collection with records, and `just sandbox` was run
      ([`development.md`](../docs/development.md) §8).
- [ ] [`tui.md`](../docs/tui.md) §5.2's empty-state sentence matches what the
      renderer prints: `No records yet.` in T8, the second sentence in T9.
- [ ] This spec's status is `Implemented`, and
      [`roadmap.md`](../docs/roadmap.md) §2's T8 row is ticked.

## Tests

`pressa-tui/tests/`, plus unit tests beside the modules they cover. No test
opens a real terminal; only the `run_effects` tests open a repository, and they
use `MemoryRepository`.

| Criteria | Test |
|---|---|
| Frames A–I | `insta` snapshots through `ratatui::TestBackend` at the stated size, in `tests/list_snapshots.rs` |
| Reverse video on the selected row, dim `—`, dim `↑ n more`, red status | assertions on `Buffer` cell styles, in the same file |
| `columns` arithmetic, the 6-character floor, dropping and `+k` | unit tests in `view/table.rs` |
| `cell` per field type, absent, null, wrong type, newline | a table-driven unit test in `view/table.rs`, one case per row of frame D's table |
| `window` sharing | its existing unit tests, moved with it |
| Navigation, clamping, `Select`, `Refresh`, `RecordsLoaded`, `OperationFailed` | `update` unit tests: build a state, send a command, assert state and effects |
| `run_effects` | integration test in `tests/effects.rs` over `MemoryRepository`, including a repository that fails |
| The drain loop | `tui::drive` with a scripted event source and a scripted effect runner, in `tests/shell.rs` |
| `dev` draws real records | `assert_cmd` cannot see a frame; covered by a test that builds the services the way `cli::dev` does and drives `drive` over a `TestBackend` |

Fixtures: `examples/blog/pressa.yaml` for frames A, B, C, E, F, G and H, loaded
rather than copied; the four-type collection and the seven-column collection
written as YAML in the test, through `support::schema_from_yaml`. Records are
built through `RecordService::create` against `MemoryRepository` so that no
test constructs a `Record` by hand — `pressa-tui` cannot, and should not be
able to.

Snapshot review is `cargo insta review`, never `--accept`.

## Open questions

None. The five this spec was drafted with were answered by the coordinator on
2026-10-03 and are folded in above.

### Decisions taken

- **Q1 — `DateTime` cells.** Absolute, not relative: the stored RFC 3339
  string's first 16 characters with the `T` replaced by a space,
  `2026-09-06 12:00`. A relative time needs the current instant, which `view`
  may not read, so it would have to be stamped into `AppState` by the loop and
  would make `chrono` a `pressa-tui` dependency and so an ADR. The absolute
  form needs no clock, no parsing and no dependency, and shows the offset as
  stored. A string that is not RFC 3339 shaped falls to the invalid-value rule
  and renders dim.
- **Q2 — `Enter` and `n`.** Neither is bound in T8. They move to
  [008](008-record-editor.md) with `Route::New`, `Route::Edit` and the screens
  behind them, which is where T7 already put them (`tui/state.rs`) and what
  [008](008-record-editor.md) already claims. The `List` hint bar is therefore
  `↑↓ Navigate   Esc Back`, and frame A's empty state is `No records yet.`
  without the sentence naming `n` — a screen may not name a key that does
  nothing ([SPEC-006](006-tui-shell.md) Q4). T9 binds both, appends
  `Press n to create the first one.` to the empty state, and retakes the
  snapshots of frames A–I.
- **Q3 — the page.** A compiled-in `PAGE` of 10 rows. `update` stays a function
  of `AppState` alone: the alternative — the loop writing the last drawn body
  height into state before every `update` — makes one field of `AppState` a
  cache of the terminal, and buys a page size that is exactly right only at the
  size the snapshots are taken at. Ten is about half the 17-row body at 80x24,
  which is what `Ctrl+D` means to a vim user, and `view` clamps the offset so
  the selection is visible whatever the real height is.
- **Q4 — [000](000-m0-golden-path.md)'s frames.** Redrawn, as proposed: frames
  B, F, H, I and J had their table rows regenerated from "The table, stated
  once" and their hint bars rejoined with three spaces, the fix
  [SPEC-006](006-tui-shell.md) Q5 had already assigned to T8. T10's confirm
  dialog in frame I was spliced back in unchanged — it is 009's, not this
  spec's — and the editor frames C, D, E and G were left alone for T9. §UX now
  records where those five frames' geometry comes from, so the next task does
  not have to guess whether a frame was derived or drawn.
- **Q5 — alignment.** Everything left-aligned, numbers included. Right-aligning
  `Number` is the conventional data-table answer, but it puts a `FieldType`
  branch in the layout code, where [ADR-0004](../docs/adr/0004-schema-driven-ui.md)
  wants the complexity in the renderers and the layout blind to type. Revisit
  with M1's sort UI, which touches the header row anyway.
