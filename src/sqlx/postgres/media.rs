//! PostgreSQL row shape for the root `Media` aggregate.

use mediaframe::{
  capture::{Device, GeoLocation},
  container::Format,
};
use uuid::Uuid;

use crate::{
  domain::{
    aggregates::media::MediaError, ErrorCode, ErrorInfo, Media, MediaErrorFlags, MediaKind, Uuid7,
  },
  sqlx::{
    dto::{
      bytes_to_checksum, millis_to_timestamp, timestamp_to_millis, uuid7_to_uuid, uuid_to_uuid7,
    },
    SqlxError,
  },
};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PgMediaRow {
  pub id: Uuid,
  pub checksum: std::vec::Vec<u8>,
  pub format: String,
  pub size: i64,
  pub duration_raw: Option<i64>,
  pub kind: i16,
  pub video_id: Option<Uuid>,
  pub audio_id: Option<Uuid>,
  pub subtitle_id: Option<Uuid>,
  pub error_flags: i32,
  /// `ErrorInfo.code` as the verified `u32` wire value; NULL = no probe
  /// error. Discriminates presence of the flattened `ErrorInfo` VO.
  pub probe_error_code: Option<i32>,
  pub probe_error_message: Option<String>,
  pub capture_date_ms: Option<i64>,
  /// Capture `Device.make`; both `device_*` NULL = absent `Device`.
  pub device_make: Option<String>,
  pub device_model: Option<String>,
  /// `GeoLocation.lat`; NULL discriminates an absent `GeoLocation`.
  pub gps_lat: Option<f64>,
  pub gps_lon: Option<f64>,
  pub gps_altitude: Option<f32>,
}

fn media_kind_to_i16(k: MediaKind) -> i16 {
  match k {
    MediaKind::Video => 0,
    MediaKind::Audio => 1,
  }
}

fn media_kind_from_i16(n: i16) -> Result<MediaKind, SqlxError> {
  match n {
    0 => Ok(MediaKind::Video),
    1 => Ok(MediaKind::Audio),
    other => Err(SqlxError::UnknownDiscriminant(format!(
      "MediaKind: {other}"
    ))),
  }
}

impl From<&Media<Uuid7>> for PgMediaRow {
  fn from(m: &Media<Uuid7>) -> Self {
    let probe_error = m.probe_error_ref();
    let device = m.device_ref();
    let gps = m.gps_ref();
    Self {
      id: uuid7_to_uuid(*m.id_ref()),
      checksum: m.checksum_ref().as_bytes().to_vec(),
      format: m.format_ref().as_str().to_owned(),
      size: m.size() as i64,
      duration_raw: m.duration_ref().and_then(|_| None::<i64>),
      kind: media_kind_to_i16(m.kind()),
      video_id: m.video_id_ref().map(|id| uuid7_to_uuid(*id)),
      audio_id: m.audio_id_ref().map(|id| uuid7_to_uuid(*id)),
      subtitle_id: m.subtitle_id_ref().map(|id| uuid7_to_uuid(*id)),
      error_flags: i32::from(m.error_flags().bits()),
      probe_error_code: probe_error.map(|e| e.code().as_u32() as i32),
      probe_error_message: probe_error.map(|e| e.message().to_owned()),
      capture_date_ms: m.capture_date_ref().map(|t| timestamp_to_millis(*t)),
      device_make: device.map(|d| d.make().to_owned()),
      device_model: device.map(|d| d.model().to_owned()),
      gps_lat: gps.map(GeoLocation::lat),
      gps_lon: gps.map(GeoLocation::lon),
      gps_altitude: gps.and_then(GeoLocation::altitude),
    }
  }
}

