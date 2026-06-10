//! `Keyframe` — a scene thumbnail + structured image analysis
//! (locked `schema/keyframe.md` r15 + r16 reopen).
//!
//! Parent → `Scene`. **No `provenance`** — hoisted to `VideoTrack` in
//! rev 16; every `Keyframe` inside a track shares the track's
//! `Provenance`. The image is inline `data: bytes::Bytes` (no
//! `location` — locked rev E); embeddings + `feature_print` live in
//! **LanceDB**, keyed by `id`.
//!
//! Many analysis VOs live in the sibling [`detections`](super::detections)
//! module to keep this file focused on the aggregate itself.

use bytes::Bytes;
use derive_more::IsVariant;
use mediaframe::frame::Dimensions;
use mediatime::Timestamp;
use smol_str::SmolStr;

use crate::domain::{KeyframeExtractor, Uuid7};

use super::detections::{
  ActionDetection, Aesthetics, AnimalAnalysis, BarcodeDetection, Detection, DocumentSegment,
  DominantColor, HorizonInfo, HumanAnalysis, ObjectDetection, SaliencyRegion, TextDetection,
  VlmAnalysis,
};

/// A scene thumbnail + its full image analysis bundle.
///
/// Generic over `Id` (default [`Uuid7`]). Fields are private per the
/// encapsulation rule; access via the getter / `with_*` / `set_*`
/// accessors.
///
/// **No `Default`** — defaulting to nil `id`/`scene_id` would be an
/// orphan thumbnail with no scene. Construct via [`Keyframe::try_new`]
/// (which also rejects zero-`dimensions`, per the locked invariant
/// that a thumbnail has positive W and H).
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframe<Id = Uuid7> {
  // --- identity / source ---
  id: Id,
  scene_id: Id,
  pts: Timestamp,

  // --- artifact ---
  /// Inline thumbnail image bytes (`bytes::Bytes`, no `location`).
  ///
  /// The byte-length of this buffer is the keyframe `size`; `size` is
  /// **not** a stored field — it is derived from `data` via
  /// [`Keyframe::size`], so the two cannot diverge.
  data: Bytes,
  mime: SmolStr,
  /// Thumbnail dimensions (`mediaframe::frame::Dimensions`).
  dimensions: Dimensions,
  extractor: KeyframeExtractor,

  // --- apple-vision structured detections ---
  classifications: std::vec::Vec<Detection>,
  objects: std::vec::Vec<ObjectDetection>,
  humans: HumanAnalysis,
  animals: AnimalAnalysis,
  actions: std::vec::Vec<ActionDetection>,
  text_detections: std::vec::Vec<TextDetection>,
  barcodes: std::vec::Vec<BarcodeDetection>,
  attention_saliency: std::vec::Vec<SaliencyRegion>,
  objectness_saliency: std::vec::Vec<SaliencyRegion>,
  horizon: HorizonInfo,
  document_segments: std::vec::Vec<DocumentSegment>,
  aesthetics: Aesthetics,

  // --- colorthief ---
  colors: std::vec::Vec<DominantColor>,

  // --- VLM ---
  vlm: VlmAnalysis,
}

