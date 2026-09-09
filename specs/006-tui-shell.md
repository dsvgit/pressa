# SPEC-006: TUI shell

Status: **Implemented** · Task: T7 · Crates: `pressa-tui`, `pressa-app` (re-exports only)
· ADRs: [0001](../docs/adr/0001-ratatui-and-crossterm.md),
[0004](../docs/adr/0004-schema-driven-ui.md),
[0005](../docs/adr/0005-command-and-keymap-architecture.md),
[0008](../docs/adr/0008-tracing-to-a-log-file.md),
[0010](../docs/adr/0010-test-only-dependencies-need-an-adr.md),
[0011](../docs/adr/0011-insta-for-snapshot-tests.md)

## Problem

`pressa dev` performs the whole startup sequence — project resolution, logging,
the database, migrations, both services — and then returns
`CliError::NotImplementedYet`, printing `pressa: the TUI arrives in T7` and
exiting 1 ([SPEC-005](005-cli.md) screen I). Nothing in the workspace draws
anything: `pressa-tui` has no `ratatui` dependency, no `AppState`, no `update`,
no keymap and no terminal lifecycle. The four layers below it are tested and
unreachable.

There is also no frame for T8 and T9 to render inside. A list view written
before a shell exists has to invent its own layout, its own key handling and
its own way of getting on screen, and the two later tasks would each invent a
different one.

## Goal

`pressa dev` opens a screen and gives it back cleanly. The shell owns the
terminal for the length of the session — raw mode and the alternate screen
entered once, restored on exit, on error and on panic — and draws the frame
every later screen lives in: a header of breadcrumbs derived from the route, a
sidebar of the schema's collections with a selection, a main panel, a status
line and a hint bar generated from the keymap. Keys reach the application only
as `Command` values resolved through one static table, and the only place state
changes is `update(&mut AppState, Command) -> Vec<Effect>`, which performs no
I/O and is tested without a terminal. `Enter` on a collection routes to it and
`Esc` comes back, so the two routes T8 fills in already exist. Below 60x16 the
shell says so instead of drawing a broken layout.

## Non-goals

This task builds the frame, not what goes in it. A reader may reasonably expect
the following here and will not get it:

| Not here | Where it lives instead |
|---|---|
| The record table: columns from `list_columns`, cell renderers, `j`/`k` over rows, viewport scrolling, the empty state, the record count in the header. `Route::List` exists and draws a placeholder (frame H) | [007](007-list-view.md), T8 |
| The record form: field editors, dirty tracking, `Ctrl+S`, validation errors under fields, `EditorInput` mode | [008](008-record-editor.md), T9 |
| Every overlay, **including help**. `Overlay`, `Context::Overlay`, the confirm dialog, the search prompt and the `?` screen. `?` is therefore *not bound* in T7 and `? Help` is *not* in the hint bar: a binding that is in the table works, and one that is not does not exist ([ADR-0005](../docs/adr/0005-command-and-keymap-architecture.md)). T11 adds the row and re-takes the snapshots | 009 (T10), 010 (T11) |
| Writing the `collections` config snapshot on startup. Deferred to M3, where [`storage.md`](../docs/storage.md) §2 says it is first read: nothing in M0 reads it, `RecordRepository` has no method for it, and `Collection` is not `Serialize`, so writing it now costs a port method, both adapters, a contract-suite case and an ADR — for a table M0 never opens | M3, [roadmap](../docs/roadmap.md) §3 |
| `AppState` at its full [`tui.md`](../docs/tui.md) §1 shape. T7 adds `schema`, `route`, `sidebar`, `status`, `should_quit`; `list`, `editor` and `overlay` arrive with the tasks that render them, so that no task ships a field nothing reads | T8, T9, T10 |
| The `Effect` variants of [`tui.md`](../docs/tui.md) §1. They name `ListParams`, `Record` and `serde_json::Value`, and T7 asks the world for nothing but the terminal — `Select` changes a route and loads nothing. `Effect` is declared uninhabited and each variant arrives with the effect runner that serves it | T8 onward |
| `run_effects`. With no inhabited `Effect` there is nothing to run; the loop calls `update` and discards an always-empty vector | T8 |
| Mouse input, resize animation, a tick timer, a frame rate. The loop blocks on one event and redraws after it | not in M0 |
| Colour beyond the 16-colour palette, background fills, themes, a `--no-color` flag | [`tui.md`](../docs/tui.md) §6 |
| Sidebar width negotiation. [`tui.md`](../docs/tui.md) §4 allows 14 as a minimum; with a 60-column floor the sidebar never has to shrink, so it is a constant 20 and the minimum is unused | reconsider if the floor drops |
| Rebindable keys, a config file, vim/emacs modes. One static table, compiled in | M1+, [ADR-0005](../docs/adr/0005-command-and-keymap-architecture.md) "Consequences" |
| A pty or terminal-emulator test. The terminal half is tested through a seam (`TerminalOps`) and a `TestBackend`; a real tty is never opened by a test | [000](000-m0-golden-path.md) "Non-goals" |

