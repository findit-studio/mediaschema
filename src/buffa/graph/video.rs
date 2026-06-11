//! Wire ⇄ graph conversions for the video subtree: `media.v2::Video` ⇄
//! [`graph::Video`] and `media.v2::VideoTrack` ⇄ [`graph::VideoTrack`].
//!
//! **Phase A**: `media.v2` carries no `Scene` / `Keyframe` messages yet
//! (`VideoTrack` reserves field 34 for the future `scenes` list), so the
//! encoders here are `TryFrom` and reject a `graph::VideoTrack` whose
//! `scenes_slice()` is non-empty with [`BuffaError::Unsupported`].
//! Decode always lifts with an empty scene list.
//!
//! ## Field correspondence — `Video`
//!
//! | wire field                       | graph field           | notes                          |
//! | -------------------------------- | --------------------- | ------------------------------ |
//! | `id` (bytes, 16)                 | `id`                  | validating                     |
//! | `total_scenes: uint32`           | `total_scenes`        | denormalized rollup            |
//! | `track_progress: IndexProgress`  | `track_progress`      | unset ⇒ empty rollup           |
//! | `tracks: repeated VideoTrack`    | `tracks: Vec<_>`      | children embedded              |
//!
//! ## Field correspondence — `VideoTrack`
//!
//! | wire field                              | graph field                  | notes                                       |
//! | --------------------------------------- | ---------------------------- | ------------------------------------------- |
//! | `id` (bytes, 16)                        | `id`                         | validating                                  |
//! | `stream_index: optional uint32`         | `stream_index`               |                                             |
//! | `container_track_id: optional uint64`   | `container_track_id`         |                                             |
//! | `start_pts` / `duration: Timestamp`     | `Option<Timestamp>`          | mediatime extern; presence = `Some`; negative duration rejected |
//! | `codec: string`                         | `codec: VideoCodec`          | slug; total `FromStr`                       |
//! | `profile: optional string`              | `profile: Option<SmolStr>`   |                                             |
//! | `level: optional uint32`                | `level: Option<u16>`         | widened; overflow ⇒ `Unsupported`           |
//! | `bit_rate` / `nb_frames`                | same                         |                                             |
//! | `has_b_frames` / `closed_gop`           | same                         |                                             |
//! | `bits_per_raw_sample: optional uint32`  | `Option<u8>`                 | widened; overflow ⇒ `Unsupported`           |
//! | `dimensions: Dimensions`                | `dimensions`                 | extern; unset ⇒ `0x0` sentinel; validating  |
//! | `visible_rect: Rect`                    | `Option<Rect>`               | extern; validated against `dimensions`      |
//! | `sample_aspect_ratio` … `field_order`   | same                         | extern; unset ⇒ domain default              |
//! | `stereo_mode` / `dovi` / `hdr_static`   | `Option<_>`                  | extern; presence = `Some`                   |
//! | `disposition: TrackDisposition`         | `disposition`                | extern; unset ⇒ empty flags                 |
//! | `metadata: repeated KeyValue`           | `metadata: IndexMap`         | insertion order preserved                   |
//! | `index_status: uint32`                  | `index_status: VideoIndexStatus` | raw bits / `from_bits_retain`           |
//! | `index_errors: repeated ErrorInfo`      | `index_errors: Vec<_>`       |                                             |
//! | `provenance: Provenance`                | `provenance`                 | unset ⇒ empty                               |
//! | *(reserved 34)*                         | `scenes`                     | phase B; encode of non-empty scenes fails   |

use buffa::MessageField;
use mediaframe::codec::VideoCodec;

use super::{
  errors_from_wire, errors_to_wire, graph_err, id_from_wire, id_to_wire, index_progress_from_wire,
  index_progress_to_wire, metadata_from_wire, metadata_to_wire, narrow_u16, narrow_u8, opt_msg,
  provenance_from_wire, provenance_to_wire, rejected,
};
use crate::{
  buffa::error::BuffaError,
  domain::{self, Uuid7, VideoIndexStatus},
  generated::media::v2 as wire,
  graph,
};

// ---------------------------------------------------------------------------
// graph::Video ⇄ wire::Video
// ---------------------------------------------------------------------------

