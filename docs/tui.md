# Terminal UI

Status: Approved · Last updated: 2026-10-03 · Crate: `pressa-tui`

## 1. State model

Elm-shaped, deliberately small:

```rust
pub struct AppState {
    pub schema: Schema,
    pub route: Route,
    pub sidebar: SidebarState,
    pub list: ListState,
    pub editor: EditorState,
    pub overlay: Option<Overlay>,
    pub status: Option<StatusMessage>,
    pub should_quit: bool,
}

pub enum Route {
    Home,
    List { collection: String },
    New  { collection: String },
    Edit { collection: String, id: RecordId },
}

pub enum Overlay {
    Help,
    ConfirmDelete { collection: String, id: RecordId },
    ConfirmDiscard { next: Box<Route> },
    Search { query: String },
}
```

`Route` is a route in the URL sense — see §2. `Overlay` is what is drawn *on
top* of the current route; it is separate from `Route` because dismissing a
help popup must not change where you are.

Shipped as of T9: all four `Route` variants, `editor`, `overlay`, and
`Overlay::ConfirmDiscard`. `ConfirmDelete` arrives with T10, `Help` and
`Search` with T11 — each adds a variant to an enum that already exists
([`specs/008`](../specs/008-record-editor.md)).

```rust
/// The draft, where the focus is, and what the last save refused. One struct
/// for both `New` and `Edit`.
pub struct EditorState {
    pub draft: serde_json::Value,     // every field key present
    pub original: serde_json::Value,  // dirty is `draft != original`, stored nowhere
    pub focus: usize,                 // index into `collection.fields`
    pub offset: usize,
    pub input: Option<String>,        // `Some` is an input context (§3)
    pub errors: Vec<FieldError>,      // at most one per field
    pub id: Option<RecordId>,         // the third crumb's fallback (§2)
}
```

```rust
pub struct ListState {
    pub records: Vec<Record>,
    pub selected: usize,
    pub offset: usize,
    pub load: Load,
}

/// Whether `records` is the collection's or the remains of a load that failed.
/// Two variants and not three: effects are synchronous (ADR-0002), so the
/// queue drains before the next draw and a `Loading` state is unobservable.
pub enum Load { Ok, Failed }
```

`Load::Failed` is why the table can say "could not load" instead of claiming a
collection is empty, and why `Failed` with a non-empty `records` is a state the
code must not produce ([`specs/007`](../specs/007-list-view.md)).

### The loop

```
crossterm event
   → Keymap::resolve(context, key) -> Option<Command>
   → update(&mut AppState, Command) -> Vec<Effect>      pure, no I/O
   → run_effects(effects, &services) -> Vec<Command>    the only I/O
   → view(&AppState, frame)                             pure render
```

```rust
pub fn update(state: &mut AppState, cmd: Command) -> Vec<Effect>;

pub enum Effect {
    /// `params` arrives with T11, which gives it a value. Until then the
    /// variant is `{ collection }` and `pressa-tui` names no `ListParams`
    /// ([`specs/007`](../specs/007-list-view.md) "Domain model").
    LoadRecords { collection: String, params: ListParams },
    LoadRecord  { collection: String, id: RecordId },
    SaveRecord  { collection: String, id: Option<RecordId>, data: serde_json::Value },
    DeleteRecord{ collection: String, id: RecordId },
}
```

Shipped as of T9: `LoadRecords { collection }`, `LoadRecord` and `SaveRecord`,
whose `id: None` is a create and `Some` an update. `DeleteRecord` arrives with
T10's runner for it.

Effects resolve back into commands (`Command::RecordsLoaded(..)`,
`Command::SaveFailed(Vec<FieldError>)`, …). Keeping I/O out of `update` is what
lets us test every interaction without a database and without a terminal.

## 2. Navigation as routes

Even in a terminal, navigation is modelled as paths:

```
/                         Home        — nothing selected yet
/posts                    List
/posts/new                New
/posts/01J8XQ…            Edit
```

Breadcrumbs are **derived from the route**, never assembled by hand:

```rust
fn breadcrumbs(state: &AppState) -> Vec<String>
// Route::Edit{"posts", id} → ["pressa", "Posts", "Hello world"]
```

