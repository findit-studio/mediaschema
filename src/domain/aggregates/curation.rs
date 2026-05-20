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

use jiff::Timestamp;
use smol_str::SmolStr;

use crate::domain::{Rgba, Uuid7};

/// A user-defined tag.
///
/// Distinct from VLM `Keyframe.VlmAnalysis.tags` (machine per-aggregate
/// label vectors); `UserTag`s are first-class entities — renameable,
/// recolourable, deduped via `id` rather than string equality.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserTag<Id = Uuid7> {
  pub id: Id,
  /// Display name. Unique-by-`name` is a projection-side concern
  /// (case-folded comparison etc.).
  pub name: SmolStr,
  /// Optional swatch for UI.
  pub color: Option<Rgba>,
  pub created_at: Timestamp,
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
      return Err(NilIdError);
    }
    Ok(Self {
      id,
      name: name.into(),
      color: None,
      created_at,
    })
  }
}

/// Error returned when an aggregate's `id` is the nil sentinel — a real
/// identity is always required at the domain edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NilIdError;

impl core::fmt::Display for NilIdError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.write_str("id must not be the nil UUID")
  }
}

#[cfg(feature = "std")]
impl std::error::Error for NilIdError {}

/// User curation of one `Scene`. Absence of any `SceneAnnotation` for a
/// scene means default (not favourite, no tags, unrated, no note).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneAnnotation<Id = Uuid7> {
  pub id: Id,
  /// FK → `Scene.id`.
  pub scene: Id,
  pub favorite: bool,
  /// References to `UserTag.id`s (not inline strings — supports
  /// rename / dedup via the tag aggregate).
  pub user_tags: Vec<Id>,
  /// `None` = unrated; otherwise typically 0–5 (range enforced by the
  /// projection).
  pub rating: Option<u8>,
  /// Free-text note; `""` = none (locked: no `Option` for `SmolStr`).
  pub note: SmolStr,
  pub updated_at: Timestamp,
}

impl SceneAnnotation<Uuid7> {
  /// Validating constructor — rejects nil `id` and nil `scene`
  /// (orphan-annotation guard). All curation state starts at "pristine"
  /// (no favourite, no tags, unrated, no note) and is mutated via
  /// field assignment.
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

/// Error returned when [`SceneAnnotation::try_new`] cannot uphold the
/// non-nil-id / non-nil-scene invariants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[cfg(feature = "std")]
impl std::error::Error for SceneAnnotationError {}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  #[test]
  fn user_tag_try_new_happy_path() {
    let t = UserTag::try_new(Uuid7::new(), "Vacation", Timestamp::default()).unwrap();
    assert_eq!(t.name.as_str(), "Vacation");
    assert!(t.color.is_none());
  }

  #[test]
  fn user_tag_try_new_rejects_nil_id() {
    assert_eq!(
      UserTag::try_new(Uuid7::nil(), "x", Timestamp::default()).err(),
      Some(NilIdError)
    );
  }

  #[test]
  fn user_tag_colour_is_user_assigned() {
    let mut t = UserTag::try_new(Uuid7::new(), "Vacation", Timestamp::default()).unwrap();
    t.color = Some(Rgba::new(0xff, 0x88, 0x00, 0xff));
    let c = t.color.expect("colour");
    assert_eq!(c.r(), 0xff);
    assert_eq!(c.g(), 0x88);
  }

  #[test]
  fn scene_annotation_try_new_is_pristine() {
    let a = SceneAnnotation::try_new(Uuid7::new(), Uuid7::new(), Timestamp::default()).unwrap();
    assert!(!a.favorite);
    assert!(a.user_tags.is_empty());
    assert!(a.rating.is_none());
    assert!(a.note.is_empty());
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
  }

  #[test]
  fn scene_annotation_tags_are_id_refs_not_strings() {
    let scene = Uuid7::new();
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let mut a = SceneAnnotation::try_new(Uuid7::new(), scene, Timestamp::default()).unwrap();
    a.favorite = true;
    a.user_tags = vec![t1, t2];
    a.rating = Some(4);
    a.note = "great driving scene".into();
    assert_eq!(a.scene, scene);
    assert!(a.favorite);
    assert_eq!(a.rating, Some(4));
    assert_eq!(a.user_tags.len(), 2);
    assert!(a.user_tags.contains(&t1));
    assert!(a.user_tags.contains(&t2));
  }
}
