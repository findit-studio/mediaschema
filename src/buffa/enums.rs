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
// Backend ⇄ wire::Backend  (infallible both ways — Backend has an
// Unspecified / unknown arm, so no UnknownEnumValue error)
// ---------------------------------------------------------------------------

impl From<Backend> for wire::Backend {
  fn from(d: Backend) -> Self {
    // Map through the pinned wire integer; codegen's `from_i32` is total
    // over the known range, and `Backend::to_i32` only emits known values.
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
  fn from(d: Backend) -> Self {
    ::buffa::EnumValue::Known(wire::Backend::from(d))
  }
}

impl From<&::buffa::EnumValue<wire::Backend>> for Backend {
  fn from(w: &::buffa::EnumValue<wire::Backend>) -> Self {
    match w {
      ::buffa::EnumValue::Known(k) => Backend::from(*k),
      // Forward-compatible unknown int -> Unspecified (total).
      ::buffa::EnumValue::Unknown(_) => Backend::Unspecified,
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
  fn backend_unknown_wire_int_is_unspecified() {
    use crate::domain::Backend;
    // The open-enum container can hold an unknown int; it decodes to
    // Unspecified (total, forward-compatible).
    let ev: ::buffa::EnumValue<wire::Backend> = ::buffa::EnumValue::Unknown(99);
    let b: Backend = (&ev).into();
    assert_eq!(b, Backend::Unspecified);
  }
}
