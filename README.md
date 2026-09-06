# Pressa

A terminal-native, schema-driven data manager in Rust.

Declare your collections in a YAML file, run one command, and get a
keyboard-driven admin UI in your terminal — list, create, edit, validate and
delete records, with no server, no browser and no UI code.

> **Status: design phase.** No code yet. The full blueprint is in `docs/` and
> `specs/`; implementation starts at task T1 in
> [`docs/roadmap.md`](docs/roadmap.md).

```
┌ pressa › Posts ───────────────────────────────────────────── 2 records ───┐
├──────────────┬────────────────────────────────────────────────────────────┤
│ Collections  │  TITLE            STATUS      VIEWS   UPDATED               │
│              │ ─────────────────────────────────────────────────────────  │
│ > Posts      │ ▸Hello world      draft           0   2m ago                │
│   Authors    │  About page       published      42   yesterday             │
│   Categories │                                                            │
├──────────────┴────────────────────────────────────────────────────────────┤
│ ↑↓ Navigate  Enter Edit  n New  d Delete  / Search  Esc Back  ? Help       │
└───────────────────────────────────────────────────────────────────────────┘
```

## Idea

```yaml
# pressa.yaml
collections:
  posts:
    label: Posts
    list_columns: [title, status]
    fields:
      - { name: title,  type: text,   required: true }
      - { name: status, type: select, options: [draft, published] }
```

```bash
pressa
```

The UI is generated from the schema. Adding an entity is a config change, never
a code change.

## Documentation

| | |
|---|---|
| [`docs/vision.md`](docs/vision.md) | What this is, who it is for, what it will never be |
| [`docs/architecture.md`](docs/architecture.md) | Crates, layers, the dependency rule |
| [`docs/domain-model.md`](docs/domain-model.md) | Schema, Collection, Field, Record, Value |
| [`docs/storage.md`](docs/storage.md) | SQLite schema, repository, migrations |
| [`docs/tui.md`](docs/tui.md) | State model, commands, keymap, screens |
| [`docs/development.md`](docs/development.md) | How work happens: specs, ADRs, agent roles |
| [`docs/roadmap.md`](docs/roadmap.md) | M0 task list and what comes after |
| [`docs/adr/`](docs/adr/) | Why the project is built this way |
| [`specs/`](specs/) | What each feature must do |
| [`AGENTS.md`](AGENTS.md) | The rules, for contributors and agents |

## Stack

Rust · [ratatui](https://ratatui.rs) + crossterm · SQLite via rusqlite
(synchronous) · serde + serde_yaml · clap · thiserror/anyhow · tracing ·
insta snapshot tests.

## Development

```bash
just ci      # fmt --check + clippy -D warnings + test
just run     # open the example project
just snap    # review snapshot diffs
```

## Provenance

`ai/` holds four independent design plans produced by different AI agents.
`docs/` is their synthesis; where they disagreed, the resolution is recorded as
an ADR and summarised in [`docs/roadmap.md`](docs/roadmap.md) §4.

## References

Payload CMS for the domain model, lazygit for the interaction model,
discovery.js for data-centric exploration. What we take from each and what we
deliberately do not: [`docs/vision.md`](docs/vision.md) §6.
