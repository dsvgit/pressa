//! The whole of what a screen can read: [`AppState`] and the small types it is
//! made of ([SPEC-006](../../../specs/006-tui-shell.md) "Domain model").
//!
//! `list`, `editor` and `overlay` from `docs/tui.md` §1 are deliberately absent:
//! a field arrives with the task that renders it.

use pressa_app::domain::{Collection, Schema};

/// Everything the UI knows. `view` gets this and nothing else, and `update` is
/// the only function that changes it.
#[derive(Debug, Clone, PartialEq)]
pub struct AppState {
    pub schema: Schema,
    pub route: Route,
    pub sidebar: SidebarState,
    pub status: Option<StatusMessage>,
    pub should_quit: bool,
}

/// Where we are, in the URL sense (`docs/tui.md` §2). `New` and `Edit` arrive
/// with T9.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Home,
    List { collection: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SidebarState {
    /// Index into `schema.collections`. Always < len, which is >= 1: the
    /// loader rejects a schema with no collections (SPEC-001).
    pub selected: usize,
    /// The row the list area would like to start at. The list's height is only
    /// known while drawing, so `view` clamps this to keep `selected` visible
    /// rather than mutating it — `view` is pure (`docs/tui.md` §6).
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusMessage {
    pub text: String,
    pub kind: StatusKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Error,
}

impl AppState {
    /// A fresh session: at Home, first collection selected, nothing to say.
    pub fn new(schema: Schema) -> AppState {
        AppState {
            schema,
            route: Route::Home,
            sidebar: SidebarState::default(),
            status: None,
            should_quit: false,
        }
    }

    /// The collection the sidebar is on.
    ///
    /// `Option` because `selected` is a plain index: a hand-built state can
    /// point past the end, and a missing collection is not worth a panic.
    pub fn selected_collection(&self) -> Option<&Collection> {
        // `get_index` keeps the schema's own order, which is the sidebar order.
        self.schema
            .collections
            .get_index(self.sidebar.selected)
            // The pair is (slug, collection); only the collection is wanted.
            .map(|(_, collection)| collection)
    }
}

/// The label the schema gives `slug`, or the slug itself when it knows no such
/// collection — a stale route is then visible rather than a panic.
pub fn collection_label<'a>(schema: &'a Schema, slug: &'a str) -> &'a str {
    schema
        .collections
        .get(slug)
        .map_or(slug, |collection| collection.label.as_str())
}
