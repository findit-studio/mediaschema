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
/// **No `Default`** — defaulting to nil `id`/`parent` would be an
/// orphan thumbnail with no scene. Construct via [`Keyframe::try_new`]
/// (which also rejects zero-`dimensions`, per the locked invariant
/// that a thumbnail has positive W and H).
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframe<Id = Uuid7> {
  // --- identity / source ---
  id: Id,
  parent: Id,
  pts: Timestamp,

  // --- artifact ---
  /// Inline thumbnail image bytes (`bytes::Bytes`, no `location`).
  data: Bytes,
  mime: SmolStr,
  size: u32,
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
  /// - nil `parent` (orphan keyframe with no `Scene`),
  /// - zero `dimensions` (a thumbnail with W=0 or H=0 is not a valid
  ///   image artifact; the locked spec calls out `dimensions` as
  ///   non-zero in its invariants).
  ///
  /// All analysis fields start empty and are filled in via the
  /// `with_*` / `set_*` mutators as apple-vision / colorthief / VLM
  /// stages land.
  pub fn try_new(
    id: Uuid7,
    parent: Uuid7,
    pts: Timestamp,
    dimensions: Dimensions,
    extractor: KeyframeExtractor,
  ) -> Result<Self, KeyframeError> {
    if id.is_nil() {
      return Err(KeyframeError::NilId);
    }
    if parent.is_nil() {
      return Err(KeyframeError::NilParent);
    }
    if dimensions.width() == 0 || dimensions.height() == 0 {
      return Err(KeyframeError::ZeroDimensions);
    }
    Ok(Self {
      id,
      parent,
      pts,
      data: Bytes::new(),
      mime: SmolStr::default(),
      size: 0,
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
      horizon: HorizonInfo::new(0.0, 0.0),
      document_segments: std::vec::Vec::new(),
      aesthetics: Aesthetics::new(0.0, false),
      colors: std::vec::Vec::new(),
      vlm: VlmAnalysis::new(),
    })
  }
}

impl<Id> Keyframe<Id> {
  // --- identity / source ---
  #[inline]
  pub const fn id(&self) -> &Id {
    &self.id
  }
  #[inline]
  pub const fn parent(&self) -> &Id {
    &self.parent
  }
  #[inline]
  pub const fn pts(&self) -> &Timestamp {
    &self.pts
  }

  // --- artifact ---
  /// Thumbnail image bytes (inline, no `location`).
  #[inline]
  pub fn data(&self) -> &[u8] {
    &self.data
  }
  /// MIME type (`""` = absent).
  #[inline]
  pub fn mime(&self) -> &str {
    self.mime.as_str()
  }
  /// Byte size of `data`.
  #[inline]
  pub const fn size(&self) -> u32 {
    self.size
  }
  /// Thumbnail dimensions (`mediaframe::frame::Dimensions`).
  #[inline]
  pub const fn dimensions(&self) -> Dimensions {
    self.dimensions
  }
  /// Which extractor produced this keyframe.
  #[inline]
  pub const fn extractor(&self) -> KeyframeExtractor {
    self.extractor
  }

  // --- apple-vision ---
  #[inline]
  pub fn classifications(&self) -> &[Detection] {
    &self.classifications
  }
  #[inline]
  pub fn objects(&self) -> &[ObjectDetection] {
    &self.objects
  }
  #[inline]
  pub const fn humans(&self) -> &HumanAnalysis {
    &self.humans
  }
  #[inline]
  pub const fn animals(&self) -> &AnimalAnalysis {
    &self.animals
  }
  #[inline]
  pub fn actions(&self) -> &[ActionDetection] {
    &self.actions
  }
  #[inline]
  pub fn text_detections(&self) -> &[TextDetection] {
    &self.text_detections
  }
  #[inline]
  pub fn barcodes(&self) -> &[BarcodeDetection] {
    &self.barcodes
  }
  #[inline]
  pub fn attention_saliency(&self) -> &[SaliencyRegion] {
    &self.attention_saliency
  }
  #[inline]
  pub fn objectness_saliency(&self) -> &[SaliencyRegion] {
    &self.objectness_saliency
  }
  #[inline]
  pub const fn horizon(&self) -> &HorizonInfo {
    &self.horizon
  }
  #[inline]
  pub fn document_segments(&self) -> &[DocumentSegment] {
    &self.document_segments
  }
  #[inline]
  pub const fn aesthetics(&self) -> &Aesthetics {
    &self.aesthetics
  }

