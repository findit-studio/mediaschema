//! SQLite row shapes for the video-cluster aggregates: the `Video`
//! facet, `VideoTrack`, `Scene`, `Keyframe` (+ the per-detection child
//! tables).
//!
//! Identity / FK columns are native `uuid`. Nested value-objects are
//! flattened into real, individually-typed columns; `Option<VO>` rides
//! as a discriminating column plus all-NULL payload columns when absent.
//! Open descriptor enums (`VideoCodec`, `PixelFormat`, `color::Info`,
//! `KeyframeExtractor`, `SceneDetector`) ride as `text` slugs / coded
//! integers per their wire form. Bitflags (`VideoIndexStatus`,
//! `TrackDisposition`) ride as their `bits()` integer. Media-time values
//! flatten to a PTS `BIGINT` + timebase num/den.
//!
//! Collections ride in child tables: `VideoTrack::index_errors` →
//! `video_track_index_error`; the `Keyframe` detection slices each have
//! their own per-kind child table keyed by `(keyframe, ordinal)`; the
//! deeper-nested sub-collections (`BodyPoseDetection::joints`,
//! `FaceLandmarksDetection::regions`, `FaceLandmarkRegion::points`) ride
//! in their own sub-child tables keyed by the parent detection's
//! `(keyframe, ordinal)` plus an inner ordinal. The reverse-FK `Vec<Id>`
//! fields (`Video::tracks`, `VideoTrack::scenes`, `Scene::keyframes`)
//! are NOT stored — they are derived by querying the child table's FK.

use std::vec::Vec;

use bytes::Bytes;
use indexmap::IndexMap;
use mediaframe::{
  codec::VideoCodec,
  color::{
    ChromaCoord, ChromaLocation, ContentLightLevel, DolbyVisionConfig, DynamicRange,
    HdrStaticMetadata, Info as ColorInfo, MasteringDisplay, Matrix, Primaries, Transfer,
  },
  disposition::TrackDisposition,
  frame::{
    Dimensions, FieldOrder, FrameRate, Rational, Rect, Rotation, SampleAspectRatio, StereoMode,
  },
  pixel_format::PixelFormat,
};
use smol_str::SmolStr;

use crate::{
  domain::{
    aggregates::video::{
      detections::{
        ActionDetection, Aesthetics, AnimalAnalysis, BarcodeDetection, BodyPose3DDetection,
        BodyPose3DHeightEstimation, BodyPose3DJoint, BodyPoseDetection, BodyPoseJoint, BoundingBox,
        Detection, DocumentSegment, DominantColor, FaceDetection, FaceLandmarkRegion,
        FaceLandmarksDetection, HandChirality, HandPoseDetection, HorizonInfo, HumanAnalysis,
        ObjectDetection, PersonInstanceMaskDetection, PersonSegmentationMask, SaliencyRegion,
        SubjectDetection, TextDetection, VlmAnalysis,
      },
      KeyframeError, SceneError, VideoError, VideoTrackError,
    },
    vo::{IndexProgress, LocalizedText, Provenance},
    ErrorCode, ErrorInfo, Keyframe, KeyframeExtractor, Rgba, Scene, SceneDetector, Uuid7, Video,
    VideoIndexStatus, VideoTrack,
  },
  sqlx::{
    dto::{bytes_to_uuid7, timestamp_from_parts},
    SqlxError,
  },
};

// ===========================================================================
// Video facet
// ===========================================================================

/// SQLite row shape for the [`Video`] facet.
///
/// `tracks` (a `Vec<Id>` reverse of `video_track.video_id`) is not stored;
/// `total_scenes` + the flattened `track_progress` rollup are.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteVideoRow {
  pub id: Vec<u8>,
  pub media_id: Vec<u8>,
  pub total_scenes: i64,
  pub track_progress_total: i64,
  pub track_progress_indexed: i64,
  pub track_progress_failed: i64,
}

impl From<&Video<Uuid7>> for SqliteVideoRow {
  fn from(v: &Video<Uuid7>) -> Self {
    let p = v.track_progress_ref();
    Self {
      id: v.id_ref().as_bytes().to_vec(),
      media_id: v.media_id_ref().as_bytes().to_vec(),
      total_scenes: i64::from(v.total_scenes()),
      track_progress_total: i64::from(p.total()),
      track_progress_indexed: i64::from(p.indexed()),
      track_progress_failed: i64::from(p.failed()),
    }
  }
}

impl TryFrom<SqliteVideoRow> for Video<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteVideoRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let media_id = bytes_to_uuid7(&r.media_id)?;
    let total_scenes = u32_from_i64(r.total_scenes, "Video.total_scenes")?;
    let progress = IndexProgress::from_parts(
      u32_from_i64(r.track_progress_total, "Video.track_progress_total")?,
      u32_from_i64(r.track_progress_indexed, "Video.track_progress_indexed")?,
      u32_from_i64(r.track_progress_failed, "Video.track_progress_failed")?,
    );
    let v = Video::try_new(id, media_id)
      .map_err(|e: VideoError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    Ok(
      v.with_total_scenes(total_scenes)
        .with_track_progress(progress),
    )
  }
}

// ===========================================================================
// VideoTrack
// ===========================================================================

/// SQLite row shape for [`VideoTrack`].
///
/// Nested `::mediaframe` descriptors flatten as: `Dimensions` →
/// `width` / `height`; `Rect` (visible_rect) → 4 cols + `has_visible_rect`;
/// `SampleAspectRatio` → `sar_num` / `sar_den`; `PixelFormat::to_u32`;
/// `ColorInfo` → 5 integer columns (primaries / transfer / matrix / range
/// / chroma_location); `HdrStaticMetadata` → `hdr_*` cols + the mastering
/// and content-light sub-discriminants; `Rotation::to_u32`; `FrameRate`
/// → `fr_num` / `fr_den` / `fr_is_vfr`; `FieldOrder::to_u32`;
/// `StereoMode::to_u32`; `DolbyVisionConfig` → 5 cols (`dovi_*`).
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteVideoTrackRow {
  pub id: Vec<u8>,
  pub video_id: Vec<u8>,

  // source locators
  pub stream_index: Option<i64>,
  pub container_track_id: Option<i64>,

  // media-time
  pub start_pts: Option<i64>,
  pub start_pts_tb_num: Option<i64>,
  pub start_pts_tb_den: Option<i64>,
  pub duration_pts: Option<i64>,
  pub duration_tb_num: Option<i64>,
  pub duration_tb_den: Option<i64>,

  // codec
  pub codec: String,
  pub profile: Option<String>,
  pub level: Option<i64>,

  // bitstream / signal
  pub bit_rate: i64,
  pub nb_frames: Option<i64>,
  pub has_b_frames: i64,
  pub closed_gop: Option<i64>,
  pub bits_per_raw_sample: Option<i64>,

  // dimensions / visible_rect / SAR / pixel format
  pub width: i64,
  pub height: i64,
  pub has_visible_rect: i64,
  pub visible_rect_x: Option<i64>,
  pub visible_rect_y: Option<i64>,
  pub visible_rect_w: Option<i64>,
  pub visible_rect_h: Option<i64>,
  pub sar_num: i64,
  pub sar_den: i64,
  pub pixel_format: i64,

  // color::Info (5 closed/coded enum integer columns)
  pub color_primaries: i64,
  pub color_transfer: i64,
  pub color_matrix: i64,
  pub color_range: i64,
  pub color_chroma_location: i64,

  // HDR static metadata (presence + nested sub-presences)
  pub has_hdr_static: i64,
  // mastering display
  pub hdr_has_mastering: i64,
  pub hdr_primary_r_x: Option<i64>,
  pub hdr_primary_r_y: Option<i64>,
  pub hdr_primary_g_x: Option<i64>,
  pub hdr_primary_g_y: Option<i64>,
  pub hdr_primary_b_x: Option<i64>,
  pub hdr_primary_b_y: Option<i64>,
  pub hdr_white_point_x: Option<i64>,
  pub hdr_white_point_y: Option<i64>,
  pub hdr_max_luminance: Option<i64>,
  pub hdr_min_luminance: Option<i64>,
  // content light
  pub hdr_has_content_light: i64,
  pub hdr_max_cll: Option<i64>,
  pub hdr_max_fall: Option<i64>,

  // rotation / frame_rate / field_order / stereo_mode
  pub rotation: i64,
  pub fr_num: i64,
  pub fr_den: i64,
  pub fr_is_vfr: i64,
  /// `AVStream.avg_frame_rate` — empirical average; defaults `0/1`
  /// (= `FrameRate::default()`, absent). For CFR content this matches
  /// (`fr_num`, `fr_den`); for VFR content the two diverge.
  pub avg_fr_num: i64,
  pub avg_fr_den: i64,
  pub field_order: i64,
  pub stereo_mode: Option<i64>,

  // dolby vision
  pub has_dovi: i64,
  pub dovi_profile: Option<i64>,
  pub dovi_level: Option<i64>,
  pub dovi_rpu_present: Option<i64>,
  pub dovi_el_present: Option<i64>,
  pub dovi_bl_signal_compat_id: Option<i64>,

  // findit signals
  pub has_embedded_captions: i64,
  pub disposition: i64,
  pub is_primary: i64,
  pub auto_selected: i64,

  // provenance
  pub provenance_model_name: String,
  pub provenance_model_version: String,
  pub provenance_prompt_version: String,
  pub provenance_indexer_version: String,

  // index status
  pub index_status: i64,
}

/// One `video_track_index_error` child row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteVideoTrackIndexErrorRow {
  pub video_track_id: Vec<u8>,
  pub ordinal: i64,
  pub code: i64,
  pub message: String,
}

/// SQLite row for `video_track_metadata`. Position in the per-
/// `video_track_id` `ordinal` sequence IS the [`IndexMap`] insertion
/// order. `video_track_from_rows` sorts by `ordinal` on decode.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteVideoTrackMetadataRow {
  pub video_track_id: Vec<u8>,
  pub ordinal: i64,
  pub key: String,
  pub value: String,
}

impl From<&VideoTrack<Uuid7>>
  for (
    SqliteVideoTrackRow,
    Vec<SqliteVideoTrackIndexErrorRow>,
    Vec<SqliteVideoTrackMetadataRow>,
  )
{
  fn from(t: &VideoTrack<Uuid7>) -> Self {
    let id = t.id_ref().as_bytes().to_vec();
    let prov = t.provenance_ref();
    let dims = t.dimensions();
    let sar = t.sample_aspect_ratio();
    let color = t.color_ref();
    let fr = t.frame_rate();
    let avg_fr = t.avg_frame_rate();
    let visible_rect = t.visible_rect();
    let dovi = t.dovi();
    let hdr = t.hdr_static_ref();
    let mastering = hdr.and_then(HdrStaticMetadata::mastering);
    let content_light = hdr.and_then(HdrStaticMetadata::content_light);
    let start_pts = t.start_pts_ref();
    let duration = t.duration_ref();
    let row = SqliteVideoTrackRow {
      id: id.clone(),
      video_id: t.video_id_ref().as_bytes().to_vec(),
      stream_index: t.stream_index().map(i64::from),
      container_track_id: t.container_track_id().map(|v| v as i64),
      start_pts: start_pts.map(mediatime::Timestamp::pts),
      start_pts_tb_num: start_pts.map(|p| i64::from(p.timebase().num())),
      start_pts_tb_den: start_pts.map(|p| i64::from(p.timebase().den().get())),
      duration_pts: duration.map(mediatime::Timestamp::pts),
      duration_tb_num: duration.map(|p| i64::from(p.timebase().num())),
      duration_tb_den: duration.map(|p| i64::from(p.timebase().den().get())),
      codec: t.codec_ref().as_str().to_owned(),
      profile: t.profile().map(str::to_owned),
      level: t.level().map(i64::from),
      bit_rate: t.bit_rate() as i64,
      nb_frames: t.nb_frames().map(|v| v as i64),
      has_b_frames: i64::from(t.has_b_frames()),
      closed_gop: t.closed_gop().map(i64::from),
      bits_per_raw_sample: t.bits_per_raw_sample().map(i64::from),
      width: i64::from(dims.width()),
      height: i64::from(dims.height()),
      has_visible_rect: i64::from(visible_rect.is_some()),
      visible_rect_x: visible_rect.map(|r| i64::from(r.x())),
      visible_rect_y: visible_rect.map(|r| i64::from(r.y())),
      visible_rect_w: visible_rect.map(|r| i64::from(r.width())),
      visible_rect_h: visible_rect.map(|r| i64::from(r.height())),
      sar_num: i64::from(sar.num()),
      sar_den: i64::from(sar.den().get()),
      pixel_format: i64::from(t.pixel_format().to_u32()),
      color_primaries: i64::from(color.primaries().to_u32()),
      color_transfer: i64::from(color.transfer().to_u32()),
      color_matrix: i64::from(color.matrix().to_u32()),
      color_range: i64::from(color.range().to_u32()),
      color_chroma_location: i64::from(color.chroma_location().to_u32()),
      has_hdr_static: i64::from(hdr.is_some()),
      hdr_has_mastering: i64::from(mastering.is_some()),
      hdr_primary_r_x: mastering.map(|m| i64::from(m.display_primaries()[0].x())),
      hdr_primary_r_y: mastering.map(|m| i64::from(m.display_primaries()[0].y())),
      hdr_primary_g_x: mastering.map(|m| i64::from(m.display_primaries()[1].x())),
      hdr_primary_g_y: mastering.map(|m| i64::from(m.display_primaries()[1].y())),
      hdr_primary_b_x: mastering.map(|m| i64::from(m.display_primaries()[2].x())),
      hdr_primary_b_y: mastering.map(|m| i64::from(m.display_primaries()[2].y())),
      hdr_white_point_x: mastering.map(|m| i64::from(m.white_point().x())),
      hdr_white_point_y: mastering.map(|m| i64::from(m.white_point().y())),
      hdr_max_luminance: mastering.map(|m| i64::from(m.max_luminance())),
      hdr_min_luminance: mastering.map(|m| i64::from(m.min_luminance())),
      hdr_has_content_light: i64::from(content_light.is_some()),
      hdr_max_cll: content_light.map(|c| i64::from(c.max_cll())),
      hdr_max_fall: content_light.map(|c| i64::from(c.max_fall())),
      rotation: i64::from(t.rotation().to_u32()),
      fr_num: i64::from(fr.rate().num()),
      fr_den: i64::from(fr.rate().den().get()),
      fr_is_vfr: i64::from(fr.is_vfr()),
      avg_fr_num: i64::from(avg_fr.rate().num()),
      avg_fr_den: i64::from(avg_fr.rate().den().get()),
      field_order: i64::from(t.field_order().to_u32()),
      stereo_mode: t.stereo_mode().map(|s| i64::from(s.to_u32())),
      has_dovi: i64::from(dovi.is_some()),
      dovi_profile: dovi.map(|d| i64::from(d.profile())),
      dovi_level: dovi.map(|d| i64::from(d.level())),
      dovi_rpu_present: dovi.map(|d| i64::from(d.rpu_present())),
      dovi_el_present: dovi.map(|d| i64::from(d.el_present())),
      dovi_bl_signal_compat_id: dovi.map(|d| i64::from(d.bl_signal_compat_id())),
      has_embedded_captions: i64::from(t.has_embedded_captions()),
      disposition: i64::from(t.disposition().bits()),
      is_primary: i64::from(t.is_primary()),
      auto_selected: i64::from(t.auto_selected()),
      provenance_model_name: prov.model_name().to_owned(),
      provenance_model_version: prov.model_version().to_owned(),
      provenance_prompt_version: prov.prompt_version().to_owned(),
      provenance_indexer_version: prov.indexer_version().to_owned(),
      index_status: i64::from(t.index_status().bits()),
    };
    let errors = t
      .index_errors_slice()
      .iter()
      .enumerate()
      .map(|(i, e)| SqliteVideoTrackIndexErrorRow {
        video_track_id: id.clone(),
        ordinal: i as i64,
        code: i64::from(e.code().as_u32()),
        message: e.message().to_owned(),
      })
      .collect();
    let metadata = t
      .metadata_ref()
      .iter()
      .enumerate()
      .map(|(i, (k, v))| SqliteVideoTrackMetadataRow {
        video_track_id: id.clone(),
        ordinal: i as i64,
        key: k.as_str().to_owned(),
        value: v.as_str().to_owned(),
      })
      .collect();
    (row, errors, metadata)
  }
}

impl
  TryFrom<(
    SqliteVideoTrackRow,
    Vec<SqliteVideoTrackIndexErrorRow>,
    Vec<SqliteVideoTrackMetadataRow>,
  )> for VideoTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, errors, metadata): (
      SqliteVideoTrackRow,
      Vec<SqliteVideoTrackIndexErrorRow>,
      Vec<SqliteVideoTrackMetadataRow>,
    ),
  ) -> Result<Self, Self::Error> {
    video_track_from_rows(r, errors, metadata)
  }
}

