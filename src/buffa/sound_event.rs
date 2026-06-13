//! Wire ⇄ domain conversions for [`SoundEvent`].
//!
//! Locked `schema/sound_events.md`. One detected, time-ranged sound-event
//! classification on an `AudioTrack` (the audio analog of `Scene`).
//! Decoding goes through [`SoundEvent::try_new`] — the same validating
//! constructor live application code uses — so the wire-side reconstruction
//! re-checks the domain's invariant model (nil-id / nil-parent rejection,
//! non-inverted span, finite-`[0,1]` score).
//!
//! Unlike [`AudioSegment`](super::audio_segment), `SoundEvent` carries only
//! scalar fields — no nested `Word` / `LocalizedText` / `Language` /
//! `VoiceFingerprint` — so the whole record reconstructs in the single
//! `try_new` call (no `with_*` builder chain needed).
//!
//! ## Field correspondence — [`SoundEvent`]
//!
//! | wire field            | domain field             | notes                                          |
//! | --------------------- | ------------------------ | ---------------------------------------------- |
//! | `id` (Bytes, 16)      | `id` (Uuid7)             | validating                                     |
//! | `audio_track_id` (Bytes, 16) | `audio_track_id` (Uuid7)  | validating; `AudioTrack.id` FK            |
//! | `index: uint32`       | `index: u32`             | 0-based ordinal                                |
//! | `span: TimeRange`     | `span: TimeRange`        | extern via `::mediatime`                       |
//! | `label: string`       | `label: SmolStr`         | CED class name; `""` = unlabeled               |
//! | `code: optional uint64` | `code: Option<u64>`    | optional class code; absent ⇒ `None`           |
//! | `score: float`        | `score: f32`             | validating (`[0,1]`-finite)                    |
//! | `detector: string`    | `detector: CedDetector`  | producer slug (`"ced"` \| `"manual"`)          |

use crate::{
  buffa::error::BuffaError,
  domain::{
    aggregates::audio::sound_event::{SoundEvent, SoundEventError},
    enums::CedDetector,
    Uuid7,
  },
  generated::media::v1 as wire,
};
use ::buffa::bytes::Bytes;

// ---------------------------------------------------------------------------
// SoundEvent<Uuid7> ⇄ wire::SoundEvent
// ---------------------------------------------------------------------------

