//! `AudioSegment` — diarization + transcript segment.
//!
//! Locked `schema/audio_segments.md` rev 3. The audio analog of `Scene`:
//! one analysis segment of an `AudioTrack`, the reconciled join of `dia`
//! speaker diarization (who) ⋈ `asry` ASR (what), as one timeline span.
//! Parent → `AudioTrack.id` (A-loc per-track; multi-track files keep
//! which-track attribution). No progress lifecycle; truth = id list +
//! `Audio.total_segments` rollup.
//!
//! `Provenance` is per-track (on `AudioTrack`), not per segment.

use derive_more::IsVariant;
use mediaframe::lang::Language;
use mediatime::TimeRange;
use smol_str::SmolStr;

use crate::domain::{vo::LocalizedText, Uuid7};

// ---------------------------------------------------------------------------
// Score validation — shared by `Word`'s validating ctors / mutators
// ---------------------------------------------------------------------------

/// A `[0,1]`-bounded probability/confidence score is valid iff it is finite
/// (no NaN / ±∞) and within the closed unit interval. `f32::is_finite`
/// already excludes NaN and infinities.
#[inline]
const fn is_valid_score(score: f32) -> bool {
  score.is_finite() && score >= 0.0 && score <= 1.0
}

// ---------------------------------------------------------------------------
// Word — nested VO
// ---------------------------------------------------------------------------

/// Word-level timing + score (`asry` output; producer-agnostic).
///
/// Locked `audio_segments.md` §Nested value-objects. `score` ∈ `[0,1]`,
/// **always present** (NaN-free per locked spec). Per-word `language`
/// (BCP-47) carries code-switch / multilingual cues.
///
/// **No infallible constructor** — `score` carries the `[0,1]`-finite
/// invariant, so construction goes through the validating
/// [`Word::try_new`] / [`Word::try_from_parts`]; `with_score` /
/// `set_score` are likewise fallible (`try_*`) so the invariant cannot be
/// broken post-construction.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
  text: SmolStr,
  span: TimeRange,
  score: f32,
  language: Option<Language>,
}

impl Word {
  /// Validating constructor from `(text, span, score)`; `language` starts
  /// `None` (use [`Word::with_language`] to set).
  ///
  /// Rejects a `score` that is non-finite (NaN / ±∞) or outside `[0,1]`
  /// (the locked per-word score invariant).
  #[inline]
  pub fn try_new(text: impl Into<SmolStr>, span: TimeRange, score: f32) -> Result<Self, WordError> {
    Self::try_from_parts(text, span, score, None)
  }

  /// Validating constructor with all four fields. Rejects a `score` that
  /// is non-finite (NaN / ±∞) or outside `[0,1]`.
  #[inline]
  pub fn try_from_parts(
    text: impl Into<SmolStr>,
    span: TimeRange,
    score: f32,
    language: Option<Language>,
  ) -> Result<Self, WordError> {
    if !is_valid_score(score) {
      return Err(WordError::ScoreOutOfRange);
    }
    Ok(Self {
      text: text.into(),
      span,
      score,
      language,
    })
  }

  /// Word text token.
  #[inline]
  pub fn text(&self) -> &str {
    self.text.as_str()
  }

  /// Time span (media-time).
  #[inline]
  pub const fn span(&self) -> &TimeRange {
    &self.span
  }

  /// `[0,1]` score; always present (NaN-free per locked spec).
  #[inline]
  pub const fn score(&self) -> f32 {
    self.score
  }

  /// Per-word BCP-47 language tag (`None` = inherits segment).
  #[inline]
  pub const fn language(&self) -> Option<Language> {
    self.language
  }

  /// Builder: replace `text`.
  #[inline]
  pub fn with_text(mut self, v: impl Into<SmolStr>) -> Self {
    self.text = v.into();
    self
  }

  /// Builder: replace `span`.
  #[inline]
  pub fn with_span(mut self, span: TimeRange) -> Self {
    self.span = span;
    self
  }

