//! Shared cross-cutting value-objects.
//!
//! Defined once here, reused by every analysis aggregate
//! (`Keyframe`/`Scene`/`AudioTrack`/`SubtitleTrack`/…) per the locked README
//! cross-cutting rules. Free-text rule: `SmolStr` with `""`=absent, never
//! `Option<SmolStr>`.
//!
//! **Encapsulation rule:** no public fields anywhere in the domain layer.
//! Access goes through `field()` getters and `with_field(...)` const-where-
//! possible setters; mutation uses `set_field(...)`.

use smol_str::SmolStr;

// ---------------------------------------------------------------------------
// Provenance — analysis-run reproducibility
// ---------------------------------------------------------------------------

/// Analysis-run reproducibility — which model/prompt/indexer produced this
/// analysis record, so a re-run on upgrade is deterministic.
///
/// Locked shared cross-cutting VO (see `schema/README.md`). Carried by every
/// analysis record — `Keyframe`, `Scene`, `AudioTrack` index-state,
/// `SubtitleTrack` — as a `provenance` field. Per-track on `AudioTrack`/
/// `SubtitleTrack` (one value per run), not per `AudioSegment`/`SubtitleCue`.
///
/// All four fields are `SmolStr` with `""`=absent. No `Option` — the locked
/// rule reserves `Option` for structured/enum/numeric absence.
///
/// **Default convention**: `Default::default()` calls [`Provenance::new`],
/// which returns the all-empty record. Use [`Provenance::from_parts`] to
/// supply all four fields in one call, or chain the `with_*` builders
/// onto `Provenance::new()` to fill incrementally.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Provenance {
  model_name: SmolStr,
  model_version: SmolStr,
  prompt_version: SmolStr,
  indexer_version: SmolStr,
}

impl Provenance {
  /// Canonical no-arg constructor — every field empty (`""`).
  /// [`Default::default`] is `Self::new()`.
  ///
  /// (Not `const fn` — `SmolStr::default()` is not `const` in
  /// `smol_str` 0.3.)
  #[inline]
  pub fn new() -> Self {
    Self {
      model_name: SmolStr::default(),
      model_version: SmolStr::default(),
      prompt_version: SmolStr::default(),
      indexer_version: SmolStr::default(),
    }
  }

  /// Construct a `Provenance` from its four fields.
  #[inline]
  pub fn from_parts(
    model_name: impl Into<SmolStr>,
    model_version: impl Into<SmolStr>,
    prompt_version: impl Into<SmolStr>,
    indexer_version: impl Into<SmolStr>,
  ) -> Self {
    Self {
      model_name: model_name.into(),
      model_version: model_version.into(),
      prompt_version: prompt_version.into(),
      indexer_version: indexer_version.into(),
    }
  }

  /// Model name (`""` = absent).
  #[inline]
  pub fn model_name(&self) -> &str {
    self.model_name.as_str()
  }

  /// Model version (`""` = absent).
  #[inline]
  pub fn model_version(&self) -> &str {
    self.model_version.as_str()
  }

  /// Prompt-template version (`""` = absent).
  #[inline]
  pub fn prompt_version(&self) -> &str {
    self.prompt_version.as_str()
  }

  /// Indexer build/version (`""` = absent).
  #[inline]
  pub fn indexer_version(&self) -> &str {
    self.indexer_version.as_str()
  }

  /// Builder: replace `model_name` and return `self`.
  #[inline]
  pub fn with_model_name(mut self, v: impl Into<SmolStr>) -> Self {
    self.model_name = v.into();
    self
  }

  /// Builder: replace `model_version` and return `self`.
  #[inline]
  pub fn with_model_version(mut self, v: impl Into<SmolStr>) -> Self {
    self.model_version = v.into();
    self
  }

  /// Builder: replace `prompt_version` and return `self`.
  #[inline]
  pub fn with_prompt_version(mut self, v: impl Into<SmolStr>) -> Self {
    self.prompt_version = v.into();
    self
  }

  /// Builder: replace `indexer_version` and return `self`.
  #[inline]
  pub fn with_indexer_version(mut self, v: impl Into<SmolStr>) -> Self {
    self.indexer_version = v.into();
    self
  }

