//! Errors returned from GraphQL resolvers / scalar parse paths.
//!
//! Resolvers can fail when a stub data source returns nothing (caller
//! responsibility) or when a scalar value is malformed at parse time
//! (e.g. a non-hex `FileChecksum` string). Both flavours funnel through
//! [`GqlError`].
//!
//! The enum follows the project convention: `#[non_exhaustive]`,
//! `derive(Debug, Clone, PartialEq, Eq, IsVariant)`, manual
//! `core::fmt::Display`, and `core::error::Error` (not `std::`).

use derive_more::IsVariant;
use smol_str::SmolStr;

use crate::domain::primitives::Uuid7Error;

/// Backend-specific error for the GraphQL boundary.
#[derive(Debug, Clone, PartialEq, Eq, IsVariant)]
#[non_exhaustive]
pub enum GqlError {
  /// A scalar value failed to parse from its string form. Carries the
  /// scalar type name and a short human reason.
  ScalarParse {
    /// Name of the scalar type (`"Uuid7"`, `"FileChecksum"`, …).
    scalar: &'static str,
    /// Short human-readable reason.
    reason: SmolStr,
  },
  /// A `Uuid7` round-trip rejected its input.
  Uuid7(Uuid7Error),
  /// A scalar `Int`/`Float` value was outside the destination integer's
  /// range (e.g. a negative `Int` decoded into `u64` bits).
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

impl core::fmt::Display for GqlError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::ScalarParse { scalar, reason } => {
        write!(f, "{scalar}: parse failed: {reason}")
      }
      Self::Uuid7(e) => write!(f, "Uuid7 decode failed: {e}"),
      Self::IntOutOfRange { field, value } => {
        write!(f, "field `{field}`: integer value {value} out of range")
      }
    }
  }
}

impl core::error::Error for GqlError {
  fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
    match self {
      Self::Uuid7(e) => Some(e),
      _ => None,
    }
  }
}

impl From<Uuid7Error> for GqlError {
  #[inline]
  fn from(e: Uuid7Error) -> Self {
    Self::Uuid7(e)
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