## User stories

- As a user, I want `pressa` to open a screen listing my collections, so that
  the schema I wrote is something I can look at rather than validate.
- As a user, I want `Enter` on a collection to take me into it and `Esc` to
  bring me back, so that the sidebar is navigation and not decoration.
- As a user, I want the bottom line to tell me which keys do something here, so
  that I never have to read documentation to use the application.
- As a user whose terminal is too small, I want to be told that, so that I do
  not diagnose a broken layout.
- As a user of an application written by agents, I want a crash to leave my
  shell usable, so that a panic costs me a rerun rather than a `reset`.
- As an implementer of T8 and T9, I want a layout, a keymap, routes and a pure
  `update` already in place, so that my task adds a panel rather than an
  architecture.

## UX

Every frame below is exactly 80x24 unless it says otherwise — the size the
`insta` snapshots are taken at. The frames are normative for layout, spacing
and wording.

Layout, stated once and used by every screen ([`tui.md`](../docs/tui.md) §4):

```text
rows (height H)                        columns (width W)
  1        header: breadcrumbs           1        left border
  1        separator                     20       sidebar
  H-7      body                          1        divider
  1        separator                     W-23     main panel
  1        status line                   1        right border
  1        separator
  1        hint bar
  1        bottom border
```

At 80x24 the body is 17 rows and the main panel 57 columns; at the 60x16
minimum, 9 rows and 37 columns.

### A — Home, `examples/blog`

```text
┌ pressa ──────────────────────────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │                                                         │
│                    │                                                         │
│ > Posts            │                                                         │
│                    │                                                         │
│                    │                                                         │
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
├────────────────────┴─────────────────────────────────────────────────────────┤
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Navigate   Enter Open   q Quit                                            │
└──────────────────────────────────────────────────────────────────────────────┘
```

Rules this frame is produced by:

- Header: `┌ ` + breadcrumbs joined with ` › ` + ` ` + fill. Home is `pressa`.
- Sidebar: the title `Collections`, a blank row, then one row per collection in
  schema order. The selected row is prefixed `> `, the others three spaces. It
  is drawn in reverse video only while the sidebar has focus — that is, at
  `Route::Home` ([`tui.md`](../docs/tui.md) §6); a text snapshot does not show
  reverse video, hence the marker and a separate style assertion. A label longer
  than the 17 usable columns is truncated with `…`.
- Main panel, Home: `Select a collection to begin.`, a blank row, and
  `<project name> · <n> collection(s)`, each centred horizontally, the
  three-row block centred vertically in the body. The noun pluralises on `n`,
  by the same rule `pressa validate` already uses.
- Status line: empty on Home.
- Hint bar: generated, see "Hints" below.

This is the Golden Path's step 3 ([000](000-m0-golden-path.md) §A) minus the
`? Help` hint, which T11 adds with the help overlay, and with the empty-state
block two rows lower — see "Decisions taken" (Q5).

### B — Home, three collections, selection at the top

The snapshot fixture for movement; `blog-cms` with `Posts`, `Authors`,
`Categories`, matching [`tui.md`](../docs/tui.md) §5.1.

```text
┌ pressa ──────────────────────────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │                                                         │
│                    │                                                         │
│ > Posts            │                                                         │
│   Authors          │                                                         │
│   Categories       │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │              Select a collection to begin.              │
│                    │                                                         │
│                    │                blog-cms · 3 collections                 │
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
│ ↑↓ Navigate   Enter Open   q Quit                                            │
└──────────────────────────────────────────────────────────────────────────────┘
```

### C — The same, after `j` `j`

