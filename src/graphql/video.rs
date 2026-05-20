//! GraphQL exposure of the Video aggregates and detection VOs.

use async_graphql::{Object, ID};

use crate::domain::{
  aggregates::video::{
    detections::{
      ActionDetection, Aesthetics, AnimalAnalysis, BarcodeDetection, BodyPose3DDetection,
      BodyPose3DHeightEstimation, BodyPose3DJoint, BodyPoseDetection, BodyPoseJoint, BoundingBox,
      Detection, DocumentSegment, DominantColor, FaceDetection, FaceLandmarkRegion,
      FaceLandmarksDetection, HandChirality, HandPoseDetection, HorizonInfo, HumanAnalysis,
      ObjectDetection, PersonInstanceMaskDetection, PersonSegmentationMask, SaliencyRegion,
      SubjectDetection, TextDetection, VlmAnalysis,
    },
    facet::IndexProgress as VideoIndexProgress,
    track::{
      ColorInfoPlaceholder, DolbyVisionConfigPlaceholder, HdrStaticMetadataPlaceholder,
      RectPlaceholder, VideoCodec,
    },
  },
  Keyframe, Scene, Uuid7, Video, VideoTrack,
};

use super::{
  bitflags::GqlVideoIndexStatus,
  enums::{GqlKeyframeExtractor, GqlSceneDetector},
  media::{GqlErrorInfo, GqlLocalizedText, GqlProvenance, GqlRgba},
  scalars::{empty_as_none, GqlMediaTimeRange, GqlMediaTimestamp},
};

// ===========================================================================
// IndexProgress (video-side)
// ===========================================================================

/// GraphQL wrapper for the video-side [`VideoIndexProgress`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GqlVideoIndexProgress(pub VideoIndexProgress);

impl From<VideoIndexProgress> for GqlVideoIndexProgress {
  #[inline]
  fn from(v: VideoIndexProgress) -> Self {
    Self(v)
  }
}
impl From<GqlVideoIndexProgress> for VideoIndexProgress {
  #[inline]
  fn from(v: GqlVideoIndexProgress) -> Self {
    v.0
  }
}

#[Object(name = "VideoIndexProgress")]
impl GqlVideoIndexProgress {
  async fn total(&self) -> u32 {
    self.0.total()
  }
  async fn indexed(&self) -> u32 {
    self.0.indexed()
  }
  async fn failed(&self) -> u32 {
    self.0.failed()
  }
  async fn has_failures(&self) -> bool {
    self.0.has_failures()
  }
}

// ===========================================================================
// VideoCodec — mixed enum (unit + Other(SmolStr))
// ===========================================================================

/// GraphQL wrapper for [`VideoCodec`]. Exposed as `{ tag, other }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GqlVideoCodec(pub VideoCodec);

impl From<VideoCodec> for GqlVideoCodec {
  #[inline]
  fn from(v: VideoCodec) -> Self {
    Self(v)
  }
}
impl From<GqlVideoCodec> for VideoCodec {
  #[inline]
  fn from(v: GqlVideoCodec) -> Self {
    v.0
  }
}

#[Object(name = "VideoCodec")]
impl GqlVideoCodec {
  /// Canonical variant tag (`H264`, `H265`, `OTHER`, …).
  async fn tag(&self) -> String {
    match &self.0 {
      VideoCodec::H264 => "H264",
      VideoCodec::H265 => "H265",
      VideoCodec::H266 => "H266",
      VideoCodec::Vp8 => "VP8",
      VideoCodec::Vp9 => "VP9",
      VideoCodec::Av1 => "AV1",
      VideoCodec::Mpeg2 => "MPEG2",
      VideoCodec::Mpeg4 => "MPEG4",
      VideoCodec::ProRes => "PRORES",
      VideoCodec::Dnxhd => "DNXHD",
      VideoCodec::Theora => "THEORA",
      VideoCodec::Other(_) => "OTHER",
      // `VideoCodec` is `#[non_exhaustive]`. Unreachable today.
      #[allow(unreachable_patterns)]
      _ => "OTHER",
    }
    .into()
  }
  /// Wire-preserved slug for `OTHER`; `null` for named variants.
  async fn other(&self) -> Option<String> {
    match &self.0 {
      VideoCodec::Other(s) => Some(s.to_string()),
      _ => None,
    }
  }
}

// ===========================================================================
// mediaframe-placeholder VOs
// ===========================================================================

/// GraphQL wrapper for the placeholder rect VO on `VideoTrack`. Will
/// be replaced by `mediaframe::Rect` post-`0.1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GqlRect(pub RectPlaceholder);

impl From<RectPlaceholder> for GqlRect {
  #[inline]
  fn from(v: RectPlaceholder) -> Self {
    Self(v)
  }
}
impl From<GqlRect> for RectPlaceholder {
  #[inline]
  fn from(v: GqlRect) -> Self {
    v.0
  }
}

#[Object(name = "Rect")]
impl GqlRect {
  async fn x(&self) -> u32 {
    self.0.x()
  }
  async fn y(&self) -> u32 {
    self.0.y()
  }
  async fn width(&self) -> u32 {
    self.0.width()
  }
  async fn height(&self) -> u32 {
    self.0.height()
  }
}

/// GraphQL wrapper for the placeholder colour-info VO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GqlColorInfo(pub ColorInfoPlaceholder);

impl From<ColorInfoPlaceholder> for GqlColorInfo {
  #[inline]
  fn from(v: ColorInfoPlaceholder) -> Self {
    Self(v)
  }
}
impl From<GqlColorInfo> for ColorInfoPlaceholder {
  #[inline]
  fn from(v: GqlColorInfo) -> Self {
    v.0
  }
}

#[Object(name = "ColorInfo")]
impl GqlColorInfo {
  async fn primaries(&self) -> u32 {
    self.0.primaries()
  }
  async fn transfer(&self) -> u32 {
    self.0.transfer()
  }
  async fn matrix(&self) -> u32 {
    self.0.matrix()
  }
  async fn range(&self) -> u32 {
    self.0.range()
  }
  async fn chroma_location(&self) -> u32 {
    self.0.chroma_location()
  }
}

