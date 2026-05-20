//! Custom GraphQL scalars for the domain primitives.
//!
//! Each scalar wraps a stable string form: hex for `FileChecksum`,
//! hyphenated UUID for `Uuid7`, RFC 3339 for `jiff::Timestamp`,
//! `<pts>@<num>/<den>` for `mediatime::Timestamp`, and
//! `[<start>,<end>]@<num>/<den>` for `mediatime::TimeRange`. Each
//! string form parses back losslessly through the corresponding
//! validating ctor.
//!
//! These are implemented via `async_graphql::Scalar` (the trait), not
//! the `scalar!` macro, so the implementation surface is explicit and
//! the parse paths can return [`crate::graphql::GqlError`] reasons.

use async_graphql::{InputValueError, InputValueResult, Scalar, ScalarType, Value};
use core::num::NonZeroU32;
use smol_str::SmolStr;

use crate::domain::{FileChecksum, Uuid7};

// ---------------------------------------------------------------------------
// Uuid7 — hyphenated UUID string.
// ---------------------------------------------------------------------------

#[Scalar(name = "Uuid7")]
impl ScalarType for Uuid7 {
  fn parse(value: Value) -> InputValueResult<Self> {
    let Value::String(s) = value else {
      return Err(InputValueError::expected_type(value));
    };
    s.parse::<Uuid7>()
      .map_err(|e| InputValueError::custom(format!("Uuid7 parse failed: {e}")))
  }

  fn to_value(&self) -> Value {
    Value::String(self.to_string())
  }
}

// ---------------------------------------------------------------------------
// FileChecksum — lower-case 64-char hex.
// ---------------------------------------------------------------------------

#[Scalar(name = "FileChecksum")]
impl ScalarType for FileChecksum {
  fn parse(value: Value) -> InputValueResult<Self> {
    let Value::String(s) = value else {
      return Err(InputValueError::expected_type(value));
    };
    if s.len() != 64 {
      return Err(InputValueError::custom(format!(
        "FileChecksum must be 64 hex chars, got {}",
        s.len()
      )));
    }
    let mut bytes = [0u8; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
      let hi = decode_hex_nibble(s.as_bytes()[i * 2])?;
      let lo = decode_hex_nibble(s.as_bytes()[i * 2 + 1])?;
      *byte = (hi << 4) | lo;
    }
    Ok(FileChecksum::from_bytes(bytes))
  }

  fn to_value(&self) -> Value {
    Value::String(self.to_string())
  }
}

fn decode_hex_nibble(b: u8) -> Result<u8, InputValueError<FileChecksum>> {
  match b {
    b'0'..=b'9' => Ok(b - b'0'),
    b'a'..=b'f' => Ok(b - b'a' + 10),
    b'A'..=b'F' => Ok(b - b'A' + 10),
    _ => Err(InputValueError::custom(format!(
      "FileChecksum: invalid hex byte 0x{b:02x}"
    ))),
  }
}

// ---------------------------------------------------------------------------
// `jiff::Timestamp` — RFC 3339 string.
// ---------------------------------------------------------------------------

/// Newtype wrapper around [`jiff::Timestamp`] for the GraphQL boundary.
/// `async_graphql::Scalar` requires a trait impl on the type it names,
/// and the foreign-type orphan rule means we cannot implement
/// `ScalarType` on `jiff::Timestamp` directly. The wrapper is
/// `#[repr(transparent)]` and exposes `From` conversions both ways.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct GqlJiffTimestamp(pub jiff::Timestamp);

impl From<jiff::Timestamp> for GqlJiffTimestamp {
  #[inline]
  fn from(t: jiff::Timestamp) -> Self {
    Self(t)
  }
}

impl From<GqlJiffTimestamp> for jiff::Timestamp {
  #[inline]
  fn from(w: GqlJiffTimestamp) -> Self {
    w.0
  }
}

#[Scalar(name = "JiffTimestamp")]
impl ScalarType for GqlJiffTimestamp {
  fn parse(value: Value) -> InputValueResult<Self> {
    let Value::String(s) = value else {
      return Err(InputValueError::expected_type(value));
    };
    s.parse::<jiff::Timestamp>()
      .map(GqlJiffTimestamp)
      .map_err(|e| InputValueError::custom(format!("JiffTimestamp parse failed: {e}")))
  }

  fn to_value(&self) -> Value {
    Value::String(self.0.to_string())
  }
}

// ---------------------------------------------------------------------------
// `mediatime::Timestamp` — `<pts>@<num>/<den>`.
// ---------------------------------------------------------------------------

/// Newtype wrapper around [`mediatime::Timestamp`]. Same orphan-rule
/// rationale as [`GqlJiffTimestamp`]. The string form is
/// `<pts>@<num>/<den>` — round-trips through the
/// `mediatime::Timebase::new(num, NonZeroU32::new(den))` ctor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct GqlMediaTimestamp(pub mediatime::Timestamp);