  /// Validating builder: replace `score`. Rejects a non-finite or
  /// out-of-`[0,1]` value (keeps the locked score invariant).
  ///
  /// Not `const` — the error path drops `self`, which is not permitted in
  /// a `const fn`.
  #[inline]
  pub fn try_with_score(mut self, score: f32) -> Result<Self, WordError> {
    if !is_valid_score(score) {
      return Err(WordError::ScoreOutOfRange);
    }
    self.score = score;
    Ok(self)
  }

  /// Validating in-place mutator for `score`. Rejects a non-finite or
  /// out-of-`[0,1]` value; on rejection `self` is left unchanged.
  #[inline]
  pub const fn try_set_score(&mut self, score: f32) -> Result<&mut Self, WordError> {
    if !is_valid_score(score) {
      return Err(WordError::ScoreOutOfRange);
    }
    self.score = score;
    Ok(self)
  }

  /// Builder: replace `language`.
  #[inline]
  pub const fn with_language(mut self, v: Option<Language>) -> Self {
    self.language = v;
    self
  }
}

/// Error returned by [`Word`]'s validating constructors / mutators when a
/// `score` cannot uphold the locked finite-`[0,1]` invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum WordError {
  /// `score` was non-finite (NaN / ±∞) or outside the closed `[0,1]`
  /// interval.
  #[error("Word score must be finite and within [0, 1]")]
  ScoreOutOfRange,
}

// ---------------------------------------------------------------------------
// AudioSegment
// ---------------------------------------------------------------------------

/// One reconciled `dia ⋈ asry` analysis segment of an `AudioTrack`.
///
/// Generic over `Id` (default [`Uuid7`]). `parent` FK → `AudioTrack.id`
/// (A-loc per-track). `speaker` FK → `Speaker` (`None` = not diarized).
///
/// **No `Default`** — defaulting to nil identities would be an orphan
/// segment with no track + a zero-length span at `t=0`. Use
/// [`AudioSegment::try_new`].
#[derive(Debug, Clone, PartialEq)]
pub struct AudioSegment<Id = Uuid7> {
  id: Id,
  parent: Id,
  index: u32,
  span: TimeRange,
  speaker: Option<Id>,
  text: LocalizedText,
  language: Option<Language>,
  words: std::vec::Vec<Word>,
  no_speech_prob: Option<f32>,
  avg_logprob: Option<f32>,
  temperature: Option<f32>,
}

impl AudioSegment<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (every aggregate row needs a real identity), nil
  /// `parent` (orphan segment with no `AudioTrack`), and inverted
  /// `span.start > span.end` (locked invariant). All other fields start
  /// in their neutral state (`speaker = None`, empty `text`, no `words`,
  /// `None` quality signals).
  pub fn try_new(
    id: Uuid7,
    parent: Uuid7,
    index: u32,
    span: TimeRange,
  ) -> Result<Self, AudioSegmentError> {
    if id.is_nil() {
      return Err(AudioSegmentError::NilId);
    }
    if parent.is_nil() {
      return Err(AudioSegmentError::NilParent);
    }
    // Defence-in-depth: `mediatime::TimeRange::new` already enforces
    // `start <= end` (panicking) and `TimeRange::try_new` rejects the
    // inverted case. Compare on *semantic* time (`Timestamp::cmp_semantic`,
    // timebase-correct) rather than raw PTS so this stays valid even if a
    // future bypass / sentinel-tolerant constructor is introduced.
    if span.start().cmp_semantic(&span.end()) == core::cmp::Ordering::Greater {
      return Err(AudioSegmentError::InvertedSpan);
    }
    Ok(Self {
      id,
      parent,
      index,
      span,
      speaker: None,
      text: LocalizedText::new(),
      language: None,
      words: std::vec::Vec::new(),
      no_speech_prob: None,
      avg_logprob: None,
      temperature: None,
    })
  }

  /// Validating builder: replace the `speaker` FK.
  ///
  /// `None` is the real "not diarized" sentinel and is always accepted. A
  /// `Some(_)` must be a real `Speaker` identity — a `Some(Uuid7::nil())`
  /// is an invalid FK and is rejected with
  /// [`AudioSegmentError::NilSpeaker`]. The infallible mutator is omitted
  /// for the canonical `Uuid7` type so this check cannot be bypassed. On
  /// rejection `self` is returned unchanged inside the `Err`.
  #[inline]
  pub fn try_with_speaker(mut self, v: Option<Uuid7>) -> Result<Self, AudioSegmentError> {
    if matches!(v, Some(id) if id.is_nil()) {
      return Err(AudioSegmentError::NilSpeaker);
    }
    self.speaker = v;
    Ok(self)
  }

  /// Validating in-place mutator for the `speaker` FK. Rejects a
  /// `Some(Uuid7::nil())` ([`AudioSegmentError::NilSpeaker`]); `None` and a
  /// valid `Some` are accepted. On rejection `self` is left unchanged.
  ///
  /// Not `const` — `Uuid7::is_nil` is not a `const fn`.
  #[inline]
  pub fn try_set_speaker(&mut self, v: Option<Uuid7>) -> Result<&mut Self, AudioSegmentError> {
    if matches!(v, Some(id) if id.is_nil()) {
      return Err(AudioSegmentError::NilSpeaker);
    }
    self.speaker = v;
    Ok(self)
  }
}

