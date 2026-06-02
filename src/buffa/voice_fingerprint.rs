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
    vo::{Provenance, VoiceFingerprint},
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
    wire::Provenance {
      model_name: d.model_name().to_owned().into(),
      model_version: d.model_version().to_owned().into(),
      prompt_version: d.prompt_version().to_owned().into(),
      indexer_version: d.indexer_version().to_owned().into(),
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
  }
}

// ---------------------------------------------------------------------------
// VoiceFingerprint<Uuid7> ⇄ wire::VoiceFingerprint
// ---------------------------------------------------------------------------

impl From<&VoiceFingerprint<Uuid7>> for wire::VoiceFingerprint {
  fn from(d: &VoiceFingerprint<Uuid7>) -> Self {
    wire::VoiceFingerprint {
      vector_id: Bytes::copy_from_slice(d.vector_id_ref().as_bytes()),
      dimensions: d.dimensions(),
      extracted_at: d.extracted_at().as_millisecond(),
      confidence: d.confidence(),
      provenance: ::buffa::MessageField::some(wire::Provenance::from(d.provenance_ref())),
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