At `New` and `Edit` the third crumb names the record: the draft's first
`list_columns` value through the table's cell renderer, cut to 20 characters,
read live so it follows a rename. When that value is absent it is the id
shortened to six characters and `…` at `Edit`, and `New` at `New`. A ULID is
not what a user recognises a record by, so the four-crumb id form this section
used to promise was dropped ([`specs/008`](../specs/008-record-editor.md) Q8).
That is why `breadcrumbs` takes the state and not the route alone.

Hand-maintained breadcrumbs drift from actual navigation state within a week.
Deriving them makes that impossible and makes the header snapshot-testable on
its own.

## 3. Commands and the keymap

No `if key == KeyCode::Char('j')` anywhere outside the keymap table.

```rust
pub enum Command {
    // navigation
    MoveUp, MoveDown, PageUp, PageDown, GoToTop, GoToBottom,
    FocusSidebar, FocusMain, Select, Back, Quit,
    // records
    NewRecord, EditRecord, DeleteRecord, Refresh,
    // editor
    NextField, PrevField, BeginEdit, CommitField, CancelEdit, Save,
    ToggleBoolean, NextOption, PrevOption,
    // overlays
    OpenHelp, OpenSearch, SearchInput(char), SearchBackspace,
    Confirm, Dismiss,
    // input
    InputChar(char), InputBackspace,
    // effect results
    RecordsLoaded(Vec<Record>), RecordLoaded(Box<Record>),
    RecordSaved(Box<Record>), RecordDeleted(RecordId),
    SaveFailed(Vec<FieldError>), OperationFailed(String),
}

pub struct KeyBinding {
    pub context: Context,       // see below
    pub key: KeyEvent,
    pub command: Command,
    pub description: &'static str,
    pub hint: Hint,
}

/// What the hint bar prints for this binding, if anything. `ShownAs` exists
/// because `j` and `↓` are two bindings that must read as one `↑↓ Navigate`
/// entry; one field rather than a `bool` plus a label, so that a hidden
/// binding carrying a label is unrepresentable (SPEC-006, Q6).
pub enum Hint {
    Hidden,
    Shown,                  // "<key label> <description>"
    ShownAs(&'static str),  // that label instead of the key's own
}
```

```rust
/// Derived from the state, never stored: overlay first, then `editor.input`,
/// then the route.
pub enum Context {
    Global, Sidebar, List,
    Editor,          // New / Edit, moving between fields
    EditorInput,     // typing into a Text, Number or DateTime field
    EditorText,      // typing into a Textarea or Json field
    ConfirmDiscard,  // the discard dialog; T10 adds ConfirmDelete
}
```

One context per dialog, because `KeyBinding.description` is a fixed string and
`y Discard` and `y Delete` cannot come from one row (SPEC-008 Q10).

The hint bar is the current context's non-`Hidden` bindings in table order,
then the non-`Hidden` `Global` ones, joined with three spaces.

The keymap is **one static table**. Both the bottom hint bar and the `?` help
overlay are generated from it by filtering on the current context. A key that
exists but is undocumented, or documented but unbound, is therefore not
possible. See [ADR-0005](adr/0005-command-and-keymap-architecture.md).

### M0 key bindings