  // --- colorthief ---
  #[inline]
  pub fn colors(&self) -> &[DominantColor] {
    &self.colors
  }

  // --- VLM ---
  #[inline]
  pub const fn vlm(&self) -> &VlmAnalysis {
    &self.vlm
  }
}

// Builders + setters per the encapsulation rule.
impl<Id> Keyframe<Id> {
  // --- artifact ---
  #[inline]
  pub fn with_data(mut self, v: impl Into<Bytes>) -> Self {
    self.data = v.into();
    self
  }
  #[inline]
  pub fn set_data(&mut self, v: impl Into<Bytes>) {
    self.data = v.into();
  }
  #[inline]
  pub fn with_mime(mut self, v: impl Into<SmolStr>) -> Self {
    self.mime = v.into();
    self
  }
  #[inline]
  pub fn set_mime(&mut self, v: impl Into<SmolStr>) {
    self.mime = v.into();
  }
  #[inline]
  pub const fn with_size(mut self, v: u32) -> Self {
    self.size = v;
    self
  }
  #[inline]
  pub const fn set_size(&mut self, v: u32) {
    self.size = v;
  }
  #[inline]
  pub const fn with_dimensions(mut self, v: Dimensions) -> Self {
    self.dimensions = v;
    self
  }
  #[inline]
  pub const fn set_dimensions(&mut self, v: Dimensions) {
    self.dimensions = v;
  }
  #[inline]
  pub const fn with_extractor(mut self, v: KeyframeExtractor) -> Self {
    self.extractor = v;
    self
  }
  #[inline]
  pub const fn set_extractor(&mut self, v: KeyframeExtractor) {
    self.extractor = v;
  }

  // --- apple-vision ---
  #[inline]
  pub fn with_classifications(mut self, v: impl Into<std::vec::Vec<Detection>>) -> Self {
    self.classifications = v.into();
    self
  }
  #[inline]
  pub fn set_classifications(&mut self, v: impl Into<std::vec::Vec<Detection>>) {
    self.classifications = v.into();
  }
  #[inline]
  pub fn with_objects(mut self, v: impl Into<std::vec::Vec<ObjectDetection>>) -> Self {
    self.objects = v.into();
    self
  }
  #[inline]
  pub fn set_objects(&mut self, v: impl Into<std::vec::Vec<ObjectDetection>>) {
    self.objects = v.into();
  }
  #[inline]
  pub fn with_humans(mut self, v: HumanAnalysis) -> Self {
    self.humans = v;
    self
  }
  #[inline]
  pub fn set_humans(&mut self, v: HumanAnalysis) {
    self.humans = v;
  }
  #[inline]
  pub fn with_animals(mut self, v: AnimalAnalysis) -> Self {
    self.animals = v;
    self
  }
  #[inline]
  pub fn set_animals(&mut self, v: AnimalAnalysis) {
    self.animals = v;
  }
  #[inline]
  pub fn with_actions(mut self, v: impl Into<std::vec::Vec<ActionDetection>>) -> Self {
    self.actions = v.into();
    self
  }
  #[inline]
  pub fn set_actions(&mut self, v: impl Into<std::vec::Vec<ActionDetection>>) {
    self.actions = v.into();
  }
  #[inline]
  pub fn with_text_detections(mut self, v: impl Into<std::vec::Vec<TextDetection>>) -> Self {
    self.text_detections = v.into();
    self
  }
  #[inline]
  pub fn set_text_detections(&mut self, v: impl Into<std::vec::Vec<TextDetection>>) {
    self.text_detections = v.into();
  }
  #[inline]
  pub fn with_barcodes(mut self, v: impl Into<std::vec::Vec<BarcodeDetection>>) -> Self {
    self.barcodes = v.into();
    self
  }
  #[inline]
  pub fn set_barcodes(&mut self, v: impl Into<std::vec::Vec<BarcodeDetection>>) {
    self.barcodes = v.into();
  }
  #[inline]
  pub fn with_attention_saliency(mut self, v: impl Into<std::vec::Vec<SaliencyRegion>>) -> Self {
    self.attention_saliency = v.into();
    self
  }
  #[inline]
  pub fn set_attention_saliency(&mut self, v: impl Into<std::vec::Vec<SaliencyRegion>>) {
    self.attention_saliency = v.into();
  }
  #[inline]
  pub fn with_objectness_saliency(mut self, v: impl Into<std::vec::Vec<SaliencyRegion>>) -> Self {
    self.objectness_saliency = v.into();
    self
  }
  #[inline]
  pub fn set_objectness_saliency(&mut self, v: impl Into<std::vec::Vec<SaliencyRegion>>) {
    self.objectness_saliency = v.into();
  }
  #[inline]
  pub const fn with_horizon(mut self, v: HorizonInfo) -> Self {
    self.horizon = v;
    self
  }
  #[inline]
  pub const fn set_horizon(&mut self, v: HorizonInfo) {
    self.horizon = v;
  }
  #[inline]
  pub fn with_document_segments(mut self, v: impl Into<std::vec::Vec<DocumentSegment>>) -> Self {
    self.document_segments = v.into();
    self
  }
  #[inline]
  pub fn set_document_segments(&mut self, v: impl Into<std::vec::Vec<DocumentSegment>>) {
    self.document_segments = v.into();
  }
  #[inline]
  pub const fn with_aesthetics(mut self, v: Aesthetics) -> Self {
    self.aesthetics = v;
    self
  }
  #[inline]
  pub const fn set_aesthetics(&mut self, v: Aesthetics) {
    self.aesthetics = v;
  }