A third `j` changes nothing: the selection clamps at the last collection and
never wraps.

```text
┌ pressa ──────────────────────────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │                                                         │
│                    │                                                         │
│   Posts            │                                                         │
│   Authors          │                                                         │
│ > Categories       │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │              Select a collection to begin.              │
│                    │                                                         │
│                    │                blog-cms · 3 collections                 │
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
│ ↑↓ Navigate   Enter Open   q Quit                                            │
└──────────────────────────────────────────────────────────────────────────────┘
```

### D — An error in the status line

`StatusKind::Error` prefixes `⚠ ` and renders red; `StatusKind::Info` renders
the text plain. Nothing in T7 sets a status — the state is reachable only by
constructing it — but the shell renders it, so the frame is specified here
rather than invented by T8.

```text
┌ pressa ──────────────────────────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │                                                         │
│                    │                                                         │
│ > Posts            │                                                         │
│                    │                                                         │
│                    │                                                         │
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
├────────────────────┴─────────────────────────────────────────────────────────┤
│ ⚠ unknown collection: drafts                                                 │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Navigate   Enter Open   q Quit                                            │
└──────────────────────────────────────────────────────────────────────────────┘
```

### E — Home at the 60x16 minimum

The smallest size that still draws the layout. Body 9 rows, main panel 37
columns; the sidebar stays 20.

```text
┌ pressa ──────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────┤
│ Collections        │                                     │
│                    │                                     │
│ > Posts            │                                     │
│                    │    Select a collection to begin.    │
│                    │                                     │
│                    │       blog-cms · 1 collection       │
│                    │                                     │
│                    │                                     │
│                    │                                     │
├────────────────────┴─────────────────────────────────────┤
│                                                          │
├──────────────────────────────────────────────────────────┤
│ ↑↓ Navigate   Enter Open   q Quit                        │
└──────────────────────────────────────────────────────────┘
```

### F — A sidebar taller than the body

Twenty collections in a 15-row list area, selection on the last. Nothing is
clipped silently ([`tui.md`](../docs/tui.md) §6): the viewport scrolls to keep
the selection visible, and the row that would have shown a hidden neighbour
shows a dim `↑ n more` or `↓ n more` instead.

```text
┌ pressa ──────────────────────────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │                                                         │
│                    │                                                         │
│ ↑ 6 more           │                                                         │
│   Coll 07          │                                                         │
│   Coll 08          │                                                         │
│   Coll 09          │                                                         │
│   Coll 10          │                                                         │
│   Coll 11          │              Select a collection to begin.              │
│   Coll 12          │                                                         │
│   Coll 13          │            many-collections · 20 collections            │
│   Coll 14          │                                                         │
│   Coll 15          │                                                         │
│   Coll 16          │                                                         │
│   Coll 17          │                                                         │
│   Coll 18          │                                                         │
│   Coll 19          │                                                         │
│ > Coll 20          │                                                         │
├────────────────────┴─────────────────────────────────────────────────────────┤
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Navigate   Enter Open   q Quit                                            │
└──────────────────────────────────────────────────────────────────────────────┘
```

### G — Terminal too small

Drawn whenever the frame is narrower than 60 columns or shorter than 16 rows.
No border, no sidebar, no hint bar: two centred lines on an otherwise blank
screen, redrawn on every resize until the terminal is large enough. `x` rather
than `×`, which is outside the character set [`tui.md`](../docs/tui.md) §6
allows.

At 40x10:

```text
+----------------------------------------+   <- not drawn: the terminal edge
|                                        |
|                                        |
|                                        |
|                                        |
|           terminal too small           |
|       needs 60x16, this is 40x10       |
|                                        |
|                                        |
|                                        |
|                                        |
+----------------------------------------+
```

At 80x15 — one row short:

```text
+--------------------------------------------------------------------------------+
|                                                                                |
|                                                                                |
|                                                                                |
|                                                                                |
|                                                                                |
|                                                                                |
|                               terminal too small                               |
|                           needs 60x16, this is 80x15                           |
|                                                                                |
|                                                                                |
|                                                                                |
|                                                                                |
|                                                                                |
|                                                                                |
|                                                                                |
+--------------------------------------------------------------------------------+
```

### H — `Route::List`, before T8

