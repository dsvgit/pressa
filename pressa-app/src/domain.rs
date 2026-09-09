//! The domain types the UI renders, re-exported so that `pressa-tui` names only
//! `pressa-app` (`docs/architecture.md` §2).
//!
//! `pressa-tui` has no `pressa-core` dependency, in any section; everything it
//! draws reaches it through here. T8 and T9 extend the list as they need
//! `Record`, `RecordId` and `FieldError`.

pub use pressa_core::schema::{Collection, Schema};
