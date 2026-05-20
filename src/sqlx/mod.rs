//! `sqlx` row-mapping backend — domain ⇄ SQL row conversions for
//! Postgres, MySQL, and SQLite.
//!
//! Each backend is its own optional feature
//! (`sqlx-postgres` / `sqlx-mysql` / `sqlx-sqlite`); enable only what
//! you ship with. Off by default.
//!
//! ## Layout
//!
//! - [`error`] — backend-specific [`SqlxError`] (`Debug + Clone + PartialEq +
//!   Eq + IsVariant + non_exhaustive`, implements [`core::error::Error`]).
//! - [`dto`] — serde-serializable DTOs mirroring the cross-cutting
//!   value-objects (`Provenance`, `LocalizedText`, `MediaDevice`,
//!   `MediaGeoLocation`, `ErrorInfo`, the structured `Location` oneof)
//!   plus helpers for the 16-byte UUID / 32-byte checksum byte
//!   conversions. The DTOs are shared across all three backends.
//! - [`postgres`] / [`mysql`] / [`sqlite`] — per-backend modules. Each
//!   ships row structs with `sqlx::FromRow` derives, `TryFrom` impls
//!   going to/from the domain aggregates, the canonical `schema.sql`
//!   DDL, and a minimal `migrations/0001_init.sql` mirror.
//!
//! ## Mapping conventions
//!
//! - **Identity columns** (`id` / `parent` / FKs): `uuid` (Postgres
//!   native), `BINARY(16)` (MySQL), `BLOB` 16 bytes (SQLite). Domain
//!   `Uuid7` is `pub use crate::sqlx::dto::{uuid7_to_uuid, uuid_to_uuid7,
//!   bytes_to_uuid7}` for symmetric conversion.
//! - **File checksum** (`FileChecksum`): `BYTEA` (Postgres),
//!   `BINARY(32)` (MySQL), `BLOB(32)` (SQLite). Round-trip via
//!   [`dto::bytes_to_checksum`].
//! - **Nested value-objects** (`Provenance`, `LocalizedText`,
//!   `MediaDevice`, `MediaGeoLocation`, `ErrorInfo`, `Location`): stored
//!   as `JSONB` (Postgres) / `JSON` (MySQL) / `TEXT` containing JSON
//!   (SQLite). The DTO module owns the canonical wire shape so all
//!   three backends round-trip identical bytes.
//! - **Domain enums** (`MediaKind`, `ScanStatus`, `SubtitleKind`,
//!   `AudioContentKind`, the `*IndexStage` types): mapped to `SMALLINT`
//!   (Postgres) / `TINYINT` (MySQL) / `INTEGER` (SQLite) via the enum
//!   `as u32`/`from_u32` round-trip helpers added per backend.
//! - **Bitflags** (`MediaErrorFlags`, the `*IndexStatus` types): stored
//!   as their underlying `u16`/`u32` `bits()` value in an integer
//!   column.
//! - **Wall-clock** (`jiff::Timestamp`): `TIMESTAMPTZ` (Postgres) /
//!   `DATETIME` (MySQL) / `TEXT` ISO-8601 (SQLite). Converted via
//!   `jiff::Timestamp` ⇄ `chrono::DateTime<Utc>` at the boundary so
//!   sqlx's native chrono support drives the encode/decode.
//!
//! ## Coverage (this revision)
//!
//! Fully mapped (round-trip tests + schema): `Media`, `WatchedLocation`,
//! `Speaker`, `UserTag`, `SceneAnnotation`, plus the three facet
//! aggregates (`Audio`, `Video`, `Subtitle`) and the `IndexProgress`
//! VO. The track-level aggregates (`AudioTrack`, `VideoTrack`,
//! `SubtitleTrack`) and per-track analysis aggregates (`AudioSegment`,
//! `Scene`, `Keyframe`, `SubtitleCue`) carry deep nested
//! mediaframe-placeholder VOs that themselves have no serde derives in
//! the domain layer; their row-struct surface is **tracked as a
//! follow-up PR** so the placeholder-flip-to-mediaframe sweep doesn't
//! land at the same time as the field-by-field row schema. The
//! per-backend `schema.sql` includes commented stubs for the deferred
//! tables.

pub mod dto;
pub mod error;

pub use error::SqlxError;

#[cfg(feature = "sqlx-postgres")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlx-postgres")))]
pub mod postgres;

#[cfg(feature = "sqlx-mysql")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlx-mysql")))]
pub mod mysql;

#[cfg(feature = "sqlx-sqlite")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlx-sqlite")))]
pub mod sqlite;
