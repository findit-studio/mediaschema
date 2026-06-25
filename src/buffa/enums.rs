//! Wire ⇄ domain conversions for the enum vocabulary.
//!
//! Covered:
//! - `media.v1::DbMediaKind`             ⇄ `domain::MediaKind`
//! - `buffa::EnumValue<DbMediaKind>`     ⇄ `domain::MediaKind`
//!
//! `domain::MediaKind` is **closed** (only `Video` / `Audio`). The wire
//! `DbMediaKind` adds an `UNSPECIFIED` zero variant for proto3 default;
//! converting an `UNSPECIFIED` (or `EnumValue::Unknown(_)`) wire value
//! to the closed domain enum surfaces a [`BuffaError::UnknownEnumValue`].
//!
//! Other domain enums (`SceneDetector`, `KeyframeExtractor`,
//! `AudioContentKind`, `ScanStatus`, the three `*IndexStage`s,
//! `SubtitleKind`) currently have **no aligned wire counterpart** under
//! `media.v1` — the wire layer uses different vocabulary
//! (`SubtitleTrackRole`, `AudioContainerFormat`, …). They're left
//! unbridged; tracked as a follow-up if/when the wire layer is
//! regenerated against the locked schema.

use crate::{
  buffa::error::BuffaError,
  domain::{Backend, MediaKind},
  generated::media::v1 as wire,
};

// ---------------------------------------------------------------------------
// DbMediaKind ⇄ MediaKind
// ---------------------------------------------------------------------------

impl TryFrom<wire::DbMediaKind> for MediaKind {
  type Error = BuffaError;

  fn try_from(w: wire::DbMediaKind) -> Result<Self, Self::Error> {
    match w {
      wire::DbMediaKind::MEDIA_KIND_VIDEO => Ok(MediaKind::Video),
      wire::DbMediaKind::MEDIA_KIND_AUDIO => Ok(MediaKind::Audio),
      // NOTE(buffa-bridge): proto3 `UNSPECIFIED` zero is the
      // pre-classification state. The domain `MediaKind` is closed
      // (set at probe — no `Unknown` arm), so an `UNSPECIFIED` wire
      // value cannot be expressed; surface it as an error so callers
      // can decide (treat as "not yet probed" / skip / etc.).
      wire::DbMediaKind::MEDIA_KIND_UNSPECIFIED => Err(BuffaError::UnknownEnumValue(0)),
    }
  }
}

impl From<MediaKind> for wire::DbMediaKind {
  /// Closed-enum encode — never `UNSPECIFIED` on the way out.
  fn from(d: MediaKind) -> Self {
    match d {
      MediaKind::Video => wire::DbMediaKind::MEDIA_KIND_VIDEO,
      MediaKind::Audio => wire::DbMediaKind::MEDIA_KIND_AUDIO,
    }
  }
}

// ---------------------------------------------------------------------------
// EnumValue<DbMediaKind> ⇄ MediaKind
// ---------------------------------------------------------------------------
//
// The wire side carries `EnumValue<DbMediaKind>` (the buffa "open enum"
// container — accepts unknown `i32`s for forward compatibility). The
// domain enum is closed, so unknown wire values surface as
// `BuffaError::UnknownEnumValue(v)`.

impl TryFrom<&::buffa::EnumValue<wire::DbMediaKind>> for MediaKind {
  type Error = BuffaError;

  fn try_from(w: &::buffa::EnumValue<wire::DbMediaKind>) -> Result<Self, Self::Error> {
    match w {
      ::buffa::EnumValue::Known(k) => MediaKind::try_from(*k),
      ::buffa::EnumValue::Unknown(v) => Err(BuffaError::UnknownEnumValue(*v)),
    }
  }
}

impl From<MediaKind> for ::buffa::EnumValue<wire::DbMediaKind> {
  fn from(d: MediaKind) -> Self {
    ::buffa::EnumValue::Known(wire::DbMediaKind::from(d))
  }
}

// ---------------------------------------------------------------------------
// Backend ⇄ wire::Backend / EnumValue<wire::Backend>
// ---------------------------------------------------------------------------
//
// `EnumValue<wire::Backend>` is the ONLY infallible wire projection of a
// `Backend`, and it's what `Provenance.backend` actually carries. There is
// deliberately NO `From<Backend> for wire::Backend`: the bare generated
// `wire::Backend` is a CLOSED enum with no `Unknown` arm, so it cannot
// represent a forward-compatible `Backend::Unknown(i)` — an infallible
// `Backend -> wire::Backend` could only do so by silently collapsing that
// code to `BACKEND_UNSPECIFIED`, reintroducing the version-skew data-loss
// class on the wire. The only `Backend -> bare wire` path is the explicitly
// fallible [`Backend::to_known_wire`], which returns `None` for `Unknown`.
//
// The reverse `From<wire::Backend> for Backend` is lossless by construction:
// a bare wire enum can only ever carry a value the schema names, so it always
// maps to a known domain variant — it can never produce `Backend::Unknown`.
// The Unknown-bearing decode is `From<&EnumValue<wire::Backend>> for Backend`.

