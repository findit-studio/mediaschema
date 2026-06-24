//! `Keyframe` — a scene thumbnail + structured image analysis
//! (locked `schema/keyframe.md` r15 + r16 reopen).
//!
//! Parent → `Scene`. **No `provenance`** — hoisted to `VideoTrack` in
//! rev 16; every `Keyframe` inside a track shares the track's
//! `Provenance`. The image is no longer inlined here — a `Keyframe`
//! references its [`Thumbnail`](super::thumbnail::Thumbnail) by FK
//! (`thumbnail_id`), so thumbnail storage (filesystem / database /
//! remote) is recorded per-thumbnail and can be mixed or migrated.
//! Embeddings + `feature_print` live in **LanceDB**, keyed by `id`.
//!
//! Many analysis VOs live in the sibling [`detections`](super::detections)
//! module to keep this file focused on the aggregate itself.

use std::vec::Vec;

use derive_more::IsVariant;
use mediaframe::frame::Dimensions;
use mediatime::Timestamp;

use crate::domain::{KeyframeExtractor, KeyframeRole, Uuid7};

use super::detections::{
  ActionDetection, Aesthetics, AnimalAnalysis, BarcodeDetection, Detection, DocumentSegment,
  DominantColor, HorizonInfo, HumanAnalysis, ObjectDetection, SaliencyRegion, TextDetection,
  VlmAnalysis,
};

/// A thumbnail + its full image analysis bundle.
///
/// Generic over `Id` (default [`Uuid7`]). Fields are private per the
/// encapsulation rule; access via the getter / `with_*` / `set_*`
/// accessors.
///
/// ## `role` and `scene_id`
///
/// A keyframe is either a **scene** keyframe (the common case — a
/// scene-representative frame, [`KeyframeRole::Scene`]) or the video's
/// **cover** poster ([`KeyframeRole::Cover`]). A scene keyframe rides the
/// `keyframe → scene → video_track` chain and so carries a `Some` `scene_id`;
/// a cover keyframe attaches at the video level and has **no scene parent**,
/// so its `scene_id` is `None`. The two cases are minted by the two
/// constructors ([`Keyframe::try_new`] / [`Keyframe::try_new_cover`]).
///
/// **No `Default`** — defaulting to a nil `id` would be an orphan thumbnail.
/// Construct via [`Keyframe::try_new`] / [`Keyframe::try_new_cover`] (which
/// also reject zero-`dimensions`, per the locked invariant that a thumbnail
/// has positive W and H).
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframe<Id = Uuid7> {
  // --- identity / source ---
  id: Id,
  /// FK → `Scene.id` for a scene keyframe; `None` for a cover keyframe
  /// (which attaches at the video level, no scene parent).
  scene_id: Option<Id>,
  /// Whether this is a scene-representative frame or the video poster.
  role: KeyframeRole,
  pts: Timestamp,

  // --- artifact ---
  /// FK → [`Thumbnail`](super::thumbnail::Thumbnail)`.id`. The image
  /// bytes (and their storage backend) live on the referenced
  /// `Thumbnail`, not inline on the keyframe.
  thumbnail_id: Id,
  /// Thumbnail dimensions (`mediaframe::frame::Dimensions`).
  dimensions: Dimensions,
  extractor: KeyframeExtractor,

  // --- apple-vision structured detections ---
  classifications: Vec<Detection>,
  objects: Vec<ObjectDetection>,
  humans: HumanAnalysis,
  animals: AnimalAnalysis,
  actions: Vec<ActionDetection>,
  text_detections: Vec<TextDetection>,
  barcodes: Vec<BarcodeDetection>,
  attention_saliency: Vec<SaliencyRegion>,
  objectness_saliency: Vec<SaliencyRegion>,
  horizon: HorizonInfo,
  document_segments: Vec<DocumentSegment>,
  aesthetics: Aesthetics,

  // --- colorthief ---
  colors: Vec<DominantColor>,

  // --- VLM ---
  vlm: VlmAnalysis,
}

