//! Domain aggregates — generic-over-`Id` value types implementing the
//! locked schema docs.
//!
//! Each aggregate is `Aggregate<Id = Uuid7>` so projection adapters may
//! plug in backend-native id representations of the *same* UUIDv7 value.
//!
//! This module starts with the **leaf aggregates** that the bigger
//! container/track aggregates will reference. Later PRs add the analysis
//! aggregates (`Scene`, `Keyframe`, `AudioSegment`, `SubtitleCue`), then
//! the container/track aggregates (`Media`, the 3 facets, the 3 `*Track`s).

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub mod audio;
pub mod curation;
pub mod media;
pub mod media_file;
pub mod speaker;
pub mod watched_location;

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub use audio::{
  Audio, AudioError, AudioSegment, AudioSegmentError, AudioTrack, AudioTrackError, Word,
};
pub use curation::{SceneAnnotation, UserTag};
pub use media::Media;
pub use media_file::MediaFile;
pub use speaker::Speaker;
pub use watched_location::WatchedLocation;
