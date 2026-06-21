//! SQLite row shape for the [`Thumbnail`] aggregate.
//!
//! `id` rides as a 16-byte `BLOB`. The storage discriminator `kind`
//! rides as a `TEXT` slug ([`ThumbnailKind::as_str`] / `from_str`). The
//! two mutually-exclusive payload slots are nullable: `data` (`BLOB`) is
//! populated for [`ThumbnailKind::Database`], `location` (`TEXT`) for
//! [`ThumbnailKind::FileSystem`] / [`ThumbnailKind::Remote`]. Following
//! the domain's empty-means-absent convention, an empty payload maps to
//! SQL `NULL` and back.

use std::vec::Vec;

use crate::{
  domain::{Thumbnail, ThumbnailError, ThumbnailKind, Uuid7},
  sqlx::{dto::bytes_to_uuid7, SqlxError},
};

/// SQLite row shape for [`Thumbnail`].
///
/// `data` / `location` are nullable: exactly one is populated per
/// [`ThumbnailKind`] (the other is `NULL`). `width` / `height` ride as
/// `INTEGER`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteThumbnailRow {
  pub id: Vec<u8>,
  pub kind: String,
  pub data: Option<Vec<u8>>,
  pub location: Option<String>,
  pub mime: String,
  pub width: i64,
  pub height: i64,
}

impl From<&Thumbnail<Uuid7>> for SqliteThumbnailRow {
  fn from(t: &Thumbnail<Uuid7>) -> Self {
    // Empty payload → SQL NULL (empty means absent), so only the slot
    // the `kind` actually uses is non-NULL.
    let data = (!t.data().is_empty()).then(|| t.data().to_vec());
    let location = (!t.location().is_empty()).then(|| t.location().to_owned());
    Self {
      id: t.id_ref().as_bytes().to_vec(),
      kind: t.kind().as_str().to_owned(),
      data,
      location,
      mime: t.mime().to_owned(),
      width: i64::from(t.width()),
      height: i64::from(t.height()),
    }
  }
}

impl TryFrom<SqliteThumbnailRow> for Thumbnail<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteThumbnailRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let kind = ThumbnailKind::from_str(&r.kind)
      .ok_or_else(|| SqlxError::UnknownDiscriminant(format!("ThumbnailKind slug: {}", r.kind)))?;
    let width = u32_from_i64(r.width, "Thumbnail.width")?;
    let height = u32_from_i64(r.height, "Thumbnail.height")?;
    // NULL → empty (empty means absent in the domain aggregate); the
    // per-kind invariant is re-validated by `Thumbnail::try_new`.
    let data = r.data.unwrap_or_default();
    let location = r.location.unwrap_or_default();
    Thumbnail::try_new(id, kind, data, location, r.mime, width, height)
      .map_err(|e: ThumbnailError| SqlxError::DomainConstructorRejected(e.to_string()))
  }
}

/// Narrow an `i64` row column to `u32`, surfacing an out-of-range value
/// as a typed [`SqlxError::DomainConstructorRejected`].
fn u32_from_i64(v: i64, what: &str) -> Result<u32, SqlxError> {
  u32::try_from(v)
    .map_err(|_| SqlxError::DomainConstructorRejected(format!("{what} out of u32 range: {v}")))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn database_thumbnail_roundtrip() {
    let t = Thumbnail::try_new(
      Uuid7::new(),
      ThumbnailKind::Database,
      std::vec![0xff_u8, 0xd8, 0xff],
      "",
      "image/jpeg",
      320,
      180,
    )
    .unwrap();
    let row: SqliteThumbnailRow = (&t).into();
    assert_eq!(row.kind, "database");
    assert_eq!(row.data.as_deref(), Some(&[0xff, 0xd8, 0xff][..]));
    assert_eq!(row.location, None);
    let t2 = Thumbnail::try_from(row).unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn filesystem_thumbnail_roundtrip() {
    let t = Thumbnail::try_new(
      Uuid7::new(),
      ThumbnailKind::FileSystem,
      bytes::Bytes::new(),
      "/var/thumbs/a.jpg",
      "image/jpeg",
      64,
      64,
    )
    .unwrap();
    let row: SqliteThumbnailRow = (&t).into();
    assert_eq!(row.kind, "filesystem");
    assert_eq!(row.data, None);
    assert_eq!(row.location.as_deref(), Some("/var/thumbs/a.jpg"));
    let t2 = Thumbnail::try_from(row).unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn remote_thumbnail_roundtrip() {
    let t = Thumbnail::try_new(
      Uuid7::new(),
      ThumbnailKind::Remote,
      bytes::Bytes::new(),
      "https://cdn.example/a.webp",
      "image/webp",
      128,
      72,
    )
    .unwrap();
    let row: SqliteThumbnailRow = (&t).into();
    assert_eq!(row.kind, "remote");
    let t2 = Thumbnail::try_from(row).unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn unknown_kind_slug_is_rejected() {
    let row = SqliteThumbnailRow {
      id: Uuid7::new().as_bytes().to_vec(),
      kind: "carrier_pigeon".to_owned(),
      data: Some(std::vec![1]),
      location: None,
      mime: "image/png".to_owned(),
      width: 1,
      height: 1,
    };
    assert!(matches!(
      Thumbnail::try_from(row),
      Err(SqlxError::UnknownDiscriminant(_))
    ));
  }
}
