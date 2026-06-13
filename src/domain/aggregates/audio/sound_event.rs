//! `SoundEvent` — a detected, time-ranged sound classification.
//!
//! Locked `schema/sound_events.md`. The audio analog of `Scene`: one
//! sound-event detection on an `AudioTrack`, produced by a **CED**
//! (sound-event detector) in the audio `CED_DONE` stage. Parent →
//! `AudioTrack.id` (A-loc per-track; multi-track files keep which-track
//! attribution). No progress lifecycle; sound events attach via their
//! `audio_track_id` FK and the DB derives them by reverse-FK.
//!
//! `Provenance` is per-track (on `AudioTrack`), not per sound event.

use derive_more::IsVariant;
use mediatime::TimeRange;
use smol_str::SmolStr;

use crate::domain::{CedDetector, Uuid7};

// ---------------------------------------------------------------------------
// Score validation — shared by `SoundEvent`'s validating ctor / mutators
// ---------------------------------------------------------------------------

/// A `[0,1]`-bounded probability/confidence score is valid iff it is finite
/// (no NaN / ±∞) and within the closed unit interval. `f32::is_finite`
/// already excludes NaN and infinities.
///
/// `SoundEvent` keeps a raw validated `f32` (mirroring this module's
/// [`Word`](super::segment::Word)) rather than the video-cluster
/// `Confidence` value-object: `Confidence` lives behind the `video`
/// feature, and `SoundEvent` is an `audio`-feature aggregate that must
/// compile under `--no-default-features --features alloc,audio` (audio
/// without video). The invariant is identical either way.
#[inline]
const fn is_valid_score(score: f32) -> bool {
  score.is_finite() && score >= 0.0 && score <= 1.0
}

// ---------------------------------------------------------------------------
// SoundEvent
// ---------------------------------------------------------------------------

/// One detected sound event on an `AudioTrack` — the audio analog of
/// [`Scene`](crate::domain::Scene).
///
/// Generic over `Id` (default [`Uuid7`]). `audio_track_id` FK →
/// `AudioTrack.id` (A-loc per-track). The CED emits a `label` (class name,
/// e.g. `"Speech"`), an optional stable soundevents `code` (`None` =
/// unmapped class), and a `[0,1]` `score`.
///
/// Fields are private per the encapsulation rule; access via the getters
/// and `with_*` / `set_*` accessors. Identity (`id`) and the parent FK
/// (`audio_track_id`) have no mutators.
///
/// **No `Default`** — defaulting to nil identities would be an orphan event
/// with no track + a zero-length span at `t=0`. Use [`SoundEvent::try_new`].
///
/// **`PartialEq` only, no `Eq`** — `score` is an `f32`.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundEvent<Id = Uuid7> {
  id: Id,
  audio_track_id: Id,
  index: u32,
  span: TimeRange,
  label: SmolStr,
  code: Option<u64>,
  score: f32,
  detector: CedDetector,
}

impl SoundEvent<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects:
  /// - nil `id` (every aggregate row needs a real identity);
  /// - nil `audio_track_id` (orphan event with no `AudioTrack`);
  /// - an inverted `span` (`span.start > span.end`);
  /// - a `score` that is non-finite (NaN / ±∞) or outside `[0,1]`.
  ///
  /// Defence-in-depth on the span: `mediatime::TimeRange::new` already
  /// enforces `start <= end` (panicking) and `TimeRange::try_new` rejects
  /// the inverted case, but `TimeRange` exposes public `with_*` / `set_*`
  /// mutators that can invert a once-valid range — so `SoundEvent`
  /// re-validates the invariant itself. The check is on **semantic** time
  /// ([`mediatime::Timestamp::cmp_semantic`], timebase-correct) rather
  /// than raw PTS, mirroring [`AudioSegment`](super::segment::AudioSegment).
  #[allow(clippy::too_many_arguments)]
  pub fn try_new(
    id: Uuid7,
    audio_track_id: Uuid7,
    index: u32,
    span: TimeRange,
    label: impl Into<SmolStr>,
    code: Option<u64>,
    score: f32,
    detector: CedDetector,
  ) -> Result<Self, SoundEventError> {
    if id.is_nil() {
      return Err(SoundEventError::NilId);
    }
    if audio_track_id.is_nil() {
      return Err(SoundEventError::NilAudioTrackId);
    }
    if span.start().cmp_semantic(&span.end()) == core::cmp::Ordering::Greater {
      return Err(SoundEventError::InvertedSpan);
    }
    if !is_valid_score(score) {
      return Err(SoundEventError::ScoreOutOfRange);
    }
    Ok(Self {
      id,
      audio_track_id,
      index,
      span,
      label: label.into(),
      code,
      score,
      detector,
    })
  }
}

