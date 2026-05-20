//! Wire ⇄ domain conversions for `Media`.
//!
//! ## Field correspondence
//!
//! Wire `media.v1::Media` wraps a nested `MediaMeta` (id / checksum /
//! name / size / created_at) and adds flat scalars (kind, EXIF capture,
//! etc.). The domain `Media` flattens these into one struct. Bridged
//! one-for-one:
//!
//! | wire field                | domain field      | notes                                |
//! | ------------------------- | ----------------- | ------------------------------------ |
//! | `meta.id` (Bytes, 16)     | `id` (Uuid7)      | validating                           |
//! | `meta.checksum` (Bytes, 32) | `checksum`       | 32 bytes; zero sentinel allowed       |
//! | `meta.name`               | `name`            | `""` = absent (SmolStr)              |
//! | `meta.size`               | `size`            | u64                                  |
//! | `meta.created_at`         | `created_at`      | i64 ms-since-epoch ↔ jiff::Timestamp  |
//! | `kind: EnumValue<DbMediaKind>` | `kind`       | UNSPECIFIED rejected (closed enum)   |
//! | `video_id: Option<Bytes>` | `video: Option<Uuid7>` | validating                      |
//! | `audio_id`                | `audio`           | validating                           |
//! | `subtitle_id`             | `subtitle`        | validating                           |
//! | `index_error`             | `probe_error`     | wire `index_error` is the closest analogue |
//! | `capture_date: i64`       | `capture_date: Option<JiffTimestamp>` | `0` means absent       |
//! | `device_make`/`device_model` | `device: Option<MediaDevice>` | both `""` ⇒ None       |
//!
//! ## Wire-only (dropped on wire→domain; emitted as proto3-default on
//! domain→wire)
//!
//! - `meta.time: MessageField<TrackTime>` — the legacy schema's
//!   per-Media timestamp anchor. The locked domain stores **media-time
//!   duration** instead (`duration: Option<mediatime::Timestamp>`);
//!   these are not equivalent so the bridge does **not** translate
//!   between them.
//! - `index_status: u32`, `error_status: u32` — legacy bitflags. The
//!   locked domain expresses errors as `error_flags: MediaErrorFlags`
//!   (a rollup over per-track failures) which is **structurally
//!   different**; the bridge passes neither.
//! - `gps_location: String` — legacy flat string. The locked domain has
//!   structured `gps: Option<MediaGeoLocation>` (lat/lon/altitude). The
//!   bridge does NOT attempt to parse / format this string; it's
//!   dropped on wire→domain and left empty on domain→wire.
//!
//! ## Domain-only (dropped on domain→wire; defaulted on wire→domain)
//!
//! - `format: SmolStr` — locked container-format slug. No wire field.
//! - `duration: Option<mediatime::Timestamp>` — locked overall duration.
//! - `error_flags: MediaErrorFlags` — locked rollup; not derivable from
//!   the wire's `error_status: u32`.

use jiff::Timestamp as JiffTimestamp;
use smol_str::SmolStr;

use crate::{
  buffa::error::BuffaError,
  domain::{
    aggregates::media::{MediaDevice, MediaError},
    ErrorInfo, FileChecksum, Media, MediaKind, Uuid7,
  },
  generated::media::v1 as wire,
};

impl TryFrom<&wire::Media> for Media<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::Media) -> Result<Self, Self::Error> {
    let meta = w
      .meta
      .as_option()
      .ok_or(BuffaError::MissingRequiredField("Media.meta"))?;

    // --- mandatory id / checksum / kind ---
    let id_arr: [u8; 16] = meta
      .id
      .as_ref()
      .try_into()
      .map_err(|_| BuffaError::IdWrongLength(meta.id.len()))?;
    let id = Uuid7::try_from_bytes(id_arr).map_err(BuffaError::from)?;

    let checksum_arr: [u8; 32] = meta
      .checksum
      .as_ref()
      .try_into()
      .map_err(|_| BuffaError::ChecksumWrongLength(meta.checksum.len()))?;
    let checksum = FileChecksum::from_bytes(checksum_arr);

    let kind = MediaKind::try_from(&w.kind)?;

    let created_at = JiffTimestamp::from_millisecond(meta.created_at)
      .map_err(|_| BuffaError::TimestampOutOfRange(meta.created_at))?;

