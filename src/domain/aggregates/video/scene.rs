//! `Scene` — one described time segment of a video stream
//! (locked `schema/scene.md` r6 + r7 reopen).
//!
//! Parent → `VideoTrack` (scene detection is per-stream). Keyframes
//! **are** the scene's thumbnails. **No `provenance`** — hoisted to
//! `VideoTrack` in rev 7; every `Scene` inside a `VideoTrack` shares
//! the track's `Provenance`.

use std::vec::Vec;

use derive_more::IsVariant;
use mediatime::TimeRange;
use smol_str::SmolStr;

use crate::domain::{SceneDetector, Uuid7};

/// One described time segment of a video stream (e.g. ~5–10 s captioned
/// "Jane is eating"). Immutable detected+analysed record — user
/// curation lives in the separate mutable `SceneAnnotation` layer.
///
/// Generic over `Id` (default [`Uuid7`]). Fields are private per the
/// encapsulation rule; access via the getter and `with_*` / `set_*`
/// accessors.
///
/// **No `Default`** — defaulting to nil `id`/`video_track_id` would be an
/// orphan segment with no track. Construct via [`Scene::try_new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scene<Id = Uuid7> {
  id: Id,
  video_track_id: Id,
  index: u32,
  span: TimeRange,
  detector: SceneDetector,
  keyframes: Vec<Id>,
  description: SmolStr,
}

impl Scene<Uuid7> {
  /// Validating constructor.
  ///
  /// Rejects:
  /// - nil `id`,
  /// - nil `video_track_id`,
  /// - an inverted `span` (`start_pts > end_pts`).
  ///
  /// `mediatime::TimeRange::try_new` rejects an inverted span at
  /// construction, but `TimeRange` also exposes public `with_*`/`set_*`
  /// mutators, so a caller can hand `Scene` a `TimeRange` that *was*
  /// valid and has since been inverted. `Scene` therefore re-validates
  /// the `start <= end` invariant itself rather than trusting upstream.
  pub fn try_new(
    id: Uuid7,
    video_track_id: Uuid7,
    index: u32,
    span: TimeRange,
    detector: SceneDetector,
  ) -> Result<Self, SceneError> {
    if id.is_nil() {
      return Err(SceneError::NilId);
    }
    if video_track_id.is_nil() {
      return Err(SceneError::NilVideoTrackId);
    }
    if span.start_pts() > span.end_pts() {
      return Err(SceneError::InvertedSpan);
    }
    Ok(Self {
      id,
      video_track_id,
      index,
      span,
      detector,
      keyframes: Vec::new(),
      description: SmolStr::default(),
    })
  }

  /// Builder: replace the `keyframes` child-ref id-list.
  #[must_use]
  #[inline(always)]
  pub fn with_keyframes(mut self, kfs: impl Into<Vec<Uuid7>>) -> Self {
    self.keyframes = kfs.into();
    self
  }

  /// In-place mutator for the `keyframes` child-ref id-list.
  #[inline(always)]
  pub fn set_keyframes(&mut self, kfs: impl Into<Vec<Uuid7>>) -> &mut Self {
    self.keyframes = kfs.into();
    self
  }
}

impl<Id> Scene<Id> {
  /// Canonical identity (also the LanceDB vector key).
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// FK → `VideoTrack.id`.
  #[inline(always)]
  pub const fn video_track_id_ref(&self) -> &Id {
    &self.video_track_id
  }

  /// 0-based scene order within the track.
  #[inline(always)]
  pub const fn index(&self) -> u32 {
    self.index
  }

  /// Media-time span (`mediatime::TimeRange`).
  #[inline(always)]
  pub const fn span_ref(&self) -> &TimeRange {
    &self.span
  }

  /// Which detector raised this scene.
  #[inline(always)]
  pub const fn detector(&self) -> SceneDetector {
    self.detector
  }

  /// Refs → child [`Keyframe`](super::keyframe::Keyframe)s — these
  /// **are** the scene's thumbnails.
  #[inline(always)]
  pub const fn keyframes_slice(&self) -> &[Id] {
    self.keyframes.as_slice()
  }

  /// VLM-generated description (`""` = none).
  #[inline(always)]
  pub fn description(&self) -> &str {
    self.description.as_str()
  }

  /// Builder: replace `index`.
  #[must_use]
  #[inline(always)]
  pub const fn with_index(mut self, index: u32) -> Self {
    self.index = index;
    self
  }

