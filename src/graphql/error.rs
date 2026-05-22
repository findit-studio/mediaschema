//! Errors returned from GraphQL resolvers / scalar parse paths.
//!
//! Resolvers can fail when a stub data source returns nothing (caller
//! responsibility) or when a scalar value is malformed at parse time
//! (e.g. a non-hex `FileChecksum` string). Both flavours funnel through
//! [`GqlError`].
//!
//! The enum follows the project convention: `#[non_exhaustive]`,
//! `derive(Debug, Clone, PartialEq, Eq, IsVariant, thiserror::Error)`
//! (the `thiserror` derive emits the `core::error::Error` impl directly,
//! not `std::error::Error` — supports no-std + no-alloc out of the box).

use derive_more::IsVariant;
use smol_str::SmolStr;

use crate::domain::primitives::Uuid7Error;

/// Backend-specific error for the GraphQL boundary.
#[derive(Debug, Clone, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum GqlError {
  /// A scalar value failed to parse from its string form. Carries the
  /// scalar type name and a short human reason.
  #[error("{scalar}: parse failed: {reason}")]
  ScalarParse {
    /// Name of the scalar type (`"Uuid7"`, `"FileChecksum"`, …).
    scalar: &'static str,
    /// Short human-readable reason.
    reason: SmolStr,
  },
  /// A `Uuid7` round-trip rejected its input.
  #[error("Uuid7 decode failed: {0}")]
  Uuid7(#[from] Uuid7Error),
  /// A scalar `Int`/`Float` value was outside the destination integer's
  /// range (e.g. a negative `Int` decoded into `u64` bits).
  #[error("field `{field}`: integer value {value} out of range")]
  IntOutOfRange {
    /// Name of the scalar / field.
    field: &'static str,
    /// Offending value rendered for the error message.
    value: i64,
  },
}

impl GqlError {
  /// Convenience: build a [`GqlError::ScalarParse`].
  #[inline]
  pub fn parse(scalar: &'static str, reason: impl Into<SmolStr>) -> Self {
    Self::ScalarParse {
      scalar,
      reason: reason.into(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_helper_builds_scalar_parse_variant() {
    let e = GqlError::parse("Uuid7", "bad hex");
    assert!(e.is_scalar_parse());
    assert_eq!(e.to_string(), "Uuid7: parse failed: bad hex");
  }

  #[test]
  fn int_out_of_range_displays_field_and_value() {
    let e = GqlError::IntOutOfRange {
      field: "size",
      value: -1,
    };
    assert!(e.is_int_out_of_range());
    assert_eq!(e.to_string(), "field `size`: integer value -1 out of range");
  }

  #[test]
  fn uuid7_from_propagates_source() {
    let e: GqlError = Uuid7Error::Nil.into();
    assert!(e.is_uuid_7());
    assert!(core::error::Error::source(&e).is_some());
  }
}