| Context | Key | Command | Hint bar |
|---|---|---|---|
| global | `?` | OpenHelp | yes |
| global | `Ctrl+C` | Quit | no |
| Sidebar | `j` / `↓` | MoveDown | yes (as `↑↓`) |
| Sidebar | `k` / `↑` | MoveUp | — |
| Sidebar | `Enter` / `l` / `→` | Select → route to List | yes |
| Sidebar | `q` | Quit | yes |
| List | `j` / `↓` | MoveDown | yes |
| List | `k` / `↑` | MoveUp | — |
| List | `g` / `G` | GoToTop / GoToBottom | no |
| List | `Ctrl+D` / `Ctrl+U` | PageDown / PageUp | no |
| List | `Enter` | EditRecord | yes |
| List | `n` | NewRecord | yes |
| List | `d` | DeleteRecord → ConfirmDelete | yes |
| List | `/` | OpenSearch | yes |
| List | `r` | Refresh | no |
| List | `h` / `←` / `Esc` | Back | yes (as `Esc Back`) |
| List | `q` | Back | — |
| Editor | `Tab` / `j` | NextField | yes (`Tab`) |
| Editor | `Shift+Tab` / `k` | PrevField | — |
| Editor | `Enter` | BeginEdit — an input context for the five text-shaped types, ToggleBoolean for a `Boolean`, NextOption for a `Select` | yes |
| Editor | `Space` | ToggleBoolean | — |
| Editor | `l` / `→` | NextOption | — |
| Editor | `h` / `←` | PrevOption | — |
| Editor | `Ctrl+S` | Save | yes |
| Editor | `Esc` | Back (ConfirmDiscard if dirty) | yes |
| EditorInput | printable | InputChar | — |
| EditorInput | `Backspace` | InputBackspace | — |
| EditorInput | `Enter` | CommitField | yes |
| EditorInput | `Esc` | CancelEdit — the draft was never written | yes |
| EditorText | printable | InputChar | — |
| EditorText | `Backspace` | InputBackspace | — |
| EditorText | `Tab` | CommitField | yes |
| EditorText | `Enter` | InsertNewline | yes |
| EditorText | `Esc` | CancelEdit | yes |
| ConfirmDiscard | `y` / `Enter` | Confirm | yes (`y`) |
| ConfirmDiscard | `n` / `Esc` / `q` | Dismiss | yes (`n`) |

`?`, `d` and `/` are bound by T10 and T11; until then they resolve to nothing,
except in the two input contexts, where every printable character — `?`
included — is text.

`q` is two context rows rather than one global row with a branch inside
`update`: it quits from the sidebar, where there is nowhere left to go back to,
and goes back from anywhere else. A context binding shadows a global one, so
resolution stays a table lookup (SPEC-006, "The keymap").

Two modes inside the editor — `Editor` navigates between fields, an input
context types into one — is the vim distinction, and it is what keeps `j`
usable for navigation without stealing it from text. There are two input
contexts rather than one so that `Enter` can commit a single-line field and
insert a newline in a multi-line one while resolution stays a table lookup;
`Tab` commits the multi-line two (SPEC-008 Q3b). The draft changes only on
`CommitField` and the per-type commands, so a cancelled edit cannot have
changed it.

`FocusSidebar` and `FocusMain` are listed in `Command` above and bound to
nothing: T7 resolved a list's `h` / `←` / `Esc` to `Back`, and nothing in M0
needs focus to move without the route moving
([`specs/007`](../specs/007-list-view.md) "Non-goals"). `PageUp` / `PageDown`
move by a compiled-in page of 10 rows, because `update` cannot see the panel's
height and `view` may not write to state (SPEC-007, Q3).

## 4. Layout

```
┌─ header: breadcrumbs ─────────────────────── right-aligned context action ─┐
├──────────────┬────────────────────────────────────────────────────────────┤
│              │                                                            │
│   sidebar    │                     main panel                             │
│  (width 20,  │              (list table / record form)                     │
│   min 14)    │                                                            │
│              │                                                            │
├──────────────┴────────────────────────────────────────────────────────────┤
│ status line: last action, or error in red                                  │
├───────────────────────────────────────────────────────────────────────────┤
│ hint bar: generated from the keymap for the current context                │
└───────────────────────────────────────────────────────────────────────────┘
```

Header 1 row, status 1 row, hint bar 1 row, sidebar 20 columns. Minimum
supported terminal is 60×16; below that we render a single "terminal too small"
message rather than a broken layout.

## 5. Screens

### 5.1 Home — nothing selected

```
┌ pressa ───────────────────────────────────────────────────────────────────┐
├──────────────┬────────────────────────────────────────────────────────────┤
│ Collections  │                                                            │
│              │              Select a collection to begin.                  │
│ > Posts      │                                                            │
│   Authors    │              blog-cms · 3 collections                       │
│   Categories │                                                            │
│              │                                                            │
├──────────────┴────────────────────────────────────────────────────────────┤
│                                                                            │
├───────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Navigate   Enter Open   ? Help   q Quit                                 │
└───────────────────────────────────────────────────────────────────────────┘
```

