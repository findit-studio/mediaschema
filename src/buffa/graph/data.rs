//! Wire ⇄ graph conversions for the data subtree: `media.v2::Data` ⇄
//! [`graph::Data`] and `media.v2::DataTrack` ⇄ [`graph::DataTrack`].
//!
//! Data tracks are flat presence-only streams (no Phase-B scene blocker),
//! so the encoders here are **infallible** `From<&graph::X>`.
//!
//! ## Field correspondence — `Data`
//!
//! | wire field                       | graph field          | notes                |
//! | -------------------------------- | -------------------- | -------------------- |
//! | `id` (bytes, 16)                 | `id`                 | validating           |
//! | `track_progress: IndexProgress`  | `track_progress`     | unset ⇒ empty rollup |
//! | `tracks: repeated DataTrack`     | `tracks: Vec<_>`     | children embedded    |
//!
//! ## Field correspondence — `DataTrack`
//!
//! | wire field                            | graph field                   | notes                          |
//! | ------------------------------------- | ----------------------------- | ------------------------------ |
//! | `id` (bytes, 16)                      | `id`                          | validating                     |
//! | `stream_index` / `container_track_id` | same                          |                                |
//! | `codec` / `codec_tag: string`         | `SmolStr`                     | `""` = unknown/absent          |
//! | `start_pts` / `duration: Timestamp`   | `Option<Timestamp>`           | mediatime extern; negative duration rejected |
//! | `nb_packets` / `byte_size`            | same                          |                                |
//! | `disposition: TrackDisposition`       | `disposition`                 | extern; unset ⇒ empty flags    |
//! | `metadata: repeated KeyValue`         | `metadata: IndexMap`          | insertion order preserved      |
//! | `index_status: uint32`                | `index_status: DataIndexStatus` | raw bits / `from_bits_retain` |
//! | `index_errors: repeated ErrorInfo`    | `index_errors: Vec<_>`        |                                |

use buffa::MessageField;
use smol_str::SmolStr;

use super::{
  errors_from_wire, errors_to_wire, graph_err, id_from_wire, id_to_wire, index_progress_from_wire,
  index_progress_to_wire, metadata_from_wire, metadata_to_wire, opt_msg, rejected,
};
use crate::{
  buffa::error::BuffaError,
  domain::{self, DataIndexStatus, Uuid7},
  generated::media::v2 as wire,
  graph,
};

// ---------------------------------------------------------------------------
// graph::Data ⇄ wire::Data
// ---------------------------------------------------------------------------

