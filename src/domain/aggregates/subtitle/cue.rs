//! `SubtitleCue<Id>` — one parsed cue of a `SubtitleTrack` (locked
//! `schema/subtitle_cues.md` r3). The heavy per-track segmented
//! aggregate (parallel to `Scene` and the planned `AudioSegment`); the
//! track keeps only the `cue_count` rollup.
//!
//! `text` and `ocr_text` are both [`LocalizedText`] and **kept distinct**
//! (source of truth differs — parsed vs OCR). Bitmap-format inline data
//! is `image: bytes::Bytes` — empty means absent (mirrors locked
//! `Keyframe.data` — no `Option`, no `Location`; cheap-clone via the
//! refcounted slice matches the wire layer's `::buffa::bytes::Bytes`).
//!
//! No `provenance` field — it lives on the subtitle_track_id `SubtitleTrack` (one
//! parse/OCR run per track, locked).
//!
//! ## Content invariants (locked `subtitle_cues.md`)
//!
//! - A cue is **non-empty**: at least one of `text` / `ocr_text` /
//!   `image` is non-empty. A fully-blank cue is schema-invalid.
//! - `ocr_text` non-empty ⇒ `image` non-empty — OCR text is derived
//!   from a bitmap, so it cannot exist without one.
//!
//! Both invariants are enforced by the domain API: [`SubtitleCue::try_new`]
//! rejects a violating cue, and every content mutator that could break
//! either invariant is fallible (`try_with_*` / `try_set_*`).

use bytes::Bytes;
use derive_more::IsVariant;
use mediatime::TimeRange;
use smol_str::SmolStr;

use crate::domain::{vo::LocalizedText, Uuid7};

/// One parsed cue. Generic over `Id` (default [`Uuid7`]).
///
/// **No `Default`** — a `SubtitleCue` with nil `id`/`subtitle_track_id` is an
/// orphan with no track. Construct via [`SubtitleCue::try_new`]. Fields
/// are private; access via getters and `with_*` / `set_*` /
/// `try_with_*` / `try_set_*` builders/mutators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleCue<Id = Uuid7> {
  id: Id,
  subtitle_track_id: Id,
  index: u32,
  span: TimeRange,
  text: LocalizedText,
  styled_text: SmolStr,
  /// Inline rendered cue bitmap (PGS/DVBSUB); empty = absent (mirrors
  /// `Keyframe.data`).
  image: Bytes,
  ocr_text: LocalizedText,
}

/// Validate the cross-field content invariants of a prospective cue.
///
/// Returns `Err` when the `(text, image, ocr_text)` triple is
/// schema-invalid. `styled_text` is render-only and never participates
/// in an invariant. Free function so `try_new` and every content
/// mutator share one source of truth.
fn validate_content(
  text: &LocalizedText,
  image: &Bytes,
  ocr_text: &LocalizedText,
) -> Result<(), SubtitleCueError> {
  // `ocr_text` is derived from a bitmap: it cannot exist without one.
  if !ocr_text.is_empty() && image.is_empty() {
    return Err(SubtitleCueError::OcrTextWithoutImage);
  }
  // A cue must carry content of some kind.
  if text.is_empty() && ocr_text.is_empty() && image.is_empty() {
    return Err(SubtitleCueError::BlankCue);
  }
  Ok(())
}

