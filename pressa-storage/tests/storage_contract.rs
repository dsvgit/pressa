//! The acceptance criteria of `specs/003-storage-repository.md`, as **one**
//! suite run against both adapters.
//!
//! Every check below is generic over `impl RecordRepository` and instantiated
//! twice by `contract_suite!`. That is the whole point of the file: if
//! `MemoryRepository` and `SqliteRepository` ever diverge — in ordering, in
//! errors, in anything observable through the port — one of the two modules
//! goes red. New storage behaviour is added here, never to one adapter's own
//! tests.

mod support;

use std::thread::sleep;
use std::time::Duration;

use pressa_core::record::{Record, RecordId};
use pressa_core::repository::{ListParams, RecordRepository, SortDirection, StorageError};
use pressa_storage::{MemoryRepository, SqliteRepository};
use serde_json::{Value as Json, json};
use support::TempTree;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Long enough that the next `now()` is a different instant, so a test can
/// assert `updated_at` actually moved.
const A_TICK: Duration = Duration::from_millis(2);

fn create<R: RecordRepository>(repo: &R, collection: &str, data: Json) -> Record {
    repo.create(collection, data).expect("create")
}

/// The `title` of each record, in the order they were listed.
fn titles(records: &[Record]) -> Vec<String> {
    records
        .iter()
        // `and_then` flattens "no such key" and "not a string" into one `None`.
        .map(|record| {
            record
                .data
                .get("title")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string()
        })
        .collect()
}

fn ids(records: &[Record]) -> Vec<String> {
    records.iter().map(|record| record.id.as_string()).collect()
}

/// Sorts by `field`, leaving everything else default.
///
/// A test helper rather than a constructor on `ListParams`: the spec asks for
/// the struct and its `Default`, and test ergonomics is no reason to widen
/// `pressa-core`'s public API.
fn sorted_by(field: &str, direction: SortDirection) -> ListParams {
    ListParams {
        sort_by: Some(field.to_string()),
        sort_direction: direction,
        ..ListParams::default()
    }
}

/// Searches for `term` across `fields`, leaving everything else default.
fn searching(term: &str, fields: &[&str]) -> ListParams {
    ListParams {
        search: Some(term.to_string()),
        // `map` copies each `&str` into an owned `String` the struct can keep.
        search_fields: fields.iter().map(|field| field.to_string()).collect(),
        ..ListParams::default()
    }
}

/// Creates one record per title, in order, so ids ascend with the arguments.
fn create_titled<R: RecordRepository>(repo: &R, collection: &str, titles: &[&str]) -> Vec<Record> {
    titles
        .iter()
        .map(|title| create(repo, collection, json!({ "title": title })))
        .collect()
}

/// The id named by a `Corrupt`, or a panic naming what came back instead.
fn corrupt_id(error: StorageError) -> String {
    match error {
        StorageError::Corrupt { id, .. } => id,
        other => panic!("expected Corrupt, got {other:?}"),
    }
}

