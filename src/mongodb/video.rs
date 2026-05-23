//! `Video` + `VideoTrack` + `Scene` + `Keyframe` ↔ bson `Document` mapping.
//!
//! The keyframe detection-VO surface (apple-vision, colorthief, VLM) is
//! comprehensive — every nested type maps via the same field-by-field
//! pattern used in the other backends.

use ::bson::{Bson, Document};
use core::{num::NonZeroU32, str::FromStr};
use mediaframe::{
  codec::VideoCodec,
  color::{
    ChromaCoord, ChromaLocation, ContentLightLevel, DolbyVisionConfig, DynamicRange,
    HdrStaticMetadata, Info as ColorInfo, MasteringDisplay, Matrix, Primaries, Transfer,
  },
  disposition::TrackDisposition,
  frame::{FieldOrder, FrameRate, Rational, Rect, Rotation, SampleAspectRatio, StereoMode},
  pixel_format::PixelFormat,
};
use smol_str::SmolStr;

use crate::domain::{
  aggregates::video::{
    detections::*, facet::Video, keyframe::Keyframe, scene::Scene, track::VideoTrack,
  },
  bitflags::VideoIndexStatus,
  enums::{KeyframeExtractor, SceneDetector},
  vo::IndexProgress as VIndexProgress,
  Uuid7,
};

use super::{error::MongoError, util::*};

// ---------------------------------------------------------------------------
// SceneDetector / KeyframeExtractor ↔ Int32
// ---------------------------------------------------------------------------

fn scene_detector_to_i32(s: SceneDetector) -> i32 {
  match s {
    SceneDetector::Histogram => 0,
    SceneDetector::Phash => 1,
    SceneDetector::Threshold => 2,
    SceneDetector::Content => 3,
    SceneDetector::Adaptive => 4,
    SceneDetector::Manual => 5,
  }
}

fn scene_detector_from_i64(v: i64, field: &'static str) -> Result<SceneDetector, MongoError> {
  match v {
    0 => Ok(SceneDetector::Histogram),
    1 => Ok(SceneDetector::Phash),
    2 => Ok(SceneDetector::Threshold),
    3 => Ok(SceneDetector::Content),
    4 => Ok(SceneDetector::Adaptive),
    5 => Ok(SceneDetector::Manual),
    _ => Err(MongoError::IntOutOfRange {
      field: SmolStr::from(field),
      value: v,
    }),
  }
}

fn keyframe_extractor_to_i32(k: KeyframeExtractor) -> i32 {
  match k {
    KeyframeExtractor::Histogram => 0,
    KeyframeExtractor::Phash => 1,
    KeyframeExtractor::Threshold => 2,
    KeyframeExtractor::Content => 3,
    KeyframeExtractor::Adaptive => 4,
    KeyframeExtractor::CompositeQuality => 5,
    KeyframeExtractor::Interval => 6,
    KeyframeExtractor::IFrame => 7,
    KeyframeExtractor::SceneRepresentative => 8,
    KeyframeExtractor::Manual => 9,
  }
}

fn keyframe_extractor_from_i64(
  v: i64,
  field: &'static str,
) -> Result<KeyframeExtractor, MongoError> {
  match v {
    0 => Ok(KeyframeExtractor::Histogram),
    1 => Ok(KeyframeExtractor::Phash),
    2 => Ok(KeyframeExtractor::Threshold),
    3 => Ok(KeyframeExtractor::Content),
    4 => Ok(KeyframeExtractor::Adaptive),
    5 => Ok(KeyframeExtractor::CompositeQuality),
    6 => Ok(KeyframeExtractor::Interval),
    7 => Ok(KeyframeExtractor::IFrame),
    8 => Ok(KeyframeExtractor::SceneRepresentative),
    9 => Ok(KeyframeExtractor::Manual),
    _ => Err(MongoError::IntOutOfRange {
      field: SmolStr::from(field),
      value: v,
    }),
  }
}

// ---------------------------------------------------------------------------
// IndexProgress (video copy) — keep this distinct from subtitle's copy
// even though the shape is identical; `subtitle::IndexProgress` is the
// re-exported canonical for now (per the locked follow-up). For the
// video facet we serialise via the same field shape so the document is
// interchangeable.
// ---------------------------------------------------------------------------

fn v_index_progress_to_bson(p: &VIndexProgress) -> Bson {
  let mut d = Document::new();
  d.insert("total", Bson::Int64(p.total() as i64));
  d.insert("indexed", Bson::Int64(p.indexed() as i64));
  d.insert("failed", Bson::Int64(p.failed() as i64));
  Bson::Document(d)
}

fn v_index_progress_from_bson(b: Bson, field: &'static str) -> Result<VIndexProgress, MongoError> {
  let mut d = as_doc(b, field)?;
  let total = as_u32(take(&mut d, "total")?, "total")?;
  let indexed = as_u32(take(&mut d, "indexed")?, "indexed")?;
  let failed = as_u32(take(&mut d, "failed")?, "failed")?;
  VIndexProgress::try_new(total, indexed, failed).map_err(|_| MongoError::IntOutOfRange {
    field: SmolStr::from(field),
    value: total as i64,
  })
}

// ---------------------------------------------------------------------------
// Video facet
// ---------------------------------------------------------------------------

impl From<&Video<Uuid7>> for Document {
  fn from(v: &Video<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*v.id_ref()));
    d.insert("parent", uuid7_to_bson(*v.parent_ref()));
    d.insert("total_scenes", Bson::Int64(v.total_scenes() as i64));
    d.insert("tracks", uuid7_vec_to_bson(v.tracks_slice()));
    d.insert(
      "track_progress",
      v_index_progress_to_bson(v.track_progress_ref()),
    );
    d
  }
}

impl TryFrom<Document> for Video<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let parent = uuid7_from_bson(take(&mut d, "parent")?, "parent")?;
    let mut v = Video::try_new(id, parent)?;
    // Fields are independent at the domain layer — see the
    // validation-responsibility note on `Video`. Restore each from the
    // stored row directly.
    if let Some(b) = take_opt(&mut d, "tracks") {
      v.set_tracks(uuid7_vec_from_bson(b, "tracks")?);
    }
    if let Some(b) = take_opt(&mut d, "total_scenes") {
      v.set_total_scenes(as_u32(b, "total_scenes")?);
    }
    if let Some(b) = take_opt(&mut d, "track_progress") {
      v.set_track_progress(v_index_progress_from_bson(b, "track_progress")?);
    }
    Ok(v)
  }
}

// ---------------------------------------------------------------------------
// VideoCodec ↔ FFmpeg short-name `String`
// ---------------------------------------------------------------------------
// `VideoCodec: FromStr<Err = Infallible>` (unknown slugs → `Other`), so
// `as_str` / `FromStr` is a total, lossless String round-trip.

fn codec_to_bson(c: &VideoCodec) -> Bson {
  Bson::String(c.as_str().to_owned())
}

fn codec_from_bson(b: Bson, field: &'static str) -> Result<VideoCodec, MongoError> {
  let Ok(codec) = VideoCodec::from_str(&as_str(b, field)?);
  Ok(codec)
}

// ---------------------------------------------------------------------------
// `mediaframe` frame / colour VOs ↔ Document
// ---------------------------------------------------------------------------

fn rect_to_bson(r: &Rect) -> Bson {
  let mut d = Document::new();
  d.insert("x", Bson::Int64(r.x() as i64));
  d.insert("y", Bson::Int64(r.y() as i64));
  d.insert("width", Bson::Int64(r.width() as i64));
  d.insert("height", Bson::Int64(r.height() as i64));
  Bson::Document(d)
}

fn rect_from_bson(b: Bson, field: &'static str) -> Result<Rect, MongoError> {
  let mut d = as_doc(b, field)?;
  let x = as_u32(take(&mut d, "x")?, "x")?;
  let y = as_u32(take(&mut d, "y")?, "y")?;
  let w = as_u32(take(&mut d, "width")?, "width")?;
  let h = as_u32(take(&mut d, "height")?, "height")?;
  Ok(Rect::new(x, y, w, h))
}

// Shared `(num, den)` decode for the two `Rational`-backed VOs below.
fn rational_from_doc(d: &mut Document) -> Result<Rational, MongoError> {
  let num = as_u32(take(d, "num")?, "num")?;
  let den_v = as_u32(take(d, "den")?, "den")?;
  let den = NonZeroU32::new(den_v).ok_or_else(|| MongoError::IntOutOfRange {
    field: SmolStr::from("den"),
    value: 0,
  })?;
  Ok(Rational::new(num, den))
}

// `SampleAspectRatio` ↔ `{ num, den }` (newtype over `Rational`).
fn sar_to_bson(s: SampleAspectRatio) -> Bson {
  let mut d = Document::new();
  d.insert("num", Bson::Int64(s.num() as i64));
  d.insert("den", Bson::Int64(s.den().get() as i64));
  Bson::Document(d)
}

