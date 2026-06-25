//! Shared helpers for the [`VoiceFingerprint`] + [`Provenance`] wire ⇄
//! domain conversions.
//!
//! [`VoiceFingerprint`] is embedded in three places (`Person.voiceprint`,
//! `Speaker.voiceprint`, `AudioSegment.voice_fingerprint`), so the
//! encode/decode pair is factored here and reused by every parent. The
//! `Provenance` pair is exposed alongside because `VoiceFingerprint`
//! always carries a `Provenance` and no parent currently embeds
//! `Provenance` on its own.
//!
//! ## Reconstruction
//!
//! Decoding uses [`VoiceFingerprint::from_parts`] — the infallible
//! storage / wire reconstruction constructor — because the wire bytes
//! were produced by a previously-validated domain instance. The
//! `vector_id` is still length-checked (16 bytes → `Uuid7`) so a
//! malformed wire value surfaces as a [`BuffaError::IdWrongLength`] /
//! [`BuffaError::IdInvalid`] rather than a panic.
//!
//! ## Encode
//!
//! `Option<&VoiceFingerprint<Uuid7>>` ⇒ `MessageField<wire::VoiceFingerprint>`
//! mirrors the parent-side `optional VoiceFingerprint voiceprint = N;`
//! proto3 surface (`None` ⇒ unset).

use ::buffa::bytes::Bytes;
use jiff::Timestamp as JiffTimestamp;
use smol_str::SmolStr;