  /// In-place mutator.
  #[inline]
  pub fn set_model_name(&mut self, v: impl Into<SmolStr>) {
    self.model_name = v.into();
  }

  /// In-place mutator.
  #[inline]
  pub fn set_model_version(&mut self, v: impl Into<SmolStr>) {
    self.model_version = v.into();
  }

  /// In-place mutator.
  #[inline]
  pub fn set_prompt_version(&mut self, v: impl Into<SmolStr>) {
    self.prompt_version = v.into();
  }

  /// In-place mutator.
  #[inline]
  pub fn set_indexer_version(&mut self, v: impl Into<SmolStr>) {
    self.indexer_version = v.into();
  }

  /// Is every field absent (`""`)? Useful when an analysis record exists
  /// but its provenance has not been recorded yet.
  #[inline]
  pub fn is_empty(&self) -> bool {
    self.model_name.is_empty()
      && self.model_version.is_empty()
      && self.prompt_version.is_empty()
      && self.indexer_version.is_empty()
  }
}

impl Default for Provenance {
  #[inline]
  fn default() -> Self {
    Self::new()
  }
}

// ---------------------------------------------------------------------------
// LocalizedText — free-text + optional translation
// ---------------------------------------------------------------------------

/// Free-text narrative paired with an optional translation to a single
/// canonical target (English by convention).
///
/// Locked shared cross-cutting VO (see `schema/README.md`). Used for *all
/// VLM natural-language output* on `Keyframe.VlmAnalysis` (every text/label
/// vector field) and `AudioSegment.text` (with the planned `asry`
/// `translated_text` extension — whisper-`translate` task). Per-token text
/// (e.g. `AudioSegment.Word.text`) stays `SmolStr` — translation is segment
/// / narrative-level only, not word-aligned.
///
/// Both fields are `SmolStr` with `""`=absent. `pt-BR` ≠ `pt-PT` are
/// distinct values; the language tag itself lives in `mediaframe::Language`
/// (a separate field where present).
///
/// **Default convention**: `Default::default()` calls
/// [`LocalizedText::new`], which returns the all-empty record. Use
/// [`LocalizedText::from_src`] for source-only, or
/// [`LocalizedText::from_src_translated`] for both fields in one call.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocalizedText {
  src: SmolStr,
  translated: SmolStr,
}

impl LocalizedText {
  /// Canonical no-arg constructor — both fields empty (`""`).
  /// [`Default::default`] is `Self::new()`.
  ///
  /// (Not `const fn` — `SmolStr::default()` is not `const` in
  /// `smol_str` 0.3.)
  #[inline]
  pub fn new() -> Self {
    Self {
      src: SmolStr::default(),
      translated: SmolStr::default(),
    }
  }

  /// Construct from explicit source + translation.
  #[inline]
  pub fn from_src_translated(src: impl Into<SmolStr>, translated: impl Into<SmolStr>) -> Self {
    Self {
      src: src.into(),
      translated: translated.into(),
    }
  }

  /// Construct from source text only (translation not yet available).
  #[inline]
  pub fn from_src(src: impl Into<SmolStr>) -> Self {
    Self {
      src: src.into(),
      translated: SmolStr::default(),
    }
  }

  /// Source text (`""` = absent).
  #[inline]
  pub fn src(&self) -> &str {
    self.src.as_str()
  }

  /// Translated text (`""` = no translation yet).
  #[inline]
  pub fn translated(&self) -> &str {
    self.translated.as_str()
  }

  /// Builder: replace `src` and return `self`.
  #[inline]
  pub fn with_src(mut self, v: impl Into<SmolStr>) -> Self {
    self.src = v.into();
    self
  }

  /// Builder: replace `translated` and return `self`.
  #[inline]
  pub fn with_translated(mut self, v: impl Into<SmolStr>) -> Self {
    self.translated = v.into();
    self
  }

  /// In-place mutator.
  #[inline]
  pub fn set_src(&mut self, v: impl Into<SmolStr>) {
    self.src = v.into();
  }

  /// In-place mutator.
  #[inline]
  pub fn set_translated(&mut self, v: impl Into<SmolStr>) {
    self.translated = v.into();
  }

