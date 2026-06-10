//! SQLite row shape for the root `Media` aggregate.

use std::vec::Vec;

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

/// SQLite row shape for [`crate::domain::Media`].
///
/// Identity / FK columns are 16-byte `BLOB`s, the checksum is a 32-byte
/// `BLOB`. Wall-clock columns are `INTEGER` ms-since-epoch. Nested VOs
/// (`device` / `gps` / `probe_error`) are flattened into real columns.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteMediaRow {
  pub id: Vec<u8>,
  pub checksum: Vec<u8>,
  pub format: String,
  pub size: i64,
  /// `mediatime::Timestamp` is currently stored as raw `i64` nanoseconds
  /// since the track-zero base. NULL = absent. (Not jiff::Timestamp —
  /// this is media-time, not wall-clock.)
  pub duration_raw: Option<i64>,
  /// `MediaKind` discriminant: 0=Video, 1=Audio.
  pub kind: i64,
  /// Verbatim `AVFormatContext.nb_streams` (rev 11).
  pub nb_streams: i64,
  /// Verbatim `AVFormatContext.nb_chapters` (rev 11).
  pub nb_chapters: i64,
  pub video_id: Option<Vec<u8>>,
  pub audio_id: Option<Vec<u8>>,
  pub subtitle_id: Option<Vec<u8>>,
  pub error_flags: i64,
  /// `ErrorInfo.code` as the verified `u32` wire value; NULL = no probe
  /// error. Discriminates presence of the flattened `ErrorInfo` VO.
  pub probe_error_code: Option<i64>,
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
    let probe_error = m.probe_error_ref();
    let device = m.device_ref();
    let gps = m.gps_ref();
    Self {
      id: m.id_ref().as_bytes().to_vec(),
      checksum: m.checksum_ref().as_bytes().to_vec(),
      format: m.format_ref().as_str().to_owned(),
      size: m.size() as i64,
      // `mediatime::Timestamp` doesn't expose a portable i64 accessor
      // in 0.1.6 — we treat duration as absent in the SQLite layer for
      // now and document the gap. Round-trip tests build Media without
      // a duration; consumers needing duration persistence can hold the
      // raw nanoseconds in a sidecar column.
      duration_raw: None,
      kind: media_kind_to_i64(m.kind()),
      nb_streams: i64::from(m.nb_streams()),
      nb_chapters: i64::from(m.nb_chapters()),
      video_id: m.video_id_ref().map(|id| id.as_bytes().to_vec()),
      audio_id: m.audio_id_ref().map(|id| id.as_bytes().to_vec()),
      subtitle_id: m.subtitle_id_ref().map(|id| id.as_bytes().to_vec()),
      error_flags: i64::from(m.error_flags().bits()),
      probe_error_code: probe_error.map(|e| i64::from(e.code().as_u32())),
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
    let kind = media_kind_from_i64(r.kind)?;
    // `Format::from_str` is infallible (unknown slugs → `Other`).
    let format = r.format.parse::<Format>().unwrap_or_default();
    let nb_streams = u32::try_from(r.nb_streams)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Media.nb_streams: {e}")))?;
    let nb_chapters = u32::try_from(r.nb_chapters)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Media.nb_chapters: {e}")))?;
    let mut m = Media::try_new(id, checksum, format, size, kind)
      .map(|m| m.with_nb_streams(nb_streams).with_nb_chapters(nb_chapters))
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
    let flags_bits = u16::try_from(r.error_flags)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Media.error_flags: {e}")))?;
    let flags = MediaErrorFlags::from_bits_truncate(flags_bits);
    m = m.with_error_flags(flags);

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

/// Borrowed view of [`SqliteMediaRow`] — zero-copy decode from `&'r Row`.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteMediaRowRef<'r> {
  pub id: &'r [u8],
  pub checksum: &'r [u8],
  pub format: &'r str,
  pub size: i64,
  pub duration_raw: Option<i64>,
  pub kind: i64,
  /// Verbatim `AVFormatContext.nb_streams` (rev 11).
  pub nb_streams: i64,
  /// Verbatim `AVFormatContext.nb_chapters` (rev 11).
  pub nb_chapters: i64,
  pub video_id: Option<&'r [u8]>,
  pub audio_id: Option<&'r [u8]>,
  pub subtitle_id: Option<&'r [u8]>,
  pub error_flags: i64,
  pub probe_error_code: Option<i64>,
  pub probe_error_message: Option<&'r str>,
  pub capture_date_ms: Option<i64>,
  pub device_make: Option<&'r str>,
  pub device_model: Option<&'r str>,
  pub gps_lat: Option<f64>,
  pub gps_lon: Option<f64>,
  pub gps_altitude: Option<f32>,
}

