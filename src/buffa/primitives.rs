//! Wire ⇄ domain conversions for the foundational newtypes:
//!
//! - `media.v1::Id`           ⇄ `domain::Uuid7`
//! - `media.v1::FileChecksum` ⇄ `domain::FileChecksum`
//! - `media.v1::ErrorInfo`    ⇄ `domain::ErrorInfo`
//! - wire `u32`               ⇄ `domain::ErrorCode` (helper functions)
//!
//! All wire → domain conversions are `TryFrom`: the domain rejects values
//! the wire layer can carry (nil/non-v7 bytes, wrong-length checksum
//! bytes). All domain → wire conversions are infallible `From` (the
//! domain is the constrained side).

use ::buffa::bytes::Bytes;
use smol_str::SmolStr;

use crate::{
  buffa::error::BuffaError,
  domain::{ErrorCode, ErrorInfo, FileChecksum, Uuid7},
  generated::media::v1 as wire,
};
// Under `feature = "alloc"` (no std), `String` / `ToOwned` / `ToString`
// aren't in the prelude — pull them in via the `extern crate alloc as std`
// alias declared in `lib.rs`. Under `feature = "std"` these come from the
// std prelude automatically; the cfg keeps the import a no-op there.
#[cfg(all(not(feature = "std"), feature = "alloc"))]
#[allow(unused_imports)]
use std::{
  borrow::ToOwned,
  string::{String, ToString},
};

// ---------------------------------------------------------------------------
// Id (wire 16-byte Bytes) ⇄ Uuid7 (domain newtype around uuid::Uuid)
// ---------------------------------------------------------------------------

impl TryFrom<&wire::Id> for Uuid7 {
  type Error = BuffaError;

  /// Decode a wire `Id` into the validating domain [`Uuid7`].
  ///
  /// The wire shape is `Bytes` (any length). A real wire-produced `Id`
  /// is always 16 bytes, but anyone can build a malformed wire value;
  /// this conversion enforces 16 bytes + non-nil + v7 layout.
  fn try_from(w: &wire::Id) -> Result<Self, Self::Error> {
    let bytes: &[u8] = w.value.as_ref();
    let arr: [u8; 16] = bytes
      .try_into()
      .map_err(|_| BuffaError::IdWrongLength(bytes.len()))?;
    Uuid7::try_from_bytes(arr).map_err(BuffaError::from)
  }
}

