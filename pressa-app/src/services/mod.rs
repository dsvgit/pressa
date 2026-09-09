//! The business API: [`RecordService`] and [`CollectionService`].
//!
//! Specified by [SPEC-004](../../../specs/004-app-services.md). Everything the
//! UI wants to do goes through here, so that validation, uniqueness and
//! collection resolution live in one testable place instead of in `update()`.

mod collection;
mod record;

pub use collection::CollectionService;
pub use record::RecordService;