/// Reconstruct a [`VideoTrack`] from its row, `index_errors` rows, and
/// `metadata` rows.
pub fn video_track_from_rows(
  r: SqliteVideoTrackRow,
  mut errors: Vec<SqliteVideoTrackIndexErrorRow>,
  mut metadata: Vec<SqliteVideoTrackMetadataRow>,
) -> Result<VideoTrack<Uuid7>, SqlxError> {
  {
    let id = bytes_to_uuid7(&r.id)?;
    let video_id = bytes_to_uuid7(&r.video_id)?;
    let mut t = VideoTrack::try_new(id, video_id)
      .map_err(|e: VideoTrackError| SqlxError::DomainConstructorRejected(e.to_string()))?;

    // Source locators.
    t = t
      .with_stream_index(opt_u32(r.stream_index, "VideoTrack.stream_index")?)
      .with_container_track_id(r.container_track_id.map(|v| v as u64));

    // Media-time.
    if let Some(pts) = r.start_pts {
      let (num, den) = require_timebase(
        r.start_pts_tb_num,
        r.start_pts_tb_den,
        "VideoTrack.start_pts",
      )?;
      t = t.with_start_pts(Some(timestamp_from_parts(pts, num, den)?));
    }
    if let Some(pts) = r.duration_pts {
      let (num, den) =
        require_timebase(r.duration_tb_num, r.duration_tb_den, "VideoTrack.duration")?;
      t = t
        .try_with_duration(Some(timestamp_from_parts(pts, num, den)?))
        .map_err(track_err)?;
    }

    // Codec.
    t = t.with_codec(parse_video_codec(&r.codec));
    if let Some(p) = r.profile {
      t = t.with_profile(Some(SmolStr::from(p)));
    }
    if let Some(level) = r.level {
      t = t.with_level(Some(u16_from_i32(level, "VideoTrack.level")?));
    }

    // Bitstream / signal.
    t = t
      .with_bit_rate(r.bit_rate as u64)
      .with_nb_frames(r.nb_frames.map(|v| v as u64))
      .with_has_b_frames(bool_from_i64(r.has_b_frames))
      .with_closed_gop(r.closed_gop.map(bool_from_i64));
    if let Some(b) = r.bits_per_raw_sample {
      t = t.with_bits_per_raw_sample(Some(u8_from_i16(b, "VideoTrack.bits_per_raw_sample")?));
    }

    // Dimensions, visible_rect, SAR, pixel_format. `dimensions` is a
    // validating mutator — set it before any crop, and before
    // index_status (which is plain on VideoTrack but still safest
    // ordered after geometry).
    let dims = Dimensions::new(
      u32_from_i64(r.width, "VideoTrack.width")?,
      u32_from_i64(r.height, "VideoTrack.height")?,
    );
    t = t.try_with_dimensions(dims).map_err(track_err)?;
    if bool_from_i64(r.has_visible_rect) {
      let rect = Rect::new(
        u32_from_i64(
          r.visible_rect_x.unwrap_or_default(),
          "VideoTrack.visible_rect_x",
        )?,
        u32_from_i64(
          r.visible_rect_y.unwrap_or_default(),
          "VideoTrack.visible_rect_y",
        )?,
        u32_from_i64(
          r.visible_rect_w.unwrap_or_default(),
          "VideoTrack.visible_rect_w",
        )?,
        u32_from_i64(
          r.visible_rect_h.unwrap_or_default(),
          "VideoTrack.visible_rect_h",
        )?,
      );
      t = t.try_with_visible_rect(Some(rect)).map_err(track_err)?;
    }
    t = t.with_sample_aspect_ratio(SampleAspectRatio::new(
      u32_from_i64(r.sar_num, "VideoTrack.sar_num")?,
      nonzero_u32_from_i64(r.sar_den, "VideoTrack.sar_den")?,
    ));
    t = t.with_pixel_format(PixelFormat::from_u32(u32_from_i64(
      r.pixel_format,
      "VideoTrack.pixel_format",
    )?));

    // Colour info (5 closed/coded integer columns).
    let color = ColorInfo::new(
      Primaries::from_u32(u32_from_i64(
        r.color_primaries,
        "VideoTrack.color_primaries",
      )?),
      Transfer::from_u32(u32_from_i64(r.color_transfer, "VideoTrack.color_transfer")?),
      Matrix::from_u32(u32_from_i64(r.color_matrix, "VideoTrack.color_matrix")?),
      DynamicRange::from_u32(u32_from_i64(r.color_range, "VideoTrack.color_range")?),
      ChromaLocation::from_u32(u32_from_i64(
        r.color_chroma_location,
        "VideoTrack.color_chroma_location",
      )?),
    );
    t = t.with_color(color);

    // HDR static metadata.
    if bool_from_i64(r.has_hdr_static) {
      let mastering = if bool_from_i64(r.hdr_has_mastering) {
        Some(MasteringDisplay::new(
          [
            ChromaCoord::new(
              u32_from_i64(
                r.hdr_primary_r_x.unwrap_or_default(),
                "VideoTrack.hdr_primary_r_x",
              )?,
              u32_from_i64(
                r.hdr_primary_r_y.unwrap_or_default(),
                "VideoTrack.hdr_primary_r_y",
              )?,
            ),
            ChromaCoord::new(
              u32_from_i64(
                r.hdr_primary_g_x.unwrap_or_default(),
                "VideoTrack.hdr_primary_g_x",
              )?,
              u32_from_i64(
                r.hdr_primary_g_y.unwrap_or_default(),
                "VideoTrack.hdr_primary_g_y",
              )?,
            ),
            ChromaCoord::new(
              u32_from_i64(
                r.hdr_primary_b_x.unwrap_or_default(),
                "VideoTrack.hdr_primary_b_x",
              )?,
              u32_from_i64(
                r.hdr_primary_b_y.unwrap_or_default(),
                "VideoTrack.hdr_primary_b_y",
              )?,
            ),
          ],
          ChromaCoord::new(
            u32_from_i64(
              r.hdr_white_point_x.unwrap_or_default(),
              "VideoTrack.hdr_white_point_x",
            )?,
            u32_from_i64(
              r.hdr_white_point_y.unwrap_or_default(),
              "VideoTrack.hdr_white_point_y",
            )?,
          ),
          u32_from_i64(
            r.hdr_max_luminance.unwrap_or_default(),
            "VideoTrack.hdr_max_luminance",
          )?,
          u32_from_i64(
            r.hdr_min_luminance.unwrap_or_default(),
            "VideoTrack.hdr_min_luminance",
          )?,
        ))
      } else {
        None
      };
      let content_light = if bool_from_i64(r.hdr_has_content_light) {
        Some(ContentLightLevel::new(
          u32_from_i64(r.hdr_max_cll.unwrap_or_default(), "VideoTrack.hdr_max_cll")?,
          u32_from_i64(
            r.hdr_max_fall.unwrap_or_default(),
            "VideoTrack.hdr_max_fall",
          )?,
        ))
      } else {
        None
      };
      t = t.with_hdr_static(Some(HdrStaticMetadata::new(mastering, content_light)));
    }

    t = t.with_rotation(Rotation::from_u32(u32_from_i64(
      r.rotation,
      "VideoTrack.rotation",
    )?));
    t = t.with_frame_rate(FrameRate::new(
      Rational::new(
        u32_from_i64(r.fr_num, "VideoTrack.fr_num")?,
        nonzero_u32_from_i64(r.fr_den, "VideoTrack.fr_den")?,
      ),
      bool_from_i64(r.fr_is_vfr),
    ));
    t = t.with_avg_frame_rate(FrameRate::new(
      Rational::new(
        u32_from_i64(r.avg_fr_num, "VideoTrack.avg_fr_num")?,
        nonzero_u32_from_i64(r.avg_fr_den, "VideoTrack.avg_fr_den")?,
      ),
      false,
    ));
    t = t.with_field_order(FieldOrder::from_u32(u32_from_i64(
      r.field_order,
      "VideoTrack.field_order",
    )?));
    if let Some(sm) = r.stereo_mode {
      t = t.with_stereo_mode(Some(StereoMode::from_u32(u32_from_i64(
        sm,
        "VideoTrack.stereo_mode",
      )?)));
    }

    if bool_from_i64(r.has_dovi) {
      t = t.with_dovi(Some(DolbyVisionConfig::new(
        u8_from_i16(
          r.dovi_profile.unwrap_or_default(),
          "VideoTrack.dovi_profile",
        )?,
        u8_from_i16(r.dovi_level.unwrap_or_default(), "VideoTrack.dovi_level")?,
        bool_from_i64(r.dovi_rpu_present.unwrap_or_default()),
        bool_from_i64(r.dovi_el_present.unwrap_or_default()),
        u8_from_i16(
          r.dovi_bl_signal_compat_id.unwrap_or_default(),
          "VideoTrack.dovi_bl_signal_compat_id",
        )?,
      )));
    }

    t = t
      .with_has_embedded_captions(bool_from_i64(r.has_embedded_captions))
      .with_disposition(TrackDisposition::from_bits_truncate(u32_from_i64(
        r.disposition,
        "VideoTrack.disposition",
      )?))
      .with_is_primary(bool_from_i64(r.is_primary))
      .with_auto_selected(bool_from_i64(r.auto_selected))
      .with_provenance(Provenance::from_parts(
        r.provenance_model_name,
        r.provenance_model_version,
        r.provenance_prompt_version,
        r.provenance_indexer_version,
      ))
      .with_index_status(VideoIndexStatus::from_bits_truncate(u32_from_i64(
        r.index_status,
        "VideoTrack.index_status",
      )?));

    errors.sort_by_key(|e| e.ordinal);
    let mut infos = Vec::with_capacity(errors.len());
    for e in errors {
      let code = u32_from_i32(e.code, "VideoTrack.index_error.code")?;
      infos.push(ErrorInfo::new(ErrorCode::from_u32(code), e.message));
    }
    t = t.with_index_errors(infos);

    metadata.sort_by_key(|m| m.ordinal);
    let mut bag = IndexMap::with_capacity(metadata.len());
    for entry in metadata {
      if entry.video_track_id != r.id {
        return Err(SqlxError::DomainConstructorRejected(
          "video_track_metadata.video_track_id does not match parent video_track.id".to_owned(),
        ));
      }
      bag.insert(SmolStr::from(entry.key), SmolStr::from(entry.value));
    }
    t = t.with_metadata(bag);

    Ok(t)
  }
}

// ===========================================================================
// Scene
// ===========================================================================

/// SQLite row shape for [`Scene`]. `keyframes` is reverse-FK
/// (`keyframe.parent`) — not stored.
///
/// `span` flattens to `start_pts` / `end_pts` PTS ticks only; the timebase
/// lives on the parent `video_track`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSceneRow {
  pub id: Vec<u8>,
  pub video_track_id: Vec<u8>,
  pub index: i64,
  pub span_start_pts: i64,
  pub span_end_pts: i64,
  pub detector: String,
  pub description: String,
}

impl From<&Scene<Uuid7>> for SqliteSceneRow {
  fn from(s: &Scene<Uuid7>) -> Self {
    let span = s.span_ref();
    Self {
      id: s.id_ref().as_bytes().to_vec(),
      video_track_id: s.video_track_id_ref().as_bytes().to_vec(),
      index: i64::from(s.index()),
      span_start_pts: span.start_pts(),
      span_end_pts: span.end_pts(),
      detector: scene_detector_to_slug(s.detector()).to_owned(),
      description: s.description().to_owned(),
    }
  }
}

/// Rebuild a [`Scene`] from a stored row. `parent_timebase` is the
/// per-stream timebase carried on the parent `video_track` row.
pub fn scene_from_row(
  r: SqliteSceneRow,
  parent_timebase: mediatime::Timebase,
) -> Result<Scene<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(&r.id)?;
  let video_track_id = bytes_to_uuid7(&r.video_track_id)?;
  let index = u32_from_i64(r.index, "Scene.index")?;
  let span = mediatime::TimeRange::try_new(r.span_start_pts, r.span_end_pts, parent_timebase)
    .ok_or_else(|| {
      SqlxError::DomainConstructorRejected(format!(
        "TimeRange start_pts ({}) must be <= end_pts ({})",
        r.span_start_pts, r.span_end_pts
      ))
    })?;
  let detector = parse_scene_detector(&r.detector)?;
  let s = Scene::try_new(id, video_track_id, index, span, detector)
    .map_err(|e: SceneError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_description(r.description);
  Ok(s)
}

// ===========================================================================
// Keyframe + per-detection child tables
// ===========================================================================

/// SQLite row shape for [`Keyframe`] — the artifact scalar columns.
/// All detection collections ride in dedicated child tables (see the
/// `Pg*DetectionRow` row types below).
///
/// `pts` flattens to a single PTS tick column; the timebase lives on the
/// parent `video_track`.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeRow {
  pub id: Vec<u8>,
  pub scene_id: Vec<u8>,
  pub pts: i64,
  pub data: Vec<u8>,
  pub mime: String,
  pub width: i64,
  pub height: i64,
  pub extractor: String,
  // VLM scalars (only `description` and `shot_type` are stored on the
  // keyframe row directly; the open-vocab `Vec<LocalizedText>` fields
  // ride in the `keyframe_vlm_label` child table keyed by `kind`).
  pub vlm_description_src: String,
  pub vlm_description_translated: String,
  pub vlm_shot_type: String,
  // Apple-vision scalars: horizon + aesthetics.
  pub horizon_angle: f32,
  pub horizon_confidence: f32,
  pub aesthetics_overall_score: f32,
  pub aesthetics_is_utility: i64,
}

// --- detection child rows ---

/// `keyframe_classification` — apple-vision `Detection` `{label,confidence}`.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeClassificationRow {
  pub keyframe_id: Vec<u8>,
  pub ordinal: i64,
  pub label: String,
  pub confidence: f32,
}

/// `keyframe_object` — `ObjectDetection`: `Detection` + optional bbox.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeObjectRow {
  pub keyframe_id: Vec<u8>,
  pub ordinal: i64,
  pub label: String,
  pub confidence: f32,
  pub has_bbox: i64,
  pub bbox_x: Option<f32>,
  pub bbox_y: Option<f32>,
  pub bbox_w: Option<f32>,
  pub bbox_h: Option<f32>,
}

/// `keyframe_action` — apple-vision body-pose-derived action `Detection`.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeActionRow {
  pub keyframe_id: Vec<u8>,
  pub ordinal: i64,
  pub label: String,
  pub confidence: f32,
}

/// `keyframe_text_detection` — OCR text + confidence + bbox.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeTextDetectionRow {
  pub keyframe_id: Vec<u8>,
  pub ordinal: i64,
  pub text: String,
  pub confidence: f32,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
}

/// `keyframe_barcode` — payload + symbology + confidence + bbox.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeBarcodeRow {
  pub keyframe_id: Vec<u8>,
  pub ordinal: i64,
  pub payload: String,
  pub symbology: String,
  pub confidence: f32,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
}

/// `keyframe_saliency` — attention / objectness saliency region (`kind` =
/// `0` for attention, `1` for objectness).
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeSaliencyRow {
  pub keyframe_id: Vec<u8>,
  pub kind: i64,
  pub ordinal: i64,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
  pub confidence: f32,
}

/// `keyframe_document_segment` — 4 normalised corners + confidence.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeDocumentSegmentRow {
  pub keyframe_id: Vec<u8>,
  pub ordinal: i64,
  pub tl_x: f32,
  pub tl_y: f32,
  pub tr_x: f32,
  pub tr_y: f32,
  pub br_x: f32,
  pub br_y: f32,
  pub bl_x: f32,
  pub bl_y: f32,
  pub confidence: f32,
}

/// `keyframe_color` — colorthief dominant colour.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeColorRow {
  pub keyframe_id: Vec<u8>,
  pub ordinal: i64,
  pub rgba: i64,
  pub name: String,
  pub percentage: f32,
  pub population: i64,
}

// --- human / animal subject + pose detection rows ---

/// `keyframe_subject` — apple-vision subject (humans + animals share the
/// shape). `scope` = `0` human-subject, `1` animal-subject.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeSubjectRow {
  pub keyframe_id: Vec<u8>,
  pub scope: i64,
  pub ordinal: i64,
  pub label: String,
  pub confidence: f32,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
}

/// `keyframe_face` — apple-vision face detection (humans `faces` +
/// `face_rectangles`). `kind` = `0` for `faces`, `1` for
/// `face_rectangles`.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeFaceRow {
  pub keyframe_id: Vec<u8>,
  pub kind: i64,
  pub ordinal: i64,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
  pub confidence: f32,
  pub capture_quality: f32,
  pub roll: f32,
  pub yaw: f32,
  pub pitch: f32,
}

/// `keyframe_body_pose` — 2-D body-pose detection (humans + animals
/// share the shape). `scope` = `0` human, `1` animal.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeBodyPoseRow {
  pub keyframe_id: Vec<u8>,
  pub scope: i64,
  pub ordinal: i64,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
  pub confidence: f32,
}

/// `keyframe_body_pose_joint` — joint of a 2-D body or hand pose row.
/// `scope` = `0` human-body, `1` animal-body, `2` hand.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeBodyPoseJointRow {
  pub keyframe_id: Vec<u8>,
  pub scope: i64,
  pub parent_ordinal: i64,
  pub ordinal: i64,
  pub name: String,
  pub x: f32,
  pub y: f32,
  pub confidence: f32,
}

/// `keyframe_hand_pose` — 2-D hand-pose detection (humans only).
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeHandPoseRow {
  pub keyframe_id: Vec<u8>,
  pub ordinal: i64,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
  pub confidence: f32,
  pub chirality: i64,
}

/// `keyframe_body_pose_3d` — 3-D body-pose detection.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeBodyPose3DRow {
  pub keyframe_id: Vec<u8>,
  pub ordinal: i64,
  pub confidence: f32,
  pub body_height: f32,
  pub height_estimation: i64,
}

/// `keyframe_body_pose_3d_joint` — joint of a 3-D body-pose row.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeBodyPose3DJointRow {
  pub keyframe_id: Vec<u8>,
  pub parent_ordinal: i64,
  pub ordinal: i64,
  pub name: String,
  pub x: f32,
  pub y: f32,
  pub z: f32,
  pub confidence: f32,
}

/// `keyframe_mask` — apple-vision instance / segmentation mask. `kind` =
/// `0` per-person instance mask, `1` whole-frame segmentation mask.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeMaskRow {
  pub keyframe_id: Vec<u8>,
  pub kind: i64,
  pub ordinal: i64,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
  pub confidence: f32,
  pub instance_index: Option<i64>,
  pub width: i64,
  pub height: i64,
  pub data: Vec<u8>,
}

/// `keyframe_face_landmarks` — bbox + confidence header for a
/// face-landmark detection.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeFaceLandmarksRow {
  pub keyframe_id: Vec<u8>,
  pub ordinal: i64,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
  pub confidence: f32,
}

/// `keyframe_face_landmark_region` — a named landmark region inside a
/// face-landmarks row.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeFaceLandmarkRegionRow {
  pub keyframe_id: Vec<u8>,
  pub parent_ordinal: i64,
  pub ordinal: i64,
  pub name: String,
}

/// `keyframe_face_landmark_point` — a normalised point inside a region.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeFaceLandmarkPointRow {
  pub keyframe_id: Vec<u8>,
  pub parent_ordinal: i64,
  pub region_ordinal: i64,
  pub ordinal: i64,
  pub x: f32,
  pub y: f32,
}

/// `keyframe_vlm_label` — one VLM open-vocab `LocalizedText` row.
/// `kind` discriminates which slice:
/// `0` = categories, `1` = tags, `2` = objects, `3` = subjects,
/// `4` = mood, `5` = emotion, `6` = lighting.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteKeyframeVlmLabelRow {
  pub keyframe_id: Vec<u8>,
  pub kind: i64,
  pub ordinal: i64,
  pub src: String,
  pub translated: String,
}

// --- bundled keyframe-row tuple ---

/// Bundled rows for a single [`Keyframe`] — the scalar `SqliteKeyframeRow`
/// plus every per-detection child slice in lockstep.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SqliteKeyframeRows {
  pub keyframe: Option<SqliteKeyframeRow>,
  pub classifications: Vec<SqliteKeyframeClassificationRow>,
  pub objects: Vec<SqliteKeyframeObjectRow>,
  pub actions: Vec<SqliteKeyframeActionRow>,
  pub text_detections: Vec<SqliteKeyframeTextDetectionRow>,
  pub barcodes: Vec<SqliteKeyframeBarcodeRow>,
  pub saliencies: Vec<SqliteKeyframeSaliencyRow>,
  pub document_segments: Vec<SqliteKeyframeDocumentSegmentRow>,
  pub colors: Vec<SqliteKeyframeColorRow>,
  pub subjects: Vec<SqliteKeyframeSubjectRow>,
  pub faces: Vec<SqliteKeyframeFaceRow>,
  pub body_poses: Vec<SqliteKeyframeBodyPoseRow>,
  pub body_pose_joints: Vec<SqliteKeyframeBodyPoseJointRow>,
  pub hand_poses: Vec<SqliteKeyframeHandPoseRow>,
  pub body_poses_3d: Vec<SqliteKeyframeBodyPose3DRow>,
  pub body_pose_3d_joints: Vec<SqliteKeyframeBodyPose3DJointRow>,
  pub masks: Vec<SqliteKeyframeMaskRow>,
  pub face_landmarks: Vec<SqliteKeyframeFaceLandmarksRow>,
  pub face_landmark_regions: Vec<SqliteKeyframeFaceLandmarkRegionRow>,
  pub face_landmark_points: Vec<SqliteKeyframeFaceLandmarkPointRow>,
  pub vlm_labels: Vec<SqliteKeyframeVlmLabelRow>,
}

