//! `sqlx` row-mapping backend — domain ⇄ SQL row conversions for
//! Postgres, MySQL, and SQLite.
//!
//! Each backend is its own optional feature
//! (`sqlx-postgres` / `sqlx-mysql` / `sqlx-sqlite`); enable only what
//! you ship with. Off by default.
//!
//! ## Layout
//!
//! - [`error`](crate::sqlx::error) — backend-specific [`SqlxError`](crate::sqlx::error::SqlxError) (`Debug + Clone + PartialEq +
//!   Eq + IsVariant + non_exhaustive`, implements [`core::error::Error`]).
//! - [`dto`](crate::sqlx::dto) — shared row-mapping helpers for the 16-byte UUID /
//!   32-byte checksum / ms-timestamp conversions. Nested value-objects
//!   are no longer stored as JSON DTOs — each scalar VO is flattened
//!   into its own real columns and the one many-to-many collection
//!   rides in a join table.
//! - [`postgres`](crate::sqlx::postgres) / [`mysql`](crate::sqlx::mysql) / [`sqlite`](crate::sqlx::sqlite) — per-backend modules. Each
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
//!   [`dto::bytes_to_checksum`](crate::sqlx::dto::bytes_to_checksum).
//! - **Nested value-objects** (capture `Device`, capture `GeoLocation`,
//!   `ErrorInfo`): flattened into real, individually-indexable columns
//!   (e.g. `Device` → `device_make` / `device_model`, `ErrorInfo` →
//!   `*_error_code` / `*_error_message`). Presence is encoded by the
//!   discriminating column's nullability — no JSON columns.
//! - **Collections** (`SceneAnnotation::user_tags`): a many-to-many
//!   join table (`scene_annotation_user_tag`) with an `ordinal` column
//!   preserving the in-aggregate order.
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
//! Fully mapped (round-trip tests + schema): `Media` (incl. the
//! published [`mediaframe::container::Format`] container slug and the
//! [`mediaframe::capture::Device`] / [`mediaframe::capture::GeoLocation`]
//! EXIF descriptors, flattened into real columns),
//! `WatchedLocation`, `Speaker`, `UserTag`, `SceneAnnotation` (with its
//! `scene_annotation_user_tag` join table).
//!
//! The track-level aggregates (`AudioTrack`, `VideoTrack`,
//! `SubtitleTrack`) and per-track analysis aggregates (`AudioSegment`,
//! `Scene`, `Keyframe`, `SubtitleCue`) carry deep nested
//! [`mediaframe`] descriptor VOs (codecs, `ChannelLayout`, `Loudness`,
//! `Fingerprint`, `PixelFormat`, `color::Info`, `Dimensions`, the frame
//! geometry enums, subtitle `Format` / `TrackOrigin`, …). Their
//! flattened-column row-struct surface is **tracked as a follow-up PR**
//! to keep this revision focused.

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
