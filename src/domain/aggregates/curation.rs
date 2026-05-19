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

impl<Id: Default> Default for UserTag<Id> {
    fn default() -> Self {
        Self {
            id: Id::default(),
            name: SmolStr::default(),
            color: None,
            created_at: Timestamp::default(),
        }
    }
}

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

impl<Id: Default> Default for SceneAnnotation<Id> {
    fn default() -> Self {
        Self {
            id: Id::default(),
            scene: Id::default(),
            favorite: false,
            user_tags: Vec::new(),
            rating: None,
            note: SmolStr::default(),
            updated_at: Timestamp::default(),
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
    fn user_tag_default_has_no_colour_and_no_name() {
        let t: UserTag = UserTag::default();
        assert!(t.id.is_nil());
        assert!(t.name.is_empty());
        assert!(t.color.is_none());
    }

    #[test]
    fn user_tag_with_colour() {
        let t: UserTag = UserTag {
            id: Uuid7::new(),
            name: "Vacation".into(),
            color: Some(Rgba::new(0xff, 0x88, 0x00, 0xff)),
            created_at: Timestamp::default(),
        };
        assert_eq!(t.name.as_str(), "Vacation");
        let c = t.color.expect("colour");
        assert_eq!(c.r(), 0xff);
        assert_eq!(c.g(), 0x88);
    }

    #[test]
    fn scene_annotation_default_is_pristine() {
        let a: SceneAnnotation = SceneAnnotation::default();
        assert!(!a.favorite);
        assert!(a.user_tags.is_empty());
        assert!(a.rating.is_none());
        assert!(a.note.is_empty());
    }

    #[test]
    fn scene_annotation_tags_are_id_refs_not_strings() {
        let scene = Uuid7::new();
        let t1 = Uuid7::new();
        let t2 = Uuid7::new();
        let a: SceneAnnotation = SceneAnnotation {
            id: Uuid7::new(),
            scene,
            favorite: true,
            user_tags: vec![t1, t2],
            rating: Some(4),
            note: "great driving scene".into(),
            updated_at: Timestamp::default(),
        };
        assert_eq!(a.scene, scene);
        assert!(a.favorite);
        assert_eq!(a.rating, Some(4));
        assert_eq!(a.user_tags.len(), 2);
        assert!(a.user_tags.contains(&t1));
        assert!(a.user_tags.contains(&t2));
    }

    #[test]
    fn scene_annotation_empty_note_is_absent() {
        // SmolStr ""=absent (locked); no Option<SmolStr>.
        let a: SceneAnnotation = SceneAnnotation {
            note: "".into(),
            ..SceneAnnotation::default()
        };
        assert!(a.note.is_empty());
    }
}
