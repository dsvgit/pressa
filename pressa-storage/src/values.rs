//! The value semantics both adapters have to agree on.
//!
//! This module exists because `specs/003` asks for two implementations that are
//! *indistinguishable through the trait*. Ordering, equality and
//! case-insensitive matching are where they would silently drift apart, so the
//! rules live here once and `MemoryRepository` follows what SQLite does rather
//! than what Rust would do by default.

use std::cmp::Ordering;

use pressa_core::record::RecordId;
use pressa_core::repository::StorageError;
use pressa_core::schema::is_identifier;
use serde_json::Value as Json;

/// The escape character the search `LIKE` uses.
pub const LIKE_ESCAPE: char = '\\';

/// A field value as SQLite's `json_extract` yields it.
///
/// SQLite has three kinds here where JSON has six: booleans come back as the
/// numbers 1 and 0, and objects and arrays come back as their JSON text. This
/// enum is that collapse, so both adapters compare the same three things.
#[derive(Debug, Clone)]
pub enum JsonKey {
    Null,
    Number(f64),
    Text(String),
}

impl JsonKey {
    /// Canonicalises a value. `None` — the key was absent — is `Null`, which is
    /// what `json_extract` returns for a path that is not there.
    pub fn of(value: Option<&Json>) -> JsonKey {
        match value {
            None | Some(Json::Null) => JsonKey::Null,
            // SQLite's JSON functions have no boolean type; true is the number 1.
            Some(Json::Bool(flag)) => JsonKey::Number(if *flag { 1.0 } else { 0.0 }),
            // `as_f64` cannot fail for a JSON number that serde_json parsed.
            Some(Json::Number(number)) => number
                .as_f64()
                .map(JsonKey::Number)
                .unwrap_or(JsonKey::Null),
            Some(Json::String(text)) => JsonKey::Text(text.clone()),
            // Objects and arrays: `json_extract` hands back their JSON text.
            Some(nested) => JsonKey::Text(nested.to_string()),
        }
    }

    /// The value at `$.<field>` of a document. A document that is not an object
    /// has no fields at all, which `get` already reports as `None`.
    pub fn at(document: &Json, field: &str) -> JsonKey {
        JsonKey::of(document.get(field))
    }

    /// The text `lower(json_extract(...))` would compare against, or `None` for
    /// `Null` — which no `LIKE` ever matches.
    pub fn as_search_text(&self) -> Option<String> {
        match self {
            JsonKey::Null => None,
            // Rust and SQLite render an integral double the same way: `100`.
            JsonKey::Number(number) => Some(number.to_string()),
            JsonKey::Text(text) => Some(text.clone()),
        }
    }

    /// The rank SQLite sorts value kinds by: NULL, then numbers, then text.
    fn rank(&self) -> u8 {
        match self {
            JsonKey::Null => 0,
            JsonKey::Number(_) => 1,
            JsonKey::Text(_) => 2,
        }
    }
}

impl Ord for JsonKey {
    /// SQLite's ordering: NULL first, then numbers numerically, then text by
    /// bytes — which is what `str`'s own `Ord` does, so the BINARY collation and
    /// Rust agree.
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            // `total_cmp` is a total order over f64, so no NaN can poison a sort.
            (JsonKey::Number(left), JsonKey::Number(right)) => left.total_cmp(right),
            (JsonKey::Text(left), JsonKey::Text(right)) => left.cmp(right),
            // Different kinds never compare equal; rank decides.
            _ => self.rank().cmp(&other.rank()),
        }
    }
}

