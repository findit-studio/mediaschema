//! Smart-folder / user-curation aggregates.
//!
//! Source doc: `schema/smart_folder.md` r1 — **in-review** (`SmartFolder`
//! and `Collection` are deferred pending the SF-filter AST design). This
//! module ships the two stable, design-complete pieces: [`UserTag`] and
//! [`SceneAnnotation`].
//!
//! User-curation is a **separate mutable layer** over the immutable
//! detected+analysed aggregates: `Scene`/`Keyframe` carry no curation
//! fields (locked), everything user-editable lives here. Smart folders
//! target `Scene` (locked SF-target = scene-level).

use derive_more::IsVariant;
use jiff::Timestamp;
use smol_str::SmolStr;

use crate::domain::{Rgba, Uuid7};

/// A user-defined tag.
///
/// Distinct from VLM `Keyframe.VlmAnalysis.tags` (machine per-aggregate
/// label vectors); `UserTag`s are first-class entities — renameable,
/// recolourable, deduped via `id` rather than string equality.
///
/// Fields are private per the encapsulation rule; access via
/// `id_ref()` / `name()` / `color()` / `created_at_ref()` getters and
/// `with_*` / `set_*` builders/mutators.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserTag<Id = Uuid7> {
  id: Id,
  name: SmolStr,
  color: Option<Rgba>,
  created_at: Timestamp,
}

impl UserTag<Uuid7> {
  /// Construct a freshly-created tag (nil-id rejected — every row needs
  /// a real identity). The colour is unset until the user picks one.
  pub fn try_new(
    id: Uuid7,
    name: impl Into<SmolStr>,
    created_at: Timestamp,
  ) -> Result<Self, NilIdError> {
    if id.is_nil() {
      return Err(NilIdError::Nil);
    }
    Ok(Self {
      id,
      name: name.into(),
      color: None,
      created_at,
    })
  }
}

impl<Id> UserTag<Id> {
  /// Canonical identity.
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// Display name. Unique-by-`name` is a projection-side concern
  /// (case-folded comparison etc.).
  #[inline(always)]
  pub fn name(&self) -> &str {
    self.name.as_str()
  }

  /// Optional swatch for UI.
  #[inline(always)]
  pub const fn color(&self) -> Option<Rgba> {
    self.color
  }

  /// When the tag was created.
  #[inline(always)]
  pub const fn created_at_ref(&self) -> &Timestamp {
    &self.created_at
  }

  /// Builder: replace `name`.
  #[inline(always)]
  #[must_use]
  pub fn with_name(mut self, name: impl Into<SmolStr>) -> Self {
    self.name = name.into();
    self
  }

  /// Builder: replace `color`.
  #[inline(always)]
  #[must_use]
  pub const fn with_color(mut self, color: Option<Rgba>) -> Self {
    self.color = color;
    self
  }

  /// In-place mutator for `name`.
  #[inline(always)]
  pub fn set_name(&mut self, name: impl Into<SmolStr>) -> &mut Self {
    self.name = name.into();
    self
  }

  /// In-place mutator for `color`.
  #[inline(always)]
  pub const fn set_color(&mut self, color: Option<Rgba>) -> &mut Self {
    self.color = color;
    self
  }
}

/// Error returned when an aggregate's `id` is the nil sentinel — a real
/// identity is always required at the domain edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum NilIdError {
  /// The only failure mode: nil id.
  #[error("id must not be the nil UUID")]
  Nil,
}

impl Default for NilIdError {
  #[inline]
  fn default() -> Self {
    Self::Nil
  }
}

/// User curation of one `Scene`. Absence of any `SceneAnnotation` for a
/// scene means default (not favourite, no tags, unrated, no note).
///
/// Fields are private; access via getters + `with_*` / `set_*`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneAnnotation<Id = Uuid7> {
  id: Id,
  scene_id: Id,
  favorite: bool,
  user_tags: std::vec::Vec<Id>,
  rating: Option<u8>,
  note: SmolStr,
  updated_at: Timestamp,
}

impl SceneAnnotation<Uuid7> {
  /// Validating constructor — rejects nil `id` and nil `scene`
  /// (orphan-annotation guard). All curation state starts at "pristine"
  /// (no favourite, no tags, unrated, no note) and is mutated via
  /// `with_*` / `set_*`.
  pub fn try_new(
    id: Uuid7,
    scene_id: Uuid7,
    updated_at: Timestamp,
  ) -> Result<Self, SceneAnnotationError> {
    if id.is_nil() {
      return Err(SceneAnnotationError::NilId);
    }
    if scene_id.is_nil() {
      return Err(SceneAnnotationError::NilSceneId);
    }
    Ok(Self {
      id,
      scene_id,
      favorite: false,
      user_tags: std::vec::Vec::new(),
      rating: None,
      note: SmolStr::default(),
      updated_at,
    })
  }
}

impl<Id> SceneAnnotation<Id> {
  /// Canonical identity.
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// FK → `Scene.id`.
  #[inline(always)]
  pub const fn scene_id_ref(&self) -> &Id {
    &self.scene_id
  }

  /// Whether the scene is favourited.
  #[inline(always)]
  pub const fn is_favorite(&self) -> bool {
    self.favorite
  }

  /// References to `UserTag.id`s (not inline strings — supports
  /// rename / dedup via the tag aggregate).
  #[inline(always)]
  pub const fn user_tags_slice(&self) -> &[Id] {
    // `Vec::as_slice` is const fn; deref-coercion (`&self.user_tags`)
    // would require the non-const `Deref` impl.
    self.user_tags.as_slice()
  }