/// GraphQL wrapper for the placeholder HDR-static VO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GqlHdrStaticMetadata(pub HdrStaticMetadataPlaceholder);

impl From<HdrStaticMetadataPlaceholder> for GqlHdrStaticMetadata {
  #[inline]
  fn from(v: HdrStaticMetadataPlaceholder) -> Self {
    Self(v)
  }
}
impl From<GqlHdrStaticMetadata> for HdrStaticMetadataPlaceholder {
  #[inline]
  fn from(v: GqlHdrStaticMetadata) -> Self {
    v.0
  }
}

#[Object(name = "HdrStaticMetadata")]
impl GqlHdrStaticMetadata {
  async fn max_cll(&self) -> u32 {
    self.0.max_cll()
  }
  async fn max_fall(&self) -> u32 {
    self.0.max_fall()
  }
}

/// GraphQL wrapper for the placeholder Dolby-Vision-config VO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GqlDolbyVisionConfig(pub DolbyVisionConfigPlaceholder);

impl From<DolbyVisionConfigPlaceholder> for GqlDolbyVisionConfig {
  #[inline]
  fn from(v: DolbyVisionConfigPlaceholder) -> Self {
    Self(v)
  }
}
impl From<GqlDolbyVisionConfig> for DolbyVisionConfigPlaceholder {
  #[inline]
  fn from(v: GqlDolbyVisionConfig) -> Self {
    v.0
  }
}

#[Object(name = "DolbyVisionConfig")]
impl GqlDolbyVisionConfig {
  async fn profile(&self) -> u32 {
    u32::from(self.0.profile())
  }
  async fn level(&self) -> u32 {
    u32::from(self.0.level())
  }
  async fn rpu_present(&self) -> bool {
    self.0.rpu_present()
  }
  async fn el_present(&self) -> bool {
    self.0.el_present()
  }
  async fn bl_present(&self) -> bool {
    self.0.bl_present()
  }
  async fn bl_signal_compatibility_id(&self) -> u32 {
    u32::from(self.0.bl_signal_compatibility_id())
  }
}

// ===========================================================================
// Misc projection objects
// ===========================================================================

/// Frame rate `{num, den, is_vfr}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GqlFrameRate {
  pub num: u32,
  pub den: u32,
  pub is_vfr: bool,
}

#[Object(name = "FrameRate")]
impl GqlFrameRate {
  async fn num(&self) -> u32 {
    self.num
  }
  async fn den(&self) -> u32 {
    self.den
  }
  async fn is_vfr(&self) -> bool {
    self.is_vfr
  }
}

/// Rational `{num, den}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GqlRational {
  pub num: u32,
  pub den: u32,
}

#[Object(name = "Rational")]
impl GqlRational {
  async fn num(&self) -> u32 {
    self.num
  }
  async fn den(&self) -> u32 {
    self.den
  }
}

/// Dimensions `{width, height}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GqlDimensions {
  pub width: u32,
  pub height: u32,
}

#[Object(name = "Dimensions")]
impl GqlDimensions {
  async fn width(&self) -> u32 {
    self.width
  }
  async fn height(&self) -> u32 {
    self.height
  }
}

/// 2D point `{x, y}`.
#[derive(Debug, Clone, Copy)]
pub struct GqlPoint2D {
  pub x: f32,
  pub y: f32,
}

#[Object(name = "Point2D")]
impl GqlPoint2D {
  async fn x(&self) -> f32 {
    self.x
  }
  async fn y(&self) -> f32 {
    self.y
  }
}

// ===========================================================================
// Video facet
// ===========================================================================

/// GraphQL wrapper for [`Video`].
#[derive(Debug, Clone)]
pub struct GqlVideo(pub Video<Uuid7>);

impl From<Video<Uuid7>> for GqlVideo {
  #[inline]
  fn from(v: Video<Uuid7>) -> Self {
    Self(v)
  }
}
impl From<GqlVideo> for Video<Uuid7> {
  #[inline]
  fn from(v: GqlVideo) -> Self {
    v.0
  }
}

#[Object(name = "Video")]
impl GqlVideo {
  async fn id(&self) -> ID {
    ID(self.0.id().to_string())
  }
  async fn total_scenes(&self) -> u32 {
    self.0.total_scenes()
  }
  async fn tracks(&self) -> std::vec::Vec<ID> {
    self
      .0
      .tracks()
      .iter()
      .map(|id| ID(id.to_string()))
      .collect()
  }
  async fn track_progress(&self) -> GqlVideoIndexProgress {
    GqlVideoIndexProgress(*self.0.track_progress())
  }
}

// ===========================================================================
// VideoTrack
// ===========================================================================

/// GraphQL wrapper for [`VideoTrack`].
#[derive(Debug, Clone)]
pub struct GqlVideoTrack(pub VideoTrack<Uuid7>);

impl From<VideoTrack<Uuid7>> for GqlVideoTrack {
  #[inline]
  fn from(v: VideoTrack<Uuid7>) -> Self {
    Self(v)
  }
}
impl From<GqlVideoTrack> for VideoTrack<Uuid7> {
  #[inline]
  fn from(v: GqlVideoTrack) -> Self {
    v.0
  }
}

