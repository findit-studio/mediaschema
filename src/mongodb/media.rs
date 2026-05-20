//! `Media` ↔ bson `Document` mapping.
//!
//! See module-root docs for the high-level mapping rules. This file
//! covers [`Media`], its nested [`MediaDevice`] / [`MediaGeoLocation`]
//! VOs, and the [`MediaKind`] / [`MediaErrorFlags`] discriminants.

use ::bson::{Bson, Document};

use crate::domain::{
  aggregates::media::{Media, MediaDevice, MediaGeoLocation},
  enums::MediaKind,
  MediaErrorFlags, Uuid7,
};

use super::{error::MongoError, util::*};

// ---------------------------------------------------------------------------
// MediaKind / MediaErrorFlags helpers
// ---------------------------------------------------------------------------

fn media_kind_to_i32(k: MediaKind) -> i32 {
  match k {
    MediaKind::Video => 0,
    MediaKind::Audio => 1,
  }
}

fn media_kind_from_i64(v: i64, field: &'static str) -> Result<MediaKind, MongoError> {
  match v {
    0 => Ok(MediaKind::Video),
    1 => Ok(MediaKind::Audio),
    _ => Err(MongoError::IntOutOfRange {
      field: smol_str::SmolStr::from(field),
      value: v,
    }),
  }
}

// ---------------------------------------------------------------------------
// MediaDevice
// ---------------------------------------------------------------------------

fn media_device_to_bson(d: &MediaDevice) -> Bson {
  let mut doc = Document::new();
  doc.insert("make", Bson::String(d.make().to_owned()));
  doc.insert("model", Bson::String(d.model().to_owned()));
  Bson::Document(doc)
}

fn media_device_from_bson(b: Bson, field: &'static str) -> Result<MediaDevice, MongoError> {
  let mut d = as_doc(b, field)?;
  let make = as_smol(take(&mut d, "make")?, "make")?;
  let model = as_smol(take(&mut d, "model")?, "model")?;
  Ok(MediaDevice::from_parts(make, model))
}

// ---------------------------------------------------------------------------
// MediaGeoLocation
// ---------------------------------------------------------------------------

fn media_geo_to_bson(g: &MediaGeoLocation) -> Bson {
  let mut doc = Document::new();
  doc.insert("lat", Bson::Double(g.lat()));
  doc.insert("lon", Bson::Double(g.lon()));
  doc.insert(
    "altitude",
    g.altitude().map(Bson::Double).unwrap_or(Bson::Null),
  );
  Bson::Document(doc)
}

fn media_geo_from_bson(b: Bson, field: &'static str) -> Result<MediaGeoLocation, MongoError> {
  let mut d = as_doc(b, field)?;
  let lat = as_f64(take(&mut d, "lat")?, "lat")?;
  let lon = as_f64(take(&mut d, "lon")?, "lon")?;
  let altitude = opt(take_opt(&mut d, "altitude"), |bb| as_f64(bb, "altitude"))?;
  Ok(MediaGeoLocation::new(lat, lon, altitude))
}

// ---------------------------------------------------------------------------
// Media
// ---------------------------------------------------------------------------

impl From<&Media<Uuid7>> for Document {
  fn from(m: &Media<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*m.id()));
    d.insert("checksum", checksum_to_bson(m.checksum()));
    d.insert("name", Bson::String(m.name().to_owned()));
    d.insert("format", Bson::String(m.format().to_owned()));
    d.insert("size", Bson::Int64(m.size() as i64));
    d.insert(
      "duration",
      m.duration()
        .map(|t| media_ts_to_bson(*t))
        .unwrap_or(Bson::Null),
    );
    d.insert("created_at", jiff_to_bson(*m.created_at()));
    d.insert("kind", Bson::Int32(media_kind_to_i32(m.kind())));
    d.insert(
      "video",
      m.video().map(|i| uuid7_to_bson(*i)).unwrap_or(Bson::Null),
    );
    d.insert(
      "audio",
      m.audio().map(|i| uuid7_to_bson(*i)).unwrap_or(Bson::Null),
    );
    d.insert(
      "subtitle",
      m.subtitle()
        .map(|i| uuid7_to_bson(*i))
        .unwrap_or(Bson::Null),
    );
    d.insert("error_flags", Bson::Int64(m.error_flags().bits() as i64));
    d.insert(
      "probe_error",
      m.probe_error()
        .map(error_info_to_bson)
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "capture_date",
      m.capture_date()
        .map(|t| jiff_to_bson(*t))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "device",
      m.device().map(media_device_to_bson).unwrap_or(Bson::Null),
    );
    d.insert("gps", m.gps().map(media_geo_to_bson).unwrap_or(Bson::Null));
    d
  }
}

