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
  /// Rejects nil `id` and nil `parent`. The locked
  /// `span.start <= span.end` invariant is **already enforced** by
  /// `mediatime::TimeRange::new` / `::try_new` (which is the only way
  /// to construct a `TimeRange`), so it's a redundant check here — we
  /// rely on the upstream type.
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

  /// Builder: replace `detector`.
  #[inline]
  pub const fn with_detector(mut self, detector: SceneDetector) -> Self {
    self.detector = detector;
    self
  }

  /// Builder: replace `keyframes`.
  #[inline]
  pub fn with_keyframes(mut self, kfs: impl Into<std::vec::Vec<Id>>) -> Self {
    self.keyframes = kfs.into();
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

  /// In-place mutator for `keyframes`.
  #[inline]
  pub fn set_keyframes(&mut self, kfs: impl Into<std::vec::Vec<Id>>) {
    self.keyframes = kfs.into();
  }

  /// In-place mutator for `description`.
  #[inline]
  pub fn set_description(&mut self, description: impl Into<SmolStr>) {
    self.description = description.into();
  }
}

/// Error returned when [`Scene::try_new`] cannot uphold a locked
/// invariant. Unit-only enum.
///
/// (The `span.start <= span.end` invariant from the locked spec is
/// enforced upstream by `mediatime::TimeRange`'s own constructors —
/// not represented here.)
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
  fn inverted_span_blocked_upstream_by_mediatime() {
    // The locked `span.start <= span.end` invariant is enforced by
    // `mediatime::TimeRange::try_new` returning `None` — Scene relies
    // on the upstream type, so an inverted span cannot reach
    // `Scene::try_new` to begin with.
    assert!(TimeRange::try_new(2000, 1000, tb()).is_none());
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
    .with_keyframes(std::vec![kf])
    .with_description("Jane is eating");
    assert_eq!(s.index(), 3);
    assert!(s.detector().is_content());
    assert_eq!(s.keyframes(), &[kf]);
    assert_eq!(s.description(), "Jane is eating");

    let mut s = s;
    s.set_index(0);
    s.set_description("");
    s.set_keyframes(std::vec::Vec::<Uuid7>::new());
    s.set_detector(SceneDetector::Manual);
    assert_eq!(s.index(), 0);
    assert!(s.description().is_empty());
    assert!(s.keyframes().is_empty());
    assert!(s.detector().is_manual());
  }
}
