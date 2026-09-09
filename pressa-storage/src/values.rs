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
/// SQLite has four kinds here where JSON has six: booleans come back as the
/// numbers 1 and 0, and objects and arrays come back as their JSON text. This
/// enum is that collapse, so both adapters compare the same four things.
///
/// Integers and reals are kept apart even though they compare numerically,
/// because they do **not** render as text alike — `7` is `7` and `7.0` is `7.0`
/// — and search compares the rendered text.
#[derive(Debug, Clone)]
pub enum JsonKey {
    Null,
    /// A JSON integer SQLite stores as INTEGER. Held as `i64` rather than `f64`
    /// so that a value past 2^53 keeps every digit, the way SQLite prints it.
    Integer(i64),
    Real(f64),
    Text(String),
}

impl JsonKey {
    /// Canonicalises a value. `None` — the key was absent — is `Null`, which is
    /// what `json_extract` returns for a path that is not there.
    pub fn of(value: Option<&Json>) -> JsonKey {
        match value {
            None | Some(Json::Null) => JsonKey::Null,
            // SQLite's JSON functions have no boolean type; true is the number 1.
            Some(Json::Bool(flag)) => JsonKey::Integer(i64::from(*flag)),
            // `as_i64` succeeds for exactly the numbers SQLite keeps as INTEGER:
            // a whole number in `i64` range. Anything else — a float, or an
            // integer too large for `i64` — SQLite stores as REAL, and so do we.
            Some(Json::Number(number)) => match number.as_i64() {
                Some(whole) => JsonKey::Integer(whole),
                // `as_f64` cannot fail for a JSON number that serde_json parsed.
                None => number.as_f64().map(real).unwrap_or(JsonKey::Null),
            },
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
            JsonKey::Integer(whole) => Some(whole.to_string()),
            JsonKey::Real(number) => Some(real_text(*number)),
            JsonKey::Text(text) => Some(text.clone()),
        }
    }

    /// The rank SQLite sorts value kinds by: NULL, then numbers, then text.
    // Integers and reals share a rank because SQL compares them numerically,
    // never by kind: `7` sorts between `6.5` and `7.5`, not beside the integers.
    fn rank(&self) -> u8 {
        match self {
            JsonKey::Null => 0,
            JsonKey::Integer(_) | JsonKey::Real(_) => 1,
            JsonKey::Text(_) => 2,
        }
    }
}

/// A real, with negative zero folded into zero.
///
/// SQLite has one zero: it prints `-0.0` as `0.0` and compares the two equal,
/// while `f64::total_cmp` orders them apart. Folding here means the two adapters
/// never disagree about a value no user can tell apart anyway.
fn real(number: f64) -> JsonKey {
    // `== 0.0` is true for both zeros, which is exactly the pair being folded.
    JsonKey::Real(if number == 0.0 { 0.0 } else { number })
}

