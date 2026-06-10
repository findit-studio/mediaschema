//! Video subtree: facet → tracks → scenes → keyframes. Standalone field
//! owners — no embedded flat aggregates, no parent FKs, no id-vecs.

use bytes::Bytes;
use indexmap::IndexMap;
use mediaframe::{
  codec::VideoCodec,
  color::{DolbyVisionConfig, HdrStaticMetadata, Info as ColorInfo},
  disposition::TrackDisposition,
  frame::{Dimensions, FieldOrder, FrameRate, Rect, Rotation, SampleAspectRatio, StereoMode},
  pixel_format::PixelFormat,
};
use mediatime::{TimeRange, Timestamp};
use smol_str::SmolStr;

use super::{parent_check, GraphError, NodeKind};
use crate::domain::{
  self,
  aggregates::video::{
    facet::VideoParts, keyframe::KeyframeParts, scene::SceneParts, track::VideoTrackParts,
    ActionDetection, Aesthetics, AnimalAnalysis, BarcodeDetection, Detection, DocumentSegment,
    DominantColor, HorizonInfo, HumanAnalysis, ObjectDetection, SaliencyRegion, TextDetection,
    VlmAnalysis,
  },
  ErrorInfo, IndexProgress, KeyframeExtractor, Provenance, SceneDetector, Uuid7, VideoIndexStatus,
};

/// The video facet with its complete track subtrees.
#[derive(Debug, Clone, PartialEq)]
pub struct Video<Id = Uuid7> {
  id: Id,
  total_scenes: u32,
  track_progress: IndexProgress,
  tracks: Vec<VideoTrack<Id>>,
}

impl Video<Uuid7> {
  /// Lift the flat facet; validates `media_id == expected_media`. Tracks
  /// arrive pre-lifted (their `video_id` was consumed by their lift).
  pub fn try_from_flat(
    expected_media: &Uuid7,
    facet: domain::Video<Uuid7>,
    tracks: Vec<VideoTrack<Uuid7>>,
  ) -> Result<Self, GraphError> {
    let VideoParts {
      id,
      media_id,
      total_scenes,
      tracks: _,
      track_progress,
    } = facet.into_parts();
    parent_check(NodeKind::VideoFacet, id, &media_id, expected_media)?;
    Ok(Self {
      id,
      total_scenes,
      track_progress,
      tracks,
    })
  }
}

impl<Id> Video<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  #[inline(always)]
  pub const fn total_scenes(&self) -> u32 {
    self.total_scenes
  }

  #[inline(always)]
  pub const fn track_progress_ref(&self) -> &IndexProgress {
    &self.track_progress
  }

  /// The track subtrees, in container stream order.
  #[inline(always)]
  pub const fn tracks_slice(&self) -> &[VideoTrack<Id>] {
    self.tracks.as_slice()
  }
}

/// One video track — every field of the flat `VideoTrack` except
/// `video_id` and the scene-id vec, plus the scenes themselves.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoTrack<Id = Uuid7> {
  id: Id,
  stream_index: Option<u32>,
  container_track_id: Option<u64>,
  start_pts: Option<Timestamp>,
  duration: Option<Timestamp>,
  codec: VideoCodec,
  profile: Option<SmolStr>,
  level: Option<u16>,
  bit_rate: u64,
  nb_frames: Option<u64>,
  has_b_frames: bool,
  closed_gop: Option<bool>,
  bits_per_raw_sample: Option<u8>,
  dimensions: Dimensions,
  visible_rect: Option<Rect>,
  sample_aspect_ratio: SampleAspectRatio,
  pixel_format: PixelFormat,
  color: ColorInfo,
  hdr_static: Option<HdrStaticMetadata>,
  rotation: Rotation,
  frame_rate: FrameRate,
  avg_frame_rate: FrameRate,
  field_order: FieldOrder,
  stereo_mode: Option<StereoMode>,
  dovi: Option<DolbyVisionConfig>,
  has_embedded_captions: bool,
  disposition: TrackDisposition,
  is_primary: bool,
  auto_selected: bool,
  scenes: Vec<Scene<Id>>,
  metadata: IndexMap<SmolStr, SmolStr>,
  index_status: VideoIndexStatus,
  index_errors: Vec<ErrorInfo>,
  provenance: Provenance,
}

