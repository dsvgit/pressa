//! Use cases: `RecordService`, `CollectionService`, config loading and
//! `AppError` — the business API the TUI talks to instead of SQL.

pub mod config;
pub mod error;
pub mod services;

pub use error::AppError;
pub use services::{CollectionService, RecordService};
