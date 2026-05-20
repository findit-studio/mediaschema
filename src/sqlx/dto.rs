//! Serde-serializable data-transfer objects mirroring the domain types
//! that get stored as JSON columns.
//!
//! The domain types in `src/domain/` deliberately do **not** derive
//! serde — domain validation flows through `try_new`/`with_*`, and
//! cross-format wire conversion is handled by the buffa codegen at a
//! separate boundary. The `sqlx` backend therefore needs its own
//! intermediate representation it can serialise to JSON and round-trip
//! back through the domain constructors.
//!
//! Every nested value-object in the locked schema (`Provenance`,
//! `LocalizedText`, `MediaDevice`, `MediaGeoLocation`, the structured
//! `Location` oneof, `ErrorInfo`) gets a matching `*Dto` here, with
//! `From<&Domain> for Dto` and `TryFrom<Dto> for Domain` impls.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::domain::{
  primitives::LocationError, ErrorCode, ErrorInfo, LocalizedText, Location, MediaDevice,
  MediaGeoLocation, Provenance, Uuid7,
};

use super::error::SqlxError;

// ---------------------------------------------------------------------------
// ProvenanceDto
// ---------------------------------------------------------------------------

/// Wire shape: `{ "model_name": "...", "model_version": "...", ... }`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceDto {
  #[serde(default)]
  pub model_name: String,
  #[serde(default)]
  pub model_version: String,
  #[serde(default)]
  pub prompt_version: String,
  #[serde(default)]
  pub indexer_version: String,
}

impl From<&Provenance> for ProvenanceDto {
  fn from(p: &Provenance) -> Self {
    Self {
      model_name: p.model_name().to_owned(),
      model_version: p.model_version().to_owned(),
      prompt_version: p.prompt_version().to_owned(),
      indexer_version: p.indexer_version().to_owned(),
    }
  }
}

impl From<ProvenanceDto> for Provenance {
  fn from(d: ProvenanceDto) -> Self {
    Provenance::from_parts(
      d.model_name,
      d.model_version,
      d.prompt_version,
      d.indexer_version,
    )
  }
}

// ---------------------------------------------------------------------------
// LocalizedTextDto
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalizedTextDto {
  #[serde(default)]
  pub src: String,
  #[serde(default)]
  pub translated: String,
}

impl From<&LocalizedText> for LocalizedTextDto {
  fn from(t: &LocalizedText) -> Self {
    Self {
      src: t.src().to_owned(),
      translated: t.translated().to_owned(),
    }
  }
}

impl From<LocalizedTextDto> for LocalizedText {
  fn from(d: LocalizedTextDto) -> Self {
    LocalizedText::from_src_translated(d.src, d.translated)
  }
}

// ---------------------------------------------------------------------------
// MediaDeviceDto + MediaGeoLocationDto (EXIF placeholders)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaDeviceDto {
  #[serde(default)]
  pub make: String,
  #[serde(default)]
  pub model: String,
}

impl From<&MediaDevice> for MediaDeviceDto {
  fn from(d: &MediaDevice) -> Self {
    Self {
      make: d.make().to_owned(),
      model: d.model().to_owned(),
    }
  }
}

impl From<MediaDeviceDto> for MediaDevice {
  fn from(d: MediaDeviceDto) -> Self {
    MediaDevice::from_parts(d.make, d.model)
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaGeoLocationDto {
  pub lat: f64,
  pub lon: f64,
  #[serde(default)]
  pub altitude: Option<f64>,
}

impl From<&MediaGeoLocation> for MediaGeoLocationDto {
  fn from(g: &MediaGeoLocation) -> Self {
    Self {
      lat: g.lat(),
      lon: g.lon(),
      altitude: g.altitude(),
    }
  }
}

impl From<MediaGeoLocationDto> for MediaGeoLocation {
  fn from(d: MediaGeoLocationDto) -> Self {
    MediaGeoLocation::new(d.lat, d.lon, d.altitude)
  }
}

// ---------------------------------------------------------------------------
// ErrorInfoDto — code stored as the verified u32 wire value
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorInfoDto {
  pub code: u32,
  #[serde(default)]
  pub message: String,
}

impl From<&ErrorInfo> for ErrorInfoDto {
  fn from(e: &ErrorInfo) -> Self {
    Self {
      code: e.code().as_u32(),
      message: e.message().to_owned(),
    }
  }
}

impl From<ErrorInfoDto> for ErrorInfo {
  fn from(d: ErrorInfoDto) -> Self {
    ErrorInfo::new(ErrorCode::from_u32(d.code), d.message)
  }
}

// ---------------------------------------------------------------------------
// LocationDto<Uuid> — the structured oneof, serialised as a tagged enum
// ---------------------------------------------------------------------------

/// Wire shape: `{ "kind": "local", "volume": "<uuid>", "components": [...] }`.
///
/// `volume` is stored as the canonical string form (`Uuid7` ↔ `uuid::Uuid`
/// string round-trip), which is stable across the three backends and
/// independent of column type (text/JSON in MySQL/SQLite, JSONB in
/// Postgres). The structured oneof on the wire is `LocationKind::Local`
/// + payload; future variants (RemoteUrl, Object) get new `kind` discriminants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LocationDto {
  Local {
    volume: String,
    components: std::vec::Vec<String>,
  },
}

impl From<&Location<Uuid7>> for LocationDto {
  fn from(l: &Location<Uuid7>) -> Self {
    let local = l.unwrap_local_ref();
    Self::Local {
      volume: local.volume().to_string(),
      components: local.components().iter().map(SmolStr::to_string).collect(),
    }
  }
}

impl TryFrom<LocationDto> for Location<Uuid7> {
  type Error = SqlxError;