impl SubtitleCue<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects:
  /// - nil `id` (every cue needs a real identity for the LanceDB
  ///   embedding key + `(subtitle_track_id, index)` projection-level uniqueness);
  /// - nil `subtitle_track_id` (orphan cue with no `SubtitleTrack` reference);
  /// - a fully-blank cue — `text`, `ocr_text` and `image` all empty
  ///   ([`SubtitleCueError::BlankCue`]);
  /// - `ocr_text` non-empty while `image` is empty
  ///   ([`SubtitleCueError::OcrTextWithoutImage`]) — OCR text is
  ///   derived from a bitmap.
  ///
  /// `mediatime::TimeRange::new` already enforces `start <= end` by
  /// construction (panic-on-violation), so a non-panicking `TimeRange`
  /// is already known-valid — no extra span check here.
  ///
  /// `styled_text` is render-only; pass [`SmolStr::default`] (or `""`)
  /// when there is no markup, then refine it via [`Self::with_styled_text`].
  #[allow(clippy::too_many_arguments)]
  pub fn try_new(
    id: Uuid7,
    subtitle_track_id: Uuid7,
    index: u32,
    span: TimeRange,
    text: LocalizedText,
    styled_text: impl Into<SmolStr>,
    image: impl Into<Bytes>,
    ocr_text: LocalizedText,
  ) -> Result<Self, SubtitleCueError> {
    if id.is_nil() {
      return Err(SubtitleCueError::NilId);
    }
    if subtitle_track_id.is_nil() {
      return Err(SubtitleCueError::NilSubtitleTrackId);
    }
    let image = image.into();
    validate_content(&text, &image, &ocr_text)?;
    Ok(Self {
      id,
      subtitle_track_id,
      index,
      span,
      text,
      styled_text: styled_text.into(),
      image,
      ocr_text,
    })
  }

  /// Convenience constructor for a text-based cue (no bitmap / OCR).
  ///
  /// Equivalent to [`Self::try_new`] with empty `image`/`ocr_text`;
  /// still rejects a blank `text` via [`SubtitleCueError::BlankCue`].
  pub fn try_new_text(
    id: Uuid7,
    subtitle_track_id: Uuid7,
    index: u32,
    span: TimeRange,
    text: LocalizedText,
  ) -> Result<Self, SubtitleCueError> {
    Self::try_new(
      id,
      subtitle_track_id,
      index,
      span,
      text,
      SmolStr::default(),
      Bytes::new(),
      LocalizedText::new(),
    )
  }
}

impl<Id> SubtitleCue<Id> {
  /// Canonical identity (also the LanceDB embedding key).
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// FK → `SubtitleTrack.id`.
  #[inline(always)]
  pub const fn subtitle_track_id_ref(&self) -> &Id {
    &self.subtitle_track_id
  }

  /// 0-based cue ordinal within the subtitle_track_id track.
  #[inline(always)]
  pub const fn index(&self) -> u32 {
    self.index
  }

  /// On-screen interval in media-time.
  #[inline(always)]
  pub const fn span_ref(&self) -> &TimeRange {
    &self.span
  }

  /// Parsed plain text (styling stripped); both `src` and `translated`
  /// fields `""` = absent.
  #[inline(always)]
  pub const fn text_ref(&self) -> &LocalizedText {
    &self.text
  }

  /// Original markup retained for render fidelity (ASS/SSA);
  /// `""` = none (string-rule: no `Option`).
  #[inline(always)]
  pub fn styled_text(&self) -> &str {
    self.styled_text.as_str()
  }

  /// Inline rendered cue bitmap (PGS/DVBSUB); empty = none.
  #[inline(always)]
  pub fn image(&self) -> &[u8] {
    &self.image
  }

  /// Text extracted from `image` by the OCR stage; kept distinct from
  /// `text`. Both `src` and `translated` fields `""` = absent.
  #[inline(always)]
  pub const fn ocr_text_ref(&self) -> &LocalizedText {
    &self.ocr_text
  }

  /// True when this cue carries no content of any kind — text, OCR
  /// text, and image all empty.
  ///
  /// A blank cue can no longer be *constructed* ([`Self::try_new`]
  /// rejects it, and every content mutator is fallible), so for a value
  /// obtained through the domain API this predicate is always `false`.
  /// It is kept as a defensive check for cues reconstructed by other
  /// means (e.g. a future deserializer that bypasses `try_new`).
  #[inline(always)]
  pub fn is_blank(&self) -> bool {
    self.text.is_empty() && self.ocr_text.is_empty() && self.image.is_empty()
  }

  // -------------------------------------------------------------------
  // Builders — invariant-free fields (`with_*`, consume self).
  // -------------------------------------------------------------------

  /// Builder: replace `index`.
  #[must_use]
  #[inline(always)]
  pub const fn with_index(mut self, v: u32) -> Self {
    self.index = v;
    self
  }

  /// Builder: replace `span`.
  #[must_use]
  #[inline(always)]
  pub const fn with_span(mut self, v: TimeRange) -> Self {
    self.span = v;
    self
  }

