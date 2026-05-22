//! Buffa wire ⇄ domain conversion layer.
//!
//! Gated on `feature = "buffa"`. The hand-written domain types live in
//! `crate::domain::*`; the buffa-generated wire types live in
//! `crate::generated::media::v1::*`. This module provides the
//! `From<&Wire> for Domain` (fallibly, via `TryFrom` where the domain
//! rejects values the wire layer can carry) and
//! `From<&Domain> for Wire` (always infallible) bridge between them.
//!
//! ## Coverage
//!
//! ### Bridged (round-trip tested):
//!
//! - **Primitives** — [`primitives`]:
//!   - `media.v1::Id`            ⇄ `domain::Uuid7`
//!   - `media.v1::FileChecksum`  ⇄ `domain::FileChecksum`
//!   - `media.v1::ErrorInfo`     ⇄ `domain::ErrorInfo` (+ `ErrorCode`)
//! - **Location oneof** — [`location`]:
//!   - `media.v1::Local`         ⇄ `domain::Location::Local`
//!   - `media.v1::Location`      ⇄ `Option<domain::Location>`
//! - **Enums** — [`enums`]:
//!   - `media.v1::DbMediaKind`           ⇄ `domain::MediaKind`
//!   - `buffa::EnumValue<DbMediaKind>`   ⇄ `domain::MediaKind`
//! - **Aggregates**:
//!   - [`watched_location`] — `media.v1::WatchedLocation` ⇄
//!     `domain::WatchedLocation` (partial: see module doc).
//!   - [`media`] — `media.v1::Media` ⇄ `domain::Media` (partial: see
//!     module doc).
//!   - [`media_file`] — `media.v1::MediaFile` ⇄ `domain::MediaFile`
//!     (1:1 — wire shape mirrors the domain, including `watch_volume`,
//!     so a single message round-trips losslessly).
//!
//! ### Not yet bridged (no clean wire counterpart)
//!
//! The buffa-generated wire layer in this crate predates the
//! `0.1.0`-locked schema redesign and uses a different field set for
//! the deeper aggregates. The following domain aggregates have **no
//! structural counterpart** in `media.v1` and are tracked as a
//! follow-up once the wire layer is regenerated against the locked
//! `schema/*.md` docs:
//!
//! - `Video` / `VideoTrack` / `Scene` / `Keyframe` — wire `Video` /
//!   `VideoTrack` / `Scene` / `Keyframe` exist but carry an entirely
//!   different field set (per-track metadata wrapped in `*Meta`
//!   messages, plus FFmpeg-shaped detection structs that don't
//!   correspond to any domain type).
//! - `Audio` / `AudioTrack` / `AudioSegment` — same situation; the
//!   wire `Audio` wraps an `AudioMeta`/`AudioStreamMeta`/`AudioSummary`
//!   tree that doesn't match the locked aggregates.
//! - `Subtitle` / `SubtitleTrack` / `SubtitleCue` — same: the wire
//!   `Subtitle` carries pre-locked-schema cue / track fields.
//! - `Speaker`, `UserTag`, `SceneAnnotation`, `IndexProgress` — no
//!   wire counterpart at all (or a fundamentally different shape
//!   such as `SpeakerSegment` with three `u32` fields).
//! - Auxiliary VOs (`Provenance`, `LocalizedText`) — bridged inline in
//!   their parent aggregate where one exists.
//! - The capture VOs are now the published mediaframe types
//!   (`mediaframe::capture::Device` / `GeoLocation`), bridged inline on
//!   wire `Media`: `Device` ⇄ `device_make`/`device_model` pair, and
//!   `GeoLocation` ⇄ the ISO 6709 `gps_location` string (round-tripped
//!   via `from_iso6709`/`to_iso6709`).
//!
//! ## Error model
//!
//! All wire → domain failures surface as [`BuffaError`] (see [`error`]).
//! Variants carry the lower-level domain validating error (`Uuid7Error`,
//! `LocationError`) verbatim so callers can recover via `is_*` /
//! `try_unwrap_*` predicates.

#![cfg_attr(docsrs, doc(cfg(feature = "buffa")))]

pub mod enums;
pub mod error;
pub mod location;
pub mod media;
pub mod media_file;
pub mod primitives;
pub mod watched_location;

pub use error::BuffaError;