impl From<&Keyframe<Uuid7>> for SqliteKeyframeRows {
  fn from(k: &Keyframe<Uuid7>) -> Self {
    let id = k.id_ref().as_bytes().to_vec();
    let pts = k.pts_ref();
    let dims = k.dimensions();
    let vlm = k.vlm_ref();
    let aesthetics = k.aesthetics_ref();
    let horizon = k.horizon_ref();
    let row = SqliteKeyframeRow {
      id: id.clone(),
      scene_id: k.scene_id_ref().as_bytes().to_vec(),
      pts: pts.pts(),
      data: k.data().to_vec(),
      mime: k.mime().to_owned(),
      width: i64::from(dims.width()),
      height: i64::from(dims.height()),
      extractor: keyframe_extractor_to_slug(k.extractor()).to_owned(),
      vlm_description_src: vlm.description_ref().src().to_owned(),
      vlm_description_translated: vlm.description_ref().translated().to_owned(),
      vlm_shot_type: vlm.shot_type().to_owned(),
      horizon_angle: horizon.angle(),
      horizon_confidence: horizon.confidence(),
      aesthetics_overall_score: aesthetics.overall_score(),
      aesthetics_is_utility: i64::from(aesthetics.is_utility()),
    };

    let mut out = Self {
      keyframe: Some(row),
      ..Default::default()
    };

    for (ordinal, d) in k.classifications_slice().iter().enumerate() {
      out.classifications.push(SqliteKeyframeClassificationRow {
        keyframe_id: id.clone(),
        ordinal: ordinal as i64,
        label: d.label().to_owned(),
        confidence: d.confidence(),
      });
    }
    for (ordinal, o) in k.objects_slice().iter().enumerate() {
      let bbox = o.bbox_ref();
      out.objects.push(SqliteKeyframeObjectRow {
        keyframe_id: id.clone(),
        ordinal: ordinal as i64,
        label: o.detection_ref().label().to_owned(),
        confidence: o.detection_ref().confidence(),
        has_bbox: i64::from(bbox.is_some()),
        bbox_x: bbox.map(BoundingBox::x),
        bbox_y: bbox.map(BoundingBox::y),
        bbox_w: bbox.map(BoundingBox::width),
        bbox_h: bbox.map(BoundingBox::height),
      });
    }
    for (ordinal, a) in k.actions_slice().iter().enumerate() {
      out.actions.push(SqliteKeyframeActionRow {
        keyframe_id: id.clone(),
        ordinal: ordinal as i64,
        label: a.detection_ref().label().to_owned(),
        confidence: a.detection_ref().confidence(),
      });
    }
    for (ordinal, t) in k.text_detections_slice().iter().enumerate() {
      let bb = t.bbox_ref();
      out.text_detections.push(SqliteKeyframeTextDetectionRow {
        keyframe_id: id.clone(),
        ordinal: ordinal as i64,
        text: t.text().to_owned(),
        confidence: t.confidence(),
        bbox_x: bb.x(),
        bbox_y: bb.y(),
        bbox_w: bb.width(),
        bbox_h: bb.height(),
      });
    }
    for (ordinal, b) in k.barcodes_slice().iter().enumerate() {
      let bb = b.bbox_ref();
      out.barcodes.push(SqliteKeyframeBarcodeRow {
        keyframe_id: id.clone(),
        ordinal: ordinal as i64,
        payload: b.payload().to_owned(),
        symbology: b.symbology().to_owned(),
        confidence: b.confidence(),
        bbox_x: bb.x(),
        bbox_y: bb.y(),
        bbox_w: bb.width(),
        bbox_h: bb.height(),
      });
    }
    for (ordinal, s) in k.attention_saliency_slice().iter().enumerate() {
      push_saliency(&mut out.saliencies, &id, 0, ordinal, s);
    }
    for (ordinal, s) in k.objectness_saliency_slice().iter().enumerate() {
      push_saliency(&mut out.saliencies, &id, 1, ordinal, s);
    }
    for (ordinal, d) in k.document_segments_slice().iter().enumerate() {
      out
        .document_segments
        .push(SqliteKeyframeDocumentSegmentRow {
          keyframe_id: id.clone(),
          ordinal: ordinal as i64,
          tl_x: d.top_left().0,
          tl_y: d.top_left().1,
          tr_x: d.top_right().0,
          tr_y: d.top_right().1,
          br_x: d.bottom_right().0,
          br_y: d.bottom_right().1,
          bl_x: d.bottom_left().0,
          bl_y: d.bottom_left().1,
          confidence: d.confidence(),
        });
    }
    for (ordinal, c) in k.colors_slice().iter().enumerate() {
      out.colors.push(SqliteKeyframeColorRow {
        keyframe_id: id.clone(),
        ordinal: ordinal as i64,
        rgba: i64::from(c.rgb().bits()),
        name: c.name().to_owned(),
        percentage: c.percentage(),
        population: i64::from(c.population()),
      });
    }

    // --- humans + animals
    let humans = k.humans_ref();
    let animals = k.animals_ref();
    for (ordinal, s) in humans.subjects_slice().iter().enumerate() {
      push_subject(&mut out.subjects, &id, 0, ordinal, s);
    }
    for (ordinal, s) in animals.subjects_slice().iter().enumerate() {
      push_subject(&mut out.subjects, &id, 1, ordinal, s);
    }
    for (ordinal, f) in humans.faces_slice().iter().enumerate() {
      push_face(&mut out.faces, &id, 0, ordinal, f);
    }
    for (ordinal, f) in humans.face_rectangles_slice().iter().enumerate() {
      push_face(&mut out.faces, &id, 1, ordinal, f);
    }
    for (ordinal, bp) in humans.body_poses_slice().iter().enumerate() {
      push_body_pose(
        &mut out.body_poses,
        &mut out.body_pose_joints,
        &id,
        0,
        ordinal,
        bp,
      );
    }
    for (ordinal, bp) in animals.body_poses_slice().iter().enumerate() {
      push_body_pose(
        &mut out.body_poses,
        &mut out.body_pose_joints,
        &id,
        1,
        ordinal,
        bp,
      );
    }
    for (ordinal, hp) in humans.hand_poses_slice().iter().enumerate() {
      let bb = hp.bbox_ref();
      out.hand_poses.push(SqliteKeyframeHandPoseRow {
        keyframe_id: id.clone(),
        ordinal: ordinal as i64,
        bbox_x: bb.x(),
        bbox_y: bb.y(),
        bbox_w: bb.width(),
        bbox_h: bb.height(),
        confidence: hp.confidence(),
        chirality: i64::from(hand_chirality_to_i16(hp.chirality())),
      });
      for (jord, j) in hp.joints_slice().iter().enumerate() {
        out.body_pose_joints.push(SqliteKeyframeBodyPoseJointRow {
          keyframe_id: id.clone(),
          scope: 2,
          parent_ordinal: ordinal as i64,
          ordinal: jord as i64,
          name: j.name().to_owned(),
          x: j.x(),
          y: j.y(),
          confidence: j.confidence(),
        });
      }
    }
    for (ordinal, b3) in humans.body_poses_3d_slice().iter().enumerate() {
      out.body_poses_3d.push(SqliteKeyframeBodyPose3DRow {
        keyframe_id: id.clone(),
        ordinal: ordinal as i64,
        confidence: b3.confidence(),
        body_height: b3.body_height(),
        height_estimation: i64::from(height_estimation_to_i16(b3.height_estimation())),
      });
      for (jord, j) in b3.joints_slice().iter().enumerate() {
        out
          .body_pose_3d_joints
          .push(SqliteKeyframeBodyPose3DJointRow {
            keyframe_id: id.clone(),
            parent_ordinal: ordinal as i64,
            ordinal: jord as i64,
            name: j.name().to_owned(),
            x: j.x(),
            y: j.y(),
            z: j.z(),
            confidence: j.confidence(),
          });
      }
    }
    for (ordinal, m) in humans.instance_masks_slice().iter().enumerate() {
      let bb = m.bbox_ref();
      let d = m.dimensions();
      out.masks.push(SqliteKeyframeMaskRow {
        keyframe_id: id.clone(),
        kind: 0,
        ordinal: ordinal as i64,
        bbox_x: bb.x(),
        bbox_y: bb.y(),
        bbox_w: bb.width(),
        bbox_h: bb.height(),
        confidence: m.confidence(),
        instance_index: Some(i64::from(m.instance_index())),
        width: i64::from(d.width()),
        height: i64::from(d.height()),
        data: m.data().to_vec(),
      });
    }
    for (ordinal, m) in humans.segmentation_masks_slice().iter().enumerate() {
      let bb = m.bbox_ref();
      let d = m.dimensions();
      out.masks.push(SqliteKeyframeMaskRow {
        keyframe_id: id.clone(),
        kind: 1,
        ordinal: ordinal as i64,
        bbox_x: bb.x(),
        bbox_y: bb.y(),
        bbox_w: bb.width(),
        bbox_h: bb.height(),
        confidence: m.confidence(),
        instance_index: None,
        width: i64::from(d.width()),
        height: i64::from(d.height()),
        data: m.data().to_vec(),
      });
    }
    for (ordinal, fl) in humans.face_landmarks_slice().iter().enumerate() {
      let bb = fl.bbox_ref();
      out.face_landmarks.push(SqliteKeyframeFaceLandmarksRow {
        keyframe_id: id.clone(),
        ordinal: ordinal as i64,
        bbox_x: bb.x(),
        bbox_y: bb.y(),
        bbox_w: bb.width(),
        bbox_h: bb.height(),
        confidence: fl.confidence(),
      });
      for (rord, region) in fl.regions_slice().iter().enumerate() {
        out
          .face_landmark_regions
          .push(SqliteKeyframeFaceLandmarkRegionRow {
            keyframe_id: id.clone(),
            parent_ordinal: ordinal as i64,
            ordinal: rord as i64,
            name: region.name().to_owned(),
          });
        for (pord, point) in region.points().into_iter().enumerate() {
          out
            .face_landmark_points
            .push(SqliteKeyframeFaceLandmarkPointRow {
              keyframe_id: id.clone(),
              parent_ordinal: ordinal as i64,
              region_ordinal: rord as i64,
              ordinal: pord as i64,
              x: point.0,
              y: point.1,
            });
        }
      }
    }

    // VLM open-vocab fields → keyframe_vlm_label.
    push_vlm(&mut out.vlm_labels, &id, 0, vlm.categories_slice());
    push_vlm(&mut out.vlm_labels, &id, 1, vlm.tags_slice());
    push_vlm(&mut out.vlm_labels, &id, 2, vlm.objects_slice());
    push_vlm(&mut out.vlm_labels, &id, 3, vlm.subjects_slice());
    push_vlm(&mut out.vlm_labels, &id, 4, vlm.mood_slice());
    push_vlm(&mut out.vlm_labels, &id, 5, vlm.emotion_slice());
    push_vlm(&mut out.vlm_labels, &id, 6, vlm.lighting_slice());

    out
  }
}

/// Rebuild a [`Keyframe`] from a stored bundle of keyframe + detection
/// child rows. `parent_timebase` is the per-stream timebase carried on
/// the parent `video_track` row.
pub fn keyframe_from_rows(
  rows: SqliteKeyframeRows,
  parent_timebase: mediatime::Timebase,
) -> Result<Keyframe<Uuid7>, SqlxError> {
  {
    let row = rows
      .keyframe
      .ok_or_else(|| SqlxError::DomainConstructorRejected("Keyframe row is missing".to_owned()))?;
    let id = bytes_to_uuid7(&row.id)?;
    let scene_id = bytes_to_uuid7(&row.scene_id)?;
    let pts = mediatime::Timestamp::new(row.pts, parent_timebase);
    let dimensions = Dimensions::new(
      u32_from_i64(row.width, "Keyframe.width")?,
      u32_from_i64(row.height, "Keyframe.height")?,
    );
    let extractor = parse_keyframe_extractor(&row.extractor)?;
    let mut kf = Keyframe::try_new(id, scene_id, pts, dimensions, extractor)
      .map_err(|e: KeyframeError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_mime(row.mime)
      .with_data(Bytes::from(row.data))
      .with_aesthetics(Aesthetics::new(
        row.aesthetics_overall_score,
        bool_from_i64(row.aesthetics_is_utility),
      ))
      .with_horizon(
        HorizonInfo::try_new(row.horizon_angle, row.horizon_confidence).map_err(detection_err)?,
      );

    // classifications
    let mut classifications = sort_by_ordinal(rows.classifications, |r| r.ordinal);
    let mut built = Vec::with_capacity(classifications.len());
    for r in classifications.drain(..) {
      built.push(Detection::try_new(r.label, r.confidence).map_err(detection_err)?);
    }
    kf = kf.with_classifications(built);

    // objects
    let mut objects = sort_by_ordinal(rows.objects, |r| r.ordinal);
    let mut built = Vec::with_capacity(objects.len());
    for r in objects.drain(..) {
      let det = Detection::try_new(r.label, r.confidence).map_err(detection_err)?;
      let bbox = if bool_from_i64(r.has_bbox) {
        Some(
          BoundingBox::try_new(
            r.bbox_x.unwrap_or_default(),
            r.bbox_y.unwrap_or_default(),
            r.bbox_w.unwrap_or_default(),
            r.bbox_h.unwrap_or_default(),
          )
          .map_err(detection_err)?,
        )
      } else {
        None
      };
      built.push(ObjectDetection::new(det, bbox));
    }
    kf = kf.with_objects(built);

    // actions
    let mut actions = sort_by_ordinal(rows.actions, |r| r.ordinal);
    let mut built = Vec::with_capacity(actions.len());
    for r in actions.drain(..) {
      let det = Detection::try_new(r.label, r.confidence).map_err(detection_err)?;
      built.push(ActionDetection::new(det));
    }
    kf = kf.with_actions(built);

    // text_detections
    let mut texts = sort_by_ordinal(rows.text_detections, |r| r.ordinal);
    let mut built = Vec::with_capacity(texts.len());
    for r in texts.drain(..) {
      let bb =
        BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
      built.push(TextDetection::try_new(r.text, r.confidence, bb).map_err(detection_err)?);
    }
    kf = kf.with_text_detections(built);

    // barcodes
    let mut barcodes = sort_by_ordinal(rows.barcodes, |r| r.ordinal);
    let mut built = Vec::with_capacity(barcodes.len());
    for r in barcodes.drain(..) {
      let bb =
        BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
      built.push(
        BarcodeDetection::try_new(r.payload, r.symbology, r.confidence, bb)
          .map_err(detection_err)?,
      );
    }
    kf = kf.with_barcodes(built);

    // saliencies — split by kind (0 = attention, 1 = objectness).
    let mut attention = Vec::new();
    let mut objectness = Vec::new();
    let mut saliencies = rows.saliencies;
    saliencies.sort_by_key(|r| (r.kind, r.ordinal));
    for r in saliencies {
      let bb =
        BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
      let region = SaliencyRegion::try_new(bb, r.confidence).map_err(detection_err)?;
      match r.kind {
        0 => attention.push(region),
        1 => objectness.push(region),
        other => {
          return Err(SqlxError::UnknownDiscriminant(format!(
            "keyframe_saliency.kind: {other}"
          )))
        }
      }
    }
    kf = kf
      .with_attention_saliency(attention)
      .with_objectness_saliency(objectness);

    // document_segments
    let mut docs = sort_by_ordinal(rows.document_segments, |r| r.ordinal);
    let mut built = Vec::with_capacity(docs.len());
    for r in docs.drain(..) {
      built.push(
        DocumentSegment::try_new(
          (r.tl_x, r.tl_y),
          (r.tr_x, r.tr_y),
          (r.br_x, r.br_y),
          (r.bl_x, r.bl_y),
          r.confidence,
        )
        .map_err(detection_err)?,
      );
    }
    kf = kf.with_document_segments(built);

    // colors
    let mut colors = sort_by_ordinal(rows.colors, |r| r.ordinal);
    let mut built = Vec::with_capacity(colors.len());
    for r in colors.drain(..) {
      let rgb = Rgba::from_bits(u32_from_i64(r.rgba, "keyframe_color.rgba")?);
      let population = u32_from_i64(r.population, "keyframe_color.population")?;
      built.push(
        DominantColor::try_new(rgb, r.name, r.percentage, population).map_err(detection_err)?,
      );
    }
    kf = kf.with_colors(built);

    // humans + animals — split by scope.
    let (human_subjects, animal_subjects) = build_subjects(rows.subjects)?;
    let (human_faces, face_rectangles) = build_faces(rows.faces)?;
    let mut joints_by_scope = group_joints_by_scope(rows.body_pose_joints);
    let (human_body_poses, animal_body_poses) =
      build_body_poses(rows.body_poses, &mut joints_by_scope)?;
    let hand_joints = joints_by_scope.remove(&2i64).unwrap_or_default();
    let hand_poses = build_hand_poses(rows.hand_poses, hand_joints)?;
    let body_poses_3d = build_body_poses_3d(rows.body_poses_3d, rows.body_pose_3d_joints)?;
    let (instance_masks, segmentation_masks) = build_masks(rows.masks)?;
    let face_landmarks = build_face_landmarks(
      rows.face_landmarks,
      rows.face_landmark_regions,
      rows.face_landmark_points,
    )?;

    let humans = HumanAnalysis::new()
      .with_subjects(human_subjects)
      .with_faces(human_faces)
      .with_face_rectangles(face_rectangles)
      .with_body_poses(human_body_poses)
      .with_hand_poses(hand_poses)
      .with_body_poses_3d(body_poses_3d)
      .with_instance_masks(instance_masks)
      .with_face_landmarks(face_landmarks)
      .with_segmentation_masks(segmentation_masks);
    let animals = AnimalAnalysis::new()
      .with_subjects(animal_subjects)
      .with_body_poses(animal_body_poses);
    kf = kf.with_humans(humans).with_animals(animals);

    // VLM
    let mut vlm = VlmAnalysis::new()
      .with_description(LocalizedText::from_src_translated(
        row.vlm_description_src,
        row.vlm_description_translated,
      ))
      .with_shot_type(row.vlm_shot_type);
    let labels = group_vlm_by_kind(rows.vlm_labels);
    vlm = vlm
      .with_categories(labels.0)
      .with_tags(labels.1)
      .with_objects(labels.2)
      .with_subjects(labels.3)
      .with_mood(labels.4)
      .with_emotion(labels.5)
      .with_lighting(labels.6);
    kf = kf.with_vlm(vlm);

    Ok(kf)
  }
}

// ---------------------------------------------------------------------------
// Helpers — slug ↔ enum, ordinal sorting, group-by, primitive narrowing
// ---------------------------------------------------------------------------

fn parse_video_codec(s: &str) -> VideoCodec {
  s.parse::<VideoCodec>()
    .unwrap_or_else(|_| VideoCodec::Other(s.into()))
}

fn scene_detector_to_slug(d: SceneDetector) -> &'static str {
  match d {
    SceneDetector::Histogram => "histogram",
    SceneDetector::Phash => "phash",
    SceneDetector::Threshold => "threshold",
    SceneDetector::Content => "content",
    SceneDetector::Adaptive => "adaptive",
    SceneDetector::Manual => "manual",
  }
}

