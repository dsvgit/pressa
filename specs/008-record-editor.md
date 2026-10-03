# SPEC-008: Record editor

Status: **Approved** · Task: T9 · Crates: `pressa-tui`, plus one pure function in
`pressa-core` re-exported through `pressa-app` (Q2)
· ADRs: [0002](../docs/adr/0002-synchronous-rusqlite.md),
[0004](../docs/adr/0004-schema-driven-ui.md),
[0005](../docs/adr/0005-command-and-keymap-architecture.md),
[0011](../docs/adr/0011-insta-for-snapshot-tests.md)

> The fifteen questions this spec was drafted with were answered by the
> coordinator on 2026-10-03 and are folded in below; they are recorded under
> "Decisions taken" because six of them change another document.

## Problem

Pressa can read a record and cannot write one. Every layer under the screen is
finished and reachable only from a test: `RecordService::create`, `update`,
`get` and `blank` are implemented and called by nothing in `pressa-tui`,
`Route` has two variants where [`tui.md`](../docs/tui.md) §1 names four,
`Context` has three where it names six, `Effect` has one of its four, and
`Enter` and `n` are bound to nothing at all — T8 left them unbound on purpose,
because binding a key to a screen that does not exist means inventing a
placeholder for T9 to delete ([SPEC-007](007-list-view.md) Q2).

The visible consequence is that T8's empty state reads `No records yet.` and
stops there: the sentence that would name the key is withheld, because a screen
may not name a key that does nothing ([SPEC-006](006-tui-shell.md) Q4). The
only way a record enters the database today is `pressa-tui/examples/seed.rs`.

T9 is the task the whole milestone was ordered around
([ADR-0007](../docs/adr/0007-vertical-slice-first.md)): **it closes the Golden
Path**. After it, a `pressa.yaml`, a typed record, a quit and a restart leave
the record still there — which is the one claim [SPEC-000](000-m0-golden-path.md)
exists to check.

## Goal

One form renderer that edits any collection's record, generated from
`Collection.fields` and from `FieldType` and from nothing else
([ADR-0004](../docs/adr/0004-schema-driven-ui.md)). `n` on a list opens a blank
draft, `Enter` opens the selected record's, and both reach the same renderer —
a label per field, an editor widget per field type, `*` on the required ones, a
dirty indicator in the header, and a form that scrolls to follow the focus when
it is taller than the panel. `Ctrl+S` sends the draft to `RecordService`, which
validates it; a refusal paints each message under the field it names and leaves
the draft alone, and a success routes back to the list and reloads it. `Esc`
leaves, and asks first when there is something to lose.

## Non-goals

T9 fills one route pair. A reader may reasonably expect the following here and
will not get it:

| Not here | Where it lives instead |
|---|---|
| Deleting (`d`) and `Overlay::ConfirmDelete`. T9 introduces `Overlay` and one confirm context for `ConfirmDiscard` alone, so T10 adds a variant rather than a mechanism. T10 owns delete and nothing else: the roadmap's T10 row, which still claims `n`, is corrected (Q13) | 009, T10 |
| Search (`/`), the help overlay (`?`) and the `? Help` entry in any hint bar. Every frame below is as of T9, which is why none of them shows it, while [000](000-m0-golden-path.md)'s frames — which are as of T12 — do | 010, T11 |
| Cursor movement inside a field: `←` / `→`, `Home` / `End`, `Delete`, word motions, selection, clipboard. `EditorInput` binds printable characters, `Backspace`, `Enter` and `Esc` and nothing else, so text is appended and removed at the end only and the cursor is always last | not in M0 |
| Validation as you type, and a per-keystroke save attempt. The schema's rules live where they already live — `validate_record` in `pressa-core`, run by `RecordService` on save ([002](002-record-validation.md), [004](004-app-services.md)); the editor adds exactly one rule of its own, the parse on commit that [002](002-record-validation.md) already assigns to it (Q6) | — |
| A `Saving` body or a spinner. Effects are synchronous ([ADR-0002](../docs/adr/0002-synchronous-rusqlite.md)): the queue drains before the next `draw`, so a save in flight is unobservable, exactly as `Load::Loading` was ([SPEC-007](007-list-view.md) "Non-goals"). The draft spec's `Clean → Dirty → Saving → Saved` is two states (Q5) | nowhere; ADR-0002 makes it unreachable |
| Autosave, save-and-stay, "save and new", duplicate, undo / redo | M1, [roadmap](../docs/roadmap.md) §3 |
| `Relation` fields, rich text, media, uploads, drafts, versioning, localization. Seven field types and no eighth ([`AGENTS.md`](../AGENTS.md) "Scope") | M2, M4, [roadmap](../docs/roadmap.md) §3 |
| `capabilities.create` and `.update`. Every M0 capability is `true` and nothing in [001](001-schema-config.md) can produce a `false` one, so the branches would be unreachable through the loader and untestable without hand-building a `Collection` no YAML can express. T10 is the first task to read a capability; a note in [`domain-model.md`](../docs/domain-model.md) §3 records where the checks go when a schema can express them (Q12) | M3, when the schema can say `create: false` |
| A field-level `unique` check before save. `NotUnique` needs storage and arrives in the same `Vec<FieldError>` as the rest ([004](004-app-services.md)); the form renders it under its field like any other message | — |
| Re-reading the record after a successful save. `RecordSaved` carries the stored `Record`, and the route leaves the editor anyway | — |
| A reload on every exit from the editor. Only a save, and a `LoadRecord` that found nothing, leave the list provably stale (Q14) | — |
| A `Value`-typed draft. The draft is a `serde_json::Value` object from end to end, as `blank` and `Record.data` both are; `Value::from_json` *rejects* data that does not fit its field, and the editor must open and fix such a record rather than refuse to show it ([domain-model](../docs/domain-model.md) §6) | — |
| Any new entry in `pressa-tui/Cargo.toml`, in any section. `FieldError`, `ErrorCode` and `RecordId` reach the UI through `pressa_app::domain`, the way [SPEC-006](006-tui-shell.md) Q2 settled and `domain.rs` already promises | — |
| A pty or terminal-emulator test. `update` and a `TestBackend`, as every TUI task before this one | [006](006-tui-shell.md)'s panic-restore test |

## User stories

- As a user, I want `n` on a list to open an empty form for that collection, so
  that I can put the first record in without a seed script.
- As a user, I want `Enter` on a row to open that record with its stored values
  and no unsaved marker, so that I can tell editing from creating.
- As a user, I want each field drawn as the kind of thing it is — a box for
  text, a toggle for a boolean, a cycler for a select — so that I do not have
  to type JSON.
- As a user, I want to see which field the keyboard is on, and whether the next
  key will type into it or move off it, so that I am never typing blind.
- As a user, I want the message about a rejected field printed under that
  field, so that I can fix it without matching errors to inputs by hand.
- As a user, I want `Ctrl+S` on a form with three empty required fields to
  refuse and show me three reasons at once, so that fixing the form is not a
  sequence of save attempts.
- As a user with a form taller than my terminal, I want it to scroll as I move
  and to tell me how many fields are off screen, so that nothing is hidden
  silently.
- As a user who typed something and pressed `Esc`, I want to be asked before it
  is thrown away, so that one keystroke cannot lose my work.
- As a user, I want the record I saved to still be there after a restart, so
  that I can trust the product with real content.
- As an implementer of T10 and T11, I want `Overlay`, a confirm context and
  `Confirm` / `Dismiss` already in place, so that the delete dialog and the
  help popup add a variant rather than a mechanism.

## UX

Every frame is exactly 80x24 unless it says otherwise — the size the `insta`
snapshots are taken at. They were generated from the rules below rather than
drawn by hand, so a disagreement between a frame and a rule is a bug in this
spec. Nothing in them is provisional: the fifteen questions were answered on
2026-10-03.

Frames A, B, D, E and I are [000](000-m0-golden-path.md)'s frames C, D, E, G
and I regenerated. Three things moved, and two are corrections rather than
decisions: the focused box's right border sat one column left of its own top
and bottom rule, and the hint bar was joined with two spaces where
[SPEC-006](006-tui-shell.md) Q5 fixed the separator at three. The third is the
focus marker and the cursor rule (Q3a, Q15). [000](000-m0-golden-path.md)
already records that its editor frames belong to T9
([SPEC-007](007-list-view.md) Q4 regenerated the table ones and left these), so
T9 carries all three back into that spec — see "Documents".