impl<Id> AudioSegment<Id> {
  /// Canonical identity.
  #[inline]
  pub const fn id(&self) -> &Id {
    &self.id
  }

  /// FK → `AudioTrack.id`.
  #[inline]
  pub const fn parent(&self) -> &Id {
    &self.parent
  }

  /// 0-based segment ordinal within the parent track.
  #[inline]
  pub const fn index(&self) -> u32 {
    self.index
  }

  /// Segment time span (media-time).
  #[inline]
  pub const fn span(&self) -> &TimeRange {
    &self.span
  }

  /// FK → `Speaker` (`None` = not diarized; raw `dia` cluster lives on
  /// `Speaker.cluster_id`).
  #[inline]
  pub const fn speaker(&self) -> Option<&Id> {
    self.speaker.as_ref()
  }

  /// Transcript text (`src` = `asry` transcript, `translated` =
  /// whisper-translate; both `""` until emitted).
  #[inline]
  pub const fn text(&self) -> &LocalizedText {
    &self.text
  }

  /// Chunk language (`asry::Transcript.language`; BCP-47).
  #[inline]
  pub const fn language(&self) -> Option<Language> {
    self.language
  }

  /// Word-level timing + scores (`asry`). May be empty (= no word timing).
  #[inline]
  pub fn words(&self) -> &[Word] {
    self.words.as_slice()
  }

  /// Whisper "no speech" probability (replaces generic `confidence`).
  #[inline]
  pub const fn no_speech_prob(&self) -> Option<f32> {
    self.no_speech_prob
  }

  /// Whisper mean token logprob.
  #[inline]
  pub const fn avg_logprob(&self) -> Option<f32> {
    self.avg_logprob
  }

  /// Whisper final decode temperature (retry/quality signal).
  #[inline]
  pub const fn temperature(&self) -> Option<f32> {
    self.temperature
  }

  // ----- Internal validation -----------------------------------------------

  /// Returns `Err(WordSpanOutOfSegment)` if any word's `span` is not
  /// contained in `self.span`, and `Err(InvertedWordSpan)` if any word's
  /// `span` is itself inverted (`word.start > word.end`).
  ///
  /// Containment is checked on **semantic** time
  /// ([`mediatime::Timestamp::cmp_semantic`], timebase-correct) — raw PTS
  /// are timebase-relative, so a word and segment in different timebases
  /// would compare meaninglessly. A word is contained iff
  /// `word.start >= seg.start` *and* `word.end <= seg.end` semantically.
  fn check_words(&self, words: &[Word]) -> Result<(), AudioSegmentError> {
    use core::cmp::Ordering;
    let (seg_start, seg_end) = (self.span.start(), self.span.end());
    for w in words {
      let (w_start, w_end) = (w.span().start(), w.span().end());
      // Reject an inverted word range outright.
      if w_start.cmp_semantic(&w_end) == Ordering::Greater {
        return Err(AudioSegmentError::InvertedWordSpan);
      }
      if w_start.cmp_semantic(&seg_start) == Ordering::Less
        || w_end.cmp_semantic(&seg_end) == Ordering::Greater
      {
        return Err(AudioSegmentError::WordSpanOutOfSegment);
      }
    }
    Ok(())
  }

