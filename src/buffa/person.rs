//! Wire ⇄ domain conversions for [`Person`].
//!
//! Locked `schema/person.md`. Cross-track / cross-modality identity
//! anchor — one `Person` ↔ many `Speaker`s (one per track they appear
//! in). Modality-neutral so a future face-side identity can hang off
//! the same aggregate.
//!
//! ## Field correspondence
//!
//! | wire field            | domain field             | notes                                          |
//! | --------------------- | ------------------------ | ---------------------------------------------- |
//! | `id` (Bytes, 16)      | `id` (Uuid7)             | validating                                     |
//! | `name: String`        | `name: SmolStr`          | `""` = unnamed (locked convention)             |
//! | `confidence: EnumValue<PersonConfidence>` | `confidence: PersonConfidence` | unknown wire value ⇒ `BuffaError::UnknownEnumValue` |
//! | `voiceprint: optional VoiceFingerprint`   | `voiceprint: Option<VoiceFingerprint<Uuid7>>` | shared encode/decode helper      |
//! | `created_at: i64` ms  | `created_at: jiff::Timestamp` | validating (out-of-range ⇒ `TimestampOutOfRange`) |
//! | `updated_at: i64` ms  | `updated_at: jiff::Timestamp` | validating                                     |
//!
//! ## Reconstruction
//!
//! Decoding uses [`Person::from_parts`] — the infallible storage / wire
//! reconstruction constructor — because the wire bytes were produced by
//! a previously-validated domain instance.

use jiff::Timestamp as JiffTimestamp;
use smol_str::SmolStr;