impl From<mediatime::Timestamp> for GqlMediaTimestamp {
  #[inline]
  fn from(t: mediatime::Timestamp) -> Self {
    Self(t)
  }
}

impl From<GqlMediaTimestamp> for mediatime::Timestamp {
  #[inline]
  fn from(w: GqlMediaTimestamp) -> Self {
    w.0
  }
}

#[Scalar(name = "MediaTimestamp")]
impl ScalarType for GqlMediaTimestamp {
  fn parse(value: Value) -> InputValueResult<Self> {
    let Value::String(s) = value else {
      return Err(InputValueError::expected_type(value));
    };
    parse_media_timestamp(s.as_str())
      .map(GqlMediaTimestamp)
      .map_err(InputValueError::custom)
  }

  fn to_value(&self) -> Value {
    Value::String(format_media_timestamp(self.0))
  }
}

fn format_media_timestamp(t: mediatime::Timestamp) -> String {
  let tb = t.timebase();
  format!("{}@{}/{}", t.pts(), tb.num(), tb.den().get())
}

fn parse_media_timestamp(s: &str) -> Result<mediatime::Timestamp, String> {
  let (pts_str, tb_str) = s
    .split_once('@')
    .ok_or_else(|| format!("MediaTimestamp missing `@`: {s:?}"))?;
  let pts: i64 = pts_str
    .parse()
    .map_err(|e| format!("MediaTimestamp: bad pts: {e}"))?;
  let tb = parse_timebase(tb_str)?;
  Ok(mediatime::Timestamp::new(pts, tb))
}

fn parse_timebase(s: &str) -> Result<mediatime::Timebase, String> {
  let (num_str, den_str) = s
    .split_once('/')
    .ok_or_else(|| format!("Timebase missing `/`: {s:?}"))?;
  let num: u32 = num_str
    .parse()
    .map_err(|e| format!("Timebase: bad num: {e}"))?;
  let den: u32 = den_str
    .parse()
    .map_err(|e| format!("Timebase: bad den: {e}"))?;
  let den = NonZeroU32::new(den).ok_or_else(|| "Timebase: den must be non-zero".to_string())?;
  Ok(mediatime::Timebase::new(num, den))
}

// ---------------------------------------------------------------------------
// `mediatime::TimeRange` — `[<start>,<end>]@<num>/<den>`.
// ---------------------------------------------------------------------------

/// Newtype wrapper around [`mediatime::TimeRange`]. Wire form is
/// `[<start>,<end>]@<num>/<den>` — round-trips through `TimeRange::new`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct GqlMediaTimeRange(pub mediatime::TimeRange);

impl From<mediatime::TimeRange> for GqlMediaTimeRange {
  #[inline]
  fn from(t: mediatime::TimeRange) -> Self {
    Self(t)
  }
}

impl From<GqlMediaTimeRange> for mediatime::TimeRange {
  #[inline]
  fn from(w: GqlMediaTimeRange) -> Self {
    w.0
  }
}

#[Scalar(name = "MediaTimeRange")]
impl ScalarType for GqlMediaTimeRange {
  fn parse(value: Value) -> InputValueResult<Self> {
    let Value::String(s) = value else {
      return Err(InputValueError::expected_type(value));
    };
    parse_media_time_range(s.as_str())
      .map(GqlMediaTimeRange)
      .map_err(InputValueError::custom)
  }

  fn to_value(&self) -> Value {
    Value::String(format_media_time_range(self.0))
  }
}

fn format_media_time_range(r: mediatime::TimeRange) -> String {
  let tb = r.timebase();
  format!(
    "[{},{}]@{}/{}",
    r.start_pts(),
    r.end_pts(),
    tb.num(),
    tb.den().get()
  )
}

fn parse_media_time_range(s: &str) -> Result<mediatime::TimeRange, String> {
  let s = s.trim();
  let inner = s
    .strip_prefix('[')
    .ok_or_else(|| format!("MediaTimeRange missing `[`: {s:?}"))?;
  let (range_part, tb_str) = inner
    .split_once("]@")
    .ok_or_else(|| format!("MediaTimeRange missing `]@`: {s:?}"))?;
  let (start_str, end_str) = range_part
    .split_once(',')
    .ok_or_else(|| format!("MediaTimeRange missing `,`: {s:?}"))?;
  let start: i64 = start_str
    .parse()
    .map_err(|e| format!("MediaTimeRange: bad start: {e}"))?;
  let end: i64 = end_str
    .parse()
    .map_err(|e| format!("MediaTimeRange: bad end: {e}"))?;
  let tb = parse_timebase(tb_str)?;
  mediatime::TimeRange::try_new(start, end, tb)
    .ok_or_else(|| format!("MediaTimeRange: inverted span (start > end): {s:?}"))
}

// ---------------------------------------------------------------------------
// Helper for resolvers: helper that lifts a `&SmolStr`-backed `&str`
// into a `String` field value. (Inlined where used.)
// ---------------------------------------------------------------------------