/// Encode the facet. Fallible only through its tracks (phase-A scenes
/// guard).
impl TryFrom<&graph::Video<Uuid7>> for wire::Video {
  type Error = BuffaError;

  fn try_from(g: &graph::Video<Uuid7>) -> Result<Self, Self::Error> {
    let tracks = g
      .tracks_slice()
      .iter()
      .map(wire::VideoTrack::try_from)
      .collect::<Result<Vec<_>, _>>()?;
    Ok(wire::Video {
      id: id_to_wire(g.id_ref()),
      total_scenes: g.total_scenes(),
      track_progress: index_progress_to_wire(g.track_progress_ref()),
      tracks,
      __buffa_unknown_fields: Default::default(),
    })
  }
}

/// Decode the facet and its track subtrees. The flat facet's `media_id`
/// is synthesized from the facet's own id (consumed by the lift).
impl TryFrom<&wire::Video> for graph::Video<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::Video) -> Result<Self, Self::Error> {
    let id = id_from_wire(&w.id, "Video.id")?;
    let tracks = w
      .tracks
      .iter()
      .map(|t| video_track_from_wire(t, id))
      .collect::<Result<Vec<_>, _>>()?;
    let flat = domain::Video::try_new(id, id)
      .map_err(rejected)?
      .with_total_scenes(w.total_scenes)
      .with_track_progress(index_progress_from_wire(&w.track_progress)?);
    graph::Video::try_from_flat(&id, flat, tracks).map_err(graph_err)
  }
}

// ---------------------------------------------------------------------------
// graph::VideoTrack ⇄ wire::VideoTrack
// ---------------------------------------------------------------------------

/// Encode one track. **Phase-A guard**: a track carrying scenes cannot
/// be represented on the wire yet and fails with
/// [`BuffaError::Unsupported`].
impl TryFrom<&graph::VideoTrack<Uuid7>> for wire::VideoTrack {
  type Error = BuffaError;

  fn try_from(g: &graph::VideoTrack<Uuid7>) -> Result<Self, Self::Error> {
    if !g.scenes_slice().is_empty() {
      return Err(BuffaError::Unsupported {
        context: "VideoTrack.scenes: media.v2 phase A has no scenes field",
      });
    }
    Ok(wire::VideoTrack {
      id: id_to_wire(g.id_ref()),
      stream_index: g.stream_index(),
      container_track_id: g.container_track_id(),
      start_pts: opt_msg(g.start_pts_ref().copied()),
      duration: opt_msg(g.duration_ref().copied()),
      codec: g.codec_ref().as_str().into(),
      profile: g.profile().map(Into::into),
      level: g.level().map(u32::from),
      bit_rate: g.bit_rate(),
      nb_frames: g.nb_frames(),
      has_b_frames: g.has_b_frames(),
      closed_gop: g.closed_gop(),
      bits_per_raw_sample: g.bits_per_raw_sample().map(u32::from),
      dimensions: MessageField::some(g.dimensions()),
      visible_rect: opt_msg(g.visible_rect()),
      sample_aspect_ratio: MessageField::some(g.sample_aspect_ratio()),
      pixel_format: MessageField::some(g.pixel_format()),
      color: MessageField::some(*g.color_ref()),
      hdr_static: opt_msg(g.hdr_static_ref().copied()),
      rotation: MessageField::some(g.rotation()),
      frame_rate: MessageField::some(g.frame_rate()),
      avg_frame_rate: MessageField::some(g.avg_frame_rate()),
      field_order: MessageField::some(g.field_order()),
      stereo_mode: opt_msg(g.stereo_mode()),
      dovi: opt_msg(g.dovi()),
      has_embedded_captions: g.has_embedded_captions(),
      disposition: MessageField::some(g.disposition()),
      is_primary: g.is_primary(),
      auto_selected: g.auto_selected(),
      metadata: metadata_to_wire(g.metadata_ref()),
      index_status: g.index_status().bits(),
      index_errors: errors_to_wire(g.index_errors_slice()),
      provenance: provenance_to_wire(g.provenance_ref()),
      __buffa_unknown_fields: Default::default(),
    })
  }
}

