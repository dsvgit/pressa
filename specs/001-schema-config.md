# SPEC-001: Schema configuration

Status: **Implemented** · Task: T2 · Crates: `pressa-core`, `pressa-app`
· ADRs: [0003](../docs/adr/0003-json-documents-not-eav.md), [0004](../docs/adr/0004-schema-driven-ui.md)

## Problem

Everything downstream — the UI, validation, storage queries — is derived from
the schema. A malformed schema that loads successfully produces failures far
away from their cause, so loading must reject anything questionable, loudly and
with a location.

## Goal

Parse `pressa.yaml` into a validated `Schema`, or fail with an error that names
the exact path and the problem.

## Non-goals

Schema migration, schema diffing against stored data, multiple config files,
config inheritance, environment variable interpolation.

## Project discovery

Walk up from the current directory looking for `pressa.yaml`, the way git finds
`.git`. Stop at the filesystem root. `--project <dir>` overrides discovery and
skips the walk. The directory containing the found file is the project root;
`.pressa/` is resolved relative to it.

## Config format

```yaml
project:
  name: blog-cms          # required

database:                 # optional
  path: .pressa/data.db   # default

collections:              # required, at least one
  posts:
    label: Posts                        # optional, default: titlecased slug
    list_columns: [title, status]       # optional, default: first 4 fields
    fields:                             # required, at least one
      - name: title                     # required
        label: Title                    # optional, default: titlecased name
        type: text                      # required
        required: true                  # optional, default false
        unique: false                   # optional, default false
      - name: status
        type: select
        options: [draft, published]     # required when type is select
```

Accepted `type` values: `text`, `textarea`, `number`, `boolean`, `datetime`,
`select`, `json`.

## API

```rust
// pressa-app
pub fn discover_project(start: &Path) -> Result<ProjectPaths, ConfigError>;
/// `--project <dir>`: the schema must be in this directory. No walking up.
pub fn project_at(dir: &Path) -> Result<ProjectPaths, ConfigError>;
pub fn load_schema(path: &Path) -> Result<Schema, ConfigError>;

pub struct ProjectPaths {
    pub root: PathBuf,
    pub config: PathBuf,     // <root>/pressa.yaml
    pub data_dir: PathBuf,   // <root>/.pressa
    pub database: PathBuf,
    pub log: PathBuf,
}
```

## Invariants

- Collection order in `Schema.collections` equals the order in the file
  (`IndexMap`) — it is the sidebar order and must be deterministic.
- Every `list_columns` entry names a field that exists in that collection.
- Collection slugs and field names match `^[a-z][a-z0-9_]*$`. This is not
  cosmetic: field names are interpolated into `json_extract` paths in SQL
  (see [`storage.md`](../docs/storage.md) §4), so the pattern is a security
  boundary.
- Field names are unique within a collection; slugs are unique in the schema.
- `select` has at least one option, and options are unique.

## Error cases

Every error carries a YAML path. Message shape: `<path>: <problem>`.

| Input | Error |
|---|---|
| No `pressa.yaml` found while walking up | `ConfigError::NotFound { searched_from }` |
| Malformed YAML | `ConfigError::Parse` with line and column from `serde_yaml` |
| `collections` empty or missing | `collections: at least one collection is required` |
| `collections.posts.fields` empty | `collections.posts.fields: at least one field is required` |
| `type: relation` | `collections.posts.fields[3].type: unknown field type 'relation' (expected one of: text, textarea, number, boolean, datetime, select, json)` |
| Duplicate field name | `collections.posts.fields[4].name: duplicate field name 'title'` |
| `list_columns: [titel]` | `collections.posts.list_columns[0]: unknown field 'titel'` |
| `select` without options | `collections.posts.fields[2].options: required for type 'select'` |
| `select` with `options: []` | `collections.posts.fields[2].options: at least one option is required` |
| Duplicate `select` option | `collections.posts.fields[2].options[2]: duplicate option 'draft'` |
| Duplicate collection slug | `collections.posts: duplicate collection slug 'posts'` |
| Slug `Posts` or `my-posts` | `collections.Posts: must match ^[a-z][a-z0-9_]*$` |

`relation` gets a dedicated mention in the "expected one of" list because it is
the type users will most plausibly try; the message must not imply it is coming
in this version.

A missing `options` key and an empty `options: []` are different mistakes and
get different messages: the first forgot the key, the second wrote a `select`
with nothing to select.

A duplicate collection slug is a rejection rather than a last-one-wins merge.
YAML mappings deduplicate silently, so the parse keeps every block as written
and the duplicate is caught in validation — a collection disappearing without a
diagnostic is the failure this spec's Problem statement is about.

## Acceptance criteria

- [ ] A valid `pressa.yaml` loads into a `Schema` with collections in file order.
- [ ] `label` defaults to the titlecased slug/name when omitted.
- [ ] `list_columns` defaults to the first four field names when omitted.
- [ ] `required` and `unique` default to `false`.
- [ ] `database.path` defaults to `.pressa/data.db`.
- [ ] Every row of the error table above produces that error, with the path in
      the message.
- [ ] `discover_project` finds a config three directories up.
- [ ] `discover_project` returns `NotFound` at the filesystem root rather than
      looping.
- [ ] `--project <dir>` bypasses discovery.
- [ ] A field named `title; DROP TABLE records` is rejected by the name pattern.

## Tests

- Table-driven parse tests: one fixture per error case, asserting the error
  variant and that the message contains the path.
- A round-trip test on `examples/blog/pressa.yaml` — the file the Golden Path
  uses must always load.
- `discover_project` tests over a temp directory tree.

## Open questions

None.