use crate::{
  buffa::error::BuffaError,
  domain::{
    vo::{Backend, Provenance, VoiceFingerprint},
    Uuid7,
  },
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

// ---------------------------------------------------------------------------
// Provenance ⇄ wire::Provenance — 4 `SmolStr` <-> 4 `String`
// ---------------------------------------------------------------------------

impl From<&Provenance> for wire::Provenance {
  fn from(d: &Provenance) -> Self {
    // `platform` is presence-bearing on the wire (`MessageField`), and the
    // decode side maps an unset field-6 to the all-empty `Platform` (the
    // "not recorded" state). An empty domain platform must therefore encode
    // to `none` — `some(<empty>)` would force a *present* empty field-6,
    // making an absent platform indistinguishable from a recorded-but-empty
    // one (empty-as-absent invariant). Mirrors `language_to_wire`'s
    // `Some -> some` / `None -> none` shape on the encode side.
    let platform = if d.platform_ref().is_empty() {
      ::buffa::MessageField::none()
    } else {
      ::buffa::MessageField::some(wire::Platform::from(d.platform_ref()))
    };
    wire::Provenance {
      model_name: d.model_name().to_owned().into(),
      model_version: d.model_version().to_owned().into(),
      prompt_version: d.prompt_version().to_owned().into(),
      indexer_version: d.indexer_version().to_owned().into(),
      // `backend` is an `EnumValue`; the generated encode gates on the
      // integer (`to_i32() != 0`), so `Backend::Unspecified` (== 0) never
      // produces a present field-5. No guard needed here.
      backend: d.backend().into(),
      platform,
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl From<&wire::Provenance> for Provenance {
  fn from(w: &wire::Provenance) -> Self {
    Provenance::from_parts(
      SmolStr::from(w.model_name.as_str()),
      SmolStr::from(w.model_version.as_str()),
      SmolStr::from(w.prompt_version.as_str()),
      SmolStr::from(w.indexer_version.as_str()),
    )
    .with_backend(Backend::from(&w.backend))
    .with_platform(crate::buffa::vo::platform_from_wire(&w.platform))
  }
}

// ---------------------------------------------------------------------------
// VoiceFingerprint<Uuid7> ⇄ wire::VoiceFingerprint
// ---------------------------------------------------------------------------

impl From<&VoiceFingerprint<Uuid7>> for wire::VoiceFingerprint {
  fn from(d: &VoiceFingerprint<Uuid7>) -> Self {
    // Same empty-as-absent rule as `Provenance.platform`: `provenance` is a
    // presence-bearing `MessageField` whose decode maps an unset field-5 to
    // the all-empty `Provenance::new()` ("not yet recorded"). An empty
    // provenance must encode to `none` so the absent and recorded-but-empty
    // forms stay distinguishable across a domain round-trip.
    let provenance = if d.provenance_ref().is_empty() {
      ::buffa::MessageField::none()
    } else {
      ::buffa::MessageField::some(wire::Provenance::from(d.provenance_ref()))
    };
    wire::VoiceFingerprint {
      vector_id: Bytes::copy_from_slice(d.vector_id_ref().as_bytes()),
      dimensions: d.dimensions(),
      extracted_at: d.extracted_at().as_millisecond(),
      confidence: d.confidence(),
      provenance,
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl TryFrom<&wire::VoiceFingerprint> for VoiceFingerprint<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::VoiceFingerprint) -> Result<Self, Self::Error> {
    let vector_id = id_from_bytes(&w.vector_id)?;
    let extracted_at = JiffTimestamp::from_millisecond(w.extracted_at)
      .map_err(|_| BuffaError::TimestampOutOfRange(w.extracted_at))?;
    // `provenance` is a singular message field — proto3 default is the
    // all-empty `Provenance::new()`, so an unset wire value decodes to
    // the empty domain `Provenance` (the locked "not yet recorded" form).
    let provenance = w
      .provenance
      .as_option()
      .map(Provenance::from)
      .unwrap_or_default();
    // `from_parts` is the storage-reconstruction ctor: the wire bytes
    // were produced by a previously-validated `try_new`, so the
    // nil/zero/range checks aren't repeated here.
    Ok(VoiceFingerprint::from_parts(
      vector_id,
      w.dimensions,
      extracted_at,
      w.confidence,
      provenance,
    ))
  }
}

// ---------------------------------------------------------------------------
// `Option<&VoiceFingerprint>` ⇄ `MessageField<wire::VoiceFingerprint>` —
// parent-side helpers (mirrors the `optional VoiceFingerprint …` proto3
// shape on `Person.voiceprint` / Speaker.voiceprint /
// AudioSegment.voice_fingerprint).
// ---------------------------------------------------------------------------

/// Encode `Option<&VoiceFingerprint<Uuid7>>` to the parent message's
/// `MessageField<wire::VoiceFingerprint>` slot.
pub(super) fn voice_fingerprint_to_wire(
  v: Option<&VoiceFingerprint<Uuid7>>,
) -> ::buffa::MessageField<wire::VoiceFingerprint> {
  match v {
    Some(v) => ::buffa::MessageField::some(wire::VoiceFingerprint::from(v)),
    None => ::buffa::MessageField::none(),
  }
}

/// Decode a parent message's `MessageField<wire::VoiceFingerprint>` slot
/// into an `Option<VoiceFingerprint<Uuid7>>`. An unset slot ⇒ `None`.
pub(super) fn voice_fingerprint_from_wire(
  w: &::buffa::MessageField<wire::VoiceFingerprint>,
) -> Result<Option<VoiceFingerprint<Uuid7>>, BuffaError> {
  match w.as_option() {
    Some(v) => VoiceFingerprint::try_from(v).map(Some),
    None => Ok(None),
  }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Decode a 16-byte wire `Bytes` (inline-bytes id convention) into the
/// validating domain `Uuid7`. Surfaces `IdWrongLength` / `IdInvalid` for
/// malformed wire input. Local copy of the helper used by
/// `media_file.rs` — kept private to this module to avoid coupling the
/// two bridges.
fn id_from_bytes(b: &Bytes) -> Result<Uuid7, BuffaError> {
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

  fn ts() -> JiffTimestamp {
    JiffTimestamp::from_millisecond(1_700_000_000_000).expect("valid timestamp")
  }

  fn vfp(confidence: Option<f32>) -> VoiceFingerprint<Uuid7> {
    VoiceFingerprint::try_new(
      Uuid7::new(),
      192,
      ts(),
      confidence,
      Provenance::from_parts("ecapa-tdnn", "v1.0.0", "p-7", "findit-indexer-0.1.0"),
    )
    .expect("valid voiceprint")
  }

  #[test]
  fn provenance_roundtrip_non_empty() {
    let d = Provenance::from_parts("m", "v", "p", "i");
    let w: wire::Provenance = (&d).into();
    let d2: Provenance = (&w).into();
    assert_eq!(d, d2);
  }

  #[test]
  fn provenance_roundtrip_all_empty() {
    let d = Provenance::new();
    let w: wire::Provenance = (&d).into();
    assert!(w.model_name.is_empty());
    let d2: Provenance = (&w).into();
    assert_eq!(d, d2);
    assert!(d2.is_empty());
  }

  #[test]
  fn provenance_roundtrip_carries_backend_and_platform() {
    use crate::domain::vo::{Backend, Platform};
    let d = Provenance::from_parts("ecapa-tdnn", "v1", "p", "idx")
      .with_backend(Backend::Onnx)
      .with_platform(Platform::from_parts("macos", "aarch64", "15.5"));
    let w: wire::Provenance = (&d).into();
    let d2: Provenance = (&w).into();
    assert_eq!(d, d2);
    assert_eq!(d2.backend(), Backend::Onnx);
    assert_eq!(d2.platform_ref().os(), "macos");
  }

  #[test]
  fn provenance_roundtrip_default_backend_platform() {
    use crate::domain::vo::Backend;
    let d = Provenance::from_parts("m", "v", "p", "i"); // Unspecified + empty
    let w: wire::Provenance = (&d).into();
    let d2: Provenance = (&w).into();
    assert_eq!(d, d2);
    assert_eq!(d2.backend(), Backend::Unspecified);
    assert!(d2.platform_ref().is_empty());
  }

  // --- empty-as-absent: presence-bearing `Provenance.platform` (field 6) ---

  /// An empty domain `Platform` must NOT produce a present (empty) field-6
  /// on the wire — it stays unset, so an absent platform and a
  /// recorded-but-empty one remain distinguishable.
  #[test]
  fn provenance_empty_platform_encodes_unset() {
    let d = Provenance::from_parts("m", "v", "p", "i"); // platform = all-empty
    assert!(d.platform_ref().is_empty());
    let w: wire::Provenance = (&d).into();
    assert!(w.platform.is_unset(), "empty platform must encode to none");
  }

  /// A wire `Provenance` with no field-6 round-trips through the domain
  /// without the re-encode introducing a present empty platform.
  #[test]
  fn provenance_unset_platform_wire_domain_wire_idempotent() {
    let mut w0 = wire::Provenance::from(&Provenance::from_parts("m", "v", "p", "i"));
    w0.platform = ::buffa::MessageField::none();
    assert!(w0.platform.is_unset());
    let d: Provenance = (&w0).into();
    assert!(d.platform_ref().is_empty());
    let w1: wire::Provenance = (&d).into();
    assert!(
      w1.platform.is_unset(),
      "round-trip must keep platform unset"
    );
  }

  /// A non-empty `Platform` round-trips PRESENT (encoded `some`, decoded
  /// back equal).
  #[test]
  fn provenance_non_empty_platform_encodes_present() {
    use crate::domain::vo::Platform;
    let d = Provenance::from_parts("m", "v", "p", "i")
      .with_platform(Platform::from_parts("macos", "aarch64", "15.5"));
    let w: wire::Provenance = (&d).into();
    assert!(
      w.platform.is_set(),
      "non-empty platform must encode to some"
    );
    let d2: Provenance = (&w).into();
    assert_eq!(d, d2);
    assert!(!d2.platform_ref().is_empty());
  }

  // --- empty-as-absent: `Provenance.backend` (EnumValue, field 5) ---

  /// `Backend::Unspecified` (wire int 0) must serialize as absent: a
  /// proto3 enum at its default value writes no field. Verified through
  /// the binary encode (`Message::encode` ⇒ bytes ⇒ `decode`), which is
  /// where `EnumValue`'s `to_i32() != 0` presence gate lives.
  #[test]
  fn provenance_unspecified_backend_is_absent_on_binary_wire() {
    use crate::domain::vo::Backend;
    use ::buffa::Message as _;

    // All fields empty + Unspecified backend ⇒ the message encodes to
    // ZERO bytes (every field is at its proto3 default / absent).
    let empty = wire::Provenance::from(&Provenance::new());
    assert_eq!(
      empty.encode_to_vec().len(),
      0,
      "all-default Provenance (incl. Unspecified backend + empty platform) must encode to no bytes",
    );

    // A Provenance that differs ONLY by a concrete backend must add
    // exactly field-5; decoding restores the backend cleanly.
    let with_backend = wire::Provenance::from(&Provenance::new().with_backend(Backend::Onnx));
    let bytes = with_backend.encode_to_vec();
    assert!(!bytes.is_empty(), "a concrete backend must produce field-5");
    let decoded = wire::Provenance::decode(&mut &bytes[..]).expect("decode");
    assert_eq!(Backend::from(&decoded.backend), Backend::Onnx);
  }

  // --- empty-as-absent: `VoiceFingerprint.provenance` (field 5) ---

  /// An empty domain `Provenance` on a `VoiceFingerprint` encodes to an
  /// unset field-5 (same invariant as `Provenance.platform`).
  #[test]
  fn voice_fingerprint_empty_provenance_encodes_unset() {
    let d = VoiceFingerprint::from_parts(Uuid7::new(), 128, ts(), None, Provenance::new());
    assert!(d.provenance_ref().is_empty());
    let w: wire::VoiceFingerprint = (&d).into();
    assert!(
      w.provenance.is_unset(),
      "empty provenance must encode to none"
    );
    // wire ⇒ domain ⇒ wire idempotency: unset stays unset.
    let d2 = VoiceFingerprint::try_from(&w).expect("roundtrip");
    assert!(d2.provenance_ref().is_empty());
    let w2: wire::VoiceFingerprint = (&d2).into();
    assert!(w2.provenance.is_unset());
  }

  /// A non-empty provenance round-trips PRESENT on a `VoiceFingerprint`.
  #[test]
  fn voice_fingerprint_non_empty_provenance_encodes_present() {
    let d = vfp(Some(0.5)); // provenance = ecapa-tdnn (non-empty)
    assert!(!d.provenance_ref().is_empty());
    let w: wire::VoiceFingerprint = (&d).into();
    assert!(
      w.provenance.is_set(),
      "non-empty provenance must encode some"
    );
    let d2 = VoiceFingerprint::try_from(&w).expect("roundtrip");
    assert_eq!(d, d2);
  }

  #[test]
  fn voice_fingerprint_roundtrip_with_confidence() {
    let d = vfp(Some(0.83));
    let w: wire::VoiceFingerprint = (&d).into();
    let d2 = VoiceFingerprint::try_from(&w).expect("roundtrip");
    assert_eq!(d, d2);
  }

  #[test]
  fn voice_fingerprint_roundtrip_without_confidence() {
    let d = vfp(None);
    let w: wire::VoiceFingerprint = (&d).into();
    assert!(w.confidence.is_none());
    let d2 = VoiceFingerprint::try_from(&w).expect("roundtrip");
    assert_eq!(d, d2);
    assert!(d2.confidence().is_none());
  }

  #[test]
  fn voice_fingerprint_unset_provenance_decodes_as_empty() {
    let d = vfp(Some(0.5));
    let mut w: wire::VoiceFingerprint = (&d).into();
    w.provenance = ::buffa::MessageField::none();
    let d2 = VoiceFingerprint::try_from(&w).expect("missing provenance ⇒ empty Provenance");
    assert!(d2.provenance_ref().is_empty());
  }

  #[test]
  fn voice_fingerprint_wrong_length_vector_id_errors() {
    let d = vfp(None);
    let mut w: wire::VoiceFingerprint = (&d).into();
    w.vector_id = Bytes::copy_from_slice(&[0u8; 8]);
    let err = VoiceFingerprint::try_from(&w).unwrap_err();
    assert!(err.is_id_wrong_length());
  }

  #[test]
  fn voice_fingerprint_nil_vector_id_errors() {
    let d = vfp(None);
    let mut w: wire::VoiceFingerprint = (&d).into();
    w.vector_id = Bytes::copy_from_slice(&[0u8; 16]);
    let err = VoiceFingerprint::try_from(&w).unwrap_err();
    assert!(err.is_id_invalid());
  }

  #[test]
  fn voice_fingerprint_extracted_at_out_of_range_errors() {
    let d = vfp(None);
    let mut w: wire::VoiceFingerprint = (&d).into();
    w.extracted_at = i64::MAX;
    let err = VoiceFingerprint::try_from(&w).unwrap_err();
    assert!(err.is_timestamp_out_of_range());
  }

  #[test]
  fn message_field_helpers_roundtrip_present_and_absent() {
    let d = vfp(Some(0.75));
    let present = voice_fingerprint_to_wire(Some(&d));
    let decoded = voice_fingerprint_from_wire(&present).expect("present roundtrip");
    assert_eq!(decoded.as_ref(), Some(&d));

    let absent = voice_fingerprint_to_wire(None);
    let decoded = voice_fingerprint_from_wire(&absent).expect("absent roundtrip");
    assert!(decoded.is_none());
  }
}