fn sar_from_bson(b: Bson, field: &'static str) -> Result<SampleAspectRatio, MongoError> {
  let mut d = as_doc(b, field)?;
  let r = rational_from_doc(&mut d)?;
  Ok(SampleAspectRatio::new(r.num(), r.den()))
}

// `FrameRate` ↔ `{ num, den, is_vfr }` (a `Rational` rate + VFR flag).
fn frame_rate_to_bson(f: FrameRate) -> Bson {
  let r = f.rate();
  let mut d = Document::new();
  d.insert("num", Bson::Int64(r.num() as i64));
  d.insert("den", Bson::Int64(r.den().get() as i64));
  d.insert("is_vfr", Bson::Boolean(f.is_vfr()));
  Bson::Document(d)
}

fn frame_rate_from_bson(b: Bson, field: &'static str) -> Result<FrameRate, MongoError> {
  let mut d = as_doc(b, field)?;
  let is_vfr = as_bool(take(&mut d, "is_vfr")?, "is_vfr")?;
  let r = rational_from_doc(&mut d)?;
  Ok(FrameRate::new(r, is_vfr))
}

// `color::Info` carries five typed enums, each `to_u32`/`from_u32`-coded;
// persisted as Int32 sub-fields.
fn color_info_to_bson(c: &ColorInfo) -> Bson {
  let mut d = Document::new();
  d.insert("primaries", Bson::Int64(c.primaries().to_u32() as i64));
  d.insert("transfer", Bson::Int64(c.transfer().to_u32() as i64));
  d.insert("matrix", Bson::Int64(c.matrix().to_u32() as i64));
  d.insert("range", Bson::Int64(c.range().to_u32() as i64));
  d.insert(
    "chroma_location",
    Bson::Int64(c.chroma_location().to_u32() as i64),
  );
  Bson::Document(d)
}

fn color_info_from_bson(b: Bson, field: &'static str) -> Result<ColorInfo, MongoError> {
  let mut d = as_doc(b, field)?;
  let p = Primaries::from_u32(as_u32(take(&mut d, "primaries")?, "primaries")?);
  let t = Transfer::from_u32(as_u32(take(&mut d, "transfer")?, "transfer")?);
  let m = Matrix::from_u32(as_u32(take(&mut d, "matrix")?, "matrix")?);
  let r = DynamicRange::from_u32(as_u32(take(&mut d, "range")?, "range")?);
  let c = ChromaLocation::from_u32(as_u32(take(&mut d, "chroma_location")?, "chroma_location")?);
  Ok(ColorInfo::new(p, t, m, r, c))
}

// `ChromaCoord` ↔ `{ x, y }`.
fn chroma_coord_to_bson(c: ChromaCoord) -> Bson {
  let mut d = Document::new();
  d.insert("x", Bson::Int64(c.x() as i64));
  d.insert("y", Bson::Int64(c.y() as i64));
  Bson::Document(d)
}

fn chroma_coord_from_bson(b: Bson, field: &'static str) -> Result<ChromaCoord, MongoError> {
  let mut d = as_doc(b, field)?;
  let x = as_u32(take(&mut d, "x")?, "x")?;
  let y = as_u32(take(&mut d, "y")?, "y")?;
  Ok(ChromaCoord::new(x, y))
}

// `MasteringDisplay` ↔ `{ primaries: [coord; 3], white_point, max_lum, min_lum }`.
fn mastering_to_bson(m: &MasteringDisplay) -> Bson {
  let mut d = Document::new();
  let prims = m.display_primaries();
  d.insert(
    "primaries",
    Bson::Array(prims.iter().map(|c| chroma_coord_to_bson(*c)).collect()),
  );
  d.insert("white_point", chroma_coord_to_bson(m.white_point()));
  d.insert("max_luminance", Bson::Int64(m.max_luminance() as i64));
  d.insert("min_luminance", Bson::Int64(m.min_luminance() as i64));
  Bson::Document(d)
}

fn mastering_from_bson(b: Bson, field: &'static str) -> Result<MasteringDisplay, MongoError> {
  let mut d = as_doc(b, field)?;
  let arr = as_array(take(&mut d, "primaries")?, "primaries")?;
  if arr.len() != 3 {
    return Err(MongoError::type_mismatch(
      field,
      "3-element primaries array",
      "other",
    ));
  }
  let mut it = arr.into_iter();
  let prims = [
    chroma_coord_from_bson(it.next().unwrap(), "primaries[0]")?,
    chroma_coord_from_bson(it.next().unwrap(), "primaries[1]")?,
    chroma_coord_from_bson(it.next().unwrap(), "primaries[2]")?,
  ];
  let white = chroma_coord_from_bson(take(&mut d, "white_point")?, "white_point")?;
  let max_lum = as_u32(take(&mut d, "max_luminance")?, "max_luminance")?;
  let min_lum = as_u32(take(&mut d, "min_luminance")?, "min_luminance")?;
  Ok(MasteringDisplay::new(prims, white, max_lum, min_lum))
}

// `ContentLightLevel` ↔ `{ max_cll, max_fall }`.
fn content_light_to_bson(c: ContentLightLevel) -> Bson {
  let mut d = Document::new();
  d.insert("max_cll", Bson::Int64(c.max_cll() as i64));
  d.insert("max_fall", Bson::Int64(c.max_fall() as i64));
  Bson::Document(d)
}

fn content_light_from_bson(b: Bson, field: &'static str) -> Result<ContentLightLevel, MongoError> {
  let mut d = as_doc(b, field)?;
  let cll = as_u32(take(&mut d, "max_cll")?, "max_cll")?;
  let fall = as_u32(take(&mut d, "max_fall")?, "max_fall")?;
  Ok(ContentLightLevel::new(cll, fall))
}

// `HdrStaticMetadata` ↔ `{ mastering?, content_light? }` — both halves
// optional (the rev that wraps `Option<MasteringDisplay>` +
// `Option<ContentLightLevel>` rather than flat MaxCLL/MaxFALL).
fn hdr_static_to_bson(h: &HdrStaticMetadata) -> Bson {
  let mut d = Document::new();
  d.insert(
    "mastering",
    h.mastering()
      .map(|m| mastering_to_bson(&m))
      .unwrap_or(Bson::Null),
  );
  d.insert(
    "content_light",
    h.content_light()
      .map(content_light_to_bson)
      .unwrap_or(Bson::Null),
  );
  Bson::Document(d)
}

fn hdr_static_from_bson(b: Bson, field: &'static str) -> Result<HdrStaticMetadata, MongoError> {
  let mut d = as_doc(b, field)?;
  let mastering = opt(take_opt(&mut d, "mastering"), |bb| {
    mastering_from_bson(bb, "mastering")
  })?;
  let content_light = opt(take_opt(&mut d, "content_light"), |bb| {
    content_light_from_bson(bb, "content_light")
  })?;
  Ok(HdrStaticMetadata::new(mastering, content_light))
}

fn dovi_to_bson(c: &DolbyVisionConfig) -> Bson {
  let mut d = Document::new();
  d.insert("profile", Bson::Int32(c.profile() as i32));
  d.insert("level", Bson::Int32(c.level() as i32));
  d.insert("rpu_present", Bson::Boolean(c.rpu_present()));
  d.insert("el_present", Bson::Boolean(c.el_present()));
  d.insert(
    "bl_signal_compat_id",
    Bson::Int32(c.bl_signal_compat_id() as i32),
  );
  Bson::Document(d)
}

fn dovi_from_bson(b: Bson, field: &'static str) -> Result<DolbyVisionConfig, MongoError> {
  let mut d = as_doc(b, field)?;
  let p = as_u8(take(&mut d, "profile")?, "profile")?;
  let l = as_u8(take(&mut d, "level")?, "level")?;
  let rpu = as_bool(take(&mut d, "rpu_present")?, "rpu_present")?;
  let el = as_bool(take(&mut d, "el_present")?, "el_present")?;
  let bl_sig = as_u8(take(&mut d, "bl_signal_compat_id")?, "bl_signal_compat_id")?;
  Ok(DolbyVisionConfig::new(p, l, rpu, el, bl_sig))
}

// ---------------------------------------------------------------------------
// VideoTrack
// ---------------------------------------------------------------------------

