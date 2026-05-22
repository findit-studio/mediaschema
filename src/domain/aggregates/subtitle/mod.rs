//! Subtitle media-kind aggregates (locked `schema/subtitle*.md`).
//!
//! - [`Subtitle`] — thin subtitle facet of a `Media` (parent → `Media`); holds
//!   only the per-track id list and the indexing roll-up.
//! - [`SubtitleTrack`] — one subtitle stream (parent → `Subtitle`); carries
//!   the per-track codec / language / origin / disposition / index state
//!   plus the per-track [`crate::domain::Provenance`].
//! - [`SubtitleCue`] — one parsed cue of a `SubtitleTrack`
//!   (parent → `SubtitleTrack`); media-time span + parsed/OCR text +
//!   optional inline bitmap.
//!
//! Each aggregate is `Aggregate<Id = Uuid7>` and validated through a
//! `try_new(...)` constructor that rejects nil ids and nil FK parents.
//!
//! The per-track descriptor fields use the published `mediaframe` types
//! ([`mediaframe::codec::SubtitleCodec`], [`mediaframe::subtitle::Format`]
//! / [`mediaframe::subtitle::TrackOrigin`], [`mediaframe::lang::Language`],
//! [`mediaframe::disposition::TrackDisposition`]) and the inline cue
//! bitmap is [`bytes::Bytes`]. The one remaining placeholder is the
//! per-track time (`mediatime::TrackTime` → `mediatime::Timestamp`)
//! pending that type's release.

pub mod cue;
pub mod facet;
pub mod track;

pub use crate::domain::vo::IndexProgress;
pub use cue::{SubtitleCue, SubtitleCueError};
pub use facet::{Subtitle, SubtitleError};
pub use track::{SubtitleTrack, SubtitleTrackError};
