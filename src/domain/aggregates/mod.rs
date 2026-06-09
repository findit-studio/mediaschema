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

// Medium-specific aggregate trees: gated by both a heap tier (the
// aggregates reach `Vec` / `SmolStr` internally) and the matching
// medium-aggregate feature so consumers can opt out of a tree they do
// not need.
// `AudioTrack::metadata` and the other per-track `IndexMap` bags use
// the std-only default hasher (`RandomState`), same constraint as the
// `Chapter` aggregate below.
#[cfg(all(feature = "std", feature = "audio"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "std", feature = "audio"))))]
pub mod audio;
// `Chapter::metadata` is `indexmap::IndexMap<SmolStr, SmolStr>`, and
// `IndexMap`'s default hasher generic is `std::hash::RandomState`
// (std-only). Gating on `std` (rather than `any(std, alloc)`) keeps
// the no-std + alloc build clean; downstream consumers wanting Chapter
// must select `std`.
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub mod chapter;
pub mod curation;
pub mod media;
pub mod media_file;
pub mod person;
pub mod speaker;
#[cfg(all(feature = "std", feature = "subtitle"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "std", feature = "subtitle"))))]
pub mod subtitle;
#[cfg(all(feature = "std", feature = "video"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "std", feature = "video"))))]
pub mod video;
pub mod watched_location;

#[cfg(all(feature = "std", feature = "audio"))]
pub use audio::{
  Audio, AudioError, AudioSegment, AudioSegmentError, AudioTrack, AudioTrackError, Word,
};
#[cfg(feature = "std")]
pub use chapter::{Chapter, ChapterError};
pub use curation::{SceneAnnotation, UserTag};
pub use media::Media;
pub use media_file::MediaFile;
pub use person::{Person, PersonConfidence, PersonError};
pub use speaker::Speaker;
#[cfg(all(feature = "std", feature = "subtitle"))]
pub use subtitle::{
  AssCue, AssData, AssStyle, LrcCue, LrcData, LrcMetadata, LrcWord, SrtCue, SrtData, Subtitle,
  SubtitleCue, SubtitleCueKind, SubtitleTrack, VttCue, VttData, VttRegion, VttStyleBlock,
};
#[cfg(all(feature = "std", feature = "video"))]
pub use video::{Keyframe, Scene, Video, VideoTrack};
pub use watched_location::WatchedLocation;
