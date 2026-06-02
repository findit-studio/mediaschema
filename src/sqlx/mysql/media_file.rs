//! MySQL row shape for the `MediaFile` aggregate.
//!
//! Identity / FK columns are `BINARY(16)` (`Vec<u8>`). The structured
//! [`Location`] is flattened to a `location_volume`
//! (`BINARY(16)`) plus a `location_path` (`TEXT`) — the path components
//! joined by `/`. Path segments never contain `/`, so the join is lossless
//! and the column stays prefix-queryable. Wall-clock `created_at` is
//! `BIGINT` milliseconds-since-epoch, NULL = absent.
//!
//! `location_path_hash` is a `BINARY(32)` SHA-256 of the same canonical
//! `location_path` string, computed at write time so the
//! `UNIQUE (location_volume, location_path_hash)` index can enforce
//! one-copy-per-path without InnoDB's prefix-length requirement on
//! variable-length `TEXT`. This column is **mysql-specific** — the pg
//! and sqlite dialects UNIQUE-index `TEXT` natively and don't carry it.
//! The hash carries no domain information — `TryFrom` only verifies its
//! length (32 bytes).

use sha2::{Digest, Sha256};

use crate::{
  domain::{Location, MediaFile, Uuid7},
  sqlx::{
    dto::{bytes_to_uuid7, millis_to_timestamp, timestamp_to_millis, uuid7_to_uuid},
    SqlxError,
  },
};

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlMediaFileRow {
  pub id: std::vec::Vec<u8>,
  pub media_id: std::vec::Vec<u8>,
  /// Filesystem creation time, ms-since-epoch; NULL = absent.
  pub created_at_ms: Option<i64>,
  /// `Location::Local` volume identity.
  pub location_volume: std::vec::Vec<u8>,
  /// `Location::Local` path components joined by `/`.
  pub location_path: String,
  /// SHA-256 of `location_path` (32 bytes); backs the
  /// `UNIQUE (location_volume, location_path_hash)` natural-key index.
  pub location_path_hash: std::vec::Vec<u8>,
  pub watched_location_id: std::vec::Vec<u8>,
  pub watch_volume: std::vec::Vec<u8>,
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

impl From<&MediaFile<Uuid7>> for MySqlMediaFileRow {
  fn from(f: &MediaFile<Uuid7>) -> Self {
    let location_volume = match f.location_ref() {
      Location::Local(local) => *local.volume_ref(),
    };
    // Build the canonical path once and hash THAT string — guarantees
    // `location_path_hash == SHA-256(location_path)` on the row.
    let location_path = location_path(f.location_ref());
    let location_path_hash = Sha256::digest(location_path.as_bytes()).to_vec();
    Self {
      id: uuid7_to_uuid(*f.id_ref()).as_bytes().to_vec(),
      media_id: uuid7_to_uuid(*f.media_id_ref()).as_bytes().to_vec(),
      created_at_ms: f.created_at_ref().map(|t| timestamp_to_millis(*t)),
      location_volume: uuid7_to_uuid(location_volume).as_bytes().to_vec(),
      location_path,
      location_path_hash,
      watched_location_id: uuid7_to_uuid(*f.watched_location_id_ref())
        .as_bytes()
        .to_vec(),
      watch_volume: uuid7_to_uuid(*f.watch_volume_ref()).as_bytes().to_vec(),
    }
  }
}

impl TryFrom<MySqlMediaFileRow> for MediaFile<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: MySqlMediaFileRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let media_id = bytes_to_uuid7(&r.media_id)?;
    let location_volume = bytes_to_uuid7(&r.location_volume)?;
    let watched_location_id = bytes_to_uuid7(&r.watched_location_id)?;
    let watch_volume = bytes_to_uuid7(&r.watch_volume)?;
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

/// Borrowed view of [`MySqlMediaFileRow`] — zero-copy decode from `&'r Row`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlMediaFileRowRef<'r> {
  pub id: &'r [u8],
  pub media_id: &'r [u8],
  pub created_at_ms: Option<i64>,
  pub location_volume: &'r [u8],
  pub location_path: &'r str,
  pub location_path_hash: &'r [u8],
  pub watched_location_id: &'r [u8],
  pub watch_volume: &'r [u8],
}

impl MySqlMediaFileRow {
  /// Cheap borrow — produces a [`MySqlMediaFileRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlMediaFileRowRef<'_> {
    MySqlMediaFileRowRef {
      id: &self.id,
      media_id: &self.media_id,
      created_at_ms: self.created_at_ms,
      location_volume: &self.location_volume,
      location_path: &self.location_path,
      location_path_hash: &self.location_path_hash,
      watched_location_id: &self.watched_location_id,
      watch_volume: &self.watch_volume,
    }
  }
}

impl<'r> TryFrom<MySqlMediaFileRowRef<'r>> for MediaFile<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: MySqlMediaFileRowRef<'r>) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
    let media_id = bytes_to_uuid7(r.media_id)?;
    let location_volume = bytes_to_uuid7(r.location_volume)?;
    let watched_location_id = bytes_to_uuid7(r.watched_location_id)?;
    let watch_volume = bytes_to_uuid7(r.watch_volume)?;
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
    let row: MySqlMediaFileRow = (&f).into();
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

  /// SHA-256 of the literal bytes `"Movies/2024/clip.mp4"` (32 bytes).
  /// Locks in the canonical-path → hash mapping so accidental encoding
  /// drift in either side surfaces as a test failure.
  #[test]
  fn media_file_location_path_hash_is_deterministic() {
    let vol = Uuid7::new();
    let wl = WatchedLocation::try_new(Uuid7::new(), vol, JiffTimestamp::default()).unwrap();
    let location = Location::try_local_uuid7(vol, ["Movies", "2024", "clip.mp4"]).unwrap();
    let f = MediaFile::try_new(Uuid7::new(), Uuid7::new(), None, location, &wl).unwrap();
    let row: MySqlMediaFileRow = (&f).into();
    assert_eq!(row.location_path, "Movies/2024/clip.mp4");
    let expected = Sha256::digest(b"Movies/2024/clip.mp4").to_vec();
    assert_eq!(row.location_path_hash, expected);
  }

  #[test]
  fn media_file_rejects_wrong_hash_length() {
    let vol = Uuid7::new();
    let wl = WatchedLocation::try_new(Uuid7::new(), vol, JiffTimestamp::default()).unwrap();
    let location = Location::try_local_uuid7(vol, ["clip.mp4"]).unwrap();
    let f = MediaFile::try_new(Uuid7::new(), Uuid7::new(), None, location, &wl).unwrap();
    let mut row: MySqlMediaFileRow = (&f).into();
    row.location_path_hash.truncate(16);
    let err = MediaFile::<Uuid7>::try_from(row).unwrap_err();
    assert!(err.is_invalid_checksum());
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
    let row: MySqlMediaFileRow = (&f).into();
    let f2: MediaFile<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(f, f2);
  }

  #[test]
  fn media_file_roundtrip_without_created_at() {
    let vol = Uuid7::new();
    let wl = WatchedLocation::try_new(Uuid7::new(), vol, JiffTimestamp::default()).unwrap();
    let location = Location::try_local_uuid7(vol, ["loose.mkv"]).unwrap();
    let f = MediaFile::try_new(Uuid7::new(), Uuid7::new(), None, location, &wl).unwrap();
    let row: MySqlMediaFileRow = (&f).into();
    assert!(row.created_at_ms.is_none());
    let f2: MediaFile<Uuid7> = row.try_into().unwrap();
    assert_eq!(f, f2);
  }
}
