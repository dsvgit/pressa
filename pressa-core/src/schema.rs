//! The schema: what collections exist, and what fields they hold.
//!
//! Everything downstream is generated from these types — the sidebar, the
//! table, the editor, the SQL (see [ADR-0004](../../docs/adr/0004-schema-driven-ui.md)).
//! Loading and validating `pressa.yaml` into a [`Schema`] lives in
//! `pressa-app`; this module only describes the shape and the invariants.

use std::path::PathBuf;

use indexmap::IndexMap;

/// The pattern every collection slug and field name must match.
///
/// Not cosmetic: names are interpolated into `json_extract` paths in SQL
/// (`docs/storage.md` §4), so this is a security boundary.
pub const IDENTIFIER_PATTERN: &str = "^[a-z][a-z0-9_]*$";

/// Checks [`IDENTIFIER_PATTERN`] by hand — a regex engine would be a new
/// dependency for one line of matching.
pub fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    // The first character carries the stricter rule, so it is pulled out.
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false, // empty, or does not start with a-z
    }
    // `all` short-circuits on the first character that breaks the rule.
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[derive(Debug, Clone, PartialEq)]
pub struct Schema {
    pub project: ProjectConfig,
    pub database: DatabaseConfig,
    /// Insertion order is preserved: it is the sidebar order.
    pub collections: IndexMap<String, Collection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectConfig {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseConfig {
    /// Relative to the directory containing `pressa.yaml`.
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Collection {
    /// Stable id, used in storage — `"posts"`.
    pub slug: String,
    /// Shown in the UI — `"Posts"`.
    pub label: String,
    pub fields: Vec<Field>,
    /// Field names shown as table columns.
    pub list_columns: Vec<String>,
    pub capabilities: CollectionCapabilities,
}

/// What the UI is allowed to offer for a collection. All true in M0; it exists
/// so read-only collections later need no special case in the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionCapabilities {
    pub create: bool,
    pub read: bool,
    pub update: bool,
    pub delete: bool,
}

impl Default for CollectionCapabilities {
    // `derive(Default)` would give all-false; M0 wants the opposite.
    fn default() -> Self {
        CollectionCapabilities {
            create: true,
            read: true,
            update: true,
            delete: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub label: String,
    pub kind: FieldType,
    pub required: bool,
    pub unique: bool,
}

/// The seven field types of M0. `Relation` is deferred to M2 on purpose.
///
/// Dispatch is a `match` on this enum rather than a trait object, so adding a
/// type makes the compiler list every place that has to handle it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    Text,
    Textarea,
    Number,
    Boolean,
    DateTime,
    Select { options: Vec<String> },
    Json,
}

impl FieldType {
    /// The names accepted in `pressa.yaml`, in the order error messages list
    /// them.
    pub const NAMES: [&'static str; 7] = [
        "text", "textarea", "number", "boolean", "datetime", "select", "json",
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_start_with_a_lowercase_letter() {
        assert!(is_identifier("title"));
        assert!(is_identifier("published_at"));
        assert!(is_identifier("h1"));

        assert!(!is_identifier(""));
        assert!(!is_identifier("Title"));
        assert!(!is_identifier("1title"));
        assert!(!is_identifier("_title"));
        assert!(!is_identifier("my-posts"));
        assert!(!is_identifier("title; DROP TABLE records"));
    }

    #[test]
    fn every_collection_may_do_everything_by_default() {
        let capabilities = CollectionCapabilities::default();
        assert!(capabilities.create && capabilities.read);
        assert!(capabilities.update && capabilities.delete);
    }
}