impl Keyframe<Uuid7> {
  /// Validating constructor for a **scene** keyframe ([`KeyframeRole::Scene`]).
  ///
  /// Rejects:
  /// - nil `id` (LanceDB embedding key collision),
  /// - nil `scene_id` (orphan scene keyframe with no `Scene`),
  /// - nil `thumbnail_id` (orphan keyframe with no `Thumbnail`),
  /// - zero `dimensions` (a thumbnail with W=0 or H=0 is not a valid
  ///   image artifact; the locked spec calls out `dimensions` as
  ///   non-zero in its invariants).
  ///
  /// The keyframe's `scene_id` is set to `Some(scene_id)` and its `role`
  /// to [`KeyframeRole::Scene`]. All analysis fields start empty and are
  /// filled in via the `with_*` / `set_*` mutators as apple-vision /
  /// colorthief / VLM stages land. For the video poster, use
  /// [`Keyframe::try_new_cover`].
  pub fn try_new(
    id: Uuid7,
    scene_id: Uuid7,
    thumbnail_id: Uuid7,
    pts: Timestamp,
    dimensions: Dimensions,
    extractor: KeyframeExtractor,
  ) -> Result<Self, KeyframeError> {
    if scene_id.is_nil() {
      return Err(KeyframeError::NilSceneId);
    }
    Self::build(
      id,
      Some(scene_id),
      KeyframeRole::Scene,
      thumbnail_id,
      pts,
      dimensions,
      extractor,
    )
  }

  /// Validating constructor for the video's **cover** poster keyframe
  /// ([`KeyframeRole::Cover`]).
  ///
  /// A cover keyframe attaches at the **video** level — it has **no scene
  /// parent**, so its `scene_id` is `None`. Rejects nil `id` / nil
  /// `thumbnail_id` / zero `dimensions` exactly like [`Keyframe::try_new`];
  /// there is no `scene_id` to reject. The result is a real, analyzable
  /// [`Keyframe`] (it carries the same image + analysis bundle), so the
  /// keyframe analyzers can process the poster frame.
  pub fn try_new_cover(
    id: Uuid7,
    thumbnail_id: Uuid7,
    pts: Timestamp,
    dimensions: Dimensions,
    extractor: KeyframeExtractor,
  ) -> Result<Self, KeyframeError> {
    Self::build(
      id,
      None,
      KeyframeRole::Cover,
      thumbnail_id,
      pts,
      dimensions,
      extractor,
    )
  }

  /// Shared constructor body for [`Keyframe::try_new`] /
  /// [`Keyframe::try_new_cover`]. The caller has already validated that a
  /// scene keyframe's `scene_id` is non-nil; this enforces the remaining
  /// invariants (nil `id` / nil `thumbnail_id` / zero `dimensions`).
  fn build(
    id: Uuid7,
    scene_id: Option<Uuid7>,
    role: KeyframeRole,
    thumbnail_id: Uuid7,
    pts: Timestamp,
    dimensions: Dimensions,
    extractor: KeyframeExtractor,
  ) -> Result<Self, KeyframeError> {
    if id.is_nil() {
      return Err(KeyframeError::NilId);
    }
    if thumbnail_id.is_nil() {
      return Err(KeyframeError::NilThumbnailId);
    }
    if dimensions.width() == 0 || dimensions.height() == 0 {
      return Err(KeyframeError::ZeroDimensions);
    }
    Ok(Self {
      id,
      scene_id,
      role,
      pts,
      thumbnail_id,
      dimensions,
      extractor,
      classifications: Vec::new(),
      objects: Vec::new(),
      humans: HumanAnalysis::new(),
      animals: AnimalAnalysis::new(),
      actions: Vec::new(),
      text_detections: Vec::new(),
      barcodes: Vec::new(),
      attention_saliency: Vec::new(),
      objectness_saliency: Vec::new(),
      // 0.0 is trivially finite and in `0.0..=1.0` — this `try_new`
      // cannot fail.
      horizon: HorizonInfo::try_new(0.0, 0.0).expect("0.0 confidence is within range"),
      document_segments: Vec::new(),
      aesthetics: Aesthetics::new(0.0, false),
      colors: Vec::new(),
      vlm: VlmAnalysis::new(),
    })
  }
}

