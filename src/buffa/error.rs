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
use smol_str::SmolStr;

use crate::domain::primitives::{LocationError, Uuid7Error};

/// Failure modes for wire → domain conversion.
///
/// Mixed enum (unit + newtype variants) → derives `IsVariant` plus
/// `Unwrap` / `TryUnwrap` accessor families with ref + ref_mut flavours,
/// matching the domain-layer convention.
#[derive(Debug, Clone, PartialEq, Eq, IsVariant, Unwrap, TryUnwrap, thiserror::Error)]
#[unwrap(ref, ref_mut)]
#[try_unwrap(ref, ref_mut)]
#[non_exhaustive]
pub enum BuffaError {
  /// Wire `Id.value` did not contain exactly 16 bytes.
  #[error("wire Id.value must be 16 bytes, got {0}")]
  IdWrongLength(usize),
  /// Wire `Id.value` parsed but failed the domain [`Uuid7`](crate::domain::Uuid7) invariant
  /// (nil / non-v7).
  #[error("wire Id failed Uuid7 invariant: {0}")]
  IdInvalid(#[from] Uuid7Error),
  /// Wire `FileChecksum.value` did not contain exactly 32 bytes.
  #[error("wire FileChecksum.value must be 32 bytes, got {0}")]
  ChecksumWrongLength(usize),
  /// Wire `Local` had no `volume` set, or `Location.kind` had no arm
  /// when the domain requires one.
  #[error("wire Local.volume is unset")]
  MissingLocationVolume,
  /// Wire `Local`'s components / volume failed the domain validating
  /// builder (empty path / nil volume).
  #[error("wire Local failed domain validation: {0}")]
  Location(#[from] LocationError),
  /// Wire `Location.kind` was set to a variant not supported by the
  /// domain (currently only `Local` is modelled — placeholder for
  /// future `Object{…}` etc.).
  #[error("wire Location.kind variant not supported by the domain")]
  UnsupportedLocationKind,
  /// Wire timestamp (ms-since-epoch i64) was outside the range jiff
  /// accepts (`jiff::Timestamp::MIN.as_millisecond()` ..=
  /// `jiff::Timestamp::MAX.as_millisecond()`).
  #[error("wire timestamp {0} ms is outside jiff::Timestamp range")]
  TimestampOutOfRange(i64),
  /// Wire enum was `EnumValue::Unknown(i32)` for a domain enum that
  /// has no `Unknown` arm.
  #[error("wire enum carries unknown value {0} for a closed domain enum")]
  UnknownEnumValue(i32),
  /// Required wire message field was unset where the domain demands a
  /// present value (e.g. `WatchedLocation.id`).
  #[error("wire message is missing required field `{0}`")]
  MissingRequiredField(&'static str),
  /// Wire `Media.gps_location` (an ISO 6709 degrees-only string) failed
  /// to parse into a [`mediaframe::capture::GeoLocation`] (malformed
  /// shape or out-of-range lat/lon). The offending string is wrapped
  /// verbatim.
  #[error("wire gps_location is not a valid ISO 6709 location: {0:?}")]
  GpsLocationMalformed(smol_str::SmolStr),
  /// Wire `Language.bcp47` failed to parse as a well-formed BCP-47
  /// language identifier via [`mediaframe::lang::Language::from_bcp47`].
  /// The offending string is wrapped verbatim.
  #[error("wire Language.bcp47 is not a valid BCP-47 tag: {0:?}")]
  LanguageMalformed(smol_str::SmolStr),
  /// Wire `SubtitleCue.kind` carried a discriminant for a format whose
  /// payload `D` type isn't implemented in this revision (reserved for
  /// issue #56).
  #[error("wire SubtitleCue.kind `{0}` not yet implemented (issue #56)")]
  UnimplementedSubtitleCueKind(i32),
  /// Wire `SubtitleCue.data` oneof slot was unset for a `kind` that
  /// requires a payload (the implemented kinds all carry a data arm).
  #[error("wire SubtitleCue.data oneof is unset for kind `{0:?}`")]
  MissingSubtitleCueData(&'static str),
  /// Wire `SubtitleCue.kind` discriminant did not match the variant of
  /// the `data` oneof actually present (a tampered wire frame). First
  /// payload field is the expected `kind`, second is the actual oneof
  /// arm name.
  #[error("wire SubtitleCue.kind `{0}` does not match data oneof variant `{1}`")]
  SubtitleCueKindOneofMismatch(&'static str, &'static str),
  /// Wire `SubtitleCue` carried an integer value that didn't fit the
  /// domain's narrower numeric type (e.g. `AssStyle.border_style` is
  /// `i16` on the domain but `int32` on the wire). First payload is
  /// the field name, second is the offending value.
  #[error("wire SubtitleCue numeric field `{0}` value {1} is out of range for the domain")]
  SubtitleNumericOutOfRange(&'static str, i32),
  /// A domain `try_new` / `try_with_*` constructor rejected the wire
  /// payload. Carries the originating error's display message; the
  /// per-aggregate typed errors have their own variants where the
  /// distinction matters.
  #[error("wire payload rejected by domain constructor: {0}")]
  DomainConstructorRejected(SmolStr),
}