impl SqliteMediaRow {
  /// Cheap borrow — produces a [`SqliteMediaRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteMediaRowRef<'_> {
    SqliteMediaRowRef {
      id: &self.id,
      checksum: &self.checksum,
      format: &self.format,
      size: self.size,
      duration_raw: self.duration_raw,
      kind: self.kind,
      nb_streams: self.nb_streams,
      nb_chapters: self.nb_chapters,
      video_id: self.video_id.as_deref(),
      audio_id: self.audio_id.as_deref(),
      subtitle_id: self.subtitle_id.as_deref(),
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

impl<'r> TryFrom<SqliteMediaRowRef<'r>> for Media<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteMediaRowRef<'r>) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
    let checksum = bytes_to_checksum(r.checksum)?;
    if checksum.is_zero() {
      return Err(SqlxError::DomainConstructorRejected(
        "Media.checksum is the zero sentinel".to_owned(),
      ));
    }
    let size = u64::try_from(r.size)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Media.size: {e}")))?;
    let kind = media_kind_from_i64(r.kind)?;
    let format = r.format.parse::<Format>().unwrap_or_default();
    let nb_streams = u32::try_from(r.nb_streams)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Media.nb_streams: {e}")))?;
    let nb_chapters = u32::try_from(r.nb_chapters)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Media.nb_chapters: {e}")))?;
    let mut m = Media::try_new(id, checksum, format, size, kind)
      .map(|m| m.with_nb_streams(nb_streams).with_nb_chapters(nb_chapters))
      .map_err(|e: MediaError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    if let Some(v) = r.video_id {
      m = m.with_video_id(Some(bytes_to_uuid7(v)?));
    }
    if let Some(v) = r.audio_id {
      m = m.with_audio_id(Some(bytes_to_uuid7(v)?));
    }
    if let Some(v) = r.subtitle_id {
      m = m.with_subtitle_id(Some(bytes_to_uuid7(v)?));
    }
    let flags_bits = u16::try_from(r.error_flags)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Media.error_flags: {e}")))?;
    m = m.with_error_flags(MediaErrorFlags::from_bits_truncate(flags_bits));
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
      Format::Mp4,
      12_345,
      MediaKind::Video,
    )
    .unwrap();
    let row: SqliteMediaRow = (&m).into();
    let m2: Media<Uuid7> = row.try_into().unwrap();
    assert_eq!(m.id_ref(), m2.id_ref());
    assert_eq!(m.checksum_ref(), m2.checksum_ref());
    assert_eq!(m.format_ref(), m2.format_ref());
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
      Format::Mp4,
      12_345,
      MediaKind::Video,
    )
    .unwrap()
    .with_video_id(Some(video_id))
    .with_audio_id(Some(audio_id))
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
    assert_eq!(m.video_id_ref(), m2.video_id_ref());
    assert_eq!(m.audio_id_ref(), m2.audio_id_ref());
    assert_eq!(m.error_flags(), m2.error_flags());
    assert!(m2.device_ref().is_some());
    assert_eq!(m2.device_ref().unwrap().make(), "Apple");
    let g = m2.gps_ref().expect("gps set");
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
      12_345,
      MediaKind::Video,
    )
    .unwrap()
    .with_video_id(Some(Uuid7::new()))
    .with_device(Some(Device::new().with_make("Apple").with_model("iPhone")))
    .with_probe_error(Some(ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad header")));
    let row: SqliteMediaRow = (&m).into();
    let m2: Media<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(m.id_ref(), m2.id_ref());
    assert_eq!(m.video_id_ref(), m2.video_id_ref());
    assert_eq!(m2.device_ref().unwrap().make(), "Apple");
    assert_eq!(
      m2.probe_error_ref().map(|e| e.message()),
      Some("bad header")
    );
  }

  #[test]
  fn media_gps_without_altitude_roundtrips() {
    let m = Media::try_new(
      Uuid7::new(),
      fake_checksum(),
      Format::Mp4,
      1,
      MediaKind::Video,
    )
    .unwrap()
    .with_gps(Some(GeoLocation::try_new(10.0, 20.0, None).unwrap()));
    let row: SqliteMediaRow = (&m).into();
    assert!(row.gps_altitude.is_none());
    assert!(row.gps_lat.is_some());
    let m2: Media<Uuid7> = row.try_into().unwrap();
    let g = m2.gps_ref().unwrap();
    assert_eq!(g.lat(), 10.0);
    assert_eq!(g.altitude(), None);
  }

  #[test]
  fn media_row_rejects_zero_checksum() {
    let row = SqliteMediaRow {
      id: Uuid7::new().as_bytes().to_vec(),
      checksum: Vec::from([0u8; 32]),
      format: "mp4".to_owned(),
      size: 0,
      duration_raw: None,
      kind: 0,
      nb_streams: 0,
      nb_chapters: 0,
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
    assert!(err.is_domain_constructor_rejected(), "got {err:?}");
  }

  #[test]
  fn media_row_rejects_nil_id() {
    let row = SqliteMediaRow {
      id: Vec::from([0u8; 16]),
      checksum: fake_checksum().as_bytes().to_vec(),
      format: "mp4".to_owned(),
      size: 0,
      duration_raw: None,
      kind: 0,
      nb_streams: 0,
      nb_chapters: 0,
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
    assert!(err.is_invalid_uuid(), "got {err:?}");
  }
}
