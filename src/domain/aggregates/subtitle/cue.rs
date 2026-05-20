//! `SubtitleCue<Id>` — one parsed cue of a `SubtitleTrack` (locked
//! `schema/subtitle_cues.md` r3). The heavy per-track segmented
//! aggregate (parallel to `Scene` and the planned `AudioSegment`); the
//! track keeps only the `cue_count` rollup.
//!
//! `text` and `ocr_text` are both [`LocalizedText`] and **kept distinct**
//! (source of truth differs — parsed vs OCR). Bitmap-format inline data
//! is `image: Vec<u8>` — empty means absent (mirrors locked
//! `Keyframe.data` — no `Option`, no `Location`).
//!
//! ### `Bytes` placeholder
//!
//! The locked schema's `image` field is typed `Bytes` (`bytes::Bytes`),
//! and the wire layer already uses `::buffa::bytes::Bytes`. The `bytes`
//! crate is not yet a direct `mediaschema` dependency, so this PR uses
//! `std::vec::Vec<u8>` for the inline cue bitmap. **TODO(mediaframe):**
//! migrate to `bytes::Bytes` once the dep is added (cheap-clone via
//! refcounted slices is the eventual goal — same migration story as
//! `Keyframe.data`).
//!
//! No `provenance` field — it lives on the parent `SubtitleTrack` (one
//! parse/OCR run per track, locked).

use derive_more::IsVariant;
use mediatime::TimeRange;
use smol_str::SmolStr;

use crate::domain::{vo::LocalizedText, Uuid7};

/// One parsed cue. Generic over `Id` (default [`Uuid7`]).
///
/// **No `Default`** — a `SubtitleCue` with nil `id`/`parent` is an
/// orphan with no track. Construct via [`SubtitleCue::try_new`]. Fields
/// are private; access via getters and `with_*` / `set_*`
/// builders/mutators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleCue<Id = Uuid7> {
  id: Id,
  parent: Id,
  index: u32,
  span: TimeRange,
  text: LocalizedText,
  styled_text: SmolStr,
  /// TODO(mediaframe / bytes): see module doc — wire schema is
  /// `bytes::Bytes`; we hold `Vec<u8>` until the `bytes` dep is added.
  /// Empty = absent (mirrors `Keyframe.data`).
  image: std::vec::Vec<u8>,
  ocr_text: LocalizedText,
}

impl SubtitleCue<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (every cue needs a real identity for LanceDB
  /// embedding key + `(parent, index)` projection-level uniqueness) and
  /// nil `parent` (orphan cue with no `SubtitleTrack` reference).
  /// `mediatime::TimeRange::new` already enforces `start <= end` by
  /// construction (panic-on-violation), so a non-panicking `TimeRange`
  /// is already known-valid — no extra span check here.
  pub fn try_new(
    id: Uuid7,
    parent: Uuid7,
    index: u32,
    span: TimeRange,
  ) -> Result<Self, SubtitleCueError> {
    if id.is_nil() {
      return Err(SubtitleCueError::NilId);
    }
    if parent.is_nil() {
      return Err(SubtitleCueError::NilParent);
    }
    Ok(Self {
      id,
      parent,
      index,
      span,
      text: LocalizedText::new(),
      styled_text: SmolStr::default(),
      image: std::vec::Vec::new(),
      ocr_text: LocalizedText::new(),
    })
  }
}

impl<Id> SubtitleCue<Id> {
  /// Canonical identity (also the LanceDB embedding key).
  #[inline]
  pub const fn id(&self) -> &Id {
    &self.id
  }

  /// FK → `SubtitleTrack.id`.
  #[inline]
  pub const fn parent(&self) -> &Id {
    &self.parent
  }

  /// 0-based cue ordinal within the parent track.
  #[inline]
  pub const fn index(&self) -> u32 {
    self.index
  }

  /// On-screen interval in media-time.
  #[inline]
  pub const fn span(&self) -> &TimeRange {
    &self.span
  }

  /// Parsed plain text (styling stripped); both `src` and `translated`
  /// fields `""` = absent.
  #[inline]
  pub const fn text(&self) -> &LocalizedText {
    &self.text
  }

  /// Original markup retained for render fidelity (ASS/SSA);
  /// `""` = none (string-rule: no `Option`).
  #[inline]
  pub fn styled_text(&self) -> &str {
    self.styled_text.as_str()
  }

  /// Inline rendered cue bitmap (PGS/DVBSUB); empty = none.
  /// TODO(mediaframe / bytes): see module doc.
  #[inline]
  pub fn image(&self) -> &[u8] {
    &self.image
  }

  /// Text extracted from `image` by the OCR stage; kept distinct from
  /// `text`. Both `src` and `translated` fields `""` = absent.
  #[inline]
  pub const fn ocr_text(&self) -> &LocalizedText {
    &self.ocr_text
  }

  /// True when this cue carries no content of any kind — text, OCR
  /// text, and image all empty. The locked invariant (cue is non-empty)
  /// is a **construction-time** projection check rather than a
  /// `try_new` rejection because callers populate the content fields
  /// via `with_*` after creation.
  #[inline]
  pub fn is_blank(&self) -> bool {
    self.text.src().is_empty()
      && self.text.translated().is_empty()
      && self.ocr_text.src().is_empty()
      && self.ocr_text.translated().is_empty()
      && self.image.is_empty()
  }

  // -------------------------------------------------------------------
  // Builders.
  // -------------------------------------------------------------------

  /// Builder: replace `index`.
  #[inline]
  pub const fn with_index(mut self, v: u32) -> Self {
    self.index = v;
    self
  }