fn parse_scene_detector(s: &str) -> Result<SceneDetector, SqlxError> {
  Ok(match s {
    "histogram" => SceneDetector::Histogram,
    "phash" => SceneDetector::Phash,
    "threshold" => SceneDetector::Threshold,
    "content" => SceneDetector::Content,
    "adaptive" => SceneDetector::Adaptive,
    "manual" => SceneDetector::Manual,
    other => {
      return Err(SqlxError::UnknownDiscriminant(format!(
        "SceneDetector slug: {other}"
      )))
    }
  })
}

fn keyframe_extractor_to_slug(e: KeyframeExtractor) -> &'static str {
  match e {
    KeyframeExtractor::Histogram => "histogram",
    KeyframeExtractor::Phash => "phash",
    KeyframeExtractor::Threshold => "threshold",
    KeyframeExtractor::Content => "content",
    KeyframeExtractor::Adaptive => "adaptive",
    KeyframeExtractor::CompositeQuality => "composite-quality",
    KeyframeExtractor::Interval => "interval",
    KeyframeExtractor::IFrame => "i-frame",
    KeyframeExtractor::SceneRepresentative => "scene-representative",
    KeyframeExtractor::Manual => "manual",
  }
}

fn parse_keyframe_extractor(s: &str) -> Result<KeyframeExtractor, SqlxError> {
  Ok(match s {
    "histogram" => KeyframeExtractor::Histogram,
    "phash" => KeyframeExtractor::Phash,
    "threshold" => KeyframeExtractor::Threshold,
    "content" => KeyframeExtractor::Content,
    "adaptive" => KeyframeExtractor::Adaptive,
    "composite-quality" => KeyframeExtractor::CompositeQuality,
    "interval" => KeyframeExtractor::Interval,
    "i-frame" => KeyframeExtractor::IFrame,
    "scene-representative" => KeyframeExtractor::SceneRepresentative,
    "manual" => KeyframeExtractor::Manual,
    other => {
      return Err(SqlxError::UnknownDiscriminant(format!(
        "KeyframeExtractor slug: {other}"
      )))
    }
  })
}

fn hand_chirality_to_i16(c: HandChirality) -> i16 {
  match c {
    HandChirality::Unknown => 0,
    HandChirality::Left => 1,
    HandChirality::Right => 2,
  }
}

fn hand_chirality_from_i16(n: i16) -> Result<HandChirality, SqlxError> {
  match n {
    0 => Ok(HandChirality::Unknown),
    1 => Ok(HandChirality::Left),
    2 => Ok(HandChirality::Right),
    other => Err(SqlxError::UnknownDiscriminant(format!(
      "HandChirality: {other}"
    ))),
  }
}

fn height_estimation_to_i16(h: BodyPose3DHeightEstimation) -> i16 {
  match h {
    BodyPose3DHeightEstimation::Unknown => 0,
    BodyPose3DHeightEstimation::Reference => 1,
    BodyPose3DHeightEstimation::Measured => 2,
  }
}

fn height_estimation_from_i16(n: i16) -> Result<BodyPose3DHeightEstimation, SqlxError> {
  match n {
    0 => Ok(BodyPose3DHeightEstimation::Unknown),
    1 => Ok(BodyPose3DHeightEstimation::Reference),
    2 => Ok(BodyPose3DHeightEstimation::Measured),
    other => Err(SqlxError::UnknownDiscriminant(format!(
      "BodyPose3DHeightEstimation: {other}"
    ))),
  }
}

fn push_saliency(
  out: &mut Vec<SqliteKeyframeSaliencyRow>,
  keyframe_id: &[u8],
  kind: i64,
  ordinal: usize,
  s: &SaliencyRegion,
) {
  let bb = s.bbox_ref();
  out.push(SqliteKeyframeSaliencyRow {
    keyframe_id: keyframe_id.to_vec(),
    kind,
    ordinal: ordinal as i64,
    bbox_x: bb.x(),
    bbox_y: bb.y(),
    bbox_w: bb.width(),
    bbox_h: bb.height(),
    confidence: s.confidence(),
  });
}

fn push_subject(
  out: &mut Vec<SqliteKeyframeSubjectRow>,
  keyframe_id: &[u8],
  scope: i64,
  ordinal: usize,
  s: &SubjectDetection,
) {
  let bb = s.bbox_ref();
  out.push(SqliteKeyframeSubjectRow {
    keyframe_id: keyframe_id.to_vec(),
    scope,
    ordinal: ordinal as i64,
    label: s.detection_ref().label().to_owned(),
    confidence: s.detection_ref().confidence(),
    bbox_x: bb.x(),
    bbox_y: bb.y(),
    bbox_w: bb.width(),
    bbox_h: bb.height(),
  });
}

fn push_face(
  out: &mut Vec<SqliteKeyframeFaceRow>,
  keyframe_id: &[u8],
  kind: i64,
  ordinal: usize,
  f: &FaceDetection,
) {
  let bb = f.bbox_ref();
  out.push(SqliteKeyframeFaceRow {
    keyframe_id: keyframe_id.to_vec(),
    kind,
    ordinal: ordinal as i64,
    bbox_x: bb.x(),
    bbox_y: bb.y(),
    bbox_w: bb.width(),
    bbox_h: bb.height(),
    confidence: f.confidence(),
    capture_quality: f.capture_quality(),
    roll: f.roll(),
    yaw: f.yaw(),
    pitch: f.pitch(),
  });
}

fn push_body_pose(
  rows: &mut Vec<SqliteKeyframeBodyPoseRow>,
  joint_rows: &mut Vec<SqliteKeyframeBodyPoseJointRow>,
  keyframe_id: &[u8],
  scope: i64,
  ordinal: usize,
  bp: &BodyPoseDetection,
) {
  let bb = bp.bbox_ref();
  rows.push(SqliteKeyframeBodyPoseRow {
    keyframe_id: keyframe_id.to_vec(),
    scope,
    ordinal: ordinal as i64,
    bbox_x: bb.x(),
    bbox_y: bb.y(),
    bbox_w: bb.width(),
    bbox_h: bb.height(),
    confidence: bp.confidence(),
  });
  for (jord, j) in bp.joints_slice().iter().enumerate() {
    joint_rows.push(SqliteKeyframeBodyPoseJointRow {
      keyframe_id: keyframe_id.to_vec(),
      scope,
      parent_ordinal: ordinal as i64,
      ordinal: jord as i64,
      name: j.name().to_owned(),
      x: j.x(),
      y: j.y(),
      confidence: j.confidence(),
    });
  }
}

fn push_vlm(
  out: &mut Vec<SqliteKeyframeVlmLabelRow>,
  keyframe_id: &[u8],
  kind: i64,
  labels: &[LocalizedText],
) {
  for (ordinal, l) in labels.iter().enumerate() {
    out.push(SqliteKeyframeVlmLabelRow {
      keyframe_id: keyframe_id.to_vec(),
      kind,
      ordinal: ordinal as i64,
      src: l.src().to_owned(),
      translated: l.translated().to_owned(),
    });
  }
}

fn sort_by_ordinal<T, F>(mut v: Vec<T>, key: F) -> Vec<T>
where
  F: FnMut(&T) -> i64,
{
  let mut key = key;
  v.sort_by_key(|t| key(t));
  v
}

fn group_joints_by_scope(
  rows: Vec<SqliteKeyframeBodyPoseJointRow>,
) -> std::collections::HashMap<i64, Vec<SqliteKeyframeBodyPoseJointRow>> {
  let mut out: std::collections::HashMap<i64, Vec<SqliteKeyframeBodyPoseJointRow>> =
    std::collections::HashMap::new();
  for r in rows {
    out.entry(r.scope).or_default().push(r);
  }
  out
}

fn build_subjects(
  rows: Vec<SqliteKeyframeSubjectRow>,
) -> Result<(Vec<SubjectDetection>, Vec<SubjectDetection>), SqlxError> {
  let mut humans = Vec::new();
  let mut animals = Vec::new();
  let mut rows = rows;
  rows.sort_by_key(|r| (r.scope, r.ordinal));
  for r in rows {
    let det = Detection::try_new(r.label, r.confidence).map_err(detection_err)?;
    let bb = BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
    let subject = SubjectDetection::new(det, bb);
    match r.scope {
      0 => humans.push(subject),
      1 => animals.push(subject),
      other => {
        return Err(SqlxError::UnknownDiscriminant(format!(
          "keyframe_subject.scope: {other}"
        )))
      }
    }
  }
  Ok((humans, animals))
}

fn build_faces(
  rows: Vec<SqliteKeyframeFaceRow>,
) -> Result<(Vec<FaceDetection>, Vec<FaceDetection>), SqlxError> {
  let mut faces = Vec::new();
  let mut face_rects = Vec::new();
  let mut rows = rows;
  rows.sort_by_key(|r| (r.kind, r.ordinal));
  for r in rows {
    let bb = BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
    let face = FaceDetection::try_new(bb, r.confidence, r.capture_quality, r.roll, r.yaw, r.pitch)
      .map_err(detection_err)?;
    match r.kind {
      0 => faces.push(face),
      1 => face_rects.push(face),
      other => {
        return Err(SqlxError::UnknownDiscriminant(format!(
          "keyframe_face.kind: {other}"
        )))
      }
    }
  }
  Ok((faces, face_rects))
}

fn build_body_poses(
  rows: Vec<SqliteKeyframeBodyPoseRow>,
  joints_by_scope: &mut std::collections::HashMap<i64, Vec<SqliteKeyframeBodyPoseJointRow>>,
) -> Result<(Vec<BodyPoseDetection>, Vec<BodyPoseDetection>), SqlxError> {
  let mut humans = Vec::new();
  let mut animals = Vec::new();
  let mut rows = rows;
  rows.sort_by_key(|r| (r.scope, r.ordinal));

  // Build joint lookup per (scope, parent_ordinal).
  let human_joints = joints_by_scope.remove(&0i64).unwrap_or_default();
  let animal_joints = joints_by_scope.remove(&1i64).unwrap_or_default();
  let human_lookup = joints_lookup(human_joints);
  let animal_lookup = joints_lookup(animal_joints);

  for r in rows {
    let bb = BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
    let joints = match r.scope {
      0 => human_lookup.get(&r.ordinal).cloned().unwrap_or_default(),
      1 => animal_lookup.get(&r.ordinal).cloned().unwrap_or_default(),
      other => {
        return Err(SqlxError::UnknownDiscriminant(format!(
          "keyframe_body_pose.scope: {other}"
        )))
      }
    };
    let joints_built = build_joints(joints)?;
    let pose = BodyPoseDetection::try_new(bb, r.confidence, joints_built).map_err(detection_err)?;
    match r.scope {
      0 => humans.push(pose),
      1 => animals.push(pose),
      _ => unreachable!(),
    }
  }
  Ok((humans, animals))
}

fn joints_lookup(
  rows: Vec<SqliteKeyframeBodyPoseJointRow>,
) -> std::collections::HashMap<i64, Vec<SqliteKeyframeBodyPoseJointRow>> {
  let mut out: std::collections::HashMap<i64, Vec<SqliteKeyframeBodyPoseJointRow>> =
    std::collections::HashMap::new();
  for r in rows {
    out.entry(r.parent_ordinal).or_default().push(r);
  }
  for v in out.values_mut() {
    v.sort_by_key(|r| r.ordinal);
  }
  out
}

fn build_joints(
  rows: Vec<SqliteKeyframeBodyPoseJointRow>,
) -> Result<Vec<BodyPoseJoint>, SqlxError> {
  let mut out = Vec::with_capacity(rows.len());
  for r in rows {
    out.push(BodyPoseJoint::try_new(r.name, r.x, r.y, r.confidence).map_err(detection_err)?);
  }
  Ok(out)
}

fn build_hand_poses(
  rows: Vec<SqliteKeyframeHandPoseRow>,
  joints: Vec<SqliteKeyframeBodyPoseJointRow>,
) -> Result<Vec<HandPoseDetection>, SqlxError> {
  let joint_lookup = joints_lookup(joints);
  let mut rows = rows;
  rows.sort_by_key(|r| r.ordinal);
  let mut out = Vec::with_capacity(rows.len());
  for r in rows {
    let bb = BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
    let chirality = hand_chirality_from_i16(r.chirality as i16)?;
    let joints = joint_lookup.get(&r.ordinal).cloned().unwrap_or_default();
    let built = build_joints(joints)?;
    out
      .push(HandPoseDetection::try_new(bb, r.confidence, chirality, built).map_err(detection_err)?);
  }
  Ok(out)
}

fn build_body_poses_3d(
  rows: Vec<SqliteKeyframeBodyPose3DRow>,
  joints: Vec<SqliteKeyframeBodyPose3DJointRow>,
) -> Result<Vec<BodyPose3DDetection>, SqlxError> {
  let mut joint_lookup: std::collections::HashMap<i64, Vec<SqliteKeyframeBodyPose3DJointRow>> =
    std::collections::HashMap::new();
  for r in joints {
    joint_lookup.entry(r.parent_ordinal).or_default().push(r);
  }
  for v in joint_lookup.values_mut() {
    v.sort_by_key(|r| r.ordinal);
  }
  let mut rows = rows;
  rows.sort_by_key(|r| r.ordinal);
  let mut out = Vec::with_capacity(rows.len());
  for r in rows {
    let height = height_estimation_from_i16(r.height_estimation as i16)?;
    let joints = joint_lookup.remove(&r.ordinal).unwrap_or_default();
    let mut built = Vec::with_capacity(joints.len());
    for j in joints {
      built.push(
        BodyPose3DJoint::try_new(j.name, j.x, j.y, j.z, j.confidence).map_err(detection_err)?,
      );
    }
    out.push(
      BodyPose3DDetection::try_new(r.confidence, r.body_height, height, built)
        .map_err(detection_err)?,
    );
  }
  Ok(out)
}

fn build_masks(
  rows: Vec<SqliteKeyframeMaskRow>,
) -> Result<
  (
    Vec<PersonInstanceMaskDetection>,
    Vec<PersonSegmentationMask>,
  ),
  SqlxError,
> {
  let mut instance = Vec::new();
  let mut whole = Vec::new();
  let mut rows = rows;
  rows.sort_by_key(|r| (r.kind, r.ordinal));
  for r in rows {
    let bb = BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
    let dims = Dimensions::new(
      u32_from_i64(r.width, "keyframe_mask.width")?,
      u32_from_i64(r.height, "keyframe_mask.height")?,
    );
    match r.kind {
      0 => {
        let idx = r
          .instance_index
          .ok_or_else(|| {
            SqlxError::DomainConstructorRejected(
              "keyframe_mask.instance_index NULL for instance mask".to_owned(),
            )
          })
          .and_then(|v| u32_from_i64(v, "keyframe_mask.instance_index"))?;
        instance.push(
          PersonInstanceMaskDetection::try_new(bb, r.confidence, idx, dims, Bytes::from(r.data))
            .map_err(detection_err)?,
        );
      }
      1 => {
        whole.push(
          PersonSegmentationMask::try_new(bb, r.confidence, dims, Bytes::from(r.data))
            .map_err(detection_err)?,
        );
      }
      other => {
        return Err(SqlxError::UnknownDiscriminant(format!(
          "keyframe_mask.kind: {other}"
        )))
      }
    }
  }
  Ok((instance, whole))
}

fn build_face_landmarks(
  rows: Vec<SqliteKeyframeFaceLandmarksRow>,
  regions: Vec<SqliteKeyframeFaceLandmarkRegionRow>,
  points: Vec<SqliteKeyframeFaceLandmarkPointRow>,
) -> Result<Vec<FaceLandmarksDetection>, SqlxError> {
  // Bucket regions per face-landmark ordinal.
  let mut regions_by_parent: std::collections::HashMap<
    i64,
    Vec<SqliteKeyframeFaceLandmarkRegionRow>,
  > = std::collections::HashMap::new();
  for r in regions {
    regions_by_parent
      .entry(r.parent_ordinal)
      .or_default()
      .push(r);
  }
  for v in regions_by_parent.values_mut() {
    v.sort_by_key(|r| r.ordinal);
  }

  // Bucket points per (face-landmark ordinal, region ordinal).
  let mut points_by_region: std::collections::HashMap<
    (i64, i64),
    Vec<SqliteKeyframeFaceLandmarkPointRow>,
  > = std::collections::HashMap::new();
  for p in points {
    points_by_region
      .entry((p.parent_ordinal, p.region_ordinal))
      .or_default()
      .push(p);
  }
  for v in points_by_region.values_mut() {
    v.sort_by_key(|r| r.ordinal);
  }

  let mut rows = rows;
  rows.sort_by_key(|r| r.ordinal);
  let mut out = Vec::with_capacity(rows.len());
  for r in rows {
    let bb = BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
    let region_rows = regions_by_parent.remove(&r.ordinal).unwrap_or_default();
    let mut built_regions = Vec::with_capacity(region_rows.len());
    for region in region_rows {
      let pts = points_by_region
        .remove(&(r.ordinal, region.ordinal))
        .unwrap_or_default();
      let pt_iter: Vec<(f32, f32)> = pts.into_iter().map(|p| (p.x, p.y)).collect();
      built_regions.push(FaceLandmarkRegion::try_new(region.name, pt_iter).map_err(detection_err)?);
    }
    out.push(
      FaceLandmarksDetection::try_new(bb, r.confidence, built_regions).map_err(detection_err)?,
    );
  }
  Ok(out)
}

#[allow(clippy::type_complexity)]
fn group_vlm_by_kind(
  rows: Vec<SqliteKeyframeVlmLabelRow>,
) -> (
  Vec<LocalizedText>,
  Vec<LocalizedText>,
  Vec<LocalizedText>,
  Vec<LocalizedText>,
  Vec<LocalizedText>,
  Vec<LocalizedText>,
  Vec<LocalizedText>,
) {
  let mut buckets: [Vec<SqliteKeyframeVlmLabelRow>; 7] = Default::default();
  for r in rows {
    if (0..7).contains(&(r.kind as i32)) {
      buckets[r.kind as usize].push(r);
    }
  }
  let mut out: [Vec<LocalizedText>; 7] = Default::default();
  for (i, bucket) in buckets.iter_mut().enumerate() {
    bucket.sort_by_key(|r| r.ordinal);
    out[i] = bucket
      .drain(..)
      .map(|r| LocalizedText::from_src_translated(r.src, r.translated))
      .collect();
  }
  let [c, t, o, s, m, e, l] = out;
  (c, t, o, s, m, e, l)
}

fn detection_err<E: core::fmt::Display>(e: E) -> SqlxError {
  SqlxError::DomainConstructorRejected(format!("detection VO: {e}"))
}

fn track_err(e: VideoTrackError) -> SqlxError {
  SqlxError::DomainConstructorRejected(e.to_string())
}

fn u32_from_i64(v: i64, what: &str) -> Result<u32, SqlxError> {
  u32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
}

fn u32_from_i32(v: i64, what: &str) -> Result<u32, SqlxError> {
  u32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
}

fn u16_from_i32(v: i64, what: &str) -> Result<u16, SqlxError> {
  u16::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
}

fn u8_from_i16(v: i64, what: &str) -> Result<u8, SqlxError> {
  u8::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
}

fn bool_from_i64(v: i64) -> bool {
  v != 0
}

fn opt_u32(v: Option<i64>, what: &str) -> Result<Option<u32>, SqlxError> {
  match v {
    None => Ok(None),
    Some(x) => Ok(Some(u32_from_i64(x, what)?)),
  }
}