### The form, stated once

The main panel is 57x17 at 80x24 and 37x9 at the 60x16 minimum
([SPEC-006](006-tui-shell.md) "UX"). Inside it, one **block** per entry of
`collection.fields`, in schema order, each followed by one blank row.

- **Label row.** Two columns of indent, the field's `label`, then ` *` when
  `required`. On the focused field the second indent column is `▸` — the same
  marker the table puts on its selected row — so focus reads the same way in
  both screens (Q3a).
- **Inline value.** `Select` and `Boolean` draw their widget on the label row,
  starting at the **value column**: the collection's longest field `label` plus
  two, never less than 18. Derived from the schema alone, so inline widgets
  line up with each other and a long label pushes the column out rather than
  being clipped (Q9). `posts` comes out at 18. `Boolean` is `[x]` / `[ ]`;
  `Select` is `‹ value ›`, and `‹ — ›` when the field has no value. On the
  focused field the widget is reverse video, which a text snapshot cannot show
  and which is asserted on cell styles instead.
- **Stacked value.** The other five types draw their value on the next row,
  indented four.
- **Values.** A field's value is the text `BeginEdit` would seed its box with:
  a `DateTime` as the stored RFC 3339 string, a `Json` field as its JSON text,
  a `Number` as the JSON number's own text. What is on screen is what you would
  be editing, which is why the form does not reuse `table::cell` (Q7). Two of
  that renderer's rules do carry over, because they are about absence rather
  than format: an absent or null value is a dim `—`, and a value whose JSON
  type its field does not allow is its own JSON text, dim — the editor opens
  such a record rather than refusing to.
- **Focus.** The focused field's value row is replaced by a box: a top rule on
  the row after the label, the value, and a bottom rule. The box spans panel
  columns 2 to 54 at 80x24 — two columns of indent, two of right gutter —
  leaving 50 columns of text. A focused `Textarea` and `Json` box is five rows
  tall; the other five are one (Q3b).
- **Cursor.** `▌` after the last character, drawn **only** while an input
  context is active. A focused field in `Context::Editor` shows its `▸`, its
  box and its value and no cursor, so the cursor means exactly one thing: the
  next printable key lands here (Q15).
- **Overflow.** Text wider than a one-row box shows its tail, with `…` in the
  box's first text column to say the head is cut — nothing is clipped silently
  ([`tui.md`](../docs/tui.md) §6). A five-row box wraps instead (Q3c).
- **Error row.** A field named by a `FieldError` gains one row directly under
  its value row or its box bottom: two spaces, `⚠ `, then the error's
  `message`, in red. One row per field; a field with two errors shows the first
  in the service's order.
- **Block heights**, in rows, before the blank row that follows: `Select` and
  `Boolean` 1; the other five unfocused 2; focused `Text`, `Number` and
  `DateTime` 4; focused `Textarea` and `Json` 8; plus 1 when the field has an
  error row.
- **Scrolling.** The form scrolls by whole **fields**, so the panel's top row
  is always a label row. The first visible field is the smallest index at which
  the focused field's block **and its trailing blank row** both fit in the
  panel — scroll the least that keeps the focused field whole (Q4). Blocks are
  1 to 9 rows tall, so `view::window`, which assumes rows of one height, cannot
  be reused. Including the blank row is what keeps a box's bottom rule off the
  last body row, where a scroll summary would otherwise overwrite it.
- **Scroll summaries.** The first body row carries a dim `↑ n more` and the
  last a dim `↓ n more`, right-aligned with a two-column gutter, overlaid on
  that row rather than replacing it (frames G, H). `n` counts the fields **no
  row of which is drawn**: a block may be cut off at the bottom — frame B's
  `Views` shows its label and not its value — which is why the summary counts
  fields and not rows.
- **Header.** Three crumbs, the third naming the record: the draft's first
  `list_columns` value through `table::cell`, truncated to 20 characters with
  `…`. When that cell is the absent `—` the crumb is the record id shortened to
  six characters and `…` at `Edit`, and the literal `New` at `New` (Q8). It
  reads the draft, so it follows a rename as each field is committed. A dirty
  draft puts ` ● unsaved · Ctrl+S ` in the right-aligned title segment
  `chrome::shell` already takes for the list's record count; a clean one puts
  nothing there, which is how frame E differs from frame D.
- **Hint bars**, one per context, every word from a binding:

  | Context | Bar |
  |---|---|
  | `Editor` | `Tab Next   Enter Edit   Ctrl+S Save   Esc Back` |
  | `EditorInput` | `Enter Done   Esc Cancel` |
  | `EditorText` | `Tab Done   Enter Newline   Esc Cancel` |
  | `ConfirmDiscard` | `y Discard   n Cancel` |

- **No branch on a slug.** Nothing in the form reads a collection's or a
  field's *name* to decide how to draw it; `label`, `kind`, `required`, the
  collection's longest label and the draft's value are the whole input
  ([ADR-0004](../docs/adr/0004-schema-driven-ui.md)).

### A — A new record, blank

`examples/blog`, `n` on `Posts`. The draft is `blank`: every field key present,
`featured` already `false`, the required `Select` empty on purpose so that a
save can report it ([004](004-app-services.md), [000](000-m0-golden-path.md) Q2).

```text
┌ pressa › Posts › New ────────────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │ ▸Title *                                                │
│                    │  ┌───────────────────────────────────────────────────┐  │
│ > Posts            │  │ —                                                 │  │
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
│ Tab Next   Enter Edit   Ctrl+S Save   Esc Back                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

`Title` is focused — `▸` on its label row, a box round its value, and no
cursor, because nothing is being typed yet. The box holds a dim `—` for the
same reason the stacked fields do: the draft has no value there. `Status` and
`Featured` are inline at the value column; the form is 24 rows and the body is
17, so `Published at` and `Metadata` are off the bottom and the last row says
so.

### B — A save the service refused

`Ctrl+S` on frame A. Three required fields, three messages, each under its own
field; the route does not change and the draft is untouched.

```text
┌ pressa › Posts › New ────────────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │ ▸Title *                                                │
│                    │  ┌───────────────────────────────────────────────────┐  │
│ > Posts            │  │ —                                                 │  │
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
│ Tab Next   Enter Edit   Ctrl+S Save   Esc Back                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

`Views` is cut off after its label, and `↓ 3 more` counts the three fields no
row of which is drawn. `3 fields need attention` is derived by `view` from
`editor.errors`, not a `StatusMessage` (Q11).

### C — Typing into a field

`Enter` on frame A, then five characters. `Context::EditorInput`: the cursor
appears, and the hint bar is the two keys that leave it.

```text
┌ pressa › Posts › New ────────────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │ ▸Title *                                                │
│                    │  ┌───────────────────────────────────────────────────┐  │
│ > Posts            │  │ Hello▌                                            │  │
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
│ Enter Done   Esc Cancel                                                      │
└──────────────────────────────────────────────────────────────────────────────┘
```

Nothing has reached the draft yet — `editor.input` holds the text and
`CommitField` writes it — so the header carries no unsaved marker. That is the
visible consequence of committing on `Enter` rather than per keystroke, and it
is the one place a user can have typed something the dirty indicator does not
yet know about.

### D — Filled, dirty, the focus moved on

[000](000-m0-golden-path.md) step 7: `Title`, `Slug`, `Status` and `Views`
filled in, the focus on `Views`.

```text
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
│                    │  ┌───────────────────────────────────────────────────┐  │
│                    │  │ 42                                                │  │
│                    │  └───────────────────────────────────────────────────┘  │
│                    │                                                         │
│                    │  Featured        [ ]                          ↓ 2 more  │
├────────────────────┴─────────────────────────────────────────────────────────┤
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ Tab Next   Enter Edit   Ctrl+S Save   Esc Back                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

The header carries the unsaved marker and the renamed crumb. No scrolling: with
`Title` unfocused its block is 3 rows rather than 5, so `Title` through `Views`
is 16 of the 17 available and the least scroll is none. [000](000-m0-golden-path.md)
drew this frame starting at `Status`, which no stated rule produces; it is
regenerated.

### E — Editing a stored record

`Enter` on the saved row. The draft is the record's `data`, so `Views` reads
`42` and `Status` reads `‹ published ›`.

```text
┌ pressa › Posts › Hello world ────────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │ ▸Title *                                                │
│                    │  ┌───────────────────────────────────────────────────┐  │
│ > Posts            │  │ Hello world                                       │  │
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
│ Tab Next   Enter Edit   Ctrl+S Save   Esc Back                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

