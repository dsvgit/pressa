//! `RecordService` — SPEC-004 "Behaviour", "Invariants" and its acceptance
//! criteria. Backed by `MemoryRepository`, so a failure here is never SQL.

mod support;

use pressa_app::{AppError, RecordService};
use pressa_core::record::RecordId;
use pressa_core::repository::ListParams;
use pressa_core::validation::{ErrorCode, FieldError, validate_record};
use pressa_storage::MemoryRepository;
use serde_json::{Value as Json, json};
use support::{CallLog, POSTS, RecordingRepository, schema_from};

/// A service over an empty in-memory repository.
fn service() -> RecordService<MemoryRepository> {
    RecordService::new(MemoryRepository::new(), schema_from(POSTS))
}

/// A service over a repository that stores nothing, plus the log of its calls.
fn recording() -> (RecordService<RecordingRepository>, CallLog) {
    let log = CallLog::default();
    // The log is cloned so the test keeps a handle after the repository moves.
    let service = RecordService::new(RecordingRepository::new(log.clone()), schema_from(POSTS));
    (service, log)
}

/// A document that satisfies every rule in `POSTS`.
fn valid(slug: &str) -> Json {
    json!({
        "title": "Hello",
        "slug": slug,
        "status": "draft",
        "views": 1,
        "featured": false,
        "published_at": null,
        "metadata": null,
    })
}

/// The field errors in an `AppError::Validation`, or a failed assertion.
fn validation_errors(error: AppError) -> Vec<FieldError> {
    match error {
        AppError::Validation(errors) => errors,
        // `other` is printed so a wrong variant says which one it got.
        other => panic!("expected AppError::Validation, got {other:?}"),
    }
}

/// Every `(field, code)` pair in a validation failure, for order-free asserts.
fn codes(errors: &[FieldError]) -> Vec<(&str, ErrorCode)> {
    errors
        .iter()
        // `as_str` borrows each name rather than cloning it.
        .map(|error| (error.field.as_str(), error.code))
        .collect()
}

// ---------------------------------------------------------------------------
// create
// ---------------------------------------------------------------------------

#[test]
fn create_stores_a_valid_document_and_returns_it_with_an_id() {
    let service = service();

    let record = service.create("posts", valid("hello")).expect("create");

    assert_eq!(record.collection, "posts");
    assert_eq!(record.data, valid("hello"));
    // The id is minted by storage and must round-trip as a ULID.
    assert_eq!(record.id.as_string().len(), 26);
    assert_eq!(
        service
            .count("posts", &ListParams::default())
            .expect("count"),
        1
    );
    // The record is readable back under the id it was given.
    assert_eq!(service.get("posts", &record.id).expect("get").id, record.id);
}

#[test]
fn create_rejects_an_invalid_document_and_writes_nothing() {
    let service = service();

    // `title` is required and missing; `views` is a number field given text.
    let error = service
        .create(
            "posts",
            json!({ "slug": "s", "status": "draft", "views": "no" }),
        )
        .expect_err("an invalid document is refused");

    let errors = validation_errors(error);
    assert!(codes(&errors).contains(&("title", ErrorCode::Required)));
    assert!(codes(&errors).contains(&("views", ErrorCode::TypeMismatch)));
    // The invariant: nothing reached the repository.
    assert_eq!(
        service
            .count("posts", &ListParams::default())
            .expect("count"),
        0
    );
}

#[test]
fn create_does_not_call_the_repository_for_an_invalid_document() {
    let (service, log) = recording();

    let error = service.create("posts", json!({})).expect_err("refused");

    validation_errors(error);
    // Not even `find_by_field`: validation fails before uniqueness is checked.
    assert_eq!(log.calls(), Vec::<String>::new());
}

#[test]
fn create_reports_a_duplicate_value_in_a_unique_field() {
    let service = service();
    service
        .create("posts", valid("taken"))
        .expect("first create");

    let error = service
        .create("posts", valid("taken"))
        .expect_err("the duplicate is refused");

    let errors = validation_errors(error);
    assert_eq!(codes(&errors), vec![("slug", ErrorCode::NotUnique)]);
    // The message is the one SPEC-002 fixes for this code.
    assert_eq!(errors[0].message, "already used by another record");
    // And the duplicate was not stored.
    assert_eq!(
        service
            .count("posts", &ListParams::default())
            .expect("count"),
        1
    );
}

#[test]
fn a_second_record_with_no_value_in_a_unique_field_is_allowed() {
    // SPEC-002 "Uniqueness": absent values are not checked, so two authors may
    // both have no `email` even though the field is unique. `find_by_field`
    // treats an absent field as null and would match, so the service has to
    // skip absent values rather than rely on the repository.
    let service = service();

    service
        .create("authors", json!({ "name": "A" }))
        .expect("first");
    service
        .create("authors", json!({ "name": "B", "email": null }))
        .expect("an explicit null does not collide either");

    assert_eq!(
        service
            .count("authors", &ListParams::default())
            .expect("count"),
        2
    );
}

// ---------------------------------------------------------------------------
// update
// ---------------------------------------------------------------------------

#[test]
fn update_does_not_report_a_record_against_its_own_unique_value() {
    let service = service();
    let record = service.create("posts", valid("mine")).expect("create");

    // The same slug, a changed title: the only match is the record itself.
    let mut data = valid("mine");
    data["title"] = json!("Renamed");
    let updated = service
        .update("posts", &record.id, data)
        .expect("its own value is not a duplicate");

    assert_eq!(updated.id, record.id);
    assert_eq!(updated.data["title"], json!("Renamed"));
}