impl TryFrom<PgMediaRow> for Media<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgMediaRow) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let checksum = bytes_to_checksum(&r.checksum)?;
    if checksum.is_zero() {
      return Err(SqlxError::DomainConstructorRejected(
        "Media.checksum is the zero sentinel".to_owned(),
      ));
    }
    let size = u64::try_from(r.size)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Media.size: {e}")))?;
    let kind = media_kind_from_i16(r.kind)?;
    // `Format::from_str` is infallible (unknown slugs → `Other`).
    let format = r.format.parse::<Format>().unwrap_or_default();
    let mut m = Media::try_new(id, checksum, format, size, kind)
      .map_err(|e: MediaError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    if let Some(v) = r.video_id {
      m = m.with_video_id(Some(uuid_to_uuid7(v)?));
    }
    if let Some(v) = r.audio_id {
      m = m.with_audio_id(Some(uuid_to_uuid7(v)?));
    }
    if let Some(v) = r.subtitle_id {
      m = m.with_subtitle_id(Some(uuid_to_uuid7(v)?));
    }
    let flag_bits = u16::try_from(r.error_flags)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Media.error_flags: {e}")))?;
    m = m.with_error_flags(MediaErrorFlags::from_bits_truncate(flag_bits));
    if let Some(code) = r.probe_error_code {
      let code = u32::try_from(code)
        .map_err(|e| SqlxError::UnknownDiscriminant(format!("Media.probe_error_code: {e}")))?;
      m = m.with_probe_error(Some(ErrorInfo::new(
        ErrorCode::from_u32(code),
        r.probe_error_message.unwrap_or_default(),
      )));
    }
    if let Some(ms) = r.capture_date_ms {
      m = m.with_capture_date(Some(millis_to_timestamp(ms)?));
    }
    if r.device_make.is_some() || r.device_model.is_some() {
      m = m.with_device(Some(
        Device::new()
          .with_make(r.device_make.unwrap_or_default())
          .with_model(r.device_model.unwrap_or_default()),
      ));
    }
    if let Some(lat) = r.gps_lat {
      let lon = r.gps_lon.ok_or_else(|| {
        SqlxError::DomainConstructorRejected(
          "Media.gps_lon missing while gps_lat present".to_owned(),
        )
      })?;
      m = m.with_gps(Some(
        GeoLocation::try_new(lat, lon, r.gps_altitude)
          .map_err(|e| SqlxError::DomainConstructorRejected(format!("GeoLocation: {e}")))?,
      ));
    }
    Ok(m)
  }
}

/// Borrowed view of [`PgMediaRow`] — zero-copy decode from `&'r Row`.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PgMediaRowRef<'r> {
  pub id: Uuid,
  pub checksum: &'r [u8],
  pub format: &'r str,
  pub size: i64,
  pub duration_raw: Option<i64>,
  pub kind: i16,
  pub video_id: Option<Uuid>,
  pub audio_id: Option<Uuid>,
  pub subtitle_id: Option<Uuid>,
  pub error_flags: i32,
  pub probe_error_code: Option<i32>,
  pub probe_error_message: Option<&'r str>,
  pub capture_date_ms: Option<i64>,
  pub device_make: Option<&'r str>,
  pub device_model: Option<&'r str>,
  pub gps_lat: Option<f64>,
  pub gps_lon: Option<f64>,
  pub gps_altitude: Option<f32>,
}

impl PgMediaRow {
  /// Cheap borrow — produces a [`PgMediaRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgMediaRowRef<'_> {
    PgMediaRowRef {
      id: self.id,
      checksum: &self.checksum,
      format: &self.format,
      size: self.size,
      duration_raw: self.duration_raw,
      kind: self.kind,
      video_id: self.video_id,
      audio_id: self.audio_id,
      subtitle_id: self.subtitle_id,
      error_flags: self.error_flags,
      probe_error_code: self.probe_error_code,
      probe_error_message: self.probe_error_message.as_deref(),
      capture_date_ms: self.capture_date_ms,
      device_make: self.device_make.as_deref(),
      device_model: self.device_model.as_deref(),
      gps_lat: self.gps_lat,
      gps_lon: self.gps_lon,
      gps_altitude: self.gps_altitude,
    }
  }
}