No unsaved marker: a draft that arrived from storage starts clean.

### F — A save that failed for a reason that is not validation

`Ctrl+S` and the repository refused. The `AppError`'s own message goes to the
status line, in one place and in one format, as the list's failed load already
does ([SPEC-007](007-list-view.md) frame G); the draft is untouched, the route
unchanged, and the form is still dirty because nothing was stored.

```text
┌ pressa › Posts › Hello world ───────────────────────── ● unsaved · Ctrl+S ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │ ▸Title *                                                │
│                    │  ┌───────────────────────────────────────────────────┐  │
│ > Posts            │  │ Hello world                                       │  │
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
│ ⚠ database error: disk I/O error                                             │
├──────────────────────────────────────────────────────────────────────────────┤
│ Tab Next   Enter Edit   Ctrl+S Save   Esc Back                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

### G — A focused `Textarea`, being typed into

`Tab` to `Content`, `Enter`, a line, `Enter` again, a second line.
`Context::EditorText`: `Enter` inserts a newline and `Tab` is what finishes the
field (Q3b).

```text
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
│                    │ ▸Content                                                │
│                    │  ┌───────────────────────────────────────────────────┐  │
│                    │  │ First line                                        │  │
│                    │  │ second line▌                                      │  │
│                    │  │                                                   │  │
│                    │  │                                                   │  │
│                    │  │                                                   │  │
│                    │  └───────────────────────────────────────────────────┘  │
│                    │                                               ↓ 4 more  │
├────────────────────┴─────────────────────────────────────────────────────────┤
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ Tab Done   Enter Newline   Esc Cancel                                        │
└──────────────────────────────────────────────────────────────────────────────┘
```

Five rows, so a stored multi-line value and a long wrapped paragraph are both
readable. A focused `Json` field is the same box with the same hint bar. The
summary sits on `Content`'s trailing blank row, which is what the scroll rule
guarantees is there.

### H — All seven field types, in a collection that is not `posts`

A fixture whose fields are one of each type, in a different collection under a
different slug, rendered by the same function — the proof
[ADR-0004](../docs/adr/0004-schema-driven-ui.md) asks for.

```text
┌ pressa › Entries › Release notes ────────────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │ ▸Title *                                                │
│                    │  ┌───────────────────────────────────────────────────┐  │
│ > Entries          │  │ Release notes                                     │  │
│                    │  └───────────────────────────────────────────────────┘  │
│                    │                                                         │
│                    │  Status *        ‹ published ›                          │
│                    │                                                         │
│                    │  Views                                                  │
│                    │    1024                                                 │
│                    │                                                         │
│                    │  Featured        [x]                                    │
│                    │                                                         │
│                    │  Published at                                           │
│                    │    2026-09-06T12:00:00Z                                 │
│                    │                                                         │
│                    │  Metadata                                               │
│                    │    {"tags":["rust"],"pinned":true}            ↓ 1 more  │
├────────────────────┴─────────────────────────────────────────────────────────┤
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ Tab Next   Enter Edit   Ctrl+S Save   Esc Back                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

`Published at` reads as the stored RFC 3339 string and `Metadata` as its JSON
text, because the form shows what you would edit rather than what the table
shows (Q7). The value column is 18 here too: `Published at` is twelve
characters, and twelve plus two is under the floor.

### I — `ConfirmDiscard`

`Esc` with a dirty draft. The dialog is centred over the main panel, which is
still drawn behind it; the route is still the editor, and no effect was
returned.

```text
┌ pressa › Posts › Hello, world! ─────────────────────── ● unsaved · Ctrl+S ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │ ▸Title *                                                │
│                    │  ┌───────────────────────────────────────────────────┐  │
│ > Posts            │  │ Hello, world!                                     │  │
│                    │  └───────────────────────────────────────────────────┘  │
│                    │                                                         │
│                    │       ┌─ Discard changes ──────────────────────┐        │
│                    │       │                                        │        │
│                    │       │  Discard unsaved changes?              │        │
│                    │       │  This cannot be undone.                │        │
│                    │       │                                        │        │
│                    │       │         y Discard    n Cancel          │        │
│                    │       └────────────────────────────────────────┘        │
│                    │                                                         │
│                    │  Views                                                  │
│                    │    42                                                   │
│                    │                                                         │
│                    │  Featured        [ ]                          ↓ 2 more  │
├────────────────────┴─────────────────────────────────────────────────────────┤
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ y Discard   n Cancel                                                         │
└──────────────────────────────────────────────────────────────────────────────┘
```

`y` discards and leaves, `n` closes the dialog and changes nothing else. The
wording names the changes rather than the record, so it cannot be mistaken for
the delete prompt it is shaped like.

### J — The editor at the 60x16 minimum

```text
┌ pressa › Posts › New ────────────────────────────────────┐
├────────────────────┬─────────────────────────────────────┤
│ Collections        │ ▸Title *                            │
│                    │  ┌───────────────────────────────┐  │
│ > Posts            │  │ —                             │  │
│                    │  └───────────────────────────────┘  │
│                    │                                     │
│                    │  Slug *                             │
│                    │    —                                │
│                    │                                     │
│                    │  Status *        ‹ — ›    ↓ 5 more  │
├────────────────────┴─────────────────────────────────────┤
│                                                          │
├──────────────────────────────────────────────────────────┤
│ Tab Next   Enter Edit   Ctrl+S Save   Esc Back           │
└──────────────────────────────────────────────────────────┘
```

Nine body rows: `Title` boxed, `Slug`, `Status`, and five fields off the
bottom. The box is 30 columns of text instead of 50, and the value column is
still 18. Below 60x16 the shell draws "terminal too small" and no form at all
([SPEC-006](006-tui-shell.md) frame G).

### K — The list, once `n` and `Enter` are bound

T8's frame A, retaken. The empty state gains the sentence T8 withheld and the
hint bar gains two entries, because both keys now do something
([SPEC-007](007-list-view.md) Q2, [SPEC-006](006-tui-shell.md) Q4).

```text
┌ pressa › Posts ──────────────────────────────────────────────── 0 records ───┐
├────────────────────┬─────────────────────────────────────────────────────────┤
│ Collections        │  TITLE              STATUS             VIEWS            │
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
│ ↑↓ Navigate   Enter Edit   n New   Esc Back                                  │
└──────────────────────────────────────────────────────────────────────────────┘
```

Frames B–I of [007](007-list-view.md) change in the hint bar only, and are
retaken with it.

## Domain model

`AppState` gains the two fields [`tui.md`](../docs/tui.md) §1 reserves, and
`Route` its other two variants:

```rust
pub struct AppState {
    pub schema: Schema,
    pub route: Route,
    pub sidebar: SidebarState,
    pub list: ListState,
    pub editor: EditorState,      // new in T9
    pub overlay: Option<Overlay>, // new in T9
    pub status: Option<StatusMessage>,
    pub should_quit: bool,
}

pub enum Route {
    Home,
    List { collection: String },
    New  { collection: String },               // new
    Edit { collection: String, id: RecordId }, // new
}

/// The draft, where the focus is, and what the last save refused.
///
/// One struct for both routes: they differ only in where `draft` came from —
/// `blank_document` or a loaded record — and in whether `SaveRecord` carries
/// an id.
pub struct EditorState {
    /// The document being edited. Always a JSON object with every field key
    /// present, as `blank_document` and `Record.data` both are.
    pub draft: Json,
    /// What `draft` was when it arrived. Dirty is `draft != original`,
    /// computed where it is needed and stored nowhere, so it cannot go stale
    /// and a value edited back to itself reports clean (Q5).
    pub original: Json,
    /// Index into `collection.fields`.
    pub focus: usize,
    /// The first field the panel draws; `view` clamps it rather than mutating
    /// it, as `list.offset` and `sidebar.offset` already are.
    pub offset: usize,
    /// The text typed so far for the focused field, and the whole of the
    /// difference between the navigating and the typing contexts: `Some` is an
    /// input context, `None` is `Context::Editor`. Derived rather than stored
    /// beside a mode, so "typing into no field" is unrepresentable
    /// (SPEC-006 "The keymap": `Context` is derived, never stored).
    pub input: Option<String>,
    /// What the last save refused, plus any parse error the editor itself
    /// produced, one per field.
    pub errors: Vec<FieldError>,
    /// The third breadcrumb when the draft cannot name itself: the route's id,
    /// shortened. Carried so `breadcrumbs` needs no `Route` lookup (Q8).
    pub id: Option<RecordId>,
}

/// Drawn on top of the current route, which is why it is not a `Route`
/// (`docs/tui.md` §1). One variant in T9; T10 and T11 add theirs.
pub enum Overlay {
    ConfirmDiscard { next: Box<Route> },
}
```