  /// Builder: replace `styled_text` (render-only; no invariant).
  #[must_use]
  #[inline(always)]
  pub fn with_styled_text(mut self, v: impl Into<SmolStr>) -> Self {
    self.styled_text = v.into();
    self
  }

  // -------------------------------------------------------------------
  // In-place setters — invariant-free fields (`set_*`, `&mut self`).
  // -------------------------------------------------------------------

  /// In-place mutator for `index`.
  #[inline(always)]
  pub const fn set_index(&mut self, v: u32) -> &mut Self {
    self.index = v;
    self
  }

  /// In-place mutator for `span`.
  #[inline(always)]
  pub const fn set_span(&mut self, v: TimeRange) -> &mut Self {
    self.span = v;
    self
  }

  /// In-place mutator for `styled_text` (render-only; no invariant).
  #[inline(always)]
  pub fn set_styled_text(&mut self, v: impl Into<SmolStr>) -> &mut Self {
    self.styled_text = v.into();
    self
  }

  // -------------------------------------------------------------------
  // Fallible builders / setters — content fields (`text`, `image`,
  // `ocr_text`) carry the cross-field invariants, so every mutator that
  // could break "non-empty" or "ocr_text ⇒ image" is fallible.
  // -------------------------------------------------------------------

  /// Fallible builder: replace `text`.
  ///
  /// Rejects the change when it would leave the cue fully blank
  /// ([`SubtitleCueError::BlankCue`]). On error the cue is consumed and
  /// not returned — mirrors [`Self::try_new`].
  #[inline]
  pub fn try_with_text(mut self, v: LocalizedText) -> Result<Self, SubtitleCueError> {
    validate_content(&v, &self.image, &self.ocr_text)?;
    self.text = v;
    Ok(self)
  }

  /// Fallible builder: replace `image`.
  ///
  /// Rejects the change when removing the image would leave `ocr_text`
  /// orphaned ([`SubtitleCueError::OcrTextWithoutImage`]) or the cue
  /// fully blank ([`SubtitleCueError::BlankCue`]).
  #[inline]
  pub fn try_with_image(mut self, v: impl Into<Bytes>) -> Result<Self, SubtitleCueError> {
    let image = v.into();
    validate_content(&self.text, &image, &self.ocr_text)?;
    self.image = image;
    Ok(self)
  }

  /// Fallible builder: replace `ocr_text`.
  ///
  /// Rejects non-empty `ocr_text` while `image` is empty
  /// ([`SubtitleCueError::OcrTextWithoutImage`]) and a change that
  /// would leave the cue fully blank ([`SubtitleCueError::BlankCue`]).
  #[inline]
  pub fn try_with_ocr_text(mut self, v: LocalizedText) -> Result<Self, SubtitleCueError> {
    validate_content(&self.text, &self.image, &v)?;
    self.ocr_text = v;
    Ok(self)
  }

  /// Fallible in-place mutator for `text`. See [`Self::try_with_text`].
  /// On error `self` is left unchanged.
  #[inline]
  pub fn try_set_text(&mut self, v: LocalizedText) -> Result<&mut Self, SubtitleCueError> {
    validate_content(&v, &self.image, &self.ocr_text)?;
    self.text = v;
    Ok(self)
  }

  /// Fallible in-place mutator for `image`. See [`Self::try_with_image`].
  /// On error `self` is left unchanged.
  #[inline]
  pub fn try_set_image(&mut self, v: impl Into<Bytes>) -> Result<&mut Self, SubtitleCueError> {
    let image = v.into();
    validate_content(&self.text, &image, &self.ocr_text)?;
    self.image = image;
    Ok(self)
  }

  /// Fallible in-place mutator for `ocr_text`. See
  /// [`Self::try_with_ocr_text`]. On error `self` is left unchanged.
  #[inline]
  pub fn try_set_ocr_text(&mut self, v: LocalizedText) -> Result<&mut Self, SubtitleCueError> {
    validate_content(&self.text, &self.image, &v)?;
    self.ocr_text = v;
    Ok(self)
  }
}

