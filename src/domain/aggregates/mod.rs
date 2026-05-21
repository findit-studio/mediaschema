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
pub mod speaker;
#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub mod subtitle;
#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub mod video;
pub mod watched_location;

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub use audio::{
  Audio, AudioError, AudioSegment, AudioSegmentError, AudioTrack, AudioTrackError, Word,
};
pub use curation::{SceneAnnotation, UserTag};
pub use media::{Media, MediaDevice, MediaGeoLocation};
pub use speaker::Speaker;
// NOTE: both `subtitle` and `video` independently define an
// `IndexProgress` type (the convention block told each subagent not to
// touch `src/domain/vo.rs`). Tracked as a follow-up: lift to
// `src/domain/vo.rs` as a shared cross-cutting VO. For now,
// `IndexProgress` re-exported here is the subtitle copy; video's copy
// is reachable as `video::IndexProgress`.
#[cfg(any(feature = "std", feature = "alloc"))]
pub use subtitle::{IndexProgress, Subtitle, SubtitleCue, SubtitleTrack};
#[cfg(any(feature = "std", feature = "alloc"))]
pub use video::{Keyframe, Scene, Video, VideoTrack};
pub use watched_location::WatchedLocation;
