//! Wire ⇄ domain conversions for `WatchedLocation`.
//!
//! ## Field correspondence
//!
//! The wire layer was generated against the legacy `findit-proto` schema
//! and does **not** carry the freshly-locked domain field set. The
//! locked `WatchedLocation` is **volume-scoped**: it carries a `volume`
//! UUID identity, not a folder path. Bridged one-for-one:
//!
//! | wire field    | domain field       | notes                                     |
//! | ------------- | ------------------ | ----------------------------------------- |
//! | `id`          | `id`               | `Id`→`Uuid7` (validating)                 |
//! | `location`    | `volume`           | oneof `Local`; only its `volume` UUID is taken — the path `components` are wire-only |
//! | `created_at`  | `added_at`         | `i64` ms-since-epoch ↔ `jiff::Timestamp`  |
//!
//! ## Wire-only fields (no domain counterpart — preserved or defaulted)
//!
//! - `name: String`               — NOTE(buffa-bridge): not modelled in the
//!   locked `schema/watched_location.md` r5; the wire field is dropped on
//!   wire→domain and emitted as `""` on domain→wire.
//! - `status: u32`                — legacy `LocationStatus` bitflags
//!   (ACTIVE / TOMBSTONED). The locked domain expresses these
//!   semantically (`is_enabled`, `last_error` for tombstone-with-reason)
//!   but doesn't preserve the wire bit-encoding.
//! - `deleted_at: Option<i64>`    — legacy tombstone timestamp.
//! - `total_files / indexed_files / total_videos / indexed_videos /
//!   total_scenes / total_audios / indexed_audios / total_failed_files /
//!   failed_videos / failed_audios: u64` — legacy scan/index rollup
//!   counters. The locked content-addressed model removed these from
//!   `WatchedLocation` ("what exists" lives in `Media`, by hash, not
//!   here — see `schema/watched_location.md` r5).
//!
//! All wire-only fields default to `0` / empty on the domain→wire path.
//!
//! - `location.components: Vec<String>` — the legacy folder path. The
//!   locked `WatchedLocation` is volume-scoped (it carries only the
//!   `volume` UUID, not a path), so the wire `Local`'s `components` are
//!   dropped on wire→domain and emitted empty on domain→wire.
//!
//! ## Domain-only fields (no wire counterpart — dropped or synthesized)
//!
//! - `recursive: bool`              — NOTE(buffa-bridge): defaults to
//!   `false` on wire→domain (the legacy wire had no recursion flag —
//!   the legacy monitor was recursive by default but the domain models
//!   it explicitly).
//! - `enabled: bool`                — defaults to `false` (matches the
//!   monitor-starts-paused convention).
//! - `is_ejectable: bool`           — defaults to `false`.
//! - `last_reconciled_at / last_reconcile_status / last_error` —
//!   all default to `None` on wire→domain (the legacy wire had no
//!   reconcile-state fields).

use jiff::Timestamp as JiffTimestamp;

use crate::{
  buffa::error::BuffaError,
  domain::{Uuid7, WatchedLocation},
  generated::media::v1 as wire,
};
// Under `feature = "alloc"` (no std), `String` / `ToOwned` / `ToString`
// aren't in the prelude — pull them in via the `extern crate alloc as std`
// alias declared in `lib.rs`. Under `feature = "std"` these come from the
// std prelude automatically; the cfg keeps the import a no-op there.
#[cfg(all(not(feature = "std"), feature = "alloc"))]
#[allow(unused_imports)]
use std::{
  borrow::ToOwned,
  string::{String, ToString},
};

impl TryFrom<&wire::WatchedLocation> for WatchedLocation<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::WatchedLocation) -> Result<Self, Self::Error> {
    let id_wire = w
      .id
      .as_option()
      .ok_or(BuffaError::MissingRequiredField("WatchedLocation.id"))?;
    let id = Uuid7::try_from(id_wire)?;

    // `Location` is a oneof with `Local` as the only arm. The locked
    // `WatchedLocation` is volume-scoped, so only the `Local.volume`
    // UUID is consumed — the path `components` are wire-only and
    // dropped here.
    let loc_wire = w
      .location
      .as_option()
      .ok_or(BuffaError::MissingRequiredField("WatchedLocation.location"))?;
    let local = match &loc_wire.kind {
      Some(wire::location::Kind::Local(local)) => local.as_ref(),
      None => return Err(BuffaError::MissingRequiredField("WatchedLocation.location")),
      #[allow(unreachable_patterns)]
      Some(_) => return Err(BuffaError::UnsupportedLocationKind),
    };
    let vol_wire = local
      .volume
      .as_option()
      .ok_or(BuffaError::MissingLocationVolume)?;
    let volume = Uuid7::try_from(vol_wire)?;

    let added_at = ms_to_jiff(w.created_at)?;
    // wire-only fields ignored; domain-only fields keep their
    // `try_new` defaults (recursive=false, enabled=false,
    // is_ejectable=false, last_*=None).
    WatchedLocation::try_new(id, volume, added_at).map_err(WatchedLocationConvert::from_err)
  }
}

