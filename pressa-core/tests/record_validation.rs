//! The acceptance criteria of `specs/002-record-validation.md`.
//!
//! An integration test on purpose: it is compiled as its own crate, so it can
//! only reach `pressa-core` through the public API — which is what makes it a
//! contract test rather than a test of the internals.

use pressa_core::schema::{Collection, CollectionCapabilities, Field, FieldType};
use pressa_core::validation::{ErrorCode, FieldError, validate_record};
// Same alias the implementation uses, so the tables below read the same.
use serde_json::{Value as Json, json};

/// Builds a one-field collection: most rules are about a single field.
fn collection_of(fields: Vec<Field>) -> Collection {
    Collection {
        slug: "posts".to_string(),
        label: "Posts".to_string(),
        // `iter()` borrows so `fields` can still be moved into the struct.
        list_columns: fields.iter().map(|f| f.name.clone()).collect(),
        fields,
        capabilities: CollectionCapabilities::default(),
    }
}

/// A field with the boring defaults; tests override what they care about.
fn field(name: &str, kind: FieldType) -> Field {
    Field {
        name: name.to_string(),
        label: name.to_string(),
        kind,
        required: false,
        unique: false,
    }
}

fn required(mut f: Field) -> Field {
    f.required = true;
    f
}

fn select_of(options: &[&str]) -> FieldType {
    // `map` over the slice copies each `&str` into an owned `String`.
    FieldType::Select {
        options: options.iter().map(|o| o.to_string()).collect(),
    }
}

/// The errors from validating a single-field document, or empty for `Ok`.
fn errors_for(kind: FieldType, value: Json) -> Vec<FieldError> {
    let collection = collection_of(vec![field("f", kind)]);
    // `unwrap_or_default` turns `Ok(())` into an empty vector.
    validate_record(&collection, &json!({ "f": value }))
        .err()
        .unwrap_or_default()
}

/// The single expected code, or `None` when the value was accepted.
fn code_for(kind: FieldType, value: Json) -> Option<ErrorCode> {
    let errors = errors_for(kind, value);
    assert!(
        errors.len() <= 1,
        "expected at most one error, got {errors:?}"
    );
    // `first()` borrows the head; `map` copies the `Copy` code out of it.
    errors.first().map(|e| e.code)
}

fn blog_post() -> Collection {
    collection_of(vec![
        required(field("title", FieldType::Text)),
        field("body", FieldType::Textarea),
        field("views", FieldType::Number),
        field("draft", FieldType::Boolean),
        field("published_at", FieldType::DateTime),
        field("status", select_of(&["draft", "published"])),
        field("meta", FieldType::Json),
    ])
}

// ---- criterion: a valid document returns Ok(()) ----

#[test]
fn a_valid_document_is_accepted() {
    let data = json!({
        "title": "Hello",
        "body": "Some text",
        "views": 12,
        "draft": true,
        "published_at": "2026-09-06T12:00:00Z",
        "status": "draft",
        "meta": { "tags": ["a", "b"] },
    });
    assert_eq!(validate_record(&blog_post(), &data), Ok(()));
}

// ---- criterion: every row of the type table produces its code ----

#[test]
fn accepted_values_are_valid() {
    let accepted = vec![
        (FieldType::Text, json!("hello")),
        (FieldType::Textarea, json!("a\nb")),
        (FieldType::Number, json!(42)),
        (FieldType::Number, json!(1.5)),
        (FieldType::Boolean, json!(true)),
        (FieldType::Boolean, json!(false)),
        (FieldType::DateTime, json!("2026-09-06T12:00:00Z")),
        (select_of(&["draft", "published"]), json!("published")),
        (FieldType::Json, json!({ "any": [1, 2] })),
        (FieldType::Json, json!("a bare string is valid JSON")),
        (FieldType::Json, json!(7)),
    ];
    for (kind, value) in accepted {
        assert_eq!(
            code_for(kind.clone(), value.clone()),
            None,
            "{kind:?} should accept {value}"
        );
    }
}

#[test]
fn rejected_values_produce_their_code() {
    let rejected = vec![
        (FieldType::Text, json!(1), ErrorCode::TypeMismatch),
        (FieldType::Textarea, json!(true), ErrorCode::TypeMismatch),
        (FieldType::Number, json!("42"), ErrorCode::TypeMismatch),
        (FieldType::Number, json!(true), ErrorCode::TypeMismatch),
        (FieldType::Boolean, json!(0), ErrorCode::TypeMismatch),
        (FieldType::Boolean, json!(1), ErrorCode::TypeMismatch),
        (FieldType::Boolean, json!("true"), ErrorCode::TypeMismatch),
        (
            FieldType::DateTime,
            json!(20260906),
            ErrorCode::TypeMismatch,
        ),
        (
            FieldType::DateTime,
            json!("not a date"),
            ErrorCode::InvalidDateTime,
        ),
        (select_of(&["draft"]), json!(3), ErrorCode::TypeMismatch),
        (
            select_of(&["draft"]),
            json!("archived"),
            ErrorCode::NotInOptions,
        ),
    ];
    for (kind, value, expected) in rejected {
        assert_eq!(
            code_for(kind.clone(), value.clone()),
            Some(expected),
            "{kind:?} should reject {value} as {expected:?}"
        );
    }
}

// ---- criterion: required and absence ----

#[test]
fn a_missing_required_field_is_required() {
    let collection = collection_of(vec![required(field("title", FieldType::Text))]);
    let errors = validate_record(&collection, &json!({})).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].field, "title");
    assert_eq!(errors[0].code, ErrorCode::Required);
    assert_eq!(errors[0].message, "required");
}