impl PartialOrd for JsonKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for JsonKey {
    // Defined through `cmp` so that equality and ordering cannot disagree —
    // which is also how `1` ends up equal to `1.0`, as it is in SQL.
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for JsonKey {}

/// Refuses a field name that is not an identifier.
///
/// Field names reach SQL by interpolation into a `json_extract` path, not by
/// binding (docs/storage.md §4), so this is a security boundary. Schema loading
/// rejects such names first; this is the backstop that makes the query safe on
/// its own terms.
pub fn checked_field(field: &str) -> Result<&str, StorageError> {
    if is_identifier(field) {
        Ok(field)
    } else {
        Err(StorageError::InvalidField {
            field: field.to_string(),
        })
    }
}

/// Refuses a document that is not a JSON object, naming the row.
///
/// Called when a record is *read*, never when one is written: `create` stores
/// `data` verbatim so that the repository can be used to repair bad data.
pub fn require_object(id: &RecordId, data: &Json) -> Result<(), StorageError> {
    if data.is_object() {
        Ok(())
    } else {
        Err(StorageError::Corrupt {
            id: id.as_string(),
            detail: "data is not a JSON object".to_string(),
        })
    }
}

/// The `LIKE` pattern for a substring search: lowercased and wrapped in `%`.
///
/// `%`, `_` and the escape character itself are escaped, because the user typed
/// characters, not wildcards — and because `MemoryRepository` matches with a
/// plain substring check, which has no wildcards to offer.
pub fn like_pattern(term: &str) -> String {
    let mut pattern = String::with_capacity(term.len() + 2);
    pattern.push('%');
    for character in term.chars() {
        if character == '%' || character == '_' || character == LIKE_ESCAPE {
            pattern.push(LIKE_ESCAPE);
        }
        pattern.push(character);
    }
    pattern.push('%');
    // ASCII-only, to match SQLite's `lower()`. Unicode-aware folding here would
    // make the two adapters disagree on non-ASCII text.
    pattern.to_ascii_lowercase()
}

/// Whether `haystack` contains `needle`, ignoring ASCII case only — the same
/// comparison `lower(...) LIKE '%...%'` makes in SQLite.
pub fn contains_ignoring_ascii_case(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nulls_sort_before_numbers_which_sort_before_text() {
        let mut keys = vec![
            JsonKey::Text("a".to_string()),
            JsonKey::Null,
            JsonKey::Number(2.0),
        ];
        keys.sort();
        assert_eq!(
            keys,
            vec![
                JsonKey::Null,
                JsonKey::Number(2.0),
                JsonKey::Text("a".to_string())
            ]
        );
    }

    #[test]
    fn a_number_equals_the_same_number_written_differently() {
        // Which is what SQL does, and what serde_json's own PartialEq does not.
        assert_eq!(JsonKey::of(Some(&json!(1))), JsonKey::of(Some(&json!(1.0))));
        assert_ne!(JsonKey::of(Some(&json!(1))), JsonKey::of(Some(&json!("1"))));
    }

    #[test]
    fn booleans_are_the_numbers_sqlite_makes_of_them() {
        assert_eq!(JsonKey::of(Some(&json!(true))), JsonKey::Number(1.0));
        assert_eq!(JsonKey::of(Some(&json!(false))), JsonKey::Number(0.0));
    }

    #[test]
    fn an_absent_key_and_an_explicit_null_are_the_same_thing() {
        let document = json!({ "here": null });
        assert_eq!(JsonKey::at(&document, "here"), JsonKey::Null);
        assert_eq!(JsonKey::at(&document, "nowhere"), JsonKey::Null);
    }

    #[test]
    fn wildcards_in_a_search_term_are_escaped() {
        assert_eq!(like_pattern("100%"), "%100\\%%");
        assert_eq!(like_pattern("a_b"), "%a\\_b%");
        assert_eq!(like_pattern("Hello"), "%hello%");
    }

    #[test]
    fn hostile_field_names_are_refused() {
        assert!(checked_field("title").is_ok());
        assert!(checked_field("published_at").is_ok());
        assert!(checked_field("title'); DROP TABLE records --").is_err());
        assert!(checked_field("").is_err());
    }

    #[test]
    fn a_document_that_is_not_an_object_is_corrupt() {
        let id = RecordId::new();
        assert!(require_object(&id, &json!({ "a": 1 })).is_ok());
        let error = require_object(&id, &json!(42)).expect_err("not an object");
        assert!(format!("{error}").contains(&id.as_string()));
    }
}
