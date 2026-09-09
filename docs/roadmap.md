# Roadmap

Status: Approved · Last updated: 2026-09-09

## 1. Milestones

| | Name | Content |
|---|---|---|
| **M0** | Vertical slice | Schema from YAML, SQLite storage, sidebar + list + editor, 7 field types, validation, create/edit/delete, search, help. **The current milestone.** |
| M1 | Data management | Filters, sort UI, pagination, bulk delete, duplicate, command palette, import/export JSON+CSV |
| M2 | Relations | `Relation` field type, relation picker overlay, reverse relations |
| M3 | Schema management | Schema diffing against the `collections` snapshot, data migrations, `pressa generate` |
| M4 | Extensibility | Hooks (`beforeChange`, `afterRead`), scripting, custom field types |
| M5 | APIs | Local API crate, REST server, MCP server |
| M6 | Collaboration | Auth, RBAC, audit log, Postgres backend |

Milestones after M0 are direction, not commitment. They exist so that "not now"
has a place to point at.

## 2. M0 order of work

Vertical, not layered — see [ADR-0007](adr/0007-vertical-slice-first.md). After
T9 the Golden Path works end to end; T10–T12 make it comfortable.

| # | Task | Crates | Spec | Key acceptance |
|---|---|---|---|---|
| T1 | Skeleton: workspace, 4 crates, justfile, CI, tracing to file | all | — | `just ci` green on an empty workspace |
| T2 | Load `pressa.yaml` → `Schema`; project discovery | core, app | [001](../specs/001-schema-config.md) | Unknown field type / duplicate name / bad `list_columns` produce an error naming the path |
| T3 | Record validation against the schema | core | [002](../specs/002-record-validation.md) | Errors are `Vec<FieldError>` with a `code`, one per offending field |
| T4 | SQLite: migrations, `SqliteRepository`, `MemoryRepository` | storage | [003](../specs/003-storage-repository.md) | Shared contract suite passes against both implementations |
| T5 | `RecordService`, `CollectionService`, `AppError` | app | [004](../specs/004-app-services.md) | Integration test: load → create → list → update → delete |
| T6 | CLI: `pressa init`, `pressa dev`, `pressa validate` | tui | [005](../specs/005-cli.md) | `assert_cmd`: `init` writes `pressa.yaml` and `.pressa/`; `validate` exits non-zero on a bad schema |
| T7 | TUI shell: terminal guard, layout, sidebar, breadcrumbs, hint bar, keymap | tui, app | [006](../specs/006-tui-shell.md) | 80×24 snapshot of Home; terminal restores on panic |
| T8 | List view: table from `list_columns`, navigation, empty state | tui | [007](../specs/007-list-view.md) | Snapshots before and after two `j`; `Enter` emits `EditRecord` |
| T9 | Record editor: form from schema, 7 editors, dirty state, save, errors | tui | [008](../specs/008-record-editor.md) | **Closes the Golden Path** — a saved record survives a restart |
| T10 | Create (`n`), delete (`d`) with confirmation | tui | 009 | Deletion is impossible without confirming; selection stays valid afterwards |
| T11 | Search `/` and help overlay `?` | tui | 010 | Help is generated from the keymap, not written by hand |
| T12 | `examples/blog`, README, end-to-end Golden Path test | — | [000](../specs/000-m0-golden-path.md) | Spec 000 passes automatically |

Specs 009 and 010 are written when T10 and T11 start — see
[`development.md`](development.md) §3.

## 3. Deliberately deferred

Each of these was proposed in at least one of the source plans in `ai/` and is
out of scope on purpose. Listed here so that "why isn't this here?" has an
answer and agents do not reintroduce them as improvements.

| Deferred | Proposed in | Reason |
|---|---|---|
| tokio + sqlx | 1.md, 4.md | [ADR-0002](adr/0002-synchronous-rusqlite.md) — no concurrency to manage in a single-user local app |
| `Relation` field type | 4.md | Only type needing a second data path and a picker overlay; M2 |
| Command palette | 1.md, 4.md | Real value, but it is breadth; M1, once there are enough commands to search |
| Hooks, Rhai, WASM plugins | 1.md, 2.md, 3.md | An extension surface before there is anything worth extending; M4 |
| REST / GraphQL / MCP | 2.md, 4.md | The service layer keeps the door open; M5 |
| tui-realm | 1.md | A component framework over ratatui adds abstraction between us and the render we snapshot-test |
| RON config | 1.md | YAML is what a user editing a schema by hand expects |
| Undo/redo stack | 1.md | Cheap in Elm architecture, but nothing in the Golden Path needs it |
| Per-collection SQL tables | — | [ADR-0003](adr/0003-json-documents-not-eav.md) |
| FTS5 search | 2.md | `LIKE` over `list_columns` is enough at MVP data sizes |
| Writing the `collections` config snapshot | — | The table is created in M0 and written in M3, where it is first read. Writing it earlier costs a port method, both adapters and a contract case for a table nothing opens ([SPEC-006](../specs/006-tui-shell.md) Q3, [`storage.md`](storage.md) §2) |

## 4. Provenance

M0 is a synthesis of four independent plans in `ai/`, which agreed on the core
(schema-driven UI, ratatui, SQLite with JSON documents, layered architecture,
SDD) and disagreed on six points. The disagreements and their resolutions:

| Question | Positions | Resolution |
|---|---|---|
| sync vs async | rusqlite (2, 3) vs tokio+sqlx (1, 4) | [ADR-0002](adr/0002-synchronous-rusqlite.md): sync |
| repo structure | workspace (1, 2, 3) vs single crate (4) | [ADR-0006](adr/0006-workspace-of-four-crates.md): workspace, 4 crates |
| config format | RON (1) vs YAML (2, 3, 4) | YAML |
| order of work | by layer (1, 2, 3) vs vertical slice (4) | [ADR-0007](adr/0007-vertical-slice-first.md): vertical |
| field types in MVP | with `Relation` (4) vs without (2, 3) | Without; M2 |
| naming | terminal-cms / cms / "don't call it CMS" (4) | `pressa`, crates `pressa-*` |

`ai/1.md` … `ai/4.md` are kept as the archived source material.
