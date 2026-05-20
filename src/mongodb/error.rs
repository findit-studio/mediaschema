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
    audio::{AudioError, AudioSegmentError, AudioTrackError},
    curation::{NilIdError, SceneAnnotationError},
    media::MediaError,
    speaker::SpeakerError,
    subtitle::{cue::SubtitleCueError, facet::SubtitleError, track::SubtitleTrackError},
    video::{
      facet::VideoError, keyframe::KeyframeError, scene::SceneError, track::VideoTrackError,
    },
    watched_location::WatchedLocationError,
  },
  primitives::Uuid7Error,
};

/// Backend-specific error returned when a `bson::Document` cannot be
/// decoded into a domain aggregate (missing required field, wrong
/// bson-type, invariant rejection from the underlying `try_new`, …).
#[derive(Debug, Clone, PartialEq, Eq, IsVariant)]
#[non_exhaustive]
pub enum MongoError {
  /// A required bson field was absent from the document.
  MissingField(SmolStr),
  /// A bson field was present but its type did not match the expected
  /// shape (`{field, want, got}`).
  TypeMismatch {
    field: SmolStr,
    want: &'static str,
    got: &'static str,
  },
  /// A `bson::Binary` field had the wrong byte length (`{field, want,
  /// got}`).
  WrongBinaryLen {
    field: SmolStr,
    want: usize,
    got: usize,
  },
  /// A bson integer was outside the destination integer's range.
  IntOutOfRange {
    field: SmolStr,
    value: i64,
  },
  /// A `Uuid7` round-trip rejected the binary payload (nil / non-v7).
  Uuid7(Uuid7Error),

  // Domain-aggregate `try_new` rejections (one variant per aggregate).
  Media(MediaError),
  WatchedLocation(WatchedLocationError),
  Speaker(SpeakerError),
  NilId(NilIdError),
  SceneAnnotation(SceneAnnotationError),
  Audio(AudioError),
  AudioTrack(AudioTrackError),
  AudioSegment(AudioSegmentError),
  Video(VideoError),
  VideoTrack(VideoTrackError),
  Scene(SceneError),
  Keyframe(KeyframeError),
  Subtitle(SubtitleError),
  SubtitleTrack(SubtitleTrackError),
  SubtitleCue(SubtitleCueError),
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

impl core::fmt::Display for MongoError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::MissingField(name) => write!(f, "missing required bson field `{name}`"),
      Self::TypeMismatch { field, want, got } => {
        write!(f, "field `{field}`: expected {want}, got {got}")
      }
      Self::WrongBinaryLen { field, want, got } => {
        write!(f, "field `{field}`: expected {want}-byte binary, got {got}")
      }
      Self::IntOutOfRange { field, value } => {
        write!(f, "field `{field}`: integer value {value} out of range")
      }
      Self::Uuid7(e) => write!(f, "Uuid7 decode failed: {e}"),
      Self::Media(e) => write!(f, "Media try_new rejected: {e}"),
      Self::WatchedLocation(e) => write!(f, "WatchedLocation try_new rejected: {e}"),
      Self::Speaker(e) => write!(f, "Speaker try_new rejected: {e}"),
      Self::NilId(e) => write!(f, "id rejected: {e}"),
      Self::SceneAnnotation(e) => write!(f, "SceneAnnotation try_new rejected: {e}"),
      Self::Audio(e) => write!(f, "Audio try_new rejected: {e}"),
      Self::AudioTrack(e) => write!(f, "AudioTrack try_new rejected: {e}"),
      Self::AudioSegment(e) => write!(f, "AudioSegment try_new rejected: {e}"),
      Self::Video(e) => write!(f, "Video try_new rejected: {e}"),
      Self::VideoTrack(e) => write!(f, "VideoTrack try_new rejected: {e}"),
      Self::Scene(e) => write!(f, "Scene try_new rejected: {e}"),
      Self::Keyframe(e) => write!(f, "Keyframe try_new rejected: {e}"),
      Self::Subtitle(e) => write!(f, "Subtitle try_new rejected: {e}"),
      Self::SubtitleTrack(e) => write!(f, "SubtitleTrack try_new rejected: {e}"),
      Self::SubtitleCue(e) => write!(f, "SubtitleCue try_new rejected: {e}"),
    }
  }
}