    let name = meta.name.as_str();
    // NOTE(buffa-bridge): no wire `format` field; domain `format`
    // (container slug) starts empty and would be set via
    // `with_format`-style API — currently the domain has no
    // `with_format` builder (locked schema doesn't expose one), so
    // construction with `format = ""` matches the wire silence.
    let mut d = Media::try_new(
      id,
      checksum,
      SmolStr::from(name),
      SmolStr::default(), // format unmodelled in wire
      meta.size,
      created_at,
      kind,
    )
    .map_err(media_err_to_buffa)?;

    // --- optional facet FKs ---
    if let Some(b) = &w.video_id {
      d = d.with_video(Some(opt_id(b)?));
    }
    if let Some(b) = &w.audio_id {
      d = d.with_audio(Some(opt_id(b)?));
    }
    if let Some(b) = &w.subtitle_id {
      d = d.with_subtitle(Some(opt_id(b)?));
    }

    // --- probe_error ---
    if let Some(ei) = w.index_error.as_option() {
      // NOTE(buffa-bridge): wire `index_error` is a generic
      // index-pipeline failure; the domain `probe_error` is
      // narrower (file unprobeable). We forward the wire value
      // verbatim — the caller is responsible for interpreting the
      // stage code.
      d = d.with_probe_error(Some(ErrorInfo::from(ei)));
    }

    // --- capture_date: wire 0 ⇒ absent ---
    if w.capture_date != 0 {
      let cap = JiffTimestamp::from_millisecond(w.capture_date)
        .map_err(|_| BuffaError::TimestampOutOfRange(w.capture_date))?;
      d = d.with_capture_date(Some(cap));
    }

    // --- device: empty ⇒ None ---
    let make = w.device_make.as_str();
    let model = w.device_model.as_str();
    if !make.is_empty() || !model.is_empty() {
      d = d.with_device(Some(MediaDevice::from_parts(make, model)));
    }

    // NOTE(buffa-bridge): wire `gps_location: String` (legacy flat
    // string) is **dropped**. The domain `gps: Option<MediaGeoLocation>`
    // is structured (lat/lon/altitude); we don't risk a string parse.

    Ok(d)
  }
}