#[Object(name = "VideoTrack")]
impl GqlVideoTrack {
  async fn id(&self) -> ID {
    ID(self.0.id().to_string())
  }
  async fn parent(&self) -> ID {
    ID(self.0.parent().to_string())
  }
  async fn stream_index(&self) -> Option<u32> {
    self.0.stream_index()
  }
  async fn container_track_id(&self) -> Option<String> {
    self.0.container_track_id().map(|v| v.to_string())
  }
  async fn start_pts(&self) -> Option<GqlMediaTimestamp> {
    self.0.start_pts().copied().map(GqlMediaTimestamp)
  }
  async fn duration(&self) -> Option<GqlMediaTimestamp> {
    self.0.duration().copied().map(GqlMediaTimestamp)
  }
  async fn codec(&self) -> GqlVideoCodec {
    GqlVideoCodec(self.0.codec().clone())
  }
  async fn profile(&self) -> Option<String> {
    self.0.profile().map(|s| s.to_string())
  }
  async fn level(&self) -> Option<u32> {
    self.0.level().map(u32::from)
  }
  async fn bit_rate(&self) -> String {
    self.0.bit_rate().to_string()
  }
  async fn nb_frames(&self) -> Option<String> {
    self.0.nb_frames().map(|v| v.to_string())
  }
  async fn has_b_frames(&self) -> bool {
    self.0.has_b_frames()
  }
  async fn closed_gop(&self) -> Option<bool> {
    self.0.closed_gop()
  }
  async fn bits_per_raw_sample(&self) -> Option<u32> {
    self.0.bits_per_raw_sample().map(u32::from)
  }
  async fn dimensions(&self) -> GqlDimensions {
    let (width, height) = self.0.dimensions();
    GqlDimensions { width, height }
  }
  async fn visible_rect(&self) -> Option<GqlRect> {
    self.0.visible_rect().copied().map(GqlRect)
  }
  async fn sample_aspect_ratio(&self) -> GqlRational {
    let (num, den) = self.0.sample_aspect_ratio();
    GqlRational { num, den }
  }
  async fn pixel_format(&self) -> u32 {
    self.0.pixel_format()
  }
  async fn color(&self) -> GqlColorInfo {
    GqlColorInfo(*self.0.color())
  }
  async fn hdr_static(&self) -> Option<GqlHdrStaticMetadata> {
    self.0.hdr_static().copied().map(GqlHdrStaticMetadata)
  }
  async fn rotation(&self) -> u32 {
    self.0.rotation()
  }
  async fn frame_rate(&self) -> GqlFrameRate {
    let (num, den, is_vfr) = self.0.frame_rate();
    GqlFrameRate { num, den, is_vfr }
  }
  async fn field_order(&self) -> u32 {
    self.0.field_order()
  }
  async fn stereo_mode(&self) -> Option<u32> {
    self.0.stereo_mode()
  }
  async fn dovi(&self) -> Option<GqlDolbyVisionConfig> {
    self.0.dovi().copied().map(GqlDolbyVisionConfig)
  }
  async fn has_embedded_captions(&self) -> bool {
    self.0.has_embedded_captions()
  }
  async fn disposition(&self) -> u32 {
    self.0.disposition()
  }
  async fn is_primary(&self) -> bool {
    self.0.is_primary()
  }
  async fn auto_selected(&self) -> bool {
    self.0.auto_selected()
  }
  async fn scenes(&self) -> std::vec::Vec<ID> {
    self
      .0
      .scenes()
      .iter()
      .map(|id| ID(id.to_string()))
      .collect()
  }
  async fn index_status(&self) -> GqlVideoIndexStatus {
    self.0.index_status().into()
  }
  async fn index_errors(&self) -> std::vec::Vec<GqlErrorInfo> {
    self
      .0
      .index_errors()
      .iter()
      .cloned()
      .map(GqlErrorInfo)
      .collect()
  }
  async fn provenance(&self) -> GqlProvenance {
    GqlProvenance(self.0.provenance().clone())
  }
}

// ===========================================================================
// Scene
// ===========================================================================

/// GraphQL wrapper for [`Scene`].
#[derive(Debug, Clone)]
pub struct GqlScene(pub Scene<Uuid7>);

impl From<Scene<Uuid7>> for GqlScene {
  #[inline]
  fn from(v: Scene<Uuid7>) -> Self {
    Self(v)
  }
}
impl From<GqlScene> for Scene<Uuid7> {
  #[inline]
  fn from(v: GqlScene) -> Self {
    v.0
  }
}

#[Object(name = "Scene")]
impl GqlScene {
  async fn id(&self) -> ID {
    ID(self.0.id().to_string())
  }
  async fn parent(&self) -> ID {
    ID(self.0.parent().to_string())
  }
  async fn index(&self) -> u32 {
    self.0.index()
  }
  async fn span(&self) -> GqlMediaTimeRange {
    GqlMediaTimeRange(*self.0.span())
  }
  async fn detector(&self) -> GqlSceneDetector {
    self.0.detector().into()
  }
  async fn keyframes(&self) -> std::vec::Vec<ID> {
    self
      .0
      .keyframes()
      .iter()
      .map(|id| ID(id.to_string()))
      .collect()
  }
  async fn description(&self) -> Option<String> {
    empty_as_none(self.0.description())
  }
}

// ===========================================================================
// Detection VOs
// ===========================================================================

/// GraphQL wrapper for [`Detection`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlDetection(pub Detection);

impl From<Detection> for GqlDetection {
  #[inline]
  fn from(v: Detection) -> Self {
    Self(v)
  }
}
impl From<GqlDetection> for Detection {
  #[inline]
  fn from(v: GqlDetection) -> Self {
    v.0
  }
}

#[Object(name = "Detection")]
impl GqlDetection {
  async fn label(&self) -> String {
    self.0.label().to_string()
  }
  async fn confidence(&self) -> f32 {
    self.0.confidence()
  }
}

/// GraphQL wrapper for [`BoundingBox`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GqlBoundingBox(pub BoundingBox);

impl From<BoundingBox> for GqlBoundingBox {
  #[inline]
  fn from(v: BoundingBox) -> Self {
    Self(v)
  }
}
impl From<GqlBoundingBox> for BoundingBox {
  #[inline]
  fn from(v: GqlBoundingBox) -> Self {
    v.0
  }
}

#[Object(name = "BoundingBox")]
impl GqlBoundingBox {
  async fn x(&self) -> f32 {
    self.0.x()
  }
  async fn y(&self) -> f32 {
    self.0.y()
  }
  async fn width(&self) -> f32 {
    self.0.width()
  }
  async fn height(&self) -> f32 {
    self.0.height()
  }
}