impl VideoTrack<Uuid7> {
  /// Lift the flat track; validates `video_id == expected_video`. Scenes
  /// arrive pre-lifted (their `video_track_id` was consumed by their
  /// lift).
  pub fn try_from_flat(
    expected_video: &Uuid7,
    track: domain::VideoTrack<Uuid7>,
    scenes: Vec<Scene<Uuid7>>,
  ) -> Result<Self, GraphError> {
    let VideoTrackParts {
      id,
      video_id,
      stream_index,
      container_track_id,
      start_pts,
      duration,
      codec,
      profile,
      level,
      bit_rate,
      nb_frames,
      has_b_frames,
      closed_gop,
      bits_per_raw_sample,
      dimensions,
      visible_rect,
      sample_aspect_ratio,
      pixel_format,
      color,
      hdr_static,
      rotation,
      frame_rate,
      avg_frame_rate,
      field_order,
      stereo_mode,
      dovi,
      has_embedded_captions,
      disposition,
      is_primary,
      auto_selected,
      scenes: _,
      metadata,
      index_status,
      index_errors,
      provenance,
    } = track.into_parts();
    parent_check(NodeKind::VideoTrack, id, &video_id, expected_video)?;
    Ok(Self {
      id,
      stream_index,
      container_track_id,
      start_pts,
      duration,
      codec,
      profile,
      level,
      bit_rate,
      nb_frames,
      has_b_frames,
      closed_gop,
      bits_per_raw_sample,
      dimensions,
      visible_rect,
      sample_aspect_ratio,
      pixel_format,
      color,
      hdr_static,
      rotation,
      frame_rate,
      avg_frame_rate,
      field_order,
      stereo_mode,
      dovi,
      has_embedded_captions,
      disposition,
      is_primary,
      auto_selected,
      scenes,
      metadata,
      index_status,
      index_errors,
      provenance,
    })
  }
}

impl<Id> VideoTrack<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  #[inline(always)]
  pub const fn stream_index(&self) -> Option<u32> {
    self.stream_index
  }

  #[inline(always)]
  pub const fn container_track_id(&self) -> Option<u64> {
    self.container_track_id
  }

  #[inline(always)]
  pub const fn start_pts_ref(&self) -> Option<&Timestamp> {
    self.start_pts.as_ref()
  }

  #[inline(always)]
  pub const fn duration_ref(&self) -> Option<&Timestamp> {
    self.duration.as_ref()
  }

  #[inline(always)]
  pub const fn codec_ref(&self) -> &VideoCodec {
    &self.codec
  }

  /// Codec profile name (`None` = unknown).
  #[inline(always)]
  pub fn profile(&self) -> Option<&str> {
    self.profile.as_deref()
  }

  #[inline(always)]
  pub const fn level(&self) -> Option<u16> {
    self.level
  }

  #[inline(always)]
  pub const fn bit_rate(&self) -> u64 {
    self.bit_rate
  }

  #[inline(always)]
  pub const fn nb_frames(&self) -> Option<u64> {
    self.nb_frames
  }

  #[inline(always)]
  pub const fn has_b_frames(&self) -> bool {
    self.has_b_frames
  }

  #[inline(always)]
  pub const fn closed_gop(&self) -> Option<bool> {
    self.closed_gop
  }

  #[inline(always)]
  pub const fn bits_per_raw_sample(&self) -> Option<u8> {
    self.bits_per_raw_sample
  }

  #[inline(always)]
  pub const fn dimensions(&self) -> Dimensions {
    self.dimensions
  }

  #[inline(always)]
  pub const fn visible_rect(&self) -> Option<Rect> {
    self.visible_rect
  }

  #[inline(always)]
  pub const fn sample_aspect_ratio(&self) -> SampleAspectRatio {
    self.sample_aspect_ratio
  }

  #[inline(always)]
  pub const fn pixel_format(&self) -> PixelFormat {
    self.pixel_format
  }

  #[inline(always)]
  pub const fn color_ref(&self) -> &ColorInfo {
    &self.color
  }

  #[inline(always)]
  pub const fn hdr_static_ref(&self) -> Option<&HdrStaticMetadata> {
    self.hdr_static.as_ref()
  }

  #[inline(always)]
  pub const fn rotation(&self) -> Rotation {
    self.rotation
  }

  #[inline(always)]
  pub const fn frame_rate(&self) -> FrameRate {
    self.frame_rate
  }

  #[inline(always)]
  pub const fn avg_frame_rate(&self) -> FrameRate {
    self.avg_frame_rate
  }

  #[inline(always)]
  pub const fn field_order(&self) -> FieldOrder {
    self.field_order
  }

  #[inline(always)]
  pub const fn stereo_mode(&self) -> Option<StereoMode> {
    self.stereo_mode
  }

  #[inline(always)]
  pub const fn dovi(&self) -> Option<DolbyVisionConfig> {
    self.dovi
  }

  #[inline(always)]
  pub const fn has_embedded_captions(&self) -> bool {
    self.has_embedded_captions
  }

  #[inline(always)]
  pub const fn disposition(&self) -> TrackDisposition {
    self.disposition
  }

  #[inline(always)]
  pub const fn is_primary(&self) -> bool {
    self.is_primary
  }

  #[inline(always)]
  pub const fn auto_selected(&self) -> bool {
    self.auto_selected
  }

  /// The scene subtrees, in track order.
  #[inline(always)]
  pub const fn scenes_slice(&self) -> &[Scene<Id>] {
    self.scenes.as_slice()
  }

  #[inline(always)]
  pub const fn metadata_ref(&self) -> &IndexMap<SmolStr, SmolStr> {
    &self.metadata
  }

  #[inline(always)]
  pub const fn index_status(&self) -> VideoIndexStatus {
    self.index_status
  }

  #[inline(always)]
  pub const fn index_errors_slice(&self) -> &[ErrorInfo] {
    self.index_errors.as_slice()
  }

  #[inline(always)]
  pub const fn provenance_ref(&self) -> &Provenance {
    &self.provenance
  }
}