  // ----- Builders ----------------------------------------------------------

  /// Builder: replace `text`.
  #[inline]
  pub fn with_text(mut self, v: LocalizedText) -> Self {
    self.text = v;
    self
  }

  /// Builder: replace `language`.
  #[inline]
  pub const fn with_language(mut self, v: Option<Language>) -> Self {
    self.language = v;
    self
  }

  /// Validating builder: replace `words`.
  ///
  /// Rejects the update unless **every** word's `span` is contained in
  /// `self.span` (locked invariant: each `Word` span ⊆ the segment span)
  /// and is not itself inverted; containment is checked on semantic
  /// (timebase-correct) time, mirroring `try_new`'s span check. `Word`'s
  /// own ctors already guarantee finite-`[0,1]` scores, so no score
  /// re-validation is needed here. On rejection `self` is returned
  /// unchanged inside the `Err`.
  #[inline]
  pub fn try_with_words(
    mut self,
    v: impl Into<std::vec::Vec<Word>>,
  ) -> Result<Self, AudioSegmentError> {
    let words = v.into();
    if let Err(e) = self.check_words(&words) {
      return Err(e);
    }
    self.words = words;
    Ok(self)
  }

  /// Validating builder: replace `no_speech_prob`.
  ///
  /// Accepts `None` or a `Some` holding a finite value in `[0,1]` (it is a
  /// Whisper silence *probability*); rejects NaN / ±∞ / out-of-range,
  /// mirroring [`Word`]'s `score` treatment. On rejection `self` is
  /// returned unchanged inside the `Err`.
  ///
  /// Not `const` — the error path drops `self`, which is not permitted in
  /// a `const fn`.
  #[inline]
  pub fn try_with_no_speech_prob(mut self, v: Option<f32>) -> Result<Self, AudioSegmentError> {
    if matches!(v, Some(p) if !is_valid_score(p)) {
      return Err(AudioSegmentError::NoSpeechProbOutOfRange);
    }
    self.no_speech_prob = v;
    Ok(self)
  }

  /// Builder: replace `avg_logprob`.
  #[inline]
  pub const fn with_avg_logprob(mut self, v: Option<f32>) -> Self {
    self.avg_logprob = v;
    self
  }

  /// Builder: replace `temperature`.
  #[inline]
  pub const fn with_temperature(mut self, v: Option<f32>) -> Self {
    self.temperature = v;
    self
  }

  // ----- Setters -----------------------------------------------------------

  /// In-place mutator for `text`.
  #[inline]
  pub fn set_text(&mut self, v: LocalizedText) {
    self.text = v;
  }

  /// In-place mutator for `language`.
  #[inline]
  pub const fn set_language(&mut self, v: Option<Language>) {
    self.language = v;
  }

  /// Validating in-place mutator for `words`. Rejects the update unless
  /// every word's `span` is contained in `self.span`; on rejection `self`
  /// is left unchanged.
  #[inline]
  pub fn try_set_words(
    &mut self,
    v: impl Into<std::vec::Vec<Word>>,
  ) -> Result<&mut Self, AudioSegmentError> {
    let words = v.into();
    self.check_words(&words)?;
    self.words = words;
    Ok(self)
  }

  /// Validating in-place mutator for `no_speech_prob`. Accepts `None` or a
  /// `Some` holding a finite value in `[0,1]`; rejects NaN / ±∞ /
  /// out-of-range. On rejection `self` is left unchanged.
  #[inline]
  pub const fn try_set_no_speech_prob(
    &mut self,
    v: Option<f32>,
  ) -> Result<&mut Self, AudioSegmentError> {
    if let Some(p) = v {
      if !is_valid_score(p) {
        return Err(AudioSegmentError::NoSpeechProbOutOfRange);
      }
    }
    self.no_speech_prob = v;
    Ok(self)
  }