fn nonzero_u32_from_i64(v: i64, what: &str) -> Result<core::num::NonZeroU32, SqlxError> {
  let raw = u32_from_i64(v, what)?;
  core::num::NonZeroU32::new(raw)
    .ok_or_else(|| SqlxError::DomainConstructorRejected(format!("{what} must be non-zero")))
}

fn require_timebase(
  num: Option<i64>,
  den: Option<i64>,
  what: &str,
) -> Result<(i64, i64), SqlxError> {
  match (num, den) {
    (Some(n), Some(d)) => Ok((n, d)),
    _ => Err(SqlxError::DomainConstructorRejected(format!(
      "{what}: PTS present but timebase columns missing"
    ))),
  }
}

// ===========================================================================
// Borrowed-view siblings (`*RowRef<'r>`) — zero-copy decode from `&'r Row`.
//
// Every row type has a `Ref` sibling: Uuid identity columns ride as
// `Vec<u8>` in the owned rows, so even otherwise-`Copy` rows
// carry at least one variable-length field worth borrowing.
// ===========================================================================

/// Borrowed view of [`SqliteVideoRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteVideoRowRef<'r> {
  pub id: &'r [u8],
  pub media_id: &'r [u8],
  pub total_scenes: i64,
  pub track_progress_total: i64,
  pub track_progress_indexed: i64,
  pub track_progress_failed: i64,
}

impl SqliteVideoRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteVideoRowRef<'_> {
    SqliteVideoRowRef {
      id: &self.id,
      media_id: &self.media_id,
      total_scenes: self.total_scenes,
      track_progress_total: self.track_progress_total,
      track_progress_indexed: self.track_progress_indexed,
      track_progress_failed: self.track_progress_failed,
    }
  }
}

impl<'r> TryFrom<SqliteVideoRowRef<'r>> for Video<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteVideoRowRef<'r>) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
    let media_id = bytes_to_uuid7(r.media_id)?;
    let total_scenes = u32_from_i64(r.total_scenes, "Video.total_scenes")?;
    let progress = IndexProgress::from_parts(
      u32_from_i64(r.track_progress_total, "Video.track_progress_total")?,
      u32_from_i64(r.track_progress_indexed, "Video.track_progress_indexed")?,
      u32_from_i64(r.track_progress_failed, "Video.track_progress_failed")?,
    );
    let v = Video::try_new(id, media_id)
      .map_err(|e: VideoError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    Ok(
      v.with_total_scenes(total_scenes)
        .with_track_progress(progress),
    )
  }
}

/// Borrowed view of [`SqliteVideoTrackRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteVideoTrackRowRef<'r> {
  pub id: &'r [u8],
  pub video_id: &'r [u8],
  pub stream_index: Option<i64>,
  pub container_track_id: Option<i64>,
  pub start_pts: Option<i64>,
  pub start_pts_tb_num: Option<i64>,
  pub start_pts_tb_den: Option<i64>,
  pub duration_pts: Option<i64>,
  pub duration_tb_num: Option<i64>,
  pub duration_tb_den: Option<i64>,
  pub codec: &'r str,
  pub profile: Option<&'r str>,
  pub level: Option<i64>,
  pub bit_rate: i64,
  pub nb_frames: Option<i64>,
  pub has_b_frames: i64,
  pub closed_gop: Option<i64>,
  pub bits_per_raw_sample: Option<i64>,
  pub width: i64,
  pub height: i64,
  pub has_visible_rect: i64,
  pub visible_rect_x: Option<i64>,
  pub visible_rect_y: Option<i64>,
  pub visible_rect_w: Option<i64>,
  pub visible_rect_h: Option<i64>,
  pub sar_num: i64,
  pub sar_den: i64,
  pub pixel_format: i64,
  pub color_primaries: i64,
  pub color_transfer: i64,
  pub color_matrix: i64,
  pub color_range: i64,
  pub color_chroma_location: i64,
  pub has_hdr_static: i64,
  pub hdr_has_mastering: i64,
  pub hdr_primary_r_x: Option<i64>,
  pub hdr_primary_r_y: Option<i64>,
  pub hdr_primary_g_x: Option<i64>,
  pub hdr_primary_g_y: Option<i64>,
  pub hdr_primary_b_x: Option<i64>,
  pub hdr_primary_b_y: Option<i64>,
  pub hdr_white_point_x: Option<i64>,
  pub hdr_white_point_y: Option<i64>,
  pub hdr_max_luminance: Option<i64>,
  pub hdr_min_luminance: Option<i64>,
  pub hdr_has_content_light: i64,
  pub hdr_max_cll: Option<i64>,
  pub hdr_max_fall: Option<i64>,
  pub rotation: i64,
  pub fr_num: i64,
  pub fr_den: i64,
  pub fr_is_vfr: i64,
  pub avg_fr_num: i64,
  pub avg_fr_den: i64,
  pub field_order: i64,
  pub stereo_mode: Option<i64>,
  pub has_dovi: i64,
  pub dovi_profile: Option<i64>,
  pub dovi_level: Option<i64>,
  pub dovi_rpu_present: Option<i64>,
  pub dovi_el_present: Option<i64>,
  pub dovi_bl_signal_compat_id: Option<i64>,
  pub has_embedded_captions: i64,
  pub disposition: i64,
  pub is_primary: i64,
  pub auto_selected: i64,
  pub provenance_model_name: &'r str,
  pub provenance_model_version: &'r str,
  pub provenance_prompt_version: &'r str,
  pub provenance_indexer_version: &'r str,
  pub index_status: i64,
}

/// Borrowed view of [`SqliteVideoTrackIndexErrorRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteVideoTrackIndexErrorRowRef<'r> {
  pub video_track_id: &'r [u8],
  pub ordinal: i64,
  pub code: i64,
  pub message: &'r str,
}

/// Borrowed view of [`SqliteVideoTrackMetadataRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteVideoTrackMetadataRowRef<'r> {
  pub video_track_id: &'r [u8],
  pub ordinal: i64,
  pub key: &'r str,
  pub value: &'r str,
}

impl SqliteVideoTrackRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteVideoTrackRowRef<'_> {
    SqliteVideoTrackRowRef {
      id: &self.id,
      video_id: &self.video_id,
      stream_index: self.stream_index,
      container_track_id: self.container_track_id,
      start_pts: self.start_pts,
      start_pts_tb_num: self.start_pts_tb_num,
      start_pts_tb_den: self.start_pts_tb_den,
      duration_pts: self.duration_pts,
      duration_tb_num: self.duration_tb_num,
      duration_tb_den: self.duration_tb_den,
      codec: &self.codec,
      profile: self.profile.as_deref(),
      level: self.level,
      bit_rate: self.bit_rate,
      nb_frames: self.nb_frames,
      has_b_frames: self.has_b_frames,
      closed_gop: self.closed_gop,
      bits_per_raw_sample: self.bits_per_raw_sample,
      width: self.width,
      height: self.height,
      has_visible_rect: self.has_visible_rect,
      visible_rect_x: self.visible_rect_x,
      visible_rect_y: self.visible_rect_y,
      visible_rect_w: self.visible_rect_w,
      visible_rect_h: self.visible_rect_h,
      sar_num: self.sar_num,
      sar_den: self.sar_den,
      pixel_format: self.pixel_format,
      color_primaries: self.color_primaries,
      color_transfer: self.color_transfer,
      color_matrix: self.color_matrix,
      color_range: self.color_range,
      color_chroma_location: self.color_chroma_location,
      has_hdr_static: self.has_hdr_static,
      hdr_has_mastering: self.hdr_has_mastering,
      hdr_primary_r_x: self.hdr_primary_r_x,
      hdr_primary_r_y: self.hdr_primary_r_y,
      hdr_primary_g_x: self.hdr_primary_g_x,
      hdr_primary_g_y: self.hdr_primary_g_y,
      hdr_primary_b_x: self.hdr_primary_b_x,
      hdr_primary_b_y: self.hdr_primary_b_y,
      hdr_white_point_x: self.hdr_white_point_x,
      hdr_white_point_y: self.hdr_white_point_y,
      hdr_max_luminance: self.hdr_max_luminance,
      hdr_min_luminance: self.hdr_min_luminance,
      hdr_has_content_light: self.hdr_has_content_light,
      hdr_max_cll: self.hdr_max_cll,
      hdr_max_fall: self.hdr_max_fall,
      rotation: self.rotation,
      fr_num: self.fr_num,
      fr_den: self.fr_den,
      fr_is_vfr: self.fr_is_vfr,
      avg_fr_num: self.avg_fr_num,
      avg_fr_den: self.avg_fr_den,
      field_order: self.field_order,
      stereo_mode: self.stereo_mode,
      has_dovi: self.has_dovi,
      dovi_profile: self.dovi_profile,
      dovi_level: self.dovi_level,
      dovi_rpu_present: self.dovi_rpu_present,
      dovi_el_present: self.dovi_el_present,
      dovi_bl_signal_compat_id: self.dovi_bl_signal_compat_id,
      has_embedded_captions: self.has_embedded_captions,
      disposition: self.disposition,
      is_primary: self.is_primary,
      auto_selected: self.auto_selected,
      provenance_model_name: &self.provenance_model_name,
      provenance_model_version: &self.provenance_model_version,
      provenance_prompt_version: &self.provenance_prompt_version,
      provenance_indexer_version: &self.provenance_indexer_version,
      index_status: self.index_status,
    }
  }
}

impl SqliteVideoTrackIndexErrorRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteVideoTrackIndexErrorRowRef<'_> {
    SqliteVideoTrackIndexErrorRowRef {
      video_track_id: &self.video_track_id,
      ordinal: self.ordinal,
      code: self.code,
      message: &self.message,
    }
  }
}

impl SqliteVideoTrackMetadataRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteVideoTrackMetadataRowRef<'_> {
    SqliteVideoTrackMetadataRowRef {
      video_track_id: &self.video_track_id,
      ordinal: self.ordinal,
      key: &self.key,
      value: &self.value,
    }
  }
}

impl<'r>
  TryFrom<(
    SqliteVideoTrackRowRef<'r>,
    Vec<SqliteVideoTrackIndexErrorRowRef<'r>>,
    Vec<SqliteVideoTrackMetadataRowRef<'r>>,
  )> for VideoTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, mut errors, mut metadata): (
      SqliteVideoTrackRowRef<'r>,
      Vec<SqliteVideoTrackIndexErrorRowRef<'r>>,
      Vec<SqliteVideoTrackMetadataRowRef<'r>>,
    ),
  ) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
    let video_id = bytes_to_uuid7(r.video_id)?;
    let mut t = VideoTrack::try_new(id, video_id)
      .map_err(|e: VideoTrackError| SqlxError::DomainConstructorRejected(e.to_string()))?;

    t = t
      .with_stream_index(opt_u32(r.stream_index, "VideoTrack.stream_index")?)
      .with_container_track_id(r.container_track_id.map(|v| v as u64));

    if let Some(pts) = r.start_pts {
      let (num, den) = require_timebase(
        r.start_pts_tb_num,
        r.start_pts_tb_den,
        "VideoTrack.start_pts",
      )?;
      t = t.with_start_pts(Some(timestamp_from_parts(pts, num, den)?));
    }
    if let Some(pts) = r.duration_pts {
      let (num, den) =
        require_timebase(r.duration_tb_num, r.duration_tb_den, "VideoTrack.duration")?;
      t = t
        .try_with_duration(Some(timestamp_from_parts(pts, num, den)?))
        .map_err(track_err)?;
    }

    t = t.with_codec(parse_video_codec(r.codec));
    if let Some(p) = r.profile {
      t = t.with_profile(Some(SmolStr::from(p)));
    }
    if let Some(level) = r.level {
      t = t.with_level(Some(u16_from_i32(level, "VideoTrack.level")?));
    }

    t = t
      .with_bit_rate(r.bit_rate as u64)
      .with_nb_frames(r.nb_frames.map(|v| v as u64))
      .with_has_b_frames(bool_from_i64(r.has_b_frames))
      .with_closed_gop(r.closed_gop.map(bool_from_i64));
    if let Some(b) = r.bits_per_raw_sample {
      t = t.with_bits_per_raw_sample(Some(u8_from_i16(b, "VideoTrack.bits_per_raw_sample")?));
    }

    let dims = Dimensions::new(
      u32_from_i64(r.width, "VideoTrack.width")?,
      u32_from_i64(r.height, "VideoTrack.height")?,
    );
    t = t.try_with_dimensions(dims).map_err(track_err)?;
    if bool_from_i64(r.has_visible_rect) {
      let rect = Rect::new(
        u32_from_i64(
          r.visible_rect_x.unwrap_or_default(),
          "VideoTrack.visible_rect_x",
        )?,
        u32_from_i64(
          r.visible_rect_y.unwrap_or_default(),
          "VideoTrack.visible_rect_y",
        )?,
        u32_from_i64(
          r.visible_rect_w.unwrap_or_default(),
          "VideoTrack.visible_rect_w",
        )?,
        u32_from_i64(
          r.visible_rect_h.unwrap_or_default(),
          "VideoTrack.visible_rect_h",
        )?,
      );
      t = t.try_with_visible_rect(Some(rect)).map_err(track_err)?;
    }
    t = t.with_sample_aspect_ratio(SampleAspectRatio::new(
      u32_from_i64(r.sar_num, "VideoTrack.sar_num")?,
      nonzero_u32_from_i64(r.sar_den, "VideoTrack.sar_den")?,
    ));
    t = t.with_pixel_format(PixelFormat::from_u32(u32_from_i64(
      r.pixel_format,
      "VideoTrack.pixel_format",
    )?));

    let color = ColorInfo::new(
      Primaries::from_u32(u32_from_i64(
        r.color_primaries,
        "VideoTrack.color_primaries",
      )?),
      Transfer::from_u32(u32_from_i64(r.color_transfer, "VideoTrack.color_transfer")?),
      Matrix::from_u32(u32_from_i64(r.color_matrix, "VideoTrack.color_matrix")?),
      DynamicRange::from_u32(u32_from_i64(r.color_range, "VideoTrack.color_range")?),
      ChromaLocation::from_u32(u32_from_i64(
        r.color_chroma_location,
        "VideoTrack.color_chroma_location",
      )?),
    );
    t = t.with_color(color);

    if bool_from_i64(r.has_hdr_static) {
      let mastering = if bool_from_i64(r.hdr_has_mastering) {
        Some(MasteringDisplay::new(
          [
            ChromaCoord::new(
              u32_from_i64(
                r.hdr_primary_r_x.unwrap_or_default(),
                "VideoTrack.hdr_primary_r_x",
              )?,
              u32_from_i64(
                r.hdr_primary_r_y.unwrap_or_default(),
                "VideoTrack.hdr_primary_r_y",
              )?,
            ),
            ChromaCoord::new(
              u32_from_i64(
                r.hdr_primary_g_x.unwrap_or_default(),
                "VideoTrack.hdr_primary_g_x",
              )?,
              u32_from_i64(
                r.hdr_primary_g_y.unwrap_or_default(),
                "VideoTrack.hdr_primary_g_y",
              )?,
            ),
            ChromaCoord::new(
              u32_from_i64(
                r.hdr_primary_b_x.unwrap_or_default(),
                "VideoTrack.hdr_primary_b_x",
              )?,
              u32_from_i64(
                r.hdr_primary_b_y.unwrap_or_default(),
                "VideoTrack.hdr_primary_b_y",
              )?,
            ),
          ],
          ChromaCoord::new(
            u32_from_i64(
              r.hdr_white_point_x.unwrap_or_default(),
              "VideoTrack.hdr_white_point_x",
            )?,
            u32_from_i64(
              r.hdr_white_point_y.unwrap_or_default(),
              "VideoTrack.hdr_white_point_y",
            )?,
          ),
          u32_from_i64(
            r.hdr_max_luminance.unwrap_or_default(),
            "VideoTrack.hdr_max_luminance",
          )?,
          u32_from_i64(
            r.hdr_min_luminance.unwrap_or_default(),
            "VideoTrack.hdr_min_luminance",
          )?,
        ))
      } else {
        None
      };
      let content_light = if bool_from_i64(r.hdr_has_content_light) {
        Some(ContentLightLevel::new(
          u32_from_i64(r.hdr_max_cll.unwrap_or_default(), "VideoTrack.hdr_max_cll")?,
          u32_from_i64(
            r.hdr_max_fall.unwrap_or_default(),
            "VideoTrack.hdr_max_fall",
          )?,
        ))
      } else {
        None
      };
      t = t.with_hdr_static(Some(HdrStaticMetadata::new(mastering, content_light)));
    }

    t = t.with_rotation(Rotation::from_u32(u32_from_i64(
      r.rotation,
      "VideoTrack.rotation",
    )?));
    t = t.with_frame_rate(FrameRate::new(
      Rational::new(
        u32_from_i64(r.fr_num, "VideoTrack.fr_num")?,
        nonzero_u32_from_i64(r.fr_den, "VideoTrack.fr_den")?,
      ),
      bool_from_i64(r.fr_is_vfr),
    ));
    t = t.with_avg_frame_rate(FrameRate::new(
      Rational::new(
        u32_from_i64(r.avg_fr_num, "VideoTrack.avg_fr_num")?,
        nonzero_u32_from_i64(r.avg_fr_den, "VideoTrack.avg_fr_den")?,
      ),
      false,
    ));
    t = t.with_field_order(FieldOrder::from_u32(u32_from_i64(
      r.field_order,
      "VideoTrack.field_order",
    )?));
    if let Some(sm) = r.stereo_mode {
      t = t.with_stereo_mode(Some(StereoMode::from_u32(u32_from_i64(
        sm,
        "VideoTrack.stereo_mode",
      )?)));
    }

    if bool_from_i64(r.has_dovi) {
      t = t.with_dovi(Some(DolbyVisionConfig::new(
        u8_from_i16(
          r.dovi_profile.unwrap_or_default(),
          "VideoTrack.dovi_profile",
        )?,
        u8_from_i16(r.dovi_level.unwrap_or_default(), "VideoTrack.dovi_level")?,
        bool_from_i64(r.dovi_rpu_present.unwrap_or_default()),
        bool_from_i64(r.dovi_el_present.unwrap_or_default()),
        u8_from_i16(
          r.dovi_bl_signal_compat_id.unwrap_or_default(),
          "VideoTrack.dovi_bl_signal_compat_id",
        )?,
      )));
    }

    t = t
      .with_has_embedded_captions(bool_from_i64(r.has_embedded_captions))
      .with_disposition(TrackDisposition::from_bits_truncate(u32_from_i64(
        r.disposition,
        "VideoTrack.disposition",
      )?))
      .with_is_primary(bool_from_i64(r.is_primary))
      .with_auto_selected(bool_from_i64(r.auto_selected))
      .with_provenance(Provenance::from_parts(
        r.provenance_model_name,
        r.provenance_model_version,
        r.provenance_prompt_version,
        r.provenance_indexer_version,
      ))
      .with_index_status(VideoIndexStatus::from_bits_truncate(u32_from_i64(
        r.index_status,
        "VideoTrack.index_status",
      )?));

    errors.sort_by_key(|e| e.ordinal);
    let mut infos = Vec::with_capacity(errors.len());
    for e in errors {
      let code = u32_from_i32(e.code, "VideoTrack.index_error.code")?;
      infos.push(ErrorInfo::new(ErrorCode::from_u32(code), e.message));
    }
    t = t.with_index_errors(infos);

    metadata.sort_by_key(|m| m.ordinal);
    let mut bag = IndexMap::with_capacity(metadata.len());
    for entry in metadata {
      if entry.video_track_id != r.id {
        return Err(SqlxError::DomainConstructorRejected(
          "video_track_metadata.video_track_id does not match parent video_track.id".to_owned(),
        ));
      }
      bag.insert(SmolStr::from(entry.key), SmolStr::from(entry.value));
    }
    t = t.with_metadata(bag);

    Ok(t)
  }
}

