//! Data subtree: facet → tracks. Standalone field owners — no embedded
//! flat aggregates, no parent FKs, no id-vecs. Presence-only: a data track
//! has no analysis children below it.

use indexmap::IndexMap;
use mediaframe::disposition::TrackDisposition;
use mediatime::Timestamp;
use smol_str::SmolStr;

use super::{parent_check, GraphError, NodeKind};
use crate::domain::{
  self,
  aggregates::data::{facet::DataParts, track::DataTrackParts},
  DataIndexStatus, ErrorInfo, IndexProgress, Uuid7,
};

/// The data facet with its complete track subtrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Data<Id = Uuid7> {
  id: Id,
  track_progress: IndexProgress,
  tracks: Vec<DataTrack<Id>>,
}

impl Data<Uuid7> {
  /// Lift the flat facet; validates `media_id == expected_media`. Tracks
  /// arrive pre-lifted (their `data_id` was consumed by their lift).
  pub fn try_from_flat(
    expected_media: &Uuid7,
    facet: domain::Data<Uuid7>,
    tracks: Vec<DataTrack<Uuid7>>,
  ) -> Result<Self, GraphError> {
    let DataParts {
      id,
      media_id,
      track_progress,
      tracks: _,
    } = facet.into_parts();
    parent_check(NodeKind::DataFacet, id, &media_id, expected_media)?;
    Ok(Self {
      id,
      track_progress,
      tracks,
    })
  }
}

impl<Id> Data<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  #[inline(always)]
  pub const fn track_progress_ref(&self) -> &IndexProgress {
    &self.track_progress
  }

  /// The track subtrees, in container stream order.
  #[inline(always)]
  pub const fn tracks_slice(&self) -> &[DataTrack<Id>] {
    self.tracks.as_slice()
  }
}

/// One data track — every field of the flat `DataTrack` except `data_id`
/// (implied by nesting).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTrack<Id = Uuid7> {
  id: Id,
  stream_index: Option<u32>,
  container_track_id: Option<u64>,
  codec: SmolStr,
  codec_tag: SmolStr,
  start_pts: Option<Timestamp>,
  duration: Option<Timestamp>,
  nb_packets: Option<u64>,
  byte_size: u64,
  disposition: TrackDisposition,
  metadata: IndexMap<SmolStr, SmolStr>,
  index_status: DataIndexStatus,
  index_errors: Vec<ErrorInfo>,
}

impl DataTrack<Uuid7> {
  /// Lift the flat track; validates `data_id == expected_data`.
  pub fn try_from_flat(
    expected_data: &Uuid7,
    track: domain::DataTrack<Uuid7>,
  ) -> Result<Self, GraphError> {
    let DataTrackParts {
      id,
      data_id,
      stream_index,
      container_track_id,
      codec,
      codec_tag,
      start_pts,
      duration,
      nb_packets,
      byte_size,
      disposition,
      metadata,
      index_status,
      index_errors,
    } = track.into_parts();
    parent_check(NodeKind::DataTrack, id, &data_id, expected_data)?;
    Ok(Self {
      id,
      stream_index,
      container_track_id,
      codec,
      codec_tag,
      start_pts,
      duration,
      nb_packets,
      byte_size,
      disposition,
      metadata,
      index_status,
      index_errors,
    })
  }
}

impl<Id> DataTrack<Id> {
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

  /// Codec slug (`""` = unknown).
  #[inline(always)]
  pub fn codec(&self) -> &str {
    self.codec.as_str()
  }

