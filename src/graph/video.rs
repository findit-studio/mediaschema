//! Video subtree: facet → tracks → scenes → keyframes. Standalone field
//! owners — no embedded flat aggregates, no parent FKs, no id-vecs.

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

use super::{facet_link_check, parent_check, GraphError, NodeKind};
use crate::domain::{
  self,
  aggregates::video::{
    facet::VideoParts, keyframe::KeyframeParts, scene::SceneParts, track::VideoTrackParts,
    ActionDetection, Aesthetics, AnimalAnalysis, BarcodeDetection, Detection, DocumentSegment,
    DominantColor, HorizonInfo, HumanAnalysis, ObjectDetection, SaliencyRegion, TextDetection,
    VlmAnalysis,
  },
  ErrorInfo, IndexProgress, KeyframeExtractor, KeyframeRole, Provenance, SceneDetector, Uuid7,
  VideoIndexStatus,
};

/// The video facet with its complete track subtrees + the video's
/// cover/poster keyframe.
#[derive(Debug, Clone, PartialEq)]
pub struct Video<Id = Uuid7> {
  id: Id,
  total_scenes: u32,
  track_progress: IndexProgress,
  tracks: Vec<VideoTrack<Id>>,
  /// The video poster / cover keyframe (`None` = no cover extracted yet).
  /// It attaches at the video level (no scene parent) and is a real,
  /// analyzable [`Keyframe`].
  cover: Option<Keyframe<Id>>,
}

