//! `CollectionService` — SPEC-004 "API" and the unknown-collection criterion.

mod support;

use pressa_app::{AppError, CollectionService};
use support::{POSTS, schema_from};

#[test]
fn all_lists_every_collection_in_schema_order() {
    let service = CollectionService::new(schema_from(POSTS));

    // `map` takes the slug out of each borrowed collection.
    let slugs: Vec<&str> = service
        .all()
        .map(|collection| collection.slug.as_str())
        .collect();

    // File order, because it is the sidebar order (SPEC-001).
    assert_eq!(slugs, vec!["posts", "authors"]);
}

#[test]
fn get_returns_the_collection_for_a_known_slug() {
    let service = CollectionService::new(schema_from(POSTS));

    let posts = service.get("posts").expect("posts exists");

    assert_eq!(posts.slug, "posts");
    assert_eq!(posts.label, "Posts");
    assert_eq!(posts.list_columns, vec!["title", "slug"]);
}

#[test]
fn get_refuses_an_unknown_slug() {
    let service = CollectionService::new(schema_from(POSTS));

    let error = service.get("ghosts").expect_err("no such collection");

    match error {
        AppError::UnknownCollection(slug) => assert_eq!(slug, "ghosts"),
        other => panic!("expected UnknownCollection, got {other:?}"),
    }
}