impl From<&graph::Data<Uuid7>> for wire::Data {
  fn from(g: &graph::Data<Uuid7>) -> Self {
    wire::Data {
      id: id_to_wire(g.id_ref()),
      track_progress: index_progress_to_wire(g.track_progress_ref()),
      tracks: g.tracks_slice().iter().map(wire::DataTrack::from).collect(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Decode the facet and its track subtrees. The flat facet's `media_id`
/// is synthesized from the facet's own id (consumed by the lift).
impl TryFrom<&wire::Data> for graph::Data<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::Data) -> Result<Self, Self::Error> {
    let id = id_from_wire(&w.id, "Data.id")?;
    let tracks = w
      .tracks
      .iter()
      .map(|t| data_track_from_wire(t, id))
      .collect::<Result<Vec<_>, _>>()?;
    let flat = domain::Data::try_new(id, id)
      .map_err(rejected)?
      .with_track_progress(index_progress_from_wire(&w.track_progress)?);
    graph::Data::try_from_flat(&id, flat, tracks).map_err(graph_err)
  }
}

// ---------------------------------------------------------------------------
// graph::DataTrack ⇄ wire::DataTrack
// ---------------------------------------------------------------------------

impl From<&graph::DataTrack<Uuid7>> for wire::DataTrack {
  fn from(g: &graph::DataTrack<Uuid7>) -> Self {
    wire::DataTrack {
      id: id_to_wire(g.id_ref()),
      stream_index: g.stream_index(),
      container_track_id: g.container_track_id(),
      codec: SmolStr::from(g.codec()),
      codec_tag: SmolStr::from(g.codec_tag()),
      start_pts: opt_msg(g.start_pts_ref().copied()),
      duration: opt_msg(g.duration_ref().copied()),
      nb_packets: g.nb_packets(),
      byte_size: g.byte_size(),
      disposition: MessageField::some(g.disposition()),
      metadata: metadata_to_wire(g.metadata_ref()),
      index_status: g.index_status().bits(),
      index_errors: errors_to_wire(g.index_errors_slice()),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Decode one track under the given parent facet id.
fn data_track_from_wire(
  w: &wire::DataTrack,
  data_id: Uuid7,
) -> Result<graph::DataTrack<Uuid7>, BuffaError> {
  let id = id_from_wire(&w.id, "DataTrack.id")?;
  let mut t = domain::DataTrack::try_new(id, data_id)
    .map_err(rejected)?
    .with_stream_index(w.stream_index)
    .with_container_track_id(w.container_track_id)
    .with_codec(w.codec.as_str())
    .with_codec_tag(w.codec_tag.as_str())
    .with_start_pts(w.start_pts.as_option().copied())
    .try_with_duration(w.duration.as_option().copied())
    .map_err(rejected)?
    .with_nb_packets(w.nb_packets)
    .with_byte_size(w.byte_size)
    .with_index_status(DataIndexStatus::from_bits_retain(w.index_status))
    .with_metadata(metadata_from_wire(&w.metadata))
    .with_index_errors(errors_from_wire(&w.index_errors));
  if let Some(v) = w.disposition.as_option() {
    t = t.with_disposition(*v);
  }
  graph::DataTrack::try_from_flat(&data_id, t).map_err(graph_err)
}

/// Standalone decode — the parent FK is synthesized from the track's own
/// id and consumed by the lift.
impl TryFrom<&wire::DataTrack> for graph::DataTrack<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::DataTrack) -> Result<Self, Self::Error> {
    let synthetic_parent = id_from_wire(&w.id, "DataTrack.id")?;
    data_track_from_wire(w, synthetic_parent)
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use core::num::NonZeroU32;

  use mediatime::{Timebase, Timestamp};

  use super::*;
  use crate::domain::{ErrorCode, ErrorInfo, IndexProgress};

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).expect("nonzero"))
  }

  fn rich_data_track(data_id: Uuid7) -> domain::DataTrack<Uuid7> {
    domain::DataTrack::try_new(Uuid7::new(), data_id)
      .expect("valid track")
      .with_stream_index(Some(3))
      .with_container_track_id(Some(7))
      .with_codec("rtmd")
      .with_codec_tag("rtmd")
      .with_start_pts(Some(Timestamp::new(0, tb())))
      .try_with_duration(Some(Timestamp::new(90_000, tb())))
      .expect("valid duration")
      .with_nb_packets(Some(2_700))
      .with_byte_size(1_024)
      .with_index_status(DataIndexStatus::PROBED)
      .with_index_errors(vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "glitch")])
      .with_metadata({
        let mut bag = indexmap::IndexMap::new();
        bag.insert(SmolStr::from("handler_name"), SmolStr::from("rtmd"));
        bag
      })
  }

  #[test]
  fn data_track_round_trips() {
    let data_id = Uuid7::new();
    let g = graph::DataTrack::try_from_flat(&data_id, rich_data_track(data_id)).expect("coherent");
    let w = wire::DataTrack::from(&g);
    let g2 = graph::DataTrack::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }

  #[test]
  fn data_facet_round_trips() {
    let media_id = Uuid7::new();
    let facet = domain::Data::try_new(Uuid7::new(), media_id)
      .expect("valid facet")
      .with_track_progress(IndexProgress::try_new(1, 1, 0).expect("valid rollup"));
    let facet_id = *facet.id_ref();
    let track =
      graph::DataTrack::try_from_flat(&facet_id, rich_data_track(facet_id)).expect("coherent");
    let g = graph::Data::try_from_flat(&media_id, facet, vec![track]).expect("coherent");
    let w = wire::Data::from(&g);
    let g2 = graph::Data::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }
}
