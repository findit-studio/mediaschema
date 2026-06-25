//! PostgreSQL row shapes for the attachment-cluster aggregates: the
//! `Attachment` facet and `AttachmentTrack` (+ the `metadata` /
//! `index_errors` child tables).
//!
//! Identity / FK columns are native `uuid`. `codec` / `filename` /
//! `mimetype` ride as `text` slugs; `disposition` / `index_status` bitflags
//! ride as integers. The reserved `blob_*` columns are always NULL in v1.
//! Collections ride in child tables with an `ordinal` order column. The
//! `Vec<Id>` reverse-FK `tracks` field is NOT stored.

use std::vec::Vec;

use indexmap::IndexMap;
use mediaframe::disposition::TrackDisposition;
use smol_str::SmolStr;
use uuid::Uuid;

use crate::{
  domain::{
    aggregates::attachment::{AttachmentError, AttachmentTrackError, BlobRef},
    vo::IndexProgress,
    Attachment, AttachmentIndexStatus, AttachmentTrack, ErrorCode, ErrorInfo, Uuid7,
  },
  sqlx::{
    dto::{uuid7_to_uuid, uuid_to_uuid7},
    SqlxError,
  },
};

// ===========================================================================
// Attachment facet
// ===========================================================================

/// PostgreSQL row shape for the [`Attachment`] facet.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgAttachmentRow {
  pub id: Uuid,
  pub media_id: Uuid,
  pub track_progress_total: i64,
  pub track_progress_indexed: i64,
  pub track_progress_failed: i64,
}

impl From<&Attachment<Uuid7>> for PgAttachmentRow {
  fn from(a: &Attachment<Uuid7>) -> Self {
    let p = a.track_progress_ref();
    Self {
      id: uuid7_to_uuid(*a.id_ref()),
      media_id: uuid7_to_uuid(*a.media_id_ref()),
      track_progress_total: i64::from(p.total()),
      track_progress_indexed: i64::from(p.indexed()),
      track_progress_failed: i64::from(p.failed()),
    }
  }
}

impl TryFrom<PgAttachmentRow> for Attachment<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgAttachmentRow) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let media_id = uuid_to_uuid7(r.media_id)?;
    let progress = IndexProgress::from_parts(
      u32_from_i64(r.track_progress_total, "Attachment.track_progress_total")?,
      u32_from_i64(
        r.track_progress_indexed,
        "Attachment.track_progress_indexed",
      )?,
      u32_from_i64(r.track_progress_failed, "Attachment.track_progress_failed")?,
    );
    let a = Attachment::try_new(id, media_id)
      .map_err(|e: AttachmentError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    Ok(a.with_track_progress(progress))
  }
}

// ===========================================================================
// AttachmentTrack
// ===========================================================================

/// PostgreSQL row shape for [`AttachmentTrack`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgAttachmentTrackRow {
  pub id: Uuid,
  pub attachment_id: Uuid,
  pub stream_index: Option<i64>,
  pub codec: String,
  pub filename: String,
  pub mimetype: String,
  pub byte_size: i64,
  pub disposition: i64,
  pub index_status: i64,
  /// Reserved `BlobRef` externalization handle — always NULL in v1.
  pub blob_uri: Option<String>,
  pub blob_byte_size: Option<i64>,
  pub blob_content_type: Option<String>,
}

/// One `attachment_track_index_error` child row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgAttachmentTrackIndexErrorRow {
  pub attachment_track_id: Uuid,
  pub ordinal: i32,
  pub code: i32,
  pub message: String,
}

/// PostgreSQL row for `attachment_track_metadata`. Position in the per-
/// `attachment_track_id` `ordinal` sequence IS the [`IndexMap`] insertion
/// order.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgAttachmentTrackMetadataRow {
  pub attachment_track_id: Uuid,
  pub ordinal: i32,
  pub key: String,
  pub value: String,
}

