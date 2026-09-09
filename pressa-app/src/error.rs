//! [`AppError`]: everything the service layer can refuse to do.
//!
//! One error type for the whole business API, because the TUI renders exactly
//! one of these per failed action (docs/architecture.md §4).

use pressa_core::validation::FieldError;

use crate::config::ConfigError;

/// What `RecordService` and `CollectionService` return on failure.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("unknown collection: {0}")]
    UnknownCollection(String),

    #[error("no record {id} in collection {collection}")]
    NotFound { collection: String, id: String },

    /// Structured and unflattened on purpose: the editor renders each error
    /// under its own field, so a joined string here would break the form.
    #[error("{} field(s) rejected", .0.len())]
    Validation(Vec<FieldError>),

    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Storage(#[from] pressa_core::repository::StorageError),
}