impl Video<Uuid7> {
  /// Lift the flat facet; validates `media_id == expected_media`. Tracks
  /// arrive pre-lifted (their `video_id` was consumed by their lift). The
  /// optional `cover` keyframe is lifted at the **video** level via
  /// [`Keyframe::lift_cover`] (no scene parent); its coherence with the
  /// facet's stored `cover_keyframe_id` is checked here.
  pub fn try_from_flat(
    expected_media: &Uuid7,
    facet: domain::Video<Uuid7>,
    tracks: Vec<VideoTrack<Uuid7>>,
    cover: Option<domain::Keyframe<Uuid7>>,
  ) -> Result<Self, GraphError> {
    let VideoParts {
      id,
      media_id,
      total_scenes,
      tracks: _,
      track_progress,
      cover_keyframe_id,
    } = facet.into_parts();
    parent_check(NodeKind::VideoFacet, id, &media_id, expected_media)?;
    let cover = cover.map(Keyframe::lift_cover).transpose()?;
    // The facet's stored cover FK must agree with the embedded cover
    // keyframe (present with the same id, or both absent), mirroring how
    // `Media` checks its facet links.
    facet_link_check(
      NodeKind::CoverKeyframe,
      id,
      cover_keyframe_id.as_ref(),
      cover.as_ref().map(|k| k.id_ref()),
    )?;
    Ok(Self {
      id,
      total_scenes,
      track_progress,
      tracks,
      cover,
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

  /// The video's cover/poster keyframe (`None` = no cover extracted yet).
  #[inline(always)]
  pub const fn cover_ref(&self) -> Option<&Keyframe<Id>> {
    self.cover.as_ref()
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
/// (implied by nesting for a scene keyframe; absent for a cover keyframe).
///
/// The `role` discriminant is retained so a keyframe read back from the
/// graph knows whether it is a scene-representative frame or the video
/// poster (the cover, which attaches at the video level via
/// [`Video::cover_ref`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframe<Id = Uuid7> {
  id: Id,
  role: KeyframeRole,
  pts: Timestamp,
  /// FK → [`Thumbnail`](crate::domain::Thumbnail)`.id` — the image bytes
  /// + storage backend live on the referenced thumbnail, not inline.
  thumbnail_id: Id,
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
  /// Lift a flat **scene** keyframe; validates `scene_id == expected_scene`
  /// (the cross-tree containment edge) and that the `role` is
  /// [`KeyframeRole::Scene`] (a cover keyframe never rides the scene chain).
  pub fn try_from_flat(
    expected_scene: &Uuid7,
    keyframe: domain::Keyframe<Uuid7>,
  ) -> Result<Self, GraphError> {
    let (parts, scene_id) = Self::split(keyframe.into_parts());
    if !parts.role.is_scene() {
      return Err(GraphError::RoleMismatch {
        kind: NodeKind::Keyframe,
        node: parts.id,
      });
    }
    // A scene keyframe always carries a `Some` scene_id (its constructor
    // requires a non-nil one); a missing scene_id here is an incoherent
    // tree.
    let scene_id = scene_id.ok_or(GraphError::ParentMismatch {
      kind: NodeKind::Keyframe,
      child: parts.id,
      referenced: Uuid7::nil(),
      expected: *expected_scene,
    })?;
    parent_check(NodeKind::Keyframe, parts.id, &scene_id, expected_scene)?;
    Ok(parts)
  }

  /// Lift the video's **cover** keyframe. A cover keyframe attaches at the
  /// video level — it has **no scene parent**, so its `scene_id` (which is
  /// `None` on the flat side) is not parent-checked. Validates that the
  /// `role` is [`KeyframeRole::Cover`].
  pub fn lift_cover(keyframe: domain::Keyframe<Uuid7>) -> Result<Self, GraphError> {
    let (parts, _scene_id) = Self::split(keyframe.into_parts());
    if !parts.role.is_cover() {
      return Err(GraphError::RoleMismatch {
        kind: NodeKind::CoverKeyframe,
        node: parts.id,
      });
    }
    Ok(parts)
  }

  /// Exhaustively destructure [`KeyframeParts`] into the graph shape,
  /// returning the (graph keyframe, flat `scene_id`) pair so each lift can
  /// apply its own parent/role policy. The `scene_id` is consumed here and
  /// not stored on the graph keyframe.
  fn split(parts: KeyframeParts<Uuid7>) -> (Self, Option<Uuid7>) {
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
    let kf = Self {
      id,
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
    };
    (kf, scene_id)
  }
}

impl<Id> Keyframe<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// Scene-representative frame ([`KeyframeRole::Scene`]) vs the video
  /// poster ([`KeyframeRole::Cover`]).
  #[inline(always)]
  pub const fn role(&self) -> KeyframeRole {
    self.role
  }

  #[inline(always)]
  pub const fn pts_ref(&self) -> &Timestamp {
    &self.pts
  }

  /// FK → [`Thumbnail`](crate::domain::Thumbnail)`.id` — the image bytes
  /// + storage backend live on the referenced thumbnail.
  #[inline(always)]
  pub const fn thumbnail_id_ref(&self) -> &Id {
    &self.thumbnail_id
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
    let thumbnail_id = Uuid7::new();
    let keyframe = domain::Keyframe::try_new(
      Uuid7::new(),
      scene_id,
      thumbnail_id,
      span().start(),
      Dimensions::new(320, 240),
      KeyframeExtractor::IFrame,
    )
    .expect("valid keyframe");
    let g = Keyframe::try_from_flat(&scene_id, keyframe).expect("coherent");
    assert_eq!(g.dimensions(), Dimensions::new(320, 240));
    assert_eq!(g.extractor(), KeyframeExtractor::IFrame);
    assert_eq!(g.thumbnail_id_ref(), &thumbnail_id);
    assert!(g.role().is_scene());
  }

  fn cover_keyframe() -> domain::Keyframe<Uuid7> {
    domain::Keyframe::try_new_cover(
      Uuid7::new(),
      Uuid7::new(),
      span().start(),
      Dimensions::new(640, 360),
      KeyframeExtractor::CompositeQuality,
    )
    .expect("valid cover keyframe")
  }

  #[test]
  fn cover_lifts_at_video_level_and_sets_facet_fk() {
    let media_id = Uuid7::new();
    let facet = domain::Video::try_new(Uuid7::new(), media_id).expect("valid facet");
    let cover = cover_keyframe();
    let cover_id = *cover.id_ref();
    // The facet must record the cover FK for the coherence check to pass.
    let facet = facet.with_cover_keyframe_id(Some(cover_id));
    let g = Video::try_from_flat(&media_id, facet, vec![], Some(cover)).expect("coherent");
    let lifted = g.cover_ref().expect("cover present");
    assert!(lifted.role().is_cover());
    assert_eq!(lifted.id_ref(), &cover_id);
    // The facet→domain projection re-derives `cover_keyframe_id`.
    let back: domain::Video<Uuid7> = (media_id, g).into();
    assert_eq!(back.cover_keyframe_id_ref(), Some(&cover_id));
  }

  #[test]
  fn cover_facet_link_mismatch_is_rejected() {
    let media_id = Uuid7::new();
    // Embedded cover present, but the facet's stored cover FK is absent.
    let facet = domain::Video::try_new(Uuid7::new(), media_id).expect("valid facet");
    let err = Video::try_from_flat(&media_id, facet, vec![], Some(cover_keyframe()))
      .expect_err("dangling cover link");
    assert!(matches!(
      err,
      GraphError::FacetLinkMismatch {
        kind: NodeKind::CoverKeyframe,
        ..
      }
    ));
  }

  #[test]
  fn scene_keyframe_handed_to_cover_lift_is_rejected() {
    let scene_id = Uuid7::new();
    let scene_kf = domain::Keyframe::try_new(
      Uuid7::new(),
      scene_id,
      Uuid7::new(),
      span().start(),
      Dimensions::new(320, 240),
      KeyframeExtractor::IFrame,
    )
    .expect("valid scene keyframe");
    let err = Keyframe::lift_cover(scene_kf).expect_err("wrong role");
    assert!(matches!(
      err,
      GraphError::RoleMismatch {
        kind: NodeKind::CoverKeyframe,
        ..
      }
    ));
  }
}

// --- conversion traits: flat ⇄ graph ---------------------------------------

/// Trait form of [`Video::try_from_flat`] —
/// `(expected_media, facet, tracks, cover)`.
impl
  TryFrom<(
    Uuid7,
    domain::Video<Uuid7>,
    Vec<VideoTrack<Uuid7>>,
    Option<domain::Keyframe<Uuid7>>,
  )> for Video<Uuid7>
{
  type Error = GraphError;

  #[inline(always)]
  fn try_from(
    (expected_media, facet, tracks, cover): (
      Uuid7,
      domain::Video<Uuid7>,
      Vec<VideoTrack<Uuid7>>,
      Option<domain::Keyframe<Uuid7>>,
    ),
  ) -> Result<Self, Self::Error> {
    Self::try_from_flat(&expected_media, facet, tracks, cover)
  }
}

/// Re-attach to `media_id` and rebuild the flat facet; the track-id vec
/// is re-derived from the embedded tracks, and `cover_keyframe_id` from the
/// embedded cover — both then dropped (convert the children first when
/// persisting the tree).
impl From<(Uuid7, Video<Uuid7>)> for domain::Video<Uuid7> {
  fn from((media_id, g): (Uuid7, Video<Uuid7>)) -> Self {
    let Video {
      id,
      total_scenes,
      track_progress,
      tracks,
      cover,
    } = g;
    domain::Video::rehydrate(VideoParts {
      id,
      media_id,
      total_scenes,
      tracks: tracks.iter().map(|t| *t.id_ref()).collect(),
      track_progress,
      cover_keyframe_id: cover.as_ref().map(|k| *k.id_ref()),
    })
  }
}

/// Trait form of [`VideoTrack::try_from_flat`] — `(expected_video, track, scenes)`.
impl TryFrom<(Uuid7, domain::VideoTrack<Uuid7>, Vec<Scene<Uuid7>>)> for VideoTrack<Uuid7> {
  type Error = GraphError;

  #[inline(always)]
  fn try_from(
    (expected_video, track, scenes): (Uuid7, domain::VideoTrack<Uuid7>, Vec<Scene<Uuid7>>),
  ) -> Result<Self, Self::Error> {
    Self::try_from_flat(&expected_video, track, scenes)
  }
}

/// Re-attach to `video_id` and rebuild the flat track; the scene-id vec
/// is re-derived from the embedded scenes, which are then dropped —
/// convert them first when persisting the tree.
impl From<(Uuid7, VideoTrack<Uuid7>)> for domain::VideoTrack<Uuid7> {
  fn from((video_id, g): (Uuid7, VideoTrack<Uuid7>)) -> Self {
    let VideoTrack {
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
    } = g;
    domain::VideoTrack::rehydrate(VideoTrackParts {
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
      scenes: scenes.iter().map(|s| *s.id_ref()).collect(),
      metadata,
      index_status,
      index_errors,
      provenance,
    })
  }
}

/// Trait form of [`Scene::try_from_flat`] — `(expected_track, scene, keyframes)`.
impl TryFrom<(Uuid7, domain::Scene<Uuid7>, Vec<domain::Keyframe<Uuid7>>)> for Scene<Uuid7> {
  type Error = GraphError;

  #[inline(always)]
  fn try_from(
    (expected_track, scene, keyframes): (Uuid7, domain::Scene<Uuid7>, Vec<domain::Keyframe<Uuid7>>),
  ) -> Result<Self, Self::Error> {
    Self::try_from_flat(&expected_track, scene, keyframes)
  }
}

/// Re-attach to `video_track_id` and rebuild the flat scene; the
/// keyframe-id vec is re-derived from the embedded keyframes, which are
/// then dropped — convert them first when persisting the tree.
impl From<(Uuid7, Scene<Uuid7>)> for domain::Scene<Uuid7> {
  fn from((video_track_id, g): (Uuid7, Scene<Uuid7>)) -> Self {
    let Scene {
      id,
      index,
      span,
      detector,
      keyframes,
      description,
    } = g;
    domain::Scene::rehydrate(SceneParts {
      id,
      video_track_id,
      index,
      span,
      detector,
      keyframes: keyframes.iter().map(|k| *k.id_ref()).collect(),
      description,
    })
  }
}

/// Trait form of [`Keyframe::try_from_flat`] — `(expected_scene, flat)`.
impl TryFrom<(Uuid7, domain::Keyframe<Uuid7>)> for Keyframe<Uuid7> {
  type Error = GraphError;

  #[inline(always)]
  fn try_from(
    (expected_scene, keyframe): (Uuid7, domain::Keyframe<Uuid7>),
  ) -> Result<Self, Self::Error> {
    Self::try_from_flat(&expected_scene, keyframe)
  }
}

/// Re-attach a **scene** keyframe to `scene_id` and rebuild the flat
/// aggregate. The supplied `scene_id` becomes the keyframe's `Some`
/// scene FK; the graph `role` is preserved (it is `Scene` for any keyframe
/// that rode the scene chain). For the cover keyframe use
/// [`cover_into_flat`].
impl From<(Uuid7, Keyframe<Uuid7>)> for domain::Keyframe<Uuid7> {
  fn from((scene_id, g): (Uuid7, Keyframe<Uuid7>)) -> Self {
    keyframe_into_flat(Some(scene_id), g)
  }
}

/// Rebuild the flat **cover** keyframe from the graph shape — no scene FK
/// (`scene_id = None`), the graph `role` (`Cover`) preserved. This is the
/// cover analog of the scene-attach [`From<(Uuid7, Keyframe)>`] impl above;
/// the cover is addressed by its video, not by a scene.
pub fn cover_into_flat(g: Keyframe<Uuid7>) -> domain::Keyframe<Uuid7> {
  keyframe_into_flat(None, g)
}

/// Shared graph→flat keyframe rebuild. `scene_id` is `Some(_)` for a scene
/// keyframe (the re-attached parent FK) and `None` for the cover.
fn keyframe_into_flat(scene_id: Option<Uuid7>, g: Keyframe<Uuid7>) -> domain::Keyframe<Uuid7> {
  let Keyframe {
    id,
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
  } = g;
  domain::Keyframe::rehydrate(KeyframeParts {
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
  })
}

#[cfg(test)]
mod conv_tests {
  use core::num::NonZeroU32;

  use mediatime::Timebase;

  use super::*;

  #[test]
  fn scene_round_trips_through_graph() {
    let track_id = Uuid7::new();
    let tb = Timebase::new(1, NonZeroU32::new(1000).unwrap());
    let flat = domain::Scene::try_new(
      Uuid7::new(),
      track_id,
      0,
      TimeRange::new(0, 1000, tb),
      SceneDetector::Manual,
    )
    .expect("valid scene");
    let lifted: Scene<Uuid7> = (track_id, flat.clone(), vec![])
      .try_into()
      .expect("coherent");
    let back: domain::Scene<Uuid7> = (track_id, lifted).into();
    assert_eq!(back, flat);
  }

  #[test]
  fn cover_keyframe_round_trips_through_graph() {
    use mediaframe::frame::Dimensions;
    let tb = Timebase::new(1, NonZeroU32::new(1000).unwrap());
    let flat = domain::Keyframe::try_new_cover(
      Uuid7::new(),
      Uuid7::new(),
      mediatime::Timestamp::new(0, tb),
      Dimensions::new(640, 360),
      KeyframeExtractor::CompositeQuality,
    )
    .expect("valid cover keyframe");
    let lifted = Keyframe::lift_cover(flat.clone()).expect("coherent");
    // The cover graph→flat door drops no fields and keeps role/scene_id.
    let back = cover_into_flat(lifted);
    assert_eq!(back, flat);
    assert!(back.role().is_cover());
    assert!(back.scene_id_ref().is_none());
  }
}
