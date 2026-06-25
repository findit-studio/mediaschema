//! Wire ⇄ domain conversions for [`AudioSegment`] (and its nested
//! [`Word`]).
//!
//! Locked `schema/audio_segments.md`. One reconciled `dia ⋈ asry`
//! speak range of an `AudioTrack`. Decoding goes through
//! [`AudioSegment::try_new`] + the (`try_`)`with_*` builder chain — the
//! same path live application code uses — so the wire-side ordering of
//! field application matches the domain's invariant model:
//!
//! 1. `try_new(id, audio_track_id, index, span)` enforces nil-id / nil-parent /
//!    inverted-span rejection.
//! 2. `try_with_words(...)` enforces semantic `word.span ⊆ seg.span`
//!    containment + non-inverted word span.
//! 3. `try_with_no_speech_prob(...)` enforces `[0,1]`-finite.
//! 4. The remaining `with_*` setters are infallible.
//!
//! The [`Word`] / `LocalizedText` / `Language` conversions this module
//! decodes through live in the shared [`vo`](super::vo) module (promoted
//! there once `media.v2` became a second embedding parent); see its
//! field-correspondence tables.
//!
//! ## Field correspondence — [`AudioSegment`]
//!
//! | wire field            | domain field             | notes                                          |
//! | --------------------- | ------------------------ | ---------------------------------------------- |
//! | `id` (Bytes, 16)      | `id` (Uuid7)             | validating                                     |
//! | `audio_track_id` (Bytes, 16) | `audio_track_id` (Uuid7)         | validating; `AudioTrack.id` FK                 |
//! | `index: uint32`       | `index: u32`             | 0-based ordinal                                |
//! | `span: TimeRange`     | `span: TimeRange`        | extern via `::mediatime`                       |
//! | `speaker_id: optional bytes` | `speaker_id: Option<Uuid7>` | 16-byte FK → `Speaker.id`; absent ⇒ `None` |
//! | `text: LocalizedText` | `text: LocalizedText`    | empty-strings round-trip as the empty VO       |
//! | `language: optional Language` | `language: Option<Language>` | unset ⇒ `None`                       |
//! | `words: repeated Word`| `words: Vec<Word>`       | validating (word-span containment)             |
//! | `no_speech_prob: optional float` | `no_speech_prob: Option<f32>` | validating (`[0,1]`-finite)         |
//! | `avg_logprob: optional float`    | `avg_logprob: Option<f32>`    | unset ⇒ `None`                    |
//! | `temperature: optional float`    | `temperature: Option<f32>`    | unset ⇒ `None`                    |
//! | `voice_fingerprint: optional VoiceFingerprint` | `voice_fingerprint: Option<VoiceFingerprint<Uuid7>>` | shared helper |

use std::vec::Vec;

use ::buffa::bytes::Bytes;

use crate::{
  buffa::{
    error::BuffaError,
    vo::{language_from_wire, language_to_wire, localized_text_from_wire, localized_text_to_wire},
    voice_fingerprint::{voice_fingerprint_from_wire, voice_fingerprint_to_wire},
  },
  domain::{
    aggregates::audio::segment::{AudioSegment, Word},
    Uuid7,
  },
  generated::media::v1 as wire,
};

// ---------------------------------------------------------------------------
// AudioSegment<Uuid7> ⇄ wire::AudioSegment
// ---------------------------------------------------------------------------