impl<Id> SoundEvent<Id> {
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

  /// 0-based sound-event order within the `audio_track_id` track.
  #[inline(always)]
  pub const fn index(&self) -> u32 {
    self.index
  }

  /// Detected window (media-time).
  #[inline(always)]
  pub const fn span_ref(&self) -> &TimeRange {
    &self.span
  }

  /// CED class name (e.g. `"Speech"`; `""` = absent).
  #[inline(always)]
  pub fn label(&self) -> &str {
    self.label.as_str()
  }

  /// Stable soundevents dataset code (`None` = unmapped class).
  #[inline(always)]
  pub const fn code(&self) -> Option<u64> {
    self.code
  }

  /// `[0,1]` detection score (always finite).
  #[inline(always)]
  pub const fn score(&self) -> f32 {
    self.score
  }

  /// Which detector raised this sound event.
  #[inline(always)]
  pub const fn detector(&self) -> CedDetector {
    self.detector
  }

  // ----- Builders -----------------------------------------------------------

  /// Builder: replace `index`.
  #[inline(always)]
  #[must_use]
  pub const fn with_index(mut self, index: u32) -> Self {
    self.index = index;
    self
  }

  /// Fallible builder: replace `span`, re-validating the
  /// `start <= end` invariant (on semantic time). Rejects an inverted span
  /// (`mediatime::TimeRange`'s own `with_*` / `set_*` mutators can produce
  /// one) with [`SoundEventError::InvertedSpan`]. On rejection `self` is
  /// returned unchanged inside the `Err`.
  #[inline]
  pub fn try_with_span(mut self, span: TimeRange) -> Result<Self, SoundEventError> {
    if span.start().cmp_semantic(&span.end()) == core::cmp::Ordering::Greater {
      return Err(SoundEventError::InvertedSpan);
    }
    self.span = span;
    Ok(self)
  }

  /// Builder: replace `label`.
  #[inline(always)]
  #[must_use]
  pub fn with_label(mut self, v: impl Into<SmolStr>) -> Self {
    self.label = v.into();
    self
  }

  /// Builder: replace `code`.
  #[inline(always)]
  #[must_use]
  pub const fn with_code(mut self, v: Option<u64>) -> Self {
    self.code = v;
    self
  }

  /// Fallible builder: replace `score`, re-validating the finite-`[0,1]`
  /// invariant. Rejects a non-finite / out-of-range value with
  /// [`SoundEventError::ScoreOutOfRange`].
  ///
  /// Not `const` — the error path drops `self`, which is not permitted in
  /// a `const fn`.
  #[inline]
  pub fn try_with_score(mut self, score: f32) -> Result<Self, SoundEventError> {
    if !is_valid_score(score) {
      return Err(SoundEventError::ScoreOutOfRange);
    }
    self.score = score;
    Ok(self)
  }

  /// Builder: replace `detector`.
  #[inline(always)]
  #[must_use]
  pub const fn with_detector(mut self, detector: CedDetector) -> Self {
    self.detector = detector;
    self
  }

  // ----- Setters ------------------------------------------------------------

  /// In-place mutator for `index`.
  #[inline(always)]
  pub const fn set_index(&mut self, index: u32) -> &mut Self {
    self.index = index;
    self
  }

