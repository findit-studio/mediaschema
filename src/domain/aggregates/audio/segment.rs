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
    // inverted case. Compare on the raw PTS so this stays valid even if
    // a future bypass constructor is introduced.
    if span.start_pts() > span.end_pts() {
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
  /// contained in `self.span` (`word.start >= seg.start` and
  /// `word.end <= seg.end`, compared on raw PTS — consistent with
  /// `try_new`'s span check).
  fn check_words(&self, words: &[Word]) -> Result<(), AudioSegmentError> {
    let (seg_start, seg_end) = (self.span.start_pts(), self.span.end_pts());
    for w in words {
      let (w_start, w_end) = (w.span().start_pts(), w.span().end_pts());
      if w_start < seg_start || w_end > seg_end {
        return Err(AudioSegmentError::WordSpanOutOfSegment);
      }
    }
    Ok(())
  }

  // ----- Builders ----------------------------------------------------------

  /// Builder: replace `speaker`.
  #[inline]
  pub fn with_speaker(mut self, v: Option<Id>) -> Self {
    self.speaker = v;
    self
  }

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
  /// `self.span` (locked invariant: each `Word` span ⊆ the segment span);
  /// containment is checked on raw PTS, mirroring `try_new`'s span check.
  /// `Word`'s own ctors already guarantee finite-`[0,1]` scores, so no
  /// score re-validation is needed here. On rejection `self` is returned
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

  /// Builder: replace `no_speech_prob`.
  #[inline]
  pub const fn with_no_speech_prob(mut self, v: Option<f32>) -> Self {
    self.no_speech_prob = v;
    self
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

  /// In-place mutator for `speaker`.
  #[inline]
  pub fn set_speaker(&mut self, v: Option<Id>) {
    self.speaker = v;
  }

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

  /// In-place mutator for `no_speech_prob`.
  #[inline]
  pub const fn set_no_speech_prob(&mut self, v: Option<f32>) {
    self.no_speech_prob = v;
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
  /// span).
  #[error("AudioSegment word span must be contained in the segment span")]
  WordSpanOutOfSegment,
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
      .with_speaker(Some(speaker))
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
      .with_no_speech_prob(Some(0.05))
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
    s.set_speaker(Some(speaker));
    s.set_text(LocalizedText::from_src("hello"));
    s.try_set_words(std::vec![Word::try_new("hi", span(0, 100), 0.9).unwrap()])
      .unwrap();
    s.set_no_speech_prob(Some(0.01));
    assert_eq!(s.speaker(), Some(&speaker));
    assert_eq!(s.text().src(), "hello");
    assert_eq!(s.words().len(), 1);
    assert!((s.no_speech_prob().unwrap() - 0.01).abs() < f32::EPSILON);
  }
}