impl From<&VideoTrack<Uuid7>> for Document {
  fn from(t: &VideoTrack<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*t.id_ref()));
    d.insert("parent", uuid7_to_bson(*t.parent_ref()));
    d.insert(
      "stream_index",
      t.stream_index()
        .map(|v| Bson::Int64(v as i64))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "container_track_id",
      t.container_track_id()
        .map(|v| Bson::Int64(v as i64))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "start_pts",
      t.start_pts_ref()
        .map(|v| media_ts_to_bson(*v))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "duration",
      t.duration_ref()
        .map(|v| media_ts_to_bson(*v))
        .unwrap_or(Bson::Null),
    );
    d.insert("codec", codec_to_bson(t.codec_ref()));
    d.insert(
      "profile",
      t.profile()
        .map(|s| Bson::String(s.to_owned()))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "level",
      t.level()
        .map(|v| Bson::Int32(v as i32))
        .unwrap_or(Bson::Null),
    );
    d.insert("bit_rate", Bson::Int64(t.bit_rate() as i64));
    d.insert(
      "nb_frames",
      t.nb_frames()
        .map(|v| Bson::Int64(v as i64))
        .unwrap_or(Bson::Null),
    );
    d.insert("has_b_frames", Bson::Boolean(t.has_b_frames()));
    d.insert(
      "closed_gop",
      t.closed_gop().map(Bson::Boolean).unwrap_or(Bson::Null),
    );
    d.insert(
      "bits_per_raw_sample",
      t.bits_per_raw_sample()
        .map(|v| Bson::Int32(v as i32))
        .unwrap_or(Bson::Null),
    );
    d.insert("dimensions", dimensions_to_bson(t.dimensions()));
    d.insert(
      "visible_rect",
      t.visible_rect()
        .map(|r| rect_to_bson(&r))
        .unwrap_or(Bson::Null),
    );
    d.insert("sample_aspect_ratio", sar_to_bson(t.sample_aspect_ratio()));
    d.insert(
      "pixel_format",
      Bson::Int64(t.pixel_format().to_u32() as i64),
    );
    d.insert("color", color_info_to_bson(t.color_ref()));
    d.insert(
      "hdr_static",
      t.hdr_static_ref()
        .map(hdr_static_to_bson)
        .unwrap_or(Bson::Null),
    );
    d.insert("rotation", Bson::Int64(t.rotation().to_u32() as i64));
    d.insert("frame_rate", frame_rate_to_bson(t.frame_rate()));
    d.insert("field_order", Bson::Int64(t.field_order().to_u32() as i64));
    d.insert(
      "stereo_mode",
      t.stereo_mode()
        .map(|v| Bson::Int64(v.to_u32() as i64))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "dovi",
      t.dovi().map(|c| dovi_to_bson(&c)).unwrap_or(Bson::Null),
    );
    d.insert(
      "has_embedded_captions",
      Bson::Boolean(t.has_embedded_captions()),
    );
    d.insert("disposition", Bson::Int64(t.disposition().to_u32() as i64));
    d.insert("is_primary", Bson::Boolean(t.is_primary()));
    d.insert("auto_selected", Bson::Boolean(t.auto_selected()));
    d.insert("scenes", uuid7_vec_to_bson(t.scenes_slice()));
    d.insert("index_status", Bson::Int64(t.index_status().bits() as i64));
    d.insert(
      "index_errors",
      error_info_vec_to_bson(t.index_errors_slice()),
    );
    d.insert("provenance", provenance_to_bson(t.provenance_ref()));
    d
  }
}

impl TryFrom<Document> for VideoTrack<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let parent = uuid7_from_bson(take(&mut d, "parent")?, "parent")?;
    let mut t = VideoTrack::try_new(id, parent)?;

    if let Some(b) = take_opt(&mut d, "stream_index") {
      t.set_stream_index(Some(as_u32(b, "stream_index")?));
    }
    if let Some(b) = take_opt(&mut d, "container_track_id") {
      t.set_container_track_id(Some(as_u64(b, "container_track_id")?));
    }
    if let Some(b) = take_opt(&mut d, "start_pts") {
      t.set_start_pts(Some(media_ts_from_bson(b, "start_pts")?));
    }
    if let Some(b) = take_opt(&mut d, "duration") {
      t.try_set_duration(Some(media_ts_from_bson(b, "duration")?))?;
    }
    if let Some(b) = take_opt(&mut d, "codec") {
      t.set_codec(codec_from_bson(b, "codec")?);
    }
    if let Some(b) = take_opt(&mut d, "profile") {
      t.set_profile(Some(as_smol(b, "profile")?));
    }
    if let Some(b) = take_opt(&mut d, "level") {
      t.set_level(Some(as_u16(b, "level")?));
    }
    if let Some(b) = take_opt(&mut d, "bit_rate") {
      t.set_bit_rate(as_u64(b, "bit_rate")?);
    }
    if let Some(b) = take_opt(&mut d, "nb_frames") {
      t.set_nb_frames(Some(as_u64(b, "nb_frames")?));
    }
    if let Some(b) = take_opt(&mut d, "has_b_frames") {
      t.set_has_b_frames(as_bool(b, "has_b_frames")?);
    }
    if let Some(b) = take_opt(&mut d, "closed_gop") {
      t.set_closed_gop(Some(as_bool(b, "closed_gop")?));
    }
    if let Some(b) = take_opt(&mut d, "bits_per_raw_sample") {
      t.set_bits_per_raw_sample(Some(as_u8(b, "bits_per_raw_sample")?));
    }
    if let Some(b) = take_opt(&mut d, "dimensions") {
      t.try_set_dimensions(dimensions_from_bson(b, "dimensions")?)?;
    }
    if let Some(b) = take_opt(&mut d, "visible_rect") {
      t.try_set_visible_rect(Some(rect_from_bson(b, "visible_rect")?))?;
    }
    if let Some(b) = take_opt(&mut d, "sample_aspect_ratio") {
      t.set_sample_aspect_ratio(sar_from_bson(b, "sample_aspect_ratio")?);
    }
    if let Some(b) = take_opt(&mut d, "pixel_format") {
      t.set_pixel_format(PixelFormat::from_u32(as_u32(b, "pixel_format")?));
    }
    if let Some(b) = take_opt(&mut d, "color") {
      t.set_color(color_info_from_bson(b, "color")?);
    }
    if let Some(b) = take_opt(&mut d, "hdr_static") {
      t.set_hdr_static(Some(hdr_static_from_bson(b, "hdr_static")?));
    }
    if let Some(b) = take_opt(&mut d, "rotation") {
      t.set_rotation(Rotation::from_u32(as_u32(b, "rotation")?));
    }
    if let Some(b) = take_opt(&mut d, "frame_rate") {
      t.set_frame_rate(frame_rate_from_bson(b, "frame_rate")?);
    }
    if let Some(b) = take_opt(&mut d, "field_order") {
      t.set_field_order(FieldOrder::from_u32(as_u32(b, "field_order")?));
    }
    if let Some(b) = take_opt(&mut d, "stereo_mode") {
      t.set_stereo_mode(Some(StereoMode::from_u32(as_u32(b, "stereo_mode")?)));
    }
    if let Some(b) = take_opt(&mut d, "dovi") {
      t.set_dovi(Some(dovi_from_bson(b, "dovi")?));
    }
    if let Some(b) = take_opt(&mut d, "has_embedded_captions") {
      t.set_has_embedded_captions(as_bool(b, "has_embedded_captions")?);
    }
    if let Some(b) = take_opt(&mut d, "disposition") {
      t.set_disposition(TrackDisposition::from_u32(as_u32(b, "disposition")?));
    }
    if let Some(b) = take_opt(&mut d, "is_primary") {
      t.set_is_primary(as_bool(b, "is_primary")?);
    }
    if let Some(b) = take_opt(&mut d, "auto_selected") {
      t.set_auto_selected(as_bool(b, "auto_selected")?);
    }
    if let Some(b) = take_opt(&mut d, "scenes") {
      t.set_scenes(uuid7_vec_from_bson(b, "scenes")?);
    }
    if let Some(b) = take_opt(&mut d, "index_status") {
      let bits = as_u64(b, "index_status")?;
      let bits32 = u32::try_from(bits).map_err(|_| MongoError::IntOutOfRange {
        field: SmolStr::from("index_status"),
        value: bits as i64,
      })?;
      t.set_index_status(VideoIndexStatus::from_bits_truncate(bits32));
    }
    if let Some(b) = take_opt(&mut d, "index_errors") {
      t.set_index_errors(error_info_vec_from_bson(b, "index_errors")?);
    }
    if let Some(b) = take_opt(&mut d, "provenance") {
      t.set_provenance(provenance_from_bson(b, "provenance")?);
    }
    Ok(t)
  }
}

// ---------------------------------------------------------------------------
// Scene
// ---------------------------------------------------------------------------

impl From<&Scene<Uuid7>> for Document {
  fn from(s: &Scene<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*s.id_ref()));
    d.insert("parent", uuid7_to_bson(*s.parent_ref()));
    d.insert("index", Bson::Int64(s.index() as i64));
    d.insert("span", time_range_to_bson(s.span_ref()));
    d.insert("detector", Bson::Int32(scene_detector_to_i32(s.detector())));
    d.insert("keyframes", uuid7_vec_to_bson(s.keyframes_slice()));
    d.insert("description", Bson::String(s.description().to_owned()));
    d
  }
}

