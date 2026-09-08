//! `load_schema` — SPEC-001 "Config format", "Invariants" and "Error cases".

mod support;

use std::path::{Path, PathBuf};

use pressa_app::config::{ConfigError, load_schema};
use pressa_core::schema::{FieldType, Schema};
use support::TempTree;

/// Writes `yaml` into a fresh tree and loads it.
fn load(yaml: &str) -> Result<Schema, ConfigError> {
    let tree = TempTree::new("schema");
    let path = tree.write("pressa.yaml", yaml);
    load_schema(&path) // `tree` is dropped here, after the file has been read
}

fn load_ok(yaml: &str) -> Schema {
    load(yaml).expect("schema loads")
}

fn load_err(yaml: &str) -> ConfigError {
    load(yaml).expect_err("schema is rejected")
}

const FULL: &str = "\
project:
  name: blog-cms

database:
  path: db/custom.db

collections:
  posts:
    label: Articles
    list_columns: [title, status]
    fields:
      - name: title
        label: Title
        type: text
        required: true
      - name: status
        type: select
        options: [draft, published]
  authors:
    fields:
      - name: name
        type: text
";

// ---------------------------------------------------------------------------
// Happy path and defaults
// ---------------------------------------------------------------------------

#[test]
fn a_valid_config_loads_with_collections_in_file_order() {
    let schema = load_ok(FULL);

    assert_eq!(schema.project.name, "blog-cms");
    // `keys()` borrows; collect to compare against a plain slice of names.
    let order: Vec<&str> = schema.collections.keys().map(String::as_str).collect();
    assert_eq!(order, ["posts", "authors"]);

    let posts = &schema.collections["posts"];
    assert_eq!(posts.slug, "posts");
    assert_eq!(posts.label, "Articles");
    assert_eq!(posts.list_columns, ["title", "status"]);
    assert_eq!(posts.fields.len(), 2);
    assert_eq!(posts.fields[0].name, "title");
    assert!(posts.fields[0].required);
    assert_eq!(posts.fields[0].kind, FieldType::Text);
    assert_eq!(
        posts.fields[1].kind,
        FieldType::Select {
            options: vec!["draft".to_string(), "published".to_string()],
        }
    );
}

#[test]
fn every_field_type_of_m0_is_accepted() {
    let schema = load_ok(
        "\
project:
  name: types
collections:
  everything:
    fields:
      - name: a
        type: text
      - name: b
        type: textarea
      - name: c
        type: number
      - name: d
        type: boolean
      - name: e
        type: datetime
      - name: f
        type: select
        options: [one]
      - name: g
        type: json
",
    );

    let kinds: Vec<&FieldType> = schema.collections["everything"]
        .fields
        .iter() // shared borrow: the fields stay owned by the schema
        .map(|field| &field.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            &FieldType::Text,
            &FieldType::Textarea,
            &FieldType::Number,
            &FieldType::Boolean,
            &FieldType::DateTime,
            &FieldType::Select {
                options: vec!["one".to_string()]
            },
            &FieldType::Json,
        ]
    );
}

#[test]
fn label_defaults_to_the_titlecased_slug_and_name() {
    let schema = load_ok(
        "\
project:
  name: defaults
collections:
  blog_posts:
    fields:
      - name: published_at
        type: datetime
",
    );

    let collection = &schema.collections["blog_posts"];
    assert_eq!(collection.label, "Blog posts");
    assert_eq!(collection.fields[0].label, "Published at");
}

#[test]
fn list_columns_default_to_the_first_four_fields() {
    let schema = load_ok(
        "\
project:
  name: defaults
collections:
  posts:
    fields:
      - name: one
        type: text
      - name: two
        type: text
      - name: three
        type: text
      - name: four
        type: text
      - name: five
        type: text
",
    );

    assert_eq!(
        schema.collections["posts"].list_columns,
        ["one", "two", "three", "four"]
    );
}

