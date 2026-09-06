# ADR-0004: The UI is generated from the schema

Status: **Accepted** · Date: 2026-09-06

## Context

The obvious way to build a CMS admin UI is a screen per entity: a Posts list, a
Posts form, a Users list, a Users form. It is also how the project fails — each
new entity is a code change, and the codebase grows linearly with the user's
data model.

All four source plans identified the alternative independently, and it is the
one idea shared by both stated references: Payload builds its admin UI from
`collections` config, and discovery.js builds views from a declarative view
model.

## Decision

There is exactly **one** list view and **one** record editor. Both take a
`&Collection` and render whatever it describes. No screen, widget or code path
may branch on a specific collection slug.

## Rationale

- Adding an entity becomes a config change: the user edits `pressa.yaml` and
  the UI already knows what to do.
- The codebase grows with the number of *field types* (bounded, ours) rather
  than the number of *collections* (unbounded, the user's).
- It concentrates UI complexity into a small number of heavily tested
  functions instead of spreading it across per-entity screens.

## Consequences

- Adding a field type is the expensive operation, and it has exactly three
  parts: validation in `pressa-core`, a table cell renderer, and a form editor
  widget. `FieldType` being an `enum` matched exhaustively means the compiler
  lists what you forgot.
- Collection-specific presentation must be expressed *in the schema*
  (`list_columns`, `label`, later `capabilities`), never in the renderer.
- A hardcoded `if collection.slug == "posts"` anywhere in `pressa-tui` is an
  automatic review rejection. This is the rule most likely to be violated by a
  well-meaning agent solving a specific test case.
