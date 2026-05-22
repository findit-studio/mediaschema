//! Wire ⇄ domain conversions for `MediaFile`.
//!
//! ## Field correspondence
//!
//! Wire `media.v1::MediaFile` is the per-copy file record introduced
//! alongside the `Media` / `MediaFile` content/copy split. N `MediaFile`s
//! may point at one `Media` (same content, distinct physical copies). The
//! wire shape mirrors the domain 1:1 — including `watch_volume`, so a
//! single message round-trips losslessly without any join.
//!
//! | wire field             | domain field            | notes                                          |
//! | ---------------------- | ----------------------- | ---------------------------------------------- |
//! | `id` (Bytes, 16)       | `id` (Uuid7)            | validating                                     |
//! | `media_id` (Bytes, 16) | `media_id` (Uuid7)      | validating                                     |
//! | `created_at: i64`      | `created_at: Option<JiffTimestamp>` | wire `0` ⇒ `None` (legacy sentinel) |
//! | `location: Location`   | `location: Location<Uuid7>` | oneof must be present + `Local` arm        |
//! | `watched_location_id` (Bytes, 16) | `watched_location_id` (Uuid7) | validating                       |
//! | `watch_volume` (Bytes, 16) | `watch_volume` (Uuid7) | validating; carried on wire so the bridge is 1:1 (no WL join) |
//!
//! ## Reconstruction
//!
//! Decoding uses [`MediaFile::from_parts`] — the infallible storage /
//! wire reconstruction constructor — because the wire bytes were produced
//! by a previously-validated domain instance. Application code building
//! a fresh `MediaFile` must still go through `try_new`.

use jiff::Timestamp as JiffTimestamp;

use crate::{
  buffa::error::BuffaError,
  domain::{Location, MediaFile, Uuid7},
  generated::media::v1 as wire,
};

impl TryFrom<&wire::MediaFile> for MediaFile<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::MediaFile) -> Result<Self, Self::Error> {
    let id = id_from_bytes(&w.id)?;
    let media_id = id_from_bytes(&w.media_id)?;
    let watched_location_id = id_from_bytes(&w.watched_location_id)?;
    let watch_volume = id_from_bytes(&w.watch_volume)?;

    // The wire `location` oneof must be present + `Local` for the
    // domain `MediaFile` (every copy has a structured location).
    let loc_wire = w
      .location
      .as_option()
      .ok_or(BuffaError::MissingRequiredField("MediaFile.location"))?;
    let location: Location<Uuid7> = match &loc_wire.kind {
      Some(wire::location::Kind::Local(local)) => Location::try_from(local.as_ref())?,
      None => return Err(BuffaError::MissingRequiredField("MediaFile.location")),
      #[allow(unreachable_patterns)]
      Some(_) => return Err(BuffaError::UnsupportedLocationKind),
    };

    // Legacy `0` ms sentinel collapses to `None` — this is the
    // wire-decode adapter's job, per the domain docstring on
    // `MediaFile::created_at`.
    let created_at = if w.created_at == 0 {
      None
    } else {
      Some(
        JiffTimestamp::from_millisecond(w.created_at)
          .map_err(|_| BuffaError::TimestampOutOfRange(w.created_at))?,
      )
    };

    Ok(MediaFile::from_parts(
      id,
      media_id,
      created_at,
      location,
      watched_location_id,
      watch_volume,
    ))
  }
}