`Enter` on `Posts` at Home. The header, the sidebar, the status line and the
hint bar are the shell's and are final; the main panel is a placeholder that T8
replaces with the table, the way `cli::dev` carried `NotImplementedYet` through
T6. The sidebar keeps its `> ` marker but loses the reverse video, because the
sidebar no longer has focus.

```text
┌ pressa › Posts ──────────────────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │                                                         │
│                    │                                                         │
│ > Posts            │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │                                                         │
│                    │              The list view arrives in T8.               │
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
│ Esc Back                                                                     │
└──────────────────────────────────────────────────────────────────────────────┘
```

The record count on the right of the header (`── 2 records ──` in
[000](000-m0-golden-path.md) §B) is T8's: T7 has loaded nothing and will not
print a number it does not have.

### I — `pressa dev` on a terminal it cannot take over

The CLI's screens ([005](005-cli.md) §UX) still apply before the shell starts.
Screen I of that spec — `pressa: the TUI arrives in T7`, exit 1 — is retired by
this task and replaced by:

```text
$ pressa dev < /dev/null > /dev/null      # not a terminal
pressa: the terminal could not be prepared: <os error>
→ exit 1
```

On success `pressa dev` prints nothing at all: the screen is the output, and
`q` returns the shell exactly as it was, with exit 0.

## Domain model

New in `pressa-tui`.

```rust
pub struct AppState {
    /// `pressa_app::domain::Schema`, re-exported from `pressa-core` — the TUI
    /// still sees only `pressa-app` types (architecture.md §2).
    pub schema: Schema,
    pub route: Route,
    pub sidebar: SidebarState,
    pub status: Option<StatusMessage>,
    pub should_quit: bool,
}

pub enum Route {
    Home,
    List { collection: String },
}

pub struct SidebarState {
    /// Index into `schema.collections`. Always < len, which is >= 1: the loader
    /// rejects a schema with no collections (SPEC-001).
    pub selected: usize,
    /// First visible row of the list area, moved only to keep `selected`
    /// visible, so that `view` never has to mutate anything.
    pub offset: usize,
}

pub struct StatusMessage {
    pub text: String,
    pub kind: StatusKind,
}

pub enum StatusKind { Info, Error }

pub enum Command {
    MoveUp,
    MoveDown,
    Select,
    Back,
    Quit,
}

/// Uninhabited in T7: the shell asks the world for nothing but the terminal.
/// Each variant arrives with the effect runner that serves it.
pub enum Effect {}

pub enum Context { Global, Sidebar, List }

pub struct KeyBinding {
    pub context: Context,
    pub key: KeyEvent,
    pub command: Command,
    pub description: &'static str,
    pub hint: Hint,
}

/// Replaces `show_in_hint_bar: bool` from `tui.md` §3, which cannot express
/// "the pair `j` and `↓` reads as `↑↓`". One field rather than two, so a
/// hidden binding with a hint label is unrepresentable rather than merely
/// wrong. `docs/tui.md` §3 is updated in the same PR.
pub enum Hint {
    Hidden,
    Shown,
    ShownAs(&'static str),
}
```

### The keymap

The whole table in T7. `?` is absent until T11 owns the help overlay.

| # | Context | Key | Command | Description | Hint |
|---|---|---|---|---|---|
| 1 | Sidebar | `j` | MoveDown | Navigate | `ShownAs("↑↓")` |
| 2 | Sidebar | `↓` | MoveDown | Navigate | Hidden |
| 3 | Sidebar | `k` | MoveUp | Navigate | Hidden |
| 4 | Sidebar | `↑` | MoveUp | Navigate | Hidden |
| 5 | Sidebar | `Enter` | Select | Open | Shown |
| 6 | Sidebar | `l` | Select | Open | Hidden |
| 7 | Sidebar | `→` | Select | Open | Hidden |
| 8 | Sidebar | `q` | Quit | Quit | Shown |
| 9 | List | `Esc` | Back | Back | Shown |
| 10 | List | `h` | Back | Back | Hidden |
| 11 | List | `←` | Back | Back | Hidden |
| 12 | List | `q` | Back | Back | Hidden |
| 13 | Global | `Ctrl+C` | Quit | Quit | Hidden |

`q` is `Quit` in the sidebar and `Back` in a list, which is
[`tui.md`](../docs/tui.md) §3's "Back / Quit at Home" expressed as two rows
rather than as a branch inside `update`.