impl TryFrom<Document> for Scene<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let parent = uuid7_from_bson(take(&mut d, "parent")?, "parent")?;
    let index = as_u32(take(&mut d, "index")?, "index")?;
    let span = time_range_from_bson(take(&mut d, "span")?, "span")?;
    let detector =
      scene_detector_from_i64(as_i64(take(&mut d, "detector")?, "detector")?, "detector")?;
    let mut s = Scene::try_new(id, parent, index, span, detector)?;
    if let Some(b) = take_opt(&mut d, "keyframes") {
      s.set_keyframes(uuid7_vec_from_bson(b, "keyframes")?);
    }
    if let Some(b) = take_opt(&mut d, "description") {
      s.set_description(as_smol(b, "description")?);
    }
    Ok(s)
  }
}

// ===========================================================================
// Keyframe detection-VO helpers
// ===========================================================================

fn detection_to_bson(d: &Detection) -> Bson {
  let mut doc = Document::new();
  doc.insert("label", Bson::String(d.label().to_owned()));
  doc.insert("confidence", Bson::Double(d.confidence() as f64));
  Bson::Document(doc)
}

fn detection_from_bson(b: Bson, field: &'static str) -> Result<Detection, MongoError> {
  let mut d = as_doc(b, field)?;
  let label = as_smol(take(&mut d, "label")?, "label")?;
  let confidence = as_f32(take(&mut d, "confidence")?, "confidence")?;
  Ok(Detection::try_new(label, confidence)?)
}

fn bbox_to_bson(b: &BoundingBox) -> Bson {
  let mut d = Document::new();
  d.insert("x", Bson::Double(b.x() as f64));
  d.insert("y", Bson::Double(b.y() as f64));
  d.insert("width", Bson::Double(b.width() as f64));
  d.insert("height", Bson::Double(b.height() as f64));
  Bson::Document(d)
}

fn bbox_from_bson(b: Bson, field: &'static str) -> Result<BoundingBox, MongoError> {
  let mut d = as_doc(b, field)?;
  let x = as_f32(take(&mut d, "x")?, "x")?;
  let y = as_f32(take(&mut d, "y")?, "y")?;
  let w = as_f32(take(&mut d, "width")?, "width")?;
  let h = as_f32(take(&mut d, "height")?, "height")?;
  Ok(BoundingBox::try_new(x, y, w, h)?)
}

fn object_detection_to_bson(o: &ObjectDetection) -> Bson {
  let mut d = Document::new();
  d.insert("detection", detection_to_bson(o.detection_ref()));
  d.insert("bbox", o.bbox_ref().map(bbox_to_bson).unwrap_or(Bson::Null));
  Bson::Document(d)
}

fn object_detection_from_bson(b: Bson, field: &'static str) -> Result<ObjectDetection, MongoError> {
  let mut d = as_doc(b, field)?;
  let detection = detection_from_bson(take(&mut d, "detection")?, "detection")?;
  let bbox = opt(take_opt(&mut d, "bbox"), |bb| bbox_from_bson(bb, "bbox"))?;
  Ok(ObjectDetection::new(detection, bbox))
}

fn action_detection_to_bson(a: &ActionDetection) -> Bson {
  let mut d = Document::new();
  d.insert("detection", detection_to_bson(a.detection_ref()));
  Bson::Document(d)
}

fn action_detection_from_bson(b: Bson, field: &'static str) -> Result<ActionDetection, MongoError> {
  let mut d = as_doc(b, field)?;
  let detection = detection_from_bson(take(&mut d, "detection")?, "detection")?;
  Ok(ActionDetection::new(detection))
}

fn text_detection_to_bson(t: &TextDetection) -> Bson {
  let mut d = Document::new();
  d.insert("text", Bson::String(t.text().to_owned()));
  d.insert("confidence", Bson::Double(t.confidence() as f64));
  d.insert("bbox", bbox_to_bson(t.bbox_ref()));
  Bson::Document(d)
}

fn text_detection_from_bson(b: Bson, field: &'static str) -> Result<TextDetection, MongoError> {
  let mut d = as_doc(b, field)?;
  let text = as_smol(take(&mut d, "text")?, "text")?;
  let conf = as_f32(take(&mut d, "confidence")?, "confidence")?;
  let bbox = bbox_from_bson(take(&mut d, "bbox")?, "bbox")?;
  Ok(TextDetection::try_new(text, conf, bbox)?)
}

fn barcode_to_bson(b: &BarcodeDetection) -> Bson {
  let mut d = Document::new();
  d.insert("payload", Bson::String(b.payload().to_owned()));
  d.insert("symbology", Bson::String(b.symbology().to_owned()));
  d.insert("confidence", Bson::Double(b.confidence() as f64));
  d.insert("bbox", bbox_to_bson(b.bbox_ref()));
  Bson::Document(d)
}

fn barcode_from_bson(b: Bson, field: &'static str) -> Result<BarcodeDetection, MongoError> {
  let mut d = as_doc(b, field)?;
  let payload = as_smol(take(&mut d, "payload")?, "payload")?;
  let symbology = as_smol(take(&mut d, "symbology")?, "symbology")?;
  let conf = as_f32(take(&mut d, "confidence")?, "confidence")?;
  let bbox = bbox_from_bson(take(&mut d, "bbox")?, "bbox")?;
  Ok(BarcodeDetection::try_new(payload, symbology, conf, bbox)?)
}

fn saliency_to_bson(s: &SaliencyRegion) -> Bson {
  let mut d = Document::new();
  d.insert("bbox", bbox_to_bson(s.bbox_ref()));
  d.insert("confidence", Bson::Double(s.confidence() as f64));
  Bson::Document(d)
}

fn saliency_from_bson(b: Bson, field: &'static str) -> Result<SaliencyRegion, MongoError> {
  let mut d = as_doc(b, field)?;
  let bbox = bbox_from_bson(take(&mut d, "bbox")?, "bbox")?;
  let conf = as_f32(take(&mut d, "confidence")?, "confidence")?;
  Ok(SaliencyRegion::try_new(bbox, conf)?)
}

fn horizon_to_bson(h: &HorizonInfo) -> Bson {
  let mut d = Document::new();
  d.insert("angle", Bson::Double(h.angle() as f64));
  d.insert("confidence", Bson::Double(h.confidence() as f64));
  Bson::Document(d)
}

fn horizon_from_bson(b: Bson, field: &'static str) -> Result<HorizonInfo, MongoError> {
  let mut d = as_doc(b, field)?;
  let a = as_f32(take(&mut d, "angle")?, "angle")?;
  let c = as_f32(take(&mut d, "confidence")?, "confidence")?;
  Ok(HorizonInfo::try_new(a, c)?)
}

fn aesthetics_to_bson(a: &Aesthetics) -> Bson {
  let mut d = Document::new();
  d.insert("overall_score", Bson::Double(a.overall_score() as f64));
  d.insert("is_utility", Bson::Boolean(a.is_utility()));
  Bson::Document(d)
}

fn aesthetics_from_bson(b: Bson, field: &'static str) -> Result<Aesthetics, MongoError> {
  let mut d = as_doc(b, field)?;
  let s = as_f32(take(&mut d, "overall_score")?, "overall_score")?;
  let u = as_bool(take(&mut d, "is_utility")?, "is_utility")?;
  Ok(Aesthetics::new(s, u))
}

fn corner_to_bson(c: (f32, f32)) -> Bson {
  Bson::Array(vec![Bson::Double(c.0 as f64), Bson::Double(c.1 as f64)])
}

fn corner_from_bson(b: Bson, field: &'static str) -> Result<(f32, f32), MongoError> {
  let arr = as_array(b, field)?;
  if arr.len() != 2 {
    return Err(MongoError::type_mismatch(field, "[Number;2]", "array"));
  }
  let mut it = arr.into_iter();
  let x = as_f32(it.next().unwrap(), field)?;
  let y = as_f32(it.next().unwrap(), field)?;
  Ok((x, y))
}

fn document_segment_to_bson(d: &DocumentSegment) -> Bson {
  let mut doc = Document::new();
  doc.insert("top_left", corner_to_bson(d.top_left()));
  doc.insert("top_right", corner_to_bson(d.top_right()));
  doc.insert("bottom_right", corner_to_bson(d.bottom_right()));
  doc.insert("bottom_left", corner_to_bson(d.bottom_left()));
  doc.insert("confidence", Bson::Double(d.confidence() as f64));
  Bson::Document(doc)
}

fn document_segment_from_bson(b: Bson, field: &'static str) -> Result<DocumentSegment, MongoError> {
  let mut d = as_doc(b, field)?;
  let tl = corner_from_bson(take(&mut d, "top_left")?, "top_left")?;
  let tr = corner_from_bson(take(&mut d, "top_right")?, "top_right")?;
  let br = corner_from_bson(take(&mut d, "bottom_right")?, "bottom_right")?;
  let bl = corner_from_bson(take(&mut d, "bottom_left")?, "bottom_left")?;
  let cf = as_f32(take(&mut d, "confidence")?, "confidence")?;
  Ok(DocumentSegment::try_new(tl, tr, br, bl, cf)?)
}

