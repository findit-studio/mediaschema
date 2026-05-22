//! PostgreSQL row mapping. `Uuid7` rides as a native `uuid` column
//! (`uuid::Uuid` in sqlx). `FileChecksum` rides as `BYTEA`. Nested VOs
//! ride as `JSONB` columns — the row struct reads them as `String` for
//! a portable mapping shape; queries selecting those columns should
//! cast as `column::text` so sqlx decodes them via the `String` type
//! adapter. Wall-clock columns are `BIGINT` milliseconds-since-epoch
//! (matches the cross-backend convention; native `TIMESTAMPTZ` is a
//! tracked follow-up).
//!
//! See the module-level [`super`] doc for the cross-backend mapping
//! conventions and current coverage scope.

pub mod leaves;
pub mod media;
pub mod media_file;

pub use leaves::{PgSceneAnnotationRow, PgSpeakerRow, PgUserTagRow, PgWatchedLocationRow};
pub use media::PgMediaRow;
pub use media_file::PgMediaFileRow;

/// Canonical PostgreSQL DDL for the mediaschema tables this revision maps.
pub const SCHEMA_SQL: &str = include_str!("schema.sql");

/// Initial migration mirror of [`SCHEMA_SQL`].
pub const MIGRATION_0001_INIT: &str = include_str!("migrations/0001_init.sql");
