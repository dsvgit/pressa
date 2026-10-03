//! Writes records into a project so that `pressa dev` has a table to draw.
//!
//! Dev tooling for [`just sandbox`](../../justfile), deliberately **not** a
//! subcommand: SPEC-005's screen A is three subcommands and `cli.rs`'s
//! `help_lists_exactly_three_subcommands` holds it there, while importing data
//! is M1 (`docs/roadmap.md` §1). An example target is neither, so `pressa
//! --help` is unchanged and nothing ships in the binary.
//!
//! ```text
//! cargo run -p pressa-tui --example seed -- <project dir> [count]
//! ```
//!
//! It writes through `RecordService`, the same door `pressa dev` reads back
//! through, so it cannot store a row the product would have refused — and it
//! reads the collections and fields out of `pressa.yaml` rather than naming
//! any, so it seeds whatever schema it is pointed at
//! ([ADR-0004](../../docs/adr/0004-schema-driven-ui.md)).
//!
//! What it does not promise: a `unique` field whose type has few values — a
//! `boolean`, or a `select` with two options — runs out of distinct values and
//! the service refuses the write. The seeder then stops with that error rather
//! than writing a partial collection, which is the right failure for a tool
//! whose whole job is to produce data the product accepts.

use std::error::Error;
use std::path::PathBuf;

use pressa_app::RecordService;
use pressa_app::config::{load_schema, project_at};
use pressa_app::domain::{Collection, Field, FieldType, Json, RecordRepository};
use pressa_storage::SqliteRepository;

/// How many records each collection gets when the count is not given. Twenty
/// outruns the fifteen body rows of an 80x24 panel, so the scroll summaries and
/// `g`/`G` have something to do.
const DEFAULT_COUNT: usize = 20;

/// The titles of SPEC-007's frames B and C, so the sandbox opens on the same
/// rows the snapshots show. After these the records are generated.
const FRAME_TITLES: [&str; 3] = ["Hello world", "About page", "Release notes"];

fn main() -> Result<(), Box<dyn Error>> {
    // `skip(1)` steps past the program name.
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().ok_or("usage: seed <project dir> [count]")?);
    let count = match args.next() {
        Some(text) => text.parse()?,
        None => DEFAULT_COUNT,
    };

    let paths = project_at(&dir)?;
    let schema = load_schema(&paths.config)?;
    // `open` creates `.pressa/` itself, so a project that has never been
    // opened is seedable.
    let repository = SqliteRepository::open(&paths.database)?;

    // Cloned because the service owns a schema; the loop below reads the other.
    let records = RecordService::new(repository, schema.clone());

    for (slug, collection) in &schema.collections {
        // Seeding twice would pile duplicates onto a collection whose unique
        // fields would then refuse the write, so an already-seeded project is
        // left alone rather than half-extended.
        let existing = records.count(slug, &Default::default())?;
        if existing > 0 {
            println!("{slug}: {existing} already there, left alone");
            continue;
        }

        for index in 0..count {
            records.create(slug, document(&records, collection, index)?)?;
        }
        println!("{slug}: {count} records");
    }

    Ok(())
}

/// One record's document, built from the collection's own fields.
///
/// Starts from `RecordService::blank`, which already has every field key at
/// `null` — and `null` is what the table draws as a dim `—`, so an optional
/// field left alone is the absent-value case rather than a missing key.
fn document<R: RecordRepository>(
    records: &RecordService<R>,
    collection: &Collection,
    index: usize,
) -> Result<Json, Box<dyn Error>> {
    let mut document = records.blank(&collection.slug)?;

    // `as_object_mut` rather than `document[key] = …`: indexing a non-object
    // `Json` panics, and this says what to do instead. It also keeps
    // `serde_json::Map` unnamed, which `pressa-tui` may not name.
    let Some(fields) = document.as_object_mut() else {
        return Err("a blank document is always a JSON object".into());
    };

    // The collection's first text field carries the frames' titles, so the
    // sandbox opens on the rows the snapshots show. Found by position rather
    // than by name: a seeder that looked for a field called `title` would only
    // work on schemas that happen to have one.
    let headline = collection
        .fields
        .iter()
        .position(|field| matches!(field.kind, FieldType::Text));

    for (column, field) in collection.fields.iter().enumerate() {
        // Every third record leaves its optional fields at `null`, so the
        // table has dim dashes in it to look at. A boolean is exempt: it has no
        // "unset", and an unchecked box is `false` (SPEC-004).
        let absent = !field.required && index % 3 == 2;
        if absent && !matches!(field.kind, FieldType::Boolean) {
            continue;
        }

        let value = if headline == Some(column) {
            Some(headline_value(field, index))
        } else {
            value_for(field, index)
        };

        if let Some(value) = value {
            fields.insert(field.name.clone(), value);
        }
    }

    Ok(document)
}

/// The headline column's value: one of SPEC-007's frame titles for the first
/// three records, then the field's label and the number.
fn headline_value(field: &Field, index: usize) -> Json {
    Json::from(
        FRAME_TITLES
            .get(index)
            .map_or_else(|| labelled(field, index), |title| title.to_string()),
    )
}

/// `Views 04` — a value that says which field and which record it came from.
fn labelled(field: &Field, index: usize) -> String {
    // Zero-padded so `Post 02` lines up beside `Post 10` on screen.
    format!("{} {:02}", field.label, index + 1)
}

/// A plausible value for `field` on record `index`, or `None` to leave it
/// blank.
///
/// One arm per `FieldType`, so adding a field type makes the compiler ask what
/// the seeder should write for it.
fn value_for(field: &Field, index: usize) -> Option<Json> {
    // 1-based: record 0 reads as 1 everywhere it is shown.
    let n = index + 1;

    match &field.kind {
        // A unique field needs a value no other record has, which the record's
        // own number supplies.
        FieldType::Text => Some(Json::from(format!("{}-{n:02}", field.name))),
        FieldType::Textarea => Some(Json::from(format!(
            "Everything there is to say about record {n}, at some length."
        ))),
        FieldType::Number => Some(Json::from(n * n)),
        FieldType::Boolean => Some(Json::from(index % 4 == 0)),
        // The stored form is RFC 3339; the table shows its first 16 characters.
        FieldType::DateTime => Some(Json::from(format!(
            "2026-09-{:02}T{:02}:{:02}:00Z",
            n % 28 + 1,
            n % 24,
            n % 60
        ))),
        // Always one of the options, or validation would refuse the record.
        FieldType::Select { options } => options
            .get(index % options.len().max(1))
            .map(|option| Json::from(option.as_str())),
        // An object whose key count varies, so the `{n}` cell varies with it.
        FieldType::Json => format!(r#"{{"source":"seed","n":{n}}}"#).parse().ok(),
    }
}