  /// `None` = unrated; otherwise typically 0–5 (range enforced by the
  /// projection).
  #[inline(always)]
  pub const fn rating(&self) -> Option<u8> {
    self.rating
  }

  /// Free-text note; `""` = none (locked: no `Option` for `SmolStr`).
  #[inline(always)]
  pub fn note(&self) -> &str {
    self.note.as_str()
  }

  /// When the annotation was last updated.
  #[inline(always)]
  pub const fn updated_at_ref(&self) -> &Timestamp {
    &self.updated_at
  }

  /// Builder: replace `favorite`.
  #[inline(always)]
  #[must_use]
  pub const fn with_favorite(mut self, favorite: bool) -> Self {
    self.favorite = favorite;
    self
  }

  /// Builder: replace `user_tags`.
  #[inline(always)]
  #[must_use]
  pub fn with_user_tags(mut self, tags: impl Into<std::vec::Vec<Id>>) -> Self {
    self.user_tags = tags.into();
    self
  }

  /// Builder: replace `rating`.
  #[inline(always)]
  #[must_use]
  pub const fn with_rating(mut self, rating: Option<u8>) -> Self {
    self.rating = rating;
    self
  }

  /// Builder: replace `note`.
  #[inline(always)]
  #[must_use]
  pub fn with_note(mut self, note: impl Into<SmolStr>) -> Self {
    self.note = note.into();
    self
  }

  /// In-place mutator for `favorite`.
  #[inline(always)]
  pub const fn set_favorite(&mut self, favorite: bool) -> &mut Self {
    self.favorite = favorite;
    self
  }

  /// In-place mutator for `user_tags`.
  #[inline(always)]
  pub fn set_user_tags(&mut self, tags: impl Into<std::vec::Vec<Id>>) -> &mut Self {
    self.user_tags = tags.into();
    self
  }

  /// In-place mutator for `rating`.
  #[inline(always)]
  pub const fn set_rating(&mut self, rating: Option<u8>) -> &mut Self {
    self.rating = rating;
    self
  }

  /// In-place mutator for `note`.
  #[inline(always)]
  pub fn set_note(&mut self, note: impl Into<SmolStr>) -> &mut Self {
    self.note = note.into();
    self
  }
}

/// Error returned when [`SceneAnnotation::try_new`] cannot uphold the
/// non-nil-id / non-nil-scene invariants. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum SceneAnnotationError {
  /// Supplied `id` was the nil sentinel.
  #[error("SceneAnnotation id must not be the nil UUID")]
  NilId,
  /// Supplied `scene` FK was nil — would be an orphan annotation with
  /// no `Scene` reference.
  #[error("SceneAnnotation `scene_id` (FK → Scene) must not be the nil UUID")]
  NilSceneId,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  #[test]
  fn user_tag_try_new_happy_path() {
    let t = UserTag::try_new(Uuid7::new(), "Vacation", Timestamp::default()).unwrap();
    assert_eq!(t.name(), "Vacation");
    assert!(t.color().is_none());
  }

  #[test]
  fn user_tag_try_new_rejects_nil_id() {
    let err = UserTag::try_new(Uuid7::nil(), "x", Timestamp::default()).unwrap_err();
    assert_eq!(err, NilIdError::Nil);
    assert!(err.is_nil());
  }

  #[test]
  fn user_tag_colour_via_builder_and_setter() {
    let t = UserTag::try_new(Uuid7::new(), "Vacation", Timestamp::default())
      .unwrap()
      .with_color(Some(Rgba::from_components(0xff, 0x88, 0x00, 0xff)));
    let c = t.color().expect("colour");
    assert_eq!(c.r(), 0xff);
    assert_eq!(c.g(), 0x88);
    let mut t = t;
    t.set_color(None);
    assert!(t.color().is_none());
  }

  #[test]
  fn scene_annotation_try_new_is_pristine() {
    let a = SceneAnnotation::try_new(Uuid7::new(), Uuid7::new(), Timestamp::default()).unwrap();
    assert!(!a.is_favorite());
    assert!(a.user_tags_slice().is_empty());
    assert!(a.rating().is_none());
    assert!(a.note().is_empty());
  }

  #[test]
  fn scene_annotation_try_new_rejects_nil_id_or_scene_id() {
    assert_eq!(
      SceneAnnotation::try_new(Uuid7::nil(), Uuid7::new(), Timestamp::default()).err(),
      Some(SceneAnnotationError::NilId)
    );
    assert_eq!(
      SceneAnnotation::try_new(Uuid7::new(), Uuid7::nil(), Timestamp::default()).err(),
      Some(SceneAnnotationError::NilSceneId)
    );
    assert!(SceneAnnotationError::NilId.is_nil_id());
    assert!(SceneAnnotationError::NilSceneId.is_nil_scene_id());
  }

  #[test]
  fn scene_annotation_tags_are_id_refs_not_strings() {
    let scene_id = Uuid7::new();
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let a = SceneAnnotation::try_new(Uuid7::new(), scene_id, Timestamp::default())
      .unwrap()
      .with_favorite(true)
      .with_user_tags(std::vec![t1, t2])
      .with_rating(Some(4))
      .with_note("great driving scene");
    assert_eq!(a.scene_id_ref(), &scene_id);
    assert!(a.is_favorite());
    assert_eq!(a.rating(), Some(4));
    assert_eq!(a.user_tags_slice().len(), 2);
    assert!(a.user_tags_slice().contains(&t1));
    assert!(a.user_tags_slice().contains(&t2));
    assert_eq!(a.note(), "great driving scene");
  }
}