impl TryFrom<Document> for Media<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let checksum = checksum_from_bson(take(&mut d, "checksum")?, "checksum")?;
    let name = as_smol(take(&mut d, "name")?, "name")?;
    let format = as_smol(take(&mut d, "format")?, "format")?;
    let size = as_u64(take(&mut d, "size")?, "size")?;
    let created_at = jiff_from_bson(take(&mut d, "created_at")?, "created_at")?;
    let kind = media_kind_from_i64(as_i64(take(&mut d, "kind")?, "kind")?, "kind")?;
    let mut m = Media::try_new(id, checksum, name, format, size, created_at, kind)?;

    if let Some(b) = take_opt(&mut d, "duration") {
      m.set_duration(Some(media_ts_from_bson(b, "duration")?));
    }
    if let Some(b) = take_opt(&mut d, "video") {
      m.set_video(Some(uuid7_from_bson(b, "video")?));
    }
    if let Some(b) = take_opt(&mut d, "audio") {
      m.set_audio(Some(uuid7_from_bson(b, "audio")?));
    }
    if let Some(b) = take_opt(&mut d, "subtitle") {
      m.set_subtitle(Some(uuid7_from_bson(b, "subtitle")?));
    }
    if let Some(b) = take_opt(&mut d, "error_flags") {
      let bits = as_u64(b, "error_flags")?;
      let bits16 = u16::try_from(bits).map_err(|_| MongoError::IntOutOfRange {
        field: smol_str::SmolStr::from("error_flags"),
        value: bits as i64,
      })?;
      m.set_error_flags(MediaErrorFlags::from_bits_truncate(bits16));
    }
    if let Some(b) = take_opt(&mut d, "probe_error") {
      m.set_probe_error(Some(error_info_from_bson(b, "probe_error")?));
    }
    if let Some(b) = take_opt(&mut d, "capture_date") {
      m.set_capture_date(Some(jiff_from_bson(b, "capture_date")?));
    }
    if let Some(b) = take_opt(&mut d, "device") {
      m.set_device(Some(media_device_from_bson(b, "device")?));
    }
    if let Some(b) = take_opt(&mut d, "gps") {
      m.set_gps(Some(media_geo_from_bson(b, "gps")?));
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
  use crate::domain::primitives::{ErrorCode, ErrorInfo, FileChecksum};
  use core::num::NonZeroU32;
  use jiff::Timestamp as JiffTimestamp;
  use mediatime::{Timebase, Timestamp as MediaTimestamp};

  fn cs() -> FileChecksum {
    let mut b = [0u8; 32];
    b[0] = 1;
    FileChecksum::from_bytes(b)
  }

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).unwrap())
  }

  #[test]
  fn media_minimal_roundtrip() {
    let m = Media::try_new(
      Uuid7::new(),
      cs(),
      "clip.mp4",
      "mp4",
      12_345,
      JiffTimestamp::default(),
      MediaKind::Video,
    )
    .unwrap();
    let doc: Document = (&m).into();
    let m2: Media<Uuid7> = doc.try_into().unwrap();
    assert_eq!(m, m2);
  }

  #[test]
  fn media_full_roundtrip() {
    let m = Media::try_new(
      Uuid7::new(),
      cs(),
      "clip.mp4",
      "mp4",
      999_999,
      JiffTimestamp::default(),
      MediaKind::Audio,
    )
    .unwrap()
    .with_video(Some(Uuid7::new()))
    .with_audio(Some(Uuid7::new()))
    .with_subtitle(Some(Uuid7::new()))
    .with_duration(Some(MediaTimestamp::new(60_000, tb())))
    .with_error_flags(MediaErrorFlags::AUDIO_ERROR | MediaErrorFlags::SUBTITLE_ERROR)
    .with_probe_error(Some(ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad header")))
    .with_capture_date(Some(JiffTimestamp::default()))
    .with_device(Some(MediaDevice::from_parts("Apple", "iPhone 15 Pro")))
    .with_gps(Some(MediaGeoLocation::new(37.7749, -122.4194, Some(20.0))));
    let doc: Document = (&m).into();
    let m2: Media<Uuid7> = doc.try_into().unwrap();
    assert_eq!(m, m2);
  }

  #[test]
  fn media_missing_id_errors() {
    let mut d = Document::new();
    d.insert("checksum", checksum_to_bson(&cs()));
    let err = Media::<Uuid7>::try_from(d).unwrap_err();
    assert!(err.is_missing_field());
  }

  #[test]
  fn media_zero_checksum_rejected() {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(Uuid7::new()));
    d.insert("checksum", checksum_to_bson(&FileChecksum::new()));
    d.insert("name", "x");
    d.insert("format", "mp4");
    d.insert("size", Bson::Int64(0));
    d.insert("created_at", jiff_to_bson(JiffTimestamp::default()));
    d.insert("kind", Bson::Int32(0));
    let err = Media::<Uuid7>::try_from(d).unwrap_err();
    assert!(err.is_media());
  }
}