/// GraphQL wrapper for [`ObjectDetection`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlObjectDetection(pub ObjectDetection);

impl From<ObjectDetection> for GqlObjectDetection {
  #[inline]
  fn from(v: ObjectDetection) -> Self {
    Self(v)
  }
}
impl From<GqlObjectDetection> for ObjectDetection {
  #[inline]
  fn from(v: GqlObjectDetection) -> Self {
    v.0
  }
}

#[Object(name = "ObjectDetection")]
impl GqlObjectDetection {
  async fn detection(&self) -> GqlDetection {
    GqlDetection(self.0.detection().clone())
  }
  async fn bbox(&self) -> Option<GqlBoundingBox> {
    self.0.bbox().copied().map(GqlBoundingBox)
  }
}

/// GraphQL wrapper for [`ActionDetection`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlActionDetection(pub ActionDetection);

impl From<ActionDetection> for GqlActionDetection {
  #[inline]
  fn from(v: ActionDetection) -> Self {
    Self(v)
  }
}
impl From<GqlActionDetection> for ActionDetection {
  #[inline]
  fn from(v: GqlActionDetection) -> Self {
    v.0
  }
}

#[Object(name = "ActionDetection")]
impl GqlActionDetection {
  async fn detection(&self) -> GqlDetection {
    GqlDetection(self.0.detection().clone())
  }
}

/// GraphQL wrapper for [`TextDetection`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlTextDetection(pub TextDetection);

impl From<TextDetection> for GqlTextDetection {
  #[inline]
  fn from(v: TextDetection) -> Self {
    Self(v)
  }
}
impl From<GqlTextDetection> for TextDetection {
  #[inline]
  fn from(v: GqlTextDetection) -> Self {
    v.0
  }
}

#[Object(name = "TextDetection")]
impl GqlTextDetection {
  async fn text(&self) -> String {
    self.0.text().to_string()
  }
  async fn confidence(&self) -> f32 {
    self.0.confidence()
  }
  async fn bbox(&self) -> GqlBoundingBox {
    GqlBoundingBox(*self.0.bbox())
  }
}

/// GraphQL wrapper for [`BarcodeDetection`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlBarcodeDetection(pub BarcodeDetection);

impl From<BarcodeDetection> for GqlBarcodeDetection {
  #[inline]
  fn from(v: BarcodeDetection) -> Self {
    Self(v)
  }
}
impl From<GqlBarcodeDetection> for BarcodeDetection {
  #[inline]
  fn from(v: GqlBarcodeDetection) -> Self {
    v.0
  }
}

#[Object(name = "BarcodeDetection")]
impl GqlBarcodeDetection {
  async fn payload(&self) -> String {
    self.0.payload().to_string()
  }
  async fn symbology(&self) -> String {
    self.0.symbology().to_string()
  }
  async fn confidence(&self) -> f32 {
    self.0.confidence()
  }
  async fn bbox(&self) -> GqlBoundingBox {
    GqlBoundingBox(*self.0.bbox())
  }
}

/// GraphQL wrapper for [`SaliencyRegion`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GqlSaliencyRegion(pub SaliencyRegion);

impl From<SaliencyRegion> for GqlSaliencyRegion {
  #[inline]
  fn from(v: SaliencyRegion) -> Self {
    Self(v)
  }
}
impl From<GqlSaliencyRegion> for SaliencyRegion {
  #[inline]
  fn from(v: GqlSaliencyRegion) -> Self {
    v.0
  }
}

#[Object(name = "SaliencyRegion")]
impl GqlSaliencyRegion {
  async fn bbox(&self) -> GqlBoundingBox {
    GqlBoundingBox(*self.0.bbox())
  }
  async fn confidence(&self) -> f32 {
    self.0.confidence()
  }
}

/// GraphQL wrapper for [`HorizonInfo`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GqlHorizonInfo(pub HorizonInfo);

impl From<HorizonInfo> for GqlHorizonInfo {
  #[inline]
  fn from(v: HorizonInfo) -> Self {
    Self(v)
  }
}
impl From<GqlHorizonInfo> for HorizonInfo {
  #[inline]
  fn from(v: GqlHorizonInfo) -> Self {
    v.0
  }
}

#[Object(name = "HorizonInfo")]
impl GqlHorizonInfo {
  async fn angle(&self) -> f32 {
    self.0.angle()
  }
  async fn confidence(&self) -> f32 {
    self.0.confidence()
  }
}

/// GraphQL wrapper for [`DocumentSegment`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GqlDocumentSegment(pub DocumentSegment);

impl From<DocumentSegment> for GqlDocumentSegment {
  #[inline]
  fn from(v: DocumentSegment) -> Self {
    Self(v)
  }
}
impl From<GqlDocumentSegment> for DocumentSegment {
  #[inline]
  fn from(v: GqlDocumentSegment) -> Self {
    v.0
  }
}

#[Object(name = "DocumentSegment")]
impl GqlDocumentSegment {
  async fn top_left(&self) -> GqlPoint2D {
    let (x, y) = self.0.top_left();
    GqlPoint2D { x, y }
  }
  async fn top_right(&self) -> GqlPoint2D {
    let (x, y) = self.0.top_right();
    GqlPoint2D { x, y }
  }
  async fn bottom_right(&self) -> GqlPoint2D {
    let (x, y) = self.0.bottom_right();
    GqlPoint2D { x, y }
  }
  async fn bottom_left(&self) -> GqlPoint2D {
    let (x, y) = self.0.bottom_left();
    GqlPoint2D { x, y }
  }
  async fn confidence(&self) -> f32 {
    self.0.confidence()
  }
}

/// GraphQL wrapper for [`BodyPoseJoint`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlBodyPoseJoint(pub BodyPoseJoint);