impl From<&MediaFile<Uuid7>> for wire::MediaFile {
  fn from(d: &MediaFile<Uuid7>) -> Self {
    wire::MediaFile {
      id: ::buffa::bytes::Bytes::copy_from_slice(d.id_ref().as_bytes()),
      media_id: ::buffa::bytes::Bytes::copy_from_slice(d.media_id_ref().as_bytes()),
      // `None` collapses to the legacy `0` ms sentinel (the symmetric
      // encode of the decode rule above).
      created_at: d.created_at_ref().map(|t| t.as_millisecond()).unwrap_or(0),
      location: ::buffa::MessageField::some(wire::Location::from(d.location_ref())),
      watched_location_id: ::buffa::bytes::Bytes::copy_from_slice(
        d.watched_location_id_ref().as_bytes(),
      ),
      watch_volume: ::buffa::bytes::Bytes::copy_from_slice(d.watch_volume_ref().as_bytes()),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Decode a 16-byte wire `Bytes` into the validating domain `Uuid7`.
/// Mirrors the inline-bytes id convention used elsewhere in this module.
fn id_from_bytes(b: &::buffa::bytes::Bytes) -> Result<Uuid7, BuffaError> {
  let arr: [u8; 16] = b
    .as_ref()
    .try_into()
    .map_err(|_| BuffaError::IdWrongLength(b.len()))?;
  Uuid7::try_from_bytes(arr).map_err(BuffaError::from)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use crate::domain::WatchedLocation;

  fn ts() -> JiffTimestamp {
    JiffTimestamp::from_millisecond(1_700_000_000_000).expect("valid timestamp")
  }

  fn watch(volume: Uuid7) -> WatchedLocation<Uuid7> {
    WatchedLocation::try_new(Uuid7::new(), volume, JiffTimestamp::default()).expect("valid watch")
  }

  fn loc(volume: Uuid7, name: &str) -> Location<Uuid7> {
    Location::try_local_uuid7(volume, ["Movies", name]).expect("valid location")
  }

  fn build_domain(media_id: Uuid7, vol: Uuid7, name: &str) -> MediaFile<Uuid7> {
    let wl = watch(vol);
    MediaFile::try_new(Uuid7::new(), media_id, Some(ts()), loc(vol, name), &wl)
      .expect("valid construction")
  }

  #[test]
  fn modelled_fields_round_trip() {
    let d = build_domain(Uuid7::new(), Uuid7::new(), "clip.mp4");
    let w: wire::MediaFile = (&d).into();
    let d2: MediaFile<Uuid7> = MediaFile::try_from(&w).expect("roundtrip");
    assert_eq!(d, d2);
  }

  #[test]
  fn created_at_none_roundtrips_via_zero_sentinel() {
    // Domain `None` ⇒ wire `0` ms (legacy sentinel) ⇒ domain `None`.
    let vol = Uuid7::new();
    let wl = watch(vol);
    let d = MediaFile::try_new(Uuid7::new(), Uuid7::new(), None, loc(vol, "x.mp4"), &wl).unwrap();
    let w: wire::MediaFile = (&d).into();
    assert_eq!(w.created_at, 0);
    let d2 = MediaFile::try_from(&w).expect("roundtrip");
    assert_eq!(d, d2);
    assert!(d2.created_at_ref().is_none());
  }

  #[test]
  fn two_media_files_share_one_media_round_trip() {
    // Codex-recommended N:1 fixture: two distinct copies pointing at the
    // SAME `media_id` (different watches / paths) both encode and
    // decode back to the originals. Confirms the wire shape carries
    // enough state to faithfully represent the content/copy split.
    let media_id = Uuid7::new();
    let vol_a = Uuid7::new();
    let vol_b = Uuid7::new();
    let f_a = build_domain(media_id, vol_a, "copy-a.mp4");
    let f_b = build_domain(media_id, vol_b, "copy-b.mp4");
    assert_eq!(f_a.media_id_ref(), f_b.media_id_ref());
    assert_ne!(f_a.id_ref(), f_b.id_ref());

    let w_a: wire::MediaFile = (&f_a).into();
    let w_b: wire::MediaFile = (&f_b).into();
    let d_a: MediaFile<Uuid7> = MediaFile::try_from(&w_a).expect("roundtrip a");
    let d_b: MediaFile<Uuid7> = MediaFile::try_from(&w_b).expect("roundtrip b");
    assert_eq!(d_a, f_a);
    assert_eq!(d_b, f_b);
    // Both decoded copies still share the same content row identity.
    assert_eq!(d_a.media_id_ref(), d_b.media_id_ref());
  }

  #[test]
  fn wire_missing_location_errors() {
    let d = build_domain(Uuid7::new(), Uuid7::new(), "clip.mp4");
    let mut w = wire::MediaFile::from(&d);
    w.location = ::buffa::MessageField::none();
    let err = MediaFile::try_from(&w).unwrap_err();
    assert!(err.is_missing_required_field());
  }

  #[test]
  fn wire_invalid_id_errors() {
    let d = build_domain(Uuid7::new(), Uuid7::new(), "clip.mp4");
    let mut w = wire::MediaFile::from(&d);
    w.id = ::buffa::bytes::Bytes::copy_from_slice(&[0u8; 8]);
    let err = MediaFile::try_from(&w).unwrap_err();
    assert!(err.is_id_wrong_length());
  }

  #[test]
  fn wire_out_of_range_created_at_errors() {
    let d = build_domain(Uuid7::new(), Uuid7::new(), "clip.mp4");
    let mut w = wire::MediaFile::from(&d);
    w.created_at = i64::MAX;
    let err = MediaFile::try_from(&w).unwrap_err();
    assert!(err.is_timestamp_out_of_range());
  }
}