`Command` gains the editor's actions, the overlay's two answers, and the three
results a new effect can produce:

```rust
pub enum Command {
    // … T7 and T8's variants, unchanged
    NewRecord, EditRecord,                        // reached from a list
    NextField, PrevField,
    BeginEdit, CommitField, CancelEdit, InsertNewline,
    ToggleBoolean, NextOption, PrevOption,
    InputChar(char), InputBackspace,
    Save,
    Confirm, Dismiss,                             // the overlay
    RecordLoaded(Box<Record>),
    RecordSaved(Box<Record>),
    SaveFailed(Vec<FieldError>),
}
```

`Box<Record>` for the two that carry one, because `Command` is matched by value
and a bare `Record` would make every variant as large as a document.

`BeginEdit` is what `Enter` resolves to in `Context::Editor`, and its `update`
arm dispatches on the focused field's `FieldType`: an input context for the
five text-shaped types, `ToggleBoolean` for a `Boolean`, `NextOption` for a
`Select`. The keymap stays a table that returns one command for one key; the
type dispatch is in `update`, which is where
[ADR-0004](../docs/adr/0004-schema-driven-ui.md) wants per-type complexity.
`ToggleBoolean`, `NextOption` and `PrevOption` also get keys of their own, so
none of them is reachable only from inside another command's arm (Q1).

`Effect` gains the two variants [`tui.md`](../docs/tui.md) §1 names:

```rust
pub enum Effect {
    LoadRecords { collection: String },                                    // T8
    LoadRecord  { collection: String, id: RecordId },                      // new
    SaveRecord  { collection: String, id: Option<RecordId>, data: Json },  // new
}
```

`id: Option<RecordId>` is the only thing that distinguishes creating from
updating by the time the effect reaches `run_effects`: `None` is
`RecordService::create`, `Some` is `update`. `Effect::DeleteRecord` stays
unwritten until T10 has a runner for it, the way `LoadRecords` waited for T8.
There is no effect for the blank draft: `blank_document` is pure, so `update`
calls it (Q2).

`Context` gains its four remaining M0 variants, and `keymap::context_for`
therefore stops being a function of the route alone — an overlay and a
typed-into field are state, not route:

```rust
pub enum Context {
    Global, Sidebar, List,     // T7, T8
    Editor,                    // New / Edit, moving between fields
    EditorInput,               // typing into one of the five single-line types
    EditorText,                // typing into a Textarea or a Json field
    ConfirmDiscard,            // the overlay; T10 adds ConfirmDelete
}
```

Two input contexts rather than one so that `Enter` can commit in a single-line
field and insert a newline in a multi-line one while resolution stays a table
lookup, and so that each gets its own correct hint bar (Q3b). One context per
overlay for the same reason: `KeyBinding.description` is a fixed `&'static str`,
and `y Discard` and T10's `y Delete` cannot both come from one row (Q10).
Precedence in `context_for`: overlay, then `editor.input`, then the route.

`pressa-core` gains one pure function, and `pressa_app::domain` the two types
the form renders — finishing the list `domain.rs` already says T9 extends:

```rust
// pressa-core/src/schema.rs — the field defaults for a new record, from the
// schema alone. No I/O, no repository, so `update` may call it.
pub fn blank_document(collection: &Collection) -> Json;

// pressa-app/src/domain.rs
pub use pressa_core::schema::blank_document;
pub use pressa_core::validation::{ErrorCode, FieldError};
```

`RecordService::blank` keeps its signature and delegates to it, so the service
layer's API is unchanged and the defaults are stated once (Q2). This is the one
answer that touches a crate T9's roadmap row does not name; it is a pure
function over a type `pressa-core` already owns, it adds no dependency to any
crate, and it is recorded here rather than discovered during implementation.

## API

```rust
// tui/view/form.rs — new. The one form, as `view/table.rs` is the one table.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, collection: &Collection);

/// How one field's value reads, and whether it is dim: the editable text, with
/// a dim `—` for absent and dim JSON for a value the field's type forbids.
/// The form's answer to ADR-0004's third question, as `table::cell` is the
/// table's — and deliberately not that function (Q7).
pub fn display(kind: &FieldType, value: Option<&Json>) -> Cell;

/// The text `BeginEdit` seeds the input buffer with, and the inverse
/// `CommitField` writes back into the draft. The pair that has to round-trip.
pub fn editable(kind: &FieldType, value: Option<&Json>) -> String;
pub fn parse(kind: &FieldType, text: &str) -> Result<Json, FieldError>;

/// The value column: the collection's longest label plus two, floor 18 (Q9).
pub fn value_column(collection: &Collection) -> usize;

/// How many rows one field's block occupies, so the scroll rule and the
/// renderer cannot disagree about where a block ends.
pub fn block_height(field: &Field, focused: bool, has_error: bool) -> usize;

/// The first field the panel draws: the least scroll that keeps the focused
/// block and its trailing blank row whole. The form's own window —
/// `view::window` assumes rows of one height (Q4).
pub fn first_visible(focus: usize, heights: &[usize], rows: usize) -> usize;

// tui/view/overlay.rs — new
pub fn render(frame: &mut Frame, area: Rect, overlay: &Overlay);

// tui/view/mod.rs
/// Gains the two editor arms, whose third crumb names the record, so it needs
/// the editor as well as the route (Q8).
pub fn breadcrumbs(state: &AppState) -> Vec<String>;

// tui/keymap.rs — four contexts, their rows, and a signature change
pub fn context_for(state: &AppState) -> Context;

// tui/effects.rs — two more arms, and the one place `AppError::Validation` is
// taken apart: its field errors become `SaveFailed`, and every other
// `AppError` becomes `OperationFailed` with its own message.
pub fn run_effects<R: RecordRepository>(
    records: &RecordService<R>,
    effects: Vec<Effect>,
) -> Vec<Command>;
```

### The keymap's new rows

| Context | Key | Command | Description | Hint |
|---|---|---|---|---|
| List | `Enter` | EditRecord | Edit | Shown |
| List | `n` | NewRecord | New | Shown |
| Editor | `Tab` | NextField | Next | Shown |
| Editor | `j` | NextField | Next | Hidden |
| Editor | `Shift+Tab` | PrevField | Previous | Hidden |
| Editor | `k` | PrevField | Previous | Hidden |
| Editor | `Enter` | BeginEdit | Edit | Shown |
| Editor | `Space` | ToggleBoolean | Toggle | Hidden |
| Editor | `l` / `→` | NextOption | Next option | Hidden |
| Editor | `h` / `←` | PrevOption | Previous option | Hidden |
| Editor | `Ctrl+S` | Save | Save | Shown |
| Editor | `Esc` | Back | Back | Shown |
| EditorInput | printable | InputChar | — | Hidden |
| EditorInput | `Backspace` | InputBackspace | — | Hidden |
| EditorInput | `Enter` | CommitField | Done | Shown |
| EditorInput | `Esc` | CancelEdit | Cancel | Shown |
| EditorText | printable | InputChar | — | Hidden |
| EditorText | `Backspace` | InputBackspace | — | Hidden |
| EditorText | `Tab` | CommitField | Done | Shown |
| EditorText | `Enter` | InsertNewline | Newline | Shown |
| EditorText | `Esc` | CancelEdit | Cancel | Shown |
| ConfirmDiscard | `y` | Confirm | Discard | Shown |
| ConfirmDiscard | `Enter` | Confirm | Discard | Hidden |
| ConfirmDiscard | `n` | Dismiss | Cancel | Shown |
| ConfirmDiscard | `Esc` | Dismiss | Cancel | Hidden |
| ConfirmDiscard | `q` | Dismiss | Cancel | Hidden |