impl From<&SoundEvent<Uuid7>> for wire::SoundEvent {
  fn from(d: &SoundEvent<Uuid7>) -> Self {
    wire::SoundEvent {
      id: Bytes::copy_from_slice(d.id_ref().as_bytes()),
      audio_track_id: Bytes::copy_from_slice(d.audio_track_id_ref().as_bytes()),
      index: d.index(),
      span: ::buffa::MessageField::some(*d.span_ref()),
      label: d.label().to_owned().into(),
      code: d.code(),
      score: d.score(),
      detector: d.detector().as_str().to_owned().into(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl TryFrom<&wire::SoundEvent> for SoundEvent<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::SoundEvent) -> Result<Self, Self::Error> {
    let id = id_from_bytes(&w.id)?;
    let audio_track_id = id_from_bytes(&w.audio_track_id)?;
    let span = w
      .span
      .as_option()
      .cloned()
      .ok_or(BuffaError::MissingRequiredField("SoundEvent.span"))?;
    // An unrecognized slug is only reachable via a tampered / out-of-contract
    // wire frame: a domain `CedDetector` always serializes to one of its
    // canonical slugs. `BuffaError` has no string-slug variant
    // (`UnknownEnumValue(i32)` is for numeric `EnumValue::Unknown` wire
    // enums, not a string), so this surfaces as `MissingRequiredField` —
    // the same "tampered wire ⇒ generic surface" strategy the rest of this
    // bridge uses.
    let detector = CedDetector::from_str(w.detector.as_str())
      .ok_or(BuffaError::MissingRequiredField("SoundEvent.detector"))?;

    // No `from_parts` is needed: `try_new` is the same public reconstruction
    // surface app code uses, and the only invariant re-checks are the ones a
    // freshly-built sound event must already pass.
    SoundEvent::try_new(
      id,
      audio_track_id,
      w.index,
      span,
      w.label.clone(),
      w.code,
      w.score,
      detector,
    )
    .map_err(sound_event_error_as_buffa)
  }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn id_from_bytes(b: &Bytes) -> Result<Uuid7, BuffaError> {
  let arr: [u8; 16] = b
    .as_ref()
    .try_into()
    .map_err(|_| BuffaError::IdWrongLength(b.len()))?;
  Uuid7::try_from_bytes(arr).map_err(BuffaError::from)
}

/// Re-decode of a previously-valid domain `SoundEvent` should always
/// succeed; a rejection therefore implies wire-tampering. Surface those as
/// `MissingRequiredField` — there's no dedicated `SoundEventError` variant
/// on `BuffaError`, and the existing GPS / Language / `AudioSegment` cases
/// use the same "tampered wire ⇒ generic surface" strategy.
fn sound_event_error_as_buffa(e: SoundEventError) -> BuffaError {
  use SoundEventError as E;
  match e {
    E::NilId => BuffaError::MissingRequiredField("SoundEvent.id"),
    E::NilAudioTrackId => BuffaError::MissingRequiredField("SoundEvent.audio_track_id"),
    E::InvertedSpan => BuffaError::MissingRequiredField("SoundEvent.span"),
    E::ScoreOutOfRange => BuffaError::MissingRequiredField("SoundEvent.score"),
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use core::num::NonZeroU32;
  use mediatime::{TimeRange, Timebase};

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).expect("nonzero"))
  }

  fn span(start: i64, end: i64) -> TimeRange {
    TimeRange::new(start, end, tb())
  }

  // ---- SoundEvent ------------------------------------------------------------

  #[test]
  fn sound_event_minimal_roundtrip() {
    let track = Uuid7::new();
    let d = SoundEvent::try_new(
      Uuid7::new(),
      track,
      0,
      span(0, 1500),
      "",
      None,
      0.0,
      CedDetector::Ced,
    )
    .unwrap();
    let w: wire::SoundEvent = (&d).into();
    let d2 = SoundEvent::try_from(&w).unwrap();
    assert_eq!(d, d2);
    assert_eq!(d2.audio_track_id_ref(), &track);
    assert_eq!(d2.label(), "");
    assert_eq!(d2.code(), None);
    assert!(d2.detector().is_ced());
  }

  #[test]
  fn sound_event_full_roundtrip() {
    let d = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      7,
      span(100, 900),
      "Siren",
      Some(316),
      0.42,
      CedDetector::Manual,
    )
    .unwrap();
    let w: wire::SoundEvent = (&d).into();
    let d2 = SoundEvent::try_from(&w).unwrap();
    assert_eq!(d, d2);
    assert_eq!(d2.index(), 7);
    assert_eq!(d2.label(), "Siren");
    assert_eq!(d2.code(), Some(316));
    assert!((d2.score() - 0.42).abs() < f32::EPSILON);
    assert!(d2.detector().is_manual());
  }

  #[test]
  fn sound_event_missing_span_errors() {
    let d = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 500),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    )
    .unwrap();
    let mut w: wire::SoundEvent = (&d).into();
    w.span = ::buffa::MessageField::none();
    let err = SoundEvent::try_from(&w).unwrap_err();
    assert!(err.is_missing_required_field());
  }

  #[test]
  fn sound_event_wrong_length_id_errors() {
    let d = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 500),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    )
    .unwrap();
    let mut w: wire::SoundEvent = (&d).into();
    w.id = Bytes::copy_from_slice(&[0u8; 8]);
    let err = SoundEvent::try_from(&w).unwrap_err();
    assert!(err.is_id_wrong_length());
  }

  #[test]
  fn sound_event_nil_id_errors() {
    let d = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 500),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    )
    .unwrap();
    let mut w: wire::SoundEvent = (&d).into();
    w.id = Bytes::copy_from_slice(&[0u8; 16]);
    let err = SoundEvent::try_from(&w).unwrap_err();
    assert!(err.is_id_invalid());
  }

  #[test]
  fn sound_event_wrong_length_track_fk_errors() {
    let d = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 500),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    )
    .unwrap();
    let mut w: wire::SoundEvent = (&d).into();
    w.audio_track_id = Bytes::copy_from_slice(&[0u8; 7]);
    let err = SoundEvent::try_from(&w).unwrap_err();
    assert!(err.is_id_wrong_length());
  }

  #[test]
  fn sound_event_unknown_detector_slug_errors() {
    let d = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 500),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    )
    .unwrap();
    let mut w: wire::SoundEvent = (&d).into();
    w.detector = "not_a_detector".into();
    let err = SoundEvent::try_from(&w).unwrap_err();
    assert!(err.is_missing_required_field());
  }

  #[test]
  fn sound_event_out_of_range_score_errors() {
    let d = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 500),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    )
    .unwrap();
    let mut w: wire::SoundEvent = (&d).into();
    w.score = 1.5;
    let err = SoundEvent::try_from(&w).unwrap_err();
    assert!(err.is_missing_required_field());
  }
}