/// Borrowed view of [`SqliteSceneRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSceneRowRef<'r> {
  pub id: &'r [u8],
  pub video_track_id: &'r [u8],
  pub index: i64,
  pub span_start_pts: i64,
  pub span_end_pts: i64,
  pub detector: &'r str,
  pub description: &'r str,
}

impl SqliteSceneRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteSceneRowRef<'_> {
    SqliteSceneRowRef {
      id: &self.id,
      video_track_id: &self.video_track_id,
      index: self.index,
      span_start_pts: self.span_start_pts,
      span_end_pts: self.span_end_pts,
      detector: &self.detector,
      description: &self.description,
    }
  }
}

/// Rebuild a [`Scene`] from a borrowed row — see [`scene_from_row`] for
/// the owned counterpart. `parent_timebase` is the per-stream timebase
/// carried on the parent `video_track` row.
pub fn scene_from_row_ref<'r>(
  r: SqliteSceneRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<Scene<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(r.id)?;
  let video_track_id = bytes_to_uuid7(r.video_track_id)?;
  let index = u32_from_i64(r.index, "Scene.index")?;
  let span = mediatime::TimeRange::try_new(r.span_start_pts, r.span_end_pts, parent_timebase)
    .ok_or_else(|| {
      SqlxError::DomainConstructorRejected(format!(
        "TimeRange start_pts ({}) must be <= end_pts ({})",
        r.span_start_pts, r.span_end_pts
      ))
    })?;
  let detector = parse_scene_detector(r.detector)?;
  let s = Scene::try_new(id, video_track_id, index, span, detector)
    .map_err(|e: SceneError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_description(r.description);
  Ok(s)
}

/// Borrowed view of [`SqliteKeyframeRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeRowRef<'r> {
  pub id: &'r [u8],
  pub scene_id: &'r [u8],
  pub pts: i64,
  pub data: &'r [u8],
  pub mime: &'r str,
  pub width: i64,
  pub height: i64,
  pub extractor: &'r str,
  pub vlm_description_src: &'r str,
  pub vlm_description_translated: &'r str,
  pub vlm_shot_type: &'r str,
  pub horizon_angle: f32,
  pub horizon_confidence: f32,
  pub aesthetics_overall_score: f32,
  pub aesthetics_is_utility: i64,
}

/// Borrowed view of [`SqliteKeyframeClassificationRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeClassificationRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub ordinal: i64,
  pub label: &'r str,
  pub confidence: f32,
}

/// Borrowed view of [`SqliteKeyframeObjectRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeObjectRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub ordinal: i64,
  pub label: &'r str,
  pub confidence: f32,
  pub has_bbox: i64,
  pub bbox_x: Option<f32>,
  pub bbox_y: Option<f32>,
  pub bbox_w: Option<f32>,
  pub bbox_h: Option<f32>,
}

/// Borrowed view of [`SqliteKeyframeActionRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeActionRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub ordinal: i64,
  pub label: &'r str,
  pub confidence: f32,
}

/// Borrowed view of [`SqliteKeyframeTextDetectionRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeTextDetectionRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub ordinal: i64,
  pub text: &'r str,
  pub confidence: f32,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
}

/// Borrowed view of [`SqliteKeyframeBarcodeRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeBarcodeRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub ordinal: i64,
  pub payload: &'r str,
  pub symbology: &'r str,
  pub confidence: f32,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
}

/// Borrowed view of [`SqliteKeyframeSaliencyRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeSaliencyRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub kind: i64,
  pub ordinal: i64,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
  pub confidence: f32,
}

/// Borrowed view of [`SqliteKeyframeDocumentSegmentRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeDocumentSegmentRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub ordinal: i64,
  pub tl_x: f32,
  pub tl_y: f32,
  pub tr_x: f32,
  pub tr_y: f32,
  pub br_x: f32,
  pub br_y: f32,
  pub bl_x: f32,
  pub bl_y: f32,
  pub confidence: f32,
}

/// Borrowed view of [`SqliteKeyframeColorRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeColorRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub ordinal: i64,
  pub rgba: i64,
  pub name: &'r str,
  pub percentage: f32,
  pub population: i64,
}

/// Borrowed view of [`SqliteKeyframeSubjectRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeSubjectRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub scope: i64,
  pub ordinal: i64,
  pub label: &'r str,
  pub confidence: f32,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
}

/// Borrowed view of [`SqliteKeyframeFaceRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeFaceRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub kind: i64,
  pub ordinal: i64,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
  pub confidence: f32,
  pub capture_quality: f32,
  pub roll: f32,
  pub yaw: f32,
  pub pitch: f32,
}

/// Borrowed view of [`SqliteKeyframeBodyPoseRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeBodyPoseRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub scope: i64,
  pub ordinal: i64,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
  pub confidence: f32,
}

/// Borrowed view of [`SqliteKeyframeBodyPoseJointRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeBodyPoseJointRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub scope: i64,
  pub parent_ordinal: i64,
  pub ordinal: i64,
  pub name: &'r str,
  pub x: f32,
  pub y: f32,
  pub confidence: f32,
}

/// Borrowed view of [`SqliteKeyframeHandPoseRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeHandPoseRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub ordinal: i64,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
  pub confidence: f32,
  pub chirality: i64,
}

/// Borrowed view of [`SqliteKeyframeBodyPose3DRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeBodyPose3DRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub ordinal: i64,
  pub confidence: f32,
  pub body_height: f32,
  pub height_estimation: i64,
}

/// Borrowed view of [`SqliteKeyframeBodyPose3DJointRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeBodyPose3DJointRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub parent_ordinal: i64,
  pub ordinal: i64,
  pub name: &'r str,
  pub x: f32,
  pub y: f32,
  pub z: f32,
  pub confidence: f32,
}

/// Borrowed view of [`SqliteKeyframeMaskRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeMaskRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub kind: i64,
  pub ordinal: i64,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
  pub confidence: f32,
  pub instance_index: Option<i64>,
  pub width: i64,
  pub height: i64,
  pub data: &'r [u8],
}

/// Borrowed view of [`SqliteKeyframeFaceLandmarksRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeFaceLandmarksRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub ordinal: i64,
  pub bbox_x: f32,
  pub bbox_y: f32,
  pub bbox_w: f32,
  pub bbox_h: f32,
  pub confidence: f32,
}

/// Borrowed view of [`SqliteKeyframeFaceLandmarkRegionRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteKeyframeFaceLandmarkRegionRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub parent_ordinal: i64,
  pub ordinal: i64,
  pub name: &'r str,
}

/// Borrowed view of [`SqliteKeyframeFaceLandmarkPointRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteKeyframeFaceLandmarkPointRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub parent_ordinal: i64,
  pub region_ordinal: i64,
  pub ordinal: i64,
  pub x: f32,
  pub y: f32,
}

/// Borrowed view of [`SqliteKeyframeVlmLabelRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteKeyframeVlmLabelRowRef<'r> {
  pub keyframe_id: &'r [u8],
  pub kind: i64,
  pub ordinal: i64,
  pub src: &'r str,
  pub translated: &'r str,
}

impl SqliteKeyframeRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeRowRef<'_> {
    SqliteKeyframeRowRef {
      id: &self.id,
      scene_id: &self.scene_id,
      pts: self.pts,
      data: &self.data,
      mime: &self.mime,
      width: self.width,
      height: self.height,
      extractor: &self.extractor,
      vlm_description_src: &self.vlm_description_src,
      vlm_description_translated: &self.vlm_description_translated,
      vlm_shot_type: &self.vlm_shot_type,
      horizon_angle: self.horizon_angle,
      horizon_confidence: self.horizon_confidence,
      aesthetics_overall_score: self.aesthetics_overall_score,
      aesthetics_is_utility: self.aesthetics_is_utility,
    }
  }
}

impl SqliteKeyframeClassificationRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeClassificationRowRef<'_> {
    SqliteKeyframeClassificationRowRef {
      keyframe_id: &self.keyframe_id,
      ordinal: self.ordinal,
      label: &self.label,
      confidence: self.confidence,
    }
  }
}

impl SqliteKeyframeObjectRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeObjectRowRef<'_> {
    SqliteKeyframeObjectRowRef {
      keyframe_id: &self.keyframe_id,
      ordinal: self.ordinal,
      label: &self.label,
      confidence: self.confidence,
      has_bbox: self.has_bbox,
      bbox_x: self.bbox_x,
      bbox_y: self.bbox_y,
      bbox_w: self.bbox_w,
      bbox_h: self.bbox_h,
    }
  }
}

impl SqliteKeyframeActionRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeActionRowRef<'_> {
    SqliteKeyframeActionRowRef {
      keyframe_id: &self.keyframe_id,
      ordinal: self.ordinal,
      label: &self.label,
      confidence: self.confidence,
    }
  }
}

impl SqliteKeyframeTextDetectionRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeTextDetectionRowRef<'_> {
    SqliteKeyframeTextDetectionRowRef {
      keyframe_id: &self.keyframe_id,
      ordinal: self.ordinal,
      text: &self.text,
      confidence: self.confidence,
      bbox_x: self.bbox_x,
      bbox_y: self.bbox_y,
      bbox_w: self.bbox_w,
      bbox_h: self.bbox_h,
    }
  }
}

impl SqliteKeyframeBarcodeRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeBarcodeRowRef<'_> {
    SqliteKeyframeBarcodeRowRef {
      keyframe_id: &self.keyframe_id,
      ordinal: self.ordinal,
      payload: &self.payload,
      symbology: &self.symbology,
      confidence: self.confidence,
      bbox_x: self.bbox_x,
      bbox_y: self.bbox_y,
      bbox_w: self.bbox_w,
      bbox_h: self.bbox_h,
    }
  }
}

impl SqliteKeyframeSaliencyRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeSaliencyRowRef<'_> {
    SqliteKeyframeSaliencyRowRef {
      keyframe_id: &self.keyframe_id,
      kind: self.kind,
      ordinal: self.ordinal,
      bbox_x: self.bbox_x,
      bbox_y: self.bbox_y,
      bbox_w: self.bbox_w,
      bbox_h: self.bbox_h,
      confidence: self.confidence,
    }
  }
}

impl SqliteKeyframeDocumentSegmentRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeDocumentSegmentRowRef<'_> {
    SqliteKeyframeDocumentSegmentRowRef {
      keyframe_id: &self.keyframe_id,
      ordinal: self.ordinal,
      tl_x: self.tl_x,
      tl_y: self.tl_y,
      tr_x: self.tr_x,
      tr_y: self.tr_y,
      br_x: self.br_x,
      br_y: self.br_y,
      bl_x: self.bl_x,
      bl_y: self.bl_y,
      confidence: self.confidence,
    }
  }
}

impl SqliteKeyframeColorRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeColorRowRef<'_> {
    SqliteKeyframeColorRowRef {
      keyframe_id: &self.keyframe_id,
      ordinal: self.ordinal,
      rgba: self.rgba,
      name: &self.name,
      percentage: self.percentage,
      population: self.population,
    }
  }
}

impl SqliteKeyframeSubjectRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeSubjectRowRef<'_> {
    SqliteKeyframeSubjectRowRef {
      keyframe_id: &self.keyframe_id,
      scope: self.scope,
      ordinal: self.ordinal,
      label: &self.label,
      confidence: self.confidence,
      bbox_x: self.bbox_x,
      bbox_y: self.bbox_y,
      bbox_w: self.bbox_w,
      bbox_h: self.bbox_h,
    }
  }
}

impl SqliteKeyframeFaceRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeFaceRowRef<'_> {
    SqliteKeyframeFaceRowRef {
      keyframe_id: &self.keyframe_id,
      kind: self.kind,
      ordinal: self.ordinal,
      bbox_x: self.bbox_x,
      bbox_y: self.bbox_y,
      bbox_w: self.bbox_w,
      bbox_h: self.bbox_h,
      confidence: self.confidence,
      capture_quality: self.capture_quality,
      roll: self.roll,
      yaw: self.yaw,
      pitch: self.pitch,
    }
  }
}

impl SqliteKeyframeBodyPoseRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeBodyPoseRowRef<'_> {
    SqliteKeyframeBodyPoseRowRef {
      keyframe_id: &self.keyframe_id,
      scope: self.scope,
      ordinal: self.ordinal,
      bbox_x: self.bbox_x,
      bbox_y: self.bbox_y,
      bbox_w: self.bbox_w,
      bbox_h: self.bbox_h,
      confidence: self.confidence,
    }
  }
}

impl SqliteKeyframeBodyPoseJointRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeBodyPoseJointRowRef<'_> {
    SqliteKeyframeBodyPoseJointRowRef {
      keyframe_id: &self.keyframe_id,
      scope: self.scope,
      parent_ordinal: self.parent_ordinal,
      ordinal: self.ordinal,
      name: &self.name,
      x: self.x,
      y: self.y,
      confidence: self.confidence,
    }
  }
}

impl SqliteKeyframeHandPoseRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeHandPoseRowRef<'_> {
    SqliteKeyframeHandPoseRowRef {
      keyframe_id: &self.keyframe_id,
      ordinal: self.ordinal,
      bbox_x: self.bbox_x,
      bbox_y: self.bbox_y,
      bbox_w: self.bbox_w,
      bbox_h: self.bbox_h,
      confidence: self.confidence,
      chirality: self.chirality,
    }
  }
}

impl SqliteKeyframeBodyPose3DRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeBodyPose3DRowRef<'_> {
    SqliteKeyframeBodyPose3DRowRef {
      keyframe_id: &self.keyframe_id,
      ordinal: self.ordinal,
      confidence: self.confidence,
      body_height: self.body_height,
      height_estimation: self.height_estimation,
    }
  }
}

impl SqliteKeyframeBodyPose3DJointRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeBodyPose3DJointRowRef<'_> {
    SqliteKeyframeBodyPose3DJointRowRef {
      keyframe_id: &self.keyframe_id,
      parent_ordinal: self.parent_ordinal,
      ordinal: self.ordinal,
      name: &self.name,
      x: self.x,
      y: self.y,
      z: self.z,
      confidence: self.confidence,
    }
  }
}

impl SqliteKeyframeMaskRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeMaskRowRef<'_> {
    SqliteKeyframeMaskRowRef {
      keyframe_id: &self.keyframe_id,
      kind: self.kind,
      ordinal: self.ordinal,
      bbox_x: self.bbox_x,
      bbox_y: self.bbox_y,
      bbox_w: self.bbox_w,
      bbox_h: self.bbox_h,
      confidence: self.confidence,
      instance_index: self.instance_index,
      width: self.width,
      height: self.height,
      data: &self.data,
    }
  }
}

impl SqliteKeyframeFaceLandmarksRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeFaceLandmarksRowRef<'_> {
    SqliteKeyframeFaceLandmarksRowRef {
      keyframe_id: &self.keyframe_id,
      ordinal: self.ordinal,
      bbox_x: self.bbox_x,
      bbox_y: self.bbox_y,
      bbox_w: self.bbox_w,
      bbox_h: self.bbox_h,
      confidence: self.confidence,
    }
  }
}

impl SqliteKeyframeFaceLandmarkRegionRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeFaceLandmarkRegionRowRef<'_> {
    SqliteKeyframeFaceLandmarkRegionRowRef {
      keyframe_id: &self.keyframe_id,
      parent_ordinal: self.parent_ordinal,
      ordinal: self.ordinal,
      name: &self.name,
    }
  }
}

impl SqliteKeyframeFaceLandmarkPointRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeFaceLandmarkPointRowRef<'_> {
    SqliteKeyframeFaceLandmarkPointRowRef {
      keyframe_id: &self.keyframe_id,
      parent_ordinal: self.parent_ordinal,
      region_ordinal: self.region_ordinal,
      ordinal: self.ordinal,
      x: self.x,
      y: self.y,
    }
  }
}

impl SqliteKeyframeVlmLabelRow {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeVlmLabelRowRef<'_> {
    SqliteKeyframeVlmLabelRowRef {
      keyframe_id: &self.keyframe_id,
      kind: self.kind,
      ordinal: self.ordinal,
      src: &self.src,
      translated: &self.translated,
    }
  }
}

/// Borrowed bundle for a single [`Keyframe`] — mirrors [`SqliteKeyframeRows`]
/// with every child slice as `Ref`s.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SqliteKeyframeRowsRef<'r> {
  pub keyframe: Option<SqliteKeyframeRowRef<'r>>,
  pub classifications: Vec<SqliteKeyframeClassificationRowRef<'r>>,
  pub objects: Vec<SqliteKeyframeObjectRowRef<'r>>,
  pub actions: Vec<SqliteKeyframeActionRowRef<'r>>,
  pub text_detections: Vec<SqliteKeyframeTextDetectionRowRef<'r>>,
  pub barcodes: Vec<SqliteKeyframeBarcodeRowRef<'r>>,
  pub saliencies: Vec<SqliteKeyframeSaliencyRowRef<'r>>,
  pub document_segments: Vec<SqliteKeyframeDocumentSegmentRowRef<'r>>,
  pub colors: Vec<SqliteKeyframeColorRowRef<'r>>,
  pub subjects: Vec<SqliteKeyframeSubjectRowRef<'r>>,
  pub faces: Vec<SqliteKeyframeFaceRowRef<'r>>,
  pub body_poses: Vec<SqliteKeyframeBodyPoseRowRef<'r>>,
  pub body_pose_joints: Vec<SqliteKeyframeBodyPoseJointRowRef<'r>>,
  pub hand_poses: Vec<SqliteKeyframeHandPoseRowRef<'r>>,
  pub body_poses_3d: Vec<SqliteKeyframeBodyPose3DRowRef<'r>>,
  pub body_pose_3d_joints: Vec<SqliteKeyframeBodyPose3DJointRowRef<'r>>,
  pub masks: Vec<SqliteKeyframeMaskRowRef<'r>>,
  pub face_landmarks: Vec<SqliteKeyframeFaceLandmarksRowRef<'r>>,
  pub face_landmark_regions: Vec<SqliteKeyframeFaceLandmarkRegionRowRef<'r>>,
  pub face_landmark_points: Vec<SqliteKeyframeFaceLandmarkPointRowRef<'r>>,
  pub vlm_labels: Vec<SqliteKeyframeVlmLabelRowRef<'r>>,
}