### 5.2 List

```
┌ pressa › Posts ───────────────────────────────────────────── 2 records ───┐
├──────────────┬────────────────────────────────────────────────────────────┤
│ Collections  │  TITLE            STATUS      VIEWS                         │
│              │ ─────────────────────────────────────────────────────────  │
│ > Posts      │ ▸Hello world      draft           0                         │
│   Authors    │  About page       published      42                         │
│   Categories │                                                            │
│              │                                                            │
├──────────────┴────────────────────────────────────────────────────────────┤
│                                                                            │
├───────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Navigate  Enter Edit  n New  d Delete  / Search  Esc Back  ? Help       │
└───────────────────────────────────────────────────────────────────────────┘
```

Columns come from `list_columns` — exactly those fields, in that order, with
no implicit extra column appended. Column widths are proportional to content
with a minimum of 6 and an ellipsis on overflow; the exact arithmetic, the
two-space gap, the marker column, the `↑ n more` scrolling and the `+k` the
header ends with when a column cannot fit are specified once, in
[`specs/007`](../specs/007-list-view.md) "The table, stated once". Every cell
and header is left-aligned, numbers included, so that one rendering path serves
all seven field types (SPEC-007, Q5).

Boolean renders as `✓` / `·`, `Json` as `{n}` with the key count (`[n]` for an
array), a `DateTime` as the stored RFC 3339 string's first 16 characters with
the `T` replaced by a space — `2026-09-06 12:00`, never a relative time, which
would need a clock `view` may not read (SPEC-007, Q1) — and an absent or null
value as a dim `—`. A value whose JSON type its field does not allow renders as
its own JSON text, dim: storage does not validate, so the table shows what is
there.

The record count sits right-aligned in the window title, three `─` from the
corner, and is omitted when the load failed.

Empty state replaces the table body with:

```
   No records yet.  Press n to create the first one.
```

T8 rendered the first sentence alone; T9 bound `n` and added the second — a
screen may not name a key that does nothing (SPEC-006, Q4). The key is read
from the keymap, not spelled. A load that
failed replaces the body with `Could not load records.  Press r to retry.`
instead, and the error itself goes to the status line.

### 5.3 Record editor

```
┌ pressa › Posts › Hello world ───────────────────────── ● unsaved · Ctrl+S ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  Title *                                                │
│                    │    Hello world                                          │
│ > Posts            │                                                         │
│                    │  Slug *                                                 │
│                    │    hello-world                                          │
│                    │                                                         │
│                    │  Status *        ‹ published ›                          │
│                    │                                                         │
│                    │  Content                                                │
│                    │    —                                                    │
│                    │                                                         │
│                    │ ▸Views                                                  │
│                    │    42                                                   │
│                    │                                                         │
│                    │  Featured        [ ]                                    │
│                    │                                                         │
│                    │  Published at                                 ↓ 1 more  │
├────────────────────┴─────────────────────────────────────────────────────────┤
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ Tab Next   Enter Edit   Ctrl+S Save   Esc Back                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

One form renderer, generated from `Collection.fields` and `FieldType` alone.
In brief:

- one block per field in schema order, each followed by a blank row; a label
  row with ` *` on a required field and `▸` on the focused one;
- `Select` (`‹ value ›`, `‹ — ›` when empty) and `Boolean` (`[x]` / `[ ]`)
  inline at the value column — two of indent, the longest label with its ` *`,
  and two of gap, never less than 18 — in reverse video when focused; the other five types stacked
  under their label, indented four;
- values are the text you would edit: a `DateTime` as its RFC 3339 string, a
  `Json` field as its JSON text; an absent value is a dim `—`, a value of the
  wrong JSON type its own JSON text, dim;
- the focused field is `▸` on its label and a reverse-video bar on its value
  row — no border, no extra row, so moving the focus moves nothing else; a
  `Textarea` or `Json` field grows to five wrapped rows only while it is typed
  into; text being typed shows its tail behind `…`, and `▌` is drawn after it
  exactly while it is being typed into;
- an error row `⚠ message`, in red, under any field the last save or commit
  refused, and `<n> field(s) need attention` on the status line, derived from
  the errors;
- the form scrolls by whole fields, the least that keeps the focused block and
  its trailing blank row whole, with dim `↑ n more` / `↓ n more` counting the
  fields not drawn at all;
- the header names the record (§2) and carries ` ● unsaved · Ctrl+S ` while
  `draft != original`.

The rules are stated once, with every frame, in
[`specs/008`](../specs/008-record-editor.md) "The form, stated once".

### 5.4 Confirm dialogs

Delete, T10's:

```
              ┌─ Delete record ────────────────────────┐
              │                                        │
              │  Delete "Hello world" from Posts?      │
              │  This cannot be undone.                │
              │                                        │
              │           y Delete    n Cancel         │
              └────────────────────────────────────────┘
