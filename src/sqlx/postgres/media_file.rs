//! PostgreSQL row shape for the `MediaFile` aggregate.
//!
//! Identity / FK columns are native `uuid`. The structured
//! [`Location`] is flattened to a `location_volume`
//! (`uuid`) plus a `location_path` (`text`) — the path components joined by
//! `/`. Path segments never contain `/`, so the join is lossless and the
//! column stays prefix-queryable. Wall-clock `created_at` is `BIGINT`
//! milliseconds-since-epoch, NULL = absent (no filesystem birth time).
//!
//! `UNIQUE (location_volume, location_path)` enforces one-copy-per-path
//! directly on the path column — postgres UNIQUE-indexes `TEXT` natively
//! and needs no fixed-width hash sidecar (the mysql dialect carries one).

use std::vec::Vec;

use uuid::Uuid;

use crate::{
  domain::{Location, MediaFile, Uuid7},
  sqlx::{
    dto::{millis_to_timestamp, timestamp_to_millis, uuid7_to_uuid, uuid_to_uuid7},
    SqlxError,
  },
};

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgMediaFileRow {
  pub id: Uuid,
  pub media_id: Uuid,
  /// Filesystem creation time, ms-since-epoch; NULL = absent.
  pub created_at_ms: Option<i64>,
  /// `Location::Local` volume identity.
  pub location_volume: Uuid,
  /// `Location::Local` path components joined by `/`.
  pub location_path: String,
  pub watched_location_id: Uuid,
  pub watch_volume: Uuid,
}

/// Join a `Location`'s path components with `/` for storage.
fn location_path(location: &Location<Uuid7>) -> String {
  match location {
    Location::Local(local) => local
      .components_slice()
      .iter()
      .map(AsRef::as_ref)
      .collect::<Vec<&str>>()
      .join("/"),
  }
}

impl From<&MediaFile<Uuid7>> for PgMediaFileRow {
  fn from(f: &MediaFile<Uuid7>) -> Self {
    let location_volume = match f.location_ref() {
      Location::Local(local) => *local.volume_ref(),
    };
    Self {
      id: uuid7_to_uuid(*f.id_ref()),
      media_id: uuid7_to_uuid(*f.media_id_ref()),
      created_at_ms: f.created_at_ref().map(|t| timestamp_to_millis(*t)),
      location_volume: uuid7_to_uuid(location_volume),
      location_path: location_path(f.location_ref()),
      watched_location_id: uuid7_to_uuid(*f.watched_location_id_ref()),
      watch_volume: uuid7_to_uuid(*f.watch_volume_ref()),
    }
  }
}

impl TryFrom<PgMediaFileRow> for MediaFile<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgMediaFileRow) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let media_id = uuid_to_uuid7(r.media_id)?;
    let location_volume = uuid_to_uuid7(r.location_volume)?;
    let watched_location_id = uuid_to_uuid7(r.watched_location_id)?;
    let watch_volume = uuid_to_uuid7(r.watch_volume)?;
    let created_at = match r.created_at_ms {
      None => None,
      Some(ms) => Some(millis_to_timestamp(ms)?),
    };
    let location = Location::try_local_uuid7(location_volume, r.location_path.split('/'))
      .map_err(|e| SqlxError::DomainConstructorRejected(format!("MediaFile.location: {e}")))?;
    Ok(MediaFile::from_parts(
      id,
      media_id,
      created_at,
      location,
      watched_location_id,
      watch_volume,
    ))
  }
}

/// Borrowed view of [`PgMediaFileRow`] — zero-copy decode from `&'r Row`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgMediaFileRowRef<'r> {
  pub id: Uuid,
  pub media_id: Uuid,
  pub created_at_ms: Option<i64>,
  pub location_volume: Uuid,
  pub location_path: &'r str,
  pub watched_location_id: Uuid,
  pub watch_volume: Uuid,
}

impl PgMediaFileRow {
  /// Cheap borrow — produces a [`PgMediaFileRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgMediaFileRowRef<'_> {
    PgMediaFileRowRef {
      id: self.id,
      media_id: self.media_id,
      created_at_ms: self.created_at_ms,
      location_volume: self.location_volume,
      location_path: &self.location_path,
      watched_location_id: self.watched_location_id,
      watch_volume: self.watch_volume,
    }
  }
}

impl<'r> TryFrom<PgMediaFileRowRef<'r>> for MediaFile<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgMediaFileRowRef<'r>) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let media_id = uuid_to_uuid7(r.media_id)?;
    let location_volume = uuid_to_uuid7(r.location_volume)?;
    let watched_location_id = uuid_to_uuid7(r.watched_location_id)?;
    let watch_volume = uuid_to_uuid7(r.watch_volume)?;
    let created_at = match r.created_at_ms {
      None => None,
      Some(ms) => Some(millis_to_timestamp(ms)?),
    };
    let location = Location::try_local_uuid7(location_volume, r.location_path.split('/'))
      .map_err(|e| SqlxError::DomainConstructorRejected(format!("MediaFile.location: {e}")))?;
    Ok(MediaFile::from_parts(
      id,
      media_id,
      created_at,
      location,
      watched_location_id,
      watch_volume,
    ))
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::WatchedLocation;
  use jiff::Timestamp as JiffTimestamp;

  #[test]
  fn media_file_roundtrip() {
    let vol = Uuid7::new();
    let wl = WatchedLocation::try_new(Uuid7::new(), vol, JiffTimestamp::default()).unwrap();
    let location = Location::try_local_uuid7(vol, ["Movies", "2024", "clip.mp4"]).unwrap();
    let f = MediaFile::try_new(
      Uuid7::new(),
      Uuid7::new(),
      Some(JiffTimestamp::from_millisecond(1_700_000_000_000).unwrap()),
      location,
      &wl,
    )
    .unwrap();
    let row: PgMediaFileRow = (&f).into();
    let f2: MediaFile<Uuid7> = row.try_into().unwrap();
    assert_eq!(f, f2);
    assert_eq!(f2.name(), "clip.mp4");
  }

  #[test]
  fn media_file_ref_roundtrip() {
    let vol = Uuid7::new();
    let wl = WatchedLocation::try_new(Uuid7::new(), vol, JiffTimestamp::default()).unwrap();
    let location = Location::try_local_uuid7(vol, ["Movies", "2024", "clip.mp4"]).unwrap();
    let f = MediaFile::try_new(
      Uuid7::new(),
      Uuid7::new(),
      Some(JiffTimestamp::from_millisecond(1_700_000_000_000).unwrap()),
      location,
      &wl,
    )
    .unwrap();
    let row: PgMediaFileRow = (&f).into();
    let f2: MediaFile<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(f, f2);
  }

  #[test]
  fn media_file_roundtrip_without_created_at() {
    let vol = Uuid7::new();
    let wl = WatchedLocation::try_new(Uuid7::new(), vol, JiffTimestamp::default()).unwrap();
    let location = Location::try_local_uuid7(vol, ["loose.mkv"]).unwrap();
    let f = MediaFile::try_new(Uuid7::new(), Uuid7::new(), None, location, &wl).unwrap();
    let row: PgMediaFileRow = (&f).into();
    assert!(row.created_at_ms.is_none());
    let f2: MediaFile<Uuid7> = row.try_into().unwrap();
    assert_eq!(f, f2);
  }
}
