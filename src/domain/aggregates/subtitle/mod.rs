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
//! ### mediaframe placeholder
//!
//! The locked schema doc references `mediaframe::Language`,
//! `mediaframe::SubtitleCodec`, `mediaframe::SubtitleFormat`,
//! `mediaframe::SubtitleTrackOrigin`, `mediaframe::TrackDisposition`,
//! and `mediatime::TrackTime` — none of which are dependencies of
//! `mediaschema` yet (see `schema/mediaframe-candidates.md`). Per-field
//! `TODO(mediaframe)` notes mark the placeholders (`SmolStr` / `u32` /
//! `u64` / `Option<…>`) that will be tightened once `mediaframe` is in.

pub mod cue;
pub mod facet;
pub mod track;

pub use cue::{SubtitleCue, SubtitleCueError};
pub use facet::{IndexProgress, Subtitle, SubtitleError};
pub use track::{SubtitleTrack, SubtitleTrackError};
