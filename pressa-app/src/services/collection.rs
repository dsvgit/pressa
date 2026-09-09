//! [`CollectionService`]: read-only access to the schema's collections.

use pressa_core::schema::{Collection, Schema};

use crate::error::AppError;

/// The schema, answered as questions the UI asks: what collections are there,
/// and what is this one? Holds the schema by value and never re-reads
/// `pressa.yaml` (SPEC-004 "Invariants").
pub struct CollectionService {
    schema: Schema,
}

impl CollectionService {
    pub fn new(schema: Schema) -> Self {
        CollectionService { schema }
    }

    /// Every collection in `pressa.yaml` order, which is the sidebar order.
    pub fn all(&self) -> impl Iterator<Item = &Collection> {
        // `values()` borrows from the IndexMap, so insertion order is preserved.
        self.schema.collections.values()
    }

    pub fn get(&self, slug: &str) -> Result<&Collection, AppError> {
        self.schema
            .collections
            .get(slug)
            // `ok_or_else` builds the error only when the lookup missed.
            .ok_or_else(|| AppError::UnknownCollection(slug.to_string()))
    }
}