  /// Fallible in-place mutator for `span`, re-validating the `start <= end`
  /// invariant (on semantic time). On success returns `&mut Self` so it
  /// chains; on an inverted span returns [`SoundEventError::InvertedSpan`]
  /// and leaves `self` unchanged.
  #[inline]
  pub const fn try_set_span(&mut self, span: TimeRange) -> Result<&mut Self, SoundEventError> {
    if let core::cmp::Ordering::Greater = span.start().cmp_semantic(&span.end()) {
      return Err(SoundEventError::InvertedSpan);
    }
    self.span = span;
    Ok(self)
  }

  /// In-place mutator for `label`.
  #[inline(always)]
  pub fn set_label(&mut self, v: impl Into<SmolStr>) -> &mut Self {
    self.label = v.into();
    self
  }

  /// In-place mutator for `code`.
  #[inline(always)]
  pub const fn set_code(&mut self, v: Option<u64>) -> &mut Self {
    self.code = v;
    self
  }

  /// Fallible in-place mutator for `score`, re-validating the finite-`[0,1]`
  /// invariant. On rejection `self` is left unchanged.
  #[inline]
  pub const fn try_set_score(&mut self, score: f32) -> Result<&mut Self, SoundEventError> {
    if !is_valid_score(score) {
      return Err(SoundEventError::ScoreOutOfRange);
    }
    self.score = score;
    Ok(self)
  }

  /// In-place mutator for `detector`.
  #[inline(always)]
  pub const fn set_detector(&mut self, detector: CedDetector) -> &mut Self {
    self.detector = detector;
    self
  }
}

