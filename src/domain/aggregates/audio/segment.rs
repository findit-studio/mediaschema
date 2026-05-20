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
use mediatime::TimeRange;
use smol_str::SmolStr;

use crate::domain::{vo::LocalizedText, Uuid7};

// ---------------------------------------------------------------------------
// Word — nested VO
// ---------------------------------------------------------------------------

/// Word-level timing + score (`asry` output; producer-agnostic).
///
/// Locked `audio_segments.md` §Nested value-objects. `score` ∈ `[0,1]`,
/// **always present** (NaN-free per locked spec). Per-word `language`
/// carries code-switch / multilingual cues.
///
/// TODO(mediaframe): `language: Option<mediaframe::Language>` (BCP-47).
/// Placeholder = `Option<SmolStr>` (the raw BCP-47 string).
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
  text: SmolStr,
  span: TimeRange,
  score: f32,
  language: Option<SmolStr>,
}

impl Word {
  /// Construct from `(text, span, score)`; `language` starts `None` (use
  /// [`Word::with_language`] to set).
  #[inline]
  pub fn new(text: impl Into<SmolStr>, span: TimeRange, score: f32) -> Self {
    Self {
      text: text.into(),
      span,
      score,
      language: None,
    }
  }

  /// Construct with all four fields.
  #[inline]
  pub fn from_parts(
    text: impl Into<SmolStr>,
    span: TimeRange,
    score: f32,
    language: Option<SmolStr>,
  ) -> Self {
    Self {
      text: text.into(),
      span,
      score,
      language,
    }
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

  /// Per-word BCP-47 language tag placeholder (`None` = inherits segment).
  /// TODO(mediaframe).
  #[inline]
  pub fn language(&self) -> Option<&str> {
    self.language.as_deref()
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

  /// Builder: replace `score`.
  #[inline]
  pub const fn with_score(mut self, score: f32) -> Self {
    self.score = score;
    self
  }

  /// Builder: replace `language` placeholder. TODO(mediaframe).
  #[inline]
  pub fn with_language(mut self, v: Option<SmolStr>) -> Self {
    self.language = v;
    self
  }
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
  // TODO(mediaframe): `language: Option<mediaframe::Language>` (BCP-47).
  // Placeholder = `Option<SmolStr>`.
  language: Option<SmolStr>,
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

  /// Chunk language placeholder (`asry::Transcript.language`).
  /// TODO(mediaframe).
  #[inline]
  pub fn language(&self) -> Option<&str> {
    self.language.as_deref()
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

  /// Builder: replace `language` placeholder. TODO(mediaframe).
  #[inline]
  pub fn with_language(mut self, v: Option<SmolStr>) -> Self {
    self.language = v;
    self
  }

  /// Builder: replace `words`.
  #[inline]
  pub fn with_words(mut self, v: impl Into<std::vec::Vec<Word>>) -> Self {
    self.words = v.into();
    self
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

  /// In-place mutator for `language`. TODO(mediaframe).
  #[inline]
  pub fn set_language(&mut self, v: Option<SmolStr>) {
    self.language = v;
  }

  /// In-place mutator for `words`.
  #[inline]
  pub fn set_words(&mut self, v: impl Into<std::vec::Vec<Word>>) {
    self.words = v.into();
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

/// Error returned when [`AudioSegment::try_new`] cannot uphold the
/// non-nil-id / non-nil-parent / non-inverted-span invariants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant)]
#[non_exhaustive]
pub enum AudioSegmentError {
  /// Supplied `id` was the nil sentinel.
  NilId,
  /// Supplied `parent` was the nil sentinel — orphan segment with no
  /// `AudioTrack` reference.
  NilParent,
  /// `span.start > span.end` — inverted segment span (locked invariant).
  InvertedSpan,
}

impl core::fmt::Display for AudioSegmentError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::NilId => f.write_str("AudioSegment id must not be the nil UUID"),
      Self::NilParent => f.write_str("AudioSegment parent (AudioTrack) must not be the nil UUID"),
      Self::InvertedSpan => f.write_str("AudioSegment span.start must be <= span.end"),
    }
  }
}

impl core::error::Error for AudioSegmentError {}

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
    let s = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 2, span(1000, 2000))
      .unwrap()
      .with_speaker(Some(speaker))
      .with_text(LocalizedText::from_src_translated("hola", "hello"))
      .with_language(Some(SmolStr::from("es")));
    assert_eq!(s.speaker(), Some(&speaker));
    assert_eq!(s.text().src(), "hola");
    assert_eq!(s.text().translated(), "hello");
    assert_eq!(s.language(), Some("es"));
  }

  #[test]
  fn words_attach_and_carry_per_word_language() {
    let w1 = Word::new("bon", span(0, 200), 0.95).with_language(Some(SmolStr::from("fr")));
    let w2 = Word::from_parts("jour", span(200, 400), 0.92, Some(SmolStr::from("fr")));
    let s = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 400))
      .unwrap()
      .with_words(std::vec![w1.clone(), w2.clone()]);
    assert_eq!(s.words().len(), 2);
    assert_eq!(s.words()[0].text(), "bon");
    assert!((s.words()[0].score() - 0.95).abs() < f32::EPSILON);
    assert_eq!(s.words()[1].language(), Some("fr"));
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
    s.set_words(std::vec![Word::new("hi", span(0, 100), 0.9)]);
    s.set_no_speech_prob(Some(0.01));
    assert_eq!(s.speaker(), Some(&speaker));
    assert_eq!(s.text().src(), "hello");
    assert_eq!(s.words().len(), 1);
    assert!((s.no_speech_prob().unwrap() - 0.01).abs() < f32::EPSILON);
  }
}