impl<Id> Keyframe<Id> {
  // --- identity / source ---
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }
  /// FK → `Scene.id` for a scene keyframe; `None` for a cover keyframe
  /// (which attaches at the video level, no scene parent).
  #[inline(always)]
  pub const fn scene_id_ref(&self) -> Option<&Id> {
    self.scene_id.as_ref()
  }
  /// Whether this is a scene-representative frame ([`KeyframeRole::Scene`])
  /// or the video poster ([`KeyframeRole::Cover`]).
  #[inline(always)]
  pub const fn role(&self) -> KeyframeRole {
    self.role
  }
  #[inline(always)]
  pub const fn pts_ref(&self) -> &Timestamp {
    &self.pts
  }

  // --- artifact ---
  /// FK → [`Thumbnail`](super::thumbnail::Thumbnail)`.id` — the image
  /// bytes + storage backend live on the referenced thumbnail.
  #[inline(always)]
  pub const fn thumbnail_id_ref(&self) -> &Id {
    &self.thumbnail_id
  }
  /// Thumbnail dimensions (`mediaframe::frame::Dimensions`).
  #[inline(always)]
  pub const fn dimensions(&self) -> Dimensions {
    self.dimensions
  }
  /// Which extractor produced this keyframe.
  #[inline(always)]
  pub const fn extractor(&self) -> KeyframeExtractor {
    self.extractor
  }

  // --- apple-vision ---
  #[inline(always)]
  pub fn classifications_slice(&self) -> &[Detection] {
    &self.classifications
  }
  #[inline(always)]
  pub fn objects_slice(&self) -> &[ObjectDetection] {
    &self.objects
  }
  #[inline(always)]
  pub const fn humans_ref(&self) -> &HumanAnalysis {
    &self.humans
  }
  #[inline(always)]
  pub const fn animals_ref(&self) -> &AnimalAnalysis {
    &self.animals
  }
  #[inline(always)]
  pub fn actions_slice(&self) -> &[ActionDetection] {
    &self.actions
  }
  #[inline(always)]
  pub fn text_detections_slice(&self) -> &[TextDetection] {
    &self.text_detections
  }
  #[inline(always)]
  pub fn barcodes_slice(&self) -> &[BarcodeDetection] {
    &self.barcodes
  }
  #[inline(always)]
  pub fn attention_saliency_slice(&self) -> &[SaliencyRegion] {
    &self.attention_saliency
  }
  #[inline(always)]
  pub fn objectness_saliency_slice(&self) -> &[SaliencyRegion] {
    &self.objectness_saliency
  }
  #[inline(always)]
  pub const fn horizon_ref(&self) -> &HorizonInfo {
    &self.horizon
  }
  #[inline(always)]
  pub fn document_segments_slice(&self) -> &[DocumentSegment] {
    &self.document_segments
  }
  #[inline(always)]
  pub const fn aesthetics_ref(&self) -> &Aesthetics {
    &self.aesthetics
  }

  // --- colorthief ---
  #[inline(always)]
  pub fn colors_slice(&self) -> &[DominantColor] {
    &self.colors
  }

  // --- VLM ---
  #[inline(always)]
  pub const fn vlm_ref(&self) -> &VlmAnalysis {
    &self.vlm
  }
}

