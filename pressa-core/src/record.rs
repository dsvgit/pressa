//! `Record` — one JSON document belonging to one collection — and its id.
//!
//! Records are stored as JSON with no database-level typing
//! ([ADR-0003](../../docs/adr/0003-json-documents-not-eav.md)), so this type
//! carries the document whole rather than a field-by-field decomposition.

use std::fmt;
use std::str::FromStr;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde_json::Value as Json;
use ulid::{Generator, Ulid};

/// The process's id generator.
///
/// A plain `Ulid::generate()` re-randomises the low bits on every call, so two
/// ids minted in the *same millisecond* can come out in either order. Storage
/// lists records with `ORDER BY id` and calls that chronological
/// (docs/storage.md §4), so "later means larger" has to hold for records created
/// back to back, not just across milliseconds. `Generator` guarantees it.
static GENERATOR: Mutex<Generator> = Mutex::new(Generator::new());

/// A record's identity: a ULID, not a UUIDv4.
///
/// ULIDs sort chronologically as text, which is what lets the default list
/// view be `ORDER BY id` with no extra index (domain-model.md §7).
// `Ord` is derived so a `BTreeMap<RecordId, _>` iterates in creation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecordId(Ulid);

impl RecordId {
    /// Mints an id: the current time, then randomness, and always larger than
    /// the last id this process minted.
    pub fn new() -> Self {
        // A poisoned lock means another thread panicked mid-mint. The generator
        // holds only the previous id, so recovering it is safe and beats refusing
        // to make records.
        let mut generator = GENERATOR
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        // The error case is the random bits overflowing inside one millisecond —
        // 2^80 ids — and `commit_overflow_increment` keeps the order intact.
        match generator.generate() {
            Ok(id) => RecordId(id),
            Err(overflow) => RecordId(overflow.commit_overflow_increment()),
        }
    }

    /// The 26-character Crockford base32 form — what is stored in SQLite.
    pub fn as_string(&self) -> String {
        self.0.to_string()
    }
}

impl Default for RecordId {
    // Clippy asks for this whenever `new()` takes no arguments.
    fn default() -> Self {
        RecordId::new()
    }
}

impl fmt::Display for RecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Delegates to `Ulid`'s own formatting rather than copying the alphabet.
        write!(f, "{}", self.0)
    }
}

/// Returned when text is not a well-formed ULID.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a valid record id: {0}")]
pub struct InvalidRecordId(pub String);

impl FromStr for RecordId {
    type Err = InvalidRecordId;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        // `map_err` drops ulid's own error type so callers need not name it.
        Ulid::from_string(text)
            .map(RecordId)
            .map_err(|_| InvalidRecordId(text.to_string()))
    }
}

/// One document, with the two timestamps storage maintains for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: RecordId,
    pub collection: String,
    /// The whole document. Normally a JSON object; a row where it is not is
    /// reported as `StorageError::Corrupt` on read rather than skipped.
    pub data: Json,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_through_their_text_form() {
        let id = RecordId::new();
        // `parse` picks the `FromStr` impl from the annotated type.
        let parsed: RecordId = id.to_string().parse().expect("a minted id parses");
        assert_eq!(id, parsed);
        assert_eq!(id.to_string().len(), 26);
    }

    #[test]
    fn nonsense_is_not_an_id() {
        assert!("not-a-ulid".parse::<RecordId>().is_err());
        assert!("".parse::<RecordId>().is_err());
    }

    #[test]
    fn ids_minted_later_sort_after_earlier_ones() {
        let first = RecordId::new();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = RecordId::new();

        assert!(first < second, "{first} should sort before {second}");
        // Text order must agree with value order: SQLite sorts the text form.
        assert!(first.as_string() < second.as_string());
    }

    #[test]
    fn ids_minted_in_the_same_millisecond_still_ascend() {
        // The case a plain `Ulid::generate()` gets wrong, and the one that makes
        // `ORDER BY id` chronological for records created back to back.
        let ids: Vec<RecordId> = (0..500).map(|_| RecordId::new()).collect();

        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "ids must ascend in the order they were minted");

        // And the same has to hold of the text form, which is what SQLite sorts.
        let text: Vec<String> = ids.iter().map(RecordId::as_string).collect();
        let mut sorted_text = text.clone();
        sorted_text.sort();
        assert_eq!(text, sorted_text);
    }
}
