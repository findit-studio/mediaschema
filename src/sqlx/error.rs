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
#[derive(Debug, Clone, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum SqlxError {
  /// The row's UUID column failed [`crate::domain::Uuid7`] validation
  /// (nil or non-v7).
  #[error("invalid UUID column: {0}")]
  InvalidUuid(String),
  /// The row's checksum column was not the required 32 bytes.
  #[error("invalid checksum column: {0}")]
  InvalidChecksum(String),
  /// The row's JSON column failed to deserialise.
  #[error("invalid JSON column: {0}")]
  InvalidJson(String),
  /// The domain aggregate's `try_new` rejected the row's contents (nil
  /// id, nil parent, empty path, zero checksum, etc.).
  #[error("domain constructor rejected row: {0}")]
  DomainConstructorRejected(String),
  /// The row's enum/discriminant column carried a value outside the
  /// known set.
  #[error("unknown discriminant: {0}")]
  UnknownDiscriminant(String),
}