Resolution: a key matches a binding when the key code and the modifiers are
equal, except that `SHIFT` on a `Char` is ignored — the character already
carries the case. Bindings in the current context are searched first, then
`Context::Global`, so a context can shadow a global key. A key in no binding
produces no command and nothing happens.

`Context` is derived from the state, never stored:
`Route::Home → Sidebar`, `Route::List → List`.

### Hints

`hints(context) -> Vec<String>` returns, in table order, one
`"<key label> <description>"` per non-`Hidden` binding of that context, then the
same for the non-`Hidden` `Global` bindings. The hint bar is those strings
joined with **three** spaces and prefixed with one.

```text
 ↑↓ Navigate   Enter Open   q Quit        Context::Sidebar
 Esc Back                                 Context::List, until T8 adds its rows
```

Key labels: a printable char as itself, `Enter`, `Esc`, `Tab`, `Backspace`,
`Ctrl+<c>`, arrows as `↑` `↓` `←` `→`, or the string inside `ShownAs`.

### Breadcrumbs

```rust
fn breadcrumbs(route: &Route, schema: &Schema) -> Vec<String>
```

| Route | Breadcrumbs | Header |
|---|---|---|
| `Home` | `["pressa"]` | `┌ pressa ───…` |
| `List { "posts" }` | `["pressa", "Posts"]` | `┌ pressa › Posts ───…` |

The second element is the collection's `label`, from the schema; a slug the
schema does not know is printed as the slug, so a stale route is visible rather
than a panic. The function is total, pure and takes no `AppState`, which is
what makes it testable independently of any screen. T9's
`["pressa", "Posts", "Edit", "01J8XQ…"]` — a record id shortened to its first
six characters and `…` — is added by T9 with its route.

## API

`pressa-app` gains one file of re-exports and nothing else:

```rust
// pressa-app/src/domain.rs — the domain types the UI renders, re-exported so
// that `pressa-tui` still names only `pressa-app` (architecture.md §2).
pub use pressa_core::schema::{Collection, Schema};
```

Modules added under `pressa-tui/src/`:

```text
tui/mod.rs        pub fn run(schema: Schema) -> Result<(), TuiError>
tui/terminal.rs   TerminalGuard, TerminalOps, install_panic_hook
tui/state.rs      AppState, Route, SidebarState, StatusMessage, StatusKind
tui/command.rs    Command, Effect
tui/keymap.rs     Context, KeyBinding, Hint, KEYMAP, resolve, hints, key_label
tui/update.rs     update
tui/view/mod.rs   view, layout, breadcrumbs
tui/view/sidebar.rs, tui/view/chrome.rs
```

```rust
/// The event loop. Enters the terminal, draws, reads, updates, and restores
/// the terminal on every exit path. Returns when the state says to quit.
pub fn run(schema: Schema) -> Result<(), TuiError>;

/// Raw mode + alternate screen on `enter`, restored on `Drop`, on `?` and on
/// panic. `enter` also installs the panic hook, once per process.
pub struct TerminalGuard { /* … */ }
impl TerminalGuard {
    pub fn enter() -> Result<TerminalGuard, TuiError>;
    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>>;
}

/// The seam the restore is tested through: the guard is generic over it, and
/// the test double records the calls instead of touching a real terminal.
pub trait TerminalOps {
    fn enter(&mut self) -> std::io::Result<()>;
    fn leave(&mut self) -> std::io::Result<()>;
}

/// Restores the terminal, then calls the hook that was installed before it, so
/// the panic message lands on a cooked screen. Idempotent: a panic that also
/// runs `Drop` restores once.
pub fn install_panic_hook();

/// Pure. The only place `AppState` changes, and it performs no I/O.
pub fn update(state: &mut AppState, cmd: Command) -> Vec<Effect>;

/// Pure. Reads the state and nothing else.
pub fn view(state: &AppState, frame: &mut ratatui::Frame);

pub fn resolve(context: Context, key: KeyEvent) -> Option<Command>;
pub fn hints(context: Context) -> Vec<String>;

#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("the terminal could not be prepared: {0}")]
    Enter(std::io::Error),
    #[error("the screen could not be drawn: {0}")]
    Draw(std::io::Error),
    #[error("the terminal stopped sending input: {0}")]
    Input(std::io::Error),
    #[error("the terminal could not be restored: {0}")]
    Leave(std::io::Error),
}
```

