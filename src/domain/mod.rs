//! Hand-written domain layer — the architectural hub for mediaschema.
//!
//! App logic programs against these types; the buffa-generated wire types at
//! the crate root are the serialization edge. Domain types are governed by
//! `domain → wire → domain` round-trip property tests (added as the aggregate
//! types come online in later stacked PRs).
//!
//! Locked-schema implementation tracks `schema/*.md`. This module starts with
//! the primitives + shared cross-cutting VOs; subsequent stacked PRs add the
//! enums + bitflags, then the leaf aggregates, then the big container/track
//! aggregates.

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub mod aggregates;
pub mod bitflags;
pub mod enums;
pub mod identified;
pub mod primitives;
#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub mod vo;

// Always available (pure no-std no-alloc):
pub use bitflags::{
  AttachmentIndexStatus, AudioIndexStatus, DataIndexStatus, MediaErrorFlags, SubtitleIndexStatus,
  VideoIndexStatus,
};
pub use enums::{
  AudioContentKind, AudioIndexStage, KeyframeExtractor, MediaKind, ScanStatus, SceneDetector,
  SubtitleIndexStage, SubtitleKind, ThumbnailKind, ThumbnailKindParseError, VideoIndexStage,
};
pub use identified::Identified;
pub use primitives::{ErrorCode, FileChecksum, Rgba, Uuid7};

// Heap-tier (gate on `any(std, alloc)`): cross-cutting aggregates and
// types that reach `Vec` / `SmolStr` regardless of which media features
// are on.
#[cfg(any(feature = "std", feature = "alloc"))]
pub use aggregates::{
  Media, MediaFile, Person, PersonConfidence, SceneAnnotation, Speaker, UserTag, WatchedLocation,
};
// Medium-specific aggregate re-exports. Gated on `std AND <medium>`
// because every `*Track` now reaches `IndexMap<SmolStr, SmolStr>` for
// its `metadata` bag and IndexMap's default hasher (`RandomState`)
// is std-only. Same constraint as the `Chapter` aggregate.
#[cfg(all(feature = "std", feature = "audio"))]
pub use aggregates::{
  Audio, AudioSegment, AudioTrack, SoundEvent, SpeechSegment, SpeechSegmentError,
  SpeechSegmentParts, Word,
};
#[cfg(all(feature = "std", feature = "video"))]
pub use aggregates::{Keyframe, Scene, Thumbnail, ThumbnailError, Video, VideoTrack};
#[cfg(all(feature = "std", feature = "subtitle"))]
pub use aggregates::{Subtitle, SubtitleCue, SubtitleTrack};
// `Chapter` is medium-independent (container-level) but std-gated like
// every IndexMap-bearing aggregate.
#[cfg(feature = "std")]
pub use aggregates::{Chapter, ChapterError};
// `Data` / `DataTrack` are likewise container-level + std-gated.
#[cfg(feature = "std")]
pub use aggregates::{Data, DataError, DataTrack, DataTrackError};
#[cfg(any(feature = "std", feature = "alloc"))]
pub use primitives::{ErrorInfo, Location};
#[cfg(any(feature = "std", feature = "alloc"))]
pub use vo::{IndexProgress, LocalizedText, Provenance, VoiceFingerprint, VoiceFingerprintError};