fn joint_to_bson(j: &BodyPoseJoint) -> Bson {
  let mut d = Document::new();
  d.insert("name", Bson::String(j.name().to_owned()));
  d.insert("x", Bson::Double(j.x() as f64));
  d.insert("y", Bson::Double(j.y() as f64));
  d.insert("confidence", Bson::Double(j.confidence() as f64));
  Bson::Document(d)
}

fn joint_from_bson(b: Bson, field: &'static str) -> Result<BodyPoseJoint, MongoError> {
  let mut d = as_doc(b, field)?;
  let name = as_smol(take(&mut d, "name")?, "name")?;
  let x = as_f32(take(&mut d, "x")?, "x")?;
  let y = as_f32(take(&mut d, "y")?, "y")?;
  let c = as_f32(take(&mut d, "confidence")?, "confidence")?;
  Ok(BodyPoseJoint::try_new(name, x, y, c)?)
}

fn joint3d_to_bson(j: &BodyPose3DJoint) -> Bson {
  let mut d = Document::new();
  d.insert("name", Bson::String(j.name().to_owned()));
  d.insert("x", Bson::Double(j.x() as f64));
  d.insert("y", Bson::Double(j.y() as f64));
  d.insert("z", Bson::Double(j.z() as f64));
  d.insert("confidence", Bson::Double(j.confidence() as f64));
  Bson::Document(d)
}

fn joint3d_from_bson(b: Bson, field: &'static str) -> Result<BodyPose3DJoint, MongoError> {
  let mut d = as_doc(b, field)?;
  let name = as_smol(take(&mut d, "name")?, "name")?;
  let x = as_f32(take(&mut d, "x")?, "x")?;
  let y = as_f32(take(&mut d, "y")?, "y")?;
  let z = as_f32(take(&mut d, "z")?, "z")?;
  let c = as_f32(take(&mut d, "confidence")?, "confidence")?;
  Ok(BodyPose3DJoint::try_new(name, x, y, z, c)?)
}

fn chirality_to_i32(c: HandChirality) -> i32 {
  match c {
    HandChirality::Unknown => 0,
    HandChirality::Left => 1,
    HandChirality::Right => 2,
  }
}

fn chirality_from_i64(v: i64, field: &'static str) -> Result<HandChirality, MongoError> {
  match v {
    0 => Ok(HandChirality::Unknown),
    1 => Ok(HandChirality::Left),
    2 => Ok(HandChirality::Right),
    _ => Err(MongoError::IntOutOfRange {
      field: SmolStr::from(field),
      value: v,
    }),
  }
}

fn height_est_to_i32(h: BodyPose3DHeightEstimation) -> i32 {
  match h {
    BodyPose3DHeightEstimation::Unknown => 0,
    BodyPose3DHeightEstimation::Reference => 1,
    BodyPose3DHeightEstimation::Measured => 2,
  }
}

fn height_est_from_i64(
  v: i64,
  field: &'static str,
) -> Result<BodyPose3DHeightEstimation, MongoError> {
  match v {
    0 => Ok(BodyPose3DHeightEstimation::Unknown),
    1 => Ok(BodyPose3DHeightEstimation::Reference),
    2 => Ok(BodyPose3DHeightEstimation::Measured),
    _ => Err(MongoError::IntOutOfRange {
      field: SmolStr::from(field),
      value: v,
    }),
  }
}

fn body_pose_to_bson(p: &BodyPoseDetection) -> Bson {
  let mut d = Document::new();
  d.insert("bbox", bbox_to_bson(p.bbox_ref()));
  d.insert("confidence", Bson::Double(p.confidence() as f64));
  d.insert(
    "joints",
    Bson::Array(p.joints_slice().iter().map(joint_to_bson).collect()),
  );
  Bson::Document(d)
}

fn body_pose_from_bson(b: Bson, field: &'static str) -> Result<BodyPoseDetection, MongoError> {
  let mut d = as_doc(b, field)?;
  let bbox = bbox_from_bson(take(&mut d, "bbox")?, "bbox")?;
  let c = as_f32(take(&mut d, "confidence")?, "confidence")?;
  let joints = as_array(take(&mut d, "joints")?, "joints")?
    .into_iter()
    .map(|x| joint_from_bson(x, "joints[]"))
    .collect::<Result<Vec<_>, _>>()?;
  Ok(BodyPoseDetection::try_new(bbox, c, joints)?)
}

fn hand_pose_to_bson(p: &HandPoseDetection) -> Bson {
  let mut d = Document::new();
  d.insert("bbox", bbox_to_bson(p.bbox_ref()));
  d.insert("confidence", Bson::Double(p.confidence() as f64));
  d.insert("chirality", Bson::Int32(chirality_to_i32(p.chirality())));
  d.insert(
    "joints",
    Bson::Array(p.joints_slice().iter().map(joint_to_bson).collect()),
  );
  Bson::Document(d)
}

fn hand_pose_from_bson(b: Bson, field: &'static str) -> Result<HandPoseDetection, MongoError> {
  let mut d = as_doc(b, field)?;
  let bbox = bbox_from_bson(take(&mut d, "bbox")?, "bbox")?;
  let c = as_f32(take(&mut d, "confidence")?, "confidence")?;
  let ch = chirality_from_i64(
    as_i64(take(&mut d, "chirality")?, "chirality")?,
    "chirality",
  )?;
  let joints = as_array(take(&mut d, "joints")?, "joints")?
    .into_iter()
    .map(|x| joint_from_bson(x, "joints[]"))
    .collect::<Result<Vec<_>, _>>()?;
  Ok(HandPoseDetection::try_new(bbox, c, ch, joints)?)
}

fn body_pose_3d_to_bson(p: &BodyPose3DDetection) -> Bson {
  let mut d = Document::new();
  d.insert("confidence", Bson::Double(p.confidence() as f64));
  d.insert("body_height", Bson::Double(p.body_height() as f64));
  d.insert(
    "height_estimation",
    Bson::Int32(height_est_to_i32(p.height_estimation())),
  );
  d.insert(
    "joints",
    Bson::Array(p.joints_slice().iter().map(joint3d_to_bson).collect()),
  );
  Bson::Document(d)
}

fn body_pose_3d_from_bson(b: Bson, field: &'static str) -> Result<BodyPose3DDetection, MongoError> {
  let mut d = as_doc(b, field)?;
  let c = as_f32(take(&mut d, "confidence")?, "confidence")?;
  let bh = as_f32(take(&mut d, "body_height")?, "body_height")?;
  let he = height_est_from_i64(
    as_i64(take(&mut d, "height_estimation")?, "height_estimation")?,
    "height_estimation",
  )?;
  let joints = as_array(take(&mut d, "joints")?, "joints")?
    .into_iter()
    .map(|x| joint3d_from_bson(x, "joints[]"))
    .collect::<Result<Vec<_>, _>>()?;
  Ok(BodyPose3DDetection::try_new(c, bh, he, joints)?)
}

fn subject_to_bson(s: &SubjectDetection) -> Bson {
  let mut d = Document::new();
  d.insert("detection", detection_to_bson(s.detection_ref()));
  d.insert("bbox", bbox_to_bson(s.bbox_ref()));
  Bson::Document(d)
}

fn subject_from_bson(b: Bson, field: &'static str) -> Result<SubjectDetection, MongoError> {
  let mut d = as_doc(b, field)?;
  let det = detection_from_bson(take(&mut d, "detection")?, "detection")?;
  let bbox = bbox_from_bson(take(&mut d, "bbox")?, "bbox")?;
  Ok(SubjectDetection::new(det, bbox))
}

fn face_to_bson(f: &FaceDetection) -> Bson {
  let mut d = Document::new();
  d.insert("bbox", bbox_to_bson(f.bbox_ref()));
  d.insert("confidence", Bson::Double(f.confidence() as f64));
  d.insert("capture_quality", Bson::Double(f.capture_quality() as f64));
  d.insert("roll", Bson::Double(f.roll() as f64));
  d.insert("yaw", Bson::Double(f.yaw() as f64));
  d.insert("pitch", Bson::Double(f.pitch() as f64));
  Bson::Document(d)
}

fn face_from_bson(b: Bson, field: &'static str) -> Result<FaceDetection, MongoError> {
  let mut d = as_doc(b, field)?;
  let bbox = bbox_from_bson(take(&mut d, "bbox")?, "bbox")?;
  let c = as_f32(take(&mut d, "confidence")?, "confidence")?;
  let q = as_f32(take(&mut d, "capture_quality")?, "capture_quality")?;
  let r = as_f32(take(&mut d, "roll")?, "roll")?;
  let y = as_f32(take(&mut d, "yaw")?, "yaw")?;
  let p = as_f32(take(&mut d, "pitch")?, "pitch")?;
  Ok(FaceDetection::try_new(bbox, c, q, r, y, p)?)
}