  /// Fallible builder: replace `span`, re-validating the
  /// `start_pts <= end_pts` invariant. Rejects an inverted span
  /// (`mediatime::TimeRange`'s own `with_*`/`set_*` mutators can produce
  /// one) with [`SceneError::InvertedSpan`].
  #[inline]
  pub fn try_with_span(mut self, span: TimeRange) -> Result<Self, SceneError> {
    if span.start_pts() > span.end_pts() {
      return Err(SceneError::InvertedSpan);
    }
    self.span = span;
    Ok(self)
  }

  /// Fallible in-place mutator for `span`, re-validating the
  /// `start_pts <= end_pts` invariant. On success returns `&mut Self`
  /// so it chains; on an inverted span returns
  /// [`SceneError::InvertedSpan`] and leaves `self` unchanged.
  #[inline]
  pub const fn try_set_span(&mut self, span: TimeRange) -> Result<&mut Self, SceneError> {
    if span.start_pts() > span.end_pts() {
      return Err(SceneError::InvertedSpan);
    }
    self.span = span;
    Ok(self)
  }

  /// Builder: replace `detector`.
  #[must_use]
  #[inline(always)]
  pub const fn with_detector(mut self, detector: SceneDetector) -> Self {
    self.detector = detector;
    self
  }

  /// Builder: replace `description`.
  #[must_use]
  #[inline(always)]
  pub fn with_description(mut self, description: impl Into<SmolStr>) -> Self {
    self.description = description.into();
    self
  }

  /// In-place mutator for `index`.
  #[inline(always)]
  pub const fn set_index(&mut self, index: u32) -> &mut Self {
    self.index = index;
    self
  }

  /// In-place mutator for `detector`.
  #[inline(always)]
  pub const fn set_detector(&mut self, detector: SceneDetector) -> &mut Self {
    self.detector = detector;
    self
  }

  /// In-place mutator for `description`.
  #[inline(always)]
  pub fn set_description(&mut self, description: impl Into<SmolStr>) -> &mut Self {
    self.description = description.into();
    self
  }
}

