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
//! (`schema/README.md` "Indexing model-correction") is the canonical
//! definition in [`crate::domain::vo`], reused here by every facet.

use std::vec::Vec;

use derive_more::IsVariant;

use crate::domain::{vo::IndexProgress, Uuid7};

/// Subtitle facet of a `Media`. Generic over `Id` (default [`Uuid7`]).
///
/// **No `Default`** — a `Subtitle` with nil `id` would shadow the
/// `Media`'s real subtitle facet (one-to-one composition). Construct via
/// [`Subtitle::try_new`]. Fields are private per the encapsulation rule;
/// access via getters and `with_*` / `set_*` builders/mutators.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Subtitle<Id = Uuid7> {
  id: Id,
  media_id: Id,
  tracks: Vec<Id>,
  track_progress: IndexProgress,
}

impl Subtitle<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (the facet must be addressable from `Media`) and
  /// nil `media_id` (orphaned facet with no `Media` reference). The
  /// `tracks` list starts empty and `track_progress` starts at zero —
  /// callers populate via `with_tracks` / `with_track_progress` once
  /// the per-track aggregates are landed.
  pub fn try_new(id: Uuid7, media_id: Uuid7) -> Result<Self, SubtitleError> {
    if id.is_nil() {
      return Err(SubtitleError::NilId);
    }
    if media_id.is_nil() {
      return Err(SubtitleError::NilMediaId);
    }
    Ok(Self {
      id,
      media_id,
      tracks: Vec::new(),
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
  pub const fn media_id_ref(&self) -> &Id {
    &self.media_id
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
  pub fn with_tracks(mut self, tracks: impl Into<Vec<Id>>) -> Self {
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
  pub fn set_tracks(&mut self, tracks: impl Into<Vec<Id>>) -> &mut Self {
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
/// non-nil-id / non-nil-media_id invariants. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum SubtitleError {
  /// Supplied `id` was the nil sentinel — would shadow the media_id
  /// `Media`'s real subtitle facet.
  #[error("Subtitle id must not be the nil UUID")]
  NilId,
  /// Supplied `media_id` was the nil sentinel — orphaned facet with no
  /// `Media` reference.
  #[error("Subtitle `media_id` (FK → Media) must not be the nil UUID")]
  NilMediaId,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  #[test]
  fn try_new_happy_path() {
    let media_id = Uuid7::new();
    let s = Subtitle::try_new(Uuid7::new(), media_id).expect("valid construction must succeed");
    assert_eq!(s.media_id_ref(), &media_id);
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
  fn try_new_rejects_nil_media_id() {
    let r = Subtitle::try_new(Uuid7::new(), Uuid7::nil());
    assert_eq!(r.err(), Some(SubtitleError::NilMediaId));
    assert!(SubtitleError::NilMediaId.is_nil_media_id());
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

/// Exhaustive by-value decomposition of [`Subtitle`] — every stored
/// field.
///
/// Public-field data-transfer struct (the conversion-boundary exception
/// to the encapsulation rule): cross-suite conversions (`crate::graph`)
/// destructure it exhaustively, so adding a field breaks them at compile
/// time instead of silently dropping data.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleParts<Id = Uuid7> {
  pub id: Id,
  pub media_id: Id,
  pub tracks: Vec<Id>,
  pub track_progress: IndexProgress,
}

impl<Id> Subtitle<Id> {
  /// Decompose into [`SubtitleParts`] — exhaustive, by value.
  #[inline(always)]
  pub fn into_parts(self) -> SubtitleParts<Id> {
    let Self {
      id,
      media_id,
      tracks,
      track_progress,
    } = self;
    SubtitleParts {
      id,
      media_id,
      tracks,
      track_progress,
    }
  }
}

impl<Id> Subtitle<Id> {
  /// Invariant-carrying constructor from [`SubtitleParts`] —
  /// `pub(crate)`, reserved for in-crate conversions from
  /// already-validated values (`crate::graph`).
  #[cfg(all(
    feature = "std",
    feature = "video",
    feature = "audio",
    feature = "subtitle"
  ))]
  #[inline(always)]
  pub(crate) fn rehydrate(parts: SubtitleParts<Id>) -> Self {
    let SubtitleParts {
      id,
      media_id,
      tracks,
      track_progress,
    } = parts;
    Self {
      id,
      media_id,
      tracks,
      track_progress,
    }
  }
}
