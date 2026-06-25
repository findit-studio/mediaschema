//! MySQL row shapes for the data-cluster aggregates: the `Data` facet and
//! `DataTrack` (+ the `metadata` / `index_errors` child tables).
//!
//! UUIDs ride as 16-byte `BINARY(16)` (`Vec<u8>`). `codec` / `codec_tag`
//! ride as `VARCHAR` slugs; `disposition` / `index_status` bitflags ride as
//! `BIGINT`. Media-time values flatten to a PTS `BIGINT` + timebase num/den.
//! Collections ride in child tables (`data_track_metadata`,
//! `data_track_index_error`) with an `ordinal` order column. The `Vec<Id>`
//! reverse-FK `tracks` field is NOT stored.

use std::vec::Vec;

use indexmap::IndexMap;
use mediaframe::disposition::TrackDisposition;
use smol_str::SmolStr;

use crate::{
  domain::{
    aggregates::data::{DataError, DataTrackError},
    vo::IndexProgress,
    Data, DataIndexStatus, DataTrack, ErrorCode, ErrorInfo, Uuid7,
  },
  sqlx::{
    dto::{bytes_to_uuid7, timestamp_from_parts},
    SqlxError,
  },
};

// ===========================================================================
// Data facet
// ===========================================================================

/// MySQL row shape for the [`Data`] facet.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlDataRow {
  pub id: Vec<u8>,
  pub media_id: Vec<u8>,
  pub track_progress_total: i64,
  pub track_progress_indexed: i64,
  pub track_progress_failed: i64,
}

impl From<&Data<Uuid7>> for MySqlDataRow {
  fn from(d: &Data<Uuid7>) -> Self {
    let p = d.track_progress_ref();
    Self {
      id: d.id_ref().as_bytes().to_vec(),
      media_id: d.media_id_ref().as_bytes().to_vec(),
      track_progress_total: i64::from(p.total()),
      track_progress_indexed: i64::from(p.indexed()),
      track_progress_failed: i64::from(p.failed()),
    }
  }
}

impl TryFrom<MySqlDataRow> for Data<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: MySqlDataRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let media_id = bytes_to_uuid7(&r.media_id)?;
    let progress = IndexProgress::from_parts(
      u32_from_i64(r.track_progress_total, "Data.track_progress_total")?,
      u32_from_i64(r.track_progress_indexed, "Data.track_progress_indexed")?,
      u32_from_i64(r.track_progress_failed, "Data.track_progress_failed")?,
    );
    let d = Data::try_new(id, media_id)
      .map_err(|e: DataError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    Ok(d.with_track_progress(progress))
  }
}

// ===========================================================================
// DataTrack
// ===========================================================================

/// MySQL row shape for [`DataTrack`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlDataTrackRow {
  pub id: Vec<u8>,
  pub data_id: Vec<u8>,
  pub stream_index: Option<i64>,
  pub container_track_id: Option<i64>,
  pub codec: String,
  pub codec_tag: String,
  pub start_pts: Option<i64>,
  pub start_pts_tb_num: Option<i64>,
  pub start_pts_tb_den: Option<i64>,
  pub duration_pts: Option<i64>,
  pub duration_tb_num: Option<i64>,
  pub duration_tb_den: Option<i64>,
  pub nb_packets: Option<i64>,
  pub byte_size: i64,
  pub disposition: i64,
  pub index_status: i64,
}

/// One `data_track_index_error` child row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlDataTrackIndexErrorRow {
  pub data_track_id: Vec<u8>,
  pub ordinal: i32,
  pub code: i32,
  pub message: String,
}

/// MySQL row for `data_track_metadata`. Position in the per-`data_track_id`
/// `ordinal` sequence IS the [`IndexMap`] insertion order.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlDataTrackMetadataRow {
  pub data_track_id: Vec<u8>,
  pub ordinal: i32,
  pub key: String,
  pub value: String,
}

impl From<&DataTrack<Uuid7>>
  for (
    MySqlDataTrackRow,
    Vec<MySqlDataTrackIndexErrorRow>,
    Vec<MySqlDataTrackMetadataRow>,
  )
{
  fn from(t: &DataTrack<Uuid7>) -> Self {
    let id = t.id_ref().as_bytes().to_vec();
    let start_pts = t.start_pts_ref();
    let duration = t.duration_ref();
    let row = MySqlDataTrackRow {
      id: id.clone(),
      data_id: t.data_id_ref().as_bytes().to_vec(),
      stream_index: t.stream_index().map(i64::from),
      container_track_id: t.container_track_id().map(|v| v as i64),
      codec: t.codec().to_owned(),
      codec_tag: t.codec_tag().to_owned(),
      start_pts: start_pts.map(mediatime::Timestamp::pts),
      start_pts_tb_num: start_pts.map(|d| i64::from(d.timebase().num())),
      start_pts_tb_den: start_pts.map(|d| i64::from(d.timebase().den().get())),
      duration_pts: duration.map(mediatime::Timestamp::pts),
      duration_tb_num: duration.map(|d| i64::from(d.timebase().num())),
      duration_tb_den: duration.map(|d| i64::from(d.timebase().den().get())),
      nb_packets: t.nb_packets().map(|v| v as i64),
      byte_size: t.byte_size() as i64,
      disposition: i64::from(t.disposition().bits()),
      index_status: i64::from(t.index_status().bits()),
    };
    let errors = t
      .index_errors_slice()
      .iter()
      .enumerate()
      .map(|(i, e)| MySqlDataTrackIndexErrorRow {
        data_track_id: id.clone(),
        ordinal: i as i32,
        code: e.code().as_u32() as i32,
        message: e.message().to_owned(),
      })
      .collect();
    let metadata = t
      .metadata_ref()
      .iter()
      .enumerate()
      .map(|(i, (k, v))| MySqlDataTrackMetadataRow {
        data_track_id: id.clone(),
        ordinal: i as i32,
        key: k.as_str().to_owned(),
        value: v.as_str().to_owned(),
      })
      .collect();
    (row, errors, metadata)
  }
}

