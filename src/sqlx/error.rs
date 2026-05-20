//! Backend-specific error type for the [`crate::sqlx`] row-mapping layer.
//!
//! Conversions between domain aggregates and database rows can fail in
//! ways the domain `try_new` validation never sees (a malformed UUID byte
//! string, a JSON payload that fails to deserialise, a value that the
//! domain constructor then rejects). [`SqlxError`] collapses all of
//! those into one stable variant set.

use derive_more::IsVariant;

/// Errors produced by [`crate::sqlx`] row → domain (and domain → row)
/// conversions.
///
/// Variants are wire-stable enough for downstream code to match on the
/// `IsVariant` predicates (`err.is_invalid_uuid()` / `err.is_invalid_checksum()`
/// / …). `#[non_exhaustive]` — new variants may be added without a
/// SemVer break.
#[derive(Debug, Clone, PartialEq, Eq, IsVariant)]
#[non_exhaustive]
pub enum SqlxError {
  /// The row's UUID column failed [`crate::domain::Uuid7`] validation
  /// (nil or non-v7).
  InvalidUuid(String),
  /// The row's checksum column was not the required 32 bytes.
  InvalidChecksum(String),
  /// The row's JSON column failed to deserialise.
  InvalidJson(String),
  /// The domain aggregate's `try_new` rejected the row's contents (nil
  /// id, nil parent, empty path, zero checksum, etc.).
  DomainConstructorRejected(String),
  /// The row's enum/discriminant column carried a value outside the
  /// known set.
  UnknownDiscriminant(String),
}

impl core::fmt::Display for SqlxError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::InvalidUuid(m) => write!(f, "invalid UUID column: {m}"),
      Self::InvalidChecksum(m) => write!(f, "invalid checksum column: {m}"),
      Self::InvalidJson(m) => write!(f, "invalid JSON column: {m}"),
      Self::DomainConstructorRejected(m) => write!(f, "domain constructor rejected row: {m}"),
      Self::UnknownDiscriminant(m) => write!(f, "unknown discriminant: {m}"),
    }
  }
}

impl core::error::Error for SqlxError {}
