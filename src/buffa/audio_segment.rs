//! Wire ⇄ domain conversions for [`AudioSegment`] (and its nested
//! [`Word`]).
//!
//! Locked `schema/audio_segments.md`. One reconciled `dia ⋈ asry`
//! speak range of an `AudioTrack`. Decoding goes through
//! [`AudioSegment::try_new`] + the (`try_`)`with_*` builder chain — the
//! same path live application code uses — so the wire-side ordering of
//! field application matches the domain's invariant model:
//!
//! 1. `try_new(id, parent, index, span)` enforces nil-id / nil-parent /
//!    inverted-span rejection.
//! 2. `try_with_words(...)` enforces semantic `word.span ⊆ seg.span`
//!    containment + non-inverted word span.
//! 3. `try_with_no_speech_prob(...)` enforces `[0,1]`-finite.
//! 4. The remaining `with_*` setters are infallible.
//!
//! ## Field correspondence — [`Word`]
//!
//! | wire field            | domain field            | notes                                          |
//! | --------------------- | ----------------------- | ---------------------------------------------- |
//! | `text: String`        | `text: SmolStr`         | `""` = absent (locked convention)              |
//! | `span: TimeRange`     | `span: TimeRange`       | extern via `::mediatime`                       |
//! | `score: float`        | `score: f32`            | validating (`[0,1]`-finite)                    |
//! | `language: optional Language` | `language: Option<Language>` | unset ⇒ `None`; parse via `mediaframe::lang::Language::from_bcp47` |
//!
//! ## Field correspondence — [`AudioSegment`]
//!
//! | wire field            | domain field             | notes                                          |
//! | --------------------- | ------------------------ | ---------------------------------------------- |
//! | `id` (Bytes, 16)      | `id` (Uuid7)             | validating                                     |
//! | `parent` (Bytes, 16)  | `parent` (Uuid7)         | validating; `AudioTrack.id` FK                 |
//! | `index: uint32`       | `index: u32`             | 0-based ordinal                                |
//! | `span: TimeRange`     | `span: TimeRange`        | extern via `::mediatime`                       |
//! | `speaker: optional bytes` | `speaker: Option<Uuid7>` | 16-byte FK → `Speaker.id`; absent ⇒ `None` |
//! | `text: LocalizedText` | `text: LocalizedText`    | empty-strings round-trip as the empty VO       |
//! | `language: optional Language` | `language: Option<Language>` | unset ⇒ `None`                       |
//! | `words: repeated Word`| `words: Vec<Word>`       | validating (word-span containment)             |
//! | `no_speech_prob: optional float` | `no_speech_prob: Option<f32>` | validating (`[0,1]`-finite)         |
//! | `avg_logprob: optional float`    | `avg_logprob: Option<f32>`    | unset ⇒ `None`                    |
//! | `temperature: optional float`    | `temperature: Option<f32>`    | unset ⇒ `None`                    |
//! | `voice_fingerprint: optional VoiceFingerprint` | `voice_fingerprint: Option<VoiceFingerprint<Uuid7>>` | shared helper |

use ::buffa::bytes::Bytes;
use mediaframe::lang::Language;
use smol_str::SmolStr;

use crate::{
  buffa::{
    error::BuffaError,
    voice_fingerprint::{voice_fingerprint_from_wire, voice_fingerprint_to_wire},
  },
  domain::{
    aggregates::audio::segment::{AudioSegment, Word},
    vo::LocalizedText,
    Uuid7,
  },
  generated::media::v1 as wire,
};

// ---------------------------------------------------------------------------
// LocalizedText ⇄ wire::LocalizedText — shared helpers
// ---------------------------------------------------------------------------
//
// `LocalizedText` only appears as a singular message field on
// `AudioSegment.text` today, so the helpers stay scoped to this module
// (mirroring how `voice_fingerprint_*` were factored out only once a
// second parent appeared).

