//! Parsing `pressa.yaml` and validating it into a [`Schema`].
//!
//! Two passes on purpose. `serde_yaml` fills a permissive `Raw*` shape — the
//! field type stays a `String` — and the second pass turns that into the
//! domain types, so every rejection can carry the YAML path that caused it
//! rather than serde's own wording.

use std::fs;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use pressa_core::schema::{
    Collection, CollectionCapabilities, DatabaseConfig, Field, FieldType, IDENTIFIER_PATTERN,
    ProjectConfig, Schema, is_identifier,
};
use serde::Deserialize;

use super::ConfigError;

/// Where the database goes when `database.path` is omitted.
const DEFAULT_DATABASE_PATH: &str = ".pressa/data.db";
/// How many fields become table columns when `list_columns` is omitted.
const DEFAULT_LIST_COLUMNS: usize = 4;

// ---------------------------------------------------------------------------
// Pass 1: the file as written
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawSchema {
    project: RawProject,
    // `default` so an absent section is the default one rather than an error.
    #[serde(default)]
    database: RawDatabase,
    // Absent `collections` must produce our own message, not serde's.
    #[serde(default)]
    collections: IndexMap<String, RawCollection>,
}

#[derive(Debug, Deserialize)]
struct RawProject {
    name: String,
}

#[derive(Debug, Default, Deserialize)]
struct RawDatabase {
    path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct RawCollection {
    label: Option<String>,
    list_columns: Option<Vec<String>>,
    #[serde(default)]
    fields: Vec<RawField>,
}

#[derive(Debug, Deserialize)]
struct RawField {
    name: String,
    label: Option<String>,
    // `type` is a Rust keyword, so the field is named `kind` and renamed here.
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    unique: bool,
    options: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Pass 2: validation into the domain types
// ---------------------------------------------------------------------------

/// Reads and validates the schema at `path`.
pub fn load_schema(path: &Path) -> Result<Schema, ConfigError> {
    let text = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    let raw: RawSchema = serde_yaml::from_str(&text).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })?;

    validate(raw)
}

fn validate(raw: RawSchema) -> Result<Schema, ConfigError> {
    if raw.collections.is_empty() {
        return Err(ConfigError::invalid(
            "collections",
            "at least one collection is required",
        ));
    }

    let mut collections = IndexMap::with_capacity(raw.collections.len());
    // `into_iter` consumes the map, so slugs and fields move into the domain
    // types instead of being cloned. Order is IndexMap's insertion order,
    // which is the order they appear in the file.
    for (slug, raw_collection) in raw.collections {
        let collection = validate_collection(&slug, raw_collection)?;
        collections.insert(slug, collection);
    }

    Ok(Schema {
        project: ProjectConfig {
            name: raw.project.name,
        },
        database: DatabaseConfig {
            path: raw
                .database
                .path
                .unwrap_or_else(|| PathBuf::from(DEFAULT_DATABASE_PATH)),
        },
        collections,
    })
}

fn validate_collection(slug: &str, raw: RawCollection) -> Result<Collection, ConfigError> {
    let at = format!("collections.{slug}");

    if !is_identifier(slug) {
        return Err(ConfigError::invalid(
            &at,
            format!("must match {IDENTIFIER_PATTERN}"),
        ));
    }

    if raw.fields.is_empty() {
        return Err(ConfigError::invalid(
            format!("{at}.fields"),
            "at least one field is required",
        ));
    }

    let mut fields: Vec<Field> = Vec::with_capacity(raw.fields.len());
    for (index, raw_field) in raw.fields.into_iter().enumerate() {
        let field = validate_field(&at, index, raw_field, &fields)?;
        fields.push(field);
    }

    let list_columns = match raw.list_columns {
        Some(columns) => {
            for (index, column) in columns.iter().enumerate() {
                // Every column must name a field, or the table would render a
                // header with nothing under it.
                if !fields.iter().any(|field| &field.name == column) {
                    return Err(ConfigError::invalid(
                        format!("{at}.list_columns[{index}]"),
                        format!("unknown field '{column}'"),
                    ));
                }
            }
            columns
        }
        // Default: the first four fields, or all of them if there are fewer.
        None => fields
            .iter()
            .take(DEFAULT_LIST_COLUMNS)
            .map(|field| field.name.clone())
            .collect(),
    };

    Ok(Collection {
        label: raw.label.unwrap_or_else(|| titlecase(slug)),
        slug: slug.to_string(),
        fields,
        list_columns,
        capabilities: CollectionCapabilities::default(),
    })
}