/// A real as SQLite prints it: fifteen significant digits, trailing zeros
/// trimmed, and always at least one digit after the point.
///
/// This is SQLite's own `%!.15g`, reimplemented because search compares the
/// *rendered* value and Rust's `f64` formatting renders differently — `1.0`
/// prints as `1` in Rust and `1.0` in SQLite, and `1e20` as twenty-one digits in
/// Rust and `1.0e+20` in SQLite. A search that found a number in one adapter and
/// not the other is exactly the drift `specs/003` exists to prevent.
fn real_text(number: f64) -> String {
    if number == 0.0 {
        return "0.0".to_string();
    }

    // Fifteen significant digits in scientific form, which is the precision
    // SQLite asks for — and it yields the exponent that picks the layout below.
    let scientific = format!("{number:.14e}");
    // `{:.14e}` always contains an `e`; the fallback only keeps this total.
    let (mantissa, exponent) = scientific
        .split_once('e')
        .unwrap_or((scientific.as_str(), "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);

    // Outside this window of exponents, `%g` switches to scientific notation.
    if !(-4..15).contains(&exponent) {
        // Same rule as C's `%g`: too large or too small, and it goes exponential.
        let sign = if exponent < 0 { '-' } else { '+' };
        // SQLite pads the exponent to two digits: `1.0e+20`, `1.0e-07`.
        format!("{}e{sign}{:02}", trimmed(mantissa), exponent.abs())
    } else {
        // Fifteen significant digits means this many after the point.
        let decimals = (14 - exponent).max(0) as usize;
        trimmed(&format!("{number:.decimals$}"))
    }
}

/// Drops trailing zeros from the fraction, keeping the point and one digit.
fn trimmed(text: &str) -> String {
    match text.split_once('.') {
        // No point at all: SQLite still prints one, as in `100000000000000.0`.
        None => format!("{text}.0"),
        Some((whole, fraction)) => {
            let fraction = fraction.trim_end_matches('0');
            if fraction.is_empty() {
                format!("{whole}.0")
            } else {
                format!("{whole}.{fraction}")
            }
        }
    }
}

impl Ord for JsonKey {
    /// SQLite's ordering: NULL first, then numbers numerically, then text by
    /// bytes — which is what `str`'s own `Ord` does, so the BINARY collation and
    /// Rust agree.
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            // Two integers compare exactly: past 2^53 an `f64` detour would call
            // neighbouring values equal, and SQLite does not.
            (JsonKey::Integer(left), JsonKey::Integer(right)) => left.cmp(right),
            // `total_cmp` is a total order over f64, so no NaN can poison a sort.
            (JsonKey::Real(left), JsonKey::Real(right)) => left.total_cmp(right),
            // Mixed kinds compare as doubles, which is what SQL means by `7 = 7.0`.
            (JsonKey::Integer(left), JsonKey::Real(right)) => (*left as f64).total_cmp(right),
            (JsonKey::Real(left), JsonKey::Integer(right)) => left.total_cmp(&(*right as f64)),
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
            JsonKey::Real(2.0),
        ];
        keys.sort();
        assert_eq!(
            keys,
            vec![
                JsonKey::Null,
                JsonKey::Real(2.0),
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
    fn integers_and_reals_interleave_rather_than_grouping_by_kind() {
        // The rank must not separate them, or `7` would sort away from `6.5`.
        let mut keys = [
            JsonKey::of(Some(&json!(7))),
            JsonKey::of(Some(&json!(7.5))),
            JsonKey::of(Some(&json!(6.5))),
            JsonKey::of(Some(&json!(6))),
        ];
        keys.sort();
        assert_eq!(
            keys.iter()
                .filter_map(JsonKey::as_search_text)
                .collect::<Vec<_>>(),
            vec!["6", "6.5", "7", "7.5"]
        );
    }

    #[test]
    fn an_integer_past_two_to_the_fifty_third_keeps_every_digit() {
        // The value `f64` cannot hold: it would round to ...992 and both the
        // comparison and the rendered text would be wrong.
        let key = JsonKey::of(Some(&json!(9_007_199_254_740_993_i64)));
        assert_eq!(key.as_search_text().as_deref(), Some("9007199254740993"));
        assert_ne!(key, JsonKey::of(Some(&json!(9_007_199_254_740_992_i64))));
    }

    #[test]
    fn booleans_are_the_numbers_sqlite_makes_of_them() {
        assert_eq!(JsonKey::of(Some(&json!(true))), JsonKey::Integer(1));
        assert_eq!(JsonKey::of(Some(&json!(false))), JsonKey::Integer(0));
        // And they render as those numbers, not as `true` and `false`.
        assert_eq!(
            JsonKey::of(Some(&json!(true))).as_search_text().as_deref(),
            Some("1")
        );
    }

    #[test]
    fn reals_render_the_way_sqlite_prints_them() {
        // Every expectation here was taken from SQLite 3.46 itself, by running
        // `cast(json_extract(...) as text)` over the same values. Search compares
        // this text, so a difference is a difference between the two adapters.
        for (number, printed) in [
            (1.0, "1.0"),
            (100.0, "100.0"),
            (2.5, "2.5"),
            (0.1, "0.1"),
            (-2.5, "-2.5"),
            (-0.0, "0.0"),
            (0.000_123_456, "0.000123456"),
            (1e-4, "0.0001"),
            (1e-5, "1.0e-05"),
            (1e-7, "1.0e-07"),
            (1e14, "100000000000000.0"),
            (1e15, "1.0e+15"),
            (1e20, "1.0e+20"),
            (1e100, "1.0e+100"),
            (1.0 / 3.0, "0.333333333333333"),
            (f64::MAX, "1.79769313486232e+308"),
        ] {
            assert_eq!(real_text(number), printed, "rendering {number:e}");
        }
    }

    #[test]
    fn a_json_integer_renders_without_a_decimal_point_and_a_real_with_one() {
        // The bug this pair guards: both used to render as `1`, so searching
        // `1.0` found the record in SQLite and missed it in memory.
        assert_eq!(
            JsonKey::of(Some(&json!(1))).as_search_text().as_deref(),
            Some("1")
        );
        assert_eq!(
            JsonKey::of(Some(&json!(1.0))).as_search_text().as_deref(),
            Some("1.0")
        );
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