impl From<&LocalizedText> for wire::LocalizedText {
  fn from(d: &LocalizedText) -> Self {
    wire::LocalizedText {
      src: d.src().to_owned(),
      translated: d.translated().to_owned(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl From<&wire::LocalizedText> for LocalizedText {
  fn from(w: &wire::LocalizedText) -> Self {
    LocalizedText::from_src_translated(
      SmolStr::from(w.src.as_str()),
      SmolStr::from(w.translated.as_str()),
    )
  }
}

/// Decode a singular `MessageField<wire::LocalizedText>` slot. An unset
/// slot decodes to the all-empty `LocalizedText` (the "not yet emitted"
/// state).
fn localized_text_from_wire(w: &::buffa::MessageField<wire::LocalizedText>) -> LocalizedText {
  match w.as_option() {
    Some(v) => LocalizedText::from(v),
    None => LocalizedText::new(),
  }
}

// ---------------------------------------------------------------------------
// Language ⇄ wire::Language — shared helpers
// ---------------------------------------------------------------------------
//
// Wire shape: `message Language { string bcp47 = 1; }`. Empty / "und"
// decode to the undetermined language. Malformed tags surface as
// `BuffaError::LanguageMalformed`.

impl From<&Language> for wire::Language {
  fn from(d: &Language) -> Self {
    wire::Language {
      bcp47: d.to_bcp47(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl TryFrom<&wire::Language> for Language {
  type Error = BuffaError;

  fn try_from(w: &wire::Language) -> Result<Self, Self::Error> {
    if w.bcp47.is_empty() {
      return Ok(Language::new());
    }
    Language::from_bcp47(w.bcp47.as_str())
      .map_err(|_| BuffaError::LanguageMalformed(SmolStr::from(w.bcp47.as_str())))
  }
}

/// Encode `Option<Language>` into a parent message's
/// `MessageField<wire::Language>` slot.
fn language_to_wire(v: Option<Language>) -> ::buffa::MessageField<wire::Language> {
  match v {
    Some(v) => ::buffa::MessageField::some(wire::Language::from(&v)),
    None => ::buffa::MessageField::none(),
  }
}

/// Decode a parent message's `MessageField<wire::Language>` slot into an
/// `Option<Language>`. An unset slot ⇒ `None`.
fn language_from_wire(
  w: &::buffa::MessageField<wire::Language>,
) -> Result<Option<Language>, BuffaError> {
  match w.as_option() {
    Some(v) => Language::try_from(v).map(Some),
    None => Ok(None),
  }
}

// ---------------------------------------------------------------------------
// Word ⇄ wire::Word
// ---------------------------------------------------------------------------

impl From<&Word> for wire::Word {
  fn from(d: &Word) -> Self {
    wire::Word {
      text: d.text().to_owned(),
      span: ::buffa::MessageField::some(d.span_ref().clone()),
      score: d.score(),
      language: language_to_wire(d.language()),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl TryFrom<&wire::Word> for Word {
  type Error = BuffaError;

  fn try_from(w: &wire::Word) -> Result<Self, Self::Error> {
    let span = w
      .span
      .as_option()
      .cloned()
      .ok_or(BuffaError::MissingRequiredField("Word.span"))?;
    let language = language_from_wire(&w.language)?;
    // The wire bytes were produced by a previously-validated `Word`, so
    // the score is already finite-`[0,1]`. `try_from_parts` re-validates
    // cheaply (one float check) and is the public reconstruction door.
    Word::try_from_parts(SmolStr::from(w.text.as_str()), span, w.score, language).map_err(|_| {
      // Domain rejection of a re-decoded score implies wire-tampering
      // (the bytes weren't produced by a valid `try_new`). We surface
      // it as a missing-required-field — there's no dedicated variant
      // and adding one for a programmer-error case is overkill.
      BuffaError::MissingRequiredField("Word.score")
    })
  }
}

// ---------------------------------------------------------------------------
// AudioSegment<Uuid7> ⇄ wire::AudioSegment
// ---------------------------------------------------------------------------

impl From<&AudioSegment<Uuid7>> for wire::AudioSegment {
  fn from(d: &AudioSegment<Uuid7>) -> Self {
    wire::AudioSegment {
      id: Bytes::copy_from_slice(d.id_ref().as_bytes()),
      parent: Bytes::copy_from_slice(d.parent_ref().as_bytes()),
      index: d.index(),
      span: ::buffa::MessageField::some(d.span_ref().clone()),
      speaker: d
        .speaker_ref()
        .map(|id| Bytes::copy_from_slice(id.as_bytes())),
      text: ::buffa::MessageField::some(wire::LocalizedText::from(d.text_ref())),
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
    let parent = id_from_bytes(&w.parent)?;
    let span = w
      .span
      .as_option()
      .cloned()
      .ok_or(BuffaError::MissingRequiredField("AudioSegment.span"))?;
    let speaker = match &w.speaker {
      Some(b) => Some(id_from_bytes(b)?),
      None => None,
    };
    let text = localized_text_from_wire(&w.text);
    let language = language_from_wire(&w.language)?;
    let words: std::vec::Vec<Word> = w
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
      .with_speaker(speaker)
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
    E::NilParent => BuffaError::MissingRequiredField("AudioSegment.parent"),
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
  use crate::domain::vo::{Provenance, VoiceFingerprint};
  use core::num::NonZeroU32;
  use jiff::Timestamp as JiffTimestamp;
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

  // ---- LocalizedText ---------------------------------------------------------

  #[test]
  fn localized_text_roundtrip_both_populated() {
    let d = LocalizedText::from_src_translated("hola", "hello");
    let w: wire::LocalizedText = (&d).into();
    let d2: LocalizedText = (&w).into();
    assert_eq!(d, d2);
  }

  #[test]
  fn localized_text_roundtrip_empty() {
    let d = LocalizedText::new();
    let w: wire::LocalizedText = (&d).into();
    let d2: LocalizedText = (&w).into();
    assert_eq!(d, d2);
    assert!(d2.is_empty());
  }

  // ---- Language --------------------------------------------------------------

  #[test]
  fn language_roundtrip_concrete_tag() {
    let d = Language::from_bcp47("zh-Hant-TW").unwrap();
    let w: wire::Language = (&d).into();
    assert_eq!(w.bcp47, "zh-Hant-TW");
    let d2 = Language::try_from(&w).unwrap();
    assert_eq!(d, d2);
  }

  #[test]
  fn language_empty_bcp47_decodes_as_undetermined() {
    let w = wire::Language {
      bcp47: String::new(),
      __buffa_unknown_fields: Default::default(),
    };
    let d = Language::try_from(&w).unwrap();
    assert!(d.is_undetermined());
  }

  #[test]
  fn language_und_roundtrip() {
    let d = Language::from_bcp47("und").unwrap();
    let w: wire::Language = (&d).into();
    let d2 = Language::try_from(&w).unwrap();
    assert_eq!(d, d2);
    assert!(d2.is_undetermined());
  }

  #[test]
  fn language_malformed_bcp47_errors() {
    let w = wire::Language {
      bcp47: String::from("xx-yy-zz-bogus"),
      __buffa_unknown_fields: Default::default(),
    };
    let err = Language::try_from(&w).unwrap_err();
    assert!(matches!(err, BuffaError::LanguageMalformed(ref s) if s == "xx-yy-zz-bogus"));
  }

  // ---- Word ------------------------------------------------------------------

  #[test]
  fn word_roundtrip_without_language() {
    let w = Word::try_new("hi", span(0, 100), 0.9).unwrap();
    let wire: wire::Word = (&w).into();
    assert!(wire.language.as_option().is_none());
    let w2 = Word::try_from(&wire).unwrap();
    assert_eq!(w, w2);
  }

  #[test]
  fn word_roundtrip_with_language() {
    let fr = Language::from_bcp47("fr").unwrap();
    let w = Word::try_from_parts("bon", span(0, 200), 0.95, Some(fr)).unwrap();
    let wire: wire::Word = (&w).into();
    let w2 = Word::try_from(&wire).unwrap();
    assert_eq!(w, w2);
  }

  #[test]
  fn word_missing_span_errors() {
    let mut wire = wire::Word::from(&Word::try_new("x", span(0, 100), 0.5).unwrap());
    wire.span = ::buffa::MessageField::none();
    let err = Word::try_from(&wire).unwrap_err();
    assert!(err.is_missing_required_field());
  }

  // ---- AudioSegment ----------------------------------------------------------

  #[test]
  fn audio_segment_minimal_roundtrip() {
    let parent = Uuid7::new();
    let d = AudioSegment::try_new(Uuid7::new(), parent, 0, span(0, 1500)).unwrap();
    let w: wire::AudioSegment = (&d).into();
    let d2 = AudioSegment::try_from(&w).unwrap();
    assert_eq!(d, d2);
    assert_eq!(d2.parent_ref(), &parent);
    assert!(d2.speaker_ref().is_none());
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
      .with_speaker(Some(speaker))
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
      .with_speaker(Some(Uuid7::new()));
    let mut w: wire::AudioSegment = (&d).into();
    w.speaker = Some(Bytes::copy_from_slice(&[0u8; 7]));
    let err = AudioSegment::try_from(&w).unwrap_err();
    assert!(err.is_id_wrong_length());
  }
}
