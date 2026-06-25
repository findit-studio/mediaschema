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
// Backend ⇄ wire::Backend / EnumValue<wire::Backend>  (infallible both ways —
// Backend has an Unspecified default + a lossless `Unknown(i32)` arm, so no
// UnknownEnumValue error)
// ---------------------------------------------------------------------------

impl From<Backend> for wire::Backend {
  /// Lossy on `Backend::Unknown(i)`: the generated `wire::Backend` is a
  /// **closed** enum with no `Unknown` arm, so a forward-compatible code it
  /// can't name falls back to `BACKEND_UNSPECIFIED`. Unknown backends are
  /// preserved on the wire through `EnumValue<wire::Backend>` instead (the
  /// `From<Backend> for EnumValue<wire::Backend>` impl below) — that, not this
  /// bare-enum projection, is the type `Provenance.backend` actually carries.
  fn from(d: Backend) -> Self {
    <wire::Backend as ::buffa::Enumeration>::from_i32(d.to_i32())
      .unwrap_or(wire::Backend::BACKEND_UNSPECIFIED)
  }
}

impl From<wire::Backend> for Backend {
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
    match d {
      Backend::Unknown(i) => ::buffa::EnumValue::Unknown(i),
      known => ::buffa::EnumValue::Known(wire::Backend::from(known)),
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

  #[test]
  fn backend_roundtrip_every_variant() {
    use crate::domain::Backend;
    let all = [
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
    for b in all {
      let w: wire::Backend = b.into();
      let b2: Backend = w.into();
      assert_eq!(b, b2, "roundtrip {b}");
    }
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