`cli.rs` changes: `CliError::NotImplementedYet` is deleted, `CliError::Tui(
#[from] TuiError)` replaces it, and `dev` ends with `tui::run(schema)` instead
of `Err(NotImplementedYet)`. The services are still constructed there, and the
`services ready` log line stays: a failure to open the database must still
surface before the screen is taken over. `RecordService` remains unused until
T8 gives the loop an effect runner.

## Invariants

- The terminal is restored exactly once per `TerminalGuard`, on every exit
  path: normal return, `?`, and panic.
- `update` performs no I/O and touches nothing but the `AppState` it is given.
- `view` performs no I/O and never mutates state.
- No key is compared to a `KeyCode` outside `keymap.rs`.
- Every string in the hint bar comes from a binding in `KEYMAP`; there is no
  literal hint text anywhere in the crate.
- `sidebar.selected` is always a valid index into `schema.collections`, which
  always has at least one entry, and `Route::List` always names a collection
  the schema has.
- Nothing writes to stdout or stderr between `TerminalGuard::enter` and its
  drop; diagnostics go to `.pressa/pressa.log`
  ([ADR-0008](../docs/adr/0008-tracing-to-a-log-file.md)).
- No screen, widget or code path branches on a collection slug
  ([ADR-0004](../docs/adr/0004-schema-driven-ui.md)).

## Error cases

| Input | Result |
|---|---|
| `pressa dev` where stdout is not a terminal | `pressa: the terminal could not be prepared: <os error>`, exit 1, terminal untouched |
| A draw fails mid-session | terminal restored, `pressa: the screen could not be drawn: <os error>`, exit 1 |
| Reading an event fails | terminal restored, `pressa: the terminal stopped sending input: <os error>`, exit 1 |
| A panic anywhere inside the loop | terminal restored first, then the panic message on a cooked screen |
| Terminal smaller than 60x16 | screen G; no error, no exit |
| A key bound to nothing | ignored; no status message, no beep |

## Acceptance criteria

Terminal lifecycle

- [x] `TerminalGuard::enter` puts the terminal in raw mode and on the alternate
      screen, and dropping it leaves both — asserted through `TerminalOps` on a
      recording double, in order.
- [x] A panic inside the scope of a guard restores the terminal exactly once:
      `catch_unwind` around a scope holding the guard, then assert the recorder
      saw one `leave`.
- [x] `install_panic_hook` calls the previously installed hook after restoring,
      so the panic message is not swallowed.
- [x] `run` returns `TuiError::Enter` when entering fails, and the process
      prints one line to stderr starting with `pressa: ` and exits 1.

Layout

- [x] An 80x24 snapshot of Home on `examples/blog/pressa.yaml` matches frame A.
- [x] A 60x16 snapshot of the same state matches frame E.
- [x] A 79x24 render and an 80x15 render both produce screen G, and 80x16 does
      not — the boundary is asserted on both axes.
      *Tested at 59x24, not 79x24: frame E draws the whole layout at 60 columns,
      so the column that breaks the 60-column floor is 59. 60x16 is asserted as
      not-too-small alongside 80x16.*
- [x] Screen G names the actual size: at 40x10 the second line is
      `needs 60x16, this is 40x10`.
- [x] An 80x24 snapshot of `Route::List { "posts" }` matches frame H, header
      `pressa › Posts` included.
- [x] The header is built by `breadcrumbs`, not by string concatenation at the
      call site: a unit test calls it for both routes and asserts the vectors.
- [x] `breadcrumbs` on a route naming a collection the schema does not have
      returns the slug and does not panic.

Sidebar

- [x] With three collections, the initial 80x24 snapshot matches frame B and
      the snapshot after two `MoveDown` matches frame C.
- [x] A third `MoveDown` leaves `sidebar.selected` at 2, and `MoveUp` at the
      top leaves it at 0 — clamped, never wrapping.
- [x] The selected sidebar row is reverse video at `Route::Home` and is not at
      `Route::List`, asserted on the buffer's cell styles rather than on the
      text snapshot; the `> ` marker is present in both.
- [x] With 20 collections and the selection on the last, the snapshot matches
      frame F: the top list row reads `↑ 6 more` and the selection is visible.
- [x] A collection whose label exceeds the sidebar's 17 usable columns is
      truncated with `…` and never overflows into the divider.