impl SqliteKeyframeRows {
  /// Cheap borrow.
  pub fn as_ref(&self) -> SqliteKeyframeRowsRef<'_> {
    SqliteKeyframeRowsRef {
      keyframe: self.keyframe.as_ref().map(SqliteKeyframeRow::as_ref),
      classifications: self
        .classifications
        .iter()
        .map(SqliteKeyframeClassificationRow::as_ref)
        .collect(),
      objects: self
        .objects
        .iter()
        .map(SqliteKeyframeObjectRow::as_ref)
        .collect(),
      actions: self
        .actions
        .iter()
        .map(SqliteKeyframeActionRow::as_ref)
        .collect(),
      text_detections: self
        .text_detections
        .iter()
        .map(SqliteKeyframeTextDetectionRow::as_ref)
        .collect(),
      barcodes: self
        .barcodes
        .iter()
        .map(SqliteKeyframeBarcodeRow::as_ref)
        .collect(),
      saliencies: self
        .saliencies
        .iter()
        .map(SqliteKeyframeSaliencyRow::as_ref)
        .collect(),
      document_segments: self
        .document_segments
        .iter()
        .map(SqliteKeyframeDocumentSegmentRow::as_ref)
        .collect(),
      colors: self
        .colors
        .iter()
        .map(SqliteKeyframeColorRow::as_ref)
        .collect(),
      subjects: self
        .subjects
        .iter()
        .map(SqliteKeyframeSubjectRow::as_ref)
        .collect(),
      faces: self
        .faces
        .iter()
        .map(SqliteKeyframeFaceRow::as_ref)
        .collect(),
      body_poses: self
        .body_poses
        .iter()
        .map(SqliteKeyframeBodyPoseRow::as_ref)
        .collect(),
      body_pose_joints: self
        .body_pose_joints
        .iter()
        .map(SqliteKeyframeBodyPoseJointRow::as_ref)
        .collect(),
      hand_poses: self
        .hand_poses
        .iter()
        .map(SqliteKeyframeHandPoseRow::as_ref)
        .collect(),
      body_poses_3d: self
        .body_poses_3d
        .iter()
        .map(SqliteKeyframeBodyPose3DRow::as_ref)
        .collect(),
      body_pose_3d_joints: self
        .body_pose_3d_joints
        .iter()
        .map(SqliteKeyframeBodyPose3DJointRow::as_ref)
        .collect(),
      masks: self
        .masks
        .iter()
        .map(SqliteKeyframeMaskRow::as_ref)
        .collect(),
      face_landmarks: self
        .face_landmarks
        .iter()
        .map(SqliteKeyframeFaceLandmarksRow::as_ref)
        .collect(),
      face_landmark_regions: self
        .face_landmark_regions
        .iter()
        .map(SqliteKeyframeFaceLandmarkRegionRow::as_ref)
        .collect(),
      face_landmark_points: self
        .face_landmark_points
        .iter()
        .map(SqliteKeyframeFaceLandmarkPointRow::as_ref)
        .collect(),
      vlm_labels: self
        .vlm_labels
        .iter()
        .map(SqliteKeyframeVlmLabelRow::as_ref)
        .collect(),
    }
  }
}

/// Rebuild a [`Keyframe`] from a borrowed bundle — see
/// [`keyframe_from_rows`] for the owned counterpart. `parent_timebase`
/// is the per-stream timebase carried on the parent `video_track` row.
pub fn keyframe_from_rows_ref<'r>(
  rows: SqliteKeyframeRowsRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<Keyframe<Uuid7>, SqlxError> {
  {
    let row = rows
      .keyframe
      .ok_or_else(|| SqlxError::DomainConstructorRejected("Keyframe row is missing".to_owned()))?;
    let id = bytes_to_uuid7(row.id)?;
    let scene_id = bytes_to_uuid7(row.scene_id)?;
    let pts = mediatime::Timestamp::new(row.pts, parent_timebase);
    let dimensions = Dimensions::new(
      u32_from_i64(row.width, "Keyframe.width")?,
      u32_from_i64(row.height, "Keyframe.height")?,
    );
    let extractor = parse_keyframe_extractor(row.extractor)?;
    let mut kf = Keyframe::try_new(id, scene_id, pts, dimensions, extractor)
      .map_err(|e: KeyframeError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_mime(row.mime)
      .with_data(Bytes::copy_from_slice(row.data))
      .with_aesthetics(Aesthetics::new(
        row.aesthetics_overall_score,
        bool_from_i64(row.aesthetics_is_utility),
      ))
      .with_horizon(
        HorizonInfo::try_new(row.horizon_angle, row.horizon_confidence).map_err(detection_err)?,
      );

    let mut classifications = rows.classifications;
    classifications.sort_by_key(|r| r.ordinal);
    let mut built = Vec::with_capacity(classifications.len());
    for r in classifications.drain(..) {
      built.push(Detection::try_new(r.label, r.confidence).map_err(detection_err)?);
    }
    kf = kf.with_classifications(built);

    let mut objects = rows.objects;
    objects.sort_by_key(|r| r.ordinal);
    let mut built = Vec::with_capacity(objects.len());
    for r in objects.drain(..) {
      let det = Detection::try_new(r.label, r.confidence).map_err(detection_err)?;
      let bbox = if bool_from_i64(r.has_bbox) {
        Some(
          BoundingBox::try_new(
            r.bbox_x.unwrap_or_default(),
            r.bbox_y.unwrap_or_default(),
            r.bbox_w.unwrap_or_default(),
            r.bbox_h.unwrap_or_default(),
          )
          .map_err(detection_err)?,
        )
      } else {
        None
      };
      built.push(ObjectDetection::new(det, bbox));
    }
    kf = kf.with_objects(built);

    let mut actions = rows.actions;
    actions.sort_by_key(|r| r.ordinal);
    let mut built = Vec::with_capacity(actions.len());
    for r in actions.drain(..) {
      let det = Detection::try_new(r.label, r.confidence).map_err(detection_err)?;
      built.push(ActionDetection::new(det));
    }
    kf = kf.with_actions(built);

    let mut texts = rows.text_detections;
    texts.sort_by_key(|r| r.ordinal);
    let mut built = Vec::with_capacity(texts.len());
    for r in texts.drain(..) {
      let bb =
        BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
      built.push(TextDetection::try_new(r.text, r.confidence, bb).map_err(detection_err)?);
    }
    kf = kf.with_text_detections(built);

    let mut barcodes = rows.barcodes;
    barcodes.sort_by_key(|r| r.ordinal);
    let mut built = Vec::with_capacity(barcodes.len());
    for r in barcodes.drain(..) {
      let bb =
        BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
      built.push(
        BarcodeDetection::try_new(r.payload, r.symbology, r.confidence, bb)
          .map_err(detection_err)?,
      );
    }
    kf = kf.with_barcodes(built);

    let mut attention = Vec::new();
    let mut objectness = Vec::new();
    let mut saliencies = rows.saliencies;
    saliencies.sort_by_key(|r| (r.kind, r.ordinal));
    for r in saliencies {
      let bb =
        BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
      let region = SaliencyRegion::try_new(bb, r.confidence).map_err(detection_err)?;
      match r.kind {
        0 => attention.push(region),
        1 => objectness.push(region),
        other => {
          return Err(SqlxError::UnknownDiscriminant(format!(
            "keyframe_saliency.kind: {other}"
          )))
        }
      }
    }
    kf = kf
      .with_attention_saliency(attention)
      .with_objectness_saliency(objectness);

    let mut docs = rows.document_segments;
    docs.sort_by_key(|r| r.ordinal);
    let mut built = Vec::with_capacity(docs.len());
    for r in docs.drain(..) {
      built.push(
        DocumentSegment::try_new(
          (r.tl_x, r.tl_y),
          (r.tr_x, r.tr_y),
          (r.br_x, r.br_y),
          (r.bl_x, r.bl_y),
          r.confidence,
        )
        .map_err(detection_err)?,
      );
    }
    kf = kf.with_document_segments(built);

    let mut colors = rows.colors;
    colors.sort_by_key(|r| r.ordinal);
    let mut built = Vec::with_capacity(colors.len());
    for r in colors.drain(..) {
      let rgb = Rgba::from_bits(u32_from_i64(r.rgba, "keyframe_color.rgba")?);
      let population = u32_from_i64(r.population, "keyframe_color.population")?;
      built.push(
        DominantColor::try_new(rgb, r.name, r.percentage, population).map_err(detection_err)?,
      );
    }
    kf = kf.with_colors(built);

    let (human_subjects, animal_subjects) = build_subjects_ref(rows.subjects)?;
    let (human_faces, face_rectangles) = build_faces_ref(rows.faces)?;
    let mut joints_by_scope = group_joints_by_scope_ref(rows.body_pose_joints);
    let (human_body_poses, animal_body_poses) =
      build_body_poses_ref(rows.body_poses, &mut joints_by_scope)?;
    let hand_joints = joints_by_scope.remove(&2).unwrap_or_default();
    let hand_poses = build_hand_poses_ref(rows.hand_poses, hand_joints)?;
    let body_poses_3d = build_body_poses_3d_ref(rows.body_poses_3d, rows.body_pose_3d_joints)?;
    let (instance_masks, segmentation_masks) = build_masks_ref(rows.masks)?;
    let face_landmarks = build_face_landmarks_ref(
      rows.face_landmarks,
      rows.face_landmark_regions,
      rows.face_landmark_points,
    )?;

    let humans = HumanAnalysis::new()
      .with_subjects(human_subjects)
      .with_faces(human_faces)
      .with_face_rectangles(face_rectangles)
      .with_body_poses(human_body_poses)
      .with_hand_poses(hand_poses)
      .with_body_poses_3d(body_poses_3d)
      .with_instance_masks(instance_masks)
      .with_face_landmarks(face_landmarks)
      .with_segmentation_masks(segmentation_masks);
    let animals = AnimalAnalysis::new()
      .with_subjects(animal_subjects)
      .with_body_poses(animal_body_poses);
    kf = kf.with_humans(humans).with_animals(animals);

    let mut vlm = VlmAnalysis::new()
      .with_description(LocalizedText::from_src_translated(
        row.vlm_description_src,
        row.vlm_description_translated,
      ))
      .with_shot_type(row.vlm_shot_type);
    let labels = group_vlm_by_kind_ref(rows.vlm_labels);
    vlm = vlm
      .with_categories(labels.0)
      .with_tags(labels.1)
      .with_objects(labels.2)
      .with_subjects(labels.3)
      .with_mood(labels.4)
      .with_emotion(labels.5)
      .with_lighting(labels.6);
    kf = kf.with_vlm(vlm);

    Ok(kf)
  }
}

// --- borrowed-row build helpers ---

fn build_subjects_ref(
  rows: Vec<SqliteKeyframeSubjectRowRef<'_>>,
) -> Result<(Vec<SubjectDetection>, Vec<SubjectDetection>), SqlxError> {
  let mut humans = Vec::new();
  let mut animals = Vec::new();
  let mut rows = rows;
  rows.sort_by_key(|r| (r.scope, r.ordinal));
  for r in rows {
    let det = Detection::try_new(r.label, r.confidence).map_err(detection_err)?;
    let bb = BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
    let subject = SubjectDetection::new(det, bb);
    match r.scope {
      0 => humans.push(subject),
      1 => animals.push(subject),
      other => {
        return Err(SqlxError::UnknownDiscriminant(format!(
          "keyframe_subject.scope: {other}"
        )))
      }
    }
  }
  Ok((humans, animals))
}

fn build_faces_ref(
  rows: Vec<SqliteKeyframeFaceRowRef<'_>>,
) -> Result<(Vec<FaceDetection>, Vec<FaceDetection>), SqlxError> {
  let mut faces = Vec::new();
  let mut face_rects = Vec::new();
  let mut rows = rows;
  rows.sort_by_key(|r| (r.kind, r.ordinal));
  for r in rows {
    let bb = BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
    let face = FaceDetection::try_new(bb, r.confidence, r.capture_quality, r.roll, r.yaw, r.pitch)
      .map_err(detection_err)?;
    match r.kind {
      0 => faces.push(face),
      1 => face_rects.push(face),
      other => {
        return Err(SqlxError::UnknownDiscriminant(format!(
          "keyframe_face.kind: {other}"
        )))
      }
    }
  }
  Ok((faces, face_rects))
}

fn group_joints_by_scope_ref<'r>(
  rows: Vec<SqliteKeyframeBodyPoseJointRowRef<'r>>,
) -> std::collections::HashMap<i64, Vec<SqliteKeyframeBodyPoseJointRowRef<'r>>> {
  let mut out: std::collections::HashMap<i64, Vec<SqliteKeyframeBodyPoseJointRowRef<'r>>> =
    std::collections::HashMap::new();
  for r in rows {
    out.entry(r.scope).or_default().push(r);
  }
  out
}

fn joints_lookup_ref<'r>(
  rows: Vec<SqliteKeyframeBodyPoseJointRowRef<'r>>,
) -> std::collections::HashMap<i64, Vec<SqliteKeyframeBodyPoseJointRowRef<'r>>> {
  let mut out: std::collections::HashMap<i64, Vec<SqliteKeyframeBodyPoseJointRowRef<'r>>> =
    std::collections::HashMap::new();
  for r in rows {
    out.entry(r.parent_ordinal).or_default().push(r);
  }
  for v in out.values_mut() {
    v.sort_by_key(|r| r.ordinal);
  }
  out
}

fn build_joints_ref(
  rows: Vec<SqliteKeyframeBodyPoseJointRowRef<'_>>,
) -> Result<Vec<BodyPoseJoint>, SqlxError> {
  let mut out = Vec::with_capacity(rows.len());
  for r in rows {
    out.push(BodyPoseJoint::try_new(r.name, r.x, r.y, r.confidence).map_err(detection_err)?);
  }
  Ok(out)
}

fn build_body_poses_ref<'r>(
  rows: Vec<SqliteKeyframeBodyPoseRowRef<'_>>,
  joints_by_scope: &mut std::collections::HashMap<i64, Vec<SqliteKeyframeBodyPoseJointRowRef<'r>>>,
) -> Result<(Vec<BodyPoseDetection>, Vec<BodyPoseDetection>), SqlxError> {
  let mut humans = Vec::new();
  let mut animals = Vec::new();
  let mut rows = rows;
  rows.sort_by_key(|r| (r.scope, r.ordinal));

  let human_joints = joints_by_scope.remove(&0).unwrap_or_default();
  let animal_joints = joints_by_scope.remove(&1).unwrap_or_default();
  let human_lookup = joints_lookup_ref(human_joints);
  let animal_lookup = joints_lookup_ref(animal_joints);

  for r in rows {
    let bb = BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
    let joints = match r.scope {
      0 => human_lookup.get(&r.ordinal).cloned().unwrap_or_default(),
      1 => animal_lookup.get(&r.ordinal).cloned().unwrap_or_default(),
      other => {
        return Err(SqlxError::UnknownDiscriminant(format!(
          "keyframe_body_pose.scope: {other}"
        )))
      }
    };
    let joints_built = build_joints_ref(joints)?;
    let pose = BodyPoseDetection::try_new(bb, r.confidence, joints_built).map_err(detection_err)?;
    match r.scope {
      0 => humans.push(pose),
      1 => animals.push(pose),
      _ => unreachable!(),
    }
  }
  Ok((humans, animals))
}

fn build_hand_poses_ref<'r>(
  rows: Vec<SqliteKeyframeHandPoseRowRef<'_>>,
  joints: Vec<SqliteKeyframeBodyPoseJointRowRef<'r>>,
) -> Result<Vec<HandPoseDetection>, SqlxError> {
  let joint_lookup = joints_lookup_ref(joints);
  let mut rows = rows;
  rows.sort_by_key(|r| r.ordinal);
  let mut out = Vec::with_capacity(rows.len());
  for r in rows {
    let bb = BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
    let chirality = hand_chirality_from_i16(r.chirality as i16)?;
    let joints = joint_lookup.get(&r.ordinal).cloned().unwrap_or_default();
    let built = build_joints_ref(joints)?;
    out
      .push(HandPoseDetection::try_new(bb, r.confidence, chirality, built).map_err(detection_err)?);
  }
  Ok(out)
}

fn build_body_poses_3d_ref<'r>(
  rows: Vec<SqliteKeyframeBodyPose3DRowRef<'_>>,
  joints: Vec<SqliteKeyframeBodyPose3DJointRowRef<'r>>,
) -> Result<Vec<BodyPose3DDetection>, SqlxError> {
  let mut joint_lookup: std::collections::HashMap<
    i64,
    Vec<SqliteKeyframeBodyPose3DJointRowRef<'r>>,
  > = std::collections::HashMap::new();
  for r in joints {
    joint_lookup.entry(r.parent_ordinal).or_default().push(r);
  }
  for v in joint_lookup.values_mut() {
    v.sort_by_key(|r| r.ordinal);
  }
  let mut rows = rows;
  rows.sort_by_key(|r| r.ordinal);
  let mut out = Vec::with_capacity(rows.len());
  for r in rows {
    let height = height_estimation_from_i16(r.height_estimation as i16)?;
    let joints = joint_lookup.remove(&r.ordinal).unwrap_or_default();
    let mut built = Vec::with_capacity(joints.len());
    for j in joints {
      built.push(
        BodyPose3DJoint::try_new(j.name, j.x, j.y, j.z, j.confidence).map_err(detection_err)?,
      );
    }
    out.push(
      BodyPose3DDetection::try_new(r.confidence, r.body_height, height, built)
        .map_err(detection_err)?,
    );
  }
  Ok(out)
}

fn build_masks_ref(
  rows: Vec<SqliteKeyframeMaskRowRef<'_>>,
) -> Result<
  (
    Vec<PersonInstanceMaskDetection>,
    Vec<PersonSegmentationMask>,
  ),
  SqlxError,
> {
  let mut instance = Vec::new();
  let mut whole = Vec::new();
  let mut rows = rows;
  rows.sort_by_key(|r| (r.kind, r.ordinal));
  for r in rows {
    let bb = BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
    let dims = Dimensions::new(
      u32_from_i64(r.width, "keyframe_mask.width")?,
      u32_from_i64(r.height, "keyframe_mask.height")?,
    );
    match r.kind {
      0 => {
        let idx = r
          .instance_index
          .ok_or_else(|| {
            SqlxError::DomainConstructorRejected(
              "keyframe_mask.instance_index NULL for instance mask".to_owned(),
            )
          })
          .and_then(|v| u32_from_i64(v, "keyframe_mask.instance_index"))?;
        instance.push(
          PersonInstanceMaskDetection::try_new(
            bb,
            r.confidence,
            idx,
            dims,
            Bytes::copy_from_slice(r.data),
          )
          .map_err(detection_err)?,
        );
      }
      1 => {
        whole.push(
          PersonSegmentationMask::try_new(bb, r.confidence, dims, Bytes::copy_from_slice(r.data))
            .map_err(detection_err)?,
        );
      }
      other => {
        return Err(SqlxError::UnknownDiscriminant(format!(
          "keyframe_mask.kind: {other}"
        )))
      }
    }
  }
  Ok((instance, whole))
}