`Space`, `l`/`→` and `h`/`←` are `Hidden` so the `Editor` bar stays the four
entries §UX states; they are reachable keys for commands `Enter` also reaches
through `BeginEdit`'s dispatch, which is what keeps every command in the enum
bound to something (Q1). A printable character cannot carry a `description`
worth printing, so both input contexts leave `InputChar` `Hidden` — the bar
names the keys that *leave* the field.

No change to `drive`, `run` or `cli::dev`: T8 built the loop as a queue
precisely because T9's save answers with a command whose `update` returns
another effect (`tui/mod.rs::drain`), and T9 is the task that first exercises
it — `Save` → `SaveRecord` → `RecordSaved` → `LoadRecords` → `RecordsLoaded`
settles in three rounds without the loop changing.

## Invariants

- `update` performs no I/O and touches nothing but the `AppState` it is given;
  `run_effects` is still the only function in the crate that calls a service.
  `blank_document` is pure, which is why calling it from `update` is not an
  exception to this (Q2).
- `view` performs no I/O, reads no clock and never mutates state.
- `editor.draft` is a JSON object with exactly the keys `collection.fields`
  names, from the moment the editor opens until it closes. Nothing in the form
  adds or removes a key, so `UnknownField` is unreachable from the UI.
- `editor.draft` never holds a value whose JSON type its field forbids
  *because of something the user typed*: `CommitField` parses first and refuses
  (Q6). A draft loaded from storage may hold one, and the form shows it.
- `editor.focus < collection.fields.len()`, which is at least 1 ([001](001-schema-config.md)).
- `editor.input` is `Some` exactly while an input context is active — one fact,
  stored once, and the reason `Context` can be derived.
- The draft changes on `CommitField`, `ToggleBoolean`, `NextOption` and
  `PrevOption`, and on nothing else. `InputChar`, `InputBackspace` and
  `InsertNewline` touch `editor.input`; `CancelEdit` drops it. A cancelled edit
  therefore cannot have changed the draft, which is what "restores the original
  value" ([`tui.md`](../docs/tui.md) §3) amounts to — and the reason frame C is
  clean while text is on screen.
- The cursor `▌` is drawn if and only if `editor.input` is `Some`.
- An invalid record is never written: `RecordService` validates before it writes
  ([004](004-app-services.md)), and the editor sends one `SaveRecord` per
  `Save` — never a retry for the same keystroke.
- A failed save leaves `editor.draft`, `editor.focus`, `editor.offset` and
  `state.route` exactly as they were. Save is all or nothing.
- Leaving a dirty editor passes through `Overlay::ConfirmDiscard`: no command
  sets `route` away from `New` or `Edit` while `draft != original`, except
  `Confirm` and `RecordSaved`.
- `errors` holds at most one entry per field. `SaveFailed` replaces the whole
  vector — one save reports one set of problems, not a growing list, the rule
  `OperationFailed` already follows for the status line — and a parse error
  replaces the entry for its own field only.
- No key is compared to a `KeyCode` outside `keymap.rs`; every hint string
  comes from `KEYMAP`.
- No screen, widget or code path branches on a collection slug or a field name
  ([ADR-0004](../docs/adr/0004-schema-driven-ui.md)).

## Error cases

