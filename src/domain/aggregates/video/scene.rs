//! `Scene` — one described time segment of a video stream
//! (locked `schema/scene.md` r6 + r7 reopen).
//!
//! Parent → `VideoTrack` (scene detection is per-stream). Keyframes
//! **are** the scene's thumbnails. **No `provenance`** — hoisted to
//! `VideoTrack` in rev 7; every `Scene` inside a `VideoTrack` shares
//! the track's `Provenance`.

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
/// **No `Default`** — defaulting to nil `id`/`parent` would be an
/// orphan segment with no track. Construct via [`Scene::try_new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scene<Id = Uuid7> {
  id: Id,
  parent: Id,
  index: u32,
  span: TimeRange,
  detector: SceneDetector,
  keyframes: std::vec::Vec<Id>,
  description: SmolStr,
}

impl Scene<Uuid7> {
  /// Validating constructor.
  ///
  /// Rejects:
  /// - nil `id`,
  /// - nil `parent`,
  /// - an inverted `span` (`start_pts > end_pts`).
  ///
  /// `mediatime::TimeRange::try_new` rejects an inverted span at
  /// construction, but `TimeRange` also exposes public `with_*`/`set_*`
  /// mutators, so a caller can hand `Scene` a `TimeRange` that *was*
  /// valid and has since been inverted. `Scene` therefore re-validates
  /// the `start <= end` invariant itself rather than trusting upstream.
  pub fn try_new(
    id: Uuid7,
    parent: Uuid7,
    index: u32,
    span: TimeRange,
    detector: SceneDetector,
  ) -> Result<Self, SceneError> {
    if id.is_nil() {
      return Err(SceneError::NilId);
    }
    if parent.is_nil() {
      return Err(SceneError::NilParent);
    }
    if span.start_pts() > span.end_pts() {
      return Err(SceneError::InvertedSpan);
    }
    Ok(Self {
      id,
      parent,
      index,
      span,
      detector,
      keyframes: std::vec::Vec::new(),
      description: SmolStr::default(),
    })
  }

  /// Validate a `keyframes` child-ref list: rejects any nil sentinel
  /// ([`SceneError::NilKeyframeRef`]) and any duplicate id
  /// ([`SceneError::DuplicateKeyframeRef`]). Each entry must be a
  /// reference to a distinct, real child `Keyframe`.
  fn validate_keyframes(keyframes: &[Uuid7]) -> Result<(), SceneError> {
    for (i, id) in keyframes.iter().enumerate() {
      if id.is_nil() {
        return Err(SceneError::NilKeyframeRef);
      }
      if keyframes[..i].contains(id) {
        return Err(SceneError::DuplicateKeyframeRef);
      }
    }
    Ok(())
  }

  /// Fallible builder: replace the `keyframes` child-ref id-list.
  ///
  /// Rejects any nil ([`SceneError::NilKeyframeRef`]) or duplicate
  /// ([`SceneError::DuplicateKeyframeRef`]) entry — every entry must be
  /// a reference to a distinct, real child `Keyframe`.
  #[inline]
  pub fn try_with_keyframes(
    mut self,
    kfs: impl Into<std::vec::Vec<Uuid7>>,
  ) -> Result<Self, SceneError> {
    self.try_set_keyframes(kfs)?;
    Ok(self)
  }

  /// Fallible in-place mutator for `keyframes` — see
  /// [`Scene::try_with_keyframes`]. On success returns `&mut Self` so
  /// it chains; on a nil or duplicate ref returns the matching
  /// [`SceneError`] and leaves `self` unchanged.
  #[inline]
  pub fn try_set_keyframes(
    &mut self,
    kfs: impl Into<std::vec::Vec<Uuid7>>,
  ) -> Result<&mut Self, SceneError> {
    let keyframes = kfs.into();
    Self::validate_keyframes(&keyframes)?;
    self.keyframes = keyframes;
    Ok(self)
  }
}

impl<Id> Scene<Id> {
  /// Canonical identity (also the LanceDB vector key).
  #[inline]
  pub const fn id(&self) -> &Id {
    &self.id
  }

  /// FK → `VideoTrack.id`.
  #[inline]
  pub const fn parent(&self) -> &Id {
    &self.parent
  }

  /// 0-based scene order within the track.
  #[inline]
  pub const fn index(&self) -> u32 {
    self.index
  }

  /// Media-time span (`mediatime::TimeRange`).
  #[inline]
  pub const fn span(&self) -> &TimeRange {
    &self.span
  }

  /// Which detector raised this scene.
  #[inline]
  pub const fn detector(&self) -> SceneDetector {
    self.detector
  }

  /// Refs → child [`Keyframe`](super::keyframe::Keyframe)s — these
  /// **are** the scene's thumbnails.
  #[inline]
  pub const fn keyframes(&self) -> &[Id] {
    self.keyframes.as_slice()
  }

  /// VLM-generated description (`""` = none).
  #[inline]
  pub fn description(&self) -> &str {
    self.description.as_str()
  }

