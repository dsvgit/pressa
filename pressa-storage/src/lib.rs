//! Storage adapters: `SqliteRepository`, `MemoryRepository` and the migration
//! runner behind the `RecordRepository` port owned by `pressa-core`.
//!
//! The two adapters must be indistinguishable through the port — that is the
//! contract of `specs/003-storage-repository.md`, and `values` and `clock` are
//! the shared rules that keep them so.

mod clock;
mod migrations;
mod values;

pub mod memory;
pub mod sqlite;

pub use memory::MemoryRepository;
pub use sqlite::SqliteRepository;