/// Error returned by [`SoundEvent`]'s validating constructor and fallible
/// mutators when an invariant cannot be upheld. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum SoundEventError {
  /// Supplied `id` was the nil sentinel.
  #[error("SoundEvent id must not be the nil UUID")]
  NilId,
  /// Supplied `audio_track_id` was the nil sentinel — orphan event with no
  /// `AudioTrack` reference.
  #[error("SoundEvent `audio_track_id` (FK → AudioTrack) must not be the nil UUID")]
  NilAudioTrackId,
  /// `span.start > span.end` — inverted detection window. A
  /// `mediatime::TimeRange` validates `start <= end` at construction, but
  /// its public `with_*` / `set_*` mutators can invert it afterwards, so
  /// `SoundEvent` re-checks the invariant on every span it accepts.
  #[error("SoundEvent span.start must be <= span.end")]
  InvertedSpan,
  /// `score` was non-finite (NaN / ±∞) or outside the closed `[0,1]`
  /// interval.
  #[error("SoundEvent score must be finite and within [0, 1]")]
  ScoreOutOfRange,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use core::num::NonZeroU32;
  use mediatime::Timebase;

  fn tb() -> Timebase {
    // A standard 1/1000 timebase (millisecond ticks) is enough for the
    // domain-level invariant tests; the exact timebase isn't material.
    Timebase::new(1, NonZeroU32::new(1000).expect("nonzero"))
  }

  fn span(start_ticks: i64, end_ticks: i64) -> TimeRange {
    TimeRange::new(start_ticks, end_ticks, tb())
  }

  #[test]
  fn try_new_happy_path() {
    let audio_track_id = Uuid7::new();
    let e = SoundEvent::try_new(
      Uuid7::new(),
      audio_track_id,
      0,
      span(0, 1500),
      "Speech",
      Some(0),
      0.87,
      CedDetector::Ced,
    )
    .expect("valid construction must succeed");
    assert_eq!(e.audio_track_id_ref(), &audio_track_id);
    assert_eq!(e.index(), 0);
    assert_eq!(e.span_ref(), &span(0, 1500));
    assert_eq!(e.label(), "Speech");
    assert_eq!(e.code(), Some(0));
    assert!((e.score() - 0.87).abs() < f32::EPSILON);
    assert!(e.detector().is_ced());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = SoundEvent::try_new(
      Uuid7::nil(),
      Uuid7::new(),
      0,
      span(0, 1500),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    );
    assert_eq!(r.err(), Some(SoundEventError::NilId));
    assert!(SoundEventError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_audio_track_id() {
    let r = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::nil(),
      0,
      span(0, 1500),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    );
    assert_eq!(r.err(), Some(SoundEventError::NilAudioTrackId));
    assert!(SoundEventError::NilAudioTrackId.is_nil_audio_track_id());
  }

  #[test]
  fn try_new_rejects_inverted_span() {
    // `TimeRange::try_new` rejects an inverted span at construction...
    assert!(TimeRange::try_new(2000, 1000, tb()).is_none());
    // ...but its public `with_*` mutators can invert a *valid* range after
    // the fact — `SoundEvent::try_new` must re-validate.
    let inverted = TimeRange::new(1_000, 5_000, tb()).with_end(0);
    assert!(inverted.start_pts() > inverted.end_pts());
    let r = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      inverted,
      "Speech",
      None,
      0.5,
      CedDetector::Manual,
    );
    assert_eq!(r.err(), Some(SoundEventError::InvertedSpan));
    assert!(SoundEventError::InvertedSpan.is_inverted_span());
  }

  #[test]
  fn try_new_accepts_zero_length_span() {
    // `start == end` is allowed (locked invariant: `start <= end`).
    SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(500, 500),
      "Doorbell",
      Some(42),
      1.0,
      CedDetector::Ced,
    )
    .expect("zero-length span ok");
  }

  #[test]
  fn try_new_rejects_non_finite_or_out_of_range_score() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 1.1] {
      let r = SoundEvent::try_new(
        Uuid7::new(),
        Uuid7::new(),
        0,
        span(0, 100),
        "Speech",
        None,
        bad,
        CedDetector::Ced,
      );
      assert_eq!(r.err(), Some(SoundEventError::ScoreOutOfRange));
    }
    // boundary values are accepted
    assert!(SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 100),
      "x",
      None,
      0.0,
      CedDetector::Ced
    )
    .is_ok());
    assert!(SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 100),
      "x",
      None,
      1.0,
      CedDetector::Ced
    )
    .is_ok());
    assert!(SoundEventError::ScoreOutOfRange.is_score_out_of_range());
  }

  #[test]
  fn builders_chain() {
    let e = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 500),
      "Speech",
      Some(0),
      0.5,
      CedDetector::Ced,
    )
    .unwrap()
    .with_index(3)
    .with_label("Music")
    .with_code(Some(137))
    .with_detector(CedDetector::Manual)
    .try_with_score(0.9)
    .unwrap()
    .try_with_span(span(100, 800))
    .unwrap();
    assert_eq!(e.index(), 3);
    assert_eq!(e.label(), "Music");
    assert_eq!(e.code(), Some(137));
    assert!(e.detector().is_manual());
    assert!((e.score() - 0.9).abs() < f32::EPSILON);
    assert_eq!(e.span_ref(), &span(100, 800));
  }

  #[test]
  fn try_with_score_and_try_set_score_validate() {
    let e = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 500),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    )
    .unwrap();
    assert!(e.clone().try_with_score(f32::NAN).is_err());
    assert!(e.clone().try_with_score(2.0).is_err());
    assert!(e.clone().try_with_score(0.8).is_ok());

    let mut e = e;
    assert_eq!(
      e.try_set_score(f32::NEG_INFINITY).err(),
      Some(SoundEventError::ScoreOutOfRange)
    );
    // rejection leaves the prior valid value in place
    assert!((e.score() - 0.5).abs() < f32::EPSILON);
    e.try_set_score(0.25).unwrap();
    assert!((e.score() - 0.25).abs() < f32::EPSILON);
  }

  #[test]
  fn try_set_span_rejects_post_construction_inversion() {
    let mut e = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 5_000),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    )
    .unwrap();
    let mut inverted = TimeRange::new(2_000, 8_000, tb());
    inverted.set_start(9_000);
    assert_eq!(
      e.try_set_span(inverted).err(),
      Some(SoundEventError::InvertedSpan)
    );
    // a valid replacement span is accepted
    e.try_set_span(span(100, 200)).unwrap();
    assert_eq!(e.span_ref(), &span(100, 200));
  }

  #[test]
  fn setters_mutate_in_place() {
    let mut e = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 500),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    )
    .unwrap();
    e.set_index(7);
    e.set_label("Dog");
    e.set_code(Some(74));
    e.set_detector(CedDetector::Manual);
    e.try_set_score(0.33).unwrap();
    assert_eq!(e.index(), 7);
    assert_eq!(e.label(), "Dog");
    assert_eq!(e.code(), Some(74));
    assert!(e.detector().is_manual());
    assert!((e.score() - 0.33).abs() < f32::EPSILON);
  }

  #[test]
  fn into_parts_exposes_every_field() {
    let track = Uuid7::new();
    let e = SoundEvent::try_new(
      Uuid7::new(),
      track,
      2,
      span(100, 900),
      "Siren",
      Some(316),
      0.42,
      CedDetector::Ced,
    )
    .unwrap();
    let parts = e.into_parts();
    assert_eq!(parts.audio_track_id, track);
    assert_eq!(parts.index, 2);
    assert_eq!(parts.span, span(100, 900));
    assert_eq!(parts.label, SmolStr::new("Siren"));
    assert_eq!(parts.code, Some(316));
    assert!((parts.score - 0.42).abs() < f32::EPSILON);
    assert_eq!(parts.detector, CedDetector::Ced);
  }

  // `rehydrate` is gated behind `all(std, video, audio, subtitle)` (it is
  // reserved for `crate::graph`, which requires all three media); the
  // `into_parts` → `rehydrate` round-trip is therefore tested under the
  // same gate, mirroring where `AudioSegment` / `Scene` exercise theirs.
  #[cfg(all(feature = "video", feature = "subtitle"))]
  #[test]
  fn into_parts_rehydrate_round_trip() {
    let e = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      2,
      span(100, 900),
      "Siren",
      Some(316),
      0.42,
      CedDetector::Ced,
    )
    .unwrap();
    let back = SoundEvent::rehydrate(e.clone().into_parts());
    assert_eq!(back, e);
  }
}