  fn try_from(d: LocationDto) -> Result<Self, Self::Error> {
    match d {
      LocationDto::Local { volume, components } => {
        let volume: Uuid7 =
          volume
            .parse()
            .map_err(|e: crate::domain::primitives::Uuid7Error| {
              SqlxError::InvalidUuid(format!("Location.volume: {e}"))
            })?;
        Location::try_local_uuid7(volume, components).map_err(|e: LocationError| {
          SqlxError::DomainConstructorRejected(format!("Location: {e}"))
        })
      }
    }
  }
}

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

/// Serialise a value to a JSON string, surfacing failures as
/// [`SqlxError::InvalidJson`].
pub fn to_json_string<T: Serialize>(v: &T) -> Result<String, SqlxError> {
  serde_json::to_string(v).map_err(|e| SqlxError::InvalidJson(e.to_string()))
}

/// Deserialise a JSON string, surfacing failures as
/// [`SqlxError::InvalidJson`].
pub fn from_json_str<'a, T: Deserialize<'a>>(s: &'a str) -> Result<T, SqlxError> {
  serde_json::from_str(s).map_err(|e| SqlxError::InvalidJson(e.to_string()))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn provenance_roundtrip() {
    let p = Provenance::from_parts("qwen2-vl-7b", "v0.3", "v1", "indexer-0.1");
    let dto: ProvenanceDto = (&p).into();
    let p2: Provenance = dto.into();
    assert_eq!(p, p2);
  }

  #[test]
  fn localized_text_roundtrip() {
    let t = LocalizedText::from_src_translated("hola", "hello");
    let dto: LocalizedTextDto = (&t).into();
    let t2: LocalizedText = dto.into();
    assert_eq!(t, t2);
  }

  #[test]
  fn media_device_roundtrip() {
    let d = MediaDevice::from_parts("Apple", "iPhone 15 Pro");
    let dto: MediaDeviceDto = (&d).into();
    let d2: MediaDevice = dto.into();
    assert_eq!(d, d2);
  }

  #[test]
  fn media_geo_location_roundtrip() {
    let g = MediaGeoLocation::new(37.7749, -122.4194, Some(20.0));
    let dto: MediaGeoLocationDto = (&g).into();
    let g2: MediaGeoLocation = dto.into();
    assert_eq!(g.lat(), g2.lat());
    assert_eq!(g.lon(), g2.lon());
    assert_eq!(g.altitude(), g2.altitude());
  }

  #[test]
  fn error_info_roundtrip_through_u32() {
    let e = ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad header");
    let dto: ErrorInfoDto = (&e).into();
    assert_eq!(dto.code, 1000);
    let e2: ErrorInfo = dto.into();
    assert_eq!(e2.code(), ErrorCode::ProbeCorrupt);
    assert_eq!(e2.message(), "bad header");
  }

  #[test]
  fn location_roundtrip_via_dto() {
    let vol = Uuid7::new();
    let l = Location::try_local_uuid7(vol, ["Movies", "2024"]).unwrap();
    let dto: LocationDto = (&l).into();
    let json = serde_json::to_string(&dto).unwrap();
    let dto2: LocationDto = serde_json::from_str(&json).unwrap();
    let l2: Location<Uuid7> = dto2.try_into().unwrap();
    assert_eq!(l, l2);
  }

  #[test]
  fn location_rejects_invalid_uuid_string() {
    let dto = LocationDto::Local {
      volume: "not-a-uuid".to_owned(),
      components: std::vec!["x".to_owned()],
    };
    let err = Location::<Uuid7>::try_from(dto).unwrap_err();
    assert!(err.is_invalid_uuid());
  }

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
}
