//! `SpeechSegment` — a detected, time-ranged speech-presence interval.
//!
//! The normalized, persistable value-object the VAD (voice-activity
//! detection) service emits: one speech interval on an `AudioTrack`,
//! produced in the audio `VAD_DONE` stage. The speech-presence analog of
//! [`SoundEvent`](super::sound_event::SoundEvent) — parent →
//! `AudioTrack.id` (A-loc per-track; multi-track files keep which-track
//! attribution), no progress lifecycle; segments attach via their
//! `audio_track_id` FK and the DB derives them by reverse-FK.

use derive_more::IsVariant;
use mediatime::TimeRange;

use crate::domain::Uuid7;

// ---------------------------------------------------------------------------
// Confidence validation — shared by `SpeechSegment`'s validating ctor
// ---------------------------------------------------------------------------

/// A `[0,1]`-bounded confidence is valid iff it is finite (no NaN / ±∞) and
/// within the closed unit interval. `f32::is_finite` already excludes NaN
/// and infinities.
///
/// `SpeechSegment` keeps a raw validated `f32` (mirroring
/// [`SoundEvent`](super::sound_event::SoundEvent)'s `score`) rather than the
/// video-cluster `Confidence` value-object: `Confidence` lives behind the
/// `video` feature, and `SpeechSegment` is an `audio`-feature aggregate that
/// must compile under `--no-default-features --features alloc,audio` (audio
/// without video). The invariant is identical either way.
#[inline]
const fn is_valid_confidence(confidence: f32) -> bool {
  confidence.is_finite() && confidence >= 0.0 && confidence <= 1.0
}

// ---------------------------------------------------------------------------
// SpeechSegment
// ---------------------------------------------------------------------------

/// One detected speech interval on an `AudioTrack` — the speech-presence
/// analog of [`SoundEvent`](super::sound_event::SoundEvent).
///
/// Generic over `Id` (default [`Uuid7`]). `audio_track_id` FK →
/// `AudioTrack.id` (A-loc per-track). The VAD emits a `[0,1]` `confidence`
/// for the detected interval.
///
/// Fields are private per the encapsulation rule; access via the getters.
///
/// **No `Default`** — defaulting to nil identities would be an orphan
/// segment with no track + a zero-length span at `t=0`. Use
/// [`SpeechSegment::try_new`].
///
/// **`PartialEq` only, no `Eq`** — `confidence` is an `f32`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeechSegment<Id = Uuid7> {
  id: Id,
  audio_track_id: Id,
  index: u32,
  span: TimeRange,
  confidence: f32,
}

impl SpeechSegment<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects:
  /// - nil `id` (every aggregate row needs a real identity);
  /// - nil `audio_track_id` (orphan segment with no `AudioTrack`);
  /// - an inverted `span` (`span.start > span.end`);
  /// - a `confidence` that is non-finite (NaN / ±∞) or outside `[0,1]`.
  ///
  /// Defence-in-depth on the span: `mediatime::TimeRange::new` already
  /// enforces `start <= end` (panicking) and `TimeRange::try_new` rejects
  /// the inverted case, but `TimeRange` exposes public `with_*` / `set_*`
  /// mutators that can invert a once-valid range — so `SpeechSegment`
  /// re-validates the invariant itself. The check is on **semantic** time
  /// ([`mediatime::Timestamp::cmp_semantic`], timebase-correct) rather
  /// than raw PTS, mirroring [`SoundEvent`](super::sound_event::SoundEvent).
  #[inline]
  pub fn try_new(
    id: Uuid7,
    audio_track_id: Uuid7,
    index: u32,
    span: TimeRange,
    confidence: f32,
  ) -> Result<Self, SpeechSegmentError> {
    if id.is_nil() {
      return Err(SpeechSegmentError::NilId);
    }
    if audio_track_id.is_nil() {
      return Err(SpeechSegmentError::NilAudioTrackId);
    }
    if span.start().cmp_semantic(&span.end()) == core::cmp::Ordering::Greater {
      return Err(SpeechSegmentError::InvertedSpan);
    }
    if !is_valid_confidence(confidence) {
      return Err(SpeechSegmentError::ConfidenceOutOfRange);
    }
    Ok(Self {
      id,
      audio_track_id,
      index,
      span,
      confidence,
    })
  }
}

impl<Id> SpeechSegment<Id> {
  /// Canonical identity (also the external vector-store key, if embedded).
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// FK → `AudioTrack.id`.
  #[inline(always)]
  pub const fn audio_track_id_ref(&self) -> &Id {
    &self.audio_track_id
  }

  /// 0-based segment order within the `audio_track_id` track.
  #[inline(always)]
  pub const fn index(&self) -> u32 {
    self.index
  }

  /// Detected speech window (media-time).
  #[inline(always)]
  pub const fn span_ref(&self) -> &TimeRange {
    &self.span
  }

  /// `[0,1]` speech-presence confidence (always finite).
  #[inline(always)]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }

  /// Decompose into [`SpeechSegmentParts`] — exhaustive, by value.
  #[inline(always)]
  pub fn into_parts(self) -> SpeechSegmentParts<Id> {
    let Self {
      id,
      audio_track_id,
      index,
      span,
      confidence,
    } = self;
    SpeechSegmentParts {
      id,
      audio_track_id,
      index,
      span,
      confidence,
    }
  }
}

/// Exhaustive by-value decomposition of [`SpeechSegment`] — every stored
/// field.
///
/// Public-field data-transfer struct (the conversion-boundary exception to
/// the encapsulation rule): cross-suite conversions destructure it
/// exhaustively, so adding a field breaks them at compile time instead of
/// silently dropping data.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeechSegmentParts<Id = Uuid7> {
  pub id: Id,
  pub audio_track_id: Id,
  pub index: u32,
  pub span: TimeRange,
  pub confidence: f32,
}

/// Error returned by [`SpeechSegment`]'s validating constructor when an
/// invariant cannot be upheld. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum SpeechSegmentError {
  /// Supplied `id` was the nil sentinel.
  #[error("SpeechSegment id must not be the nil UUID")]
  NilId,
  /// Supplied `audio_track_id` was the nil sentinel — orphan segment with
  /// no `AudioTrack` reference.
  #[error("SpeechSegment `audio_track_id` (FK → AudioTrack) must not be the nil UUID")]
  NilAudioTrackId,
  /// `span.start > span.end` — inverted detection window. A
  /// `mediatime::TimeRange` validates `start <= end` at construction, but
  /// its public `with_*` / `set_*` mutators can invert it afterwards, so
  /// `SpeechSegment` re-checks the invariant on every span it accepts.
  #[error("SpeechSegment span.start must be <= span.end")]
  InvertedSpan,
  /// `confidence` was non-finite (NaN / ±∞) or outside the closed `[0,1]`
  /// interval.
  #[error("SpeechSegment confidence must be finite and within [0, 1]")]
  ConfidenceOutOfRange,
}

#[cfg(all(test, feature = "std"))]
mod tests;