- [x] The sidebar lists collections in schema order for a schema whose
      collections are not alphabetical, proving the order comes from the
      `IndexMap` rather than from a sort.

Routing

- [x] `Select` at Home sets `Route::List` naming the selected collection, and
      returns no effects.
- [x] `Back` at `Route::List` returns to `Route::Home` with the sidebar
      selection unchanged.
- [x] `Select` routes to whichever collection is selected, asserted for two
      different indices, so no code path names a slug.

Keymap and hints

- [x] `resolve(Context::Sidebar, k)` returns `MoveDown` for `j` and `↓`,
      `MoveUp` for `k` and `↑`, `Select` for `Enter`, `l` and `→`, and `Quit`
      for `q`.
- [x] `resolve(Context::List, k)` returns `Back` for `Esc`, `h`, `←` and `q` —
      the same key that quits at Home goes back in a list.
- [x] `resolve` returns `Quit` for `Ctrl+C` in both contexts, and `None` for a
      key in no binding (`z`, and `?` until T11).
- [x] `resolve(Context::Sidebar, Char('c'))` is `None` while `Ctrl+C` is
      `Quit` — modifiers are part of the match.
- [x] The hint bar rows of frames A and H equal `hints(..)` for their contexts
      joined with three spaces, computed from `KEYMAP` in the test; no test and
      no renderer contains the literal `Navigate`.
- [x] For every binding the hint bar shows, `resolve` on that binding's key
      returns that binding's command — a hint cannot name a key that does
      nothing.
- [x] `grep` finds no `KeyCode::` outside `keymap.rs` — asserted by a test that
      reads the crate's own sources, in the style of
      `pressa-tui/tests/architecture.rs`.

`update`

- [x] `update` is a pure function: the same state and command produce the same
      state, and the returned `Vec<Effect>` is empty for every T7 command.
- [x] `Command::Quit` sets `should_quit`, and the loop exits on the next pass
      with `Ok(())` and exit code 0.
      *The loop half is asserted through `tui::drive`, the loop over an injected
      terminal and event source; the exit code is `cli::run`'s existing
      `Ok` → `ExitCode::SUCCESS`, covered by SPEC-005's tests.*
- [x] `Command::MoveDown` on the last collection returns no effects and leaves
      the state equal to what it was.

Status line

- [x] `StatusKind::Error` renders `⚠ ` before the text and in red, asserted on
      the cell styles; `StatusKind::Info` renders the text with neither.
- [x] The 80x24 snapshot of Home with an error status matches frame D.

Startup and the CLI

- [x] `pressa dev` on `examples/blog` no longer prints `the TUI arrives in T7`,
      and `CliError::NotImplementedYet` no longer exists.
- [x] `.pressa/pressa.log` gains a line when the loop starts and one when it
      exits cleanly; nothing is written to stdout or stderr on the success path.
      *The two log lines are asserted. The silent success path is not: observing
      it needs a pty, which "Non-goals" rules out. Nothing in the crate writes to
      either stream outside `cli.rs`, which runs before the guard.*
- [x] The `sandbox` recipe in the `justfile` no longer promises
      `pressa: the TUI arrives in T7`, and its README says what `dev` does now
      (`AGENTS.md`, [`development.md`](../docs/development.md) §8).

Documents

The approval PR already carried the edits the six answers implied:
[000](000-m0-golden-path.md) §A centred (Q5), [`tui.md`](../docs/tui.md) §3's
`Hint` and split `q` rows (Q6), [`storage.md`](../docs/storage.md) §2 and
[`roadmap.md`](../docs/roadmap.md) §3 on the deferred snapshot (Q3), and
[`architecture.md`](../docs/architecture.md) §2 on `pressa_app::domain` and
`insta` (Q1, Q2). What is left belongs with the code that changes:

- [x] [005](005-cli.md): screen I and the two criteria asserting
      `the TUI arrives in T7` are marked superseded by this spec.

Boundaries

- [x] `pressa-tui/Cargo.toml` gains `ratatui` and `crossterm` as its only new
      runtime dependencies ([ADR-0001](../docs/adr/0001-ratatui-and-crossterm.md))
      and `insta` as its only new dev-dependency (ADR-0011); `pressa-core`,
      `pressa-storage` and `pressa-app` gain no dependency at all.
