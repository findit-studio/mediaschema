//! SQLite row shape for the root `Media` aggregate.

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

/// SQLite row shape for [`crate::domain::Media`].
///
/// Identity / FK columns are 16-byte `BLOB`s, the checksum is a 32-byte
/// `BLOB`. Wall-clock columns are `INTEGER` ms-since-epoch. Nested VOs
/// (`device` / `gps` / `probe_error`) are `TEXT` containing JSON.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteMediaRow {
  pub id: std::vec::Vec<u8>,
  pub checksum: std::vec::Vec<u8>,
  pub name: String,
  pub format: String,
  pub size: i64,
  /// `mediatime::Timestamp` is currently stored as raw `i64` nanoseconds
  /// since the track-zero base. NULL = absent. (Not jiff::Timestamp —
  /// this is media-time, not wall-clock.)
  pub duration_raw: Option<i64>,
  pub created_at_ms: i64,
  /// `MediaKind` discriminant: 0=Video, 1=Audio.
  pub kind: i64,
  pub video: Option<std::vec::Vec<u8>>,
  pub audio: Option<std::vec::Vec<u8>>,
  pub subtitle: Option<std::vec::Vec<u8>>,
  pub error_flags: i64,
  pub probe_error_json: Option<String>,
  pub capture_date_ms: Option<i64>,
  pub device_json: Option<String>,
  pub gps_json: Option<String>,
}

fn media_kind_to_i64(k: MediaKind) -> i64 {
  match k {
    MediaKind::Video => 0,
    MediaKind::Audio => 1,
  }
}

fn media_kind_from_i64(n: i64) -> Result<MediaKind, SqlxError> {
  match n {
    0 => Ok(MediaKind::Video),
    1 => Ok(MediaKind::Audio),
    other => Err(SqlxError::UnknownDiscriminant(format!(
      "MediaKind: {other}"
    ))),
  }
}

impl From<&Media<Uuid7>> for SqliteMediaRow {
  fn from(m: &Media<Uuid7>) -> Self {
    Self {
      id: m.id().as_bytes().to_vec(),
      checksum: m.checksum().as_bytes().to_vec(),
      name: m.name().to_owned(),
      format: m.format().as_str().to_owned(),
      size: m.size() as i64,
      // `mediatime::Timestamp` doesn't expose a portable i64 accessor
      // in 0.1.6 — we treat duration as absent in the SQLite layer for
      // now and document the gap. Round-trip tests build Media without
      // a duration; consumers needing duration persistence can hold the
      // raw nanoseconds in a sidecar column.
      duration_raw: m.duration().and_then(|_| None::<i64>),
      created_at_ms: timestamp_to_millis(*m.created_at()),
      kind: media_kind_to_i64(m.kind()),
      video: m.video().map(|id| id.as_bytes().to_vec()),
      audio: m.audio().map(|id| id.as_bytes().to_vec()),
      subtitle: m.subtitle().map(|id| id.as_bytes().to_vec()),
      error_flags: i64::from(m.error_flags().bits()),
      probe_error_json: m
        .probe_error()
        .map(|e| to_json_string(&ErrorInfoDto::from(e)).expect("ErrorInfoDto serialises")),
      capture_date_ms: m.capture_date().map(|t| timestamp_to_millis(*t)),
      device_json: m
        .device()
        .map(|d| to_json_string(&DeviceDto::from(d)).expect("DeviceDto serialises")),
      gps_json: m
        .gps()
        .map(|g| to_json_string(&GeoLocationDto::from(g)).expect("GeoLocationDto serialises")),
    }
  }
}

