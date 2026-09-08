//! Record validation: does this JSON document match its collection's schema?
//!
//! Records are stored as JSON with no database-level typing
//! ([ADR-0003](../../docs/adr/0003-json-documents-not-eav.md)), so this module
//! is the only guarantee that stored data is renderable. The contract is
//! `specs/002-record-validation.md`.

use chrono::DateTime;
use serde_json::Value as Json;

use crate::schema::{Collection, Field, FieldType};

/// One violation, addressed to the field it belongs to so the editor can
/// render it under that input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    /// Field name, or `""` when the document itself is not an object.
    pub field: String,
    pub code: ErrorCode,
    /// Human-readable, lowercase, without the field name.
    pub message: String,
}

/// What tests assert on. The prose in `message` is for humans only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    Required,
    TypeMismatch,
    NotInOptions,
    UnknownField,
    /// Never produced here — it needs storage. See `specs/004`.
    NotUnique,
    InvalidDateTime,
    InvalidJson,
}

impl FieldError {
    /// Small constructor so the call sites below stay one line each.
    fn new(field: &str, code: ErrorCode, message: &str) -> Self {
        FieldError {
            field: field.to_string(),
            code,
            message: message.to_string(),
        }
    }
}

/// Validates `data` against `collection`, returning **every** violation.
///
/// All of them, not the first: the editor renders each message under its own
/// field, and failing one at a time would make fixing a form a sequence of
/// save attempts. Errors come in schema field order, then unknown keys.
///
/// `NotUnique` is never produced here — it needs storage, so `RecordService`
/// appends it (`specs/004`).
pub fn validate_record(collection: &Collection, data: &Json) -> Result<(), Vec<FieldError>> {
    // `as_object` borrows the map; `let ... else` bails out when it is not one.
    let Some(object) = data.as_object() else {
        return Err(vec![FieldError::new(
            "",
            ErrorCode::TypeMismatch,
            "must be a JSON object",
        )]);
    };

    let mut errors = Vec::new();

    // Schema order first, because that is the order the form renders in.
    for field in &collection.fields {
        // `get` returns `Option<&Json>`: `None` is a key that is not there.
        if let Some(error) = check_field(field, object.get(&field.name)) {
            errors.push(error);
        }
    }

    // Then keys the schema has no field for. Rejected rather than dropped:
    // silently discarding supplied data is worse than refusing it.
    for key in object.keys() {
        // `any` short-circuits on the first field with a matching name.
        if !collection.fields.iter().any(|field| &field.name == key) {
            errors.push(FieldError::new(
                key,
                ErrorCode::UnknownField,
                "unknown field",
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Checks one field. `value` is `None` when the key is missing entirely.
fn check_field(field: &Field, value: Option<&Json>) -> Option<FieldError> {
    // A field is absent if the key is missing *or* its value is null.
    let present = match value {
        Some(Json::Null) | None => None,
        Some(value) => Some(value),
    };

    // An empty string is a user leaving the box blank, not a value.
    let blank = matches!(present, Some(Json::String(text)) if text.is_empty());

    if field.required && (present.is_none() || blank) {
        return Some(FieldError::new(
            &field.name,
            ErrorCode::Required,
            "required",
        ));
    }

    // Absence of a non-required field is valid and means "no value".
    let value = present?;

    // `map` rebuilds the (code, message) pair into an error for this field.
    check_type(&field.kind, value)
        .map(|(code, message)| FieldError::new(&field.name, code, &message))
}

/// The per-type table from `specs/002`. `None` means the value is acceptable.
fn check_type(kind: &FieldType, value: &Json) -> Option<(ErrorCode, String)> {
    let type_mismatch = |message: &str| Some((ErrorCode::TypeMismatch, message.to_string()));

    match kind {
        // Numeric strings are rejected on purpose: coercing here would make the
        // same document validate differently depending on where it came from.
        FieldType::Text | FieldType::Textarea => match value {
            Json::String(_) => None,
            _ => type_mismatch("must be text"),
        },
        FieldType::Number => match value {
            Json::Number(_) => None,
            _ => type_mismatch("must be a number"),
        },
        FieldType::Boolean => match value {
            Json::Bool(_) => None,
            _ => type_mismatch("must be true or false"),
        },
        FieldType::DateTime => match value {
            // A string that will not parse is a different fault from a number.
            Json::String(text) => match DateTime::parse_from_rfc3339(text) {
                Ok(_) => None,
                Err(_) => Some((
                    ErrorCode::InvalidDateTime,
                    "must be a date like 2026-09-06T12:00:00Z".to_string(),
                )),
            },
            _ => type_mismatch("must be a date"),
        },
        FieldType::Select { options } => match value {
            // `contains` needs a `&String`, which is what `iter().any` avoids.
            Json::String(text) if options.iter().any(|option| option == text) => None,
            Json::String(_) => Some((
                ErrorCode::NotInOptions,
                format!("must be one of: {}", options.join(", ")),
            )),
            _ => type_mismatch("must be text"),
        },
        // Anything that parsed as JSON is already a valid JSON value.
        FieldType::Json => None,
    }
}
