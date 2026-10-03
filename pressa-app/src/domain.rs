//! The domain types the UI renders, re-exported so that `pressa-tui` names only
//! `pressa-app` (`docs/architecture.md` §2).
//!
//! `pressa-tui` has no `pressa-core` dependency, in any section; everything it
//! draws reaches it through here. T9 extends the list as it needs `FieldError`.

pub use pressa_core::record::{Record, RecordId};
pub use pressa_core::repository::RecordRepository;
pub use pressa_core::schema::{Collection, Field, FieldType, Schema};
/// The stored form of a document, re-exported under the name the UI uses.
pub use serde_json::Value as Json;