impl From<&Media<Uuid7>> for wire::Media {
  fn from(d: &Media<Uuid7>) -> Self {
    let meta = wire::MediaMeta {
      id: ::buffa::bytes::Bytes::copy_from_slice(d.id().as_bytes()),
      checksum: ::buffa::bytes::Bytes::copy_from_slice(d.checksum().as_bytes()),
      name: d.name().to_owned(),
      size: d.size(),
      // NOTE(buffa-bridge): wire `meta.time` (MessageField<TrackTime>)
      // is unmodelled by the locked domain — emitted unset.
      time: ::buffa::MessageField::none(),
      created_at: d.created_at().as_millisecond(),
      __buffa_unknown_fields: Default::default(),
    };

    let video_id = d
      .video()
      .map(|id| ::buffa::bytes::Bytes::copy_from_slice(id.as_bytes()));
    let audio_id = d
      .audio()
      .map(|id| ::buffa::bytes::Bytes::copy_from_slice(id.as_bytes()));
    let subtitle_id = d
      .subtitle()
      .map(|id| ::buffa::bytes::Bytes::copy_from_slice(id.as_bytes()));

    let index_error = match d.probe_error() {
      Some(ei) => ::buffa::MessageField::some(wire::ErrorInfo::from(ei)),
      None => ::buffa::MessageField::none(),
    };

    let (device_make, device_model) = match d.device() {
      Some(dev) => (dev.make().to_owned(), dev.model().to_owned()),
      None => (String::new(), String::new()),
    };

    wire::Media {
      meta: ::buffa::MessageField::some(meta),
      kind: ::buffa::EnumValue::from(d.kind()),
      // NOTE(buffa-bridge): see module doc — these wire fields are
      // not derivable from the locked domain. Emit proto3 defaults.
      index_status: 0,
      index_error,
      video_id,
      audio_id,
      subtitle_id,
      error_status: 0,
      capture_date: d.capture_date().map(|t| t.as_millisecond()).unwrap_or(0),
      device_make,
      device_model,
      gps_location: String::new(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

// --- helpers --------------------------------------------------------------

fn opt_id(b: &::buffa::bytes::Bytes) -> Result<Uuid7, BuffaError> {
  let arr: [u8; 16] = b
    .as_ref()
    .try_into()
    .map_err(|_| BuffaError::IdWrongLength(b.len()))?;
  Uuid7::try_from_bytes(arr).map_err(BuffaError::from)
}

fn media_err_to_buffa(e: MediaError) -> BuffaError {
  match e {
    MediaError::NilId => BuffaError::IdInvalid(crate::domain::primitives::Uuid7Error::Nil),
    // The locked domain's `MediaError::ZeroChecksum` rejects the
    // all-zero sentinel because a probed file always has its content
    // hash. A wire value with zero bytes is therefore a programming
    // error (not a wrong-length condition) — surface it as
    // ChecksumWrongLength(32) to keep the variant set small.
    MediaError::ZeroChecksum => BuffaError::ChecksumWrongLength(32),
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  fn build_domain() -> Media<Uuid7> {
    let id = Uuid7::new();
    let cs = FileChecksum::from_bytes([7u8; 32]);
    Media::try_new(
      id,
      cs,
      "clip.mp4",
      "",
      1_234_567,
      JiffTimestamp::from_millisecond(1_700_000_000_000).unwrap(),
      MediaKind::Video,
    )
    .unwrap()
  }

  #[test]
  fn modelled_fields_round_trip() {
    let d = build_domain()
      .with_video(Some(Uuid7::new()))
      .with_audio(Some(Uuid7::new()))
      .with_capture_date(Some(
        JiffTimestamp::from_millisecond(1_500_000_000_000).unwrap(),
      ))
      .with_device(Some(MediaDevice::from_parts("Sony", "A7M3")))
      .with_probe_error(Some(ErrorInfo::new(
        crate::domain::ErrorCode::ProbeCorrupt,
        "bad container",
      )));
    let w: wire::Media = (&d).into();
    let d2: Media<Uuid7> = Media::try_from(&w).expect("roundtrip");
    assert_eq!(d.id(), d2.id());
    assert_eq!(d.checksum(), d2.checksum());
    assert_eq!(d.name(), d2.name());
    assert_eq!(d.size(), d2.size());
    assert_eq!(d.created_at(), d2.created_at());
    assert_eq!(d.kind(), d2.kind());
    assert_eq!(d.video(), d2.video());
    assert_eq!(d.audio(), d2.audio());
    assert_eq!(d.subtitle(), d2.subtitle());
    assert_eq!(d.capture_date(), d2.capture_date());
    assert_eq!(d.device(), d2.device());
    assert_eq!(d.probe_error(), d2.probe_error());
  }

  #[test]
  fn wire_missing_meta_errors() {
    let w = wire::Media {
      meta: ::buffa::MessageField::none(),
      kind: MediaKind::Video.into(),
      ..Default::default()
    };
    let err = Media::try_from(&w).unwrap_err();
    assert!(err.is_missing_required_field());
  }

  #[test]
  fn wire_invalid_id_errors() {
    let meta = wire::MediaMeta {
      id: ::buffa::bytes::Bytes::copy_from_slice(&[0u8; 8]),
      checksum: ::buffa::bytes::Bytes::copy_from_slice(&[1u8; 32]),
      created_at: 0,
      ..Default::default()
    };
    let w = wire::Media {
      meta: ::buffa::MessageField::some(meta),
      kind: MediaKind::Video.into(),
      ..Default::default()
    };
    let err = Media::try_from(&w).unwrap_err();
    assert!(err.is_id_wrong_length());
  }

  #[test]
  fn wire_unspecified_kind_errors() {
    let d = build_domain();
    let mut w = wire::Media::from(&d);
    w.kind = ::buffa::EnumValue::Known(wire::DbMediaKind::MEDIA_KIND_UNSPECIFIED);
    let err = Media::try_from(&w).unwrap_err();
    assert!(err.is_unknown_enum_value());
  }

  #[test]
  fn domain_to_wire_emits_proto3_defaults_for_unmodelled_fields() {
    let d = build_domain();
    let w: wire::Media = (&d).into();
    // index_status / error_status / gps_location are zero/empty
    // since the domain doesn't carry them.
    assert_eq!(w.index_status, 0);
    assert_eq!(w.error_status, 0);
    assert!(w.gps_location.is_empty());
    // The nested `meta.time` is also unset (no domain counterpart).
    let meta = w.meta.as_option().unwrap();
    assert!(meta.time.is_unset());
  }
}
