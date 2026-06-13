//! Audio media-kind cluster: the [`Audio`] facet, the [`AudioTrack`] track
//! aggregate (per-recording metadata + signal analysis carried via
//! `mediaframe::audio` VOs — `Tags` / `CoverArt` / `Loudness` /
//! `Fingerprint`), the [`AudioSegment`] heavy segmented-ML analog of
//! `Scene` (with its `Word`-level timing VO), and the [`SoundEvent`]
//! CED sound-event detection (the audio analog of `Scene`'s detector
//! field).
//!
//! Locked specs:
//! - `schema/audio.md` rev 8 (facet, A-loc per-track rollup)
//! - `schema/audio_track.md` rev 3 (per-recording metadata + signal
//!   analysis + per-track `Provenance`)
//! - `schema/audio_segments.md` rev 3 (reconciled `dia`⋈`asry` segment)
//! - `schema/sound_events.md` (CED sound-event detection)
//!
//! `AudioFileRecord` is **not** a separate aggregate per the locked schema —
//! file-level scalars live on `Media`/`Audio`, per-recording music metadata
//! on `AudioTrack`. See `schema/audio_file_record.md` (SUPERSEDED).

pub mod facet;
pub mod segment;
pub mod sound_event;
pub mod track;

pub use facet::{Audio, AudioError};
pub use segment::{AudioSegment, AudioSegmentError, Word};
pub use sound_event::{SoundEvent, SoundEventError};
pub use track::{AudioTrack, AudioTrackError};
