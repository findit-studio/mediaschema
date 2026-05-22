//! MySQL row shape for the root `Media` aggregate.

use mediaframe::{
  capture::{Device, GeoLocation},
  container::Format,
};

use crate::{
  domain::{aggregates::media::MediaError, ErrorInfo, Media, MediaErrorFlags, MediaKind, Uuid7},
  sqlx::{
    dto::{
      bytes_to_checksum, bytes_to_uuid7, from_json_str, millis_to_timestamp, timestamp_to_millis,
      to_json_string, DeviceDto, ErrorInfoDto, GeoLocationDto,
    },
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
  pub video: Option<std::vec::Vec<u8>>,
  pub audio: Option<std::vec::Vec<u8>>,
  pub subtitle: Option<std::vec::Vec<u8>>,
  pub error_flags: u16,
  pub probe_error_json: Option<String>,
  pub capture_date_ms: Option<i64>,
  pub device_json: Option<String>,
  pub gps_json: Option<String>,
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
    Self {
      id: m.id_ref().as_bytes().to_vec(),
      checksum: m.checksum_ref().as_bytes().to_vec(),
      format: m.format_ref().as_str().to_owned(),
      size: m.size(),
      duration_raw: m.duration_ref().and_then(|_| None::<i64>),
      kind: media_kind_to_i16(m.kind()),
      video: m.video_ref().map(|id| id.as_bytes().to_vec()),
      audio: m.audio_ref().map(|id| id.as_bytes().to_vec()),
      subtitle: m.subtitle_ref().map(|id| id.as_bytes().to_vec()),
      error_flags: m.error_flags().bits(),
      probe_error_json: m
        .probe_error_ref()
        .map(|e| to_json_string(&ErrorInfoDto::from(e)).expect("ErrorInfoDto serialises")),
      capture_date_ms: m.capture_date_ref().map(|t| timestamp_to_millis(*t)),
      device_json: m
        .device_ref()
        .map(|d| to_json_string(&DeviceDto::from(d)).expect("DeviceDto serialises")),
      gps_json: m
        .gps_ref()
        .map(|g| to_json_string(&GeoLocationDto::from(g)).expect("GeoLocationDto serialises")),
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
    if let Some(v) = r.video {
      m = m.with_video(Some(bytes_to_uuid7(&v)?));
    }
    if let Some(v) = r.audio {
      m = m.with_audio(Some(bytes_to_uuid7(&v)?));
    }
    if let Some(v) = r.subtitle {
      m = m.with_subtitle(Some(bytes_to_uuid7(&v)?));
    }
    m = m.with_error_flags(MediaErrorFlags::from_bits_truncate(r.error_flags));
    if let Some(j) = r.probe_error_json {
      let dto: ErrorInfoDto = from_json_str(&j)?;
      m = m.with_probe_error(Some(ErrorInfo::from(dto)));
    }
    if let Some(ms) = r.capture_date_ms {
      m = m.with_capture_date(Some(millis_to_timestamp(ms)?));
    }
    if let Some(j) = r.device_json {
      let dto: DeviceDto = from_json_str(&j)?;
      m = m.with_device(Some(Device::from(dto)));
    }
    if let Some(j) = r.gps_json {
      let dto: GeoLocationDto = from_json_str(&j)?;
      m = m.with_gps(Some(GeoLocation::try_from(dto)?));
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
    let m = Media::try_new(Uuid7::new(), fake_checksum(), Format::Mp4, 1, MediaKind::Audio)
      .unwrap()
      .with_audio(Some(Uuid7::new()))
      .with_error_flags(MediaErrorFlags::AUDIO_ERROR);
    let row: MySqlMediaRow = (&m).into();
    let m2: Media<Uuid7> = row.try_into().unwrap();
    assert_eq!(m.id_ref(), m2.id_ref());
    assert_eq!(m.checksum_ref(), m2.checksum_ref());
    assert_eq!(m.kind(), m2.kind());
    assert_eq!(m.audio_ref(), m2.audio_ref());
    assert_eq!(m.error_flags(), m2.error_flags());
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
      video: None,
      audio: None,
      subtitle: None,
      error_flags: 0,
      probe_error_json: None,
      capture_date_ms: None,
      device_json: None,
      gps_json: None,
    };
    let err = Media::<Uuid7>::try_from(row).unwrap_err();
    assert!(err.is_domain_constructor_rejected());
  }
}
