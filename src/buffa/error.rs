//! Error type surfaced by the buffa wire → domain bridge.
//!
//! The domain layer is **strictly more constrained** than the wire layer
//! (the domain rejects nil-id, non-v7 layout, malformed checksum bytes,
//! empty path / nil volume on `Local`, etc.). The reverse direction is
//! always infallible because the domain is the validated side; therefore
//! only `From<&Wire> for Domain` may fail and surface a [`BuffaError`].
//!
//! Variants are kept newtype-wrapped where they delegate to a lower-level
//! validating constructor's error so callers can `try_unwrap_*` /
//! `unwrap_*_ref` on the inner error per the derive_more pattern.

use derive_more::{IsVariant, TryUnwrap, Unwrap};

use crate::domain::primitives::{LocationError, Uuid7Error};

/// Failure modes for wire → domain conversion.
///
/// Mixed enum (unit + newtype variants) → derives `IsVariant` plus
/// `Unwrap` / `TryUnwrap` accessor families with ref + ref_mut flavours,
/// matching the domain-layer convention.
#[derive(Debug, Clone, PartialEq, Eq, IsVariant, Unwrap, TryUnwrap)]
#[unwrap(ref, ref_mut)]
#[try_unwrap(ref, ref_mut)]
#[non_exhaustive]
pub enum BuffaError {
  /// Wire `Id.value` did not contain exactly 16 bytes.
  IdWrongLength(usize),
  /// Wire `Id.value` parsed but failed the domain [`Uuid7`] invariant
  /// (nil / non-v7).
  IdInvalid(Uuid7Error),
  /// Wire `FileChecksum.value` did not contain exactly 32 bytes.
  ChecksumWrongLength(usize),
  /// Wire `Local` had no `volume` set, or `Location.kind` had no arm
  /// when the domain requires one.
  MissingLocationVolume,
  /// Wire `Local`'s components / volume failed the domain validating
  /// builder (empty path / nil volume).
  Location(LocationError),
  /// Wire `Location.kind` was set to a variant not supported by the
  /// domain (currently only `Local` is modelled — placeholder for
  /// future `Object{…}` etc.).
  UnsupportedLocationKind,
  /// Wire timestamp (ms-since-epoch i64) was outside the range jiff
  /// accepts (`jiff::Timestamp::MIN.as_millisecond()` ..=
  /// `jiff::Timestamp::MAX.as_millisecond()`).
  TimestampOutOfRange(i64),
  /// Wire enum was `EnumValue::Unknown(i32)` for a domain enum that
  /// has no `Unknown` arm.
  UnknownEnumValue(i32),
  /// Required wire message field was unset where the domain demands a
  /// present value (e.g. `WatchedLocation.id`).
  MissingRequiredField(&'static str),
}

impl core::fmt::Display for BuffaError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::IdWrongLength(n) => write!(f, "wire Id.value must be 16 bytes, got {n}"),
      Self::IdInvalid(e) => write!(f, "wire Id failed Uuid7 invariant: {e}"),
      Self::ChecksumWrongLength(n) => {
        write!(f, "wire FileChecksum.value must be 32 bytes, got {n}")
      }
      Self::MissingLocationVolume => f.write_str("wire Local.volume is unset"),
      Self::Location(e) => write!(f, "wire Local failed domain validation: {e}"),
      Self::UnsupportedLocationKind => {
        f.write_str("wire Location.kind variant not supported by the domain")
      }
      Self::TimestampOutOfRange(n) => {
        write!(f, "wire timestamp {n} ms is outside jiff::Timestamp range")
      }
      Self::UnknownEnumValue(v) => write!(
        f,
        "wire enum carries unknown value {v} for a closed domain enum"
      ),
      Self::MissingRequiredField(name) => {
        write!(f, "wire message is missing required field `{name}`")
      }
    }
  }
}

impl core::error::Error for BuffaError {
  fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
    match self {
      Self::IdInvalid(e) => Some(e),
      Self::Location(e) => Some(e),
      _ => None,
    }
  }
}

impl From<Uuid7Error> for BuffaError {
  fn from(e: Uuid7Error) -> Self {
    Self::IdInvalid(e)
  }
}

impl From<LocationError> for BuffaError {
  fn from(e: LocationError) -> Self {
    Self::Location(e)
  }
}
