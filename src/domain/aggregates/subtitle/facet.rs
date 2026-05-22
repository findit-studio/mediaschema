//! `Subtitle<Id>` — the subtitle facet of a `Media` (locked
//! `schema/subtitle.md` r3). A **thin aggregate**: just the per-track id
//! list and the indexing roll-up. **No scalar metadata of its own** —
//! file-level data is on `Media`; per-track detail
//! (format/language/role/origin/codec) and analysis (cues/OCR/search)
//! live on [`SubtitleTrack`](super::SubtitleTrack).
//!
//! ### `IndexProgress`
//!
//! The shared `IndexProgress { total, indexed, failed }` rollup VO
//! (`schema/README.md` "Indexing model-correction") is defined
//! **locally in this module** for now. The same shape will be needed by
//! `Video` and `Audio` facets — those land in parallel PRs that may
//! introduce their own copies. Lifting `IndexProgress` to
//! `src/domain/vo.rs` is a tracked follow-up once the three facet PRs
//! have all merged (avoids cross-PR merge conflicts in this stacked
//! rollout).

use derive_more::IsVariant;

use crate::domain::Uuid7;

/// A `{ total, indexed, failed }` rollup of a kind container's child
/// tracks' indexing state. Denormalised cache, not the source of truth
/// (each track's `*IndexStatus` + `index_errors` is authoritative).
///
/// Invariants enforced by the projection layer, not this VO:
/// `indexed + failed <= total`. Constructing one with violating values
/// is not an aggregate-level error — it's a denormalisation bug to be
/// caught by the rollup recompute, not by `try_new`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IndexProgress {
  total: u32,
  indexed: u32,
  failed: u32,
}

impl IndexProgress {
  /// Canonical no-arg constructor — every field zero (no tracks, no
  /// progress). [`Default::default`] is `Self::new()`.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      total: 0,
      indexed: 0,
      failed: 0,
    }
  }

  /// Construct from the three counts directly.
  #[inline(always)]
  pub const fn from_parts(total: u32, indexed: u32, failed: u32) -> Self {
    Self {
      total,
      indexed,
      failed,
    }
  }

  /// Total child tracks.
  #[inline(always)]
  pub const fn total(&self) -> u32 {
    self.total
  }

  /// Tracks whose pipeline is fully indexed (per the kind-specific
  /// `is_fully_indexed` predicate).
  #[inline(always)]
  pub const fn indexed(&self) -> u32 {
    self.indexed
  }

  /// Tracks whose derived stage is `Failed` (any retained live error).
  /// Drives the `Media.error_flags.SUBTITLE_ERROR` bit.
  #[inline(always)]
  pub const fn failed(&self) -> u32 {
    self.failed
  }

  /// Builder: replace `total`.
  #[must_use]
  #[inline(always)]
  pub const fn with_total(mut self, total: u32) -> Self {
    self.total = total;
    self
  }

  /// Builder: replace `indexed`.
  #[must_use]
  #[inline(always)]
  pub const fn with_indexed(mut self, indexed: u32) -> Self {
    self.indexed = indexed;
    self
  }

  /// Builder: replace `failed`.
  #[must_use]
  #[inline(always)]
  pub const fn with_failed(mut self, failed: u32) -> Self {
    self.failed = failed;
    self
  }

  /// In-place mutator for `total`.
  #[inline(always)]
  pub const fn set_total(&mut self, total: u32) -> &mut Self {
    self.total = total;
    self
  }

  /// In-place mutator for `indexed`.
  #[inline(always)]
  pub const fn set_indexed(&mut self, indexed: u32) -> &mut Self {
    self.indexed = indexed;
    self
  }

  /// In-place mutator for `failed`.
  #[inline(always)]
  pub const fn set_failed(&mut self, failed: u32) -> &mut Self {
    self.failed = failed;
    self
  }
}

/// Subtitle facet of a `Media`. Generic over `Id` (default [`Uuid7`]).
///
/// **No `Default`** — a `Subtitle` with nil `id` would shadow the
/// `Media`'s real subtitle facet (one-to-one composition). Construct via
/// [`Subtitle::try_new`]. Fields are private per the encapsulation rule;
/// access via getters and `with_*` / `set_*` builders/mutators.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Subtitle<Id = Uuid7> {
  id: Id,
  parent: Id,
  tracks: std::vec::Vec<Id>,
  track_progress: IndexProgress,
}

impl Subtitle<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (the facet must be addressable from `Media`) and
  /// nil `parent` (orphaned facet with no `Media` reference). The
  /// `tracks` list starts empty and `track_progress` starts at zero —
  /// callers populate via `with_tracks` / `with_track_progress` once
  /// the per-track aggregates are landed.
  pub fn try_new(id: Uuid7, parent: Uuid7) -> Result<Self, SubtitleError> {
    if id.is_nil() {
      return Err(SubtitleError::NilId);
    }
    if parent.is_nil() {
      return Err(SubtitleError::NilParent);
    }
    Ok(Self {
      id,
      parent,
      tracks: std::vec::Vec::new(),
      track_progress: IndexProgress::new(),
    })
  }
}

