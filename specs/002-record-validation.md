# SPEC-002: Record validation

Status: **Draft** · Task: T3 · Crate: `pressa-core`
· ADR: [0003](../docs/adr/0003-json-documents-not-eav.md)

## Problem

Records are stored as JSON with no database-level typing
([ADR-0003](../docs/adr/0003-json-documents-not-eav.md)). The validator is
therefore the *only* guarantee that stored data matches the schema. If it is
wrong or incomplete, the database silently accumulates records the UI cannot
render.

## Goal

Given a `Collection` and a `serde_json::Value`, return every violation as a
structured, field-addressable error — or confirm the document is storable.

## Non-goals

`min`/`max`, regex constraints, cross-field rules, custom validators. Each is a
schema feature that does not exist yet; adding validation for it now would be
validating a field that cannot be declared.

## API

```rust
pub fn validate_record(
    collection: &Collection,
    data: &serde_json::Value,
) -> Result<(), Vec<FieldError>>;

pub struct FieldError { pub field: String, pub code: ErrorCode, pub message: String }

pub enum ErrorCode {
    Required, TypeMismatch, NotInOptions, UnknownField,
    NotUnique, InvalidDateTime, InvalidJson,
}
```

Returns **all** violations, not the first. The editor renders every message
under its field at once; failing one at a time would make fixing a form a
sequence of save attempts.

## Rules per field type

`data` must be a JSON object; anything else is a single `TypeMismatch` on the
field `""`.

A field is **absent** if the key is missing or its value is `null`. Absence of a
non-required field is valid and means "no value".

| Type | Accepted JSON | Rejected → code |
|---|---|---|
| `Text`, `Textarea` | string | anything else → `TypeMismatch` |
| `Number` | number | strings, including numeric ones → `TypeMismatch` |
| `Boolean` | `true` / `false` | `0`, `1`, `"true"` → `TypeMismatch` |
| `DateTime` | RFC 3339 string | non-string → `TypeMismatch`; unparseable string → `InvalidDateTime` |
| `Select` | string present in `options` | non-string → `TypeMismatch`; unlisted string → `NotInOptions` |
| `Json` | any valid JSON value | — (a value that got this far is already valid JSON; `InvalidJson` is produced by the editor when parsing user text) |

Numeric strings are rejected deliberately. Coercion here would mean the same
document validates differently depending on where it came from, and the editor
is the right place to parse `"42"` into `42`.

## Required and unknown

- `required: true` and the field is absent → `Required`, message `"required"`.
- `required: true` and the value is an empty string → `Required`. An empty
  string is a user leaving the box blank, not a value.
- A key in `data` with no matching field → `UnknownField` on that key.
  Rejecting rather than dropping it: silently discarding data the user or a
  script supplied is worse than refusing it.

## Uniqueness

`unique: true` cannot be checked in `pressa-core` — it needs storage. The split:

- `validate_record` never emits `NotUnique`.
- `RecordService::create` / `update` call `validate_record` first, then for each
  unique field call `repo.find_by_field`, and append `NotUnique` errors for any
  match whose id differs from the record being saved.
- Absent values are not checked for uniqueness — two records may both have no
  slug.

This keeps `pressa-core` free of I/O while keeping the rule enforceable.
See [`specs/004`](004-app-services.md).

## Error messages

Human-readable, lowercase, no field name (the UI already places the message
under its field):

| Code | Message |
|---|---|
| `Required` | `required` |
| `TypeMismatch` | `must be a number` / `must be text` / `must be true or false` / `must be a date` |
| `NotInOptions` | `must be one of: draft, published` |
| `UnknownField` | `unknown field` |
| `NotUnique` | `already used by another record` |
| `InvalidDateTime` | `must be a date like 2026-09-06T12:00:00Z` |
| `InvalidJson` | `not valid JSON` |

## Acceptance criteria

- [ ] A valid document returns `Ok(())`.
- [ ] Every row of the type table produces its code for a rejecting input.
- [ ] A missing required field yields `Required`.
- [ ] An empty string in a required text field yields `Required`.
- [ ] An explicit `null` in a required field yields `Required`.
- [ ] A missing optional field is valid.
- [ ] An unknown key yields `UnknownField` naming that key.
- [ ] Three broken fields yield three errors in one call, in schema field order.
- [ ] `validate_record` never returns `NotUnique`.
- [ ] `data` that is an array or a string yields one `TypeMismatch`.
- [ ] `"42"` in a `Number` field is rejected.
- [ ] `"2026-13-45"` in a `DateTime` field yields `InvalidDateTime`, not `TypeMismatch`.

## Tests

Table-driven over `(field_type, json_value) -> expected code`, one case per row
above, plus multi-error ordering tests. No I/O — this whole spec is testable
with literals.

## Open questions

None.