/// Decode one track under the given parent facet id: flat builder chain
/// first (`try_with_dimensions` before `try_with_visible_rect` — the
/// crop is validated against the coded frame), then the lift with an
/// empty scene list (phase A).
fn video_track_from_wire(
  w: &wire::VideoTrack,
  video_id: Uuid7,
) -> Result<graph::VideoTrack<Uuid7>, BuffaError> {
  let id = id_from_wire(&w.id, "VideoTrack.id")?;
  let Ok(codec) = w.codec.as_str().parse::<VideoCodec>();
  let level = w
    .level
    .map(|v| narrow_u16(v, "VideoTrack.level: u16"))
    .transpose()?;
  let bits_per_raw_sample = w
    .bits_per_raw_sample
    .map(|v| narrow_u8(v, "VideoTrack.bits_per_raw_sample: u8"))
    .transpose()?;

  let mut t = domain::VideoTrack::try_new(id, video_id)
    .map_err(rejected)?
    .with_stream_index(w.stream_index)
    .with_container_track_id(w.container_track_id)
    .with_start_pts(w.start_pts.as_option().copied())
    .try_with_duration(w.duration.as_option().copied())
    .map_err(rejected)?
    .with_codec(codec)
    .with_profile(w.profile.clone())
    .with_level(level)
    .with_bit_rate(w.bit_rate)
    .with_nb_frames(w.nb_frames)
    .with_has_b_frames(w.has_b_frames)
    .with_closed_gop(w.closed_gop)
    .with_bits_per_raw_sample(bits_per_raw_sample);
  if let Some(v) = w.dimensions.as_option() {
    t = t.try_with_dimensions(*v).map_err(rejected)?;
  }
  if let Some(v) = w.visible_rect.as_option() {
    t = t.try_with_visible_rect(Some(*v)).map_err(rejected)?;
  }
  if let Some(v) = w.sample_aspect_ratio.as_option() {
    t = t.with_sample_aspect_ratio(*v);
  }
  if let Some(v) = w.pixel_format.as_option() {
    t = t.with_pixel_format(*v);
  }
  if let Some(v) = w.color.as_option() {
    t = t.with_color(*v);
  }
  if let Some(v) = w.rotation.as_option() {
    t = t.with_rotation(*v);
  }
  if let Some(v) = w.frame_rate.as_option() {
    t = t.with_frame_rate(*v);
  }
  if let Some(v) = w.avg_frame_rate.as_option() {
    t = t.with_avg_frame_rate(*v);
  }
  if let Some(v) = w.field_order.as_option() {
    t = t.with_field_order(*v);
  }
  if let Some(v) = w.disposition.as_option() {
    t = t.with_disposition(*v);
  }
  t = t
    .with_hdr_static(w.hdr_static.as_option().copied())
    .with_stereo_mode(w.stereo_mode.as_option().copied())
    .with_dovi(w.dovi.as_option().copied())
    .with_has_embedded_captions(w.has_embedded_captions)
    .with_is_primary(w.is_primary)
    .with_auto_selected(w.auto_selected)
    .with_metadata(metadata_from_wire(&w.metadata))
    .with_index_status(VideoIndexStatus::from_bits_retain(w.index_status))
    .with_index_errors(errors_from_wire(&w.index_errors))
    .with_provenance(provenance_from_wire(&w.provenance));

  graph::VideoTrack::try_from_flat(&video_id, t, Vec::new()).map_err(graph_err)
}

/// Standalone decode — the parent FK is synthesized from the track's
/// own id and consumed by the lift.
impl TryFrom<&wire::VideoTrack> for graph::VideoTrack<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::VideoTrack) -> Result<Self, Self::Error> {
    let synthetic_parent = id_from_wire(&w.id, "VideoTrack.id")?;
    video_track_from_wire(w, synthetic_parent)
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use core::num::NonZeroU32;

  use mediaframe::frame::Dimensions;
  use mediatime::{TimeRange, Timebase, Timestamp};
  use smol_str::SmolStr;

