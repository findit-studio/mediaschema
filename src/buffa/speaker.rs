//! Wire ⇄ domain conversions for [`Speaker`].
//!
//! Locked `schema/speaker.md`. Per-track diarized voice. Decoding goes
//! through [`Speaker::try_new`] + the (`try_`)`with_*` / `maybe_*`
//! builder chain — the same path live application code uses — so
//! the wire-side ordering of field application matches the domain's
//! invariant model:
//!
//! 1. `try_new(id, audio_track_id, cluster_id, name)` enforces nil-id / nil-parent.
//! 2. `try_with_speech_duration(...)` enforces non-negative.
//! 3. `maybe_voiceprint(...)` / `maybe_person(...)` are infallible.
//!
//! ## Field correspondence
//!
//! | wire field            | domain field             | notes                                          |
//! | --------------------- | ------------------------ | ---------------------------------------------- |
//! | `id` (Bytes, 16)      | `id` (Uuid7)             | validating                                     |
//! | `audio_track_id` (Bytes, 16) | `audio_track_id` (Uuid7)         | validating; `AudioTrack.id` FK                 |
//! | `cluster_id: uint32`  | `cluster_id: u32`        | `dia` cluster label within the track           |
//! | `name: String`        | `name: SmolStr`          | `""` = unassigned (locked convention)          |
//! | `speech_duration: optional Timestamp` | `speech_duration: Option<mediatime::Timestamp>` | extern via `::mediatime` |
//! | `voiceprint: optional VoiceFingerprint` | `voiceprint: Option<VoiceFingerprint<Uuid7>>` | shared helper |
//! | `person_id: optional bytes` | `person_id: Option<Uuid7>` | 16-byte FK → `Person.id`; absent ⇒ `None`     |

use ::buffa::bytes::Bytes;
use smol_str::SmolStr;

use crate::{
  buffa::{
    error::BuffaError,
    voice_fingerprint::{voice_fingerprint_from_wire, voice_fingerprint_to_wire},
  },
  domain::{aggregates::speaker::Speaker, Uuid7},
  generated::media::v1 as wire,
};
// Under `feature = "alloc"` (no std), `String` / `ToOwned` / `ToString`
// aren't in the prelude — pull them in via the `extern crate alloc as std`
// alias declared in `lib.rs`. Under `feature = "std"` these come from the
// std prelude automatically; the cfg keeps the import a no-op there.
#[cfg(all(not(feature = "std"), feature = "alloc"))]
#[allow(unused_imports)]
use std::{borrow::ToOwned, string::{String, ToString}};


// ---------------------------------------------------------------------------
// Speaker<Uuid7> ⇄ wire::Speaker
// ---------------------------------------------------------------------------