fn flm_region_to_bson(r: &FaceLandmarkRegion) -> Bson {
  let mut d = Document::new();
  d.insert("name", Bson::String(r.name().to_owned()));
  d.insert(
    "points",
    Bson::Array(r.points().iter().copied().map(corner_to_bson).collect()),
  );
  Bson::Document(d)
}

fn flm_region_from_bson(b: Bson, field: &'static str) -> Result<FaceLandmarkRegion, MongoError> {
  let mut d = as_doc(b, field)?;
  let name = as_smol(take(&mut d, "name")?, "name")?;
  let pts = as_array(take(&mut d, "points")?, "points")?
    .into_iter()
    .map(|x| corner_from_bson(x, "points[]"))
    .collect::<Result<Vec<_>, _>>()?;
  Ok(FaceLandmarkRegion::try_new(name, pts)?)
}

fn face_lms_to_bson(f: &FaceLandmarksDetection) -> Bson {
  let mut d = Document::new();
  d.insert("bbox", bbox_to_bson(f.bbox_ref()));
  d.insert("confidence", Bson::Double(f.confidence() as f64));
  d.insert(
    "regions",
    Bson::Array(f.regions_slice().iter().map(flm_region_to_bson).collect()),
  );
  Bson::Document(d)
}

fn face_lms_from_bson(b: Bson, field: &'static str) -> Result<FaceLandmarksDetection, MongoError> {
  let mut d = as_doc(b, field)?;
  let bbox = bbox_from_bson(take(&mut d, "bbox")?, "bbox")?;
  let c = as_f32(take(&mut d, "confidence")?, "confidence")?;
  let regions = as_array(take(&mut d, "regions")?, "regions")?
    .into_iter()
    .map(|x| flm_region_from_bson(x, "regions[]"))
    .collect::<Result<Vec<_>, _>>()?;
  Ok(FaceLandmarksDetection::try_new(bbox, c, regions)?)
}

fn person_instance_mask_to_bson(m: &PersonInstanceMaskDetection) -> Bson {
  let mut d = Document::new();
  d.insert("bbox", bbox_to_bson(m.bbox_ref()));
  d.insert("confidence", Bson::Double(m.confidence() as f64));
  d.insert("instance_index", Bson::Int64(m.instance_index() as i64));
  d.insert("dimensions", dimensions_to_bson(m.dimensions()));
  d.insert("data", bytes_to_bson(m.data()));
  Bson::Document(d)
}

fn person_instance_mask_from_bson(
  b: Bson,
  field: &'static str,
) -> Result<PersonInstanceMaskDetection, MongoError> {
  let mut d = as_doc(b, field)?;
  let bbox = bbox_from_bson(take(&mut d, "bbox")?, "bbox")?;
  let c = as_f32(take(&mut d, "confidence")?, "confidence")?;
  let idx = as_u32(take(&mut d, "instance_index")?, "instance_index")?;
  let dims = dimensions_from_bson(take(&mut d, "dimensions")?, "dimensions")?;
  let data = as_binary(take(&mut d, "data")?, "data")?;
  Ok(PersonInstanceMaskDetection::try_new(
    bbox, c, idx, dims, data,
  )?)
}

fn person_seg_mask_to_bson(m: &PersonSegmentationMask) -> Bson {
  let mut d = Document::new();
  d.insert("bbox", bbox_to_bson(m.bbox_ref()));
  d.insert("confidence", Bson::Double(m.confidence() as f64));
  d.insert("dimensions", dimensions_to_bson(m.dimensions()));
  d.insert("data", bytes_to_bson(m.data()));
  Bson::Document(d)
}

fn person_seg_mask_from_bson(
  b: Bson,
  field: &'static str,
) -> Result<PersonSegmentationMask, MongoError> {
  let mut d = as_doc(b, field)?;
  let bbox = bbox_from_bson(take(&mut d, "bbox")?, "bbox")?;
  let c = as_f32(take(&mut d, "confidence")?, "confidence")?;
  let dims = dimensions_from_bson(take(&mut d, "dimensions")?, "dimensions")?;
  let data = as_binary(take(&mut d, "data")?, "data")?;
  Ok(PersonSegmentationMask::try_new(bbox, c, dims, data)?)
}

fn human_to_bson(h: &HumanAnalysis) -> Bson {
  let mut d = Document::new();
  d.insert(
    "subjects",
    Bson::Array(h.subjects_slice().iter().map(subject_to_bson).collect()),
  );
  d.insert(
    "faces",
    Bson::Array(h.faces_slice().iter().map(face_to_bson).collect()),
  );
  d.insert(
    "body_poses",
    Bson::Array(h.body_poses_slice().iter().map(body_pose_to_bson).collect()),
  );
  d.insert(
    "hand_poses",
    Bson::Array(h.hand_poses_slice().iter().map(hand_pose_to_bson).collect()),
  );
  d.insert(
    "body_poses_3d",
    Bson::Array(
      h.body_poses_3d_slice()
        .iter()
        .map(body_pose_3d_to_bson)
        .collect(),
    ),
  );
  d.insert(
    "instance_masks",
    Bson::Array(
      h.instance_masks_slice()
        .iter()
        .map(person_instance_mask_to_bson)
        .collect(),
    ),
  );
  d.insert(
    "face_rectangles",
    Bson::Array(h.face_rectangles_slice().iter().map(face_to_bson).collect()),
  );
  d.insert(
    "face_landmarks",
    Bson::Array(
      h.face_landmarks_slice()
        .iter()
        .map(face_lms_to_bson)
        .collect(),
    ),
  );
  d.insert(
    "segmentation_masks",
    Bson::Array(
      h.segmentation_masks_slice()
        .iter()
        .map(person_seg_mask_to_bson)
        .collect(),
    ),
  );
  Bson::Document(d)
}

fn human_from_bson(b: Bson, field: &'static str) -> Result<HumanAnalysis, MongoError> {
  let mut d = as_doc(b, field)?;
  let subjects = as_array(take(&mut d, "subjects")?, "subjects")?
    .into_iter()
    .map(|x| subject_from_bson(x, "subjects[]"))
    .collect::<Result<Vec<_>, _>>()?;
  let faces = as_array(take(&mut d, "faces")?, "faces")?
    .into_iter()
    .map(|x| face_from_bson(x, "faces[]"))
    .collect::<Result<Vec<_>, _>>()?;
  let body_poses = as_array(take(&mut d, "body_poses")?, "body_poses")?
    .into_iter()
    .map(|x| body_pose_from_bson(x, "body_poses[]"))
    .collect::<Result<Vec<_>, _>>()?;
  let hand_poses = as_array(take(&mut d, "hand_poses")?, "hand_poses")?
    .into_iter()
    .map(|x| hand_pose_from_bson(x, "hand_poses[]"))
    .collect::<Result<Vec<_>, _>>()?;
  let body_poses_3d = as_array(take(&mut d, "body_poses_3d")?, "body_poses_3d")?
    .into_iter()
    .map(|x| body_pose_3d_from_bson(x, "body_poses_3d[]"))
    .collect::<Result<Vec<_>, _>>()?;
  let instance_masks = as_array(take(&mut d, "instance_masks")?, "instance_masks")?
    .into_iter()
    .map(|x| person_instance_mask_from_bson(x, "instance_masks[]"))
    .collect::<Result<Vec<_>, _>>()?;
  let face_rectangles = as_array(take(&mut d, "face_rectangles")?, "face_rectangles")?
    .into_iter()
    .map(|x| face_from_bson(x, "face_rectangles[]"))
    .collect::<Result<Vec<_>, _>>()?;
  let face_landmarks = as_array(take(&mut d, "face_landmarks")?, "face_landmarks")?
    .into_iter()
    .map(|x| face_lms_from_bson(x, "face_landmarks[]"))
    .collect::<Result<Vec<_>, _>>()?;
  let segmentation_masks = as_array(take(&mut d, "segmentation_masks")?, "segmentation_masks")?
    .into_iter()
    .map(|x| person_seg_mask_from_bson(x, "segmentation_masks[]"))
    .collect::<Result<Vec<_>, _>>()?;
  Ok(
    HumanAnalysis::new()
      .with_subjects(subjects)
      .with_faces(faces)
      .with_body_poses(body_poses)
      .with_hand_poses(hand_poses)
      .with_body_poses_3d(body_poses_3d)
      .with_instance_masks(instance_masks)
      .with_face_rectangles(face_rectangles)
      .with_face_landmarks(face_landmarks)
      .with_segmentation_masks(segmentation_masks),
  )
}

fn animal_to_bson(a: &AnimalAnalysis) -> Bson {
  let mut d = Document::new();
  d.insert(
    "subjects",
    Bson::Array(a.subjects_slice().iter().map(subject_to_bson).collect()),
  );
  d.insert(
    "body_poses",
    Bson::Array(a.body_poses_slice().iter().map(body_pose_to_bson).collect()),
  );
  Bson::Document(d)
}

