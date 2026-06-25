//! Shared wire ⇄ domain helpers for the cross-cutting VOs that more than
//! one parent embeds: [`LocalizedText`], the wire `Language` message and
//! [`Word`].
//!
//! These conversions started life module-private in
//! [`audio_segment`](super::audio_segment) (the only parent at the time
//! was the audio cluster) with the explicit note that they would be
//! promoted "the next time a non-audio parent embeds one". The `media.v2`
//! graph surface is that parent — `media.v2.SubtitleCue.text` embeds
//! `media.v1.LocalizedText` and `media.v2.AudioSegment.words` embeds
//! `media.v1.Word` — so the helpers now live here, `pub(super)` for every
//! bridge under [`crate::buffa`]. Behaviour is unchanged from the v1
//! originals.
//!
//! The sibling [`voice_fingerprint`](super::voice_fingerprint) module
//! plays the same role for `VoiceFingerprint` + `Provenance` and is
//! reused as-is.
//!
//! ## Field correspondence — [`LocalizedText`]
//!
//! | wire field            | domain field        | notes                              |
//! | --------------------- | ------------------- | ---------------------------------- |
//! | `src: String`         | `src: SmolStr`      | `""` = absent (locked convention)  |
//! | `translated: String`  | `translated: SmolStr` | `""` = absent                    |
//!
//! ## Field correspondence — wire `Language`
//!
//! | wire field        | domain type         | notes                                              |
//! | ----------------- | ------------------- | -------------------------------------------------- |
//! | `bcp47: String`   | [`Language`]        | `""`/`"und"` ⇒ undetermined; malformed ⇒ [`BuffaError::LanguageMalformed`] |
//!
//! ## Field correspondence — [`Word`] (`feature = "audio"`)
//!
//! | wire field            | domain field            | notes                                          |
//! | --------------------- | ----------------------- | ---------------------------------------------- |
//! | `text: String`        | `text: SmolStr`         | `""` = absent (locked convention)              |
//! | `span: TimeRange`     | `span: TimeRange`       | extern via `::mediatime`; required             |
//! | `score: float`        | `score: f32`            | validating (`[0,1]`-finite)                    |
//! | `language: optional Language` | `language: Option<Language>` | unset ⇒ `None`                    |

use buffa::MessageField;
use mediaframe::lang::Language;
use smol_str::SmolStr;

#[cfg(feature = "audio")]
use crate::domain::aggregates::audio::segment::Word;
use crate::{
  buffa::error::BuffaError,
  domain::vo::{LocalizedText, Platform},
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
// LocalizedText ⇄ wire::LocalizedText
// ---------------------------------------------------------------------------

impl From<&LocalizedText> for wire::LocalizedText {
  fn from(d: &LocalizedText) -> Self {
    wire::LocalizedText {
      src: d.src().to_owned().into(),
      translated: d.translated().to_owned().into(),
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
pub(super) fn localized_text_from_wire(w: &MessageField<wire::LocalizedText>) -> LocalizedText {
  match w.as_option() {
    Some(v) => LocalizedText::from(v),
    None => LocalizedText::new(),
  }
}

// ---------------------------------------------------------------------------
// Platform ⇄ wire::Platform — 3 SmolStr <-> 3 SmolStr
// ---------------------------------------------------------------------------

impl From<&Platform> for wire::Platform {
  fn from(d: &Platform) -> Self {
    wire::Platform {
      os: d.os().to_owned().into(),
      arch: d.arch().to_owned().into(),
      os_version: d.os_version().to_owned().into(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl From<&wire::Platform> for Platform {
  fn from(w: &wire::Platform) -> Self {
    Platform::from_parts(
      SmolStr::from(w.os.as_str()),
      SmolStr::from(w.arch.as_str()),
      SmolStr::from(w.os_version.as_str()),
    )
  }
}

/// Decode a singular `MessageField<wire::Platform>` slot. An unset slot
/// decodes to the all-empty `Platform` (the "not recorded" state).
pub(super) fn platform_from_wire(w: &MessageField<wire::Platform>) -> Platform {
  match w.as_option() {
    Some(v) => Platform::from(v),
    None => Platform::new(),
  }
}

// ---------------------------------------------------------------------------
// Language ⇄ wire::Language
// ---------------------------------------------------------------------------
//
// Wire shape: `message Language { string bcp47 = 1; }`. Empty / "und"
// decode to the undetermined language. Malformed tags surface as
// `BuffaError::LanguageMalformed`.

impl From<&Language> for wire::Language {
  fn from(d: &Language) -> Self {
    wire::Language {
      bcp47: d.to_bcp47().into(),
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
pub(super) fn language_to_wire(v: Option<Language>) -> MessageField<wire::Language> {
  match v {
    Some(v) => MessageField::some(wire::Language::from(&v)),
    None => MessageField::none(),
  }
}

/// Decode a parent message's `MessageField<wire::Language>` slot into an
/// `Option<Language>`. An unset slot ⇒ `None`.
pub(super) fn language_from_wire(
  w: &MessageField<wire::Language>,
) -> Result<Option<Language>, BuffaError> {
  match w.as_option() {
    Some(v) => Language::try_from(v).map(Some),
    None => Ok(None),
  }
}

// ---------------------------------------------------------------------------
// Word ⇄ wire::Word (`feature = "audio"` — the domain Word lives in the
// audio aggregate tree)
// ---------------------------------------------------------------------------

#[cfg(feature = "audio")]
#[cfg_attr(docsrs, doc(cfg(feature = "audio")))]
impl From<&Word> for wire::Word {
  fn from(d: &Word) -> Self {
    wire::Word {
      text: d.text().to_owned().into(),
      span: MessageField::some(*d.span_ref()),
      score: d.score(),
      language: language_to_wire(d.language()),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

#[cfg(feature = "audio")]
#[cfg_attr(docsrs, doc(cfg(feature = "audio")))]
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
    Word::try_from_parts(SmolStr::from(w.text.as_str()), span, w.score, language)
      .map_err(|e| BuffaError::DomainConstructorRejected(SmolStr::from(e.to_string())))
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

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
      bcp47: SmolStr::default(),
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
      bcp47: SmolStr::from("xx-yy-zz-bogus"),
      __buffa_unknown_fields: Default::default(),
    };
    let err = Language::try_from(&w).unwrap_err();
    assert!(matches!(err, BuffaError::LanguageMalformed(ref s) if s == "xx-yy-zz-bogus"));
  }

  // ---- Platform --------------------------------------------------------------

  #[test]
  fn platform_roundtrip_populated() {
    use crate::domain::vo::Platform;
    let d = Platform::from_parts("macos", "aarch64", "15.5");
    let w: wire::Platform = (&d).into();
    let d2: Platform = (&w).into();
    assert_eq!(d, d2);
  }

  #[test]
  fn platform_roundtrip_empty() {
    use crate::domain::vo::Platform;
    let d = Platform::new();
    let w: wire::Platform = (&d).into();
    let d2: Platform = (&w).into();
    assert_eq!(d, d2);
    assert!(d2.is_empty());
  }

  // ---- Word ------------------------------------------------------------------

  #[cfg(feature = "audio")]
  mod word {
    use core::num::NonZeroU32;

    use mediatime::{TimeRange, Timebase};

    use super::*;

    fn span(start: i64, end: i64) -> TimeRange {
      TimeRange::new(
        start,
        end,
        Timebase::new(1, NonZeroU32::new(1000).expect("nonzero")),
      )
    }

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
      wire.span = MessageField::none();
      let err = Word::try_from(&wire).unwrap_err();
      assert!(err.is_missing_required_field());
    }
  }
}
