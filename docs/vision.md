# Vision

Status: Approved · Last updated: 2026-09-06

## 1. What Pressa is

**Pressa is a terminal-native, schema-driven data manager.**

You declare your entities in a YAML file, run one command, and get a
keyboard-driven admin UI in your terminal that can list, create, edit,
validate and delete records — without writing a line of UI code.

Content management is the *first* use case, not the definition. The same model
fits invoices, products, feature flags, configuration, seed data, or any small
structured dataset a developer wants to edit locally without spinning up a
browser, a server, or a Docker container.

```
pressa.yaml  ──▶  Schema  ──▶  generic list view + generic record editor
                    │
                    └────────▶  validation + SQLite storage
```

The single most important architectural bet: **there is no "Posts screen".**
There is one list view and one record editor that render *any* collection from
its schema. Adding an entity is a config change, never a code change.

## 2. Who it is for

A developer or a technical editor who:

- works in the terminal and does not want to leave it;
- needs to inspect and edit local structured data regularly;
- finds a full CMS (server, database, admin build step) disproportionate;
- values keyboard-first, fast, boring tooling over a pretty web UI.

## 3. The canonical screen

```
┌──────────────────────────────────────────────────────────────────────┐
│ pressa > Posts > Edit > 01J8X…                          [Ctrl+S] Save│
├──────────────┬───────────────────────────────────────────────────────┤
│              │                                                       │
│ Collections  │  Title                                                │
│              │  ┌───────────────────────────────────────────────┐    │
│ > Posts      │  │ Hello world                                   │    │
│   Authors    │  └───────────────────────────────────────────────┘    │
│   Categories │                                                       │
│              │  Slug                                                 │
│              │  hello-world                                          │
│              │                                                       │
│              │  Status        published                              │
│              │                                                       │
├──────────────┴───────────────────────────────────────────────────────┤
│ ↑↓ Navigate  Enter Edit  Esc Back  Ctrl+S Save  / Search  ? Help     │
└──────────────────────────────────────────────────────────────────────┘
```

Left panel is always present. The bottom hint bar changes with context. No
decoration beyond what carries information: this should read as a *tool*, not
as a website rendered in a terminal.

## 4. Goals of M0 (the MVP)

M0 is done when the Golden Path in [`specs/000-m0-golden-path.md`](../specs/000-m0-golden-path.md)
passes end to end:

1. Declare collections in `pressa.yaml`.
2. Run `pressa` in the project directory.
3. Browse collections in the sidebar.
4. Browse records of the selected collection in a table.
5. Create, open, edit and delete records with the keyboard.
6. Get validation errors from the schema before anything is saved.
7. Quit, restart — the data is still there.

Anything beyond that is not M0.

## 5. Anti-goals

Deliberately **not** in M0. Each one is a natural next step and each one is
capable of consuming the entire project if allowed in early:

| Not in M0 | Where it may land |
|---|---|
| Authentication, RBAC, multi-user | M6 |
| REST API, GraphQL, MCP server | M5 |
| Web UI | not planned |
| Plugins, hooks, scripting (Rhai/WASM) | M4 |
| Rich text editor, media library, uploads | after M2 |
| Relation fields, many-to-many | M2 |
| Drafts, versioning, publish workflow | after M3 |
| Localization | after M3 |
| Filters, sorting UI, bulk operations, command palette | M1 |
| Postgres, MongoDB, remote sync, realtime | M6 |
| Schema migrations for existing data | M3 |

The rule for agents: **a feature on this list is not "an improvement", it is a
scope violation.** See [`AGENTS.md`](../AGENTS.md).

## 6. References and what we take from each

| Reference | What we borrow | What we ignore |
|---|---|---|
| [Payload CMS](https://payloadcms.com) | Domain model: `Collection → Fields → Document`; declarative config; strict separation of the data layer from the admin UI; a Local API as the central business API | Its feature surface — access control, hooks, versioning, plugins — all of which arrive later, if ever |
| [lazygit](https://github.com/jesseduffield/lazygit) | Interaction model: persistent sidebar, keyboard-first, context-sensitive hint bar, panels, minimal decoration | Its git-specific screen composition |
| [discovery.js](https://github.com/discoveryjs/discovery) | Data-centric interaction: data → table → selection → details → filter → exploration | Its component architecture — a browser has a mouse, pixels and infinite space; a terminal has a keyboard, characters and a fixed viewport, so our UI is modal and command-oriented instead |

## 7. Why "Pressa" and not "CMS"

The internal vocabulary avoids the word CMS on purpose. The engine manages
*collections of records described by a schema*. Calling that a CMS narrows the
design — it invites content-specific concepts (drafts, publishing, media) into
the core, where they do not belong. CMS is a preset, not the product.

## 8. Success criteria beyond M0

Pressa is working if adding a brand-new entity type to a project takes editing
`pressa.yaml` and nothing else, and if a new field type is a change confined to
`pressa-core` plus one editor widget in `pressa-tui`.