use crate::{
  buffa::{
    error::BuffaError,
    voice_fingerprint::{voice_fingerprint_from_wire, voice_fingerprint_to_wire},
  },
  domain::{
    aggregates::person::{Person, PersonConfidence},
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
// PersonConfidence ⇄ wire::PersonConfidence (+ EnumValue<…>)
// ---------------------------------------------------------------------------

impl From<PersonConfidence> for wire::PersonConfidence {
  fn from(d: PersonConfidence) -> Self {
    match d {
      PersonConfidence::AutoMatched => wire::PersonConfidence::PERSON_CONFIDENCE_AUTO_MATCHED,
      PersonConfidence::UserConfirmed => wire::PersonConfidence::PERSON_CONFIDENCE_USER_CONFIRMED,
    }
  }
}

impl From<wire::PersonConfidence> for PersonConfidence {
  /// Closed wire enum, closed domain enum — every variant has a peer.
  fn from(w: wire::PersonConfidence) -> Self {
    match w {
      wire::PersonConfidence::PERSON_CONFIDENCE_AUTO_MATCHED => PersonConfidence::AutoMatched,
      wire::PersonConfidence::PERSON_CONFIDENCE_USER_CONFIRMED => PersonConfidence::UserConfirmed,
    }
  }
}

impl From<PersonConfidence> for ::buffa::EnumValue<wire::PersonConfidence> {
  fn from(d: PersonConfidence) -> Self {
    ::buffa::EnumValue::Known(wire::PersonConfidence::from(d))
  }
}

impl TryFrom<&::buffa::EnumValue<wire::PersonConfidence>> for PersonConfidence {
  type Error = BuffaError;

  /// `EnumValue::Known` decodes verbatim. The wire layer carries an open
  /// enum container; an unknown raw `i32` surfaces as
  /// [`BuffaError::UnknownEnumValue`].
  fn try_from(w: &::buffa::EnumValue<wire::PersonConfidence>) -> Result<Self, Self::Error> {
    match w {
      ::buffa::EnumValue::Known(k) => Ok(PersonConfidence::from(*k)),
      ::buffa::EnumValue::Unknown(v) => Err(BuffaError::UnknownEnumValue(*v)),
    }
  }
}

// ---------------------------------------------------------------------------
// Person<Uuid7> ⇄ wire::Person
// ---------------------------------------------------------------------------

impl From<&Person<Uuid7>> for wire::Person {
  fn from(d: &Person<Uuid7>) -> Self {
    wire::Person {
      id: ::buffa::bytes::Bytes::copy_from_slice(d.id_ref().as_bytes()),
      name: d.name().to_owned().into(),
      confidence: d.confidence().into(),
      voiceprint: voice_fingerprint_to_wire(d.voiceprint_ref()),
      created_at: d.created_at().as_millisecond(),
      updated_at: d.updated_at().as_millisecond(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl TryFrom<&wire::Person> for Person<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::Person) -> Result<Self, Self::Error> {
    let id = id_from_bytes(&w.id)?;
    let name = SmolStr::from(w.name.as_str());
    let confidence = PersonConfidence::try_from(&w.confidence)?;
    let voiceprint = voice_fingerprint_from_wire(&w.voiceprint)?;
    let created_at = ms_to_jiff(w.created_at)?;
    let updated_at = ms_to_jiff(w.updated_at)?;
    // `from_parts` is the raw storage-reconstruction ctor: the wire
    // bytes were produced by a previously-validated `try_new`, so the
    // nil-id check is not repeated here.
    Ok(Person::from_parts(
      id, name, voiceprint, confidence, created_at, updated_at,
    ))
  }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn id_from_bytes(b: &::buffa::bytes::Bytes) -> Result<Uuid7, BuffaError> {
  let arr: [u8; 16] = b
    .as_ref()
    .try_into()
    .map_err(|_| BuffaError::IdWrongLength(b.len()))?;
  Uuid7::try_from_bytes(arr).map_err(BuffaError::from)
}

fn ms_to_jiff(ms: i64) -> Result<JiffTimestamp, BuffaError> {
  JiffTimestamp::from_millisecond(ms).map_err(|_| BuffaError::TimestampOutOfRange(ms))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use crate::domain::vo::{Provenance, VoiceFingerprint};

  fn ts() -> JiffTimestamp {
    JiffTimestamp::from_millisecond(1_700_000_000_000).expect("valid timestamp")
  }

  fn vfp() -> VoiceFingerprint<Uuid7> {
    VoiceFingerprint::try_new(
      Uuid7::new(),
      192,
      ts(),
      Some(0.83),
      Provenance::from_parts("ecapa-tdnn", "v1.0.0", "", "findit-indexer-0.1.0"),
    )
    .expect("valid voiceprint")
  }

  // ---- PersonConfidence ------------------------------------------------------

  #[test]
  fn person_confidence_roundtrip_auto_matched() {
    let d = PersonConfidence::AutoMatched;
    let w: wire::PersonConfidence = d.into();
    assert_eq!(w, wire::PersonConfidence::PERSON_CONFIDENCE_AUTO_MATCHED);
    let d2: PersonConfidence = w.into();
    assert_eq!(d, d2);
  }

  #[test]
  fn person_confidence_roundtrip_user_confirmed() {
    let d = PersonConfidence::UserConfirmed;
    let w: wire::PersonConfidence = d.into();
    assert_eq!(w, wire::PersonConfidence::PERSON_CONFIDENCE_USER_CONFIRMED);
    let d2: PersonConfidence = w.into();
    assert_eq!(d, d2);
  }

  #[test]
  fn person_confidence_enum_value_known_roundtrip() {
    let ev: ::buffa::EnumValue<wire::PersonConfidence> = PersonConfidence::UserConfirmed.into();
    assert!(ev.is_known());
    let d = PersonConfidence::try_from(&ev).unwrap();
    assert_eq!(d, PersonConfidence::UserConfirmed);
  }

  #[test]
  fn person_confidence_enum_value_unknown_errors() {
    let ev: ::buffa::EnumValue<wire::PersonConfidence> = ::buffa::EnumValue::Unknown(42);
    let err = PersonConfidence::try_from(&ev).unwrap_err();
    assert_eq!(err.try_unwrap_unknown_enum_value_ref(), Ok(&42));
  }

  // ---- Person round-trip -----------------------------------------------------

  #[test]
  fn person_minimal_roundtrip() {
    let p = Person::try_new(Uuid7::new(), "", ts(), ts()).expect("valid construction");
    let w: wire::Person = (&p).into();
    let p2: Person<Uuid7> = Person::try_from(&w).expect("roundtrip");
    assert_eq!(p, p2);
    assert!(p2.voiceprint_ref().is_none());
    assert_eq!(p2.confidence(), PersonConfidence::AutoMatched);
  }

  #[test]
  fn person_full_roundtrip_with_voiceprint() {
    let p = Person::try_new(Uuid7::new(), "Jane Doe", ts(), ts())
      .unwrap()
      .with_voiceprint(vfp())
      .with_confidence(PersonConfidence::UserConfirmed);
    let w: wire::Person = (&p).into();
    let p2: Person<Uuid7> = Person::try_from(&w).expect("roundtrip");
    assert_eq!(p, p2);
  }

  #[test]
  fn person_voiceprint_absence_round_trips_as_unset_message_field() {
    let p = Person::try_new(Uuid7::new(), "", ts(), ts()).unwrap();
    let w: wire::Person = (&p).into();
    assert!(w.voiceprint.as_option().is_none());
    let p2: Person<Uuid7> = Person::try_from(&w).unwrap();
    assert!(p2.voiceprint_ref().is_none());
  }

  // ---- Failure modes ---------------------------------------------------------

  #[test]
  fn person_wrong_length_id_errors() {
    let p = Person::try_new(Uuid7::new(), "", ts(), ts()).unwrap();
    let mut w: wire::Person = (&p).into();
    w.id = ::buffa::bytes::Bytes::copy_from_slice(&[0u8; 8]);
    let err = Person::try_from(&w).unwrap_err();
    assert!(err.is_id_wrong_length());
  }

  #[test]
  fn person_nil_id_errors() {
    let p = Person::try_new(Uuid7::new(), "", ts(), ts()).unwrap();
    let mut w: wire::Person = (&p).into();
    w.id = ::buffa::bytes::Bytes::copy_from_slice(&[0u8; 16]);
    let err = Person::try_from(&w).unwrap_err();
    assert!(err.is_id_invalid());
  }

  #[test]
  fn person_unknown_confidence_enum_errors() {
    let p = Person::try_new(Uuid7::new(), "", ts(), ts()).unwrap();
    let mut w: wire::Person = (&p).into();
    w.confidence = ::buffa::EnumValue::Unknown(7);
    let err = Person::try_from(&w).unwrap_err();
    assert!(err.is_unknown_enum_value());
  }

  #[test]
  fn person_created_at_out_of_range_errors() {
    let p = Person::try_new(Uuid7::new(), "", ts(), ts()).unwrap();
    let mut w: wire::Person = (&p).into();
    w.created_at = i64::MAX;
    let err = Person::try_from(&w).unwrap_err();
    assert!(err.is_timestamp_out_of_range());
  }

  // ---- Cross-Person identity reuse -------------------------------------------

  /// Two `Speaker`-style FKs (encoded inline as the same `Person.id`
  /// bytes) point at the same `Person` row and round-trip identically.
  /// `Speaker` has no wire counterpart yet, so the test is expressed at
  /// the `Person` level: encode → decode → re-encode preserves the id,
  /// so two consumers holding the same wire bytes will decode to the
  /// same domain id.
  #[test]
  fn person_shared_by_two_speakers_round_trip() {
    let pid = Uuid7::new();
    let p = Person::try_new(pid, "Jane", ts(), ts())
      .unwrap()
      .with_confidence(PersonConfidence::UserConfirmed)
      .with_voiceprint(vfp());
    let w: wire::Person = (&p).into();
    let d1 = Person::try_from(&w).unwrap();
    let d2 = Person::try_from(&w).unwrap();
    assert_eq!(d1.id_ref(), &pid);
    assert_eq!(d2.id_ref(), &pid);
    assert_eq!(d1, d2);
    assert_eq!(d1, p);
  }
}
