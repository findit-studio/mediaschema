//! SQLite row shapes for the attachment-cluster aggregates: the
//! `Attachment` facet and `AttachmentTrack` (+ the `metadata` /
//! `index_errors` child tables).
//!
//! UUIDs ride as 16-byte `BLOB`s. Integer-affinity columns ride as `i64`.
//! `codec` / `filename` / `mimetype` ride as `TEXT` slugs; `disposition` /
//! `index_status` bitflags ride as integers. The reserved `blob_*` columns
//! are always NULL in v1. Collections ride in child tables
//! (`attachment_track_metadata`, `attachment_track_index_error`) with an
//! `ordinal` order column. The `Vec<Id>` reverse-FK `tracks` field is NOT
//! stored.

use std::vec::Vec;

use indexmap::IndexMap;
use mediaframe::disposition::TrackDisposition;
use smol_str::SmolStr;

use crate::{
  domain::{
    aggregates::attachment::{AttachmentError, AttachmentTrackError, BlobRef},
    vo::IndexProgress,
    Attachment, AttachmentIndexStatus, AttachmentTrack, ErrorCode, ErrorInfo, Uuid7,
  },
  sqlx::{dto::bytes_to_uuid7, SqlxError},
};

// ===========================================================================
// Attachment facet
// ===========================================================================

/// SQLite row shape for the [`Attachment`] facet.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteAttachmentRow {
  pub id: Vec<u8>,
  pub media_id: Vec<u8>,
  pub track_progress_total: i64,
  pub track_progress_indexed: i64,
  pub track_progress_failed: i64,
}

impl From<&Attachment<Uuid7>> for SqliteAttachmentRow {
  fn from(a: &Attachment<Uuid7>) -> Self {
    let p = a.track_progress_ref();
    Self {
      id: a.id_ref().as_bytes().to_vec(),
      media_id: a.media_id_ref().as_bytes().to_vec(),
      track_progress_total: i64::from(p.total()),
      track_progress_indexed: i64::from(p.indexed()),
      track_progress_failed: i64::from(p.failed()),
    }
  }
}

impl TryFrom<SqliteAttachmentRow> for Attachment<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteAttachmentRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let media_id = bytes_to_uuid7(&r.media_id)?;
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

/// SQLite row shape for [`AttachmentTrack`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteAttachmentTrackRow {
  pub id: Vec<u8>,
  pub attachment_id: Vec<u8>,
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
pub struct SqliteAttachmentTrackIndexErrorRow {
  pub attachment_track_id: Vec<u8>,
  pub ordinal: i64,
  pub code: i64,
  pub message: String,
}

/// SQLite row for `attachment_track_metadata`. Position in the per-
/// `attachment_track_id` `ordinal` sequence IS the [`IndexMap`] insertion
/// order; `attachment_track_from_rows` sorts by `ordinal` on decode.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteAttachmentTrackMetadataRow {
  pub attachment_track_id: Vec<u8>,
  pub ordinal: i64,
  pub key: String,
  pub value: String,
}

impl From<&AttachmentTrack<Uuid7>>
  for (
    SqliteAttachmentTrackRow,
    Vec<SqliteAttachmentTrackIndexErrorRow>,
    Vec<SqliteAttachmentTrackMetadataRow>,
  )
{
  fn from(t: &AttachmentTrack<Uuid7>) -> Self {
    let id = t.id_ref().as_bytes().to_vec();
    let blob = t.blob_ref();
    let row = SqliteAttachmentTrackRow {
      id: id.clone(),
      attachment_id: t.attachment_id_ref().as_bytes().to_vec(),
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
      .map(|(i, e)| SqliteAttachmentTrackIndexErrorRow {
        attachment_track_id: id.clone(),
        ordinal: i as i64,
        code: i64::from(e.code().as_u32()),
        message: e.message().to_owned(),
      })
      .collect();
    let metadata = t
      .metadata_ref()
      .iter()
      .enumerate()
      .map(|(i, (k, v))| SqliteAttachmentTrackMetadataRow {
        attachment_track_id: id.clone(),
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
    SqliteAttachmentTrackRow,
    Vec<SqliteAttachmentTrackIndexErrorRow>,
    Vec<SqliteAttachmentTrackMetadataRow>,
  )> for AttachmentTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, errors, metadata): (
      SqliteAttachmentTrackRow,
      Vec<SqliteAttachmentTrackIndexErrorRow>,
      Vec<SqliteAttachmentTrackMetadataRow>,
    ),
  ) -> Result<Self, Self::Error> {
    attachment_track_from_rows(r, errors, metadata)
  }
}

/// Reconstruct an [`AttachmentTrack`] from its row, `index_errors` rows, and
/// `metadata` rows.
pub fn attachment_track_from_rows(
  r: SqliteAttachmentTrackRow,
  mut errors: Vec<SqliteAttachmentTrackIndexErrorRow>,
  mut metadata: Vec<SqliteAttachmentTrackMetadataRow>,
) -> Result<AttachmentTrack<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(&r.id)?;
  let attachment_id = bytes_to_uuid7(&r.attachment_id)?;
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

  // Reserved BlobRef — `blob_uri IS NOT NULL` discriminates presence (v1
  // never writes it, but the door reconstructs it if a future row does).
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
    let code = u32_from_i64(e.code, "AttachmentTrack.index_error.code")?;
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
    let row: SqliteAttachmentRow = (&a).into();
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
      .with_index_errors(vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "x")])
      .with_metadata({
        let mut bag = IndexMap::new();
        bag.insert(SmolStr::from("filename"), SmolStr::from("font.ttf"));
        bag
      });
    let (row, errors, metadata): (
      SqliteAttachmentTrackRow,
      Vec<SqliteAttachmentTrackIndexErrorRow>,
      Vec<SqliteAttachmentTrackMetadataRow>,
    ) = (&t).into();
    assert!(row.blob_uri.is_none(), "blob columns are NULL in v1");
    let t2 = attachment_track_from_rows(row, errors, metadata).unwrap();
    assert_eq!(t, t2);
    assert!(t2.blob_ref().is_none());
  }

  #[test]
  fn attachment_track_roundtrip_blob_some() {
    let blob = BlobRef::try_new("file:///x.ttf", 10, "font/ttf").unwrap();
    let t = AttachmentTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_filename("x.ttf")
      .with_blob(Some(blob.clone()));
    let (row, errors, metadata): (
      SqliteAttachmentTrackRow,
      Vec<SqliteAttachmentTrackIndexErrorRow>,
      Vec<SqliteAttachmentTrackMetadataRow>,
    ) = (&t).into();
    assert_eq!(row.blob_uri.as_deref(), Some("file:///x.ttf"));
    let t2 = attachment_track_from_rows(row, errors, metadata).unwrap();
    assert_eq!(t, t2);
    assert_eq!(t2.blob_ref(), Some(&blob));
  }
}