/// Exhaustive by-value decomposition of [`SoundEvent`] — every stored
/// field.
///
/// Public-field data-transfer struct (the conversion-boundary exception to
/// the encapsulation rule): cross-suite conversions (`crate::graph`)
/// destructure it exhaustively, so adding a field breaks them at compile
/// time instead of silently dropping data.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundEventParts<Id = Uuid7> {
  pub id: Id,
  pub audio_track_id: Id,
  pub index: u32,
  pub span: TimeRange,
  pub label: SmolStr,
  pub code: Option<u64>,
  pub score: f32,
  pub detector: CedDetector,
}

impl<Id> SoundEvent<Id> {
  /// Decompose into [`SoundEventParts`] — exhaustive, by value.
  #[inline(always)]
  pub fn into_parts(self) -> SoundEventParts<Id> {
    let Self {
      id,
      audio_track_id,
      index,
      span,
      label,
      code,
      score,
      detector,
    } = self;
    SoundEventParts {
      id,
      audio_track_id,
      index,
      span,
      label,
      code,
      score,
      detector,
    }
  }
}

impl<Id> SoundEvent<Id> {
  /// Invariant-carrying constructor from [`SoundEventParts`] —
  /// `pub(crate)`, reserved for in-crate conversions from already-validated
  /// values (`crate::graph`).
  #[cfg(all(
    feature = "std",
    feature = "video",
    feature = "audio",
    feature = "subtitle"
  ))]
  #[inline(always)]
  pub(crate) fn rehydrate(parts: SoundEventParts<Id>) -> Self {
    let SoundEventParts {
      id,
      audio_track_id,
      index,
      span,
      label,
      code,
      score,
      detector,
    } = parts;
    Self {
      id,
      audio_track_id,
      index,
      span,
      label,
      code,
      score,
      detector,
    }
  }
}