/// `seen` is the fields validated so far in this collection — the only place a
/// duplicate name can come from.
fn validate_field(
    collection_at: &str,
    index: usize,
    raw: RawField,
    seen: &[Field],
) -> Result<Field, ConfigError> {
    let at = format!("{collection_at}.fields[{index}]");

    if !is_identifier(&raw.name) {
        return Err(ConfigError::invalid(
            format!("{at}.name"),
            format!("must match {IDENTIFIER_PATTERN}"),
        ));
    }

    if seen.iter().any(|field| field.name == raw.name) {
        return Err(ConfigError::invalid(
            format!("{at}.name"),
            format!("duplicate field name '{}'", raw.name),
        ));
    }

    let kind = validate_field_type(&at, &raw.kind, raw.options)?;

    Ok(Field {
        label: raw.label.unwrap_or_else(|| titlecase(&raw.name)),
        name: raw.name,
        kind,
        required: raw.required,
        unique: raw.unique,
    })
}

fn validate_field_type(
    at: &str,
    kind: &str,
    options: Option<Vec<String>>,
) -> Result<FieldType, ConfigError> {
    match kind {
        "text" => Ok(FieldType::Text),
        "textarea" => Ok(FieldType::Textarea),
        "number" => Ok(FieldType::Number),
        "boolean" => Ok(FieldType::Boolean),
        "datetime" => Ok(FieldType::DateTime),
        "json" => Ok(FieldType::Json),
        "select" => validate_select(at, options),
        // `relation` lands here like any other unknown name: the list names
        // what M0 has, and promises nothing about what is coming.
        unknown => Err(ConfigError::invalid(
            format!("{at}.type"),
            format!(
                "unknown field type '{unknown}' (expected one of: {})",
                FieldType::NAMES.join(", ")
            ),
        )),
    }
}

fn validate_select(at: &str, options: Option<Vec<String>>) -> Result<FieldType, ConfigError> {
    let Some(options) = options else {
        return Err(ConfigError::invalid(
            format!("{at}.options"),
            "required for type 'select'",
        ));
    };

    if options.is_empty() {
        return Err(ConfigError::invalid(
            format!("{at}.options"),
            "at least one option is required",
        ));
    }

    for (index, option) in options.iter().enumerate() {
        // Only the options before this one are checked, so the error points at
        // the second occurrence rather than the first.
        if options[..index].contains(option) {
            return Err(ConfigError::invalid(
                format!("{at}.options[{index}]"),
                format!("duplicate option '{option}'"),
            ));
        }
    }

    Ok(FieldType::Select { options })
}

/// `published_at` -> `Published at`: underscores become spaces and the first
/// letter is capitalised. Matches the labels in `examples/blog/pressa.yaml`.
fn titlecase(identifier: &str) -> String {
    let spaced = identifier.replace('_', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        // `as_str()` is the rest of the string after the first character,
        // which the iterator has already stepped past.
        Some(first) => first.to_uppercase().to_string() + chars.as_str(),
        None => spaced,
    }
}

#[cfg(test)]
mod tests {
    use super::titlecase;

    #[test]
    fn titlecase_capitalises_and_unscores() {
        assert_eq!(titlecase("posts"), "Posts");
        assert_eq!(titlecase("published_at"), "Published at");
        assert_eq!(titlecase("blog_posts_2"), "Blog posts 2");
    }
}
