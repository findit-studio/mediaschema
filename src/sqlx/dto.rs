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

// ---------------------------------------------------------------------------
// Helpers: media-time `Timebase` / `Timestamp` / `TimeRange`
// ---------------------------------------------------------------------------
//
// Media-time values carry a rational timebase, so a single ms column is
// lossy. Each media-time value flattens into real columns: a PTS tick
// `BIGINT` plus the timebase numerator / denominator (`*_tb_num` /
// `*_tb_den`). `Timebase::den` is `NonZeroU32`, so a zero denominator from
// the row is rejected as a typed error.

/// Rebuild a `mediatime::Timebase` from a stored numerator / denominator
/// pair. A zero (or `u32`-overflowing) denominator surfaces as a typed
/// [`SqlxError::DomainConstructorRejected`].
pub fn timebase_from_parts(num: i64, den: i64) -> Result<mediatime::Timebase, SqlxError> {
  let num = u32::try_from(num)
    .map_err(|e| SqlxError::DomainConstructorRejected(format!("Timebase.num: {e}")))?;
  let den = u32::try_from(den)
    .ok()
    .and_then(core::num::NonZeroU32::new)
    .ok_or_else(|| {
      SqlxError::DomainConstructorRejected(format!("Timebase.den must be a non-zero u32: {den}"))
    })?;
  Ok(mediatime::Timebase::new(num, den))
}

/// Rebuild a `mediatime::Timestamp` from a stored `(pts, tb_num, tb_den)`
/// triple.
pub fn timestamp_from_parts(
  pts: i64,
  tb_num: i64,
  tb_den: i64,
) -> Result<mediatime::Timestamp, SqlxError> {
  Ok(mediatime::Timestamp::new(
    pts,
    timebase_from_parts(tb_num, tb_den)?,
  ))
}

/// Rebuild a `mediatime::TimeRange` from a stored
/// `(start_pts, end_pts, tb_num, tb_den)` tuple. An inverted
/// `start > end` range is rejected (`TimeRange::try_new` returns `None`).
pub fn time_range_from_parts(
  start_pts: i64,
  end_pts: i64,
  tb_num: i64,
  tb_den: i64,
) -> Result<mediatime::TimeRange, SqlxError> {
  let tb = timebase_from_parts(tb_num, tb_den)?;
  mediatime::TimeRange::try_new(start_pts, end_pts, tb).ok_or_else(|| {
    SqlxError::DomainConstructorRejected(format!(
      "TimeRange start_pts ({start_pts}) must be <= end_pts ({end_pts})"
    ))
  })
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

  #[test]
  fn timebase_from_parts_rejects_zero_denominator() {
    assert!(timebase_from_parts(1, 0).is_err());
    assert!(timebase_from_parts(1, -5).is_err());
    let tb = timebase_from_parts(1, 1000).unwrap();
    assert_eq!(tb.num(), 1);
    assert_eq!(tb.den().get(), 1000);
  }

  #[test]
  fn media_time_roundtrip() {
    let tb = timebase_from_parts(1, 90_000).unwrap();
    let ts = mediatime::Timestamp::new(45_000, tb);
    let ts2 = timestamp_from_parts(
      ts.pts(),
      i64::from(ts.timebase().num()),
      i64::from(ts.timebase().den().get()),
    )
    .unwrap();
    assert_eq!(ts, ts2);

    let tr = mediatime::TimeRange::new(1_000, 2_500, tb);
    let tr2 = time_range_from_parts(
      tr.start_pts(),
      tr.end_pts(),
      i64::from(tr.timebase().num()),
      i64::from(tr.timebase().den().get()),
    )
    .unwrap();
    assert_eq!(tr, tr2);
  }

  #[test]
  fn time_range_from_parts_rejects_inverted() {
    assert!(time_range_from_parts(2_000, 1_000, 1, 1000).is_err());
  }
}
