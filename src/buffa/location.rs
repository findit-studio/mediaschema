//! Wire ⇄ domain conversions for the `Location` oneof.
//!
//! Wire `media.v1::Location` is a oneof with the single arm
//! `Local(Box<media.v1::Local>)`. The domain mirror — `domain::Location`
//! — is a `#[non_exhaustive]` enum with the matching `Local(LocalLocation)`
//! arm. The unset oneof (`Location { kind: None }`) maps to no domain
//! value; the bridge therefore goes through `Option<domain::Location>`
//! on the wire→domain side.

use crate::{
  buffa::error::BuffaError,
  domain::{primitives::LocalLocation, Location, Uuid7},
  generated::media::v1 as wire,
};

// ---------------------------------------------------------------------------
// Local (wire) ⇄ LocalLocation<Uuid7>'s parent enum (Location<Uuid7>)
// ---------------------------------------------------------------------------

impl TryFrom<&wire::Local> for Location<Uuid7> {
  type Error = BuffaError;

  /// Decode a wire `Local` into a domain [`Location::Local`]. Goes
  /// through the validating builder
  /// [`Location::try_local_uuid7`] — rejects empty path / nil volume.
  fn try_from(w: &wire::Local) -> Result<Self, Self::Error> {
    // NOTE(buffa-bridge): wire `Local.volume: MessageField<Id>` is
    // optional at the protobuf level. The domain requires a present
    // non-nil v7 volume; an unset wire volume surfaces as
    // `MissingLocationVolume`.
    let wire_vol = w
      .volume
      .as_option()
      .ok_or(BuffaError::MissingLocationVolume)?;
    let volume = Uuid7::try_from(wire_vol)?;
    // Domain validating builder enforces both invariants (nil volume
    // already rejected by Uuid7::try_from above; the empty-path check
    // is the remaining one).
    Location::try_local_uuid7(volume, w.components.iter().cloned()).map_err(BuffaError::from)
  }
}

impl From<&Location<Uuid7>> for wire::Local {
  /// Encode a domain `Location::Local` payload as a wire `Local`.
  /// Domain enforces non-empty path + non-nil volume, so this is
  /// infallible.
  fn from(d: &Location<Uuid7>) -> Self {
    // Domain is `#[non_exhaustive]` but currently only has the `Local`
    // variant. `unwrap_local_ref` is the IsVariant accessor; this
    // panics only if a future non-Local variant is introduced without
    // updating this bridge.
    let local = d.unwrap_local_ref();
    let volume_wire = wire::Id::from(local.volume_ref());
    let components = local
      .components_slice()
      .iter()
      .map(|c| c.as_str().to_owned())
      .collect();
    wire::Local {
      volume: ::buffa::MessageField::some(volume_wire),
      components,
      __buffa_unknown_fields: Default::default(),
    }
  }
}

// ---------------------------------------------------------------------------
// Location (wire oneof) ⇄ Option<Location<Uuid7>>
// ---------------------------------------------------------------------------

impl TryFrom<&wire::Location> for Option<Location<Uuid7>> {
  type Error = BuffaError;

  /// Decode a wire `Location` (a oneof). `kind = None` → `None`;
  /// `Local(_)` → `Some(Location::Local(_))`. A future non-`Local` arm
  /// would surface as `UnsupportedLocationKind`.
  fn try_from(w: &wire::Location) -> Result<Self, Self::Error> {
    match &w.kind {
      None => Ok(None),
      Some(wire::location::Kind::Local(local)) => Ok(Some(Location::try_from(local.as_ref())?)),
      // NOTE(buffa-bridge): the wire enum is `#[non_exhaustive]`-ish
      // via codegen; today only `Local` exists. Pattern-matching
      // exhaustively today; if buffa-codegen adds an arm we'll hear
      // about it at compile time.
      #[allow(unreachable_patterns)]
      Some(_) => Err(BuffaError::UnsupportedLocationKind),
    }
  }
}

impl From<&Option<Location<Uuid7>>> for wire::Location {
  /// Encode an optional domain `Location` as a wire oneof. `None` ⇒
  /// `kind: None`; `Some(Local(_))` ⇒ `kind: Some(Local(…))`.
  fn from(d: &Option<Location<Uuid7>>) -> Self {
    let kind = d.as_ref().map(|loc| {
      wire::location::Kind::Local(::buffa::alloc::boxed::Box::new(wire::Local::from(loc)))
    });
    wire::Location {
      kind,
      __buffa_unknown_fields: Default::default(),
    }
  }
}

// Convenience: a plain `&Location` (always-set) maps directly to a wire
// `Location` with `kind: Some(…)`.
impl From<&Location<Uuid7>> for wire::Location {
  fn from(d: &Location<Uuid7>) -> Self {
    let kind = Some(wire::location::Kind::Local(
      ::buffa::alloc::boxed::Box::new(wire::Local::from(d)),
    ));
    wire::Location {
      kind,
      __buffa_unknown_fields: Default::default(),
    }
  }
}

// Helper alias for the type that domain code holds for the inner local
// payload — re-exported so downstream tests don't have to chase imports.
#[allow(dead_code)]
type _LocalLocationAlias = LocalLocation<Uuid7>;

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  fn fresh() -> (Uuid7, Uuid7) {
    (Uuid7::new(), Uuid7::new())
  }

  #[test]
  fn local_roundtrip() {
    let (_id, vol) = fresh();
    let d = Location::try_local_uuid7(vol, ["Movies", "Holiday"]).unwrap();
    let w: wire::Local = wire::Local::from(&d);
    let d2 = Location::try_from(&w).expect("roundtrip");
    assert_eq!(d, d2);
  }

  #[test]
  fn local_rejects_missing_volume() {
    let w = wire::Local {
      volume: ::buffa::MessageField::none(),
      components: std::vec!["foo".to_owned()],
      __buffa_unknown_fields: Default::default(),
    };
    let err = Location::try_from(&w).unwrap_err();
    assert!(err.is_missing_location_volume());
  }

  #[test]
  fn local_rejects_empty_path() {
    let vol = Uuid7::new();
    let w = wire::Local {
      volume: ::buffa::MessageField::some(wire::Id::from(&vol)),
      components: std::vec::Vec::new(),
      __buffa_unknown_fields: Default::default(),
    };
    let err = Location::try_from(&w).unwrap_err();
    assert!(err.is_location(), "{:?}", err);
  }

  #[test]
  fn location_oneof_some_roundtrip() {
    let (_id, vol) = fresh();
    let d: Option<Location<Uuid7>> = Some(Location::try_local_uuid7(vol, ["a", "b"]).unwrap());
    let w: wire::Location = wire::Location::from(&d);
    let d2: Option<Location<Uuid7>> = Option::<Location<Uuid7>>::try_from(&w).expect("roundtrip");
    assert_eq!(d, d2);
  }

  #[test]
  fn location_oneof_none_roundtrip() {
    let d: Option<Location<Uuid7>> = None;
    let w: wire::Location = wire::Location::from(&d);
    assert!(w.kind.is_none());
    let d2: Option<Location<Uuid7>> = Option::<Location<Uuid7>>::try_from(&w).expect("roundtrip");
    assert!(d2.is_none());
  }

  #[test]
  fn plain_location_to_wire_sets_kind() {
    let vol = Uuid7::new();
    let d = Location::try_local_uuid7(vol, ["x"]).unwrap();
    let w = wire::Location::from(&d);
    assert!(w.kind.is_some());
  }
}