// Builders + setters per the encapsulation rule.
impl<Id> Keyframe<Id> {
  // --- artifact ---
  /// Builder: replace the `thumbnail_id` FK (→
  /// [`Thumbnail`](super::thumbnail::Thumbnail)`.id`).
  #[must_use]
  #[inline(always)]
  pub fn with_thumbnail_id(mut self, v: Id) -> Self {
    self.thumbnail_id = v;
    self
  }
  /// In-place mutator for the `thumbnail_id` FK.
  #[inline(always)]
  pub fn set_thumbnail_id(&mut self, v: Id) -> &mut Self {
    self.thumbnail_id = v;
    self
  }
  /// Fallible builder for `dimensions`, re-validating the locked
  /// non-zero-extent invariant (`width > 0 && height > 0`). Rejects a
  /// zero-width/zero-height `Dimensions` with
  /// [`KeyframeError::ZeroDimensions`].
  #[inline]
  pub fn try_with_dimensions(mut self, v: Dimensions) -> Result<Self, KeyframeError> {
    if v.width() == 0 || v.height() == 0 {
      return Err(KeyframeError::ZeroDimensions);
    }
    self.dimensions = v;
    Ok(self)
  }
  /// Fallible in-place mutator for `dimensions` — see
  /// [`Keyframe::try_with_dimensions`]. On success returns `&mut Self`
  /// so it chains; on zero extent returns
  /// [`KeyframeError::ZeroDimensions`] and leaves `self` unchanged.
  #[inline]
  pub const fn try_set_dimensions(&mut self, v: Dimensions) -> Result<&mut Self, KeyframeError> {
    if v.width() == 0 || v.height() == 0 {
      return Err(KeyframeError::ZeroDimensions);
    }
    self.dimensions = v;
    Ok(self)
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_extractor(mut self, v: KeyframeExtractor) -> Self {
    self.extractor = v;
    self
  }
  #[inline(always)]
  pub const fn set_extractor(&mut self, v: KeyframeExtractor) -> &mut Self {
    self.extractor = v;
    self
  }
  /// Builder: replace the `role` discriminant. Note this does **not**
  /// touch `scene_id` — flipping a scene keyframe to `Cover` (or back)
  /// without also reconciling `scene_id` is the caller's responsibility;
  /// the canonical paths are the two constructors.
  #[must_use]
  #[inline(always)]
  pub const fn with_role(mut self, v: KeyframeRole) -> Self {
    self.role = v;
    self
  }
  /// In-place mutator for the `role` discriminant. See
  /// [`Keyframe::with_role`].
  #[inline(always)]
  pub const fn set_role(&mut self, v: KeyframeRole) -> &mut Self {
    self.role = v;
    self
  }

  // --- apple-vision ---
  #[must_use]
  #[inline(always)]
  pub fn with_classifications(mut self, v: impl Into<Vec<Detection>>) -> Self {
    self.classifications = v.into();
    self
  }
  #[inline(always)]
  pub fn set_classifications(&mut self, v: impl Into<Vec<Detection>>) -> &mut Self {
    self.classifications = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_objects(mut self, v: impl Into<Vec<ObjectDetection>>) -> Self {
    self.objects = v.into();
    self
  }
  #[inline(always)]
  pub fn set_objects(&mut self, v: impl Into<Vec<ObjectDetection>>) -> &mut Self {
    self.objects = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_humans(mut self, v: HumanAnalysis) -> Self {
    self.humans = v;
    self
  }
  #[inline(always)]
  pub fn set_humans(&mut self, v: HumanAnalysis) -> &mut Self {
    self.humans = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_animals(mut self, v: AnimalAnalysis) -> Self {
    self.animals = v;
    self
  }
  #[inline(always)]
  pub fn set_animals(&mut self, v: AnimalAnalysis) -> &mut Self {
    self.animals = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_actions(mut self, v: impl Into<Vec<ActionDetection>>) -> Self {
    self.actions = v.into();
    self
  }
  #[inline(always)]
  pub fn set_actions(&mut self, v: impl Into<Vec<ActionDetection>>) -> &mut Self {
    self.actions = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_text_detections(mut self, v: impl Into<Vec<TextDetection>>) -> Self {
    self.text_detections = v.into();
    self
  }
  #[inline(always)]
  pub fn set_text_detections(&mut self, v: impl Into<Vec<TextDetection>>) -> &mut Self {
    self.text_detections = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_barcodes(mut self, v: impl Into<Vec<BarcodeDetection>>) -> Self {
    self.barcodes = v.into();
    self
  }
  #[inline(always)]
  pub fn set_barcodes(&mut self, v: impl Into<Vec<BarcodeDetection>>) -> &mut Self {
    self.barcodes = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_attention_saliency(mut self, v: impl Into<Vec<SaliencyRegion>>) -> Self {
    self.attention_saliency = v.into();
    self
  }
  #[inline(always)]
  pub fn set_attention_saliency(&mut self, v: impl Into<Vec<SaliencyRegion>>) -> &mut Self {
    self.attention_saliency = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_objectness_saliency(mut self, v: impl Into<Vec<SaliencyRegion>>) -> Self {
    self.objectness_saliency = v.into();
    self
  }
  #[inline(always)]
  pub fn set_objectness_saliency(&mut self, v: impl Into<Vec<SaliencyRegion>>) -> &mut Self {
    self.objectness_saliency = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_horizon(mut self, v: HorizonInfo) -> Self {
    self.horizon = v;
    self
  }
  #[inline(always)]
  pub const fn set_horizon(&mut self, v: HorizonInfo) -> &mut Self {
    self.horizon = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_document_segments(mut self, v: impl Into<Vec<DocumentSegment>>) -> Self {
    self.document_segments = v.into();
    self
  }
  #[inline(always)]
  pub fn set_document_segments(&mut self, v: impl Into<Vec<DocumentSegment>>) -> &mut Self {
    self.document_segments = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_aesthetics(mut self, v: Aesthetics) -> Self {
    self.aesthetics = v;
    self
  }
  #[inline(always)]
  pub const fn set_aesthetics(&mut self, v: Aesthetics) -> &mut Self {
    self.aesthetics = v;
    self
  }

  // --- colorthief ---
  #[must_use]
  #[inline(always)]
  pub fn with_colors(mut self, v: impl Into<Vec<DominantColor>>) -> Self {
    self.colors = v.into();
    self
  }
  #[inline(always)]
  pub fn set_colors(&mut self, v: impl Into<Vec<DominantColor>>) -> &mut Self {
    self.colors = v.into();
    self
  }

  // --- VLM ---
  #[must_use]
  #[inline(always)]
  pub fn with_vlm(mut self, v: VlmAnalysis) -> Self {
    self.vlm = v;
    self
  }
  #[inline(always)]
  pub fn set_vlm(&mut self, v: VlmAnalysis) -> &mut Self {
    self.vlm = v;
    self
  }
}

/// Error returned when [`Keyframe::try_new`] cannot uphold a locked
/// invariant. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum KeyframeError {
  /// Supplied `id` was the nil sentinel.
  #[error("Keyframe id must not be the nil UUID")]
  NilId,
  /// Supplied `scene_id` was the nil sentinel — orphan keyframe with no
  /// `Scene` reference.
  #[error("Keyframe `scene_id` (FK → Scene) must not be the nil UUID")]
  NilSceneId,
  /// Supplied `thumbnail_id` was the nil sentinel — orphan keyframe with
  /// no `Thumbnail` reference.
  #[error("Keyframe `thumbnail_id` (FK → Thumbnail) must not be the nil UUID")]
  NilThumbnailId,
  /// `dimensions.width() == 0` or `dimensions.height() == 0` — a
  /// zero-extent thumbnail is not a valid artifact (locked invariant).
  #[error("Keyframe dimensions must be non-zero (locked invariant)")]
  ZeroDimensions,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use crate::domain::vo::LocalizedText;
  use core::num::NonZeroU32;
  use mediatime::Timebase;

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).unwrap())
  }

  #[test]
  fn try_new_happy_path() {
    let scene_id = Uuid7::new();
    let thumbnail_id = Uuid7::new();
    let ts = Timestamp::new(1234, tb());
    let kf = Keyframe::try_new(
      Uuid7::new(),
      scene_id,
      thumbnail_id,
      ts,
      Dimensions::new(320, 180),
      KeyframeExtractor::CompositeQuality,
    )
    .unwrap();
    assert_eq!(kf.scene_id_ref(), Some(&scene_id));
    assert!(kf.role().is_scene());
    assert_eq!(kf.thumbnail_id_ref(), &thumbnail_id);
    assert_eq!(kf.pts_ref(), &ts);
    assert_eq!(kf.dimensions(), Dimensions::new(320, 180));
    assert!(kf.extractor().is_composite_quality());
    assert!(kf.classifications_slice().is_empty());
    assert!(kf.colors_slice().is_empty());
    assert_eq!(kf.vlm_ref().shot_type(), "");
  }

  #[test]
  fn try_new_cover_has_no_scene_and_cover_role() {
    let thumbnail_id = Uuid7::new();
    let ts = Timestamp::new(1234, tb());
    let kf = Keyframe::try_new_cover(
      Uuid7::new(),
      thumbnail_id,
      ts,
      Dimensions::new(320, 180),
      KeyframeExtractor::CompositeQuality,
    )
    .unwrap();
    assert_eq!(
      kf.scene_id_ref(),
      None,
      "cover keyframe has no scene parent"
    );
    assert!(kf.role().is_cover());
    assert_eq!(kf.thumbnail_id_ref(), &thumbnail_id);
  }

  #[test]
  fn try_new_cover_rejects_nil_id_thumbnail_and_zero_dims() {
    let ts = Timestamp::new(0, tb());
    assert_eq!(
      Keyframe::try_new_cover(
        Uuid7::nil(),
        Uuid7::new(),
        ts,
        Dimensions::new(1, 1),
        KeyframeExtractor::Manual
      )
      .err(),
      Some(KeyframeError::NilId)
    );
    assert_eq!(
      Keyframe::try_new_cover(
        Uuid7::new(),
        Uuid7::nil(),
        ts,
        Dimensions::new(1, 1),
        KeyframeExtractor::Manual
      )
      .err(),
      Some(KeyframeError::NilThumbnailId)
    );
    assert_eq!(
      Keyframe::try_new_cover(
        Uuid7::new(),
        Uuid7::new(),
        ts,
        Dimensions::new(0, 1),
        KeyframeExtractor::Manual
      )
      .err(),
      Some(KeyframeError::ZeroDimensions)
    );
  }

  #[test]
  fn role_and_scene_id_survive_into_parts_rehydrate() {
    let ts = Timestamp::new(0, tb());
    let scene = Keyframe::try_new(
      Uuid7::new(),
      Uuid7::new(),
      Uuid7::new(),
      ts,
      Dimensions::new(2, 2),
      KeyframeExtractor::IFrame,
    )
    .unwrap();
    let scene2 = Keyframe::rehydrate(scene.clone().into_parts());
    assert_eq!(scene, scene2);
    assert!(scene2.role().is_scene());
    assert!(scene2.scene_id_ref().is_some());

    let cover = Keyframe::try_new_cover(
      Uuid7::new(),
      Uuid7::new(),
      ts,
      Dimensions::new(2, 2),
      KeyframeExtractor::IFrame,
    )
    .unwrap();
    let cover2 = Keyframe::rehydrate(cover.clone().into_parts());
    assert_eq!(cover, cover2);
    assert!(cover2.role().is_cover());
    assert!(cover2.scene_id_ref().is_none());
  }

  #[test]
  fn try_new_rejects_nil_id_and_parent() {
    let ts = Timestamp::new(0, tb());
    assert_eq!(
      Keyframe::try_new(
        Uuid7::nil(),
        Uuid7::new(),
        Uuid7::new(),
        ts,
        Dimensions::new(1, 1),
        KeyframeExtractor::Manual
      )
      .err(),
      Some(KeyframeError::NilId)
    );
    assert_eq!(
      Keyframe::try_new(
        Uuid7::new(),
        Uuid7::nil(),
        Uuid7::new(),
        ts,
        Dimensions::new(1, 1),
        KeyframeExtractor::Manual
      )
      .err(),
      Some(KeyframeError::NilSceneId)
    );
    assert_eq!(
      Keyframe::try_new(
        Uuid7::new(),
        Uuid7::new(),
        Uuid7::nil(),
        ts,
        Dimensions::new(1, 1),
        KeyframeExtractor::Manual
      )
      .err(),
      Some(KeyframeError::NilThumbnailId)
    );
    assert!(KeyframeError::NilId.is_nil_id());
    assert!(KeyframeError::NilSceneId.is_nil_scene_id());
    assert!(KeyframeError::NilThumbnailId.is_nil_thumbnail_id());
  }

  #[test]
  fn try_new_rejects_zero_dimensions() {
    let ts = Timestamp::new(0, tb());
    for (w, h) in [(0u32, 1u32), (1, 0), (0, 0)] {
      let r = Keyframe::try_new(
        Uuid7::new(),
        Uuid7::new(),
        Uuid7::new(),
        ts,
        Dimensions::new(w, h),
        KeyframeExtractor::IFrame,
      );
      assert_eq!(
        r.err(),
        Some(KeyframeError::ZeroDimensions),
        "({w}, {h}) should be rejected"
      );
    }
    assert!(KeyframeError::ZeroDimensions.is_zero_dimensions());
  }

  #[test]
  fn builders_and_setters_chain() {
    let ts = Timestamp::new(7000, tb());
    let thumb_a = Uuid7::new();
    let thumb_b = Uuid7::new();
    let kf = Keyframe::try_new(
      Uuid7::new(),
      Uuid7::new(),
      thumb_a,
      ts,
      Dimensions::new(1920, 1080),
      KeyframeExtractor::IFrame,
    )
    .unwrap()
    .with_thumbnail_id(thumb_b)
    .with_classifications(std::vec![Detection::try_new("dog", 0.97).unwrap()])
    .with_vlm(
      VlmAnalysis::new()
        .with_description(LocalizedText::from_src("a dog running"))
        .with_tags(std::vec![LocalizedText::from_src("dog")])
        .with_shot_type("medium-shot"),
    );
    assert_eq!(kf.thumbnail_id_ref(), &thumb_b);
    assert_eq!(kf.classifications_slice().len(), 1);
    assert_eq!(kf.classifications_slice()[0].label(), "dog");
    assert_eq!(kf.vlm_ref().description_ref().src(), "a dog running");
    assert_eq!(kf.vlm_ref().tags_slice().len(), 1);
    assert_eq!(kf.vlm_ref().shot_type(), "medium-shot");

    let mut kf = kf;
    kf.set_thumbnail_id(thumb_a);
    kf.try_set_dimensions(Dimensions::new(2, 2)).unwrap();
    assert_eq!(kf.thumbnail_id_ref(), &thumb_a);
    assert_eq!(kf.dimensions(), Dimensions::new(2, 2));
  }

  #[test]
  fn dimension_mutators_reject_zero_extent() {
    let ts = Timestamp::new(0, tb());
    let mut kf = Keyframe::try_new(
      Uuid7::new(),
      Uuid7::new(),
      Uuid7::new(),
      ts,
      Dimensions::new(320, 180),
      KeyframeExtractor::IFrame,
    )
    .unwrap();

    // Zero-extent dimensions are rejected through both fallible
    // mutators; `self` keeps the previously-valid dimensions.
    for (w, h) in [(0u32, 1u32), (1, 0), (0, 0)] {
      assert_eq!(
        kf.clone().try_with_dimensions(Dimensions::new(w, h)).err(),
        Some(KeyframeError::ZeroDimensions),
        "({w}, {h}) builder should be rejected"
      );
      assert_eq!(
        kf.try_set_dimensions(Dimensions::new(w, h)).err(),
        Some(KeyframeError::ZeroDimensions),
        "({w}, {h}) setter should be rejected"
      );
    }
    assert_eq!(kf.dimensions(), Dimensions::new(320, 180));

    // A valid replacement is accepted.
    kf.try_set_dimensions(Dimensions::new(2, 2)).unwrap();
    assert_eq!(kf.dimensions(), Dimensions::new(2, 2));
  }
}

/// Exhaustive by-value decomposition of [`Keyframe`] — every stored
/// field.
///
/// Public-field data-transfer struct (the conversion-boundary exception
/// to the encapsulation rule): cross-suite conversions (`crate::graph`)
/// destructure it exhaustively, so adding a field breaks them at compile
/// time instead of silently dropping data.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyframeParts<Id = Uuid7> {
  pub id: Id,
  /// FK → `Scene.id` for a scene keyframe; `None` for a cover keyframe.
  pub scene_id: Option<Id>,
  /// Scene-representative frame vs the video poster.
  pub role: KeyframeRole,
  pub pts: Timestamp,
  pub thumbnail_id: Id,
  pub dimensions: Dimensions,
  pub extractor: KeyframeExtractor,
  pub classifications: Vec<Detection>,
  pub objects: Vec<ObjectDetection>,
  pub humans: HumanAnalysis,
  pub animals: AnimalAnalysis,
  pub actions: Vec<ActionDetection>,
  pub text_detections: Vec<TextDetection>,
  pub barcodes: Vec<BarcodeDetection>,
  pub attention_saliency: Vec<SaliencyRegion>,
  pub objectness_saliency: Vec<SaliencyRegion>,
  pub horizon: HorizonInfo,
  pub document_segments: Vec<DocumentSegment>,
  pub aesthetics: Aesthetics,
  pub colors: Vec<DominantColor>,
  pub vlm: VlmAnalysis,
}

impl<Id> Keyframe<Id> {
  /// Decompose into [`KeyframeParts`] — exhaustive, by value.
  #[inline(always)]
  pub fn into_parts(self) -> KeyframeParts<Id> {
    let Self {
      id,
      scene_id,
      role,
      pts,
      thumbnail_id,
      dimensions,
      extractor,
      classifications,
      objects,
      humans,
      animals,
      actions,
      text_detections,
      barcodes,
      attention_saliency,
      objectness_saliency,
      horizon,
      document_segments,
      aesthetics,
      colors,
      vlm,
    } = self;
    KeyframeParts {
      id,
      scene_id,
      role,
      pts,
      thumbnail_id,
      dimensions,
      extractor,
      classifications,
      objects,
      humans,
      animals,
      actions,
      text_detections,
      barcodes,
      attention_saliency,
      objectness_saliency,
      horizon,
      document_segments,
      aesthetics,
      colors,
      vlm,
    }
  }
}

impl<Id> Keyframe<Id> {
  /// Invariant-carrying constructor from [`KeyframeParts`] —
  /// `pub(crate)`, reserved for in-crate conversions from
  /// already-validated values (`crate::graph`).
  #[cfg(all(
    feature = "std",
    feature = "video",
    feature = "audio",
    feature = "subtitle"
  ))]
  #[inline(always)]
  pub(crate) fn rehydrate(parts: KeyframeParts<Id>) -> Self {
    let KeyframeParts {
      id,
      scene_id,
      role,
      pts,
      thumbnail_id,
      dimensions,
      extractor,
      classifications,
      objects,
      humans,
      animals,
      actions,
      text_detections,
      barcodes,
      attention_saliency,
      objectness_saliency,
      horizon,
      document_segments,
      aesthetics,
      colors,
      vlm,
    } = parts;
    Self {
      id,
      scene_id,
      role,
      pts,
      thumbnail_id,
      dimensions,
      extractor,
      classifications,
      objects,
      humans,
      animals,
      actions,
      text_detections,
      barcodes,
      attention_saliency,
      objectness_saliency,
      horizon,
      document_segments,
      aesthetics,
      colors,
      vlm,
    }
  }
}
