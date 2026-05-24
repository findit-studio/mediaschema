//! MySQL row shape for the root `Media` aggregate.

use mediaframe::{
  capture::{Device, GeoLocation},
  container::Format,
};

use crate::{
  domain::{
    aggregates::media::MediaError, ErrorCode, ErrorInfo, Media, MediaErrorFlags, MediaKind, Uuid7,
  },
  sqlx::{
    dto::{bytes_to_checksum, bytes_to_uuid7, millis_to_timestamp, timestamp_to_millis},
    SqlxError,
  },
};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct MySqlMediaRow {
  pub id: std::vec::Vec<u8>,
  pub checksum: std::vec::Vec<u8>,
  pub format: String,
  pub size: u64,
  pub duration_raw: Option<i64>,
  pub kind: i16,
  pub video_id: Option<std::vec::Vec<u8>>,
  pub audio_id: Option<std::vec::Vec<u8>>,
  pub subtitle_id: Option<std::vec::Vec<u8>>,
  pub error_flags: u16,
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

impl From<&Media<Uuid7>> for MySqlMediaRow {
  fn from(m: &Media<Uuid7>) -> Self {
    let probe_error = m.probe_error_ref();
    let device = m.device_ref();
    let gps = m.gps_ref();
    Self {
      id: m.id_ref().as_bytes().to_vec(),
      checksum: m.checksum_ref().as_bytes().to_vec(),
      format: m.format_ref().as_str().to_owned(),
      size: m.size(),
      duration_raw: m.duration_ref().and_then(|_| None::<i64>),
      kind: media_kind_to_i16(m.kind()),
      video_id: m.video_id_ref().map(|id| id.as_bytes().to_vec()),
      audio_id: m.audio_id_ref().map(|id| id.as_bytes().to_vec()),
      subtitle_id: m.subtitle_id_ref().map(|id| id.as_bytes().to_vec()),
      error_flags: m.error_flags().bits(),
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

impl TryFrom<MySqlMediaRow> for Media<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: MySqlMediaRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let checksum = bytes_to_checksum(&r.checksum)?;
    if checksum.is_zero() {
      return Err(SqlxError::DomainConstructorRejected(
        "Media.checksum is the zero sentinel".to_owned(),
      ));
    }
    let kind = media_kind_from_i16(r.kind)?;
    // `Format::from_str` is infallible (unknown slugs → `Other`).
    let format = r.format.parse::<Format>().unwrap_or_default();
    let mut m = Media::try_new(id, checksum, format, r.size, kind)
      .map_err(|e: MediaError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    if let Some(v) = r.video_id {
      m = m.with_video_id(Some(bytes_to_uuid7(&v)?));
    }
    if let Some(v) = r.audio_id {
      m = m.with_audio_id(Some(bytes_to_uuid7(&v)?));
    }
    if let Some(v) = r.subtitle_id {
      m = m.with_subtitle_id(Some(bytes_to_uuid7(&v)?));
    }
    m = m.with_error_flags(MediaErrorFlags::from_bits_truncate(r.error_flags));
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
      MediaKind::Audio,
    )
    .unwrap()
    .with_audio_id(Some(Uuid7::new()))
    .with_error_flags(MediaErrorFlags::AUDIO_ERROR)
    .with_device(Some(Device::new().with_make("Sony").with_model("A7 IV")));
    let row: MySqlMediaRow = (&m).into();
    let m2: Media<Uuid7> = row.try_into().unwrap();
    assert_eq!(m.id_ref(), m2.id_ref());
    assert_eq!(m.checksum_ref(), m2.checksum_ref());
    assert_eq!(m.kind(), m2.kind());
    assert_eq!(m.audio_id_ref(), m2.audio_id_ref());
    assert_eq!(m.error_flags(), m2.error_flags());
    assert_eq!(m2.device_ref().unwrap().model(), "A7 IV");
  }

  #[test]
  fn media_row_rejects_zero_checksum() {
    let row = MySqlMediaRow {
      id: Uuid7::new().as_bytes().to_vec(),
      checksum: std::vec::Vec::from([0u8; 32]),
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
    assert!(err.is_domain_constructor_rejected());
  }
}