impl From<&Speaker<Uuid7>> for wire::Speaker {
  fn from(d: &Speaker<Uuid7>) -> Self {
    wire::Speaker {
      id: Bytes::copy_from_slice(d.id_ref().as_bytes()),
      audio_track_id: Bytes::copy_from_slice(d.audio_track_id_ref().as_bytes()),
      cluster_id: d.cluster_id(),
      name: d.name().to_owned(),
      speech_duration: match d.speech_duration_ref() {
        Some(ts) => ::buffa::MessageField::some(*ts),
        None => ::buffa::MessageField::none(),
      },
      voiceprint: voice_fingerprint_to_wire(d.voiceprint_ref()),
      person_id: d
        .person_id_ref()
        .map(|id| Bytes::copy_from_slice(id.as_bytes())),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl TryFrom<&wire::Speaker> for Speaker<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::Speaker) -> Result<Self, Self::Error> {
    let id = id_from_bytes(&w.id)?;
    let parent = id_from_bytes(&w.audio_track_id)?;
    let speech_duration = w.speech_duration.as_option().copied();
    let voiceprint = voice_fingerprint_from_wire(&w.voiceprint)?;
    let person = match &w.person_id {
      Some(b) => Some(id_from_bytes(b)?),
      None => None,
    };

    // `try_new` + builder chain matches the live-code reconstruction
    // path. The wire bytes were produced by a previously-validated
    // domain `Speaker`, so the only invariant that could re-trip here
    // is a tampered-with `speech_duration` — surfaced as
    // `MissingRequiredField` (no dedicated `SpeakerError` variant on
    // `BuffaError`, same strategy as `AudioSegment`).
    let speaker = Speaker::try_new(id, parent, w.cluster_id, SmolStr::from(w.name.as_str()))
      .map_err(speaker_error_as_buffa)?
      .try_with_speech_duration(speech_duration)
      .map_err(speaker_error_as_buffa)?
      .maybe_voiceprint(voiceprint)
      .maybe_person_id(person);
    Ok(speaker)
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

fn speaker_error_as_buffa(e: crate::domain::aggregates::speaker::SpeakerError) -> BuffaError {
  use crate::domain::aggregates::speaker::SpeakerError as E;
  match e {
    E::NilId => BuffaError::MissingRequiredField("Speaker.id"),
    E::NilAudioTrackId => BuffaError::MissingRequiredField("Speaker.audio_track_id"),
    E::NegativeSpeechDuration => BuffaError::MissingRequiredField("Speaker.speech_duration"),
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use crate::domain::vo::{Provenance, VoiceFingerprint};
  use core::num::NonZeroU32;
  use jiff::Timestamp as JiffTimestamp;
  use mediatime::{Timebase, Timestamp};

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).expect("nonzero"))
  }

  fn vfp() -> VoiceFingerprint<Uuid7> {
    VoiceFingerprint::try_new(
      Uuid7::new(),
      192,
      JiffTimestamp::from_millisecond(1_700_000_000_000).expect("valid ts"),
      Some(0.83),
      Provenance::from_parts("ecapa-tdnn", "v1.0.0", "", "findit-indexer-0.1.0"),
    )
    .expect("valid voiceprint")
  }

  #[test]
  fn speaker_minimal_roundtrip() {
    let parent = Uuid7::new();
    let d = Speaker::try_new(Uuid7::new(), parent, 2, "Jane").unwrap();
    let w: wire::Speaker = (&d).into();
    let d2 = Speaker::try_from(&w).unwrap();
    assert_eq!(d, d2);
    assert_eq!(d2.audio_track_id_ref(), &parent);
    assert!(d2.voiceprint_ref().is_none());
    assert!(d2.person_id_ref().is_none());
    assert!(d2.speech_duration_ref().is_none());
  }

  #[test]
  fn speaker_full_roundtrip_with_voiceprint_person_and_duration() {
    let pid = Uuid7::new();
    let d = Speaker::try_new(Uuid7::new(), Uuid7::new(), 0, "Jane")
      .unwrap()
      .try_with_speech_duration(Some(Timestamp::new(5000, tb())))
      .unwrap()
      .with_voiceprint(vfp())
      .with_person_id(pid);
    let w: wire::Speaker = (&d).into();
    let d2 = Speaker::try_from(&w).unwrap();
    assert_eq!(d, d2);
    assert_eq!(d2.person_id_ref(), Some(&pid));
  }

  #[test]
  fn speaker_voiceprint_absence_roundtrips_as_unset_message_field() {
    let d = Speaker::try_new(Uuid7::new(), Uuid7::new(), 0, "").unwrap();
    let w: wire::Speaker = (&d).into();
    assert!(w.voiceprint.as_option().is_none());
    assert!(w.person_id.is_none());
    let d2 = Speaker::try_from(&w).unwrap();
    assert!(d2.voiceprint_ref().is_none());
  }

  #[test]
  fn speaker_wrong_length_id_errors() {
    let d = Speaker::try_new(Uuid7::new(), Uuid7::new(), 0, "").unwrap();
    let mut w: wire::Speaker = (&d).into();
    w.id = Bytes::copy_from_slice(&[0u8; 8]);
    let err = Speaker::try_from(&w).unwrap_err();
    assert!(err.is_id_wrong_length());
  }

  #[test]
  fn speaker_nil_id_errors() {
    let d = Speaker::try_new(Uuid7::new(), Uuid7::new(), 0, "").unwrap();
    let mut w: wire::Speaker = (&d).into();
    w.id = Bytes::copy_from_slice(&[0u8; 16]);
    let err = Speaker::try_from(&w).unwrap_err();
    assert!(err.is_id_invalid());
  }

  #[test]
  fn speaker_wrong_length_person_fk_errors() {
    let d = Speaker::try_new(Uuid7::new(), Uuid7::new(), 0, "")
      .unwrap()
      .with_person_id(Uuid7::new());
    let mut w: wire::Speaker = (&d).into();
    w.person_id = Some(Bytes::copy_from_slice(&[0u8; 7]));
    let err = Speaker::try_from(&w).unwrap_err();
    assert!(err.is_id_wrong_length());
  }

  // ---- Cross-aggregate FK round-trip ----------------------------------------

  /// A `Speaker` linked to a `Person.id` plus multiple `AudioSegment`s
  /// referencing that `Speaker.id` round-trip identically — the FK bytes
  /// survive the wire trip unchanged.
  #[test]
  fn speaker_and_audio_segments_share_fks_after_roundtrip() {
    // The `buffa::audio_segment` module brings the
    // `From<&AudioSegment> for wire::AudioSegment` impls into the crate
    // via `inventory`-style impl-in-its-own-module; no explicit `use`
    // is needed here — the impls are visible wherever the parent
    // types are.
    use crate::domain::aggregates::audio::segment::AudioSegment;
    use mediatime::TimeRange;

    let person_id = Uuid7::new();
    let speaker_id = Uuid7::new();
    let track_id = Uuid7::new();
    let spk = Speaker::try_new(speaker_id, track_id, 0, "Jane")
      .unwrap()
      .with_person_id(person_id)
      .with_voiceprint(vfp());
    let seg1 =
      AudioSegment::<Uuid7>::try_new(Uuid7::new(), track_id, 0, TimeRange::new(0, 500, tb()))
        .unwrap()
        .with_speaker_id(Some(speaker_id));
    let seg2 =
      AudioSegment::<Uuid7>::try_new(Uuid7::new(), track_id, 1, TimeRange::new(500, 1000, tb()))
        .unwrap()
        .with_speaker_id(Some(speaker_id));

    let w_spk: wire::Speaker = (&spk).into();
    let w_seg1: wire::AudioSegment = (&seg1).into();
    let w_seg2: wire::AudioSegment = (&seg2).into();
    let spk2 = Speaker::try_from(&w_spk).unwrap();
    let seg1b = AudioSegment::<Uuid7>::try_from(&w_seg1).unwrap();
    let seg2b = AudioSegment::<Uuid7>::try_from(&w_seg2).unwrap();

    assert_eq!(spk2.person_id_ref(), Some(&person_id));
    assert_eq!(spk2.id_ref(), &speaker_id);
    assert_eq!(seg1b.speaker_id_ref(), Some(&speaker_id));
    assert_eq!(seg2b.speaker_id_ref(), Some(&speaker_id));
    assert_eq!(seg1b.audio_track_id_ref(), &track_id);
    assert_eq!(seg2b.audio_track_id_ref(), &track_id);
  }
}