impl<'r> TryFrom<PgMediaRowRef<'r>> for Media<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgMediaRowRef<'r>) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let checksum = bytes_to_checksum(r.checksum)?;
    if checksum.is_zero() {
      return Err(SqlxError::DomainConstructorRejected(
        "Media.checksum is the zero sentinel".to_owned(),
      ));
    }
    let size = u64::try_from(r.size)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Media.size: {e}")))?;
    let kind = media_kind_from_i16(r.kind)?;
    let format = r.format.parse::<Format>().unwrap_or_default();
    let mut m = Media::try_new(id, checksum, format, size, kind)
      .map_err(|e: MediaError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    if let Some(v) = r.video_id {
      m = m.with_video_id(Some(uuid_to_uuid7(v)?));
    }
    if let Some(v) = r.audio_id {
      m = m.with_audio_id(Some(uuid_to_uuid7(v)?));
    }
    if let Some(v) = r.subtitle_id {
      m = m.with_subtitle_id(Some(uuid_to_uuid7(v)?));
    }
    let flag_bits = u16::try_from(r.error_flags)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Media.error_flags: {e}")))?;
    m = m.with_error_flags(MediaErrorFlags::from_bits_truncate(flag_bits));
    if let Some(code) = r.probe_error_code {
      let code = u32::try_from(code)
        .map_err(|e| SqlxError::UnknownDiscriminant(format!("Media.probe_error_code: {e}")))?;
      m = m.with_probe_error(Some(ErrorInfo::new(
        ErrorCode::from_u32(code),
        r.probe_error_message.unwrap_or_default(),
      )));
    }
    if let Some(ms) = r.capture_date_ms {
      m = m.with_capture_date(Some(millis_to_timestamp(ms)?));
    }
    if r.device_make.is_some() || r.device_model.is_some() {
      m = m.with_device(Some(
        Device::new()
          .with_make(r.device_make.unwrap_or_default())
          .with_model(r.device_model.unwrap_or_default()),
      ));
    }
    if let Some(lat) = r.gps_lat {
      let lon = r.gps_lon.ok_or_else(|| {
        SqlxError::DomainConstructorRejected(
          "Media.gps_lon missing while gps_lat present".to_owned(),
        )
      })?;
      m = m.with_gps(Some(
        GeoLocation::try_new(lat, lon, r.gps_altitude)
          .map_err(|e| SqlxError::DomainConstructorRejected(format!("GeoLocation: {e}")))?,
      ));
    }
    Ok(m)
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::FileChecksum;

  fn fake_checksum() -> FileChecksum {
    let mut b = [0u8; 32];
    b[0] = 0x01;
    FileChecksum::from_bytes(b)
  }

  #[test]
  fn media_roundtrip() {
    let m = Media::try_new(
      Uuid7::new(),
      fake_checksum(),
      Format::Mp4,
      1,
      MediaKind::Video,
    )
    .unwrap()
    .with_device(Some(Device::new().with_make("Apple").with_model("iPhone")))
    .with_gps(Some(
      GeoLocation::try_new(37.7749, -122.4194, Some(20.0)).unwrap(),
    ))
    .with_probe_error(Some(ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad header")));
    let row: PgMediaRow = (&m).into();
    let m2: Media<Uuid7> = row.try_into().unwrap();
    assert_eq!(m.id_ref(), m2.id_ref());
    assert_eq!(m.checksum_ref(), m2.checksum_ref());
    assert_eq!(m2.device_ref().unwrap().make(), "Apple");
    let g = m2.gps_ref().unwrap();
    assert_eq!(g.lat(), 37.7749);
    assert_eq!(g.altitude(), Some(20.0));
    assert_eq!(
      m2.probe_error_ref().map(|e| e.code()),
      Some(ErrorCode::ProbeCorrupt)
    );
    assert_eq!(
      m2.probe_error_ref().map(|e| e.message()),
      Some("bad header")
    );
  }

  #[test]
  fn media_ref_roundtrip() {
    let m = Media::try_new(
      Uuid7::new(),
      fake_checksum(),
      Format::Mp4,
      1,
      MediaKind::Video,
    )
    .unwrap()
    .with_device(Some(Device::new().with_make("Apple").with_model("iPhone")))
    .with_gps(Some(
      GeoLocation::try_new(37.7749, -122.4194, Some(20.0)).unwrap(),
    ))
    .with_probe_error(Some(ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad header")));
    let row: PgMediaRow = (&m).into();
    let m2: Media<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(m.id_ref(), m2.id_ref());
    assert_eq!(m.checksum_ref(), m2.checksum_ref());
    assert_eq!(m2.device_ref().unwrap().make(), "Apple");
    assert_eq!(
      m2.probe_error_ref().map(|e| e.message()),
      Some("bad header")
    );
  }

  #[test]
  fn media_row_rejects_nil_id() {
    let row = PgMediaRow {
      id: uuid::Uuid::nil(),
      checksum: fake_checksum().as_bytes().to_vec(),
      format: String::new(),
      size: 0,
      duration_raw: None,
      kind: 0,
      video_id: None,
      audio_id: None,
      subtitle_id: None,
      error_flags: 0,
      probe_error_code: None,
      probe_error_message: None,
      capture_date_ms: None,
      device_make: None,
      device_model: None,
      gps_lat: None,
      gps_lon: None,
      gps_altitude: None,
    };
    let err = Media::<Uuid7>::try_from(row).unwrap_err();
    assert!(err.is_invalid_uuid());
  }
}