impl<Id> Subtitle<Id> {
  /// Canonical identity (referenced by `Media.subtitle`).
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// FK → `Media.id`.
  #[inline(always)]
  pub const fn parent_ref(&self) -> &Id {
    &self.parent
  }

  /// Forward refs to child `SubtitleTrack`s.
  #[inline(always)]
  pub const fn tracks_slice(&self) -> &[Id] {
    self.tracks.as_slice()
  }

  /// Roll-up of each `SubtitleTrack`'s derived stage (denormalised
  /// cache; truth = per-track `SubtitleIndexStatus` + `index_errors`).
  #[inline(always)]
  pub const fn track_progress_ref(&self) -> &IndexProgress {
    &self.track_progress
  }

  /// Builder: replace `tracks`.
  #[must_use]
  #[inline(always)]
  pub fn with_tracks(mut self, tracks: impl Into<std::vec::Vec<Id>>) -> Self {
    self.tracks = tracks.into();
    self
  }

  /// Builder: replace `track_progress`.
  #[must_use]
  #[inline(always)]
  pub const fn with_track_progress(mut self, progress: IndexProgress) -> Self {
    self.track_progress = progress;
    self
  }

  /// In-place mutator for `tracks`.
  #[inline(always)]
  pub fn set_tracks(&mut self, tracks: impl Into<std::vec::Vec<Id>>) -> &mut Self {
    self.tracks = tracks.into();
    self
  }

  /// In-place mutator for `track_progress`.
  #[inline(always)]
  pub const fn set_track_progress(&mut self, progress: IndexProgress) -> &mut Self {
    self.track_progress = progress;
    self
  }
}

/// Error returned when [`Subtitle::try_new`] cannot uphold the
/// non-nil-id / non-nil-parent invariants. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum SubtitleError {
  /// Supplied `id` was the nil sentinel — would shadow the parent
  /// `Media`'s real subtitle facet.
  #[error("Subtitle id must not be the nil UUID")]
  NilId,
  /// Supplied `parent` was the nil sentinel — orphaned facet with no
  /// `Media` reference.
  #[error("Subtitle parent (Media) must not be the nil UUID")]
  NilParent,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  #[test]
  fn try_new_happy_path() {
    let parent = Uuid7::new();
    let s = Subtitle::try_new(Uuid7::new(), parent).expect("valid construction must succeed");
    assert_eq!(s.parent_ref(), &parent);
    assert!(s.tracks_slice().is_empty());
    assert_eq!(s.track_progress_ref(), &IndexProgress::new());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = Subtitle::try_new(Uuid7::nil(), Uuid7::new());
    assert_eq!(r.err(), Some(SubtitleError::NilId));
    assert!(SubtitleError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_parent() {
    let r = Subtitle::try_new(Uuid7::new(), Uuid7::nil());
    assert_eq!(r.err(), Some(SubtitleError::NilParent));
    assert!(SubtitleError::NilParent.is_nil_parent());
  }

  #[test]
  fn builders_and_setters_chain() {
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let s = Subtitle::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![t1, t2])
      .with_track_progress(IndexProgress::from_parts(2, 1, 0));
    assert_eq!(s.tracks_slice().len(), 2);
    assert!(s.tracks_slice().contains(&t1));
    assert!(s.tracks_slice().contains(&t2));
    assert_eq!(s.track_progress_ref().total(), 2);
    assert_eq!(s.track_progress_ref().indexed(), 1);
    assert_eq!(s.track_progress_ref().failed(), 0);
  }

  #[test]
  fn setters_mutate_in_place() {
    let mut s = Subtitle::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    s.set_tracks(std::vec![Uuid7::new()]);
    s.set_track_progress(IndexProgress::from_parts(1, 0, 1));
    assert_eq!(s.tracks_slice().len(), 1);
    assert_eq!(s.track_progress_ref().failed(), 1);
  }

  #[test]
  fn index_progress_builders_and_setters() {
    let p = IndexProgress::new()
      .with_total(5)
      .with_indexed(3)
      .with_failed(1);
    assert_eq!(p.total(), 5);
    assert_eq!(p.indexed(), 3);
    assert_eq!(p.failed(), 1);

    let mut p = p;
    p.set_total(10);
    p.set_indexed(7);
    p.set_failed(2);
    assert_eq!(p.total(), 10);
    assert_eq!(p.indexed(), 7);
    assert_eq!(p.failed(), 2);
  }
}
