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
/// `id()` / `name()` / `color()` / `created_at()` getters and
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
  #[inline]
  pub fn id(&self) -> &Id {
    &self.id
  }

  /// Display name. Unique-by-`name` is a projection-side concern
  /// (case-folded comparison etc.).
  #[inline]
  pub fn name(&self) -> &str {
    self.name.as_str()
  }

  /// Optional swatch for UI.
  #[inline]
  pub const fn color(&self) -> Option<Rgba> {
    self.color
  }

  /// When the tag was created.
  #[inline]
  pub const fn created_at(&self) -> &Timestamp {
    &self.created_at
  }

  /// Builder: replace `name`.
  #[inline]
  pub fn with_name(mut self, name: impl Into<SmolStr>) -> Self {
    self.name = name.into();
    self
  }

  /// Builder: replace `color`.
  #[inline]
  pub const fn with_color(mut self, color: Option<Rgba>) -> Self {
    self.color = color;
    self
  }

  /// In-place mutator for `name`.
  #[inline]
  pub fn set_name(&mut self, name: impl Into<SmolStr>) {
    self.name = name.into();
  }

  /// In-place mutator for `color`.
  #[inline]
  pub const fn set_color(&mut self, color: Option<Rgba>) {
    self.color = color;
  }
}

/// Error returned when an aggregate's `id` is the nil sentinel — a real
/// identity is always required at the domain edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant)]
#[non_exhaustive]
pub enum NilIdError {
  /// The only failure mode: nil id.
  Nil,
}

impl Default for NilIdError {
  #[inline]
  fn default() -> Self {
    Self::Nil
  }
}

impl core::fmt::Display for NilIdError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.write_str("id must not be the nil UUID")
  }
}

impl core::error::Error for NilIdError {}

/// User curation of one `Scene`. Absence of any `SceneAnnotation` for a
/// scene means default (not favourite, no tags, unrated, no note).
///
/// Fields are private; access via getters + `with_*` / `set_*`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneAnnotation<Id = Uuid7> {
  id: Id,
  scene: Id,
  favorite: bool,
  user_tags: Vec<Id>,
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
    scene: Uuid7,
    updated_at: Timestamp,
  ) -> Result<Self, SceneAnnotationError> {
    if id.is_nil() {
      return Err(SceneAnnotationError::NilId);
    }
    if scene.is_nil() {
      return Err(SceneAnnotationError::NilScene);
    }
    Ok(Self {
      id,
      scene,
      favorite: false,
      user_tags: Vec::new(),
      rating: None,
      note: SmolStr::default(),
      updated_at,
    })
  }
}

impl<Id> SceneAnnotation<Id> {
  /// Canonical identity.
  #[inline]
  pub fn id(&self) -> &Id {
    &self.id
  }

  /// FK → `Scene.id`.
  #[inline]
  pub fn scene(&self) -> &Id {
    &self.scene
  }

  /// Whether the scene is favourited.
  #[inline]
  pub const fn is_favorite(&self) -> bool {
    self.favorite
  }

  /// References to `UserTag.id`s (not inline strings — supports
  /// rename / dedup via the tag aggregate).
  #[inline]
  pub fn user_tags(&self) -> &[Id] {
    &self.user_tags
  }

  /// `None` = unrated; otherwise typically 0–5 (range enforced by the
  /// projection).
  #[inline]
  pub const fn rating(&self) -> Option<u8> {
    self.rating
  }

  /// Free-text note; `""` = none (locked: no `Option` for `SmolStr`).
  #[inline]
  pub fn note(&self) -> &str {
    self.note.as_str()
  }

  /// When the annotation was last updated.
  #[inline]
  pub const fn updated_at(&self) -> &Timestamp {
    &self.updated_at
  }

  /// Builder: replace `favorite`.
  #[inline]
  pub const fn with_favorite(mut self, favorite: bool) -> Self {
    self.favorite = favorite;
    self
  }

  /// Builder: replace `user_tags`.
  #[inline]
  pub fn with_user_tags(mut self, tags: impl Into<Vec<Id>>) -> Self {
    self.user_tags = tags.into();
    self
  }

  /// Builder: replace `rating`.
  #[inline]
  pub const fn with_rating(mut self, rating: Option<u8>) -> Self {
    self.rating = rating;
    self
  }

  /// Builder: replace `note`.
  #[inline]
  pub fn with_note(mut self, note: impl Into<SmolStr>) -> Self {
    self.note = note.into();
    self
  }

  /// In-place mutator for `favorite`.
  #[inline]
  pub const fn set_favorite(&mut self, favorite: bool) {
    self.favorite = favorite;
  }

  /// In-place mutator for `user_tags`.
  #[inline]
  pub fn set_user_tags(&mut self, tags: impl Into<Vec<Id>>) {
    self.user_tags = tags.into();
  }

  /// In-place mutator for `rating`.
  #[inline]
  pub const fn set_rating(&mut self, rating: Option<u8>) {
    self.rating = rating;
  }

  /// In-place mutator for `note`.
  #[inline]
  pub fn set_note(&mut self, note: impl Into<SmolStr>) {
    self.note = note.into();
  }
}

/// Error returned when [`SceneAnnotation::try_new`] cannot uphold the
/// non-nil-id / non-nil-scene invariants. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant)]
#[non_exhaustive]
pub enum SceneAnnotationError {
  /// Supplied `id` was the nil sentinel.
  NilId,
  /// Supplied `scene` FK was nil — would be an orphan annotation with
  /// no `Scene` reference.
  NilScene,
}

impl core::fmt::Display for SceneAnnotationError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::NilId => f.write_str("SceneAnnotation id must not be the nil UUID"),
      Self::NilScene => f.write_str("SceneAnnotation scene FK must not be the nil UUID"),
    }
  }
}

impl core::error::Error for SceneAnnotationError {}

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
    assert!(a.user_tags().is_empty());
    assert!(a.rating().is_none());
    assert!(a.note().is_empty());
  }

  #[test]
  fn scene_annotation_try_new_rejects_nil_id_or_scene() {
    assert_eq!(
      SceneAnnotation::try_new(Uuid7::nil(), Uuid7::new(), Timestamp::default()).err(),
      Some(SceneAnnotationError::NilId)
    );
    assert_eq!(
      SceneAnnotation::try_new(Uuid7::new(), Uuid7::nil(), Timestamp::default()).err(),
      Some(SceneAnnotationError::NilScene)
    );
    assert!(SceneAnnotationError::NilId.is_nil_id());
    assert!(SceneAnnotationError::NilScene.is_nil_scene());
  }

  #[test]
  fn scene_annotation_tags_are_id_refs_not_strings() {
    let scene = Uuid7::new();
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let a = SceneAnnotation::try_new(Uuid7::new(), scene, Timestamp::default())
      .unwrap()
      .with_favorite(true)
      .with_user_tags(vec![t1, t2])
      .with_rating(Some(4))
      .with_note("great driving scene");
    assert_eq!(a.scene(), &scene);
    assert!(a.is_favorite());
    assert_eq!(a.rating(), Some(4));
    assert_eq!(a.user_tags().len(), 2);
    assert!(a.user_tags().contains(&t1));
    assert!(a.user_tags().contains(&t2));
    assert_eq!(a.note(), "great driving scene");
  }
}
