//! Data media-facet cluster: the [`Data`] facet + the [`DataTrack`] track
//! aggregate.
//!
//! Data tracks carry container-level **timed-metadata** streams (FFmpeg
//! `codec_type=data`): Sony rtmd, GoPro GPMF, MISB KLV, timecode. v1
//! records **presence + a codec/stream descriptor + container metadata
//! only** — no rtmd/GPMF/KLV sample parsing, no per-sample child
//! aggregate. The facet is the data analog of [`Audio`](super::audio) /
//! [`Subtitle`](super::subtitle), minus any heavy analysis subtree.

pub mod facet;
pub mod track;

pub use facet::{Data, DataError, DataParts};
pub use track::{DataTrack, DataTrackError, DataTrackParts};