  /// FourCC / handler tag (`""` = absent).
  #[inline(always)]
  pub fn codec_tag(&self) -> &str {
    self.codec_tag.as_str()
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
  pub const fn nb_packets(&self) -> Option<u64> {
    self.nb_packets
  }

  #[inline(always)]
  pub const fn byte_size(&self) -> u64 {
    self.byte_size
  }

  #[inline(always)]
  pub const fn disposition(&self) -> TrackDisposition {
    self.disposition
  }

  #[inline(always)]
  pub const fn metadata_ref(&self) -> &IndexMap<SmolStr, SmolStr> {
    &self.metadata
  }

  #[inline(always)]
  pub const fn index_status(&self) -> DataIndexStatus {
    self.index_status
  }

  #[inline(always)]
  pub const fn index_errors_slice(&self) -> &[ErrorInfo] {
    self.index_errors.as_slice()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn coherent_data_subtree_lifts() {
    let media_id = Uuid7::new();
    let facet = domain::Data::try_new(Uuid7::new(), media_id).expect("valid facet");
    let facet_id = *facet.id_ref();
    let track = domain::DataTrack::try_new(Uuid7::new(), facet_id)
      .expect("valid track")
      .with_codec("rtmd");
    let g_track = DataTrack::try_from_flat(&facet_id, track).expect("coherent");
    assert_eq!(g_track.codec(), "rtmd");
    let g = Data::try_from_flat(&media_id, facet, vec![g_track]).expect("coherent");
    assert_eq!(g.tracks_slice().len(), 1);
  }

  #[test]
  fn track_under_wrong_facet_is_rejected() {
    let track = domain::DataTrack::try_new(Uuid7::new(), Uuid7::new()).expect("valid track");
    let err = DataTrack::try_from_flat(&Uuid7::new(), track).expect_err("incoherent");
    assert!(matches!(
      err,
      GraphError::ParentMismatch {
        kind: NodeKind::DataTrack,
        ..
      }
    ));
  }

  #[test]
  fn facet_under_wrong_media_is_rejected() {
    let facet = domain::Data::try_new(Uuid7::new(), Uuid7::new()).expect("valid facet");
    let err = Data::try_from_flat(&Uuid7::new(), facet, vec![]).expect_err("incoherent");
    assert!(matches!(
      err,
      GraphError::ParentMismatch {
        kind: NodeKind::DataFacet,
        ..
      }
    ));
  }
}

// --- conversion traits: flat ⇄ graph ---------------------------------------

/// Trait form of [`Data::try_from_flat`] — `(expected_media, facet, tracks)`.
impl TryFrom<(Uuid7, domain::Data<Uuid7>, Vec<DataTrack<Uuid7>>)> for Data<Uuid7> {
  type Error = GraphError;

  #[inline(always)]
  fn try_from(
    (expected_media, facet, tracks): (Uuid7, domain::Data<Uuid7>, Vec<DataTrack<Uuid7>>),
  ) -> Result<Self, Self::Error> {
    Self::try_from_flat(&expected_media, facet, tracks)
  }
}

/// Re-attach to `media_id` and rebuild the flat facet; the track-id vec is
/// re-derived from the embedded tracks, which are then dropped — convert
/// them first when persisting the tree.
impl From<(Uuid7, Data<Uuid7>)> for domain::Data<Uuid7> {
  fn from((media_id, g): (Uuid7, Data<Uuid7>)) -> Self {
    let Data {
      id,
      track_progress,
      tracks,
    } = g;
    domain::Data::rehydrate(DataParts {
      id,
      media_id,
      track_progress,
      tracks: tracks.iter().map(|t| *t.id_ref()).collect(),
    })
  }
}

/// Trait form of [`DataTrack::try_from_flat`] — `(expected_data, track)`.
impl TryFrom<(Uuid7, domain::DataTrack<Uuid7>)> for DataTrack<Uuid7> {
  type Error = GraphError;

  #[inline(always)]
  fn try_from(
    (expected_data, track): (Uuid7, domain::DataTrack<Uuid7>),
  ) -> Result<Self, Self::Error> {
    Self::try_from_flat(&expected_data, track)
  }
}

/// Re-attach to `data_id` and rebuild the flat track.
impl From<(Uuid7, DataTrack<Uuid7>)> for domain::DataTrack<Uuid7> {
  fn from((data_id, g): (Uuid7, DataTrack<Uuid7>)) -> Self {
    let DataTrack {
      id,
      stream_index,
      container_track_id,
      codec,
      codec_tag,
      start_pts,
      duration,
      nb_packets,
      byte_size,
      disposition,
      metadata,
      index_status,
      index_errors,
    } = g;
    domain::DataTrack::rehydrate(DataTrackParts {
      id,
      data_id,
      stream_index,
      container_track_id,
      codec,
      codec_tag,
      start_pts,
      duration,
      nb_packets,
      byte_size,
      disposition,
      metadata,
      index_status,
      index_errors,
    })
  }
}

#[cfg(test)]
mod conv_tests {
  use super::*;

  #[test]
  fn track_round_trips_through_graph() {
    let data_id = Uuid7::new();
    let flat = domain::DataTrack::try_new(Uuid7::new(), data_id)
      .expect("valid track")
      .with_codec("gpmf")
      .with_byte_size(42);
    let lifted: DataTrack<Uuid7> = (data_id, flat.clone()).try_into().expect("coherent");
    let back: domain::DataTrack<Uuid7> = (data_id, lifted).into();
    assert_eq!(back, flat);
  }

  #[test]
  fn facet_round_trips_through_graph() {
    let media_id = Uuid7::new();
    let flat = domain::Data::try_new(Uuid7::new(), media_id)
      .expect("valid facet")
      .with_track_progress(IndexProgress::try_new(1, 1, 0).unwrap());
    let lifted: Data<Uuid7> = (media_id, flat.clone(), vec![])
      .try_into()
      .expect("coherent");
    let back: domain::Data<Uuid7> = (media_id, lifted).into();
    assert_eq!(back, flat);
  }
}