- [x] `pressa-tui` names no crate but `pressa-app` outside `cli::dev`: the
      domain types reach it through `pressa_app::domain`, and `pressa-core` is
      absent from its `Cargo.toml` in every section.
- [x] `pressa-tui/tests/architecture.rs` gains `pressa-core` to `pressa-tui`'s
      forbidden list, matching [`architecture.md`](../docs/architecture.md) §2,
      and `rusqlite` is still absent from every section.
- [x] No `unwrap()`, `expect()` or `panic!()` outside `#[cfg(test)]`.
- [x] No identifier in `pressa-tui` is a collection slug
      ([ADR-0004](../docs/adr/0004-schema-driven-ui.md)).

## Tests

`pressa-tui/tests/`, plus unit tests beside the modules they cover. No test
opens a real terminal and no test opens a database.

| Criteria | Test |
|---|---|
| Terminal enter/leave, panic restore, hook chaining | unit tests in `tui/terminal.rs` over a `TerminalOps` double and `catch_unwind` |
| Frames A–H | `insta` snapshots via `ratatui::TestBackend` at the stated size, in `tests/shell_snapshots.rs` |
| Reverse video, red status, dim `↑ n more` | assertions on `Buffer` cell styles, in the same file, not on the snapshot text |
| Sidebar movement, clamping, order, scrolling; `Select` and `Back` | `update` unit tests: build a state, send a command, assert state and effects |
| `resolve`, `hints`, hint/binding agreement | unit tests in `tui/keymap.rs` driving the table itself |
| No `KeyCode::` outside the keymap | a source-scanning test next to `tests/architecture.rs` |
| `dev` no longer prints the stub, exit codes, log lines | `assert_cmd` in the existing `tests/cli.rs`, with `--project` on a `TempTree` |

Fixtures: `examples/blog/pressa.yaml` for frames A, D, E and H, used directly so
the example cannot drift; a three-collection and a twenty-collection schema
built in the test for frames B, C and F. Snapshots are reviewed with
`cargo insta review`, never accepted blindly.

## Open questions

None. The six this spec was drafted with were answered by the coordinator on
2026-09-09 and are folded in above.

### Decisions taken

- **Q1 — `insta`.** Admitted as a test-only dependency by
  [ADR-0011](../docs/adr/0011-insta-for-snapshot-tests.md), accepted 2026-09-09,
  under the same test ADR-0010 applied to `assert_cmd`: what the test prints
  when it fails. Nothing further blocks implementation.
- **Q2 — naming `Schema`.** `pressa-app` re-exports the domain types the UI
  renders (`pressa_app::domain`). No new Cargo edge, no ADR, and
  [`architecture.md`](../docs/architecture.md) §2's rule — "everywhere else it
  sees only `pressa-app` types" — is implemented literally rather than amended.
  T8 and T9 extend the re-export list as they need `Record`, `RecordId` and
  `FieldError`.
- **Q3 — the `collections` snapshot.** Deferred to M3, where it is first read.
  M0 never opens the table, and writing it now would cost a port method in
  `pressa-core`, both adapters, a contract-suite case and an ADR. Removed from
  T7's scope; `storage.md` §2 and `roadmap.md` §3 are corrected in this PR.
- **Q4 — bindings whose behaviour lands later.** `?` is *not* bound in T7: the
  help overlay is T11's, and a hint may not name a key that does nothing.
  `Enter` *is* bound, because routing is the shell's own job — `Select` sets
  `Route::List` and the main panel draws frame H's placeholder until T8 fills
  it. [000](000-m0-golden-path.md) §A stays as it is on the `? Help` hint: it
  describes Home after T11, not after T7.
- **Q5 — frame A against [000](000-m0-golden-path.md) §A.** The rule wins: the
  three-row block is centred vertically, as [`tui.md`](../docs/tui.md) §5.1
  already is. §A's frame moves two rows down in this PR. Hint strings are joined
  with three spaces everywhere; §B's two-space frame is corrected by T8.
- **Q6 — `KeyBinding`'s hint.** `show_in_hint_bar: bool` becomes
  `Hint { Hidden, Shown, ShownAs(&'static str) }`, so that `j` and `↓` can read
  as one `↑↓ Navigate` and a hidden binding cannot carry a label.
  `docs/tui.md` §3 is updated in the same PR.