impl From<&AttachmentTrack<Uuid7>>
  for (
    PgAttachmentTrackRow,
    Vec<PgAttachmentTrackIndexErrorRow>,
    Vec<PgAttachmentTrackMetadataRow>,
  )
{
  fn from(t: &AttachmentTrack<Uuid7>) -> Self {
    let id = uuid7_to_uuid(*t.id_ref());
    let blob = t.blob_ref();
    let row = PgAttachmentTrackRow {
      id,
      attachment_id: uuid7_to_uuid(*t.attachment_id_ref()),
      stream_index: t.stream_index().map(i64::from),
      codec: t.codec().to_owned(),
      filename: t.filename().to_owned(),
      mimetype: t.mimetype().to_owned(),
      byte_size: t.byte_size() as i64,
      disposition: i64::from(t.disposition().bits()),
      index_status: i64::from(t.index_status().bits()),
      blob_uri: blob.map(|b| b.uri().to_owned()),
      blob_byte_size: blob.map(|b| b.byte_size() as i64),
      blob_content_type: blob.map(|b| b.content_type().to_owned()),
    };
    let errors = t
      .index_errors_slice()
      .iter()
      .enumerate()
      .map(|(i, e)| PgAttachmentTrackIndexErrorRow {
        attachment_track_id: id,
        ordinal: i as i32,
        code: e.code().as_u32() as i32,
        message: e.message().to_owned(),
      })
      .collect();
    let metadata = t
      .metadata_ref()
      .iter()
      .enumerate()
      .map(|(i, (k, v))| PgAttachmentTrackMetadataRow {
        attachment_track_id: id,
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
    PgAttachmentTrackRow,
    Vec<PgAttachmentTrackIndexErrorRow>,
    Vec<PgAttachmentTrackMetadataRow>,
  )> for AttachmentTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, errors, metadata): (
      PgAttachmentTrackRow,
      Vec<PgAttachmentTrackIndexErrorRow>,
      Vec<PgAttachmentTrackMetadataRow>,
    ),
  ) -> Result<Self, Self::Error> {
    attachment_track_from_rows(r, errors, metadata)
  }
}

/// Reconstruct an [`AttachmentTrack`] from its row, `index_errors` rows,
/// and `metadata` rows.
pub fn attachment_track_from_rows(
  r: PgAttachmentTrackRow,
  mut errors: Vec<PgAttachmentTrackIndexErrorRow>,
  mut metadata: Vec<PgAttachmentTrackMetadataRow>,
) -> Result<AttachmentTrack<Uuid7>, SqlxError> {
  let id = uuid_to_uuid7(r.id)?;
  let attachment_id = uuid_to_uuid7(r.attachment_id)?;
  let mut t = AttachmentTrack::try_new(id, attachment_id)
    .map_err(|e: AttachmentTrackError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_codec(r.codec)
    .with_filename(r.filename)
    .with_mimetype(r.mimetype)
    .with_stream_index(opt_u32(r.stream_index, "AttachmentTrack.stream_index")?)
    .with_byte_size(r.byte_size as u64)
    .with_disposition(TrackDisposition::from_bits_truncate(u32_from_i64(
      r.disposition,
      "AttachmentTrack.disposition",
    )?))
    .with_index_status(AttachmentIndexStatus::from_bits_truncate(u32_from_i64(
      r.index_status,
      "AttachmentTrack.index_status",
    )?));

  if let Some(uri) = r.blob_uri {
    let byte_size = u64::try_from(r.blob_byte_size.unwrap_or(0)).map_err(|e| {
      SqlxError::UnknownDiscriminant(format!("AttachmentTrack.blob_byte_size: {e}"))
    })?;
    let blob = BlobRef::try_new(uri, byte_size, r.blob_content_type.unwrap_or_default())
      .map_err(|e| SqlxError::DomainConstructorRejected(format!("BlobRef: {e}")))?;
    t = t.with_blob(Some(blob));
  }

  errors.sort_by_key(|e| e.ordinal);
  let mut infos = Vec::with_capacity(errors.len());
  for e in errors {
    let code = u32_from_i32(e.code, "AttachmentTrack.index_error.code")?;
    infos.push(ErrorInfo::new(ErrorCode::from_u32(code), e.message));
  }
  t = t.with_index_errors(infos);

  metadata.sort_by_key(|m| m.ordinal);
  let mut bag = IndexMap::with_capacity(metadata.len());
  for entry in metadata {
    if entry.attachment_track_id != r.id {
      return Err(SqlxError::DomainConstructorRejected(
        "attachment_track_metadata.attachment_track_id does not match parent attachment_track.id"
          .to_owned(),
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

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn attachment_facet_roundtrip() {
    let a = Attachment::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_track_progress(IndexProgress::try_new(2, 1, 0).unwrap());
    let row: PgAttachmentRow = (&a).into();
    let a2: Attachment<Uuid7> = row.try_into().unwrap();
    assert_eq!(a.id_ref(), a2.id_ref());
    assert_eq!(a.track_progress_ref(), a2.track_progress_ref());
  }

  #[test]
  fn attachment_track_roundtrip_blob_none() {
    let t = AttachmentTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec("ttf")
      .with_filename("font.ttf")
      .with_mimetype("font/ttf")
      .with_byte_size(4_096)
      .with_index_status(AttachmentIndexStatus::PROBED)
      .with_index_errors(vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "x")]);
    let (row, errors, metadata): (
      PgAttachmentTrackRow,
      Vec<PgAttachmentTrackIndexErrorRow>,
      Vec<PgAttachmentTrackMetadataRow>,
    ) = (&t).into();
    assert!(row.blob_uri.is_none());
    let t2 = attachment_track_from_rows(row, errors, metadata).unwrap();
    assert_eq!(t, t2);
  }
}
