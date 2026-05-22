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
pub mod primitives;
#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub mod vo;

// Always available (pure no-std no-alloc):
pub use bitflags::{AudioIndexStatus, MediaErrorFlags, SubtitleIndexStatus, VideoIndexStatus};
pub use enums::{
  AudioContentKind, AudioIndexStage, KeyframeExtractor, MediaKind, ScanStatus, SceneDetector,
  SubtitleIndexStage, SubtitleKind, VideoIndexStage,
};
pub use primitives::{ErrorCode, FileChecksum, Rgba, Uuid7};

// Heap-tier (gate on `any(std, alloc)`): aggregates and types that reach
// `Vec` / `SmolStr`.
#[cfg(any(feature = "std", feature = "alloc"))]
pub use aggregates::{
  IndexProgress, Media, MediaFile, SceneAnnotation, Speaker, Subtitle, SubtitleCue, SubtitleTrack,
  UserTag, WatchedLocation,
};
#[cfg(any(feature = "std", feature = "alloc"))]
pub use primitives::{ErrorInfo, Location};
#[cfg(any(feature = "std", feature = "alloc"))]
pub use vo::{LocalizedText, Provenance};
