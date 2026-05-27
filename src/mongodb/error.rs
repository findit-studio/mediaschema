//! Errors for bson ↔ domain conversion.
//!
//! Every fallible decode path funnels through [`MongoError`]. The enum is
//! `#[non_exhaustive]` (new failure modes may be added without a major
//! bump) and implements `core::error::Error` per the project's
//! 1.95-MSRV convention (never `std::error::Error`).

use derive_more::IsVariant;
use smol_str::SmolStr;

use crate::domain::{
  aggregates::{
    curation::{NilIdError, SceneAnnotationError},
    media::MediaError,
    speaker::SpeakerError,
    watched_location::WatchedLocationError,
  },
  primitives::{LocationError, Uuid7Error},
};
// Medium-specific error imports — gated per feature so a build with
// only one medium on does not drag in the others' error trees.
#[cfg(feature = "audio")]
use crate::domain::aggregates::audio::{
  segment::WordError, AudioError, AudioSegmentError, AudioTrackError,
};
#[cfg(feature = "subtitle")]
use crate::domain::aggregates::subtitle::{
  cue::SubtitleCueError, facet::SubtitleError, track::SubtitleTrackError,
};
#[cfg(feature = "video")]
use crate::domain::aggregates::video::{
  detections::DetectionError, facet::VideoError, keyframe::KeyframeError, scene::SceneError,
  track::VideoTrackError,
};

/// Backend-specific error returned when a `bson::Document` cannot be
/// decoded into a domain aggregate (missing required field, wrong
/// bson-type, invariant rejection from the underlying `try_new`, …).
// `Eq` is intentionally not derived: the `GeoLocation` variant wraps
// `mediaframe::capture::GeoLocationError`, which carries the rejected
// `f64` lat/lon and so is only `PartialEq` (NaN ≠ NaN). `PartialEq` is
// enough for the test-suite's `assert_eq!`-on-`unwrap_err()` patterns.
#[derive(Debug, Clone, PartialEq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum MongoError {
  /// A required bson field was absent from the document.
  #[error("missing required bson field `{0}`")]
  MissingField(SmolStr),
  /// A bson field was present but its type did not match the expected
  /// shape (`{field, want, got}`).
  #[error("field `{field}`: expected {want}, got {got}")]
  TypeMismatch {
    field: SmolStr,
    want: &'static str,
    got: &'static str,
  },
  /// A `bson::Binary` field had the wrong byte length (`{field, want,
  /// got}`).
  #[error("field `{field}`: expected {want}-byte binary, got {got}")]
  WrongBinaryLen {
    field: SmolStr,
    want: usize,
    got: usize,
  },
  /// A bson integer was outside the destination integer's range.
  #[error("field `{field}`: integer value {value} out of range")]
  IntOutOfRange { field: SmolStr, value: i64 },
  /// A `Uuid7` round-trip rejected the binary payload (nil / non-v7).
  #[error("Uuid7 decode failed: {0}")]
  Uuid7(#[from] Uuid7Error),
  /// A `{volume, components}` document violated a [`Location`]
  /// invariant (nil volume / empty path).
  ///
  /// [`Location`]: crate::domain::primitives::Location
  #[error("Location decode failed: {0}")]
  Location(#[from] LocationError),

  // `mediaframe` value-object rejections surfaced at the bson edge —
  // these flow up from the typed descriptor / VO decoders (`Language`
  // from a BCP-47 string, `GeoLocation` lat/lon ranges, `Fingerprint`
  // empty-algorithm, `CoverArt` empty mime/data).
  /// A BCP-47 string failed to decode into a [`mediaframe::lang::Language`].
  #[error("Language decode failed: {0}")]
  Language(#[from] ::mediaframe::lang::LanguageError),
  /// A `{lat, lon, altitude}` document violated a
  /// [`mediaframe::capture::GeoLocation`] invariant.
  #[error("GeoLocation decode failed: {0}")]
  GeoLocation(#[from] ::mediaframe::capture::GeoLocationError),
  /// An `{algorithm, value}` document violated a
  /// [`mediaframe::audio::Fingerprint`] invariant (empty algorithm).
  #[error("audio Fingerprint decode failed: {0}")]
  Fingerprint(#[from] ::mediaframe::audio::FingerprintError),
  /// A `{mime, data}` document violated a
  /// [`mediaframe::audio::CoverArt`] invariant (empty mime / data).
  #[error("audio CoverArt decode failed: {0}")]
  CoverArt(#[from] ::mediaframe::audio::CoverArtError),

  // Domain-aggregate `try_new` rejections (one variant per aggregate).
  #[error("Media try_new rejected: {0}")]
  Media(#[from] MediaError),
  #[error("WatchedLocation try_new rejected: {0}")]
  WatchedLocation(#[from] WatchedLocationError),
  #[error("Speaker try_new rejected: {0}")]
  Speaker(#[from] SpeakerError),
  #[error("id rejected: {0}")]
  NilId(#[from] NilIdError),
  #[error("SceneAnnotation try_new rejected: {0}")]
  SceneAnnotation(#[from] SceneAnnotationError),
  #[cfg(feature = "audio")]
  #[error("Audio try_new rejected: {0}")]
  Audio(#[from] AudioError),
  #[cfg(feature = "audio")]
  #[error("AudioTrack try_new rejected: {0}")]
  AudioTrack(#[from] AudioTrackError),
  #[cfg(feature = "audio")]
  #[error("AudioSegment try_new rejected: {0}")]
  AudioSegment(#[from] AudioSegmentError),
  #[cfg(feature = "audio")]
  #[error("Word try_from_parts rejected: {0}")]
  Word(#[from] WordError),
  #[cfg(feature = "video")]
  #[error("Video try_new rejected: {0}")]
  Video(#[from] VideoError),
  #[cfg(feature = "video")]
  #[error("VideoTrack try_new rejected: {0}")]
  VideoTrack(#[from] VideoTrackError),
  #[cfg(feature = "video")]
  #[error("Scene try_new rejected: {0}")]
  Scene(#[from] SceneError),
  #[cfg(feature = "video")]
  #[error("Keyframe try_new rejected: {0}")]
  Keyframe(#[from] KeyframeError),
  #[cfg(feature = "video")]
  #[error("keyframe detection VO try_new rejected: {0}")]
  Detection(#[from] DetectionError),
  #[cfg(feature = "subtitle")]
  #[error("Subtitle try_new rejected: {0}")]
  Subtitle(#[from] SubtitleError),
  #[cfg(feature = "subtitle")]
  #[error("SubtitleTrack try_new rejected: {0}")]
  SubtitleTrack(#[from] SubtitleTrackError),
  #[cfg(feature = "subtitle")]
  #[error("SubtitleCue try_new rejected: {0}")]
  SubtitleCue(#[from] SubtitleCueError),
}

impl MongoError {
  /// Convenience: build a [`MongoError::MissingField`] from any
  /// `Into<SmolStr>`.
  #[inline]
  pub fn missing(field: impl Into<SmolStr>) -> Self {
    Self::MissingField(field.into())
  }

  /// Convenience: build a [`MongoError::TypeMismatch`].
  #[inline]
  pub fn type_mismatch(field: impl Into<SmolStr>, want: &'static str, got: &'static str) -> Self {
    Self::TypeMismatch {
      field: field.into(),
      want,
      got,
    }
  }
}