  /// Both fields empty?
  #[inline]
  pub fn is_empty(&self) -> bool {
    self.src.is_empty() && self.translated.is_empty()
  }

  /// Best available text: `translated` if non-empty, else `src`. Useful
  /// for default UI rendering / search-index population.
  #[inline]
  pub fn display(&self) -> &str {
    if !self.translated.is_empty() {
      self.translated.as_str()
    } else {
      self.src.as_str()
    }
  }
}

impl Default for LocalizedText {
  #[inline]
  fn default() -> Self {
    Self::new()
  }
}

// ===========================================================================
// Tests
// ===========================================================================

// vo.rs itself is only compiled under `feature = "alloc"`, so the
// test module is automatically alloc-gated. No separate gate needed.
#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn provenance_new_is_empty_and_default_delegates() {
    // `Default::default()` calls `Self::new()` per the rule.
    let p = Provenance::new();
    assert!(p.is_empty());
    assert_eq!(p.model_name(), "");
    assert_eq!(p.model_version(), "");
    assert_eq!(p.prompt_version(), "");
    assert_eq!(p.indexer_version(), "");
    assert_eq!(Provenance::default(), p);
  }

  #[test]
  fn provenance_construction_and_emptiness() {
    let p = Provenance::from_parts(
      "qwen2-vl-7b",
      "v0.3.0",
      "vlm-prompt@2",
      "findit-indexer-0.1.0",
    );
    assert!(!p.is_empty());
    assert_eq!(p.model_name(), "qwen2-vl-7b");
    assert_eq!(p.indexer_version(), "findit-indexer-0.1.0");

    // Even one non-empty field defeats is_empty.
    let p2 = Provenance::from_parts("", "", "", "x");
    assert!(!p2.is_empty());
  }

  #[test]
  fn provenance_builders_chain() {
    let p = Provenance::default()
      .with_model_name("qwen2-vl-7b")
      .with_model_version("v0.3.0")
      .with_prompt_version("vlm-prompt@2")
      .with_indexer_version("findit-indexer-0.1.0");
    assert_eq!(p.model_name(), "qwen2-vl-7b");
    assert_eq!(p.model_version(), "v0.3.0");
    assert_eq!(p.prompt_version(), "vlm-prompt@2");
    assert_eq!(p.indexer_version(), "findit-indexer-0.1.0");
  }

  #[test]
  fn provenance_setters_mutate_in_place() {
    let mut p = Provenance::default();
    p.set_model_name("qwen2-vl-7b");
    p.set_indexer_version("findit-indexer-0.1.0");
    assert_eq!(p.model_name(), "qwen2-vl-7b");
    assert_eq!(p.indexer_version(), "findit-indexer-0.1.0");
  }

  #[test]
  fn localized_text_new_is_empty_and_default_delegates() {
    let t = LocalizedText::new();
    assert!(t.is_empty());
    assert_eq!(t.display(), "");
    assert_eq!(LocalizedText::default(), t);
  }

  #[test]
  fn localized_text_from_src_no_translation() {
    let t = LocalizedText::from_src("Jane is eating");
    assert!(!t.is_empty());
    assert_eq!(t.src(), "Jane is eating");
    assert_eq!(t.translated(), "");
    // No translation → display falls back to src.
    assert_eq!(t.display(), "Jane is eating");
  }

  #[test]
  fn localized_text_display_prefers_translation() {
    let t = LocalizedText::from_src_translated("\u{4f60}\u{597d}", "Hello");
    assert_eq!(t.src(), "\u{4f60}\u{597d}");
    assert_eq!(t.translated(), "Hello");
    // Translation present → display returns it.
    assert_eq!(t.display(), "Hello");
  }

  #[test]
  fn localized_text_translation_only_displays_translation() {
    let t = LocalizedText::from_src_translated("", "Hello");
    assert!(!t.is_empty());
    assert_eq!(t.display(), "Hello");
  }

  #[test]
  fn localized_text_builders_and_setters() {
    let t = LocalizedText::default()
      .with_src("Jane")
      .with_translated("Jane");
    assert_eq!(t.display(), "Jane");
    let mut t = t;
    t.set_translated("");
    assert_eq!(t.translated(), "");
    assert_eq!(t.display(), "Jane");
  }
}