  /// Builder: replace `span`.
  #[inline]
  pub const fn with_span(mut self, v: TimeRange) -> Self {
    self.span = v;
    self
  }

  /// Builder: replace `text`.
  #[inline]
  pub fn with_text(mut self, v: LocalizedText) -> Self {
    self.text = v;
    self
  }

  /// Builder: replace `styled_text`.
  #[inline]
  pub fn with_styled_text(mut self, v: impl Into<SmolStr>) -> Self {
    self.styled_text = v.into();
    self
  }

  /// Builder: replace `image`.
  #[inline]
  pub fn with_image(mut self, v: impl Into<std::vec::Vec<u8>>) -> Self {
    self.image = v.into();
    self
  }

  /// Builder: replace `ocr_text`.
  #[inline]
  pub fn with_ocr_text(mut self, v: LocalizedText) -> Self {
    self.ocr_text = v;
    self
  }

  // -------------------------------------------------------------------
  // In-place setters.
  // -------------------------------------------------------------------

  /// In-place mutator for `index`.
  #[inline]
  pub const fn set_index(&mut self, v: u32) {
    self.index = v;
  }

  /// In-place mutator for `span`.
  #[inline]
  pub const fn set_span(&mut self, v: TimeRange) {
    self.span = v;
  }

  /// In-place mutator for `text`.
  #[inline]
  pub fn set_text(&mut self, v: LocalizedText) {
    self.text = v;
  }

  /// In-place mutator for `styled_text`.
  #[inline]
  pub fn set_styled_text(&mut self, v: impl Into<SmolStr>) {
    self.styled_text = v.into();
  }

  /// In-place mutator for `image`.
  #[inline]
  pub fn set_image(&mut self, v: impl Into<std::vec::Vec<u8>>) {
    self.image = v.into();
  }

  /// In-place mutator for `ocr_text`.
  #[inline]
  pub fn set_ocr_text(&mut self, v: LocalizedText) {
    self.ocr_text = v;
  }
}

/// Error returned when [`SubtitleCue::try_new`] cannot uphold the
/// non-nil-id / non-nil-parent invariants. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum SubtitleCueError {
  /// Supplied `id` was the nil sentinel — would collide as a LanceDB
  /// embedding key.
  #[error("SubtitleCue id must not be the nil UUID")]
  NilId,
  /// Supplied `parent` was the nil sentinel — orphan cue with no
  /// `SubtitleTrack` reference.
  #[error("SubtitleCue parent (SubtitleTrack) must not be the nil UUID")]
  NilParent,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use mediatime::Timebase;

  fn span() -> TimeRange {
    TimeRange::new(1000, 2000, Timebase::default())
  }

  #[test]
  fn try_new_happy_path() {
    let parent = Uuid7::new();
    let c = SubtitleCue::try_new(Uuid7::new(), parent, 0, span())
      .expect("valid construction must succeed");
    assert_eq!(c.parent(), &parent);
    assert_eq!(c.index(), 0);
    assert_eq!(c.span(), &span());
    assert!(c.text().src().is_empty());
    assert!(c.text().translated().is_empty());
    assert!(c.styled_text().is_empty());
    assert!(c.image().is_empty());
    assert!(c.ocr_text().src().is_empty());
    assert!(c.is_blank());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = SubtitleCue::try_new(Uuid7::nil(), Uuid7::new(), 0, span());
    assert_eq!(r.err(), Some(SubtitleCueError::NilId));
    assert!(SubtitleCueError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_parent() {
    let r = SubtitleCue::try_new(Uuid7::new(), Uuid7::nil(), 0, span());
    assert_eq!(r.err(), Some(SubtitleCueError::NilParent));
    assert!(SubtitleCueError::NilParent.is_nil_parent());
  }

  #[test]
  fn text_subs_use_text_field() {
    let c = SubtitleCue::try_new(Uuid7::new(), Uuid7::new(), 0, span())
      .unwrap()
      .with_text(LocalizedText::from_src_translated(
        "\u{4f60}\u{597d}",
        "Hello",
      ))
      .with_styled_text("{\\b1}Hello{\\b0}");
    assert_eq!(c.text().src(), "\u{4f60}\u{597d}");
    assert_eq!(c.text().translated(), "Hello");
    assert_eq!(c.styled_text(), "{\\b1}Hello{\\b0}");
    assert!(c.image().is_empty());
    assert!(!c.is_blank());
  }

  #[test]
  fn bitmap_subs_use_image_and_ocr() {
    let bitmap = std::vec![0u8, 1, 2, 3];
    let c = SubtitleCue::try_new(Uuid7::new(), Uuid7::new(), 5, span())
      .unwrap()
      .with_index(5)
      .with_image(bitmap.clone())
      .with_ocr_text(LocalizedText::from_src("Hello (OCR)"));
    assert_eq!(c.index(), 5);
    assert_eq!(c.image(), bitmap.as_slice());
    assert_eq!(c.ocr_text().src(), "Hello (OCR)");
    assert!(!c.is_blank());
  }

  #[test]
  fn setters_mutate_in_place() {
    let mut c = SubtitleCue::try_new(Uuid7::new(), Uuid7::new(), 0, span()).unwrap();
    c.set_index(7);
    c.set_text(LocalizedText::from_src("Bonjour"));
    c.set_styled_text("Bonjour");
    c.set_image(std::vec![42u8]);
    c.set_ocr_text(LocalizedText::from_src("Bonjour (OCR)"));
    assert_eq!(c.index(), 7);
    assert_eq!(c.text().src(), "Bonjour");
    assert_eq!(c.styled_text(), "Bonjour");
    assert_eq!(c.image(), &[42u8]);
    assert_eq!(c.ocr_text().src(), "Bonjour (OCR)");
  }
}