  /// Builder: replace `index`.
  #[inline]
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
  #[inline]
  pub const fn with_detector(mut self, detector: SceneDetector) -> Self {
    self.detector = detector;
    self
  }

  /// Builder: replace `description`.
  #[inline]
  pub fn with_description(mut self, description: impl Into<SmolStr>) -> Self {
    self.description = description.into();
    self
  }

  /// In-place mutator for `index`.
  #[inline]
  pub const fn set_index(&mut self, index: u32) {
    self.index = index;
  }

  /// In-place mutator for `detector`.
  #[inline]
  pub const fn set_detector(&mut self, detector: SceneDetector) {
    self.detector = detector;
  }

  /// In-place mutator for `description`.
  #[inline]
  pub fn set_description(&mut self, description: impl Into<SmolStr>) {
    self.description = description.into();
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
  /// Supplied `parent` was the nil sentinel — orphan scene with no
  /// `VideoTrack`.
  #[error("Scene parent (VideoTrack) must not be the nil UUID")]
  NilParent,
  /// Supplied `span` was inverted (`start_pts > end_pts`). A
  /// `mediatime::TimeRange` validates `start <= end` at construction,
  /// but its public `with_*`/`set_*` mutators can invert it afterwards,
  /// so `Scene` re-checks the invariant on every span it accepts.
  #[error("Scene span must not be inverted (start_pts <= end_pts)")]
  InvertedSpan,
  /// A `keyframes` entry was the nil sentinel — every entry must
  /// reference a real child `Keyframe`.
  #[error("Scene keyframes ref must not be the nil UUID")]
  NilKeyframeRef,
  /// A `keyframes` entry was a duplicate — every child `Keyframe` ref
  /// must be distinct.
  #[error("Scene keyframes refs must be unique")]
  DuplicateKeyframeRef,
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
    let parent = Uuid7::new();
    let span = TimeRange::new(5_000, 10_000, tb());
    let s = Scene::try_new(Uuid7::new(), parent, 0, span, SceneDetector::Adaptive).unwrap();
    assert_eq!(s.parent(), &parent);
    assert_eq!(s.index(), 0);
    assert_eq!(s.span(), &span);
    assert!(s.detector().is_adaptive());
    assert!(s.keyframes().is_empty());
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
      Some(SceneError::NilParent)
    );
    assert!(SceneError::NilId.is_nil_id());
    assert!(SceneError::NilParent.is_nil_parent());
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
    assert_eq!(s.span(), &span);

    // A valid replacement span is accepted.
    let next = TimeRange::new(100, 200, tb());
    s.try_set_span(next).unwrap();
    assert_eq!(s.span(), &next);

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
    assert_eq!(s.span().start_pts(), s.span().end_pts());
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
    .try_with_keyframes(std::vec![kf])
    .unwrap()
    .with_description("Jane is eating");
    assert_eq!(s.index(), 3);
    assert!(s.detector().is_content());
    assert_eq!(s.keyframes(), &[kf]);
    assert_eq!(s.description(), "Jane is eating");

    let mut s = s;
    s.set_index(0);
    s.set_description("");
    s.try_set_keyframes(std::vec::Vec::<Uuid7>::new()).unwrap();
    s.set_detector(SceneDetector::Manual);
    assert_eq!(s.index(), 0);
    assert!(s.description().is_empty());
    assert!(s.keyframes().is_empty());
    assert!(s.detector().is_manual());
  }

  #[test]
  fn keyframes_reject_nil_and_duplicate_refs() {
    // rev-4 finding 1: the infallible keyframe-list setter let a
    // `Scene<Uuid7>` persist a nil ref or `[same, same]`.
    let span = TimeRange::new(0, 5_000, tb());
    let s = Scene::try_new(Uuid7::new(), Uuid7::new(), 0, span, SceneDetector::Manual).unwrap();
    let kf = Uuid7::new();

    // A nil ref is rejected through the consuming builder...
    assert_eq!(
      s.clone().try_with_keyframes(std::vec![Uuid7::nil()]).err(),
      Some(SceneError::NilKeyframeRef)
    );
    // ...and a duplicate ref.
    assert_eq!(
      s.clone().try_with_keyframes(std::vec![kf, kf]).err(),
      Some(SceneError::DuplicateKeyframeRef)
    );

    // The in-place setter rejects both and leaves `self` unchanged.
    let mut s = s;
    assert_eq!(
      s.try_set_keyframes(std::vec![Uuid7::nil()]).err(),
      Some(SceneError::NilKeyframeRef)
    );
    assert_eq!(
      s.try_set_keyframes(std::vec![kf, kf]).err(),
      Some(SceneError::DuplicateKeyframeRef)
    );
    assert!(s.keyframes().is_empty());
    assert!(SceneError::NilKeyframeRef.is_nil_keyframe_ref());
    assert!(SceneError::DuplicateKeyframeRef.is_duplicate_keyframe_ref());

    // A list of distinct, non-nil refs is accepted.
    let kf2 = Uuid7::new();
    s.try_set_keyframes(std::vec![kf, kf2]).unwrap();
    assert_eq!(s.keyframes().len(), 2);
  }
}
