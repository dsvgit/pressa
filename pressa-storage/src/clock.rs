//! The one place timestamps are made and rendered.
//!
//! Both adapters share it so that a record created in memory and a record
//! created in SQLite carry the same kind of value, and so that a timestamp
//! survives a round trip through the database unchanged.

use chrono::{DateTime, SecondsFormat, Utc};
use pressa_core::record::RecordId;
use pressa_core::repository::StorageError;

/// Now, as storage stamps it. The service layer never passes timestamps in —
/// that keeps "when did this change" a single source of truth (docs/storage.md §5).
pub fn now() -> DateTime<Utc> {
    Utc::now()
}

/// The stored form: RFC 3339, always `Z`, always nine fractional digits.
///
/// Fixed precision on purpose — the number of digits has to be enough to carry
/// whatever the clock returned, or `create` and the `get` after it would
/// disagree about `created_at`.
pub fn format(at: &DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Nanos, true)
}

/// Reads a stored timestamp back. A row that does not parse is corrupt, not a
/// reason to panic.
pub fn parse(id: &RecordId, text: &str) -> Result<DateTime<Utc>, StorageError> {
    DateTime::parse_from_rfc3339(text)
        // Parsing yields a fixed offset; the domain works in UTC throughout.
        .map(|at| at.with_timezone(&Utc))
        .map_err(|error| StorageError::Corrupt {
            id: id.as_string(),
            detail: format!("timestamp {text:?} is not RFC 3339: {error}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timestamp_survives_the_round_trip_exactly() {
        let at = now();
        let id = RecordId::new();
        assert_eq!(parse(&id, &format(&at)).expect("parses"), at);
    }

    #[test]
    fn nonsense_is_corrupt_and_names_the_record() {
        let id = RecordId::new();
        let error = parse(&id, "last tuesday").expect_err("not a timestamp");
        assert!(matches!(error, StorageError::Corrupt { .. }));
        assert!(format!("{error}").contains(&id.as_string()));
    }
}