impl From<BodyPoseJoint> for GqlBodyPoseJoint {
  #[inline]
  fn from(v: BodyPoseJoint) -> Self {
    Self(v)
  }
}
impl From<GqlBodyPoseJoint> for BodyPoseJoint {
  #[inline]
  fn from(v: GqlBodyPoseJoint) -> Self {
    v.0
  }
}

#[Object(name = "BodyPoseJoint")]
impl GqlBodyPoseJoint {
  async fn name(&self) -> String {
    self.0.name().to_string()
  }
  async fn x(&self) -> f32 {
    self.0.x()
  }
  async fn y(&self) -> f32 {
    self.0.y()
  }
  async fn confidence(&self) -> f32 {
    self.0.confidence()
  }
}

/// GraphQL wrapper for [`BodyPose3DJoint`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlBodyPose3DJoint(pub BodyPose3DJoint);

impl From<BodyPose3DJoint> for GqlBodyPose3DJoint {
  #[inline]
  fn from(v: BodyPose3DJoint) -> Self {
    Self(v)
  }
}
impl From<GqlBodyPose3DJoint> for BodyPose3DJoint {
  #[inline]
  fn from(v: GqlBodyPose3DJoint) -> Self {
    v.0
  }
}

#[Object(name = "BodyPose3DJoint")]
impl GqlBodyPose3DJoint {
  async fn name(&self) -> String {
    self.0.name().to_string()
  }
  async fn x(&self) -> f32 {
    self.0.x()
  }
  async fn y(&self) -> f32 {
    self.0.y()
  }
  async fn z(&self) -> f32 {
    self.0.z()
  }
  async fn confidence(&self) -> f32 {
    self.0.confidence()
  }
}

/// GraphQL wrapper for [`BodyPoseDetection`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlBodyPoseDetection(pub BodyPoseDetection);

impl From<BodyPoseDetection> for GqlBodyPoseDetection {
  #[inline]
  fn from(v: BodyPoseDetection) -> Self {
    Self(v)
  }
}
impl From<GqlBodyPoseDetection> for BodyPoseDetection {
  #[inline]
  fn from(v: GqlBodyPoseDetection) -> Self {
    v.0
  }
}

#[Object(name = "BodyPoseDetection")]
impl GqlBodyPoseDetection {
  async fn bbox(&self) -> GqlBoundingBox {
    GqlBoundingBox(*self.0.bbox())
  }
  async fn confidence(&self) -> f32 {
    self.0.confidence()
  }
  async fn joints(&self) -> std::vec::Vec<GqlBodyPoseJoint> {
    self
      .0
      .joints()
      .iter()
      .cloned()
      .map(GqlBodyPoseJoint)
      .collect()
  }
}

/// GraphQL wrapper for [`HandPoseDetection`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlHandPoseDetection(pub HandPoseDetection);

impl From<HandPoseDetection> for GqlHandPoseDetection {
  #[inline]
  fn from(v: HandPoseDetection) -> Self {
    Self(v)
  }
}
impl From<GqlHandPoseDetection> for HandPoseDetection {
  #[inline]
  fn from(v: GqlHandPoseDetection) -> Self {
    v.0
  }
}

#[Object(name = "HandPoseDetection")]
impl GqlHandPoseDetection {
  async fn bbox(&self) -> GqlBoundingBox {
    GqlBoundingBox(*self.0.bbox())
  }
  async fn confidence(&self) -> f32 {
    self.0.confidence()
  }
  async fn chirality(&self) -> String {
    match self.0.chirality() {
      HandChirality::Left => "LEFT",
      HandChirality::Right => "RIGHT",
      HandChirality::Unknown => "UNKNOWN",
      #[allow(unreachable_patterns)]
      _ => "UNKNOWN",
    }
    .into()
  }
  async fn joints(&self) -> std::vec::Vec<GqlBodyPoseJoint> {
    self
      .0
      .joints()
      .iter()
      .cloned()
      .map(GqlBodyPoseJoint)
      .collect()
  }
}

/// GraphQL wrapper for [`BodyPose3DDetection`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlBodyPose3DDetection(pub BodyPose3DDetection);

impl From<BodyPose3DDetection> for GqlBodyPose3DDetection {
  #[inline]
  fn from(v: BodyPose3DDetection) -> Self {
    Self(v)
  }
}
impl From<GqlBodyPose3DDetection> for BodyPose3DDetection {
  #[inline]
  fn from(v: GqlBodyPose3DDetection) -> Self {
    v.0
  }
}

#[Object(name = "BodyPose3DDetection")]
impl GqlBodyPose3DDetection {
  async fn confidence(&self) -> f32 {
    self.0.confidence()
  }
  async fn body_height(&self) -> f32 {
    self.0.body_height()
  }
  async fn height_estimation(&self) -> String {
    match self.0.height_estimation() {
      BodyPose3DHeightEstimation::Measured => "MEASURED",
      BodyPose3DHeightEstimation::Reference => "REFERENCE",
      #[allow(unreachable_patterns)]
      _ => "UNKNOWN",
    }
    .into()
  }
  async fn joints(&self) -> std::vec::Vec<GqlBodyPose3DJoint> {
    self
      .0
      .joints()
      .iter()
      .cloned()
      .map(GqlBodyPose3DJoint)
      .collect()
  }
}

/// GraphQL wrapper for [`SubjectDetection`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlSubjectDetection(pub SubjectDetection);

impl From<SubjectDetection> for GqlSubjectDetection {
  #[inline]
  fn from(v: SubjectDetection) -> Self {
    Self(v)
  }
}
impl From<GqlSubjectDetection> for SubjectDetection {
  #[inline]
  fn from(v: GqlSubjectDetection) -> Self {
    v.0
  }
}

#[Object(name = "SubjectDetection")]
impl GqlSubjectDetection {
  async fn detection(&self) -> GqlDetection {
    GqlDetection(self.0.detection().clone())
  }
  async fn bbox(&self) -> GqlBoundingBox {
    GqlBoundingBox(*self.0.bbox())
  }
}