/// Tiny helper: convert an empty-as-absent `&str` into `Option<String>`.
/// Used by resolvers where the locked free-text rule ("`""` = absent")
/// projects to a nullable GraphQL field.
#[inline]
pub fn empty_as_none(s: &str) -> Option<String> {
  if s.is_empty() {
    None
  } else {
    Some(s.to_string())
  }
}

/// Wrap a [`SmolStr`] in `Option`, dropping `""` to `None`.
#[inline]
pub fn smolstr_empty_as_none(s: &SmolStr) -> Option<String> {
  if s.is_empty() {
    None
  } else {
    Some(s.as_str().to_string())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use core::num::NonZeroU32;

  #[test]
  fn uuid7_roundtrips_through_scalar_value() {
    let u = Uuid7::new();
    let v = ScalarType::to_value(&u);
    let back = <Uuid7 as ScalarType>::parse(v).unwrap();
    assert_eq!(back, u);
  }

  #[test]
  fn uuid7_rejects_non_string_and_bad_input() {
    assert!(<Uuid7 as ScalarType>::parse(Value::Number(42.into())).is_err());
    assert!(<Uuid7 as ScalarType>::parse(Value::String("not-a-uuid".into())).is_err());
    // Nil string: parsed as UUID but rejected by validating ctor.
    assert!(<Uuid7 as ScalarType>::parse(Value::String(
      "00000000-0000-0000-0000-000000000000".into()
    ))
    .is_err());
  }

  #[test]
  fn file_checksum_roundtrips() {
    let bytes = [0xde, 0xad, 0xbe, 0xef]
      .into_iter()
      .cycle()
      .take(32)
      .collect::<std::vec::Vec<u8>>()
      .try_into()
      .unwrap();
    let cs = FileChecksum::from_bytes(bytes);
    let v = ScalarType::to_value(&cs);
    let back = <FileChecksum as ScalarType>::parse(v).unwrap();
    assert_eq!(back, cs);
  }

  #[test]
  fn file_checksum_rejects_wrong_length_and_bad_hex() {
    assert!(<FileChecksum as ScalarType>::parse(Value::String("deadbeef".into())).is_err());
    let bad_hex = "z".repeat(64);
    assert!(<FileChecksum as ScalarType>::parse(Value::String(bad_hex.into())).is_err());
  }

  #[test]
  fn jiff_timestamp_roundtrips() {
    let now: jiff::Timestamp = "2026-05-21T12:34:56.789Z".parse().unwrap();
    let w = GqlJiffTimestamp(now);
    let v = ScalarType::to_value(&w);
    let back = <GqlJiffTimestamp as ScalarType>::parse(v).unwrap();
    assert_eq!(back, w);
  }

  #[test]
  fn jiff_timestamp_rejects_bad_input() {
    assert!(<GqlJiffTimestamp as ScalarType>::parse(Value::String("not-a-time".into())).is_err());
    assert!(<GqlJiffTimestamp as ScalarType>::parse(Value::Number(0.into())).is_err());
  }

  #[test]
  fn media_timestamp_roundtrips() {
    let tb = mediatime::Timebase::new(1, NonZeroU32::new(1000).unwrap());
    let t = mediatime::Timestamp::new(12_345, tb);
    let w = GqlMediaTimestamp(t);
    let v = ScalarType::to_value(&w);
    assert_eq!(v, Value::String("12345@1/1000".into()));
    let back = <GqlMediaTimestamp as ScalarType>::parse(v).unwrap();
    assert_eq!(back, w);
  }

  #[test]
  fn media_timestamp_rejects_malformed() {
    for bad in ["", "12345", "12345@", "12345@1/0", "abc@1/1"] {
      assert!(
        <GqlMediaTimestamp as ScalarType>::parse(Value::String(bad.into())).is_err(),
        "input {bad:?} should be rejected"
      );
    }
  }

  #[test]
  fn media_time_range_roundtrips() {
    let tb = mediatime::Timebase::new(1, NonZeroU32::new(1000).unwrap());
    let r = mediatime::TimeRange::new(100, 200, tb);
    let w = GqlMediaTimeRange(r);
    let v = ScalarType::to_value(&w);
    assert_eq!(v, Value::String("[100,200]@1/1000".into()));
    let back = <GqlMediaTimeRange as ScalarType>::parse(v).unwrap();
    assert_eq!(back, w);
  }

  #[test]
  fn media_time_range_rejects_inverted() {
    // start > end
    let bad = "[200,100]@1/1000";
    assert!(<GqlMediaTimeRange as ScalarType>::parse(Value::String(bad.into())).is_err());
  }

  #[test]
  fn empty_as_none_drops_empty_to_none() {
    assert_eq!(empty_as_none(""), None);
    assert_eq!(empty_as_none("x"), Some("x".to_string()));
  }
}