impl Keyframe<Uuid7> {
  /// Validating constructor.
  ///
  /// Rejects:
  /// - nil `id` (LanceDB embedding key collision),
  /// - nil `scene_id` (orphan keyframe with no `Scene`),
  /// - zero `dimensions` (a thumbnail with W=0 or H=0 is not a valid
  ///   image artifact; the locked spec calls out `dimensions` as
  ///   non-zero in its invariants).
  ///
  /// All analysis fields start empty and are filled in via the
  /// `with_*` / `set_*` mutators as apple-vision / colorthief / VLM
  /// stages land.
  pub fn try_new(
    id: Uuid7,
    scene_id: Uuid7,
    pts: Timestamp,
    dimensions: Dimensions,
    extractor: KeyframeExtractor,
  ) -> Result<Self, KeyframeError> {
    if id.is_nil() {
      return Err(KeyframeError::NilId);
    }
    if scene_id.is_nil() {
      return Err(KeyframeError::NilSceneId);
    }
    if dimensions.width() == 0 || dimensions.height() == 0 {
      return Err(KeyframeError::ZeroDimensions);
    }
    Ok(Self {
      id,
      scene_id,
      pts,
      data: Bytes::new(),
      mime: SmolStr::default(),
      dimensions,
      extractor,
      classifications: std::vec::Vec::new(),
      objects: std::vec::Vec::new(),
      humans: HumanAnalysis::new(),
      animals: AnimalAnalysis::new(),
      actions: std::vec::Vec::new(),
      text_detections: std::vec::Vec::new(),
      barcodes: std::vec::Vec::new(),
      attention_saliency: std::vec::Vec::new(),
      objectness_saliency: std::vec::Vec::new(),
      // 0.0 is trivially finite and in `0.0..=1.0` — this `try_new`
      // cannot fail.
      horizon: HorizonInfo::try_new(0.0, 0.0).expect("0.0 confidence is within range"),
      document_segments: std::vec::Vec::new(),
      aesthetics: Aesthetics::new(0.0, false),
      colors: std::vec::Vec::new(),
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
  #[inline(always)]
  pub const fn scene_id_ref(&self) -> &Id {
    &self.scene_id
  }
  #[inline(always)]
  pub const fn pts_ref(&self) -> &Timestamp {
    &self.pts
  }

  // --- artifact ---
  /// Thumbnail image bytes (inline, no `location`).
  #[inline(always)]
  pub fn data(&self) -> &[u8] {
    &self.data
  }
  /// Owned handle to the image bytes — O(1) refcount clone, no copy.
  #[inline(always)]
  pub fn data_bytes(&self) -> Bytes {
    self.data.clone()
  }
  /// MIME type (`""` = absent).
  #[inline(always)]
  pub fn mime(&self) -> &str {
    self.mime.as_str()
  }
  /// Byte size of `data` — **derived** from `data.len()`, never stored
  /// independently, so it can never diverge from the actual buffer.
  #[inline(always)]
  pub fn size(&self) -> u64 {
    self.data.len() as u64
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
  #[must_use]
  #[inline(always)]
  pub fn with_data(mut self, v: impl Into<Bytes>) -> Self {
    self.data = v.into();
    self
  }
  #[inline(always)]
  pub fn set_data(&mut self, v: impl Into<Bytes>) -> &mut Self {
    self.data = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_mime(mut self, v: impl Into<SmolStr>) -> Self {
    self.mime = v.into();
    self
  }
  #[inline(always)]
  pub fn set_mime(&mut self, v: impl Into<SmolStr>) -> &mut Self {
    self.mime = v.into();
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

  // --- apple-vision ---
  #[must_use]
  #[inline(always)]
  pub fn with_classifications(mut self, v: impl Into<std::vec::Vec<Detection>>) -> Self {
    self.classifications = v.into();
    self
  }
  #[inline(always)]
  pub fn set_classifications(&mut self, v: impl Into<std::vec::Vec<Detection>>) -> &mut Self {
    self.classifications = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_objects(mut self, v: impl Into<std::vec::Vec<ObjectDetection>>) -> Self {
    self.objects = v.into();
    self
  }
  #[inline(always)]
  pub fn set_objects(&mut self, v: impl Into<std::vec::Vec<ObjectDetection>>) -> &mut Self {
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
  pub fn with_actions(mut self, v: impl Into<std::vec::Vec<ActionDetection>>) -> Self {
    self.actions = v.into();
    self
  }
  #[inline(always)]
  pub fn set_actions(&mut self, v: impl Into<std::vec::Vec<ActionDetection>>) -> &mut Self {
    self.actions = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_text_detections(mut self, v: impl Into<std::vec::Vec<TextDetection>>) -> Self {
    self.text_detections = v.into();
    self
  }
  #[inline(always)]
  pub fn set_text_detections(&mut self, v: impl Into<std::vec::Vec<TextDetection>>) -> &mut Self {
    self.text_detections = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_barcodes(mut self, v: impl Into<std::vec::Vec<BarcodeDetection>>) -> Self {
    self.barcodes = v.into();
    self
  }
  #[inline(always)]
  pub fn set_barcodes(&mut self, v: impl Into<std::vec::Vec<BarcodeDetection>>) -> &mut Self {
    self.barcodes = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_attention_saliency(mut self, v: impl Into<std::vec::Vec<SaliencyRegion>>) -> Self {
    self.attention_saliency = v.into();
    self
  }
  #[inline(always)]
  pub fn set_attention_saliency(
    &mut self,
    v: impl Into<std::vec::Vec<SaliencyRegion>>,
  ) -> &mut Self {
    self.attention_saliency = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_objectness_saliency(mut self, v: impl Into<std::vec::Vec<SaliencyRegion>>) -> Self {
    self.objectness_saliency = v.into();
    self
  }
  #[inline(always)]
  pub fn set_objectness_saliency(
    &mut self,
    v: impl Into<std::vec::Vec<SaliencyRegion>>,
  ) -> &mut Self {
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
  pub fn with_document_segments(mut self, v: impl Into<std::vec::Vec<DocumentSegment>>) -> Self {
    self.document_segments = v.into();
    self
  }
  #[inline(always)]
  pub fn set_document_segments(
    &mut self,
    v: impl Into<std::vec::Vec<DocumentSegment>>,
  ) -> &mut Self {
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
  pub fn with_colors(mut self, v: impl Into<std::vec::Vec<DominantColor>>) -> Self {
    self.colors = v.into();
    self
  }
  #[inline(always)]
  pub fn set_colors(&mut self, v: impl Into<std::vec::Vec<DominantColor>>) -> &mut Self {
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
    let ts = Timestamp::new(1234, tb());
    let kf = Keyframe::try_new(
      Uuid7::new(),
      scene_id,
      ts,
      Dimensions::new(320, 180),
      KeyframeExtractor::CompositeQuality,
    )
    .unwrap();
    assert_eq!(kf.scene_id_ref(), &scene_id);
    assert_eq!(kf.pts_ref(), &ts);
    assert_eq!(kf.dimensions(), Dimensions::new(320, 180));
    assert!(kf.extractor().is_composite_quality());
    assert!(kf.data().is_empty());
    assert!(kf.classifications_slice().is_empty());
    assert!(kf.colors_slice().is_empty());
    assert_eq!(kf.vlm_ref().shot_type(), "");
  }

  #[test]
  fn try_new_rejects_nil_id_and_parent() {
    let ts = Timestamp::new(0, tb());
    assert_eq!(
      Keyframe::try_new(
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
      Keyframe::try_new(
        Uuid7::new(),
        Uuid7::nil(),
        ts,
        Dimensions::new(1, 1),
        KeyframeExtractor::Manual
      )
      .err(),
      Some(KeyframeError::NilSceneId)
    );
    assert!(KeyframeError::NilId.is_nil_id());
    assert!(KeyframeError::NilSceneId.is_nil_scene_id());
  }

  #[test]
  fn try_new_rejects_zero_dimensions() {
    let ts = Timestamp::new(0, tb());
    for (w, h) in [(0u32, 1u32), (1, 0), (0, 0)] {
      let r = Keyframe::try_new(
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
    let kf = Keyframe::try_new(
      Uuid7::new(),
      Uuid7::new(),
      ts,
      Dimensions::new(1920, 1080),
      KeyframeExtractor::IFrame,
    )
    .unwrap()
    .with_mime("image/jpeg")
    .with_data(std::vec![0xff, 0xd8, 0xff])
    .with_classifications(std::vec![Detection::try_new("dog", 0.97).unwrap()])
    .with_vlm(
      VlmAnalysis::new()
        .with_description(LocalizedText::from_src("a dog running"))
        .with_tags(std::vec![LocalizedText::from_src("dog")])
        .with_shot_type("medium-shot"),
    );
    assert_eq!(kf.mime(), "image/jpeg");
    // `size` is derived from `data` — 3 bytes in ⇒ size 3.
    assert_eq!(kf.size(), 3);
    assert_eq!(kf.data().len(), 3);
    assert_eq!(kf.classifications_slice().len(), 1);
    assert_eq!(kf.classifications_slice()[0].label(), "dog");
    assert_eq!(kf.vlm_ref().description_ref().src(), "a dog running");
    assert_eq!(kf.vlm_ref().tags_slice().len(), 1);
    assert_eq!(kf.vlm_ref().shot_type(), "medium-shot");

    let mut kf = kf;
    kf.set_mime("");
    kf.set_data(Bytes::new());
    kf.try_set_dimensions(Dimensions::new(2, 2)).unwrap();
    assert!(kf.mime().is_empty());
    // `size` tracks `data` with no separate setter — clearing `data`
    // drops `size` to 0 automatically.
    assert_eq!(kf.size(), 0);
    assert!(kf.data().is_empty());
    assert_eq!(kf.dimensions(), Dimensions::new(2, 2));
  }

  #[test]
  fn size_is_derived_from_data() {
    let ts = Timestamp::new(0, tb());
    let kf = Keyframe::try_new(
      Uuid7::new(),
      Uuid7::new(),
      ts,
      Dimensions::new(8, 8),
      KeyframeExtractor::IFrame,
    )
    .unwrap()
    .with_data(std::vec![1u8, 2, 3, 4, 5]);
    assert_eq!(kf.size(), kf.data().len() as u64);
    assert_eq!(kf.size(), 5);
  }

  #[test]
  fn dimension_mutators_reject_zero_extent() {
    let ts = Timestamp::new(0, tb());
    let mut kf = Keyframe::try_new(
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
  pub scene_id: Id,
  pub pts: Timestamp,
  pub data: Bytes,
  pub mime: SmolStr,
  pub dimensions: Dimensions,
  pub extractor: KeyframeExtractor,
  pub classifications: std::vec::Vec<Detection>,
  pub objects: std::vec::Vec<ObjectDetection>,
  pub humans: HumanAnalysis,
  pub animals: AnimalAnalysis,
  pub actions: std::vec::Vec<ActionDetection>,
  pub text_detections: std::vec::Vec<TextDetection>,
  pub barcodes: std::vec::Vec<BarcodeDetection>,
  pub attention_saliency: std::vec::Vec<SaliencyRegion>,
  pub objectness_saliency: std::vec::Vec<SaliencyRegion>,
  pub horizon: HorizonInfo,
  pub document_segments: std::vec::Vec<DocumentSegment>,
  pub aesthetics: Aesthetics,
  pub colors: std::vec::Vec<DominantColor>,
  pub vlm: VlmAnalysis,
}

impl<Id> Keyframe<Id> {
  /// Decompose into [`KeyframeParts`] — exhaustive, by value.
  #[inline(always)]
  pub fn into_parts(self) -> KeyframeParts<Id> {
    let Self {
      id,
      scene_id,
      pts,
      data,
      mime,
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
      pts,
      data,
      mime,
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
      pts,
      data,
      mime,
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
      pts,
      data,
      mime,
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