/// GraphQL wrapper for [`FaceDetection`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GqlFaceDetection(pub FaceDetection);

impl From<FaceDetection> for GqlFaceDetection {
  #[inline]
  fn from(v: FaceDetection) -> Self {
    Self(v)
  }
}
impl From<GqlFaceDetection> for FaceDetection {
  #[inline]
  fn from(v: GqlFaceDetection) -> Self {
    v.0
  }
}

#[Object(name = "FaceDetection")]
impl GqlFaceDetection {
  async fn bbox(&self) -> GqlBoundingBox {
    GqlBoundingBox(*self.0.bbox())
  }
  async fn confidence(&self) -> f32 {
    self.0.confidence()
  }
  async fn capture_quality(&self) -> f32 {
    self.0.capture_quality()
  }
  async fn roll(&self) -> f32 {
    self.0.roll()
  }
  async fn yaw(&self) -> f32 {
    self.0.yaw()
  }
  async fn pitch(&self) -> f32 {
    self.0.pitch()
  }
}

/// GraphQL wrapper for [`FaceLandmarkRegion`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlFaceLandmarkRegion(pub FaceLandmarkRegion);

impl From<FaceLandmarkRegion> for GqlFaceLandmarkRegion {
  #[inline]
  fn from(v: FaceLandmarkRegion) -> Self {
    Self(v)
  }
}
impl From<GqlFaceLandmarkRegion> for FaceLandmarkRegion {
  #[inline]
  fn from(v: GqlFaceLandmarkRegion) -> Self {
    v.0
  }
}

#[Object(name = "FaceLandmarkRegion")]
impl GqlFaceLandmarkRegion {
  async fn name(&self) -> String {
    self.0.name().to_string()
  }
  async fn points(&self) -> std::vec::Vec<GqlPoint2D> {
    self
      .0
      .points()
      .iter()
      .map(|(x, y)| GqlPoint2D { x: *x, y: *y })
      .collect()
  }
}

/// GraphQL wrapper for [`FaceLandmarksDetection`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlFaceLandmarksDetection(pub FaceLandmarksDetection);

impl From<FaceLandmarksDetection> for GqlFaceLandmarksDetection {
  #[inline]
  fn from(v: FaceLandmarksDetection) -> Self {
    Self(v)
  }
}
impl From<GqlFaceLandmarksDetection> for FaceLandmarksDetection {
  #[inline]
  fn from(v: GqlFaceLandmarksDetection) -> Self {
    v.0
  }
}

#[Object(name = "FaceLandmarksDetection")]
impl GqlFaceLandmarksDetection {
  async fn bbox(&self) -> GqlBoundingBox {
    GqlBoundingBox(*self.0.bbox())
  }
  async fn confidence(&self) -> f32 {
    self.0.confidence()
  }
  async fn regions(&self) -> std::vec::Vec<GqlFaceLandmarkRegion> {
    self
      .0
      .regions()
      .iter()
      .cloned()
      .map(GqlFaceLandmarkRegion)
      .collect()
  }
}

/// GraphQL wrapper for [`PersonInstanceMaskDetection`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlPersonInstanceMaskDetection(pub PersonInstanceMaskDetection);

impl From<PersonInstanceMaskDetection> for GqlPersonInstanceMaskDetection {
  #[inline]
  fn from(v: PersonInstanceMaskDetection) -> Self {
    Self(v)
  }
}
impl From<GqlPersonInstanceMaskDetection> for PersonInstanceMaskDetection {
  #[inline]
  fn from(v: GqlPersonInstanceMaskDetection) -> Self {
    v.0
  }
}

#[Object(name = "PersonInstanceMaskDetection")]
impl GqlPersonInstanceMaskDetection {
  async fn bbox(&self) -> GqlBoundingBox {
    GqlBoundingBox(*self.0.bbox())
  }
  async fn confidence(&self) -> f32 {
    self.0.confidence()
  }
  async fn instance_index(&self) -> u32 {
    self.0.instance_index()
  }
  async fn dimensions(&self) -> GqlDimensions {
    let (width, height) = self.0.dimensions();
    GqlDimensions { width, height }
  }
  async fn byte_len(&self) -> usize {
    self.0.data().len()
  }
}

/// GraphQL wrapper for [`PersonSegmentationMask`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlPersonSegmentationMask(pub PersonSegmentationMask);

impl From<PersonSegmentationMask> for GqlPersonSegmentationMask {
  #[inline]
  fn from(v: PersonSegmentationMask) -> Self {
    Self(v)
  }
}
impl From<GqlPersonSegmentationMask> for PersonSegmentationMask {
  #[inline]
  fn from(v: GqlPersonSegmentationMask) -> Self {
    v.0
  }
}

#[Object(name = "PersonSegmentationMask")]
impl GqlPersonSegmentationMask {
  async fn bbox(&self) -> GqlBoundingBox {
    GqlBoundingBox(*self.0.bbox())
  }
  async fn confidence(&self) -> f32 {
    self.0.confidence()
  }
  async fn dimensions(&self) -> GqlDimensions {
    let (width, height) = self.0.dimensions();
    GqlDimensions { width, height }
  }
  async fn byte_len(&self) -> usize {
    self.0.data().len()
  }
}

/// GraphQL wrapper for [`HumanAnalysis`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlHumanAnalysis(pub HumanAnalysis);

impl From<HumanAnalysis> for GqlHumanAnalysis {
  #[inline]
  fn from(v: HumanAnalysis) -> Self {
    Self(v)
  }
}
impl From<GqlHumanAnalysis> for HumanAnalysis {
  #[inline]
  fn from(v: GqlHumanAnalysis) -> Self {
    v.0
  }
}

