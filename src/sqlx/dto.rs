//! Shared row-mapping helpers for the `sqlx` backend.
//!
//! The domain types in `src/domain/` deliberately do **not** derive
//! serde — domain validation flows through `try_new`/`with_*`, and
//! cross-format wire conversion is handled by the buffa codegen at a
//! separate boundary.
//!
//! Nested value-objects are no longer stored as JSON: each scalar VO is
//! flattened into its own real, individually-indexable columns (see the
//! per-backend `media.rs` / `leaves.rs`), and the one many-to-many
//! collection (`SceneAnnotation::user_tags`) rides in the
//! `scene_annotation_user_tag` join table. This module therefore only
//! carries the byte / UUID / timestamp conversion helpers shared across
//! the three backends.

use crate::domain::Uuid7;

use super::error::SqlxError;

// ---------------------------------------------------------------------------
// Helpers: Uuid7 ↔ raw 16-byte BLOB / native uuid::Uuid
// ---------------------------------------------------------------------------

/// Convert a `Uuid7` to its native `uuid::Uuid` form (for Postgres
/// `uuid` columns + MySQL/SQLite via byte-encoded BLOB).
#[inline]
pub fn uuid7_to_uuid(id: Uuid7) -> uuid::Uuid {
  uuid::Uuid::from(id)
}

/// Convert a `uuid::Uuid` from a row into a validated `Uuid7`. Surfaces
/// any `Uuid7Error` (nil / non-v7) as a typed [`SqlxError::InvalidUuid`].
pub fn uuid_to_uuid7(u: uuid::Uuid) -> Result<Uuid7, SqlxError> {
  Uuid7::try_from(u).map_err(|e| SqlxError::InvalidUuid(e.to_string()))
}

/// Decode a row's 16-byte BLOB column (MySQL / SQLite UUID storage) into a
/// validated `Uuid7`.
pub fn bytes_to_uuid7(bytes: &[u8]) -> Result<Uuid7, SqlxError> {
  if bytes.len() != 16 {
    return Err(SqlxError::InvalidUuid(format!(
      "expected 16 bytes, got {}",
      bytes.len()
    )));
  }
  let mut arr = [0u8; 16];
  arr.copy_from_slice(bytes);
  Uuid7::try_from_bytes(arr).map_err(|e| SqlxError::InvalidUuid(e.to_string()))
}

/// Decode a row's 32-byte BLOB column into a validated [`crate::domain::FileChecksum`].
pub fn bytes_to_checksum(bytes: &[u8]) -> Result<crate::domain::FileChecksum, SqlxError> {
  if bytes.len() != 32 {
    return Err(SqlxError::InvalidChecksum(format!(
      "expected 32 bytes, got {}",
      bytes.len()
    )));
  }
  let mut arr = [0u8; 32];
  arr.copy_from_slice(bytes);
  Ok(crate::domain::FileChecksum::from_bytes(arr))
}

/// Convert a `jiff::Timestamp` to milliseconds since the Unix epoch
/// (matches the locked `schema/media.md` ms-resolution convention).
#[inline]
pub fn timestamp_to_millis(t: jiff::Timestamp) -> i64 {
  t.as_millisecond()
}

/// Convert milliseconds-since-epoch back to a `jiff::Timestamp`.
/// Out-of-range values surface as [`SqlxError::DomainConstructorRejected`]
/// (the underlying jiff error is `range`-typed).
pub fn millis_to_timestamp(ms: i64) -> Result<jiff::Timestamp, SqlxError> {
  jiff::Timestamp::from_millisecond(ms)
    .map_err(|e| SqlxError::DomainConstructorRejected(format!("jiff::Timestamp: {e}")))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn bytes_to_uuid7_rejects_wrong_length() {
    assert!(bytes_to_uuid7(&[0u8; 10]).is_err());
    // 16 zero bytes is the nil sentinel — rejected by Uuid7 validation.
    assert!(bytes_to_uuid7(&[0u8; 16]).is_err());
  }

  #[test]
  fn bytes_to_checksum_rejects_wrong_length() {
    assert!(bytes_to_checksum(&[0u8; 16]).is_err());
    let cs = bytes_to_checksum(&[0u8; 32]).unwrap();
    assert!(cs.is_zero());
  }

  #[test]
  fn uuid7_roundtrip() {
    let id = Uuid7::new();
    let u = uuid7_to_uuid(id);
    assert_eq!(uuid_to_uuid7(u).unwrap(), id);
    assert_eq!(bytes_to_uuid7(id.as_bytes()).unwrap(), id);
  }

  #[test]
  fn timestamp_roundtrip() {
    let ms = 1_700_000_000_000;
    let t = millis_to_timestamp(ms).unwrap();
    assert_eq!(timestamp_to_millis(t), ms);
  }
}