impl core::error::Error for MongoError {
  fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
    match self {
      Self::Uuid7(e) => Some(e),
      Self::Media(e) => Some(e),
      Self::WatchedLocation(e) => Some(e),
      Self::Speaker(e) => Some(e),
      Self::NilId(e) => Some(e),
      Self::SceneAnnotation(e) => Some(e),
      Self::Audio(e) => Some(e),
      Self::AudioTrack(e) => Some(e),
      Self::AudioSegment(e) => Some(e),
      Self::Video(e) => Some(e),
      Self::VideoTrack(e) => Some(e),
      Self::Scene(e) => Some(e),
      Self::Keyframe(e) => Some(e),
      Self::Subtitle(e) => Some(e),
      Self::SubtitleTrack(e) => Some(e),
      Self::SubtitleCue(e) => Some(e),
      _ => None,
    }
  }
}

// `From` impls for ergonomic `?` propagation.
impl From<Uuid7Error> for MongoError {
  #[inline]
  fn from(e: Uuid7Error) -> Self {
    Self::Uuid7(e)
  }
}
impl From<MediaError> for MongoError {
  #[inline]
  fn from(e: MediaError) -> Self {
    Self::Media(e)
  }
}
impl From<WatchedLocationError> for MongoError {
  #[inline]
  fn from(e: WatchedLocationError) -> Self {
    Self::WatchedLocation(e)
  }
}
impl From<SpeakerError> for MongoError {
  #[inline]
  fn from(e: SpeakerError) -> Self {
    Self::Speaker(e)
  }
}
impl From<NilIdError> for MongoError {
  #[inline]
  fn from(e: NilIdError) -> Self {
    Self::NilId(e)
  }
}
impl From<SceneAnnotationError> for MongoError {
  #[inline]
  fn from(e: SceneAnnotationError) -> Self {
    Self::SceneAnnotation(e)
  }
}
impl From<AudioError> for MongoError {
  #[inline]
  fn from(e: AudioError) -> Self {
    Self::Audio(e)
  }
}
impl From<AudioTrackError> for MongoError {
  #[inline]
  fn from(e: AudioTrackError) -> Self {
    Self::AudioTrack(e)
  }
}
impl From<AudioSegmentError> for MongoError {
  #[inline]
  fn from(e: AudioSegmentError) -> Self {
    Self::AudioSegment(e)
  }
}
impl From<VideoError> for MongoError {
  #[inline]
  fn from(e: VideoError) -> Self {
    Self::Video(e)
  }
}
impl From<VideoTrackError> for MongoError {
  #[inline]
  fn from(e: VideoTrackError) -> Self {
    Self::VideoTrack(e)
  }
}
impl From<SceneError> for MongoError {
  #[inline]
  fn from(e: SceneError) -> Self {
    Self::Scene(e)
  }
}
impl From<KeyframeError> for MongoError {
  #[inline]
  fn from(e: KeyframeError) -> Self {
    Self::Keyframe(e)
  }
}
impl From<SubtitleError> for MongoError {
  #[inline]
  fn from(e: SubtitleError) -> Self {
    Self::Subtitle(e)
  }
}
impl From<SubtitleTrackError> for MongoError {
  #[inline]
  fn from(e: SubtitleTrackError) -> Self {
    Self::SubtitleTrack(e)
  }
}
impl From<SubtitleCueError> for MongoError {
  #[inline]
  fn from(e: SubtitleCueError) -> Self {
    Self::SubtitleCue(e)
  }
}