fn animal_from_bson(b: Bson, field: &'static str) -> Result<AnimalAnalysis, MongoError> {
  let mut d = as_doc(b, field)?;
  let subjects = as_array(take(&mut d, "subjects")?, "subjects")?
    .into_iter()
    .map(|x| subject_from_bson(x, "subjects[]"))
    .collect::<Result<Vec<_>, _>>()?;
  let body_poses = as_array(take(&mut d, "body_poses")?, "body_poses")?
    .into_iter()
    .map(|x| body_pose_from_bson(x, "body_poses[]"))
    .collect::<Result<Vec<_>, _>>()?;
  Ok(
    AnimalAnalysis::new()
      .with_subjects(subjects)
      .with_body_poses(body_poses),
  )
}

fn dominant_color_to_bson(c: &DominantColor) -> Bson {
  let mut d = Document::new();
  d.insert("rgb", rgba_to_bson(c.rgb()));
  d.insert("name", Bson::String(c.name().to_owned()));
  d.insert("percentage", Bson::Double(c.percentage() as f64));
  d.insert("population", Bson::Int64(c.population() as i64));
  Bson::Document(d)
}

fn dominant_color_from_bson(b: Bson, field: &'static str) -> Result<DominantColor, MongoError> {
  let mut d = as_doc(b, field)?;
  let rgb = rgba_from_bson(take(&mut d, "rgb")?, "rgb")?;
  let name = as_smol(take(&mut d, "name")?, "name")?;
  let pct = as_f32(take(&mut d, "percentage")?, "percentage")?;
  let pop = as_u32(take(&mut d, "population")?, "population")?;
  Ok(DominantColor::try_new(rgb, name, pct, pop)?)
}

fn vlm_to_bson(v: &VlmAnalysis) -> Bson {
  let mut d = Document::new();
  d.insert("categories", loc_text_vec_to_bson(v.categories_slice()));
  d.insert("description", loc_text_to_bson(v.description_ref()));
  d.insert("tags", loc_text_vec_to_bson(v.tags_slice()));
  d.insert("shot_type", Bson::String(v.shot_type().to_owned()));
  d.insert("objects", loc_text_vec_to_bson(v.objects_slice()));
  d.insert("subjects", loc_text_vec_to_bson(v.subjects_slice()));
  d.insert("mood", loc_text_vec_to_bson(v.mood_slice()));
  d.insert("emotion", loc_text_vec_to_bson(v.emotion_slice()));
  d.insert("lighting", loc_text_vec_to_bson(v.lighting_slice()));
  Bson::Document(d)
}

fn vlm_from_bson(b: Bson, field: &'static str) -> Result<VlmAnalysis, MongoError> {
  let mut d = as_doc(b, field)?;
  Ok(
    VlmAnalysis::new()
      .with_categories(loc_text_vec_from_bson(
        take(&mut d, "categories")?,
        "categories",
      )?)
      .with_description(loc_text_from_bson(
        take(&mut d, "description")?,
        "description",
      )?)
      .with_tags(loc_text_vec_from_bson(take(&mut d, "tags")?, "tags")?)
      .with_shot_type(as_smol(take(&mut d, "shot_type")?, "shot_type")?)
      .with_objects(loc_text_vec_from_bson(take(&mut d, "objects")?, "objects")?)
      .with_subjects(loc_text_vec_from_bson(
        take(&mut d, "subjects")?,
        "subjects",
      )?)
      .with_mood(loc_text_vec_from_bson(take(&mut d, "mood")?, "mood")?)
      .with_emotion(loc_text_vec_from_bson(take(&mut d, "emotion")?, "emotion")?)
      .with_lighting(loc_text_vec_from_bson(
        take(&mut d, "lighting")?,
        "lighting",
      )?),
  )
}

// ===========================================================================
// Keyframe
// ===========================================================================

impl From<&Keyframe<Uuid7>> for Document {
  fn from(k: &Keyframe<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*k.id_ref()));
    d.insert("parent", uuid7_to_bson(*k.parent_ref()));
    d.insert("pts", media_ts_to_bson(*k.pts_ref()));
    d.insert("data", bytes_to_bson(k.data()));
    d.insert("mime", Bson::String(k.mime().to_owned()));
    d.insert("size", Bson::Int64(k.size() as i64));
    d.insert("dimensions", dimensions_to_bson(k.dimensions()));
    d.insert(
      "extractor",
      Bson::Int32(keyframe_extractor_to_i32(k.extractor())),
    );
    d.insert(
      "classifications",
      Bson::Array(
        k.classifications_slice()
          .iter()
          .map(detection_to_bson)
          .collect(),
      ),
    );
    d.insert(
      "objects",
      Bson::Array(
        k.objects_slice()
          .iter()
          .map(object_detection_to_bson)
          .collect(),
      ),
    );
    d.insert("humans", human_to_bson(k.humans_ref()));
    d.insert("animals", animal_to_bson(k.animals_ref()));
    d.insert(
      "actions",
      Bson::Array(
        k.actions_slice()
          .iter()
          .map(action_detection_to_bson)
          .collect(),
      ),
    );
    d.insert(
      "text_detections",
      Bson::Array(
        k.text_detections_slice()
          .iter()
          .map(text_detection_to_bson)
          .collect(),
      ),
    );
    d.insert(
      "barcodes",
      Bson::Array(k.barcodes_slice().iter().map(barcode_to_bson).collect()),
    );
    d.insert(
      "attention_saliency",
      Bson::Array(
        k.attention_saliency_slice()
          .iter()
          .map(saliency_to_bson)
          .collect(),
      ),
    );
    d.insert(
      "objectness_saliency",
      Bson::Array(
        k.objectness_saliency_slice()
          .iter()
          .map(saliency_to_bson)
          .collect(),
      ),
    );
    d.insert("horizon", horizon_to_bson(k.horizon_ref()));
    d.insert(
      "document_segments",
      Bson::Array(
        k.document_segments_slice()
          .iter()
          .map(document_segment_to_bson)
          .collect(),
      ),
    );
    d.insert("aesthetics", aesthetics_to_bson(k.aesthetics_ref()));
    d.insert(
      "colors",
      Bson::Array(
        k.colors_slice()
          .iter()
          .map(dominant_color_to_bson)
          .collect(),
      ),
    );
    d.insert("vlm", vlm_to_bson(k.vlm_ref()));
    d
  }
}