#[test]
fn list_columns_default_to_every_field_when_there_are_fewer_than_four() {
    let schema = load_ok(
        "\
project:
  name: defaults
collections:
  posts:
    fields:
      - name: one
        type: text
      - name: two
        type: text
",
    );

    assert_eq!(schema.collections["posts"].list_columns, ["one", "two"]);
}

#[test]
fn required_and_unique_default_to_false() {
    let schema = load_ok(
        "\
project:
  name: defaults
collections:
  posts:
    fields:
      - name: title
        type: text
",
    );

    let field = &schema.collections["posts"].fields[0];
    assert!(!field.required);
    assert!(!field.unique);
}

#[test]
fn database_path_defaults_to_the_project_data_directory() {
    let schema = load_ok(
        "\
project:
  name: defaults
collections:
  posts:
    fields:
      - name: title
        type: text
",
    );

    assert_eq!(schema.database.path, PathBuf::from(".pressa/data.db"));
}

#[test]
fn a_configured_database_path_is_kept() {
    let schema = load_ok(FULL);

    assert_eq!(schema.database.path, PathBuf::from("db/custom.db"));
}

#[test]
fn every_collection_can_do_everything_in_m0() {
    let schema = load_ok(FULL);

    let capabilities = &schema.collections["posts"].capabilities;
    assert!(capabilities.create);
    assert!(capabilities.read);
    assert!(capabilities.update);
    assert!(capabilities.delete);
}

#[test]
fn the_blog_example_always_loads() {
    // The Golden Path (SPEC-000) runs against this file; it must never rot.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/blog/pressa.yaml"); // relative to pressa-app/
    let schema = load_schema(&path).expect("examples/blog/pressa.yaml loads");

    assert_eq!(schema.project.name, "blog-cms");
    let posts = &schema.collections["posts"];
    assert_eq!(posts.label, "Posts");
    assert_eq!(posts.fields.len(), 8);
    assert_eq!(posts.list_columns, ["title", "status", "views"]);
    assert!(posts.fields[1].unique); // slug
}

// ---------------------------------------------------------------------------
// The error table
// ---------------------------------------------------------------------------

#[test]
fn every_row_of_the_error_table_reports_its_path() {
    // (what the spec calls it, the config, the exact expected message)
    let cases: [(&str, &str, &str); 9] = [
        (
            "collections missing",
            "\
project:
  name: x
",
            "collections: at least one collection is required",
        ),
        (
            "collections empty",
            "\
project:
  name: x
collections: {}
",
            "collections: at least one collection is required",
        ),
        (
            "fields empty",
            "\
project:
  name: x
collections:
  posts:
    fields: []
",
            "collections.posts.fields: at least one field is required",
        ),
        (
            "fields missing",
            "\
project:
  name: x
collections:
  posts:
    label: Posts
",
            "collections.posts.fields: at least one field is required",
        ),
        (
            "unknown field type",
            "\
project:
  name: x
collections:
  posts:
    fields:
      - name: title
        type: text
      - name: slug
        type: text
      - name: body
        type: textarea
      - name: author
        type: relation
",
            "collections.posts.fields[3].type: unknown field type 'relation' \
             (expected one of: text, textarea, number, boolean, datetime, select, json)",
        ),
        (
            "duplicate field name",
            "\
project:
  name: x
collections:
  posts:
    fields:
      - name: title
        type: text
      - name: a
        type: text
      - name: b
        type: text
      - name: c
        type: text
      - name: title
        type: text
",
            "collections.posts.fields[4].name: duplicate field name 'title'",
        ),
        (
            "unknown list column",
            "\
project:
  name: x
collections:
  posts:
    list_columns: [titel]
    fields:
      - name: title
        type: text
",
            "collections.posts.list_columns[0]: unknown field 'titel'",
        ),
        (
            "select without options",
            "\
project:
  name: x
collections:
  posts:
    fields:
      - name: title
        type: text
      - name: body
        type: textarea
      - name: status
        type: select
",
            "collections.posts.fields[2].options: required for type 'select'",
        ),
        (
            "capitalised slug",
            "\
project:
  name: x
collections:
  Posts:
    fields:
      - name: title
        type: text
",
            "collections.Posts: must match ^[a-z][a-z0-9_]*$",
        ),
    ];

    for (name, yaml, expected) in cases {
        let error = load_err(yaml);
        assert!(
            matches!(error, ConfigError::Invalid { .. }),
            "{name}: expected Invalid, got {error:?}"
        );
        assert_eq!(error.to_string(), expected, "{name}");
    }
}

