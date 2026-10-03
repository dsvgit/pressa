//! The whole of what a screen can read: [`AppState`] and the small types it is
//! made of ([SPEC-006](../../../specs/006-tui-shell.md) "Domain model").

use pressa_app::domain::{Collection, FieldError, Json, Record, RecordId, Schema};

/// Everything the UI knows. `view` gets this and nothing else, and `update` is
/// the only function that changes it.
#[derive(Debug, Clone, PartialEq)]
pub struct AppState {
    pub schema: Schema,
    pub route: Route,
    pub sidebar: SidebarState,
    pub list: ListState,
    pub editor: EditorState,
    pub overlay: Option<Overlay>,
    pub status: Option<StatusMessage>,
    pub should_quit: bool,
}

/// Where we are, in the URL sense (`docs/tui.md` §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Home,
    List { collection: String },
    New { collection: String },
    Edit { collection: String, id: RecordId },
}

impl Route {
    /// The collection an editor route names, or `None` outside the editor.
    pub fn editing(&self) -> Option<&str> {
        match self {
            // One arm for both: they differ only in whether a save carries an id.
            Route::New { collection } | Route::Edit { collection, .. } => Some(collection),
            Route::Home | Route::List { .. } => None,
        }
    }
}

/// The draft, where the focus is, and what the last save refused.
///
/// One struct for both editor routes: they differ only in where `draft` came
/// from — `blank_document` or a loaded record — and in whether `SaveRecord`
/// carries an id (SPEC-008 "Domain model").
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EditorState {
    /// The document being edited. A JSON object with every field key present
    /// from the moment the editor opens until it closes; `null` only before a
    /// `LoadRecord` has answered, and after the editor is cleared.
    pub draft: Json,
    /// What `draft` was when it arrived. Dirty is `draft != original`, computed
    /// where it is needed and stored nowhere, so it cannot go stale (Q5).
    pub original: Json,
    /// Index into `collection.fields`.
    pub focus: usize,
    /// The first field the panel would like to draw. `view` derives the window
    /// from the focus instead (`form::first_visible`), so this stays 0; it is
    /// kept because `docs/tui.md` §1 reserves it, as `list.offset` is.
    pub offset: usize,
    /// The text typed so far for the focused field: `Some` is an input
    /// context, `None` is `Context::Editor`. One fact stored once, which is
    /// why `Context` can be derived.
    pub input: Option<String>,
    /// What the last save refused, plus any parse error the editor itself
    /// produced — at most one per field.
    pub errors: Vec<FieldError>,
    /// The record's id at `Edit`, carried so `breadcrumbs` needs no `Route`
    /// lookup when the draft cannot name itself (Q8).
    pub id: Option<RecordId>,
}

impl EditorState {
    /// A fresh editor over `draft`, clean because `original` is the same.
    pub fn over(draft: Json, id: Option<RecordId>) -> EditorState {
        EditorState {
            // Cloned once: the two must start equal and then diverge.
            original: draft.clone(),
            draft,
            id,
            ..EditorState::default()
        }
    }

    /// Whether anything was committed since the draft arrived.
    pub fn is_dirty(&self) -> bool {
        self.draft != self.original
    }

    /// Whether the draft has arrived: an object, rather than the `null` an
    /// `Edit` route holds while its `LoadRecord` is in flight.
    pub fn is_loaded(&self) -> bool {
        self.draft.is_object()
    }
}

/// Drawn on top of the current route, which is why it is not a `Route`
/// (`docs/tui.md` §1). One variant in T9; T10 and T11 add theirs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    /// "Discard unsaved changes?" — `next` is where `Confirm` goes.
    ConfirmDiscard { next: Box<Route> },
}

/// The records of the collection the route names, and where we are in them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ListState {
    pub records: Vec<Record>,
    /// Index into `records`. 0 when there are none.
    pub selected: usize,
    /// The row the table would like to start at; `view` clamps it rather than
    /// mutating it, as the sidebar's already does.
    pub offset: usize,
    pub load: Load,
}

/// Whether the records in [`ListState`] are the collection's or the remains of
/// a load that failed.
///
/// Two variants and not three: effects are synchronous (ADR-0002), so the queue
/// drains before the next draw and a `Loading` body is unobservable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Load {
    #[default]
    Ok,
    Failed,
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
            list: ListState::default(),
            editor: EditorState::default(),
            overlay: None,
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