fn build_face_landmarks_ref<'r>(
  rows: Vec<SqliteKeyframeFaceLandmarksRowRef<'r>>,
  regions: Vec<SqliteKeyframeFaceLandmarkRegionRowRef<'r>>,
  points: Vec<SqliteKeyframeFaceLandmarkPointRowRef<'r>>,
) -> Result<Vec<FaceLandmarksDetection>, SqlxError> {
  let mut regions_by_parent: std::collections::HashMap<
    i64,
    Vec<SqliteKeyframeFaceLandmarkRegionRowRef<'r>>,
  > = std::collections::HashMap::new();
  for r in regions {
    regions_by_parent
      .entry(r.parent_ordinal)
      .or_default()
      .push(r);
  }
  for v in regions_by_parent.values_mut() {
    v.sort_by_key(|r| r.ordinal);
  }

  let mut points_by_region: std::collections::HashMap<
    (i64, i64),
    Vec<SqliteKeyframeFaceLandmarkPointRowRef<'r>>,
  > = std::collections::HashMap::new();
  for p in points {
    points_by_region
      .entry((p.parent_ordinal, p.region_ordinal))
      .or_default()
      .push(p);
  }
  for v in points_by_region.values_mut() {
    v.sort_by_key(|r| r.ordinal);
  }

  let mut rows = rows;
  rows.sort_by_key(|r| r.ordinal);
  let mut out = Vec::with_capacity(rows.len());
  for r in rows {
    let bb = BoundingBox::try_new(r.bbox_x, r.bbox_y, r.bbox_w, r.bbox_h).map_err(detection_err)?;
    let region_rows = regions_by_parent.remove(&r.ordinal).unwrap_or_default();
    let mut built_regions = Vec::with_capacity(region_rows.len());
    for region in region_rows {
      let pts = points_by_region
        .remove(&(r.ordinal, region.ordinal))
        .unwrap_or_default();
      let pt_iter: Vec<(f32, f32)> = pts.into_iter().map(|p| (p.x, p.y)).collect();
      built_regions.push(FaceLandmarkRegion::try_new(region.name, pt_iter).map_err(detection_err)?);
    }
    out.push(
      FaceLandmarksDetection::try_new(bb, r.confidence, built_regions).map_err(detection_err)?,
    );
  }
  Ok(out)
}

#[allow(clippy::type_complexity)]
fn group_vlm_by_kind_ref<'r>(
  rows: Vec<SqliteKeyframeVlmLabelRowRef<'r>>,
) -> (
  Vec<LocalizedText>,
  Vec<LocalizedText>,
  Vec<LocalizedText>,
  Vec<LocalizedText>,
  Vec<LocalizedText>,
  Vec<LocalizedText>,
  Vec<LocalizedText>,
) {
  let mut buckets: [Vec<SqliteKeyframeVlmLabelRowRef<'r>>; 7] = Default::default();
  for r in rows {
    if (0..7).contains(&r.kind) {
      buckets[r.kind as usize].push(r);
    }
  }
  let mut out: [Vec<LocalizedText>; 7] = Default::default();
  for (i, bucket) in buckets.iter_mut().enumerate() {
    bucket.sort_by_key(|r| r.ordinal);
    out[i] = bucket
      .drain(..)
      .map(|r| LocalizedText::from_src_translated(r.src, r.translated))
      .collect();
  }
  let [c, t, o, s, m, e, l] = out;
  (c, t, o, s, m, e, l)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;
  use core::num::NonZeroU32;
  use mediatime::{TimeRange, Timebase, Timestamp};

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).unwrap())
  }

  #[test]
  fn video_facet_roundtrip() {
    let v = Video::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_total_scenes(7)
      .with_track_progress(IndexProgress::try_new(2, 1, 0).unwrap());
    let row: SqliteVideoRow = (&v).into();
    let v2: Video<Uuid7> = row.try_into().unwrap();
    assert_eq!(v.id_ref(), v2.id_ref());
    assert_eq!(v2.total_scenes(), 7);
    assert_eq!(v2.track_progress_ref().total(), 2);
  }

  #[test]
  fn video_track_roundtrip_minimal() {
    let t = VideoTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    let tuple: (
      SqliteVideoTrackRow,
      Vec<SqliteVideoTrackIndexErrorRow>,
      Vec<SqliteVideoTrackMetadataRow>,
    ) = (&t).into();
    let t2: VideoTrack<Uuid7> = tuple.try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn video_track_roundtrip_full() {
    let fr = FrameRate::new(Rational::new(24000, NonZeroU32::new(1001).unwrap()), false);
    let color = ColorInfo::new(
      Primaries::Bt709,
      Transfer::Bt709,
      Matrix::Bt709,
      DynamicRange::Limited,
      ChromaLocation::Left,
    );
    let mastering = MasteringDisplay::new(
      [
        ChromaCoord::new(35400, 14600),
        ChromaCoord::new(8500, 39850),
        ChromaCoord::new(6550, 2300),
      ],
      ChromaCoord::new(15635, 16450),
      10_000_000,
      50,
    );
    let cll = ContentLightLevel::new(4000, 800);
    let hdr = HdrStaticMetadata::new(Some(mastering), Some(cll));
    let t = VideoTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_stream_index(Some(0))
      .with_container_track_id(Some(42))
      .with_start_pts(Some(Timestamp::new(0, tb())))
      .try_with_duration(Some(Timestamp::new(180_000, tb())))
      .unwrap()
      .with_codec(VideoCodec::Hevc)
      .with_profile(Some(SmolStr::new("Main10")))
      .with_level(Some(150))
      .with_bit_rate(8_000_000)
      .with_nb_frames(Some(43_200))
      .with_has_b_frames(true)
      .with_closed_gop(Some(true))
      .with_bits_per_raw_sample(Some(10))
      .try_with_dimensions(Dimensions::new(3840, 2160))
      .unwrap()
      .try_with_visible_rect(Some(Rect::new(0, 0, 3840, 2160)))
      .unwrap()
      .with_sample_aspect_ratio(SampleAspectRatio::new(1, NonZeroU32::new(1).unwrap()))
      .with_pixel_format(PixelFormat::from_u32(0x0a))
      .with_color(color)
      .with_hdr_static(Some(hdr))
      .with_rotation(Rotation::D90)
      .with_frame_rate(fr)
      .with_field_order(FieldOrder::Progressive)
      .with_stereo_mode(Some(StereoMode::SideBySide))
      .with_dovi(Some(DolbyVisionConfig::new(8, 9, true, false, 1)))
      .with_has_embedded_captions(true)
      .with_disposition(TrackDisposition::empty())
      .with_is_primary(true)
      .with_auto_selected(true)
      .with_provenance(Provenance::from_parts("v", "1", "p", "i"))
      .with_index_status(VideoIndexStatus::PROBED | VideoIndexStatus::SCENE_DETECTED)
      .with_index_errors(std::vec![ErrorInfo::new(
        ErrorCode::SceneDetectionFailed,
        "bad"
      ),]);
    let tuple: (
      SqliteVideoTrackRow,
      Vec<SqliteVideoTrackIndexErrorRow>,
      Vec<SqliteVideoTrackMetadataRow>,
    ) = (&t).into();
    assert_eq!(tuple.1.len(), 1);
    let t2: VideoTrack<Uuid7> = tuple.try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn scene_roundtrip() {
    let s = Scene::try_new(
      Uuid7::new(),
      Uuid7::new(),
      3,
      TimeRange::new(5_000, 10_000, tb()),
      SceneDetector::Adaptive,
    )
    .unwrap()
    .with_description("Jane is eating");
    let row: SqliteSceneRow = (&s).into();
    let s2 = scene_from_row(row, tb()).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn keyframe_roundtrip_minimal() {
    let kf = Keyframe::try_new(
      Uuid7::new(),
      Uuid7::new(),
      Timestamp::new(0, tb()),
      Dimensions::new(320, 180),
      KeyframeExtractor::CompositeQuality,
    )
    .unwrap();
    let rows: SqliteKeyframeRows = (&kf).into();
    let kf2 = keyframe_from_rows(rows, tb()).unwrap();
    assert_eq!(kf, kf2);
  }

  #[test]
  fn keyframe_roundtrip_full() {
    let bb = BoundingBox::try_new(0.1, 0.2, 0.3, 0.4).unwrap();
    let mut kf = Keyframe::try_new(
      Uuid7::new(),
      Uuid7::new(),
      Timestamp::new(7000, tb()),
      Dimensions::new(1920, 1080),
      KeyframeExtractor::IFrame,
    )
    .unwrap()
    .with_mime("image/jpeg")
    .with_data(std::vec![0xff_u8, 0xd8, 0xff])
    .with_classifications(std::vec![Detection::try_new("dog", 0.97).unwrap()])
    .with_objects(std::vec![
      ObjectDetection::new(Detection::try_new("ball", 0.6).unwrap(), Some(bb)),
      ObjectDetection::new(Detection::try_new("frisbee", 0.5).unwrap(), None),
    ])
    .with_actions(std::vec![ActionDetection::new(
      Detection::try_new("running", 0.8).unwrap(),
    )])
    .with_text_detections(std::vec![
      TextDetection::try_new("HELLO", 0.95, bb).unwrap(),
    ])
    .with_barcodes(std::vec![BarcodeDetection::try_new(
      "payload", "qr", 0.9, bb
    )
    .unwrap(),])
    .with_attention_saliency(std::vec![SaliencyRegion::try_new(bb, 0.7).unwrap()])
    .with_objectness_saliency(std::vec![SaliencyRegion::try_new(bb, 0.8).unwrap()])
    .with_horizon(HorizonInfo::try_new(0.05, 0.9).unwrap())
    .with_document_segments(std::vec![DocumentSegment::try_new(
      (0.1, 0.1),
      (0.9, 0.1),
      (0.9, 0.9),
      (0.1, 0.9),
      0.85,
    )
    .unwrap()])
    .with_aesthetics(Aesthetics::new(0.75, false))
    .with_colors(std::vec![DominantColor::try_new(
      Rgba::from_components(10, 20, 30, 255),
      "red",
      33.3,
      100
    )
    .unwrap(),])
    .with_vlm(
      VlmAnalysis::new()
        .with_description(LocalizedText::from_src("a dog running"))
        .with_tags(std::vec![LocalizedText::from_src("dog")])
        .with_shot_type("medium-shot")
        .with_categories(std::vec![LocalizedText::from_src("animals")])
        .with_objects(std::vec![LocalizedText::from_src("ball")])
        .with_subjects(std::vec![LocalizedText::from_src("dog")])
        .with_mood(std::vec![LocalizedText::from_src("playful")])
        .with_emotion(std::vec![LocalizedText::from_src("joy")])
        .with_lighting(std::vec![LocalizedText::from_src("daylight")]),
    );
    let humans = HumanAnalysis::new()
      .with_subjects(std::vec![SubjectDetection::new(
        Detection::try_new("person", 0.9).unwrap(),
        bb,
      )])
      .with_faces(std::vec![FaceDetection::try_new(
        bb, 0.95, 0.8, 0.1, 0.2, 0.3
      )
      .unwrap(),])
      .with_face_rectangles(std::vec![FaceDetection::try_new(
        bb, 0.9, 0.7, 0.0, 0.0, 0.0
      )
      .unwrap(),])
      .with_body_poses(std::vec![BodyPoseDetection::try_new(
        bb,
        0.9,
        std::vec![
          BodyPoseJoint::try_new("nose", 0.5, 0.2, 0.95).unwrap(),
          BodyPoseJoint::try_new("left_eye", 0.45, 0.18, 0.93).unwrap(),
        ],
      )
      .unwrap()])
      .with_hand_poses(std::vec![HandPoseDetection::try_new(
        bb,
        0.8,
        HandChirality::Right,
        std::vec![BodyPoseJoint::try_new("thumb", 0.3, 0.4, 0.9).unwrap()],
      )
      .unwrap()])
      .with_body_poses_3d(std::vec![BodyPose3DDetection::try_new(
        0.8,
        1.75,
        BodyPose3DHeightEstimation::Measured,
        std::vec![BodyPose3DJoint::try_new("head", 0.0, 1.7, 0.0, 0.95).unwrap()],
      )
      .unwrap()])
      .with_instance_masks(std::vec![PersonInstanceMaskDetection::try_new(
        bb,
        0.9,
        0,
        Dimensions::new(32, 16),
        Bytes::from_static(&[0u8, 255]),
      )
      .unwrap()])
      .with_face_landmarks(std::vec![FaceLandmarksDetection::try_new(
        bb,
        0.92,
        std::vec![
          FaceLandmarkRegion::try_new("leftEye", std::vec![(0.4, 0.3), (0.42, 0.31)]).unwrap(),
          FaceLandmarkRegion::try_new("outerLips", std::vec![(0.5, 0.6)]).unwrap(),
        ],
      )
      .unwrap()])
      .with_segmentation_masks(std::vec![PersonSegmentationMask::try_new(
        bb,
        0.85,
        Dimensions::new(64, 32),
        Bytes::from_static(&[1u8, 2, 3]),
      )
      .unwrap()]);
    let animals = AnimalAnalysis::new()
      .with_subjects(std::vec![SubjectDetection::new(
        Detection::try_new("dog", 0.92).unwrap(),
        bb,
      )])
      .with_body_poses(std::vec![BodyPoseDetection::try_new(
        bb,
        0.88,
        std::vec![BodyPoseJoint::try_new("snout", 0.3, 0.3, 0.9).unwrap()],
      )
      .unwrap()]);
    kf = kf.with_humans(humans).with_animals(animals);

    let rows: SqliteKeyframeRows = (&kf).into();
    let kf2 = keyframe_from_rows(rows, tb()).unwrap();
    assert_eq!(kf, kf2);
  }

  #[test]
  fn video_track_index_errors_rebuild_in_ordinal_order() {
    let t = VideoTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_index_errors(std::vec![
        ErrorInfo::new(ErrorCode::ProbeCorrupt, "a"),
        ErrorInfo::new(ErrorCode::PathNotFound, "b"),
        ErrorInfo::new(ErrorCode::SceneDetectionFailed, "c"),
      ]);
    let (row, mut errs, meta): (
      SqliteVideoTrackRow,
      Vec<SqliteVideoTrackIndexErrorRow>,
      Vec<SqliteVideoTrackMetadataRow>,
    ) = (&t).into();
    errs.reverse();
    let t2: VideoTrack<Uuid7> = (row, errs, meta).try_into().unwrap();
    assert_eq!(t2.index_errors_slice().len(), 3);
    assert_eq!(t2.index_errors_slice()[0].message(), "a");
  }

  #[test]
  fn video_track_metadata_roundtrip_preserves_insertion_order() {
    let mut meta = IndexMap::new();
    meta.insert(SmolStr::new("encoder"), SmolStr::new("x265"));
    meta.insert(SmolStr::new("language"), SmolStr::new("eng"));
    let t = VideoTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_metadata(meta);
    let (row, errs, mut metadata): (
      SqliteVideoTrackRow,
      Vec<SqliteVideoTrackIndexErrorRow>,
      Vec<SqliteVideoTrackMetadataRow>,
    ) = (&t).into();
    assert_eq!(metadata.len(), 2);
    metadata.reverse();
    let t2: VideoTrack<Uuid7> = (row, errs, metadata).try_into().unwrap();
    let keys: Vec<&str> = t2.metadata_ref().keys().map(SmolStr::as_str).collect();
    assert_eq!(keys, std::vec!["encoder", "language"]);
  }

  // -------------------------------------------------------------------------
  // Ref-row round-trip coverage
  // -------------------------------------------------------------------------

  #[test]
  fn video_facet_ref_roundtrip() {
    let v = Video::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_total_scenes(7)
      .with_track_progress(IndexProgress::try_new(2, 1, 0).unwrap());
    let row: SqliteVideoRow = (&v).into();
    let v2: Video<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(v.id_ref(), v2.id_ref());
    assert_eq!(v.media_id_ref(), v2.media_id_ref());
    assert_eq!(v2.total_scenes(), 7);
    assert_eq!(v2.track_progress_ref().total(), 2);
  }

  #[test]
  fn video_track_ref_roundtrip() {
    let fr = FrameRate::new(Rational::new(24000, NonZeroU32::new(1001).unwrap()), false);
    let color = ColorInfo::new(
      Primaries::Bt709,
      Transfer::Bt709,
      Matrix::Bt709,
      DynamicRange::Limited,
      ChromaLocation::Left,
    );
    let t = VideoTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(VideoCodec::Hevc)
      .with_profile(Some(SmolStr::new("Main10")))
      .with_bit_rate(8_000_000)
      .try_with_dimensions(Dimensions::new(1920, 1080))
      .unwrap()
      .with_sample_aspect_ratio(SampleAspectRatio::new(1, NonZeroU32::new(1).unwrap()))
      .with_color(color)
      .with_frame_rate(fr)
      .with_provenance(Provenance::from_parts("v", "1", "p", "i"))
      .with_index_status(VideoIndexStatus::PROBED)
      .with_index_errors(std::vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad")]);
    let (row, errs, meta): (
      SqliteVideoTrackRow,
      Vec<SqliteVideoTrackIndexErrorRow>,
      Vec<SqliteVideoTrackMetadataRow>,
    ) = (&t).into();
    let err_refs: Vec<SqliteVideoTrackIndexErrorRowRef<'_>> = errs
      .iter()
      .map(SqliteVideoTrackIndexErrorRow::as_ref)
      .collect();
    let meta_refs: Vec<SqliteVideoTrackMetadataRowRef<'_>> = meta
      .iter()
      .map(SqliteVideoTrackMetadataRow::as_ref)
      .collect();
    let t2: VideoTrack<Uuid7> = (row.as_ref(), err_refs, meta_refs).try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn scene_ref_roundtrip() {
    let s = Scene::try_new(
      Uuid7::new(),
      Uuid7::new(),
      3,
      TimeRange::new(5_000, 10_000, tb()),
      SceneDetector::Adaptive,
    )
    .unwrap()
    .with_description("Jane is eating");
    let row: SqliteSceneRow = (&s).into();
    let s2 = scene_from_row_ref(row.as_ref(), tb()).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn keyframe_ref_roundtrip_minimal() {
    let kf = Keyframe::try_new(
      Uuid7::new(),
      Uuid7::new(),
      Timestamp::new(0, tb()),
      Dimensions::new(320, 180),
      KeyframeExtractor::CompositeQuality,
    )
    .unwrap();
    let rows: SqliteKeyframeRows = (&kf).into();
    let kf2 = keyframe_from_rows_ref(rows.as_ref(), tb()).unwrap();
    assert_eq!(kf, kf2);
  }

  #[test]
  fn keyframe_ref_roundtrip_with_classification_object_and_vlm() {
    let bb = BoundingBox::try_new(0.1, 0.2, 0.3, 0.4).unwrap();
    let kf = Keyframe::try_new(
      Uuid7::new(),
      Uuid7::new(),
      Timestamp::new(7000, tb()),
      Dimensions::new(1920, 1080),
      KeyframeExtractor::IFrame,
    )
    .unwrap()
    .with_mime("image/jpeg")
    .with_data(std::vec![0xff_u8, 0xd8, 0xff])
    .with_classifications(std::vec![Detection::try_new("dog", 0.97).unwrap()])
    .with_objects(std::vec![ObjectDetection::new(
      Detection::try_new("ball", 0.6).unwrap(),
      Some(bb),
    )])
    .with_vlm(
      VlmAnalysis::new()
        .with_description(LocalizedText::from_src("a dog running"))
        .with_tags(std::vec![LocalizedText::from_src("dog")])
        .with_shot_type("medium-shot"),
    );
    let rows: SqliteKeyframeRows = (&kf).into();
    let kf2 = keyframe_from_rows_ref(rows.as_ref(), tb()).unwrap();
    assert_eq!(kf, kf2);
  }
}