/// One scene — every field of the flat `Scene` except `video_track_id`
/// and the keyframe-id vec, plus the keyframes themselves.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene<Id = Uuid7> {
  id: Id,
  index: u32,
  span: TimeRange,
  detector: SceneDetector,
  keyframes: Vec<Keyframe<Id>>,
  description: SmolStr,
}

impl Scene<Uuid7> {
  /// Lift the flat scene; validates `video_track_id == expected_track`
  /// and lifts the flat keyframes against this scene's id.
  pub fn try_from_flat(
    expected_track: &Uuid7,
    scene: domain::Scene<Uuid7>,
    keyframes: Vec<domain::Keyframe<Uuid7>>,
  ) -> Result<Self, GraphError> {
    let SceneParts {
      id,
      video_track_id,
      index,
      span,
      detector,
      keyframes: _,
      description,
    } = scene.into_parts();
    parent_check(NodeKind::Scene, id, &video_track_id, expected_track)?;
    let keyframes = keyframes
      .into_iter()
      .map(|k| Keyframe::try_from_flat(&id, k))
      .collect::<Result<Vec<_>, _>>()?;
    Ok(Self {
      id,
      index,
      span,
      detector,
      keyframes,
      description,
    })
  }
}

impl<Id> Scene<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  #[inline(always)]
  pub const fn index(&self) -> u32 {
    self.index
  }

  #[inline(always)]
  pub const fn span_ref(&self) -> &TimeRange {
    &self.span
  }

  #[inline(always)]
  pub const fn detector(&self) -> SceneDetector {
    self.detector
  }

  /// The scene's keyframes.
  #[inline(always)]
  pub const fn keyframes_slice(&self) -> &[Keyframe<Id>] {
    self.keyframes.as_slice()
  }

  /// Scene description (`""` = absent).
  #[inline(always)]
  pub fn description(&self) -> &str {
    self.description.as_str()
  }
}

/// One keyframe — every field of the flat `Keyframe` except `scene_id`
/// (implied by nesting).
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframe<Id = Uuid7> {
  id: Id,
  pts: Timestamp,
  data: Bytes,
  mime: SmolStr,
  dimensions: Dimensions,
  extractor: KeyframeExtractor,
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
  colors: Vec<DominantColor>,
  vlm: VlmAnalysis,
}

impl Keyframe<Uuid7> {
  /// Lift the flat keyframe; validates `scene_id == expected_scene`.
  pub fn try_from_flat(
    expected_scene: &Uuid7,
    keyframe: domain::Keyframe<Uuid7>,
  ) -> Result<Self, GraphError> {
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
    } = keyframe.into_parts();
    parent_check(NodeKind::Keyframe, id, &scene_id, expected_scene)?;
    Ok(Self {
      id,
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
    })
  }
}