impl Backend {
  /// Project to the bare generated `wire::Backend`, or `None` when this is a
  /// [`Backend::Unknown`] code the closed wire enum cannot name.
  ///
  /// This is the **only** `Backend -> bare wire::Backend` conversion, and it
  /// is deliberately fallible: an infallible one would have to collapse an
  /// unknown code to `BACKEND_UNSPECIFIED`, silently dropping a
  /// forward-compatible backend (the version-skew data-loss class). For an
  /// infallible, lossless wire projection use
  /// `EnumValue<wire::Backend>` (`Provenance.backend`'s type) instead.
  ///
  /// `Some` for every schema-named variant (the pinned integers `0..=14`);
  /// `None` only for `Backend::Unknown(_)`.
  #[inline]
  #[must_use]
  pub fn to_known_wire(self) -> Option<wire::Backend> {
    <wire::Backend as ::buffa::Enumeration>::from_i32(self.to_i32())
  }
}

impl From<wire::Backend> for Backend {
  /// Lossless by construction: the bare closed `wire::Backend` can only hold a
  /// value the schema names, so it always decodes to a known domain variant
  /// (never `Backend::Unknown`). The Unknown-bearing decode is the
  /// `From<&EnumValue<wire::Backend>>` impl below.
  fn from(w: wire::Backend) -> Self {
    Backend::from_i32(<wire::Backend as ::buffa::Enumeration>::to_i32(&w))
  }
}

impl From<Backend> for ::buffa::EnumValue<wire::Backend> {
  /// Lossless. A known `Backend` rides as `EnumValue::Known`; a
  /// `Backend::Unknown(i)` (a forward-compatible code) rides as
  /// `EnumValue::Unknown(i)` — buffa's open-enum wrapper carries the raw
  /// integer verbatim on the wire, so an unrecognized backend survives a
  /// domain→wire→domain round-trip instead of being flattened to
  /// `Unspecified`.
  fn from(d: Backend) -> Self {
    match d.to_known_wire() {
      Some(known) => ::buffa::EnumValue::Known(known),
      // Unreachable for any non-`Unknown` variant (all are schema-named);
      // `to_known_wire` returns `None` only for `Backend::Unknown(i)`, which
      // the open-enum wrapper carries verbatim — never lossily.
      None => ::buffa::EnumValue::Unknown(d.to_i32()),
    }
  }
}