  // --- colorthief ---
  #[inline]
  pub fn with_colors(mut self, v: impl Into<std::vec::Vec<DominantColor>>) -> Self {
    self.colors = v.into();
    self
  }
  #[inline]
  pub fn set_colors(&mut self, v: impl Into<std::vec::Vec<DominantColor>>) {
    self.colors = v.into();
  }

  // --- VLM ---
  #[inline]
  pub fn with_vlm(mut self, v: VlmAnalysis) -> Self {
    self.vlm = v;
    self
  }
  #[inline]
  pub fn set_vlm(&mut self, v: VlmAnalysis) {
    self.vlm = v;
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
  /// Supplied `parent` was the nil sentinel — orphan keyframe with no
  /// `Scene` reference.
  #[error("Keyframe parent (Scene) must not be the nil UUID")]
  NilParent,
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
    let parent = Uuid7::new();
    let ts = Timestamp::new(1234, tb());
    let kf = Keyframe::try_new(
      Uuid7::new(),
      parent,
      ts,
      Dimensions::new(320, 180),
      KeyframeExtractor::CompositeQuality,
    )
    .unwrap();
    assert_eq!(kf.parent(), &parent);
    assert_eq!(kf.pts(), &ts);
    assert_eq!(kf.dimensions(), Dimensions::new(320, 180));
    assert!(kf.extractor().is_composite_quality());
    assert!(kf.data().is_empty());
    assert!(kf.classifications().is_empty());
    assert!(kf.colors().is_empty());
    assert_eq!(kf.vlm().shot_type(), "");
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
      Some(KeyframeError::NilParent)
    );
    assert!(KeyframeError::NilId.is_nil_id());
    assert!(KeyframeError::NilParent.is_nil_parent());
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
    .with_size(42_000)
    .with_data(std::vec![0xff, 0xd8, 0xff])
    .with_classifications(std::vec![Detection::new("dog", 0.97)])
    .with_vlm(
      VlmAnalysis::new()
        .with_description(LocalizedText::from_src("a dog running"))
        .with_tags(std::vec![LocalizedText::from_src("dog")])
        .with_shot_type("medium-shot"),
    );
    assert_eq!(kf.mime(), "image/jpeg");
    assert_eq!(kf.size(), 42_000);
    assert_eq!(kf.data().len(), 3);
    assert_eq!(kf.classifications().len(), 1);
    assert_eq!(kf.classifications()[0].label(), "dog");
    assert_eq!(kf.vlm().description().src(), "a dog running");
    assert_eq!(kf.vlm().tags().len(), 1);
    assert_eq!(kf.vlm().shot_type(), "medium-shot");

    let mut kf = kf;
    kf.set_mime("");
    kf.set_size(0);
    kf.set_data(Bytes::new());
    kf.set_dimensions(Dimensions::new(2, 2));
    assert!(kf.mime().is_empty());
    assert_eq!(kf.size(), 0);
    assert!(kf.data().is_empty());
    assert_eq!(kf.dimensions(), Dimensions::new(2, 2));
  }
}