impl
  TryFrom<(
    MySqlDataTrackRow,
    Vec<MySqlDataTrackIndexErrorRow>,
    Vec<MySqlDataTrackMetadataRow>,
  )> for DataTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, errors, metadata): (
      MySqlDataTrackRow,
      Vec<MySqlDataTrackIndexErrorRow>,
      Vec<MySqlDataTrackMetadataRow>,
    ),
  ) -> Result<Self, Self::Error> {
    data_track_from_rows(r, errors, metadata)
  }
}

/// Reconstruct a [`DataTrack`] from its row, `index_errors` rows, and
/// `metadata` rows.
pub fn data_track_from_rows(
  r: MySqlDataTrackRow,
  mut errors: Vec<MySqlDataTrackIndexErrorRow>,
  mut metadata: Vec<MySqlDataTrackMetadataRow>,
) -> Result<DataTrack<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(&r.id)?;
  let data_id = bytes_to_uuid7(&r.data_id)?;
  let mut t = DataTrack::try_new(id, data_id)
    .map_err(|e: DataTrackError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_codec(r.codec)
    .with_codec_tag(r.codec_tag)
    .with_stream_index(opt_u32(r.stream_index, "DataTrack.stream_index")?)
    .with_container_track_id(r.container_track_id.map(|v| v as u64))
    .with_nb_packets(r.nb_packets.map(|v| v as u64))
    .with_byte_size(r.byte_size as u64)
    .with_disposition(TrackDisposition::from_bits_truncate(u32_from_i64(
      r.disposition,
      "DataTrack.disposition",
    )?))
    .with_index_status(DataIndexStatus::from_bits_truncate(u32_from_i64(
      r.index_status,
      "DataTrack.index_status",
    )?));

  if let Some(pts) = r.start_pts {
    let (num, den) = require_timebase(
      r.start_pts_tb_num,
      r.start_pts_tb_den,
      "DataTrack.start_pts",
    )?;
    t = t.with_start_pts(Some(timestamp_from_parts(pts, num, den)?));
  }
  if let Some(pts) = r.duration_pts {
    let (num, den) = require_timebase(r.duration_tb_num, r.duration_tb_den, "DataTrack.duration")?;
    t = t
      .try_with_duration(Some(timestamp_from_parts(pts, num, den)?))
      .map_err(track_err)?;
  }

  errors.sort_by_key(|e| e.ordinal);
  let mut infos = Vec::with_capacity(errors.len());
  for e in errors {
    let code = u32_from_i32(e.code, "DataTrack.index_error.code")?;
    infos.push(ErrorInfo::new(ErrorCode::from_u32(code), e.message));
  }
  t = t.with_index_errors(infos);

  metadata.sort_by_key(|m| m.ordinal);
  let mut bag = IndexMap::with_capacity(metadata.len());
  for entry in metadata {
    if entry.data_track_id != r.id {
      return Err(SqlxError::DomainConstructorRejected(
        "data_track_metadata.data_track_id does not match parent data_track.id".to_owned(),
      ));
    }
    bag.insert(SmolStr::from(entry.key), SmolStr::from(entry.value));
  }
  t = t.with_metadata(bag);

  Ok(t)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn track_err(e: DataTrackError) -> SqlxError {
  SqlxError::DomainConstructorRejected(e.to_string())
}

fn u32_from_i64(v: i64, what: &str) -> Result<u32, SqlxError> {
  u32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
}

fn u32_from_i32(v: i32, what: &str) -> Result<u32, SqlxError> {
  u32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
}

fn opt_u32(v: Option<i64>, what: &str) -> Result<Option<u32>, SqlxError> {
  match v {
    None => Ok(None),
    Some(x) => Ok(Some(u32_from_i64(x, what)?)),
  }
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
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;
  use core::num::NonZeroU32;
  use mediatime::{Timebase, Timestamp};

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).unwrap())
  }

  #[test]
  fn data_facet_roundtrip() {
    let d = Data::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_track_progress(IndexProgress::try_new(2, 1, 0).unwrap());
    let row: MySqlDataRow = (&d).into();
    let d2: Data<Uuid7> = row.try_into().unwrap();
    assert_eq!(d.id_ref(), d2.id_ref());
    assert_eq!(d.track_progress_ref(), d2.track_progress_ref());
  }

  #[test]
  fn data_track_roundtrip_full() {
    let t = DataTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec("rtmd")
      .with_codec_tag("rtmd")
      .with_start_pts(Some(Timestamp::new(0, tb())))
      .try_with_duration(Some(Timestamp::new(90_000, tb())))
      .unwrap()
      .with_nb_packets(Some(2_700))
      .with_byte_size(1_024)
      .with_index_status(DataIndexStatus::PROBED)
      .with_index_errors(vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "x")])
      .with_metadata({
        let mut bag = IndexMap::new();
        bag.insert(SmolStr::from("handler_name"), SmolStr::from("rtmd"));
        bag
      });
    let (row, errors, metadata): (
      MySqlDataTrackRow,
      Vec<MySqlDataTrackIndexErrorRow>,
      Vec<MySqlDataTrackMetadataRow>,
    ) = (&t).into();
    let t2 = data_track_from_rows(row, errors, metadata).unwrap();
    assert_eq!(t, t2);
  }
}