  use super::*;
  use crate::domain::{ErrorCode, ErrorInfo, IndexProgress, Provenance, SceneDetector};

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).expect("nonzero"))
  }

  fn span() -> TimeRange {
    TimeRange::new(0, 1000, tb())
  }

  fn rich_track(video_id: Uuid7) -> domain::VideoTrack<Uuid7> {
    domain::VideoTrack::try_new(Uuid7::new(), video_id)
      .expect("valid track")
      .with_stream_index(Some(0))
      .with_container_track_id(Some(1))
      .with_start_pts(Some(Timestamp::new(0, tb())))
      .try_with_duration(Some(Timestamp::new(42_000, tb())))
      .expect("valid duration")
      .with_codec("hevc".parse().expect("total"))
      .with_profile(Some(SmolStr::from("Main 10")))
      .with_level(Some(120))
      .with_bit_rate(8_000_000)
      .with_nb_frames(Some(1_008))
      .with_has_b_frames(true)
      .with_closed_gop(Some(false))
      .with_bits_per_raw_sample(Some(10))
      .try_with_dimensions(Dimensions::new(3840, 2160))
      .expect("valid dims")
      .with_has_embedded_captions(false)
      .with_is_primary(true)
      .with_auto_selected(true)
      .with_index_status(VideoIndexStatus::PROBED)
      .with_index_errors(vec![ErrorInfo::new(ErrorCode::Cancelled, "stopped")])
      .with_provenance(Provenance::from_parts("ffprobe", "7.0", "", "indexer-0.1"))
  }

  #[test]
  fn video_track_round_trips() {
    let video_id = Uuid7::new();
    let g =
      graph::VideoTrack::try_from_flat(&video_id, rich_track(video_id), vec![]).expect("coherent");
    let w = wire::VideoTrack::try_from(&g).expect("encodes");
    let g2 = graph::VideoTrack::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }

  #[test]
  fn video_facet_round_trips() {
    let media_id = Uuid7::new();
    let facet = domain::Video::try_new(Uuid7::new(), media_id)
      .expect("valid facet")
      .with_total_scenes(3)
      .with_track_progress(IndexProgress::try_new(2, 1, 1).expect("valid rollup"));
    let facet_id = *facet.id_ref();
    let track =
      graph::VideoTrack::try_from_flat(&facet_id, rich_track(facet_id), vec![]).expect("coherent");
    let g = graph::Video::try_from_flat(&media_id, facet, vec![track]).expect("coherent");
    let w = wire::Video::try_from(&g).expect("encodes");
    let g2 = graph::Video::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }

  /// Phase-A guard: a `graph::VideoTrack` lifted with a scene cannot be
  /// encoded while the wire surface has no `scenes` field.
  #[test]
  fn video_track_with_scene_encode_is_unsupported() {
    let video_id = Uuid7::new();
    let track = domain::VideoTrack::try_new(Uuid7::new(), video_id).expect("valid track");
    let track_id = *track.id_ref();
    let scene = domain::Scene::try_new(Uuid7::new(), track_id, 0, span(), SceneDetector::Manual)
      .expect("valid scene");
    let lifted_scene = graph::Scene::try_from_flat(&track_id, scene, vec![]).expect("coherent");
    let g =
      graph::VideoTrack::try_from_flat(&video_id, track, vec![lifted_scene]).expect("coherent");

    let err = wire::VideoTrack::try_from(&g).unwrap_err();
    assert!(err.is_unsupported());
    assert!(matches!(err, BuffaError::Unsupported { context } if context.contains("scenes")));

    // The guard propagates through the facet encoder too.
    let media_id = Uuid7::new();
    let facet = domain::Video::try_new(video_id, media_id).expect("valid facet");
    let g_facet = graph::Video::try_from_flat(&media_id, facet, vec![g]).expect("coherent");
    assert!(wire::Video::try_from(&g_facet)
      .unwrap_err()
      .is_unsupported());
  }

  #[test]
  fn video_track_level_overflow_errors() {
    let video_id = Uuid7::new();
    let g =
      graph::VideoTrack::try_from_flat(&video_id, rich_track(video_id), vec![]).expect("coherent");
    let mut w = wire::VideoTrack::try_from(&g).expect("encodes");
    w.level = Some(0x1_0000);
    let err = graph::VideoTrack::try_from(&w).unwrap_err();
    assert!(err.is_unsupported());
  }
}