  /// In-place mutator for `avg_logprob`.
  #[inline]
  pub const fn set_avg_logprob(&mut self, v: Option<f32>) {
    self.avg_logprob = v;
  }

  /// In-place mutator for `temperature`.
  #[inline]
  pub const fn set_temperature(&mut self, v: Option<f32>) {
    self.temperature = v;
  }
}

/// Error returned by [`AudioSegment`]'s validating constructor and
/// word-list mutators when an invariant cannot be upheld.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum AudioSegmentError {
  /// Supplied `id` was the nil sentinel.
  #[error("AudioSegment id must not be the nil UUID")]
  NilId,
  /// Supplied `parent` was the nil sentinel — orphan segment with no
  /// `AudioTrack` reference.
  #[error("AudioSegment parent (AudioTrack) must not be the nil UUID")]
  NilParent,
  /// `span.start > span.end` — inverted segment span (locked invariant).
  #[error("AudioSegment span.start must be <= span.end")]
  InvertedSpan,
  /// A `Word` in the supplied word list has a `span` not contained in the
  /// segment's own `span` (locked invariant: each word span ⊆ segment
  /// span). Containment is evaluated on semantic (timebase-correct) time.
  #[error("AudioSegment word span must be contained in the segment span")]
  WordSpanOutOfSegment,
  /// A `Word` in the supplied word list has an inverted `span`
  /// (`word.start > word.end`).
  #[error("AudioSegment word span.start must be <= span.end")]
  InvertedWordSpan,
  /// `no_speech_prob` was a `Some` holding a non-finite (NaN / ±∞) or
  /// out-of-`[0,1]` value — it is a Whisper silence probability.
  #[error("AudioSegment no_speech_prob must be finite and within [0, 1]")]
  NoSpeechProbOutOfRange,
  /// The `speaker` FK was set to `Some(Uuid7::nil())` — the nil UUID is not
  /// a real `Speaker` identity; `None` is the "not diarized" sentinel.
  #[error("AudioSegment speaker FK must not be Some(nil UUID)")]
  NilSpeaker,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use core::num::NonZeroU32;
  use mediatime::Timebase;

  fn tb() -> Timebase {
    // A standard 1/1000 timebase (millisecond ticks) is enough for the
    // domain-level invariant tests; the exact timebase isn't material.
    Timebase::new(1, NonZeroU32::new(1000).expect("nonzero"))
  }

  fn span(start_ticks: i64, end_ticks: i64) -> TimeRange {
    // `TimeRange::new` panics on `end < start`; for the inverted-span
    // negative test we go through `TimeRange::try_new` directly inline.
    TimeRange::new(start_ticks, end_ticks, tb())
  }

  #[test]
  fn try_new_happy_path() {
    let parent = Uuid7::new();
    let s = AudioSegment::try_new(Uuid7::new(), parent, 0, span(0, 1500))
      .expect("valid construction must succeed");
    assert_eq!(s.parent(), &parent);
    assert_eq!(s.index(), 0);
    assert!(s.speaker().is_none());
    assert!(s.text().is_empty());
    assert!(s.words().is_empty());
    assert!(s.language().is_none());
    assert!(s.no_speech_prob().is_none());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = AudioSegment::try_new(Uuid7::nil(), Uuid7::new(), 0, span(0, 1500));
    assert_eq!(r.err(), Some(AudioSegmentError::NilId));
    assert!(AudioSegmentError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_parent() {
    let r = AudioSegment::try_new(Uuid7::new(), Uuid7::nil(), 0, span(0, 1500));
    assert_eq!(r.err(), Some(AudioSegmentError::NilParent));
    assert!(AudioSegmentError::NilParent.is_nil_parent());
  }

  #[test]
  fn inverted_span_variant_is_constructible_and_reports_predicate() {
    // `mediatime::TimeRange::new` itself enforces `start <= end` (panicking),
    // and `TimeRange::try_new` rejects the inverted case — so a real
    // inverted `TimeRange` cannot be reached from safe public APIs.
    // The `InvertedSpan` arm is defence-in-depth (in case `mediatime` ever
    // exposes a bypass ctor or moves to a sentinel-tolerant `TimeRange`).
    // Confirm only the predicate + Display surface here.
    let e = AudioSegmentError::InvertedSpan;
    assert!(e.is_inverted_span());
    assert_eq!(
      format!("{e}"),
      "AudioSegment span.start must be <= span.end"
    );
  }

  #[test]
  fn try_new_accepts_zero_length_span() {
    // `start == end` is allowed (locked invariant: `start <= end`).
    AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(500, 500))
      .expect("zero-length span ok");
  }

  #[test]
  fn builders_attach_speaker_and_text() {
    let speaker = Uuid7::new();
    let es = Language::from_bcp47("es").unwrap();
    let s = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 2, span(1000, 2000))
      .unwrap()
      .try_with_speaker(Some(speaker))
      .unwrap()
      .with_text(LocalizedText::from_src_translated("hola", "hello"))
      .with_language(Some(es));
    assert_eq!(s.speaker(), Some(&speaker));
    assert_eq!(s.text().src(), "hola");
    assert_eq!(s.text().translated(), "hello");
    assert_eq!(s.language(), Some(es));
    assert_eq!(s.language().unwrap().language(), "es");
  }

  #[test]
  fn words_attach_and_carry_per_word_language() {
    let fr = Language::from_bcp47("fr").unwrap();
    let w1 = Word::try_new("bon", span(0, 200), 0.95)
      .unwrap()
      .with_language(Some(fr));
    let w2 = Word::try_from_parts("jour", span(200, 400), 0.92, Some(fr)).unwrap();
    let s = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 400))
      .unwrap()
      .try_with_words(std::vec![w1.clone(), w2.clone()])
      .unwrap();
    assert_eq!(s.words().len(), 2);
    assert_eq!(s.words()[0].text(), "bon");
    assert!((s.words()[0].score() - 0.95).abs() < f32::EPSILON);
    assert_eq!(s.words()[1].language(), Some(fr));
  }

  #[test]
  fn word_try_new_rejects_non_finite_or_out_of_range_score() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 1.1] {
      let r = Word::try_new("x", span(0, 100), bad);
      assert_eq!(r.err(), Some(WordError::ScoreOutOfRange));
    }
    // boundary values are accepted
    assert!(Word::try_new("x", span(0, 100), 0.0).is_ok());
    assert!(Word::try_new("x", span(0, 100), 1.0).is_ok());
    assert!(WordError::ScoreOutOfRange.is_score_out_of_range());
  }

  #[test]
  fn word_try_with_score_and_try_set_score_validate() {
    let w = Word::try_new("x", span(0, 100), 0.5).unwrap();
    assert!(w.clone().try_with_score(f32::NAN).is_err());
    assert!(w.clone().try_with_score(2.0).is_err());
    assert!(w.clone().try_with_score(0.8).is_ok());

    let mut w = w;
    assert!(w.try_set_score(f32::NEG_INFINITY).is_err());
    // rejection leaves the value unchanged
    assert!((w.score() - 0.5).abs() < f32::EPSILON);
    w.try_set_score(0.25).unwrap();
    assert!((w.score() - 0.25).abs() < f32::EPSILON);
  }

  #[test]
  fn try_with_words_rejects_word_span_outside_segment() {
    let seg = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(100, 400)).unwrap();
    // word starts before the segment
    let early = Word::try_new("a", span(0, 200), 0.9).unwrap();
    let r = seg.clone().try_with_words(std::vec![early]);
    assert_eq!(r.err(), Some(AudioSegmentError::WordSpanOutOfSegment));
    // word ends after the segment
    let late = Word::try_new("b", span(300, 500), 0.9).unwrap();
    let r = seg.clone().try_with_words(std::vec![late]);
    assert_eq!(r.err(), Some(AudioSegmentError::WordSpanOutOfSegment));
    assert!(AudioSegmentError::WordSpanOutOfSegment.is_word_span_out_of_segment());
    // a fully-contained word is accepted
    let ok = Word::try_new("c", span(150, 300), 0.9).unwrap();
    assert!(seg.try_with_words(std::vec![ok]).is_ok());
  }

  #[test]
  fn try_set_words_rejects_and_leaves_words_unchanged() {
    let mut seg = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(100, 400)).unwrap();
    let good = Word::try_new("c", span(150, 300), 0.9).unwrap();
    seg.try_set_words(std::vec![good.clone()]).unwrap();
    assert_eq!(seg.words().len(), 1);
    let bad = Word::try_new("d", span(0, 50), 0.9).unwrap();
    let r = seg.try_set_words(std::vec![bad]);
    assert_eq!(r.err(), Some(AudioSegmentError::WordSpanOutOfSegment));
    // the prior valid word list is still in place
    assert_eq!(seg.words(), &[good]);
  }

  #[test]
  fn whisper_quality_signals_attach() {
    let s = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 500))
      .unwrap()
      .try_with_no_speech_prob(Some(0.05))
      .unwrap()
      .with_avg_logprob(Some(-0.4))
      .with_temperature(Some(0.0));
    assert!((s.no_speech_prob().unwrap() - 0.05).abs() < f32::EPSILON);
    assert!((s.avg_logprob().unwrap() - -0.4).abs() < f32::EPSILON);
    assert!((s.temperature().unwrap() - 0.0).abs() < f32::EPSILON);
  }

  #[test]
  fn setters_mutate_in_place() {
    let mut s = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 500)).unwrap();
    let speaker = Uuid7::new();
    s.try_set_speaker(Some(speaker)).unwrap();
    s.set_text(LocalizedText::from_src("hello"));
    s.try_set_words(std::vec![Word::try_new("hi", span(0, 100), 0.9).unwrap()])
      .unwrap();
    s.try_set_no_speech_prob(Some(0.01)).unwrap();
    assert_eq!(s.speaker(), Some(&speaker));
    assert_eq!(s.text().src(), "hello");
    assert_eq!(s.words().len(), 1);
    assert!((s.no_speech_prob().unwrap() - 0.01).abs() < f32::EPSILON);
  }

  // --- Finding 1: semantic word containment ---------------------------------

  /// A different `Timebase` whose ticks are 90× finer than `tb()` —
  /// `mpeg_tb()`'s `90_000` ticks = `tb()`'s `1_000` ticks (both 1 second).
  fn mpeg_tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(90_000).expect("nonzero"))
  }

  #[test]
  fn check_words_uses_semantic_time_across_timebases() {
    // Segment span 0..1s in the 1/1000 timebase.
    let seg = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 1000)).unwrap();
    // A word at 0..1.5s expressed in the 1/90_000 timebase: raw end PTS is
    // 135_000, which is numerically *less* than the segment's raw end PTS
    // 1000 — so a raw-PTS comparison would wrongly accept it. Semantically
    // it ends at 1.5s, well past the segment's 1.0s end.
    let w_late = Word::try_new("late", TimeRange::new(0, 135_000, mpeg_tb()), 0.9).unwrap();
    let r = seg.clone().try_with_words(std::vec![w_late]);
    assert_eq!(r.err(), Some(AudioSegmentError::WordSpanOutOfSegment));

    // A word genuinely contained: 0.25s..0.75s in the fine timebase
    // (22_500..67_500 ticks) is ⊆ the 0..1s segment.
    let w_ok = Word::try_new("ok", TimeRange::new(22_500, 67_500, mpeg_tb()), 0.9).unwrap();
    assert!(seg.try_with_words(std::vec![w_ok]).is_ok());
  }

  #[test]
  fn check_words_rejects_inverted_word_span() {
    let seg = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 1000)).unwrap();
    // `TimeRange::new` panics on inverted ranges; `try_new` returns `None`.
    // Build an inverted range via `with_end` (no invariant re-check) so the
    // domain-level `InvertedWordSpan` arm is exercised on a real value.
    let inverted = TimeRange::new(100, 200, tb()).with_end(50);
    let w = Word::try_new("bad", inverted, 0.9).unwrap();
    let r = seg.try_with_words(std::vec![w]);
    assert_eq!(r.err(), Some(AudioSegmentError::InvertedWordSpan));
    assert!(AudioSegmentError::InvertedWordSpan.is_inverted_word_span());
  }

  // --- Finding 2: no_speech_prob validation ---------------------------------

  #[test]
  fn try_with_no_speech_prob_rejects_invalid_and_accepts_boundaries() {
    let seg = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 500)).unwrap();
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 1.1] {
      let r = seg.clone().try_with_no_speech_prob(Some(bad));
      assert_eq!(r.err(), Some(AudioSegmentError::NoSpeechProbOutOfRange));
    }
    assert!(seg.clone().try_with_no_speech_prob(None).is_ok());
    assert!(seg.clone().try_with_no_speech_prob(Some(0.0)).is_ok());
    assert!(seg.try_with_no_speech_prob(Some(1.0)).is_ok());
    assert!(AudioSegmentError::NoSpeechProbOutOfRange.is_no_speech_prob_out_of_range());
  }

  #[test]
  fn try_set_no_speech_prob_rejects_and_leaves_value_unchanged() {
    let mut seg = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 500)).unwrap();
    seg.try_set_no_speech_prob(Some(0.3)).unwrap();
    let r = seg.try_set_no_speech_prob(Some(f32::NAN));
    assert_eq!(r.err(), Some(AudioSegmentError::NoSpeechProbOutOfRange));
    // rejection leaves the prior valid value in place
    assert!((seg.no_speech_prob().unwrap() - 0.3).abs() < f32::EPSILON);
    let r = seg.try_set_no_speech_prob(Some(2.0));
    assert_eq!(r.err(), Some(AudioSegmentError::NoSpeechProbOutOfRange));
    assert!((seg.no_speech_prob().unwrap() - 0.3).abs() < f32::EPSILON);
    // a valid update goes through
    seg.try_set_no_speech_prob(Some(0.9)).unwrap();
    assert!((seg.no_speech_prob().unwrap() - 0.9).abs() < f32::EPSILON);
  }

  // --- Finding 3 (round 4): speaker FK rejects Some(nil) --------------------

  #[test]
  fn try_with_speaker_rejects_some_nil() {
    let seg = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 500)).unwrap();
    assert_eq!(
      seg.try_with_speaker(Some(Uuid7::nil())).err(),
      Some(AudioSegmentError::NilSpeaker)
    );
    assert!(AudioSegmentError::NilSpeaker.is_nil_speaker());
  }

  #[test]
  fn try_with_speaker_accepts_none_and_valid_some() {
    let seg = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 500)).unwrap();
    // `None` — the "not diarized" sentinel — is always accepted.
    let n = seg.clone().try_with_speaker(None).expect("None accepted");
    assert!(n.speaker().is_none());
    // a real `Speaker` id is accepted.
    let speaker = Uuid7::new();
    let s = seg
      .try_with_speaker(Some(speaker))
      .expect("valid speaker accepted");
    assert_eq!(s.speaker(), Some(&speaker));
  }

  #[test]
  fn try_set_speaker_rejects_some_nil_and_leaves_value_unchanged() {
    let mut seg = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 500)).unwrap();
    let speaker = Uuid7::new();
    seg.try_set_speaker(Some(speaker)).unwrap();
    // a `Some(nil)` update is rejected, leaving the prior value in place
    assert_eq!(
      seg.try_set_speaker(Some(Uuid7::nil())).err(),
      Some(AudioSegmentError::NilSpeaker)
    );
    assert_eq!(seg.speaker(), Some(&speaker));
    // clearing to `None` goes through
    seg.try_set_speaker(None).unwrap();
    assert!(seg.speaker().is_none());
  }
}
