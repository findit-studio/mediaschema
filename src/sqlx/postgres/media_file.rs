//! PostgreSQL row shape for the `MediaFile` aggregate.
//!
//! Identity / FK columns are native `uuid`. The structured
//! [`Location`](crate::domain::Location) is flattened to a `location_volume`
//! (`uuid`) plus a `location_path` (`text`) — the path components joined by
//! `/`. Path segments never contain `/`, so the join is lossless and the
//! column stays prefix-queryable. Wall-clock `created_at` is `BIGINT`
//! milliseconds-since-epoch, NULL = absent (no filesystem birth time).
//!
//! `location_path_hash` is a `BYTEA` SHA-256 of the same canonical
//! `location_path` string, computed at write time so the
//! `UNIQUE (location_volume, location_path_hash)` index can enforce
//! one-copy-per-path on a fixed-width column. It carries no domain
//! information — `TryFrom` only verifies its length (32 bytes); the
//! domain reconstructs from `location_volume` + `location_path`.

use sha2::{Digest, Sha256};
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
  /// SHA-256 of `location_path` (32 bytes); backs the
  /// `UNIQUE (location_volume, location_path_hash)` natural-key index.
  pub location_path_hash: std::vec::Vec<u8>,
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
    // Build the canonical path once and hash THAT string — guarantees
    // `location_path_hash == SHA-256(location_path)` on the row.
    let location_path = location_path(f.location_ref());
    let location_path_hash = Sha256::digest(location_path.as_bytes()).to_vec();
    Self {
      id: uuid7_to_uuid(*f.id_ref()),
      media_id: uuid7_to_uuid(*f.media_id_ref()),
      created_at_ms: f.created_at_ref().map(|t| timestamp_to_millis(*t)),
      location_volume: uuid7_to_uuid(location_volume),
      location_path,
      location_path_hash,
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
    // The hash carries no domain information; verify its width only.
    // The domain reconstructs from `location_volume` + `location_path`.
    if r.location_path_hash.len() != 32 {
      return Err(SqlxError::InvalidChecksum(format!(
        "MediaFile.location_path_hash: expected 32 bytes, got {}",
        r.location_path_hash.len()
      )));
    }
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
    // Hash must match SHA-256 of the canonical path written to the row.
    assert_eq!(row.location_path_hash.len(), 32);
    assert_eq!(
      row.location_path_hash,
      Sha256::digest(row.location_path.as_bytes()).to_vec()
    );
    let f2: MediaFile<Uuid7> = row.try_into().unwrap();
    assert_eq!(f, f2);
    assert_eq!(f2.name(), "clip.mp4");
  }

  /// SHA-256 of the literal bytes `"Movies/2024/clip.mp4"` (32 hex bytes).
  /// Locks in the canonical-path → hash mapping so accidental encoding
  /// drift in either side surfaces as a test failure.
  #[test]
  fn media_file_location_path_hash_is_deterministic() {
    let vol = Uuid7::new();
    let wl = WatchedLocation::try_new(Uuid7::new(), vol, JiffTimestamp::default()).unwrap();
    let location = Location::try_local_uuid7(vol, ["Movies", "2024", "clip.mp4"]).unwrap();
    let f = MediaFile::try_new(Uuid7::new(), Uuid7::new(), None, location, &wl).unwrap();
    let row: PgMediaFileRow = (&f).into();
    assert_eq!(row.location_path, "Movies/2024/clip.mp4");
    // `sha256("Movies/2024/clip.mp4")` — independent witness.
    let expected = Sha256::digest(b"Movies/2024/clip.mp4").to_vec();
    assert_eq!(row.location_path_hash, expected);
  }

  #[test]
  fn media_file_rejects_wrong_hash_length() {
    let vol = Uuid7::new();
    let wl = WatchedLocation::try_new(Uuid7::new(), vol, JiffTimestamp::default()).unwrap();
    let location = Location::try_local_uuid7(vol, ["clip.mp4"]).unwrap();
    let f = MediaFile::try_new(Uuid7::new(), Uuid7::new(), None, location, &wl).unwrap();
    let mut row: PgMediaFileRow = (&f).into();
    row.location_path_hash.truncate(16);
    let err = MediaFile::<Uuid7>::try_from(row).unwrap_err();
    assert!(err.is_invalid_checksum());
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