/// The field named by an `InvalidField`, or a panic naming what came instead.
fn invalid_field(error: StorageError) -> String {
    match error {
        StorageError::InvalidField { field } => field,
        other => panic!("expected InvalidField, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Create, get
// ---------------------------------------------------------------------------

/// Criterion: create → get returns an equal record.
fn check_create_then_get_are_equal<R: RecordRepository>(repo: &R) {
    let created = create(repo, "posts", json!({ "title": "Hello", "n": 1 }));

    // The outer `expect` is the storage error, the inner one is "no such row".
    let fetched = repo
        .get("posts", &created.id)
        .expect("get")
        .expect("the record just created is there");

    assert_eq!(created, fetched);
}

/// Criterion: create assigns a ULID and equal `created_at` / `updated_at`.
fn check_create_assigns_a_ulid_and_equal_timestamps<R: RecordRepository>(repo: &R) {
    let created = create(repo, "posts", json!({ "title": "Hello" }));

    assert_eq!(created.collection, "posts");
    assert_eq!(created.id.as_string().len(), 26, "a ULID is 26 characters");
    assert!(
        created.id.as_string().parse::<RecordId>().is_ok(),
        "the assigned id parses back as a ULID"
    );
    assert_eq!(
        created.created_at, created.updated_at,
        "a record that has never been edited was last touched when it was made"
    );
}

/// Behaviour: `create` stores `data` verbatim and does not validate.
fn check_create_does_not_validate<R: RecordRepository>(repo: &R) {
    // A document the schema would reject: storage still takes it, because a
    // repository that validates cannot be used to repair bad data.
    let created = create(repo, "posts", json!({ "no_such_field": true }));
    assert_eq!(created.data, json!({ "no_such_field": true }));
}

/// `get` on an id that was never issued is absence, not an error.
fn check_get_on_a_missing_id_is_none<R: RecordRepository>(repo: &R) {
    let absent = repo.get("posts", &RecordId::new()).expect("get");
    assert_eq!(absent, None);
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

/// Criterion: update changes `data` and `updated_at`, preserves `created_at`
/// and `id`.
fn check_update_replaces_data_and_moves_updated_at<R: RecordRepository>(repo: &R) {
    let created = create(repo, "posts", json!({ "title": "before", "extra": 1 }));
    sleep(A_TICK); // so `updated_at > created_at` is not a coin flip

    let updated = repo
        .update("posts", &created.id, json!({ "title": "after" }))
        .expect("update");

    assert_eq!(updated.id, created.id, "the id is not reassigned");
    assert_eq!(
        updated.created_at, created.created_at,
        "created_at is when it was created, forever"
    );
    assert!(
        updated.updated_at > created.updated_at,
        "updated_at moved: {} is not after {}",
        updated.updated_at,
        created.updated_at
    );
    assert_eq!(
        updated.data,
        json!({ "title": "after" }),
        "data is replaced wholesale, so `extra` is gone rather than merged"
    );

    // And all of that is what was written, not just what was returned.
    let fetched = repo
        .get("posts", &created.id)
        .expect("get")
        .expect("present");
    assert_eq!(fetched, updated);
}

/// Criterion: update on a missing id → `NotFound`.
fn check_update_on_a_missing_id_is_not_found<R: RecordRepository>(repo: &R) {
    let missing = RecordId::new();

    let error = repo
        .update("posts", &missing, json!({ "title": "x" }))
        .expect_err("updating nothing is an error, never a silent success");

    assert_eq!(
        error,
        StorageError::NotFound {
            collection: "posts".to_string(),
            id: missing.as_string(),
        }
    );
}

/// A record of another collection is not reachable under the wrong slug.
fn check_update_will_not_cross_collections<R: RecordRepository>(repo: &R) {
    let page = create(repo, "pages", json!({ "title": "a page" }));

    let error = repo
        .update("posts", &page.id, json!({ "title": "x" }))
        .expect_err("that id does not belong to `posts`");

    assert_eq!(error, StorageError::not_found("posts", &page.id));
    // The real record is untouched.
    let fetched = repo.get("pages", &page.id).expect("get").expect("present");
    assert_eq!(fetched.data, json!({ "title": "a page" }));
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

/// Criterion: delete removes the record; get then returns `Ok(None)`.
fn check_delete_removes_the_record<R: RecordRepository>(repo: &R) {
    let created = create(repo, "posts", json!({ "title": "doomed" }));

    repo.delete("posts", &created.id).expect("delete");

    assert_eq!(repo.get("posts", &created.id).expect("get"), None);
    assert_eq!(
        repo.count("posts", &ListParams::default()).expect("count"),
        0
    );
}

/// Criterion: delete on a missing id → `NotFound`.
fn check_delete_on_a_missing_id_is_not_found<R: RecordRepository>(repo: &R) {
    let missing = RecordId::new();

    let error = repo
        .delete("posts", &missing)
        .expect_err("deleting nothing is an error");

    assert_eq!(error, StorageError::not_found("posts", &missing));
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

/// Criterion: list returns only the requested collection.
fn check_list_returns_only_the_requested_collection<R: RecordRepository>(repo: &R) {
    let post = create(repo, "posts", json!({ "title": "a post" }));
    create(repo, "pages", json!({ "title": "a page" }));
    create(repo, "pages", json!({ "title": "another page" }));

    let listed = repo.list("posts", &ListParams::default()).expect("list");

    assert_eq!(ids(&listed), vec![post.id.as_string()]);
    assert_eq!(
        repo.count("posts", &ListParams::default()).expect("count"),
        1
    );
    assert_eq!(
        repo.count("pages", &ListParams::default()).expect("count"),
        2
    );
    // A collection nobody has written to is empty, not an error.
    assert!(
        repo.list("ghosts", &ListParams::default())
            .expect("list")
            .is_empty()
    );
}

/// Behaviour: the default order is `id` ascending, which is chronological.
fn check_the_default_order_is_chronological<R: RecordRepository>(repo: &R) {
    let created = create_titled(repo, "posts", &["first", "second", "third"]);

    let listed = repo.list("posts", &ListParams::default()).expect("list");

    assert_eq!(titles(&listed), vec!["first", "second", "third"]);
    assert_eq!(ids(&listed), ids(&created));
    // Which is to say: sorted by id.
    let mut sorted = ids(&listed);
    sorted.sort();
    assert_eq!(ids(&listed), sorted);
}

/// Criterion: list honours `limit` and `offset`.
fn check_list_honours_limit_and_offset<R: RecordRepository>(repo: &R) {
    create_titled(repo, "posts", &["t0", "t1", "t2", "t3", "t4"]);

    let page = |limit, offset| {
        let params = ListParams {
            limit,
            offset,
            ..ListParams::default()
        };
        titles(&repo.list("posts", &params).expect("list"))
    };

    assert_eq!(page(Some(2), None), vec!["t0", "t1"], "limit alone");
    assert_eq!(
        page(Some(2), Some(1)),
        vec!["t1", "t2"],
        "limit with offset"
    );
    assert_eq!(
        page(None, Some(3)),
        vec!["t3", "t4"],
        "offset alone still reaches the end"
    );
    assert!(page(Some(2), Some(9)).is_empty(), "offset past the end");
    assert_eq!(
        page(Some(99), None).len(),
        5,
        "a limit larger than the data"
    );

    // `count` answers "how many are there", so paging does not change it.
    let params = ListParams {
        limit: Some(2),
        offset: Some(1),
        ..ListParams::default()
    };
    assert_eq!(repo.count("posts", &params).expect("count"), 5);
}

/// Criterion: sort by a field, ascending and descending, is correct.
fn check_sort_by_a_field_runs_both_ways<R: RecordRepository>(repo: &R) {
    create_titled(repo, "posts", &["banana", "apple", "cherry"]);

    let ascending = repo
        .list("posts", &sorted_by("title", SortDirection::Asc))
        .expect("list");
    assert_eq!(titles(&ascending), vec!["apple", "banana", "cherry"]);

    let descending = repo
        .list("posts", &sorted_by("title", SortDirection::Desc))
        .expect("list");
    assert_eq!(titles(&descending), vec!["cherry", "banana", "apple"]);
}

/// Criterion: sorting is stable across repeated calls with equal keys.
///
/// The keys are deliberately *mixed* rather than all identical. With one repeated
/// key, both a quicksort and SQLite's sorter happen to leave the input order
/// alone, so the test would pass against an implementation that has no
/// tiebreaker at all. Interleaving groups makes the sort genuinely permute, and
/// then only the trailing `id ASC` keeps each group internally ordered.
fn check_sort_is_stable_when_keys_are_equal<R: RecordRepository>(repo: &R) {
    // "c", "a", "b", "c", "a", "b", … — 20 of each, out of order on the way in.
    let keys: Vec<&str> = (0..60).map(|n| ["c", "a", "b"][n % 3]).collect();
    let created = create_titled(repo, "posts", &keys);

    // What a *stable* sort by title of the created order would give.
    let expected = |direction: SortDirection| {
        let mut ordered = created.clone();
        ordered.sort_by(|left, right| {
            let by_title =
                titles(std::slice::from_ref(left)).cmp(&titles(std::slice::from_ref(right)));
            match direction {
                SortDirection::Asc => by_title,
                SortDirection::Desc => by_title.reverse(),
            }
        });
        ids(&ordered)
    };

    for direction in [SortDirection::Asc, SortDirection::Desc] {
        let params = sorted_by("title", direction);
        let first = repo.list("posts", &params).expect("list");
        let again = repo.list("posts", &params).expect("list");

        assert_eq!(
            ids(&first),
            ids(&again),
            "{direction:?}: two identical calls must not reshuffle equal keys"
        );
        assert_eq!(
            ids(&first),
            expected(direction),
            "{direction:?}: within each group of equal titles, ids must ascend"
        );
    }
}

/// Sorting by a field some records do not have still yields a total order.
fn check_sort_tolerates_a_missing_field<R: RecordRepository>(repo: &R) {
    let with = create(repo, "posts", json!({ "title": "has one", "rank": 2 }));
    let without = create(repo, "posts", json!({ "title": "has none" }));

    let ascending = repo
        .list("posts", &sorted_by("rank", SortDirection::Asc))
        .expect("list");

    // Absent sorts before present, the way SQL orders NULL first ascending.
    assert_eq!(
        ids(&ascending),
        vec![without.id.as_string(), with.id.as_string()]
    );
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// Criterion: search matches case-insensitively across the named fields only.
fn check_search_is_case_insensitive_over_the_named_fields<R: RecordRepository>(repo: &R) {
    let in_title = create(
        repo,
        "posts",
        json!({ "title": "Hello World", "body": "quiet" }),
    );
    let in_body = create(
        repo,
        "posts",
        json!({ "title": "Goodbye", "body": "HELLO again" }),
    );
    create(repo, "posts", json!({ "title": "Nothing", "body": "here" }));

    let search = |term: &str, fields: &[&str]| {
        let params = searching(term, fields);
        let found = repo.list("posts", &params).expect("list");
        // `count` must agree with `list`, or paging a search would lie.
        assert_eq!(
            repo.count("posts", &params).expect("count"),
            found.len() as u64,
            "count and list disagree about `{term}`"
        );
        ids(&found)
    };

    assert_eq!(
        search("hello", &["title"]),
        vec![in_title.id.as_string()],
        "lowercase term matches a capitalised value"
    );
    assert_eq!(
        search("HELLO", &["title"]),
        vec![in_title.id.as_string()],
        "and the other way round"
    );
    assert_eq!(
        search("hello", &["title", "body"]),
        vec![in_title.id.as_string(), in_body.id.as_string()],
        "both fields are searched when both are named"
    );
    assert_eq!(
        search("hello", &["body"]),
        vec![in_body.id.as_string()],
        "a field that is not named is not searched"
    );
    assert!(
        search("nowhere", &["title", "body"]).is_empty(),
        "no match is an empty list, not an error"
    );
    // A substring, not a prefix and not a whole-value match.
    assert_eq!(
        search("orl", &["title"]),
        vec![in_title.id.as_string()],
        "matching is substring"
    );
}

/// Searching a number field matches the number *as text*, and both adapters
/// must render it the same way — SQLite's way, since it is the one that cannot
/// be changed.
///
/// `number` is an M0 field type, so this is reachable: without it, searching
/// `1.0` found the record against the file and missed it against memory, which
/// is precisely the "passes against memory, breaks in production" failure the
/// spec's Problem section is about.
fn check_search_renders_numbers_the_way_sqlite_does<R: RecordRepository>(repo: &R) {
    // Each case gets its own collection, so one case's digits can never satisfy
    // another's search.
    let finds = |case: &str, value: Json, term: &str| {
        let record = create(repo, case, json!({ "rank": value }));
        let found = repo.list(case, &searching(term, &["rank"])).expect("list");
        ids(&found) == vec![record.id.as_string()]
    };

    assert!(
        finds("whole", json!(7), "7"),
        "an integer renders as itself"
    );
    assert!(
        !finds("whole_with_a_point", json!(7), "7.0"),
        "an integer has no decimal point to match"
    );
    assert!(
        finds("real", json!(1.0), "1.0"),
        "a real keeps its point, the way SQLite prints it"
    );
    assert!(finds("fraction", json!(2.5), "2.5"));
    assert!(
        finds("huge", json!(1e20), "1.0e+20"),
        "a large real renders in exponent form, not as twenty-one digits"
    );
    assert!(
        finds("tiny", json!(1e-7), "1.0e-07"),
        "and a small one likewise, with a two-digit exponent"
    );
    assert!(
        finds(
            "past_the_double",
            json!(9_007_199_254_740_993_i64),
            "9007199254740993"
        ),
        "an integer past 2^53 keeps every digit rather than rounding"
    );
    assert!(
        finds("flag", json!(true), "1"),
        "a boolean searches as the number SQLite makes of it"
    );
}

/// `%` and `_` are characters the user typed, not wildcards.
fn check_search_treats_wildcards_as_literals<R: RecordRepository>(repo: &R) {
    let literal = create(repo, "posts", json!({ "title": "100% done" }));
    create(repo, "posts", json!({ "title": "nothing here" }));

    let params = searching("0% d", &["title"]);
    let found = repo.list("posts", &params).expect("list");
    assert_eq!(ids(&found), vec![literal.id.as_string()]);

    // If `%` were a wildcard this would match everything; it must match nothing.
    let params = searching("%nothing", &["title"]);
    assert!(repo.list("posts", &params).expect("list").is_empty());
}

// ---------------------------------------------------------------------------
// find_by_field
// ---------------------------------------------------------------------------

/// Criterion: `find_by_field` returns every match and an empty vector for none.
fn check_find_by_field_returns_every_match<R: RecordRepository>(repo: &R) {
    let first = create(repo, "posts", json!({ "title": "a", "status": "draft" }));
    create(repo, "posts", json!({ "title": "b", "status": "live" }));
    let third = create(repo, "posts", json!({ "title": "c", "status": "draft" }));
    // Same field value, different collection: must not be returned.
    create(repo, "pages", json!({ "title": "d", "status": "draft" }));

    let found = repo
        .find_by_field("posts", "status", &json!("draft"))
        .expect("find_by_field");
    assert_eq!(
        ids(&found),
        vec![first.id.as_string(), third.id.as_string()],
        "every match, in id order"
    );

    let none = repo
        .find_by_field("posts", "status", &json!("archived"))
        .expect("find_by_field");
    assert!(none.is_empty(), "no match is an empty vector, not an error");

    // Non-string values compare too — uniqueness applies to numbers as well.
    let numbered = create(repo, "posts", json!({ "title": "e", "rank": 7 }));
    let found = repo
        .find_by_field("posts", "rank", &json!(7))
        .expect("find_by_field");
    assert_eq!(ids(&found), vec![numbered.id.as_string()]);
}

// ---------------------------------------------------------------------------
// Corruption and hostile field names
// ---------------------------------------------------------------------------

/// Criterion: a corrupt row yields `Corrupt` naming the id.
fn check_a_corrupt_row_is_reported_with_its_id<R: RecordRepository>(repo: &R) {
    // `create` stores verbatim and does not validate, which is how a row whose
    // `data` is not an object gets there in the first place.
    let created = create(repo, "posts", json!(42));

    let error = repo
        .get("posts", &created.id)
        .expect_err("a non-object document is corrupt on the way out");
    assert_eq!(corrupt_id(error), created.id.as_string());

    let error = repo
        .list("posts", &ListParams::default())
        .expect_err("and list refuses rather than silently skipping it");
    assert_eq!(corrupt_id(error), created.id.as_string());

    // `find_by_field` only sees rows it selected, and a document with no fields
    // at all matches a search for a null value — so this one reaches it.
    let error = repo
        .find_by_field("posts", "status", &json!(null))
        .expect_err("and so does find_by_field, for a row it selected");
    assert_eq!(corrupt_id(error), created.id.as_string());

    // A search that cannot select the corrupt row must not fail on it either:
    // `$.status` of a non-object is absent, so the row is simply not matched.
    let found = repo
        .find_by_field("posts", "status", &json!("draft"))
        .expect("a row that was not selected cannot be corrupt");
    assert!(found.is_empty());
}

/// Field names are interpolated into SQL, so a name that is not an identifier
/// is refused rather than quoted (docs/storage.md §4).
fn check_hostile_field_names_are_refused<R: RecordRepository>(repo: &R) {
    create(repo, "posts", json!({ "title": "a" }));
    let hostile = "title') = 1 OR (1";

    let params = sorted_by(hostile, SortDirection::Asc);
    let error = repo.list("posts", &params).expect_err("sort_by is checked");
    assert_eq!(invalid_field(error), hostile);

    let params = searching("a", &[hostile]);
    let error = repo
        .list("posts", &params)
        .expect_err("search_fields is checked");
    assert_eq!(invalid_field(error), hostile);

    let error = repo
        .count("posts", &params)
        .expect_err("count runs the same clause, so it is checked too");
    assert_eq!(invalid_field(error), hostile);

    let error = repo
        .find_by_field("posts", hostile, &json!("a"))
        .expect_err("find_by_field is checked");
    assert_eq!(invalid_field(error), hostile);
}

// ---------------------------------------------------------------------------
// The suite, instantiated twice
// ---------------------------------------------------------------------------

/// A fresh in-memory repository. The `()` is a guard placeholder: the SQLite
/// constructor returns a `TempTree` there and the macro destructures both alike.
fn memory_repository() -> (MemoryRepository, ()) {
    (MemoryRepository::new(), ())
}

/// A fresh database in its own temporary directory. The `TempTree` is returned
/// so the test can hold it: dropping it deletes the file.
fn sqlite_repository() -> (SqliteRepository, TempTree) {
    let tree = TempTree::new("contract");
    let repo = SqliteRepository::open(tree.database()).expect("open a fresh database");
    (repo, tree)
}

/// Expands the whole suite for one constructor. Criterion: the shared contract
/// suite runs against both implementations.
macro_rules! contract_suite {
    ($name:ident, $constructor:path) => {
        mod $name {
            use super::*;

            // Each test gets its own repository, so nothing leaks between them.
            macro_rules! contract_test {
                ($test:ident, $check:path) => {
                    #[test]
                    fn $test() {
                        // `_guard` keeps the temporary directory alive for the
                        // body; dropping it early would delete the database.
                        let (repo, _guard) = $constructor();
                        $check(&repo);
                    }
                };
            }

            contract_test!(create_then_get_are_equal, check_create_then_get_are_equal);
            contract_test!(
                create_assigns_a_ulid_and_equal_timestamps,
                check_create_assigns_a_ulid_and_equal_timestamps
            );
            contract_test!(create_does_not_validate, check_create_does_not_validate);
            contract_test!(
                get_on_a_missing_id_is_none,
                check_get_on_a_missing_id_is_none
            );
            contract_test!(
                update_replaces_data_and_moves_updated_at,
                check_update_replaces_data_and_moves_updated_at
            );
            contract_test!(
                update_on_a_missing_id_is_not_found,
                check_update_on_a_missing_id_is_not_found
            );
            contract_test!(
                update_will_not_cross_collections,
                check_update_will_not_cross_collections
            );
            contract_test!(delete_removes_the_record, check_delete_removes_the_record);
            contract_test!(
                delete_on_a_missing_id_is_not_found,
                check_delete_on_a_missing_id_is_not_found
            );
            contract_test!(
                list_returns_only_the_requested_collection,
                check_list_returns_only_the_requested_collection
            );
            contract_test!(
                the_default_order_is_chronological,
                check_the_default_order_is_chronological
            );
            contract_test!(
                list_honours_limit_and_offset,
                check_list_honours_limit_and_offset
            );
            contract_test!(
                sort_by_a_field_runs_both_ways,
                check_sort_by_a_field_runs_both_ways
            );
            contract_test!(
                sort_is_stable_when_keys_are_equal,
                check_sort_is_stable_when_keys_are_equal
            );
            contract_test!(
                sort_tolerates_a_missing_field,
                check_sort_tolerates_a_missing_field
            );
            contract_test!(
                search_is_case_insensitive_over_the_named_fields,
                check_search_is_case_insensitive_over_the_named_fields
            );
            contract_test!(
                search_renders_numbers_the_way_sqlite_does,
                check_search_renders_numbers_the_way_sqlite_does
            );
            contract_test!(
                search_treats_wildcards_as_literals,
                check_search_treats_wildcards_as_literals
            );
            contract_test!(
                find_by_field_returns_every_match,
                check_find_by_field_returns_every_match
            );
            contract_test!(
                a_corrupt_row_is_reported_with_its_id,
                check_a_corrupt_row_is_reported_with_its_id
            );
            contract_test!(
                hostile_field_names_are_refused,
                check_hostile_field_names_are_refused
            );
        }
    };
}

contract_suite!(memory, memory_repository);
contract_suite!(sqlite, sqlite_repository);