#[test]
fn update_reports_a_unique_value_held_by_another_record() {
    let service = service();
    service.create("posts", valid("first")).expect("first");
    let second = service.create("posts", valid("second")).expect("second");

    let error = service
        .update("posts", &second.id, valid("first"))
        .expect_err("another record holds it");

    assert_eq!(
        codes(&validation_errors(error)),
        vec![("slug", ErrorCode::NotUnique)]
    );
}

#[test]
fn update_of_a_missing_id_is_not_found() {
    let service = service();
    let absent = RecordId::new();

    let error = service
        .update("posts", &absent, valid("hello"))
        .expect_err("there is no such record");

    // Translated to the app-level variant, not passed through as a storage error.
    match error {
        AppError::NotFound { collection, id } => {
            assert_eq!(collection, "posts");
            assert_eq!(id, absent.as_string());
        }
        other => panic!("expected AppError::NotFound, got {other:?}"),
    }
}

#[test]
fn update_rejects_an_invalid_document_without_writing() {
    let service = service();
    let record = service.create("posts", valid("hello")).expect("create");

    let error = service
        .update("posts", &record.id, json!({ "title": "Only" }))
        .expect_err("refused");

    validation_errors(error);
    // The stored document is untouched.
    let stored = service.get("posts", &record.id).expect("get");
    assert_eq!(stored.data, valid("hello"));
}

// ---------------------------------------------------------------------------
// get and delete
// ---------------------------------------------------------------------------

#[test]
fn get_of_a_missing_id_is_not_found() {
    let service = service();

    let error = service
        .get("posts", &RecordId::new())
        .expect_err("there is no such record");

    assert!(matches!(error, AppError::NotFound { .. }), "got {error:?}");
}

#[test]
fn delete_removes_the_record_and_a_second_delete_is_not_found() {
    let service = service();
    let record = service.create("posts", valid("hello")).expect("create");

    service.delete("posts", &record.id).expect("first delete");
    assert_eq!(
        service
            .count("posts", &ListParams::default())
            .expect("count"),
        0
    );

    let error = service
        .delete("posts", &record.id)
        .expect_err("it is already gone");
    assert!(matches!(error, AppError::NotFound { .. }), "got {error:?}");
}

// ---------------------------------------------------------------------------
// Unknown collections
// ---------------------------------------------------------------------------

#[test]
fn every_method_refuses_an_unknown_collection_without_touching_the_repository() {
    let (service, log) = recording();
    let id = RecordId::new();
    let params = ListParams::default();

    // Each entry is one method called with a slug the schema does not have.
    let attempts: Vec<(&str, AppError)> = vec![
        ("list", service.list("ghosts", &params).unwrap_err()),
        ("count", service.count("ghosts", &params).unwrap_err()),
        ("get", service.get("ghosts", &id).unwrap_err()),
        ("create", service.create("ghosts", valid("s")).unwrap_err()),
        (
            "update",
            service.update("ghosts", &id, valid("s")).unwrap_err(),
        ),
        ("delete", service.delete("ghosts", &id).unwrap_err()),
        ("blank", service.blank("ghosts").unwrap_err()),
    ];

    for (method, error) in attempts {
        match error {
            AppError::UnknownCollection(slug) => assert_eq!(slug, "ghosts", "from {method}"),
            other => panic!("{method} gave {other:?}, expected UnknownCollection"),
        }
    }

    // The whole point: the repository was never asked anything.
    assert_eq!(log.calls(), Vec::<String>::new());
}

// ---------------------------------------------------------------------------
// blank
// ---------------------------------------------------------------------------

#[test]
fn blank_gives_every_field_a_key_with_false_for_booleans_and_null_otherwise() {
    let service = service();

    let blank = service.blank("posts").expect("blank");

    assert_eq!(
        blank,
        json!({
            "title": null,
            "slug": null,
            // The required `Select` is left empty on purpose (SPEC-004).
            "status": null,
            "views": null,
            "featured": false,
            "published_at": null,
            "metadata": null,
        })
    );
}

#[test]
fn a_blank_record_fails_validation_with_one_required_per_required_field() {
    let schema = schema_from(POSTS);
    let service = RecordService::new(MemoryRepository::new(), schema.clone());
    let posts = schema.collections.get("posts").expect("posts exists");

    let blank = service.blank("posts").expect("blank");
    let errors = validate_record(posts, &blank).expect_err("a blank record is incomplete");

    // Exactly the three required fields, the `Select` among them, and nothing
    // else: `featured: false` is a real boolean and `views: null` is optional.
    assert_eq!(
        codes(&errors),
        vec![
            ("title", ErrorCode::Required),
            ("slug", ErrorCode::Required),
            ("status", ErrorCode::Required),
        ]
    );
}

// ---------------------------------------------------------------------------
// list and count
// ---------------------------------------------------------------------------

#[test]
fn list_returns_the_collections_records_in_id_order() {
    let service = service();
    let first = service.create("posts", valid("one")).expect("first");
    let second = service.create("posts", valid("two")).expect("second");
    service
        .create("authors", json!({ "name": "A" }))
        .expect("other collection");

    let listed = service.list("posts", &ListParams::default()).expect("list");

    // `map` pulls out just the ids so the assert reads as an order check.
    let ids: Vec<RecordId> = listed.iter().map(|record| record.id).collect();
    assert_eq!(ids, vec![first.id, second.id]);
    assert_eq!(
        service
            .count("posts", &ListParams::default())
            .expect("count"),
        2
    );
}
