//! The SPEC-004 integration criterion: load schema → create → list → get →
//! update → delete, against a real `SqliteRepository` on a temp file.
//!
//! The service tests use `MemoryRepository`; this one exists to prove the same
//! service behaves identically over SQL, and that nothing in the service layer
//! depended on the in-memory adapter.

mod support;

use pressa_app::{AppError, CollectionService, RecordService};
use pressa_core::repository::ListParams;
use pressa_storage::SqliteRepository;
use serde_json::json;
use support::{POSTS, TempTree, schema_from};

#[test]
fn the_whole_record_lifecycle_works_against_sqlite() {
    let tree = TempTree::new("services-sqlite");
    let schema = schema_from(POSTS);
    // Nested, so opening has to create `.pressa/` the way a first run does.
    let repo = SqliteRepository::open(tree.root().join(".pressa").join("data.db"))
        .expect("open the database");
    let collections = CollectionService::new(schema.clone());
    let service = RecordService::new(repo, schema);

    // The collection the UI would have selected from the sidebar.
    assert_eq!(collections.get("posts").expect("posts").label, "Posts");

    // create — from the blank document the editor would have started from.
    let mut data = service.blank("posts").expect("blank");
    data["title"] = json!("First post");
    data["slug"] = json!("first-post");
    data["status"] = json!("draft");
    let created = service.create("posts", data).expect("create");

    // list
    let listed = service.list("posts", &ListParams::default()).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);

    // get
    let fetched = service.get("posts", &created.id).expect("get");
    assert_eq!(fetched.data["title"], json!("First post"));

    // update — its own slug is not a duplicate of itself.
    let mut changed = fetched.data.clone();
    changed["status"] = json!("published");
    let updated = service
        .update("posts", &created.id, changed)
        .expect("update");
    assert_eq!(updated.data["status"], json!("published"));
    assert_eq!(updated.created_at, created.created_at); // created_at is left alone

    // Validation still applies over SQL, and still writes nothing.
    let error = service
        .update("posts", &created.id, json!({ "title": "no slug" }))
        .expect_err("an invalid document is refused");
    assert!(matches!(error, AppError::Validation(_)), "got {error:?}");
    assert_eq!(
        service.get("posts", &created.id).expect("get").data["status"],
        json!("published"),
        "the refused update must not have changed the row"
    );

    // delete
    service.delete("posts", &created.id).expect("delete");
    assert_eq!(
        service
            .count("posts", &ListParams::default())
            .expect("count"),
        0
    );
    let error = service
        .delete("posts", &created.id)
        .expect_err("it is already gone");
    assert!(matches!(error, AppError::NotFound { .. }), "got {error:?}");
}