impl From<&WatchedLocation<Uuid7>> for wire::WatchedLocation {
  fn from(d: &WatchedLocation<Uuid7>) -> Self {
    let id = wire::Id::from(d.id_ref());
    // The locked `WatchedLocation` carries only the `volume` UUID; the
    // wire `Local` additionally has a path `components` list with no
    // domain counterpart — emitted empty.
    let location = wire::Location {
      kind: Some(wire::location::Kind::Local(
        ::buffa::alloc::boxed::Box::new(wire::Local {
          volume: ::buffa::MessageField::some(wire::Id::from(d.volume_ref())),
          components: std::vec::Vec::new(),
          __buffa_unknown_fields: Default::default(),
        }),
      )),
      __buffa_unknown_fields: Default::default(),
    };
    let created_at = jiff_to_ms(d.added_at_ref());
    wire::WatchedLocation {
      id: ::buffa::MessageField::some(id),
      location: ::buffa::MessageField::some(location),
      // NOTE(buffa-bridge): wire-only fields default to "absent"
      // (proto3 zero) — the locked domain doesn't model them.
      name: String::new(),
      status: 0,
      created_at,
      deleted_at: None,
      total_files: 0,
      indexed_files: 0,
      total_videos: 0,
      indexed_videos: 0,
      total_scenes: 0,
      total_audios: 0,
      indexed_audios: 0,
      total_failed_files: 0,
      failed_videos: 0,
      failed_audios: 0,
      __buffa_unknown_fields: Default::default(),
    }
  }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Decode wire ms-since-epoch into `jiff::Timestamp`. Surfaces
/// `TimestampOutOfRange` for the (unlikely) wire values outside jiff's
/// supported range.
fn ms_to_jiff(ms: i64) -> Result<JiffTimestamp, BuffaError> {
  JiffTimestamp::from_millisecond(ms).map_err(|_| BuffaError::TimestampOutOfRange(ms))
}

/// Encode `jiff::Timestamp` as wire ms-since-epoch (always lossless at
/// ms resolution — jiff is sub-second-aware but the wire field is ms).
fn jiff_to_ms(t: &JiffTimestamp) -> i64 {
  t.as_millisecond()
}

/// Wrap the domain's `WatchedLocationError` into [`BuffaError`]. We use
/// a small helper module rather than a `From` impl on the domain error
/// (would need to live in domain code) — the bridge owns this mapping.
mod watched_location_convert_impl {
  use crate::{
    buffa::error::BuffaError, domain::aggregates::watched_location::WatchedLocationError,
  };

  pub(super) struct WatchedLocationConvert;

  impl WatchedLocationConvert {
    pub(super) fn from_err(e: WatchedLocationError) -> BuffaError {
      match e {
        // Both invariants are a nil-UUID rejection — `id` and `volume`
        // are the only two identity fields on the locked aggregate.
        // Every `WatchedLocationError` variant is a nil-UUID rejection
        // — `id` and `volume` are the locked aggregate's only two
        // identity fields.
        WatchedLocationError::NilId | WatchedLocationError::NilVolume => {
          BuffaError::IdInvalid(crate::domain::primitives::Uuid7Error::Nil)
        }
      }
    }
  }
}
use watched_location_convert_impl::WatchedLocationConvert;

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  fn build_domain() -> WatchedLocation<Uuid7> {
    WatchedLocation::try_new(
      Uuid7::new(),
      Uuid7::new(),
      JiffTimestamp::from_millisecond(1_700_000_000_000).unwrap(),
    )
    .unwrap()
  }

  #[test]
  fn domain_to_wire_to_domain_preserves_modelled_fields() {
    let d = build_domain();
    let w: wire::WatchedLocation = (&d).into();
    let d2 = WatchedLocation::try_from(&w).expect("roundtrip");
    // The fields the bridge actually preserves:
    assert_eq!(d.id_ref(), d2.id_ref());
    assert_eq!(d.volume_ref(), d2.volume_ref());
    assert_eq!(d.added_at_ref(), d2.added_at_ref());
    // Domain-only fields stay at try_new defaults — wire dropped them.
    assert!(!d2.is_recursive());
    assert!(!d2.is_enabled());
    assert!(!d2.is_ejectable());
    assert!(d2.last_reconciled_at_ref().is_none());
    assert!(d2.last_reconcile_status_ref().is_none());
    assert!(d2.last_error_ref().is_none());
  }

  #[test]
  fn wire_only_fields_emit_as_proto3_default() {
    let d = build_domain();
    let w: wire::WatchedLocation = (&d).into();
    assert!(w.name.is_empty());
    assert_eq!(w.status, 0);
    assert!(w.deleted_at.is_none());
    assert_eq!(w.total_files, 0);
    assert_eq!(w.indexed_files, 0);
    assert_eq!(w.total_videos, 0);
    assert_eq!(w.indexed_audios, 0);
    assert_eq!(w.failed_videos, 0);
  }

  /// Build a wire `Location` (`Local` arm) carrying just a volume UUID.
  fn wire_local(volume: &Uuid7) -> wire::Location {
    wire::Location {
      kind: Some(wire::location::Kind::Local(
        ::buffa::alloc::boxed::Box::new(wire::Local {
          volume: ::buffa::MessageField::some(wire::Id::from(volume)),
          components: std::vec::Vec::new(),
          __buffa_unknown_fields: Default::default(),
        }),
      )),
      __buffa_unknown_fields: Default::default(),
    }
  }

  #[test]
  fn wire_to_domain_missing_id_errors() {
    let vol = Uuid7::new();
    let w = wire::WatchedLocation {
      id: ::buffa::MessageField::none(),
      location: ::buffa::MessageField::some(wire_local(&vol)),
      created_at: 0,
      ..Default::default()
    };
    let err = WatchedLocation::try_from(&w).unwrap_err();
    assert!(err.is_missing_required_field());
  }

  #[test]
  fn wire_to_domain_missing_location_errors() {
    let id = Uuid7::new();
    let w = wire::WatchedLocation {
      id: ::buffa::MessageField::some(wire::Id::from(&id)),
      location: ::buffa::MessageField::none(),
      created_at: 0,
      ..Default::default()
    };
    let err = WatchedLocation::try_from(&w).unwrap_err();
    assert!(err.is_missing_required_field());
  }
}