/// Error returned when [`SubtitleCue::try_new`] or a fallible content
/// mutator cannot uphold a cue invariant. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum SubtitleCueError {
  /// Supplied `id` was the nil sentinel — would collide as a LanceDB
  /// embedding key.
  #[error("SubtitleCue id must not be the nil UUID")]
  NilId,
  /// Supplied `subtitle_track_id` was the nil sentinel — orphan cue with no
  /// `SubtitleTrack` reference.
  #[error("SubtitleCue `subtitle_track_id` (FK → SubtitleTrack) must not be the nil UUID")]
  NilSubtitleTrackId,
  /// A cue must carry content — `text`, `ocr_text` and `image` cannot
  /// all be empty.
  #[error("SubtitleCue must be non-empty: text, ocr_text and image are all empty")]
  BlankCue,
  /// `ocr_text` is non-empty while `image` is empty — OCR text is
  /// derived from a bitmap, so it cannot exist without one.
  #[error("SubtitleCue ocr_text is set but image is empty (OCR text requires a bitmap)")]
  OcrTextWithoutImage,
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
  fn try_new_happy_path_text() {
    let subtitle_track_id = Uuid7::new();
    let c = SubtitleCue::try_new_text(
      Uuid7::new(),
      subtitle_track_id,
      0,
      span(),
      LocalizedText::from_src("Hello"),
    )
    .expect("valid text cue must construct");
    assert_eq!(c.subtitle_track_id_ref(), &subtitle_track_id);
    assert_eq!(c.index(), 0);
    assert_eq!(c.span_ref(), &span());
    assert_eq!(c.text_ref().src(), "Hello");
    assert!(c.styled_text().is_empty());
    assert!(c.image().is_empty());
    assert!(c.ocr_text_ref().is_empty());
    assert!(!c.is_blank());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = SubtitleCue::try_new_text(
      Uuid7::nil(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("x"),
    );
    assert_eq!(r.err(), Some(SubtitleCueError::NilId));
    assert!(SubtitleCueError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_subtitle_track_id() {
    let r = SubtitleCue::try_new_text(
      Uuid7::new(),
      Uuid7::nil(),
      0,
      span(),
      LocalizedText::from_src("x"),
    );
    assert_eq!(r.err(), Some(SubtitleCueError::NilSubtitleTrackId));
    assert!(SubtitleCueError::NilSubtitleTrackId.is_nil_subtitle_track_id());
  }

  #[test]
  fn try_new_rejects_fully_blank_cue() {
    let r = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::new(),
      "",
      Bytes::new(),
      LocalizedText::new(),
    );
    assert_eq!(r.err(), Some(SubtitleCueError::BlankCue));
    assert!(SubtitleCueError::BlankCue.is_blank_cue());
  }

  #[test]
  fn try_new_rejects_ocr_text_without_image() {
    let r = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::new(),
      "",
      Bytes::new(),
      LocalizedText::from_src("Hello (OCR)"),
    );
    assert_eq!(r.err(), Some(SubtitleCueError::OcrTextWithoutImage));
    assert!(SubtitleCueError::OcrTextWithoutImage.is_ocr_text_without_image());
  }

  #[test]
  fn try_new_bitmap_cue_with_image_and_ocr() {
    let bitmap = std::vec![0u8, 1, 2, 3];
    let c = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      5,
      span(),
      LocalizedText::new(),
      "",
      bitmap.clone(),
      LocalizedText::from_src("Hello (OCR)"),
    )
    .expect("bitmap cue with image + ocr_text must construct");
    assert_eq!(c.index(), 5);
    assert_eq!(c.image(), bitmap.as_slice());
    assert_eq!(c.ocr_text_ref().src(), "Hello (OCR)");
    assert!(!c.is_blank());
  }

  #[test]
  fn try_new_image_only_cue_is_valid() {
    // A bitmap cue before the OCR stage runs: image present, no text.
    let c = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::new(),
      "",
      std::vec![9u8],
      LocalizedText::new(),
    )
    .expect("image-only cue is non-blank and ocr_text is absent");
    assert_eq!(c.image(), &[9u8]);
    assert!(!c.is_blank());
  }

  #[test]
  fn invariant_free_builders_chain() {
    let c = SubtitleCue::try_new_text(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("hi"),
    )
    .unwrap()
    .with_index(5)
    .with_styled_text("{\\b1}hi{\\b0}");
    assert_eq!(c.index(), 5);
    assert_eq!(c.styled_text(), "{\\b1}hi{\\b0}");
  }

  #[test]
  fn try_with_text_rejects_blanking() {
    // Clearing `text` on a text-only cue would leave it fully blank.
    let c = SubtitleCue::try_new_text(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("hi"),
    )
    .unwrap();
    let r = c.try_with_text(LocalizedText::new());
    assert_eq!(r.err(), Some(SubtitleCueError::BlankCue));
  }

  #[test]
  fn try_with_ocr_text_rejects_without_image() {
    let c = SubtitleCue::try_new_text(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("hi"),
    )
    .unwrap();
    let r = c.try_with_ocr_text(LocalizedText::from_src("Hello (OCR)"));
    assert_eq!(r.err(), Some(SubtitleCueError::OcrTextWithoutImage));
  }

  #[test]
  fn try_with_image_rejects_orphaning_ocr_text() {
    // Removing the image while `ocr_text` is set must fail.
    let c = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::new(),
      "",
      std::vec![1u8],
      LocalizedText::from_src("Hello (OCR)"),
    )
    .unwrap();
    let r = c.try_with_image(Bytes::new());
    assert_eq!(r.err(), Some(SubtitleCueError::OcrTextWithoutImage));
  }

  #[test]
  fn try_with_image_then_ocr_text_builds_valid_bitmap_cue() {
    let c = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::new(),
      "",
      std::vec![7u8],
      LocalizedText::new(),
    )
    .unwrap()
    .try_with_ocr_text(LocalizedText::from_src("OCR"))
    .expect("ocr_text with an image present is valid");
    assert_eq!(c.image(), &[7u8]);
    assert_eq!(c.ocr_text_ref().src(), "OCR");
  }

  #[test]
  fn try_set_ocr_text_rejects_without_image_and_leaves_self_unchanged() {
    let mut c = SubtitleCue::try_new_text(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("hi"),
    )
    .unwrap();
    let r = c.try_set_ocr_text(LocalizedText::from_src("OCR"));
    assert_eq!(r.err(), Some(SubtitleCueError::OcrTextWithoutImage));
    assert!(
      c.ocr_text_ref().is_empty(),
      "rejected setter must not mutate"
    );
    assert_eq!(c.text_ref().src(), "hi");
  }

  #[test]
  fn try_set_text_rejects_blanking_and_leaves_self_unchanged() {
    let mut c = SubtitleCue::try_new_text(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("hi"),
    )
    .unwrap();
    let r = c.try_set_text(LocalizedText::new());
    assert_eq!(r.err(), Some(SubtitleCueError::BlankCue));
    assert_eq!(c.text_ref().src(), "hi", "rejected setter must not mutate");
  }

  #[test]
  fn try_set_image_then_ocr_text_succeeds() {
    let mut c = SubtitleCue::try_new_text(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("hi"),
    )
    .unwrap();
    c.try_set_image(std::vec![3u8])
      .expect("setting an image is always valid");
    c.try_set_ocr_text(LocalizedText::from_src("OCR"))
      .expect("ocr_text with image present is valid");
    assert_eq!(c.image(), &[3u8]);
    assert_eq!(c.ocr_text_ref().src(), "OCR");
  }

  #[test]
  fn try_set_image_can_clear_when_text_present() {
    // Clearing an image is fine while `text` keeps the cue non-blank
    // and `ocr_text` is empty.
    let mut c = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("hi"),
      "",
      std::vec![1u8],
      LocalizedText::new(),
    )
    .unwrap();
    c.try_set_image(Bytes::new())
      .expect("clearing image is valid while text keeps the cue non-blank");
    assert!(c.image().is_empty());
  }

  #[test]
  fn invariant_free_setters_mutate_in_place_and_chain() {
    let mut c = SubtitleCue::try_new_text(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("hi"),
    )
    .unwrap();
    c.set_index(7).set_styled_text("Bonjour");
    assert_eq!(c.index(), 7);
    assert_eq!(c.styled_text(), "Bonjour");
  }
}