```

Discard, T9's — `Esc` on a dirty editor. It names the changes rather than the
record, so it cannot be mistaken for the delete prompt it is shaped like. Its
rows are cleared across the panel; the form stays drawn above and below it.

```
              ┌─ Discard changes ──────────────────────┐
              │                                        │
              │  Discard unsaved changes?              │
              │  This cannot be undone.                │
              │                                        │
              │         y Discard    n Cancel          │
              └────────────────────────────────────────┘
```

Both key lines come from the keymap, joined with four spaces.

### 5.5 Search

The search overlay is a one-line prompt above the hint bar; the table filters
as you type. `Enter` keeps the filter and returns focus to the list, `Esc`
clears it.

```
├───────────────────────────────────────────────────────────────────────────┤
│ / hello▌                                                    2 of 14 match │
├───────────────────────────────────────────────────────────────────────────┤
```

### 5.6 Help

Rendered from the keymap, grouped by context, centred over the current screen.
No hand-written help text exists anywhere in the codebase.

## 6. Rendering rules

- `view` is a pure function of `&AppState`; it performs no I/O and never
  mutates state. Everything it needs must already be in the state.
- Colours are limited to the terminal's 16-colour palette so themes stay
  readable everywhere. Selection is reverse video, errors are red, hints are
  dim. No truecolour, no background fills.
- No Unicode beyond box-drawing characters, `✓`, `·`, `▸`, `‹›`, `⚠`, `●`,
  `—`, `↑`, `↓`, `…` and `▌`. The arrows are the scroll summaries the sidebar has
  drawn since T7; `·` is a false boolean, `—` an absent value, and `▌` the
  editor's cursor, drawn only while a field is being typed into.
- Every panel that can overflow scrolls; nothing is ever clipped silently.

## 7. Terminal lifecycle

Raw mode and the alternate screen are entered in one place and restored in one
place, including on panic:

```rust
let _guard = TerminalGuard::enter()?;   // Drop restores, plus a panic hook
```

A TUI that leaves the terminal in raw mode on a crash is unusable, and agents
crash things constantly. This is tested: a test that panics inside the guard
asserts the restore ran.

`tracing` writes to `.pressa/pressa.log` only. Nothing may print to stdout or
stderr while the TUI owns the screen.

## 8. Testing

Two layers, both mandatory from the first TUI task:

1. **`update` unit tests** — no terminal, no repository. Build a state, send a
   command, assert the new state and the returned effects.
   ```rust
   let mut s = state_with_records(3);
   let fx = update(&mut s, Command::MoveDown);
   assert_eq!(s.list.selected, 1);
   assert!(fx.is_empty());
   ```
2. **Snapshot tests** — `ratatui::TestBackend` at a fixed 80×24, rendered into
   a buffer and captured with `insta`.
   ```rust
   let backend = TestBackend::new(80, 24);
   let mut terminal = Terminal::new(backend)?;
   terminal.draw(|f| view(&state, f))?;
   insta::assert_snapshot!(terminal.backend());
   ```

This pair is what gives an agent a deterministic pass/fail signal on a UI it
cannot see. It is not optional and not deferred: the first TUI task ships with
snapshots. Review snapshot diffs like code — `cargo insta review`, never
`--accept` blindly.