impl From<&Uuid7> for wire::Id {
  /// Encode a domain [`Uuid7`] as a wire `Id` (16-byte `Bytes`).
  fn from(d: &Uuid7) -> Self {
    let raw: [u8; 16] = *d.as_bytes();
    wire::Id {
      value: Bytes::copy_from_slice(&raw),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

// ---------------------------------------------------------------------------
// FileChecksum (wire 32-byte Bytes) ⇄ FileChecksum (domain)
// ---------------------------------------------------------------------------

impl TryFrom<&wire::FileChecksum> for FileChecksum {
  type Error = BuffaError;

  /// Decode a wire `FileChecksum` into the domain newtype. Requires
  /// exactly 32 bytes (256-bit hash); the all-zero sentinel is allowed
  /// (it represents "not yet computed").
  fn try_from(w: &wire::FileChecksum) -> Result<Self, Self::Error> {
    let bytes: &[u8] = w.value.as_ref();
    let arr: [u8; 32] = bytes
      .try_into()
      .map_err(|_| BuffaError::ChecksumWrongLength(bytes.len()))?;
    Ok(FileChecksum::from_bytes(arr))
  }
}

impl From<&FileChecksum> for wire::FileChecksum {
  /// Encode the domain newtype as a wire `FileChecksum` (32-byte `Bytes`).
  fn from(d: &FileChecksum) -> Self {
    let raw: [u8; 32] = *d.as_bytes();
    wire::FileChecksum {
      value: Bytes::copy_from_slice(&raw),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

// ---------------------------------------------------------------------------
// ErrorInfo ⇄ ErrorInfo (both sides: { code: u32 | ErrorCode, message })
// ---------------------------------------------------------------------------

impl From<&wire::ErrorInfo> for ErrorInfo {
  /// Decode a wire `ErrorInfo`. Wire `code: u32` is always lossless —
  /// unknown wire values land in [`ErrorCode::Unknown`]. Wire empty
  /// `message: String` (proto3 default) maps to the domain's `""`-means
  /// -absent convention (still a `SmolStr`, just empty).
  fn from(w: &wire::ErrorInfo) -> Self {
    ErrorInfo::new(
      ErrorCode::from_u32(w.code),
      SmolStr::from(w.message.as_str()),
    )
  }
}

impl From<&ErrorInfo> for wire::ErrorInfo {
  /// Encode the domain `ErrorInfo` (always lossless — `ErrorCode::as_u32`
  /// + `message().to_string()`).
  fn from(d: &ErrorInfo) -> Self {
    wire::ErrorInfo {
      code: d.code().as_u32(),
      message: d.message().to_string(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  // ---- Uuid7 / Id ----

  #[test]
  fn uuid7_roundtrip() {
    let d = Uuid7::new();
    let w: wire::Id = wire::Id::from(&d);
    let d2 = Uuid7::try_from(&w).expect("roundtrip");
    assert_eq!(d, d2);
  }

  #[test]
  fn uuid7_rejects_wrong_length() {
    let w = wire::Id {
      value: Bytes::copy_from_slice(&[0u8; 4]),
      __buffa_unknown_fields: Default::default(),
    };
    let err = Uuid7::try_from(&w).unwrap_err();
    assert!(err.is_id_wrong_length());
    assert_eq!(err.try_unwrap_id_wrong_length_ref(), Ok(&4));
  }

  #[test]
  fn uuid7_rejects_nil_wire_bytes() {
    let w = wire::Id {
      value: Bytes::copy_from_slice(&[0u8; 16]),
      __buffa_unknown_fields: Default::default(),
    };
    let err = Uuid7::try_from(&w).unwrap_err();
    assert!(err.is_id_invalid());
  }

  // ---- FileChecksum ----

  #[test]
  fn checksum_roundtrip() {
    let raw = [7u8; 32];
    let d = FileChecksum::from_bytes(raw);
    let w: wire::FileChecksum = wire::FileChecksum::from(&d);
    let d2 = FileChecksum::try_from(&w).expect("roundtrip");
    assert_eq!(d, d2);
    assert_eq!(d2.as_bytes(), &raw);
  }

  #[test]
  fn checksum_zero_sentinel_roundtrips() {
    let d = FileChecksum::new();
    let w = wire::FileChecksum::from(&d);
    let d2 = FileChecksum::try_from(&w).unwrap();
    assert!(d2.is_zero());
  }

  #[test]
  fn checksum_rejects_wrong_length() {
    let w = wire::FileChecksum {
      value: Bytes::copy_from_slice(&[1u8; 16]),
      __buffa_unknown_fields: Default::default(),
    };
    let err = FileChecksum::try_from(&w).unwrap_err();
    assert!(err.is_checksum_wrong_length());
    assert_eq!(err.try_unwrap_checksum_wrong_length_ref(), Ok(&16));
  }

  // ---- ErrorInfo ----

  #[test]
  fn error_info_known_code_roundtrip() {
    let d = ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad header");
    let w = wire::ErrorInfo::from(&d);
    assert_eq!(w.code, 1000);
    assert_eq!(w.message, "bad header");
    let d2 = ErrorInfo::from(&w);
    assert_eq!(d2, d);
  }

  #[test]
  fn error_info_unknown_code_round_trips_verbatim() {
    let w = wire::ErrorInfo {
      code: 99_999,
      message: String::from("future code"),
      __buffa_unknown_fields: Default::default(),
    };
    let d = ErrorInfo::from(&w);
    assert!(d.code().is_unknown());
    let w2 = wire::ErrorInfo::from(&d);
    assert_eq!(w2.code, 99_999);
    assert_eq!(w2.message, "future code");
  }

  #[test]
  fn error_info_empty_message_is_preserved() {
    let d = ErrorInfo::code_only(ErrorCode::Cancelled);
    let w = wire::ErrorInfo::from(&d);
    assert!(w.message.is_empty());
    let d2 = ErrorInfo::from(&w);
    assert!(d2.message().is_empty());
    assert_eq!(d2.code(), ErrorCode::Cancelled);
  }
}