| Input | Result |
|---|---|
| `Ctrl+S` and the service returns `AppError::Validation` | `SaveFailed`: each message under the field it names, route unchanged, draft unchanged, `view` derives `<n> field(s) need attention` (frame B) |
| `Ctrl+S` and the repository fails | `OperationFailed`: `⚠ <the AppError's message>` in the status line, draft and route unchanged, the form still dirty (frame F) |
| `Ctrl+S` on a clean draft loaded from storage | one `SaveRecord` all the same; an idempotent update is cheaper than a special case, and `updated_at` moving is correct |
| `Esc` with `draft == original` | `Route::List` for the same collection, no overlay, no effect, no reload (Q14) |
| `Esc` with `draft != original` | `Overlay::ConfirmDiscard { next: List }` and no effect; `Confirm` leaves and clears the editor, `Dismiss` closes the overlay and changes nothing else (frame I) |
| `Esc` in an input context | `editor.input` is dropped; the draft was never written, so the field reads as it did |
| `CommitField` on text that will not parse for its type | the field stays in its input context with the text intact, and a `FieldError` for that field is put in `editor.errors` and drawn under it: `InvalidJson` / `not valid JSON`, `InvalidDateTime` / `must be a date like 2026-09-06T12:00:00Z`, `TypeMismatch` / `must be a number` ([002](002-record-validation.md)'s messages, unchanged). `Esc` is always available to abandon the field (Q6) |
| A successful `CommitField` on a field that had a parse error | that field's error is removed; the others are left alone |
| A draft value whose JSON type its field does not allow — a record the schema has since outgrown | drawn as its own JSON text, dim; the field opens and can be retyped (Q7) |
| `Enter` on a list with no records, or whose load failed | nothing: no route change, no effect, no status message |
| `n` on a collection whose load failed | the editor opens: creating a record does not depend on having read any |
| `EditRecord` on a record another process deleted | `LoadRecord` → `AppError::NotFound` → `OperationFailed`, and the route returns to `List` with one `LoadRecords`: the list is provably stale, since it is showing a row that is gone (Q14) |
| `NextField` on the last field, `PrevField` on the first | clamped, never wrapping, as the list and the sidebar are |
| `NextOption` on a `Select` with the last option selected, or with no value | wraps to the first; `PrevOption` from no value selects the last. [000](000-m0-golden-path.md) step 7 presses `NextOption` twice from empty and expects `draft` then `published` |
| `BeginEdit`, `ToggleBoolean`, `NextOption` or `PrevOption` on a field of the wrong type | nothing: a no-op rather than a panic. `BeginEdit` dispatches by type and has an arm for all seven; the other three check and return |
| A value wider than its box | the tail, with `…` in the first text column (Q3c) |
| A `Save` or a `Tab` while the overlay is open | nothing: `Context::ConfirmDiscard` binds `y`, `Enter`, `n`, `Esc` and `q`, and no other key resolves except the global `Ctrl+C` |
| A terminal under 60x16 | "terminal too small"; no form, no overlay ([SPEC-006](006-tui-shell.md) frame G) |
| A `Route::New` or `Edit` naming a collection the schema does not know | the Home body, as `Route::List` already does: a stale route is visible rather than fatal |

## Acceptance criteria

Routes and reaching the form

- [ ] `Enter` at `Route::List` with a record selected sets
      `Route::Edit { collection, id }` to that record's id and returns exactly
      one `Effect::LoadRecord` for it.
- [ ] `Enter` at `Route::List` with no records, and with `Load::Failed`,
      changes no state and returns no effect.
- [ ] `n` at `Route::List` sets `Route::New { collection }` and returns no
      effect: the draft comes from `blank_document`, which `update` calls.
- [ ] `blank_document` returns an object with every field key of the
      collection, `false` for a `Boolean` and `null` for the other six — and
      `RecordService::blank` returns exactly what it returns, for every
      collection in `examples/blog` and in the seven-type fixture.
- [ ] `RecordLoaded` fills `draft` and `original` with the record's `data`,
      puts `focus` and `offset` at 0, and leaves `errors` empty.
- [ ] A draft from `RecordLoaded` and one from `blank_document` are both clean:
      no unsaved marker in either (frames A and E).
- [ ] `breadcrumbs` third crumb is the draft's first `list_columns` value
      through `table::cell`, truncated to 20 characters with `…`; it is the
      shortened id at `Edit` and the literal `New` at `New` when that cell is
      the absent `—`; and it follows a committed rename (frames A, D, E, I).
- [ ] `breadcrumbs` prints the slug for a collection the schema does not know
      rather than panicking, at `New` and `Edit` as it already does at `List`.
- [ ] `context_for` returns `Editor` at `New` and `Edit`, `EditorInput` when
      `editor.input` is `Some` on one of the five single-line types,
      `EditorText` when it is `Some` on a `Textarea` or `Json`, and
      `ConfirmDiscard` whenever `state.overlay` is `Some` — overlay first,
      input second, route last.

Form layout

- [ ] The 80x24 snapshots match frames A, B, C, D, E, F, G, H and I, and the
      60x16 snapshot matches frame J.
- [ ] `block_height` returns 1 for `Select` and `Boolean`, 2 for the other five
      unfocused, 4 for a focused `Text`, `Number` and `DateTime`, 8 for a
      focused `Textarea` and `Json`, and one more than each of those when the
      field has an error row.
- [ ] `value_column` is the collection's longest `label` plus two, floored at
      18: 18 for `posts` and for the seven-type fixture, and greater than 18
      for a fixture with a 20-character label — and no inline widget is ever
      drawn over its own label.
- [ ] The focused field's label row begins `▸` and no other label row does;
      moving the focus moves the marker (frames A and D).
- [ ] A focused `Select`'s and `Boolean`'s widget is reverse video and an
      unfocused one is not, asserted on the buffer's cell styles.
- [ ] A required field's label ends in ` *` and an optional one's does not,
      asserted for both in one snapshot.
- [ ] An error row sits directly under its field's value row or box bottom,
      carries the `FieldError`'s own `message`, and is red — asserted on cell
      styles.
- [ ] A field with no `FieldError` has no error row, and three errors produce
      exactly three error rows (frame B).
- [ ] `first_visible` returns the least scroll that keeps the focused block and
      its trailing blank row whole, for every focus from 0 to 7 at 17 rows and
      at 9 rows; it returns 0 for frame D's state, and the last body row is
      never a box rule when anything is hidden below.
- [ ] The first body row carries `↑ n more` and the last `↓ n more`, dim,
      right-aligned with a two-column gutter and overlaid rather than
      replacing the row, and `n` counts the fields no row of which is drawn —
      asserted against frames A (`↓ 2 more`), B (`↓ 3 more`, with `Views` cut
      off after its label), G (`↓ 4 more`) and H (`↓ 1 more`).
- [ ] A form shorter than the panel carries neither summary.
- [ ] The header's right segment is ` ● unsaved · Ctrl+S ` when
      `draft != original` and absent when they are equal (frames D and E), and
      it goes through `chrome::shell`'s existing `right` parameter rather than
      a second mechanism.
- [ ] `<n> field(s) need attention` is produced by `view` from
      `editor.errors`, is absent when `errors` is empty, and no `StatusMessage`
      is written on `SaveFailed` — asserted by checking `state.status` is
      unchanged across a `SaveFailed`.
- [ ] That line reads `1 field needs attention` and `3 fields need attention`:
      the verb agrees as well as the noun.

The seven widgets

- [ ] All seven field types render in one snapshot of a fixture collection with
      one field of each, six unfocused and one focused (frame H).
- [ ] A focused `Textarea` and a focused `Json` render a five-row box, and the
      other five a one-row box (frames G and A).
- [ ] `Boolean` renders `[x]` for `true` and `[ ]` for `false`, inline at the
      value column.
- [ ] `Select` renders `‹ draft ›` for a value and `‹ — ›` for none, inline at
      the value column, with the `—` dim.
- [ ] `display` returns the editable text, not the table's: a `DateTime` is the
      stored RFC 3339 string and not `2026-09-06 12:00`, and a `Json` object is
      its JSON text and not `{3}` — asserted against `table::cell` returning
      something different for the same inputs.
- [ ] An absent or null value renders a dim `—` for every field type, including
      inside a focused box, and no type renders it as an empty row.
- [ ] A value whose JSON type the field does not allow renders as its own JSON
      text with `dim` set and does not panic — asserted for a number in a
      `Text` field and a string in a `Boolean` field.
- [ ] `parse(kind, &editable(kind, value))` returns `value` again for every
      field type and every JSON value that type accepts.
- [ ] A value wider than a one-row box draws its tail with `…` in the first
      text column, counted in characters and not bytes, and never writes past
      the box's right border.
- [ ] The seven-type fixture's slug and field names differ from `posts`', and
      it is rendered by the same `form::render`.

Editing

- [ ] `NextField` and `PrevField` move `focus` by one and clamp at both ends,
      returning no effects.
- [ ] `BeginEdit` on one of the five single-line types sets `editor.input` to
      `editable(kind, value)`; on a `Boolean` it toggles the draft; on a
      `Select` it advances the option — one command, dispatched on
      `FieldType`, with an arm for all seven.
- [ ] `InputChar` appends to `editor.input`, `InputBackspace` removes the last
      character, and `InputBackspace` on an empty buffer is a no-op.
- [ ] `InsertNewline` appends `\n` to `editor.input`, and resolves only in
      `Context::EditorText`.
- [ ] None of `InputChar`, `InputBackspace` or `InsertNewline` changes
      `editor.draft`, and the state stays clean through all three.
- [ ] `CommitField` writes the parsed value into `draft` under the field's
      name, clears `editor.input`, and leaves `focus` where it was.
- [ ] `CommitField` on `"42"` in a `Number` field stores the JSON number `42`,
      not the string `"42"`.
- [ ] `CommitField` on text that does not parse leaves `editor.input` `Some`
      with the text unchanged, leaves `draft` unchanged, and puts one
      `FieldError` for that field in `errors` with the code and message
      [002](002-record-validation.md) specifies — asserted for `Number`,
      `DateTime` and `Json`.
- [ ] A later successful `CommitField` on that field removes its error and
      leaves the other entries alone.
- [ ] `CancelEdit` clears `editor.input` and leaves `draft` byte-identical to
      what it was before `BeginEdit` — asserted after a `BeginEdit` and three
      `InputChar`s, and after a failed commit.
- [ ] `ToggleBoolean` flips a `Boolean` field's value and leaves every other
      key of the draft untouched.
- [ ] `NextOption` and `PrevOption` move through `options` and wrap; from no
      value `NextOption` selects the first option and `PrevOption` the last.
- [ ] `NextOption` twice on an empty required `Select` with
      `[draft, published]` leaves `published` in the draft
      ([000](000-m0-golden-path.md) step 7).
- [ ] An editor command aimed at the wrong field type — `ToggleBoolean` on a
      `Text`, `NextOption` on a `Number` — changes nothing and returns no
      effect.
- [ ] The cursor is in the snapshot when `editor.input` is `Some` and absent
      when it is `None`, for the same focused field (frames A and C).
- [ ] Every editing command leaves `draft` an object with exactly the schema's
      field keys, asserted after the whole of [000](000-m0-golden-path.md)
      step 7's sequence.

Dirty state and leaving

- [ ] A draft that has been committed to reports dirty, and one edited back to
      its original value reports clean; `EditorState` holds no `dirty` flag.
- [ ] `Back` at `New` or `Edit` with a clean draft sets `Route::List` for the
      same collection, clears the editor, opens no overlay and returns no
      effect.
- [ ] `Back` with a dirty draft sets `overlay` to
      `ConfirmDiscard { next: List { collection } }`, leaves `route` at the
      editor, and returns no effect.
- [ ] `Confirm` on that overlay sets `route` to the overlay's `next`, clears
      the overlay and the editor, and returns no effect.
- [ ] `Dismiss` closes the overlay and leaves `route`, `draft`, `focus` and
      `offset` unchanged.
- [ ] No command other than `Confirm` and `RecordSaved` moves `route` off
      `New` or `Edit` while the draft is dirty — asserted by sending every
      `Command` variant to a dirty editor state and checking the route.
- [ ] The `ConfirmDiscard` snapshot matches frame I: the dialog centred over
      the main panel, the panel still drawn behind it, and the hint bar
      `y Discard   n Cancel`.

Saving

- [ ] `Save` at `Route::New` returns exactly one
      `Effect::SaveRecord { collection, id: None, data }` whose `data` is the
      draft, and one at `Route::Edit` with `id: Some(the route's id)`.
- [ ] `Save` returns exactly one effect per command: two `Save`s in a row
      produce two effects, and one `Save` never produces two.
- [ ] `run_effects` turns `SaveRecord { id: None }` into `RecordSaved` with the
      record `RecordService::create` returns, against a `MemoryRepository`.
- [ ] `run_effects` turns `SaveRecord { id: Some(..) }` into `RecordSaved` with
      the record `update` returns, with the same id and a later `updated_at`.
- [ ] `run_effects` turns `AppError::Validation(errors)` into
      `SaveFailed(errors)` with the field errors unchanged — same order, same
      `field`, same `code`, same `message` — and into no `OperationFailed`.
- [ ] `run_effects` turns every other `AppError` from a save into
      `OperationFailed(error.to_string())` and into no `SaveFailed`.
- [ ] `run_effects` turns `LoadRecord` into `RecordLoaded`, and a missing
      record into `OperationFailed`.
- [ ] `SaveFailed` stores the errors, leaves `draft`, `focus`, `offset` and
      `route` equal to what they were, and returns no effect.
- [ ] `SaveFailed` replaces the previous errors rather than appending: two
      failed saves leave one set.
- [ ] `RecordSaved` sets `Route::List` for the collection, clears the editor,
      sets an `Info` status reading `Record saved.`, and returns exactly one
      `Effect::LoadRecords` for that collection — from both `New` and `Edit`
      ([000](000-m0-golden-path.md) §Invariants).
- [ ] `OperationFailed` after a `LoadRecord` that found nothing sets
      `Route::List` and returns one `LoadRecords`; after a failed *save* it
      changes neither the route nor the draft.
- [ ] The save round trip settles in `drive` without a redraw in the middle:
      `Save` → `SaveRecord` → `RecordSaved` → `LoadRecords` →
      `RecordsLoaded`, asserted with a scripted effect runner and a
      `TestBackend`.
- [ ] A save that fails does not end the loop: `OperationFailed` is a command
      like any other and the session continues.
- [ ] The repository is not written when the service refuses: `count` is
      unchanged after a `SaveFailed`, asserted against `MemoryRepository`.

Keymap and hints

- [ ] Every row of "The keymap's new rows" resolves to the command it names, in
      the context it names, and nowhere else.
- [ ] `resolve(Context::EditorInput, k)` returns `InputChar(c)` for every
      printable character, `CommitField` for `Enter`, `CancelEdit` for `Esc`,
      and nothing for `Tab`, `j` or `Ctrl+S`.
- [ ] `resolve(Context::EditorText, Enter)` is `InsertNewline` and
      `resolve(Context::EditorText, Tab)` is `CommitField` — the two keys swap
      roles between the input contexts, and `resolve(Context::EditorInput,
      Enter)` is still `CommitField`.
- [ ] `resolve(Context::ConfirmDiscard, k)` returns `Confirm` for `y` and
      `Enter` and `Dismiss` for `n`, `Esc` and `q`, and nothing for `Ctrl+S`.
- [ ] `Ctrl+C` still quits from `Editor`, both input contexts and
      `ConfirmDiscard`: a context shadows a global binding, it does not
      replace it.
- [ ] `hints` returns exactly §UX's four bars for `Editor`, `EditorInput`,
      `EditorText` and `ConfirmDiscard`, and
      `["↑↓ Navigate", "Enter Edit", "n New", "Esc Back"]` for `List`.
- [ ] Every frame's hint bar equals `hints(its context)` joined with three
      spaces, computed from `KEYMAP` in the test; no literal hint text appears
      in a renderer or a test.
- [ ] Every visible hint still names a key that resolves to its own command,
      and `?` still resolves to nothing in all seven contexts.
- [ ] Every `Command` variant the enum declares is reachable from at least one
      binding or one effect result — a scan over `KEYMAP` plus the three
      result commands, so a variant nobody can trigger cannot ship.
- [ ] No `KeyCode::` outside `keymap.rs` — the existing source scan still
      passes.

Purity and boundaries

- [ ] `update` returns the same state and effects for the same input, called
      twice, for every command this spec adds; no test of `update` opens a
      repository.
- [ ] Every snapshot test calls `view` with no service in scope, and
      `display`, `editable`, `parse`, `value_column`, `block_height` and
      `first_visible` take no clock, no `AppState` and no `Schema`.
- [ ] `blank_document` is pure: called twice for the same collection it returns
      equal documents, and it opens no repository.
- [ ] `pressa-tui/Cargo.toml` gains no dependency, in any section:
      `serde_json`, `chrono` and `pressa-core` are all still absent, and
      neither does `pressa-core/Cargo.toml` or `pressa-app/Cargo.toml`.
- [ ] `pressa-tui/tests/architecture.rs` still passes unchanged.
- [ ] No `unwrap()`, `expect()` or `panic!()` outside `#[cfg(test)]`; indexing
      a `Vec` by a stored index happens through `get`.
- [ ] No identifier in `pressa-tui` is a collection slug or a field name.

The Golden Path

- [ ] Steps 5 to 11 and 13 of [000](000-m0-golden-path.md) pass against a real
      `SqliteRepository` in a `TempDir`, driven through `update` and
      `run_effects`: `n`, a refused save, four filled fields, a save, the
      list, `Enter`, a rename, a save.
- [ ] Step 6: exactly three `FieldError`s come back — `title`, `slug`,
      `status` — each with `code == ErrorCode::Required` and
      `message == "required"`, in schema field order, and `count("posts")` is
      0 immediately afterwards.
- [ ] After step 8 the database holds one record whose `title`, `slug`,
      `status` and `views` are `"Hello world"`, `"hello-world"`,
      `"published"` and the JSON number `42`, read back through
      `RecordService`.
- [ ] Step 11 updates in place: same `id`, same `created_at`, strictly later
      `updated_at`.
- [ ] After dropping the repository and reopening the same database file, that
      record is still there with the renamed title — the claim the whole
      milestone is for.
- [ ] The same sequence against a collection whose slug is not `posts`, with
      identically shaped fields, produces the same state transitions and the
      same stored data.

Documents

- [ ] [`tui.md`](../docs/tui.md) §1 lists `EditorState`, `Overlay` and the
      three new `Effect` variants as shipped; §2 records the third crumb naming
      the record rather than the four-crumb id form it promised (Q8); §3 lists
      the seven contexts, the new keymap rows, and the two input contexts; §5.2
      carries the two-sentence empty state; §5.3 is replaced by this spec's
      "The form, stated once" and a pointer here; §5.4 keeps the delete dialog
      and gains the discard one; §6 admits `▌` to the character set.
- [ ] [SPEC-006](006-tui-shell.md) "Breadcrumbs" is corrected where it promises
      T9 adds `["pressa", "Posts", "Edit", "01J8XQ…"]`.
- [ ] [`domain-model.md`](../docs/domain-model.md) §3 records that no M0 schema
      can express a `false` capability and where the `create` / `update` checks
      go when one can (Q12), and §8 records that `InvalidJson` and
      `InvalidDateTime` are produced by `form::parse`.
- [ ] [000](000-m0-golden-path.md)'s frames C, D, E, G and I are regenerated
      from this spec's rules — the box border, the three-space hint bar, the
      `▸` marker, the cursor rule, the renamed crumbs and frame E's scroll —
      and its two [008](008-record-editor.md)-owned details, the editor's
      scroll markers and `‹ — ›`, match §UX. Its step-7 command sequence still
      names the commands those keystrokes resolve to.
- [ ] [007](007-list-view.md)'s frames A–I are retaken with the new `List` hint
      bar, and its frame A with the two-sentence empty state (frame K);
      [007](007-list-view.md) Q2 said T9 would.
- [ ] [`roadmap.md`](../docs/roadmap.md) §2's T10 row reads "Delete (`d`) with
      confirmation" — `n` belongs to T9 (Q13) — and §2's T9 row is ticked.
- [ ] The `sandbox` recipe and its `README.md` say what `dev` now lets a human
      do — create, edit and save a record — and `just sandbox` was run
      ([`development.md`](../docs/development.md) §8).
- [ ] This spec's status is `Implemented`.

## Tests

`pressa-tui/tests/`, plus unit tests beside the modules they cover. No test
opens a real terminal; only the `run_effects` and Golden Path tests open a
repository.

| Criteria | Test |
|---|---|
| Frames A–K | `insta` snapshots through `ratatui::TestBackend`, in `tests/editor_snapshots.rs` |
| Red error rows, the dim `—`, the dim scroll summaries, reverse-video inline widgets, the unsaved marker | assertions on `Buffer` cell styles, in the same file |
| `block_height`, `first_visible`, `value_column`, `display`, `editable`, `parse`, the round trip | table-driven unit tests in `view/form.rs`, one case per field type |
| `blank_document`, and `RecordService::blank` returning the same thing | unit test in `pressa-core`, plus one case in `pressa-app/tests/record_service.rs` |
| Routes, focus movement, the input buffer, the draft writes, parse failures, dirty, the overlay, `SaveFailed`, `RecordSaved` | `update` unit tests: build a state, send a command, assert state and effects |
| "no command moves the route off a dirty editor" and "every `Command` variant is reachable" | two tests that iterate the enum and `KEYMAP`, so a variant added later cannot slip past them |
| `LoadRecord`, `SaveRecord` in both shapes, `Validation` → `SaveFailed`, other `AppError` → `OperationFailed` | integration tests in `tests/effects.rs` over `MemoryRepository`, including a repository that fails |
| The save round trip draining in one pass | `tui::drive` with a scripted event source and a scripted effect runner, in `tests/shell.rs` |
| Keymap resolution and hints for the four new contexts | unit tests in `keymap.rs`, as T7's and T8's |
| Golden Path steps 5–11 and 13 | `pressa-tui/tests/golden_path.rs`, a `TempDir` and a real `SqliteRepository`. T9 writes the steps it owns; T12 completes the file ([000](000-m0-golden-path.md) §Tests) |

Fixtures: `examples/blog/pressa.yaml` for frames A–G and I–K, loaded rather
than copied; the seven-type collection (frame H) and the long-label collection
written as YAML in the test through `support::schema_from_yaml`. Records are
built through `RecordService::create` against `MemoryRepository`, so no test
constructs a `Record` by hand.

Snapshot review is `cargo insta review`, never `--accept`
([ADR-0011](../docs/adr/0011-insta-for-snapshot-tests.md)).

## Open questions

None. The fifteen this spec was drafted with were answered by the coordinator
on 2026-10-03 and are folded in above.

### Decisions taken

- **Q1 — `Enter` in the editor.** `Enter` resolves to one command,
  `BeginEdit`, whose `update` arm dispatches on the focused field's
  `FieldType`: an input context for the five text-shaped types, a toggle for a
  `Boolean`, the next option for a `Select`. `ToggleBoolean`, `NextOption` and
  `PrevOption` also get keys — `Space`, `l`/`→`, `h`/`←`, all `Hidden` — so no
  command is reachable only from inside another's arm. The keymap stays a table
  returning one command for one key ([ADR-0005](../docs/adr/0005-command-and-keymap-architecture.md));
  the per-type branch is in `update`, where
  [ADR-0004](../docs/adr/0004-schema-driven-ui.md) wants it. [000](000-m0-golden-path.md)'s
  step-7 sequence is still accurate, because `NextOption` is still what that
  keystroke produces.
- **Q2 — the blank draft.** `blank_document(&Collection) -> Json` becomes a
  pure function in `pressa-core`, re-exported through `pressa_app::domain` so
  `update` can call it, with `RecordService::blank` delegating. No fourth
  `Effect`, no "effect" that performs no I/O, and the defaults stay stated
  once. It is the one answer that touches crates T9's roadmap row does not
  name; it adds no dependency anywhere, and recording it here rather than
  discovering it mid-diff is what the stop rule asks for
  ([`AGENTS.md`](../AGENTS.md)).
- **Q3a — focus.** `▸` in the second indent column of the focused field's label
  row, for **every** field type, plus reverse video on a focused inline
  `Select` or `Boolean` widget. One marker, the same character the table uses
  for its selected row, so focus reads the same way in both screens; block
  heights are unchanged, which is why frames A–J keep their geometry.
- **Q3b — the multi-line box, and `Enter`.** `Textarea` and `Json` keep their
  five-row box, and a newline is typed with `Enter` — which means the key that
  *commits* those two fields is `Tab`. Expressed as two input contexts,
  `EditorInput` and `EditorText`, derived from the focused field's type: a
  table lookup still returns one command for one key, and each context gets its
  own correct hint bar. No modifier key, and no dependence on the kitty
  keyboard protocol, which `Shift+Enter` would have needed and which the
  terminals most people use do not report.
- **Q3c — overflow.** A one-row box shows the tail of its value with `…` in the
  first text column; a five-row box wraps. Nothing is clipped silently
  ([`tui.md`](../docs/tui.md) §6), and `…` is the marker the table already
  uses.
- **Q4 — the scroll rule.** Scroll by whole fields, the least that keeps the
  focused field's block *and its trailing blank row* whole. Including the blank
  row is not cosmetic: it is what keeps a box's bottom rule off the last body
  row, where the `↓ n more` summary is drawn. Frame D is regenerated — it needs
  no scroll at all — because [000](000-m0-golden-path.md)'s `Status` start is
  not produced by this rule or by any other that was stated.
- **Q5 — dirty.** `draft != original`, computed where it is needed and stored
  nowhere, so a value edited back to itself reports clean. The states are clean
  and dirty: `Saving` is unreachable for the reason ADR-0002 made
  `Load::Loading` unreachable ([SPEC-007](007-list-view.md) "Non-goals"), and
  `Saved` is a route change. The header is the only place either shows.
- **Q6 — a commit that will not parse.** The editor parses on commit and
  refuses: the field stays in its input context with the typed text intact, and
  the editor produces the `FieldError` itself. This is what
  [002](002-record-validation.md) means by "`InvalidJson` is produced by the
  editor when parsing user text" — the one `ErrorCode` `validate_record` never
  returns — and it keeps the draft free of values its own schema forbids. The
  messages are [002](002-record-validation.md)'s, unchanged. `Esc` always
  abandons the field.
- **Q7 — unfocused values.** The editable text, not `table::cell`: a
  `DateTime` reads as the stored RFC 3339 string and a `Json` field as its JSON
  text, so what is on screen is what you would be editing and a value does not
  change format when you focus it. Two of the table's rules carry over because
  they are about absence rather than format — the dim `—`, and dim JSON for a
  value the field's type forbids. `{3}` tells a user nothing about their own
  data in a form they came to edit it in.
- **Q8 — the third crumb.** The draft's first `list_columns` value through
  `table::cell`, truncated to 20 characters, read live so it follows a rename;
  the record id shortened to six characters and `…` at `Edit` when that value
  cannot name the record, and the literal `New` at `New`. `breadcrumbs`
  therefore takes the state rather than the route alone.
  [`tui.md`](../docs/tui.md) §2 and [SPEC-006](006-tui-shell.md)'s promise of a
  fourth, id-only crumb are corrected: a ULID is not what a user recognises a
  record by.
- **Q9 — the value column.** The collection's longest field `label` plus two,
  never less than 18. Derived from the schema alone, so it is snapshot-stable
  and blind to the data; inline widgets line up within a collection; and a
  label longer than sixteen characters pushes the column out rather than being
  clipped. `posts` comes out at 18, so no frame moved.
- **Q10 — the overlay's hints and wording.** One `Context` per overlay —
  `ConfirmDiscard` now, `ConfirmDelete` in T10 — so `y Discard` and
  `y Delete` each come from their own row and resolution stays a table lookup.
  `Context` is already derived from state, so deriving it from `state.overlay`
  is the same mechanism. The dialog is `Discard changes` /
  `Discard unsaved changes?` / `This cannot be undone.`, parallel in shape to
  the delete dialog but naming the changes rather than the record.
- **Q11 — the attention line.** Derived by `view` from `editor.errors`, not a
  `StatusMessage`. It cannot go stale, nothing has to decide what clears it,
  and the status line stays reserved for things that happened rather than a
  count of current state — so `RecordsLoaded`'s existing rule needs no
  exception. The verb agrees as well as the noun.
- **Q12 — capabilities.** Not read in T9. Every M0 capability is `true` and
  nothing in [001](001-schema-config.md) can produce a `false` one, so the
  branches would be unreachable through the loader and untestable without
  hand-building a `Collection` no YAML can express.
  [`domain-model.md`](../docs/domain-model.md) §3 gains a note saying where the
  `create` and `update` checks go when a schema can express them, so the
  decision is recoverable in M3 rather than rediscovered.
- **Q13 — the roadmap's T10 row.** Corrected to "Delete (`d`) with
  confirmation". T9 ships `n`, `Enter`, both routes and the editor; T10 ships
  `d`, `Overlay::ConfirmDelete` and `Effect::DeleteRecord`. Two tasks claiming
  the same key is exactly the drift the roadmap exists to prevent.
- **Q14 — reloading the list.** After a save, and after a `LoadRecord` that
  found nothing — the two cases where the list is provably stale, the second
  because it is showing a row that no longer exists. `Back` from a clean form
  and a confirmed discard return to the list untouched: neither changed the
  database, so a query would be work done to change nothing.
- **Q15 — what marks input mode.** The cursor, and only the cursor. `▌` is
  drawn if and only if `editor.input` is `Some`, so a focused field in
  `Context::Editor` shows its `▸`, its box and its value and nothing else. The
  cursor then means exactly one thing: the next printable key lands here.
  [000](000-m0-golden-path.md)'s frames C, E and G drew a cursor while
  explicitly in `Editor` mode and are corrected along with the box border.