#[Object(name = "HumanAnalysis")]
impl GqlHumanAnalysis {
  async fn subjects(&self) -> std::vec::Vec<GqlSubjectDetection> {
    self
      .0
      .subjects()
      .iter()
      .cloned()
      .map(GqlSubjectDetection)
      .collect()
  }
  async fn faces(&self) -> std::vec::Vec<GqlFaceDetection> {
    self
      .0
      .faces()
      .iter()
      .copied()
      .map(GqlFaceDetection)
      .collect()
  }
  async fn body_poses(&self) -> std::vec::Vec<GqlBodyPoseDetection> {
    self
      .0
      .body_poses()
      .iter()
      .cloned()
      .map(GqlBodyPoseDetection)
      .collect()
  }
  async fn hand_poses(&self) -> std::vec::Vec<GqlHandPoseDetection> {
    self
      .0
      .hand_poses()
      .iter()
      .cloned()
      .map(GqlHandPoseDetection)
      .collect()
  }
  async fn body_poses_3d(&self) -> std::vec::Vec<GqlBodyPose3DDetection> {
    self
      .0
      .body_poses_3d()
      .iter()
      .cloned()
      .map(GqlBodyPose3DDetection)
      .collect()
  }
  async fn instance_masks(&self) -> std::vec::Vec<GqlPersonInstanceMaskDetection> {
    self
      .0
      .instance_masks()
      .iter()
      .cloned()
      .map(GqlPersonInstanceMaskDetection)
      .collect()
  }
  async fn face_rectangles(&self) -> std::vec::Vec<GqlFaceDetection> {
    self
      .0
      .face_rectangles()
      .iter()
      .copied()
      .map(GqlFaceDetection)
      .collect()
  }
  async fn face_landmarks(&self) -> std::vec::Vec<GqlFaceLandmarksDetection> {
    self
      .0
      .face_landmarks()
      .iter()
      .cloned()
      .map(GqlFaceLandmarksDetection)
      .collect()
  }
  async fn segmentation_masks(&self) -> std::vec::Vec<GqlPersonSegmentationMask> {
    self
      .0
      .segmentation_masks()
      .iter()
      .cloned()
      .map(GqlPersonSegmentationMask)
      .collect()
  }
}

/// GraphQL wrapper for [`AnimalAnalysis`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlAnimalAnalysis(pub AnimalAnalysis);

impl From<AnimalAnalysis> for GqlAnimalAnalysis {
  #[inline]
  fn from(v: AnimalAnalysis) -> Self {
    Self(v)
  }
}
impl From<GqlAnimalAnalysis> for AnimalAnalysis {
  #[inline]
  fn from(v: GqlAnimalAnalysis) -> Self {
    v.0
  }
}

#[Object(name = "AnimalAnalysis")]
impl GqlAnimalAnalysis {
  async fn subjects(&self) -> std::vec::Vec<GqlSubjectDetection> {
    self
      .0
      .subjects()
      .iter()
      .cloned()
      .map(GqlSubjectDetection)
      .collect()
  }
  async fn body_poses(&self) -> std::vec::Vec<GqlBodyPoseDetection> {
    self
      .0
      .body_poses()
      .iter()
      .cloned()
      .map(GqlBodyPoseDetection)
      .collect()
  }
}

/// GraphQL wrapper for [`Aesthetics`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GqlAesthetics(pub Aesthetics);

impl From<Aesthetics> for GqlAesthetics {
  #[inline]
  fn from(v: Aesthetics) -> Self {
    Self(v)
  }
}
impl From<GqlAesthetics> for Aesthetics {
  #[inline]
  fn from(v: GqlAesthetics) -> Self {
    v.0
  }
}

#[Object(name = "Aesthetics")]
impl GqlAesthetics {
  async fn overall_score(&self) -> f32 {
    self.0.overall_score()
  }
  async fn is_utility(&self) -> bool {
    self.0.is_utility()
  }
}

/// GraphQL wrapper for [`DominantColor`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlDominantColor(pub DominantColor);

impl From<DominantColor> for GqlDominantColor {
  #[inline]
  fn from(v: DominantColor) -> Self {
    Self(v)
  }
}
impl From<GqlDominantColor> for DominantColor {
  #[inline]
  fn from(v: GqlDominantColor) -> Self {
    v.0
  }
}

#[Object(name = "DominantColor")]
impl GqlDominantColor {
  async fn rgb(&self) -> GqlRgba {
    GqlRgba(self.0.rgb())
  }
  async fn name(&self) -> String {
    self.0.name().to_string()
  }
  async fn percentage(&self) -> f32 {
    self.0.percentage()
  }
  async fn population(&self) -> u32 {
    self.0.population()
  }
}

/// GraphQL wrapper for [`VlmAnalysis`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlVlmAnalysis(pub VlmAnalysis);

impl From<VlmAnalysis> for GqlVlmAnalysis {
  #[inline]
  fn from(v: VlmAnalysis) -> Self {
    Self(v)
  }
}
impl From<GqlVlmAnalysis> for VlmAnalysis {
  #[inline]
  fn from(v: GqlVlmAnalysis) -> Self {
    v.0
  }
}

#[Object(name = "VlmAnalysis")]
impl GqlVlmAnalysis {
  async fn categories(&self) -> std::vec::Vec<GqlLocalizedText> {
    self
      .0
      .categories()
      .iter()
      .cloned()
      .map(GqlLocalizedText)
      .collect()
  }
  async fn description(&self) -> GqlLocalizedText {
    GqlLocalizedText(self.0.description().clone())
  }
  async fn tags(&self) -> std::vec::Vec<GqlLocalizedText> {
    self
      .0
      .tags()
      .iter()
      .cloned()
      .map(GqlLocalizedText)
      .collect()
  }
  async fn shot_type(&self) -> Option<String> {
    empty_as_none(self.0.shot_type())
  }
  async fn objects(&self) -> std::vec::Vec<GqlLocalizedText> {
    self
      .0
      .objects()
      .iter()
      .cloned()
      .map(GqlLocalizedText)
      .collect()
  }
  async fn subjects(&self) -> std::vec::Vec<GqlLocalizedText> {
    self
      .0
      .subjects()
      .iter()
      .cloned()
      .map(GqlLocalizedText)
      .collect()
  }
  async fn mood(&self) -> std::vec::Vec<GqlLocalizedText> {
    self
      .0
      .mood()
      .iter()
      .cloned()
      .map(GqlLocalizedText)
      .collect()
  }
  async fn emotion(&self) -> std::vec::Vec<GqlLocalizedText> {
    self
      .0
      .emotion()
      .iter()
      .cloned()
      .map(GqlLocalizedText)
      .collect()
  }
  async fn lighting(&self) -> std::vec::Vec<GqlLocalizedText> {
    self
      .0
      .lighting()
      .iter()
      .cloned()
      .map(GqlLocalizedText)
      .collect()
  }
}