impl TryFrom<Document> for Keyframe<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let parent = uuid7_from_bson(take(&mut d, "parent")?, "parent")?;
    let pts = media_ts_from_bson(take(&mut d, "pts")?, "pts")?;
    let dimensions = dimensions_from_bson(take(&mut d, "dimensions")?, "dimensions")?;
    let extractor = keyframe_extractor_from_i64(
      as_i64(take(&mut d, "extractor")?, "extractor")?,
      "extractor",
    )?;
    let mut k = Keyframe::try_new(id, parent, pts, dimensions, extractor)?;

    if let Some(b) = take_opt(&mut d, "data") {
      k.set_data(as_binary(b, "data")?);
    }
    if let Some(b) = take_opt(&mut d, "mime") {
      k.set_mime(as_smol(b, "mime")?);
    }
    // `size` is derived from `data` (`Keyframe::size()`); it is written
    // to the document for query/projection use but never decoded — once
    // `data` is restored above, `size()` round-trips for free.
    let _ = take_opt(&mut d, "size");
    if let Some(b) = take_opt(&mut d, "classifications") {
      let v = as_array(b, "classifications")?
        .into_iter()
        .map(|x| detection_from_bson(x, "classifications[]"))
        .collect::<Result<Vec<_>, _>>()?;
      k.set_classifications(v);
    }
    if let Some(b) = take_opt(&mut d, "objects") {
      let v = as_array(b, "objects")?
        .into_iter()
        .map(|x| object_detection_from_bson(x, "objects[]"))
        .collect::<Result<Vec<_>, _>>()?;
      k.set_objects(v);
    }
    if let Some(b) = take_opt(&mut d, "humans") {
      k.set_humans(human_from_bson(b, "humans")?);
    }
    if let Some(b) = take_opt(&mut d, "animals") {
      k.set_animals(animal_from_bson(b, "animals")?);
    }
    if let Some(b) = take_opt(&mut d, "actions") {
      let v = as_array(b, "actions")?
        .into_iter()
        .map(|x| action_detection_from_bson(x, "actions[]"))
        .collect::<Result<Vec<_>, _>>()?;
      k.set_actions(v);
    }
    if let Some(b) = take_opt(&mut d, "text_detections") {
      let v = as_array(b, "text_detections")?
        .into_iter()
        .map(|x| text_detection_from_bson(x, "text_detections[]"))
        .collect::<Result<Vec<_>, _>>()?;
      k.set_text_detections(v);
    }
    if let Some(b) = take_opt(&mut d, "barcodes") {
      let v = as_array(b, "barcodes")?
        .into_iter()
        .map(|x| barcode_from_bson(x, "barcodes[]"))
        .collect::<Result<Vec<_>, _>>()?;
      k.set_barcodes(v);
    }
    if let Some(b) = take_opt(&mut d, "attention_saliency") {
      let v = as_array(b, "attention_saliency")?
        .into_iter()
        .map(|x| saliency_from_bson(x, "attention_saliency[]"))
        .collect::<Result<Vec<_>, _>>()?;
      k.set_attention_saliency(v);
    }
    if let Some(b) = take_opt(&mut d, "objectness_saliency") {
      let v = as_array(b, "objectness_saliency")?
        .into_iter()
        .map(|x| saliency_from_bson(x, "objectness_saliency[]"))
        .collect::<Result<Vec<_>, _>>()?;
      k.set_objectness_saliency(v);
    }
    if let Some(b) = take_opt(&mut d, "horizon") {
      k.set_horizon(horizon_from_bson(b, "horizon")?);
    }
    if let Some(b) = take_opt(&mut d, "document_segments") {
      let v = as_array(b, "document_segments")?
        .into_iter()
        .map(|x| document_segment_from_bson(x, "document_segments[]"))
        .collect::<Result<Vec<_>, _>>()?;
      k.set_document_segments(v);
    }
    if let Some(b) = take_opt(&mut d, "aesthetics") {
      k.set_aesthetics(aesthetics_from_bson(b, "aesthetics")?);
    }
    if let Some(b) = take_opt(&mut d, "colors") {
      let v = as_array(b, "colors")?
        .into_iter()
        .map(|x| dominant_color_from_bson(x, "colors[]"))
        .collect::<Result<Vec<_>, _>>()?;
      k.set_colors(v);
    }
    if let Some(b) = take_opt(&mut d, "vlm") {
      k.set_vlm(vlm_from_bson(b, "vlm")?);
    }
    Ok(k)
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::{
    primitives::{ErrorCode, ErrorInfo},
    vo::{LocalizedText, Provenance},
    Rgba,
  };
  use ::mediaframe::frame::Dimensions;
  use core::num::NonZeroU32;
  use mediatime::{TimeRange, Timebase, Timestamp as MediaTimestamp};

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).unwrap())
  }

  fn sp(s: i64, e: i64) -> TimeRange {
    TimeRange::new(s, e, tb())
  }

  #[test]
  fn video_facet_roundtrip() {
    let v = Video::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_tracks(vec![Uuid7::new(), Uuid7::new()])
      .with_total_scenes(7)
      .with_track_progress(VIndexProgress::try_new(2, 1, 0).unwrap());
    let doc: Document = (&v).into();
    let v2: Video<Uuid7> = doc.try_into().unwrap();
    assert_eq!(v, v2);
  }

  #[test]
  fn scene_roundtrip() {
    let s = Scene::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(0, 5_000),
      SceneDetector::Adaptive,
    )
    .unwrap()
    .with_keyframes(vec![Uuid7::new()])
    .with_description("a dog running");
    let doc: Document = (&s).into();
    let s2: Scene<Uuid7> = doc.try_into().unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn video_track_roundtrip() {
    let t = VideoTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_stream_index(Some(0))
      .with_container_track_id(Some(7))
      .with_start_pts(Some(MediaTimestamp::new(0, tb())))
      .try_with_duration(Some(MediaTimestamp::new(60_000, tb())))
      .unwrap()
      .with_codec(VideoCodec::Hevc)
      .with_profile(Some(SmolStr::from("Main10")))
      .with_level(Some(150))
      .with_bit_rate(8_000_000)
      .with_nb_frames(Some(1440))
      .with_has_b_frames(true)
      .with_closed_gop(Some(true))
      .with_bits_per_raw_sample(Some(10))
      .try_with_dimensions(Dimensions::new(3840, 2160))
      .unwrap()
      .try_with_visible_rect(Some(Rect::new(0, 0, 3840, 2160)))
      .unwrap()
      .with_sample_aspect_ratio(SampleAspectRatio::new(1, NonZeroU32::new(1).unwrap()))
      .with_pixel_format(PixelFormat::from_u32(0x0a))
      .with_color(ColorInfo::new(
        Primaries::from_u32(9),
        Transfer::from_u32(16),
        Matrix::from_u32(9),
        DynamicRange::from_u32(1),
        ChromaLocation::from_u32(2),
      ))
      .with_hdr_static(Some(HdrStaticMetadata::new(
        Some(MasteringDisplay::new(
          [
            ChromaCoord::new(34000, 16000),
            ChromaCoord::new(13250, 34500),
            ChromaCoord::new(7500, 3000),
          ],
          ChromaCoord::new(15635, 16450),
          10_000_000,
          50,
        )),
        Some(ContentLightLevel::new(4000, 400)),
      )))
      .with_rotation(Rotation::from_u32(0))
      .with_frame_rate(FrameRate::new(
        Rational::new(24_000, NonZeroU32::new(1001).unwrap()),
        false,
      ))
      .with_field_order(FieldOrder::from_u32(0))
      .with_stereo_mode(Some(StereoMode::from_u32(0)))
      .with_dovi(Some(DolbyVisionConfig::new(8, 9, true, false, 1)))
      .with_has_embedded_captions(true)
      .with_disposition(TrackDisposition::from_u32(0x21))
      .with_is_primary(true)
      .with_auto_selected(false)
      .with_scenes(vec![Uuid7::new()])
      .with_index_status(VideoIndexStatus::PROBED)
      .with_index_errors(vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad")])
      .with_provenance(Provenance::from_parts("qwen", "v1", "p", "idx"));
    let doc: Document = (&t).into();
    let t2: VideoTrack<Uuid7> = doc.try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn keyframe_roundtrip() {
    let k = Keyframe::try_new(
      Uuid7::new(),
      Uuid7::new(),
      MediaTimestamp::new(1234, tb()),
      Dimensions::new(320, 180),
      KeyframeExtractor::CompositeQuality,
    )
    .unwrap()
    .with_mime("image/jpeg")
    .with_data(vec![0xff, 0xd8, 0xff])
    .with_classifications(vec![Detection::try_new("dog", 0.97).unwrap()])
    .with_objects(vec![ObjectDetection::new(
      Detection::try_new("dog", 0.95).unwrap(),
      Some(BoundingBox::try_new(0.1, 0.2, 0.3, 0.4).unwrap()),
    )])
    .with_humans(HumanAnalysis::new().with_faces(vec![FaceDetection::try_new(
      BoundingBox::try_new(0.0, 0.0, 0.5, 0.5).unwrap(),
      0.9,
      0.8,
      0.0,
      0.0,
      0.0,
    )
    .unwrap()]))
    .with_animals(AnimalAnalysis::new())
    .with_actions(vec![ActionDetection::new(
      Detection::try_new("run", 0.6).unwrap(),
    )])
    .with_text_detections(vec![TextDetection::try_new(
      "hello",
      0.92,
      BoundingBox::try_new(0.0, 0.0, 1.0, 1.0).unwrap(),
    )
    .unwrap()])
    .with_barcodes(vec![BarcodeDetection::try_new(
      "abc",
      "qr",
      0.99,
      BoundingBox::try_new(0.0, 0.0, 0.1, 0.1).unwrap(),
    )
    .unwrap()])
    .with_attention_saliency(vec![SaliencyRegion::try_new(
      BoundingBox::try_new(0.0, 0.0, 0.2, 0.2).unwrap(),
      0.5,
    )
    .unwrap()])
    .with_horizon(HorizonInfo::try_new(0.05, 0.99).unwrap())
    .with_aesthetics(Aesthetics::new(0.8, false))
    .with_colors(vec![DominantColor::try_new(
      Rgba::from_components(0xff, 0x80, 0x00, 0xff),
      "orange",
      35.0,
      1024,
    )
    .unwrap()])
    .with_vlm(
      VlmAnalysis::new()
        .with_description(LocalizedText::from_src("a dog running"))
        .with_tags(vec![LocalizedText::from_src("dog")])
        .with_shot_type("medium-shot"),
    );
    let doc: Document = (&k).into();
    let k2: Keyframe<Uuid7> = doc.try_into().unwrap();
    assert_eq!(k, k2);
  }

  #[test]
  fn keyframe_zero_dimensions_rejected() {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(Uuid7::new()));
    d.insert("parent", uuid7_to_bson(Uuid7::new()));
    d.insert("pts", media_ts_to_bson(MediaTimestamp::new(0, tb())));
    d.insert("dimensions", dimensions_to_bson(Dimensions::new(0, 0)));
    d.insert("extractor", Bson::Int32(9));
    let err = Keyframe::<Uuid7>::try_from(d).unwrap_err();
    assert!(err.is_keyframe());
  }
}