#[test]
fn an_empty_string_in_a_required_field_is_required() {
    let collection = collection_of(vec![required(field("title", FieldType::Text))]);
    let errors = validate_record(&collection, &json!({ "title": "" })).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, ErrorCode::Required);
}

#[test]
fn an_explicit_null_in_a_required_field_is_required() {
    let collection = collection_of(vec![required(field("title", FieldType::Text))]);
    let errors = validate_record(&collection, &json!({ "title": null })).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, ErrorCode::Required);
}

#[test]
fn a_missing_optional_field_is_valid() {
    let collection = collection_of(vec![field("body", FieldType::Textarea)]);
    assert_eq!(validate_record(&collection, &json!({})), Ok(()));
    // An explicit null is the same as absence, so it is valid too.
    assert_eq!(
        validate_record(&collection, &json!({ "body": null })),
        Ok(())
    );
}

// ---- criterion: unknown keys ----

#[test]
fn an_unknown_key_is_rejected_by_name() {
    let collection = collection_of(vec![field("title", FieldType::Text)]);
    let data = json!({ "title": "Hello", "slug": "hello" });
    let errors = validate_record(&collection, &data).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].field, "slug");
    assert_eq!(errors[0].code, ErrorCode::UnknownField);
}

// ---- criterion: all violations at once, in schema field order ----

#[test]
fn three_broken_fields_yield_three_errors_in_schema_field_order() {
    let data = json!({
        "title": "",
        "body": "fine",
        "views": "many",
        "draft": true,
        "published_at": "2026-09-06T12:00:00Z",
        "status": "archived",
        "meta": {},
    });
    let errors = validate_record(&blog_post(), &data).unwrap_err();
    // `iter()` borrows each error so the vector is not consumed here.
    let seen: Vec<(&str, ErrorCode)> = errors.iter().map(|e| (e.field.as_str(), e.code)).collect();
    assert_eq!(
        seen,
        vec![
            ("title", ErrorCode::Required),
            ("views", ErrorCode::TypeMismatch),
            ("status", ErrorCode::NotInOptions),
        ]
    );
}

#[test]
fn schema_field_errors_come_before_unknown_keys() {
    let collection = collection_of(vec![
        required(field("title", FieldType::Text)),
        field("views", FieldType::Number),
    ]);
    let data = json!({ "views": "many", "zzz_unknown": 1 });
    let errors = validate_record(&collection, &data).unwrap_err();
    let seen: Vec<(&str, ErrorCode)> = errors.iter().map(|e| (e.field.as_str(), e.code)).collect();
    assert_eq!(
        seen,
        vec![
            ("title", ErrorCode::Required),
            ("views", ErrorCode::TypeMismatch),
            ("zzz_unknown", ErrorCode::UnknownField),
        ]
    );
}

// ---- criterion: NotUnique is never produced here ----

#[test]
fn validate_record_never_reports_not_unique() {
    let mut unique_title = required(field("title", FieldType::Text));
    unique_title.unique = true;
    let collection = collection_of(vec![unique_title]);

    // A valid value on a unique field is still just valid.
    assert_eq!(
        validate_record(&collection, &json!({ "title": "Hello" })),
        Ok(())
    );
    // And a broken one reports its own problem, never NotUnique.
    let errors = validate_record(&collection, &json!({ "title": 1 })).unwrap_err();
    assert!(errors.iter().all(|e| e.code != ErrorCode::NotUnique));
}

// ---- criterion: the document itself must be an object ----

#[test]
fn a_non_object_document_is_a_single_type_mismatch() {
    for data in [json!([1, 2, 3]), json!("a string"), json!(7), json!(null)] {
        let errors = validate_record(&blog_post(), &data).unwrap_err();
        assert_eq!(errors.len(), 1, "{data} should give exactly one error");
        assert_eq!(errors[0].field, "");
        assert_eq!(errors[0].code, ErrorCode::TypeMismatch);
    }
}

// ---- criterion: numeric strings are not coerced ----

#[test]
fn numeric_strings_are_not_coerced_into_numbers() {
    assert_eq!(
        code_for(FieldType::Number, json!("42")),
        Some(ErrorCode::TypeMismatch)
    );
}

// ---- criterion: unparseable dates are InvalidDateTime, not TypeMismatch ----

#[test]
fn an_unparseable_date_is_invalid_datetime_not_type_mismatch() {
    assert_eq!(
        code_for(FieldType::DateTime, json!("2026-13-45")),
        Some(ErrorCode::InvalidDateTime)
    );
    // A non-string is the other branch: a type problem, not a parse one.
    assert_eq!(
        code_for(FieldType::DateTime, json!(false)),
        Some(ErrorCode::TypeMismatch)
    );
}

// ---- the message table from the spec ----

#[test]
fn messages_match_the_spec() {
    let cases = vec![
        (FieldType::Text, json!(1), "must be text"),
        (FieldType::Number, json!("42"), "must be a number"),
        (FieldType::Boolean, json!(1), "must be true or false"),
        (FieldType::DateTime, json!(1), "must be a date"),
        (
            FieldType::DateTime,
            json!("nope"),
            "must be a date like 2026-09-06T12:00:00Z",
        ),
        (
            select_of(&["draft", "published"]),
            json!("archived"),
            "must be one of: draft, published",
        ),
    ];
    for (kind, value, expected) in cases {
        let errors = errors_for(kind.clone(), value.clone());
        assert_eq!(errors.len(), 1, "{kind:?} / {value}");
        assert_eq!(errors[0].message, expected, "{kind:?} / {value}");
    }
}