// ===========================================================================
// Keyframe
// ===========================================================================

/// GraphQL wrapper for [`Keyframe`].
#[derive(Debug, Clone)]
pub struct GqlKeyframe(pub Keyframe<Uuid7>);

impl From<Keyframe<Uuid7>> for GqlKeyframe {
  #[inline]
  fn from(v: Keyframe<Uuid7>) -> Self {
    Self(v)
  }
}
impl From<GqlKeyframe> for Keyframe<Uuid7> {
  #[inline]
  fn from(v: GqlKeyframe) -> Self {
    v.0
  }
}

#[Object(name = "Keyframe")]
impl GqlKeyframe {
  async fn id(&self) -> ID {
    ID(self.0.id().to_string())
  }
  async fn parent(&self) -> ID {
    ID(self.0.parent().to_string())
  }
  async fn pts(&self) -> GqlMediaTimestamp {
    GqlMediaTimestamp(*self.0.pts())
  }
  async fn mime(&self) -> Option<String> {
    empty_as_none(self.0.mime())
  }
  async fn size(&self) -> u32 {
    self.0.size()
  }
  async fn byte_len(&self) -> usize {
    self.0.data().len()
  }
  async fn dimensions(&self) -> GqlDimensions {
    let (width, height) = self.0.dimensions();
    GqlDimensions { width, height }
  }
  async fn extractor(&self) -> GqlKeyframeExtractor {
    self.0.extractor().into()
  }
  async fn classifications(&self) -> std::vec::Vec<GqlDetection> {
    self
      .0
      .classifications()
      .iter()
      .cloned()
      .map(GqlDetection)
      .collect()
  }
  async fn objects(&self) -> std::vec::Vec<GqlObjectDetection> {
    self
      .0
      .objects()
      .iter()
      .cloned()
      .map(GqlObjectDetection)
      .collect()
  }
  async fn humans(&self) -> GqlHumanAnalysis {
    GqlHumanAnalysis(self.0.humans().clone())
  }
  async fn animals(&self) -> GqlAnimalAnalysis {
    GqlAnimalAnalysis(self.0.animals().clone())
  }
  async fn actions(&self) -> std::vec::Vec<GqlActionDetection> {
    self
      .0
      .actions()
      .iter()
      .cloned()
      .map(GqlActionDetection)
      .collect()
  }
  async fn text_detections(&self) -> std::vec::Vec<GqlTextDetection> {
    self
      .0
      .text_detections()
      .iter()
      .cloned()
      .map(GqlTextDetection)
      .collect()
  }
  async fn barcodes(&self) -> std::vec::Vec<GqlBarcodeDetection> {
    self
      .0
      .barcodes()
      .iter()
      .cloned()
      .map(GqlBarcodeDetection)
      .collect()
  }
  async fn attention_saliency(&self) -> std::vec::Vec<GqlSaliencyRegion> {
    self
      .0
      .attention_saliency()
      .iter()
      .copied()
      .map(GqlSaliencyRegion)
      .collect()
  }
  async fn objectness_saliency(&self) -> std::vec::Vec<GqlSaliencyRegion> {
    self
      .0
      .objectness_saliency()
      .iter()
      .copied()
      .map(GqlSaliencyRegion)
      .collect()
  }
  async fn horizon(&self) -> GqlHorizonInfo {
    GqlHorizonInfo(*self.0.horizon())
  }
  async fn document_segments(&self) -> std::vec::Vec<GqlDocumentSegment> {
    self
      .0
      .document_segments()
      .iter()
      .copied()
      .map(GqlDocumentSegment)
      .collect()
  }
  async fn aesthetics(&self) -> GqlAesthetics {
    GqlAesthetics(*self.0.aesthetics())
  }
  async fn colors(&self) -> std::vec::Vec<GqlDominantColor> {
    self
      .0
      .colors()
      .iter()
      .cloned()
      .map(GqlDominantColor)
      .collect()
  }
  async fn vlm(&self) -> GqlVlmAnalysis {
    GqlVlmAnalysis(self.0.vlm().clone())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use mediatime::{Timebase, Timestamp};
  use smol_str::SmolStr;

  #[test]
  fn video_codec_wrapper_tags_roundtrip() {
    let h264: GqlVideoCodec = VideoCodec::H264.into();
    let other: GqlVideoCodec = VideoCodec::Other(SmolStr::new("hap")).into();
    let back: VideoCodec = h264.clone().into();
    assert_eq!(back, VideoCodec::H264);
    let back2: VideoCodec = other.clone().into();
    assert_eq!(back2, VideoCodec::Other(SmolStr::new("hap")));
  }

  #[test]
  fn keyframe_wrapper_roundtrips() {
    let id = Uuid7::new();
    let parent = Uuid7::new();
    let tb = Timebase::new(1, core::num::NonZeroU32::new(1000).unwrap());
    let pts = Timestamp::new(0, tb);
    let k = Keyframe::try_new(
      id,
      parent,
      pts,
      64,
      64,
      crate::domain::KeyframeExtractor::Manual,
    )
    .unwrap();
    let g: GqlKeyframe = k.clone().into();
    let back: Keyframe<Uuid7> = g.into();
    assert_eq!(back.id(), k.id());
  }
}