impl From<&::buffa::EnumValue<wire::Backend>> for Backend {
  /// Lossless inverse. `Known` decodes verbatim; an `Unknown(i)` wire integer
  /// routes through [`Backend::from_i32`], which preserves it as
  /// `Backend::Unknown(i)` (rather than collapsing every unknown to
  /// `Unspecified`). `from_i32` also re-promotes a known number that somehow
  /// arrived in the `Unknown` arm back to its named variant.
  fn from(w: &::buffa::EnumValue<wire::Backend>) -> Self {
    match w {
      ::buffa::EnumValue::Known(k) => Backend::from(*k),
      ::buffa::EnumValue::Unknown(i) => Backend::from_i32(*i),
    }
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  #[test]
  fn db_media_kind_roundtrip_video() {
    let d = MediaKind::Video;
    let w: wire::DbMediaKind = d.into();
    assert_eq!(w, wire::DbMediaKind::MEDIA_KIND_VIDEO);
    let d2: MediaKind = MediaKind::try_from(w).unwrap();
    assert_eq!(d, d2);
  }

  #[test]
  fn db_media_kind_roundtrip_audio() {
    let d = MediaKind::Audio;
    let w: wire::DbMediaKind = d.into();
    assert_eq!(w, wire::DbMediaKind::MEDIA_KIND_AUDIO);
    let d2: MediaKind = MediaKind::try_from(w).unwrap();
    assert_eq!(d, d2);
  }

  #[test]
  fn db_media_kind_unspecified_is_error() {
    let err = MediaKind::try_from(wire::DbMediaKind::MEDIA_KIND_UNSPECIFIED).unwrap_err();
    assert!(err.is_unknown_enum_value());
    assert_eq!(err.try_unwrap_unknown_enum_value_ref(), Ok(&0));
  }

  #[test]
  fn enum_value_known_roundtrip() {
    let ev: ::buffa::EnumValue<wire::DbMediaKind> = MediaKind::Video.into();
    assert!(ev.is_known());
    let d = MediaKind::try_from(&ev).unwrap();
    assert_eq!(d, MediaKind::Video);
  }

  #[test]
  fn enum_value_unknown_is_error() {
    let ev: ::buffa::EnumValue<wire::DbMediaKind> = ::buffa::EnumValue::Unknown(42);
    let err = MediaKind::try_from(&ev).unwrap_err();
    assert_eq!(err.try_unwrap_unknown_enum_value_ref(), Ok(&42));
  }

  const KNOWN_BACKENDS: [Backend; 15] = [
    Backend::Unspecified,
    Backend::Cpu,
    Backend::Onnx,
    Backend::Ggml,
    Backend::Mlx,
    Backend::AppleVision,
    Backend::CoreMl,
    Backend::Candle,
    Backend::Burn,
    Backend::Tract,
    Backend::Torch,
    Backend::TensorRt,
    Backend::OpenVino,
    Backend::TfLite,
    Backend::ExecuTorch,
  ];

  #[test]
  fn backend_roundtrip_every_variant() {
    // Every schema-named variant projects to a bare wire enum (`to_known_wire`
    // is `Some`) and round-trips back losslessly.
    for b in KNOWN_BACKENDS {
      let w = b
        .to_known_wire()
        .expect("known variant has a bare wire enum");
      let b2: Backend = w.into();
      assert_eq!(b, b2, "roundtrip {b}");
    }
  }

  #[test]
  fn backend_roundtrip_every_variant_via_enum_value() {
    // The infallible projection `Provenance.backend` actually uses.
    for b in KNOWN_BACKENDS {
      let ev: ::buffa::EnumValue<wire::Backend> = b.into();
      assert!(ev.is_known(), "known variant rides as Known: {b}");
      let b2: Backend = (&ev).into();
      assert_eq!(b, b2, "EnumValue roundtrip {b}");
    }
  }

  #[test]
  fn backend_unknown_has_no_bare_wire_projection() {
    // REGRESSION: the bare closed `wire::Backend` cannot name an unknown code,
    // so the only `Backend -> bare wire` path (`to_known_wire`) refuses it
    // rather than silently collapsing to `BACKEND_UNSPECIFIED`. There is no
    // infallible `From<Backend> for wire::Backend` to drop the code through.
    assert_eq!(Backend::Unknown(15).to_known_wire(), None);
    assert_eq!(Backend::Unknown(-1).to_known_wire(), None);
    assert_eq!(Backend::Unknown(i32::MAX).to_known_wire(), None);
    // And every KNOWN variant still has one (so the `EnumValue` Known arm is
    // lossless by construction).
    for b in KNOWN_BACKENDS {
      assert!(b.to_known_wire().is_some(), "known variant projects: {b}");
    }

    // The lossless path remains the `EnumValue` wrapper: an unknown code rides
    // verbatim as `EnumValue::Unknown(i)`, NOT as `BACKEND_UNSPECIFIED`.
    let ev: ::buffa::EnumValue<wire::Backend> = Backend::Unknown(15).into();
    assert!(ev.is_unknown());
    assert_eq!(ev.to_i32(), 15);
    assert_eq!(Backend::from(&ev), Backend::Unknown(15));
  }

  #[test]
  fn backend_unknown_wire_int_is_preserved() {
    use crate::domain::Backend;
    // buffa's open-enum wrapper carries an unknown int verbatim, and the
    // bridge now preserves it as `Backend::Unknown(i)` rather than collapsing
    // to `Unspecified` — so the WIRE layer round-trips unknown backends, not
    // just SQL.
    let ev: ::buffa::EnumValue<wire::Backend> = ::buffa::EnumValue::Unknown(99);
    let b: Backend = (&ev).into();
    assert_eq!(b, Backend::Unknown(99));

    // domain Unknown -> wire EnumValue -> domain is lossless.
    let back: ::buffa::EnumValue<wire::Backend> = Backend::Unknown(99).into();
    assert!(back.is_unknown());
    assert_eq!(back.to_i32(), 99);
    let b2: Backend = (&back).into();
    assert_eq!(b2, Backend::Unknown(99));
  }

  #[test]
  fn backend_unknown_round_trips_through_binary_wire() {
    use crate::domain::vo::Provenance;
    use ::buffa::Message as _;

    // End-to-end: a Provenance carrying an unknown backend, encoded to bytes
    // and decoded back, preserves `Backend::Unknown(i)`. This proves the wire
    // (not just the in-memory bridge) keeps the forward-compatible code.
    let d = Provenance::new().with_backend(Backend::from_i32(15));
    assert_eq!(d.backend(), Backend::Unknown(15));
    let w = wire::Provenance::from(&d);
    let bytes = w.encode_to_vec();
    let decoded = wire::Provenance::decode(&mut &bytes[..]).expect("decode");
    assert_eq!(Backend::from(&decoded.backend), Backend::Unknown(15));
  }
}
