//! PostgreSQL row shape for the `MediaFile` aggregate.
//!
//! Identity / FK columns are native `uuid`. The structured
//! [`Location`](crate::domain::Location) is flattened to a `location_volume`
//! (`uuid`) plus a `location_path` (`text`) — the path components joined by
//! `/`. Path segments never contain `/`, so the join is lossless and the
//! column stays prefix-queryable. Wall-clock `created_at` is `BIGINT`
//! milliseconds-since-epoch, NULL = absent (no filesystem birth time).

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
      .collect::<std::vec::Vec<&str>>()
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
