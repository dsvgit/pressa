# Terminal UI

Status: Approved · Last updated: 2026-09-06 · Crate: `pressa-tui`

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
    LoadRecords { collection: String, params: ListParams },
    LoadRecord  { collection: String, id: RecordId },
    SaveRecord  { collection: String, id: Option<RecordId>, data: serde_json::Value },
    DeleteRecord{ collection: String, id: RecordId },
}
```

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
fn breadcrumbs(route: &Route, schema: &Schema) -> Vec<String>
// Route::Edit{"posts", id} → ["pressa", "Posts", "Edit", "01J8XQ…"]
```

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
    pub context: Context,       // Sidebar | List | Editor | EditorInput | Overlay
    pub key: KeyEvent,
    pub command: Command,
    pub description: &'static str,
    pub show_in_hint_bar: bool,
}
```

The keymap is **one static table**. Both the bottom hint bar and the `?` help
overlay are generated from it by filtering on the current context. A key that
exists but is undocumented, or documented but unbound, is therefore not
possible. See [ADR-0005](adr/0005-command-and-keymap-architecture.md).

### M0 key bindings

| Context | Key | Command | Hint bar |
|---|---|---|---|
| global | `?` | OpenHelp | yes |
| global | `q` | Back / Quit at Home | yes |
| global | `Ctrl+C` | Quit | no |
| Sidebar | `j` / `↓` | MoveDown | yes (as `↑↓`) |
| Sidebar | `k` / `↑` | MoveUp | — |
| Sidebar | `Enter` / `l` / `→` | Select → route to List | yes |
| List | `j` / `↓` | MoveDown | yes |
| List | `k` / `↑` | MoveUp | — |
| List | `g` / `G` | GoToTop / GoToBottom | no |
| List | `Ctrl+D` / `Ctrl+U` | PageDown / PageUp | no |
| List | `Enter` | EditRecord | yes |
| List | `n` | NewRecord | yes |
| List | `d` | DeleteRecord → ConfirmDelete | yes |
| List | `/` | OpenSearch | yes |
| List | `r` | Refresh | no |
| List | `h` / `←` / `Esc` | FocusSidebar | yes (as `Esc Back`) |
| Editor | `Tab` / `j` | NextField | yes |
| Editor | `Shift+Tab` / `k` | PrevField | — |
| Editor | `Enter` | BeginEdit (text) / ToggleBoolean / NextOption (select) | yes |
| Editor | `Ctrl+S` | Save | yes |
| Editor | `Esc` | Back (ConfirmDiscard if dirty) | yes |
| EditorInput | printable | InputChar | — |
| EditorInput | `Backspace` | InputBackspace | — |
| EditorInput | `Enter` | CommitField | yes |
| EditorInput | `Esc` | CancelEdit — restores the original value | yes |
| Overlay | `y` / `Enter` | Confirm | yes |
| Overlay | `n` / `Esc` / `q` | Dismiss | yes |

Two modes inside the editor — `Editor` navigates between fields, `EditorInput`
types into one — is the vim distinction, and it is what keeps `j` usable for
navigation without stealing it from text.

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
│ Collections  │  TITLE            STATUS      VIEWS   UPDATED               │
│              │ ─────────────────────────────────────────────────────────  │
│ > Posts      │ ▸Hello world      draft           0   2m ago                │
│   Authors    │  About page       published      42   yesterday             │
│   Categories │                                                            │
│              │                                                            │
├──────────────┴────────────────────────────────────────────────────────────┤
│                                                                            │
├───────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Navigate  Enter Edit  n New  d Delete  / Search  Esc Back  ? Help       │
└───────────────────────────────────────────────────────────────────────────┘
```

Columns come from `list_columns`. Column widths are proportional to content
with a minimum of 6 and an ellipsis on overflow. Boolean renders as `✓` / `·`,
`Json` renders as `{…}` with the key count, `Null` renders as a dim `—`.

Empty state replaces the table body with:

```
   No records yet.  Press n to create the first one.
```

### 5.3 Record editor

```
┌ pressa › Posts › Edit ────────────────────────────── ● unsaved · Ctrl+S ──┐
├──────────────┬────────────────────────────────────────────────────────────┤
│ Collections  │  Title *                                                   │
│              │  ┌──────────────────────────────────────────────────────┐  │
│ > Posts      │  │ Hello world                                          │  │
│   Authors    │  └──────────────────────────────────────────────────────┘  │
│   Categories │                                                            │
│              │  Slug *                                                    │
│              │    hello-world                                             │
│              │                                                            │
│              │  Status *        ‹ draft ›                                 │
│              │                                                            │
│              │  Featured        [x]                                       │
│              │                                                            │
│              │  Views                                                     │
│              │    0                                                       │
│              │  ⚠ must be a number                                        │
│              │                                                            │
├──────────────┴────────────────────────────────────────────────────────────┤
│ 1 field needs attention                                                    │
├───────────────────────────────────────────────────────────────────────────┤
│ Tab Next  Enter Edit  Ctrl+S Save  Esc Back  ? Help                        │
└───────────────────────────────────────────────────────────────────────────┘
```

The focused field is boxed; unfocused fields are drawn as plain values. `*`
marks required. Validation messages appear directly under their field, which is
why `FieldError` carries a field name rather than being a string.

Per type: `Text` single line, `Textarea` a 5-row box, `Number` a text input
validated on commit, `Boolean` a `[x]` toggle, `Select` a `‹ value ›` cycler,
`DateTime` a text input in RFC 3339, `Json` a textarea validated on commit.

### 5.4 Confirm delete

```
              ┌─ Delete record ────────────────────────┐
              │                                        │
              │  Delete "Hello world" from Posts?      │
              │  This cannot be undone.                │
              │                                        │
              │           y Delete    n Cancel         │
              └────────────────────────────────────────┘
```

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
- No Unicode beyond box-drawing characters, `✓`, `▸`, `‹›`, `⚠`, `●` and `…`.
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