/// Error returned when a [`Scene`] constructor or fallible mutator
/// cannot uphold a locked invariant. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum SceneError {
  /// Supplied `id` was the nil sentinel.
  #[error("Scene id must not be the nil UUID")]
  NilId,
  /// Supplied `video_track_id` was the nil sentinel — orphan scene with no
  /// `VideoTrack`.
  #[error("Scene `video_track_id` (FK → VideoTrack) must not be the nil UUID")]
  NilVideoTrackId,
  /// Supplied `span` was inverted (`start_pts > end_pts`). A
  /// `mediatime::TimeRange` validates `start <= end` at construction,
  /// but its public `with_*`/`set_*` mutators can invert it afterwards,
  /// so `Scene` re-checks the invariant on every span it accepts.
  #[error("Scene span must not be inverted (start_pts <= end_pts)")]
  InvertedSpan,
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
    // 1/1000 (millisecond timebase) — the value doesn't matter for
    // invariant tests, just that it's a valid `Timebase`.
    Timebase::new(1, NonZeroU32::new(1000).unwrap())
  }

  #[test]
  fn try_new_happy_path() {
    let video_track_id = Uuid7::new();
    let span = TimeRange::new(5_000, 10_000, tb());
    let s = Scene::try_new(
      Uuid7::new(),
      video_track_id,
      0,
      span,
      SceneDetector::Adaptive,
    )
    .unwrap();
    assert_eq!(s.video_track_id_ref(), &video_track_id);
    assert_eq!(s.index(), 0);
    assert_eq!(s.span_ref(), &span);
    assert!(s.detector().is_adaptive());
    assert!(s.keyframes_slice().is_empty());
    assert!(s.description().is_empty());
  }

  #[test]
  fn try_new_rejects_nil_id_or_parent() {
    let span = TimeRange::new(0, 100, tb());
    assert_eq!(
      Scene::try_new(Uuid7::nil(), Uuid7::new(), 0, span, SceneDetector::Manual).err(),
      Some(SceneError::NilId)
    );
    assert_eq!(
      Scene::try_new(Uuid7::new(), Uuid7::nil(), 0, span, SceneDetector::Manual).err(),
      Some(SceneError::NilVideoTrackId)
    );
    assert!(SceneError::NilId.is_nil_id());
    assert!(SceneError::NilVideoTrackId.is_nil_video_track_id());
  }

  #[test]
  fn try_new_rejects_inverted_span() {
    // `TimeRange::try_new` rejects an inverted span at construction...
    assert!(TimeRange::try_new(2000, 1000, tb()).is_none());
    // ...but its public `with_*` mutators can invert a *valid* range
    // after the fact — `Scene::try_new` must re-validate.
    let inverted = TimeRange::new(1_000, 5_000, tb()).with_end(0);
    assert!(inverted.start_pts() > inverted.end_pts());
    assert_eq!(
      Scene::try_new(
        Uuid7::new(),
        Uuid7::new(),
        0,
        inverted,
        SceneDetector::Manual
      )
      .err(),
      Some(SceneError::InvertedSpan)
    );
    assert!(SceneError::InvertedSpan.is_inverted_span());
  }

  #[test]
  fn try_set_span_rejects_post_construction_inversion() {
    let span = TimeRange::new(0, 5_000, tb());
    let mut s = Scene::try_new(Uuid7::new(), Uuid7::new(), 0, span, SceneDetector::Manual).unwrap();

    // A mutated-to-inverted TimeRange is rejected, and `self` is left
    // unchanged.
    let mut inverted = TimeRange::new(2_000, 8_000, tb());
    inverted.set_start(9_000);
    assert_eq!(
      s.try_set_span(inverted).err(),
      Some(SceneError::InvertedSpan)
    );
    assert_eq!(s.span_ref(), &span);

    // A valid replacement span is accepted.
    let next = TimeRange::new(100, 200, tb());
    s.try_set_span(next).unwrap();
    assert_eq!(s.span_ref(), &next);

    // Same for the consuming builder.
    let inverted2 = TimeRange::new(3_000, 9_000, tb()).with_start(10_000);
    assert_eq!(
      s.clone().try_with_span(inverted2).err(),
      Some(SceneError::InvertedSpan)
    );
  }

  #[test]
  fn instantaneous_span_is_allowed() {
    // start == end is allowed (`<=` invariant).
    let span = TimeRange::new(7_000, 7_000, tb());
    let s = Scene::try_new(Uuid7::new(), Uuid7::new(), 0, span, SceneDetector::Manual)
      .expect("instantaneous span allowed");
    assert_eq!(s.span_ref().start_pts(), s.span_ref().end_pts());
  }

  #[test]
  fn builders_and_setters_chain() {
    let span = TimeRange::new(0, 5_000, tb());
    let kf = Uuid7::new();
    let s = Scene::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span,
      SceneDetector::Histogram,
    )
    .unwrap()
    .with_index(3)
    .with_detector(SceneDetector::Content)
    .with_keyframes(std::vec![kf])
    .with_description("Jane is eating");
    assert_eq!(s.index(), 3);
    assert!(s.detector().is_content());
    assert_eq!(s.keyframes_slice(), &[kf]);
    assert_eq!(s.description(), "Jane is eating");

    let mut s = s;
    s.set_index(0);
    s.set_description("");
    s.set_keyframes(Vec::<Uuid7>::new());
    s.set_detector(SceneDetector::Manual);
    assert_eq!(s.index(), 0);
    assert!(s.description().is_empty());
    assert!(s.keyframes_slice().is_empty());
    assert!(s.detector().is_manual());
  }
}

/// Exhaustive by-value decomposition of [`Scene`] — every stored field.
///
/// Public-field data-transfer struct (the conversion-boundary exception
/// to the encapsulation rule): cross-suite conversions (`crate::graph`)
/// destructure it exhaustively, so adding a field breaks them at compile
/// time instead of silently dropping data.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneParts<Id = Uuid7> {
  pub id: Id,
  pub video_track_id: Id,
  pub index: u32,
  pub span: TimeRange,
  pub detector: SceneDetector,
  pub keyframes: Vec<Id>,
  pub description: SmolStr,
}

impl<Id> Scene<Id> {
  /// Decompose into [`SceneParts`] — exhaustive, by value.
  #[inline(always)]
  pub fn into_parts(self) -> SceneParts<Id> {
    let Self {
      id,
      video_track_id,
      index,
      span,
      detector,
      keyframes,
      description,
    } = self;
    SceneParts {
      id,
      video_track_id,
      index,
      span,
      detector,
      keyframes,
      description,
    }
  }
}

impl<Id> Scene<Id> {
  /// Invariant-carrying constructor from [`SceneParts`] — `pub(crate)`,
  /// reserved for in-crate conversions from already-validated values
  /// (`crate::graph`).
  #[cfg(all(
    feature = "std",
    feature = "video",
    feature = "audio",
    feature = "subtitle"
  ))]
  #[inline(always)]
  pub(crate) fn rehydrate(parts: SceneParts<Id>) -> Self {
    let SceneParts {
      id,
      video_track_id,
      index,
      span,
      detector,
      keyframes,
      description,
    } = parts;
    Self {
      id,
      video_track_id,
      index,
      span,
      detector,
      keyframes,
      description,
    }
  }
}