#[test]
fn a_hyphenated_slug_is_rejected() {
    let error = load_err(
        "\
project:
  name: x
collections:
  my-posts:
    fields:
      - name: title
        type: text
",
    );

    assert_eq!(
        error.to_string(),
        "collections.my-posts: must match ^[a-z][a-z0-9_]*$"
    );
}

#[test]
fn malformed_yaml_reports_the_line_and_column() {
    let error = load_err(
        "\
project:
  name: x
 collections: oops
",
    );

    assert!(matches!(error, ConfigError::Parse { .. }), "{error:?}");
    let message = error.to_string();
    assert!(message.contains("line"), "{message}");
    assert!(message.contains("column"), "{message}");
}

#[test]
fn a_missing_file_is_an_io_error_not_a_panic() {
    let tree = TempTree::new("absent");
    let error = load_schema(&tree.root().join("pressa.yaml")).expect_err("no such file");

    assert!(matches!(error, ConfigError::Io { .. }), "{error:?}");
}

// ---------------------------------------------------------------------------
// The name pattern is a security boundary (SPEC-001 "Invariants")
// ---------------------------------------------------------------------------

#[test]
fn a_field_name_carrying_sql_is_rejected() {
    // Field names are interpolated into `json_extract` paths (docs/storage.md
    // §4), so the pattern is the thing standing between a config and an
    // injection.
    let error = load_err(
        "\
project:
  name: x
collections:
  posts:
    fields:
      - name: 'title; DROP TABLE records'
        type: text
",
    );

    assert_eq!(
        error.to_string(),
        "collections.posts.fields[0].name: must match ^[a-z][a-z0-9_]*$"
    );
}

#[test]
fn field_names_must_start_with_a_lowercase_letter() {
    for bad in ["Title", "1title", "_title", "ti-tle", "ti tle", ""] {
        let yaml = format!(
            "\
project:
  name: x
collections:
  posts:
    fields:
      - name: '{bad}'
        type: text
"
        );
        let error = load_err(&yaml);
        assert_eq!(
            error.to_string(),
            "collections.posts.fields[0].name: must match ^[a-z][a-z0-9_]*$",
            "accepted {bad:?}"
        );
    }
}

#[test]
fn underscores_and_digits_are_allowed_after_the_first_letter() {
    let schema = load_ok(
        "\
project:
  name: x
collections:
  posts_2:
    fields:
      - name: h1_heading
        type: text
",
    );

    assert_eq!(schema.collections["posts_2"].fields[0].name, "h1_heading");
}

// ---------------------------------------------------------------------------
// select options (SPEC-001 "Invariants")
// ---------------------------------------------------------------------------

#[test]
fn select_needs_at_least_one_option() {
    let error = load_err(
        "\
project:
  name: x
collections:
  posts:
    fields:
      - name: status
        type: select
        options: []
",
    );

    assert_eq!(
        error.to_string(),
        "collections.posts.fields[0].options: at least one option is required"
    );
}

#[test]
fn select_options_must_be_unique() {
    let error = load_err(
        "\
project:
  name: x
collections:
  posts:
    fields:
      - name: status
        type: select
        options: [draft, published, draft]
",
    );

    assert_eq!(
        error.to_string(),
        "collections.posts.fields[0].options[2]: duplicate option 'draft'"
    );
}
