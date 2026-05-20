//! Shared cross-cutting value-objects.
//!
//! Defined once here, reused by every analysis aggregate
//! (`Keyframe`/`Scene`/`AudioTrack`/`SubtitleTrack`/…) per the locked README
//! cross-cutting rules. Free-text rule: `SmolStr` with `""`=absent, never
//! `Option<SmolStr>`.

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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Provenance {
  pub model_name: SmolStr,
  pub model_version: SmolStr,
  pub prompt_version: SmolStr,
  pub indexer_version: SmolStr,
}

impl Provenance {
  /// Construct a `Provenance` from its four fields.
  #[inline]
  pub fn new(
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct LocalizedText {
  pub src: SmolStr,
  pub translated: SmolStr,
}

impl LocalizedText {
  /// Construct from explicit source + translation.
  #[inline]
  pub fn new(src: impl Into<SmolStr>, translated: impl Into<SmolStr>) -> Self {
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

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn provenance_default_is_empty() {
    let p = Provenance::default();
    assert!(p.is_empty());
    assert_eq!(p.model_name.as_str(), "");
    assert_eq!(p.model_version.as_str(), "");
    assert_eq!(p.prompt_version.as_str(), "");
    assert_eq!(p.indexer_version.as_str(), "");
  }

  #[test]
  fn provenance_construction_and_emptiness() {
    let p = Provenance::new(
      "qwen2-vl-7b",
      "v0.3.0",
      "vlm-prompt@2",
      "findit-indexer-0.1.0",
    );
    assert!(!p.is_empty());
    assert_eq!(p.model_name.as_str(), "qwen2-vl-7b");
    assert_eq!(p.indexer_version.as_str(), "findit-indexer-0.1.0");

    // Even one non-empty field defeats is_empty.
    let p2 = Provenance::new("", "", "", "x");
    assert!(!p2.is_empty());
  }

  #[test]
  fn localized_text_default_is_empty() {
    let t = LocalizedText::default();
    assert!(t.is_empty());
    assert_eq!(t.display(), "");
  }

  #[test]
  fn localized_text_from_src_no_translation() {
    let t = LocalizedText::from_src("Jane is eating");
    assert!(!t.is_empty());
    assert_eq!(t.src.as_str(), "Jane is eating");
    assert_eq!(t.translated.as_str(), "");
    // No translation → display falls back to src.
    assert_eq!(t.display(), "Jane is eating");
  }

  #[test]
  fn localized_text_display_prefers_translation() {
    let t = LocalizedText::new("\u{4f60}\u{597d}", "Hello");
    assert_eq!(t.src.as_str(), "\u{4f60}\u{597d}");
    assert_eq!(t.translated.as_str(), "Hello");
    // Translation present → display returns it.
    assert_eq!(t.display(), "Hello");
  }

  #[test]
  fn localized_text_translation_only_displays_translation() {
    let t = LocalizedText::new("", "Hello");
    assert!(!t.is_empty());
    assert_eq!(t.display(), "Hello");
  }
}