impl<Id> Keyframe<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  #[inline(always)]
  pub const fn pts_ref(&self) -> &Timestamp {
    &self.pts
  }

  /// Thumbnail image bytes (inline).
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

  /// Byte size of `data` — derived from `data.len()`.
  #[inline(always)]
  pub fn size(&self) -> u64 {
    self.data.len() as u64
  }

  #[inline(always)]
  pub const fn dimensions(&self) -> Dimensions {
    self.dimensions
  }

  #[inline(always)]
  pub const fn extractor(&self) -> KeyframeExtractor {
    self.extractor
  }

  #[inline(always)]
  pub const fn classifications_slice(&self) -> &[Detection] {
    self.classifications.as_slice()
  }

  #[inline(always)]
  pub const fn objects_slice(&self) -> &[ObjectDetection] {
    self.objects.as_slice()
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
  pub const fn actions_slice(&self) -> &[ActionDetection] {
    self.actions.as_slice()
  }

  #[inline(always)]
  pub const fn text_detections_slice(&self) -> &[TextDetection] {
    self.text_detections.as_slice()
  }

  #[inline(always)]
  pub const fn barcodes_slice(&self) -> &[BarcodeDetection] {
    self.barcodes.as_slice()
  }

  #[inline(always)]
  pub const fn attention_saliency_slice(&self) -> &[SaliencyRegion] {
    self.attention_saliency.as_slice()
  }

  #[inline(always)]
  pub const fn objectness_saliency_slice(&self) -> &[SaliencyRegion] {
    self.objectness_saliency.as_slice()
  }

  #[inline(always)]
  pub const fn horizon_ref(&self) -> &HorizonInfo {
    &self.horizon
  }

  #[inline(always)]
  pub const fn document_segments_slice(&self) -> &[DocumentSegment] {
    self.document_segments.as_slice()
  }

  #[inline(always)]
  pub const fn aesthetics_ref(&self) -> &Aesthetics {
    &self.aesthetics
  }

  #[inline(always)]
  pub const fn colors_slice(&self) -> &[DominantColor] {
    self.colors.as_slice()
  }

  #[inline(always)]
  pub const fn vlm_ref(&self) -> &VlmAnalysis {
    &self.vlm
  }
}

#[cfg(test)]
mod tests {
  use core::num::NonZeroU32;

  use mediatime::{TimeRange, Timebase};

  use super::*;

  fn span() -> TimeRange {
    TimeRange::new(0, 1000, Timebase::new(1, NonZeroU32::new(1000).unwrap()))
  }

  #[test]
  fn coherent_track_subtree_lifts() {
    let video_id = Uuid7::new();
    let track = domain::VideoTrack::try_new(Uuid7::new(), video_id).expect("valid track");
    let track_id = *track.id_ref();
    let scene = domain::Scene::try_new(Uuid7::new(), track_id, 0, span(), SceneDetector::Manual)
      .expect("valid scene");
    let lifted_scene = Scene::try_from_flat(&track_id, scene, vec![]).expect("coherent");
    let node = VideoTrack::try_from_flat(&video_id, track, vec![lifted_scene]).expect("coherent");
    assert_eq!(node.scenes_slice().len(), 1);
    assert_eq!(node.id_ref(), &track_id);
  }

  #[test]
  fn scene_under_wrong_track_is_rejected() {
    let scene =
      domain::Scene::try_new(Uuid7::new(), Uuid7::new(), 0, span(), SceneDetector::Manual)
        .expect("valid scene");
    let err = Scene::try_from_flat(&Uuid7::new(), scene, vec![]).expect_err("incoherent");
    assert!(matches!(
      err,
      GraphError::ParentMismatch {
        kind: NodeKind::Scene,
        ..
      }
    ));
  }

  #[test]
  fn keyframe_under_wrong_scene_is_rejected() {
    let track_id = Uuid7::new();
    let scene = domain::Scene::try_new(Uuid7::new(), track_id, 0, span(), SceneDetector::Manual)
      .expect("valid scene");
    let keyframe = domain::Keyframe::try_new(
      Uuid7::new(),
      Uuid7::new(),
      span().start(),
      Dimensions::new(320, 240),
      KeyframeExtractor::IFrame,
    )
    .expect("valid keyframe");
    let err = Scene::try_from_flat(&track_id, scene, vec![keyframe]).expect_err("incoherent");
    assert!(matches!(
      err,
      GraphError::ParentMismatch {
        kind: NodeKind::Keyframe,
        ..
      }
    ));
  }

  #[test]
  fn lifted_keyframe_keeps_artifact_fields() {
    let scene_id = Uuid7::new();
    let keyframe = domain::Keyframe::try_new(
      Uuid7::new(),
      scene_id,
      span().start(),
      Dimensions::new(320, 240),
      KeyframeExtractor::IFrame,
    )
    .expect("valid keyframe");
    let g = Keyframe::try_from_flat(&scene_id, keyframe).expect("coherent");
    assert_eq!(g.dimensions(), Dimensions::new(320, 240));
    assert_eq!(g.extractor(), KeyframeExtractor::IFrame);
    assert!(g.data().is_empty());
  }
}