impl From<&AudioSegment<Uuid7>> for wire::AudioSegment {
  fn from(d: &AudioSegment<Uuid7>) -> Self {
    wire::AudioSegment {
      id: Bytes::copy_from_slice(d.id_ref().as_bytes()),
      audio_track_id: Bytes::copy_from_slice(d.audio_track_id_ref().as_bytes()),
      index: d.index(),
      span: ::buffa::MessageField::some(*d.span_ref()),
      speaker_id: d
        .speaker_id_ref()
        .map(|id| Bytes::copy_from_slice(id.as_bytes())),
      // `text` is presence-bearing and decodes unset ⇒ empty
      // `LocalizedText` (via `localized_text_from_wire`), so an empty
      // domain text encodes to `none` — `some(<empty>)` would force a
      // present empty field and break the empty-as-absent invariant.
      text: localized_text_to_wire(d.text_ref()),
      language: language_to_wire(d.language()),
      words: d.words_slice().iter().map(wire::Word::from).collect(),
      no_speech_prob: d.no_speech_prob(),
      avg_logprob: d.avg_logprob(),
      temperature: d.temperature(),
      voice_fingerprint: voice_fingerprint_to_wire(d.voice_fingerprint_ref()),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl TryFrom<&wire::AudioSegment> for AudioSegment<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::AudioSegment) -> Result<Self, Self::Error> {
    let id = id_from_bytes(&w.id)?;
    let parent = id_from_bytes(&w.audio_track_id)?;
    let span = w
      .span
      .as_option()
      .cloned()
      .ok_or(BuffaError::MissingRequiredField("AudioSegment.span"))?;
    let speaker = match &w.speaker_id {
      Some(b) => Some(id_from_bytes(b)?),
      None => None,
    };
    let text = localized_text_from_wire(&w.text);
    let language = language_from_wire(&w.language)?;
    let words: Vec<Word> = w
      .words
      .iter()
      .map(Word::try_from)
      .collect::<Result<_, _>>()?;
    let voice_fingerprint = voice_fingerprint_from_wire(&w.voice_fingerprint)?;

    // No `from_parts` exists for `AudioSegment` — and none is needed:
    // `try_new` + the (`try_`)`with_*` builder chain is the same public
    // reconstruction surface app code uses, and the only invariant
    // re-checks are the same ones a freshly-built segment must pass.
    let mut seg = AudioSegment::try_new(id, parent, w.index, span)
      .map_err(audio_segment_error_as_buffa)?
      .with_speaker_id(speaker)
      .with_text(text)
      .with_language(language)
      .with_avg_logprob(w.avg_logprob)
      .with_temperature(w.temperature)
      .with_voice_fingerprint(voice_fingerprint);

    if !words.is_empty() {
      seg = seg
        .try_with_words(words)
        .map_err(audio_segment_error_as_buffa)?;
    }
    if w.no_speech_prob.is_some() {
      seg = seg
        .try_with_no_speech_prob(w.no_speech_prob)
        .map_err(audio_segment_error_as_buffa)?;
    }
    Ok(seg)
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

/// Re-decode of a previously-valid domain `AudioSegment` should always
/// succeed; a rejection therefore implies wire-tampering. Surface those
/// as `MissingRequiredField` — there's no dedicated `AudioSegmentError`
/// variant on `BuffaError`, and the existing GPS / Language cases use
/// the same "tampered wire ⇒ generic surface" strategy.
fn audio_segment_error_as_buffa(
  e: crate::domain::aggregates::audio::segment::AudioSegmentError,
) -> BuffaError {
  use crate::domain::aggregates::audio::segment::AudioSegmentError as E;
  match e {
    E::NilId => BuffaError::MissingRequiredField("AudioSegment.id"),
    E::NilAudioTrackId => BuffaError::MissingRequiredField("AudioSegment.audio_track_id"),
    E::InvertedSpan => BuffaError::MissingRequiredField("AudioSegment.span"),
    E::WordSpanOutOfSegment | E::InvertedWordSpan => {
      BuffaError::MissingRequiredField("AudioSegment.words")
    }
    E::NoSpeechProbOutOfRange => BuffaError::MissingRequiredField("AudioSegment.no_speech_prob"),
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use crate::domain::vo::{LocalizedText, Provenance, VoiceFingerprint};
  use core::num::NonZeroU32;
  use jiff::Timestamp as JiffTimestamp;
  use mediaframe::lang::Language;
  use mediatime::{TimeRange, Timebase};

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).expect("nonzero"))
  }

  fn span(start: i64, end: i64) -> TimeRange {
    TimeRange::new(start, end, tb())
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

  // ---- AudioSegment ----------------------------------------------------------

  #[test]
  fn audio_segment_minimal_roundtrip() {
    let parent = Uuid7::new();
    let d = AudioSegment::try_new(Uuid7::new(), parent, 0, span(0, 1500)).unwrap();
    let w: wire::AudioSegment = (&d).into();
    let d2 = AudioSegment::try_from(&w).unwrap();
    assert_eq!(d, d2);
    assert_eq!(d2.audio_track_id_ref(), &parent);
    assert!(d2.speaker_id_ref().is_none());
    assert!(d2.voice_fingerprint_ref().is_none());
    assert!(d2.words_slice().is_empty());
  }

  #[test]
  fn audio_segment_full_roundtrip_with_words_and_fingerprint() {
    let speaker = Uuid7::new();
    let es = Language::from_bcp47("es").unwrap();
    let w1 = Word::try_new("hola", span(0, 200), 0.95)
      .unwrap()
      .with_language(Some(es));
    let w2 = Word::try_new("mundo", span(200, 400), 0.93).unwrap();
    let d = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 400))
      .unwrap()
      .with_speaker_id(Some(speaker))
      .with_text(LocalizedText::from_src_translated(
        "hola mundo",
        "hello world",
      ))
      .with_language(Some(es))
      .try_with_words(std::vec![w1, w2])
      .unwrap()
      .try_with_no_speech_prob(Some(0.05))
      .unwrap()
      .with_avg_logprob(Some(-0.4))
      .with_temperature(Some(0.0))
      .with_voice_fingerprint(Some(vfp()));
    let w: wire::AudioSegment = (&d).into();
    let d2 = AudioSegment::try_from(&w).unwrap();
    assert_eq!(d, d2);
  }

  #[test]
  fn audio_segment_voice_fingerprint_absent_roundtrip() {
    let d = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 500)).unwrap();
    let w: wire::AudioSegment = (&d).into();
    assert!(w.voice_fingerprint.as_option().is_none());
    let d2 = AudioSegment::try_from(&w).unwrap();
    assert!(d2.voice_fingerprint_ref().is_none());
  }

  #[test]
  fn audio_segment_missing_span_errors() {
    let d = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 500)).unwrap();
    let mut w: wire::AudioSegment = (&d).into();
    w.span = ::buffa::MessageField::none();
    let err = AudioSegment::try_from(&w).unwrap_err();
    assert!(err.is_missing_required_field());
  }

  #[test]
  fn audio_segment_wrong_length_id_errors() {
    let d = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 500)).unwrap();
    let mut w: wire::AudioSegment = (&d).into();
    w.id = Bytes::copy_from_slice(&[0u8; 8]);
    let err = AudioSegment::try_from(&w).unwrap_err();
    assert!(err.is_id_wrong_length());
  }

  #[test]
  fn audio_segment_nil_id_errors() {
    let d = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 500)).unwrap();
    let mut w: wire::AudioSegment = (&d).into();
    w.id = Bytes::copy_from_slice(&[0u8; 16]);
    let err = AudioSegment::try_from(&w).unwrap_err();
    assert!(err.is_id_invalid());
  }

  #[test]
  fn audio_segment_wrong_length_speaker_fk_errors() {
    let d = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 500))
      .unwrap()
      .with_speaker_id(Some(Uuid7::new()));
    let mut w: wire::AudioSegment = (&d).into();
    w.speaker_id = Some(Bytes::copy_from_slice(&[0u8; 7]));
    let err = AudioSegment::try_from(&w).unwrap_err();
    assert!(err.is_id_wrong_length());
  }
}