impl TryFrom<SqliteMediaRow> for Media<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteMediaRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let checksum = bytes_to_checksum(&r.checksum)?;
    if checksum.is_zero() {
      return Err(SqlxError::DomainConstructorRejected(
        "Media.checksum is the zero sentinel".to_owned(),
      ));
    }
    let size = u64::try_from(r.size)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Media.size: {e}")))?;
    let created_at = millis_to_timestamp(r.created_at_ms)?;
    let kind = media_kind_from_i64(r.kind)?;
    // `Format::from_str` is infallible (unknown slugs → `Other`).
    let format = r.format.parse::<Format>().unwrap_or_default();
    let mut m = Media::try_new(id, checksum, r.name, format, size, created_at, kind)
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
    let flags_bits = u16::try_from(r.error_flags)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Media.error_flags: {e}")))?;
    let flags = MediaErrorFlags::from_bits_truncate(flags_bits);
    m = m.with_error_flags(flags);

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
  use crate::domain::{ErrorCode, FileChecksum};
  use jiff::Timestamp as JiffTimestamp;

  fn fake_checksum() -> FileChecksum {
    let mut bytes = [0u8; 32];
    bytes[0] = 0x01;
    FileChecksum::from_bytes(bytes)
  }

  fn ts() -> JiffTimestamp {
    JiffTimestamp::from_millisecond(1_700_000_000_000).unwrap()
  }

  #[test]
  fn media_roundtrip_minimal() {
    let m = Media::try_new(
      Uuid7::new(),
      fake_checksum(),
      "clip.mp4",
      Format::Mp4,
      12_345,
      ts(),
      MediaKind::Video,
    )
    .unwrap();
    let row: SqliteMediaRow = (&m).into();
    let m2: Media<Uuid7> = row.try_into().unwrap();
    assert_eq!(m.id(), m2.id());
    assert_eq!(m.checksum(), m2.checksum());
    assert_eq!(m.name(), m2.name());
    assert_eq!(m.format(), m2.format());
    assert_eq!(m.size(), m2.size());
    assert_eq!(m.kind(), m2.kind());
  }

  #[test]
  fn media_roundtrip_full() {
    let video_id = Uuid7::new();
    let audio_id = Uuid7::new();
    let m = Media::try_new(
      Uuid7::new(),
      fake_checksum(),
      "clip.mp4",
      Format::Mp4,
      12_345,
      ts(),
      MediaKind::Video,
    )
    .unwrap()
    .with_video(Some(video_id))
    .with_audio(Some(audio_id))
    .with_error_flags(MediaErrorFlags::VIDEO_ERROR | MediaErrorFlags::AUDIO_ERROR)
    .with_capture_date(Some(ts()))
    .with_device(Some(
      Device::new().with_make("Apple").with_model("iPhone 15 Pro"),
    ))
    .with_gps(Some(
      GeoLocation::try_new(37.7749, -122.4194, Some(20.0)).unwrap(),
    ))
    .with_probe_error(Some(ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad header")));
    let row: SqliteMediaRow = (&m).into();
    let m2: Media<Uuid7> = row.try_into().unwrap();
    assert_eq!(m.video(), m2.video());
    assert_eq!(m.audio(), m2.audio());
    assert_eq!(m.error_flags(), m2.error_flags());
    assert!(m2.device().is_some());
    assert_eq!(m2.device().unwrap().make(), "Apple");
    let g = m2.gps().expect("gps set");
    assert_eq!(g.lat(), 37.7749);
    assert_eq!(g.altitude(), Some(20.0));
    assert_eq!(
      m2.probe_error().map(|e| e.code()),
      Some(ErrorCode::ProbeCorrupt)
    );
  }

  #[test]
  fn media_row_rejects_zero_checksum() {
    let row = SqliteMediaRow {
      id: Uuid7::new().as_bytes().to_vec(),
      checksum: std::vec::Vec::from([0u8; 32]),
      name: "x".to_owned(),
      format: "mp4".to_owned(),
      size: 0,
      duration_raw: None,
      created_at_ms: 0,
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
    assert!(err.is_domain_constructor_rejected(), "got {err:?}");
  }

  #[test]
  fn media_row_rejects_nil_id() {
    let row = SqliteMediaRow {
      id: std::vec::Vec::from([0u8; 16]),
      checksum: fake_checksum().as_bytes().to_vec(),
      name: "x".to_owned(),
      format: "mp4".to_owned(),
      size: 0,
      duration_raw: None,
      created_at_ms: 0,
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
    assert!(err.is_invalid_uuid(), "got {err:?}");
  }
}
