//! `Video` — thin video facet aggregate (locked `schema/video.md` r8).
//!
//! Groups the file's video tracks + the per-kind indexing roll-up. **No
//! scalar stream metadata** of its own — file-level lives on `Media`,
//! stream/scene-level on `VideoTrack` (locked three-level data placement
//! rule). `scenes` was removed in rev 8 → moved to `VideoTrack.scenes`
//! (scene detection is per video stream); the facet keeps only the
//! `total_scenes` denormalized rollup.
//!
//! ### Validation-responsibility boundary
//!
//! The facet stores `tracks` (id refs), `total_scenes`
//! (`Σ VideoTrack.scenes.len()`), and `track_progress` (per-kind rollup
//! over `tracks`) as **independent fields**. Keeping them consistent —
//! e.g. `track_progress.total() == tracks.len()`, or
//! `tracks.is_empty() ⇒ total_scenes == 0` — is a
//! cross-field/rollup-coupling concern owned by the application /
//! storage layer (the database is the source of truth for rollups; the
//! domain type is rebuilt from a row without filler-synthesizing a
//! `tracks` Vec to satisfy a derived count). The domain type enforces
//! only intrinsic single-value invariants (here: non-nil `id`).

use derive_more::IsVariant;

use crate::domain::{vo::IndexProgress, Uuid7};

// ---------------------------------------------------------------------------
// Video — the thin facet aggregate
// ---------------------------------------------------------------------------

/// The video facet of a `Media`. Holds the facet identity, child-track
/// id list, the `total_scenes` rollup, and the per-kind index progress.
/// **Generic over `Id`** (default [`Uuid7`]) — the parent FK lives on
/// `Media.video`, and each `VideoTrack` carries the back-reference.
///
/// Fields are private per the encapsulation rule; access via the
/// `id_ref` / `total_scenes` / `tracks_slice` / `track_progress_ref`
/// getters and the `with_*` / `set_*` builders/mutators.
///
/// **No `Default`** — defaulting to a nil id would be indistinguishable
/// from a real "video facet with unset id" record. Construct via
/// [`Video::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Video<Id = Uuid7> {
  id: Id,
  total_scenes: u32,
  tracks: std::vec::Vec<Id>,
  track_progress: IndexProgress,
}

impl Video<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  /// Rejects nil `id` (every facet row needs a real identity).
  ///
  /// The facet starts with no tracks (`tracks = []`), no scenes
  /// (`total_scenes = 0`), and an empty index-progress rollup; the
  /// indexer fills these in via `with_*` / `set_*` as tracks are
  /// created and processed — or the storage layer assembles them
  /// directly from a row.
  pub fn try_new(id: Uuid7) -> Result<Self, VideoError> {
    if id.is_nil() {
      return Err(VideoError::NilId);
    }
    Ok(Self {
      id,
      total_scenes: 0,
      tracks: std::vec::Vec::new(),
      track_progress: IndexProgress::new(),
    })
  }
}

impl<Id> Video<Id> {
  /// Canonical identity (referenced by `Media.video`; child
  /// `VideoTrack.parent` points here).
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// Total scenes across all child tracks — denormalised rollup
  /// (`Σ over its VideoTracks of scenes.len()`).
  #[inline(always)]
  pub const fn total_scenes(&self) -> u32 {
    self.total_scenes
  }

  /// Refs → child [`VideoTrack`](super::track::VideoTrack)s.
  #[inline(always)]
  pub const fn tracks_slice(&self) -> &[Id] {
    self.tracks.as_slice()
  }

  /// Per-kind indexing rollup over the facet's tracks.
  #[inline(always)]
  pub const fn track_progress_ref(&self) -> &IndexProgress {
    &self.track_progress
  }

  /// Builder: replace the `tracks` id-list.
  #[must_use]
  #[inline(always)]
  pub fn with_tracks(mut self, tracks: impl Into<std::vec::Vec<Id>>) -> Self {
    self.tracks = tracks.into();
    self
  }

  /// Builder: replace `total_scenes`.
  #[must_use]
  #[inline(always)]
  pub const fn with_total_scenes(mut self, n: u32) -> Self {
    self.total_scenes = n;
    self
  }

  /// Builder: replace the `track_progress` rollup.
  #[must_use]
  #[inline(always)]
  pub const fn with_track_progress(mut self, p: IndexProgress) -> Self {
    self.track_progress = p;
    self
  }

  /// In-place mutator for `tracks`.
  #[inline(always)]
  pub fn set_tracks(&mut self, tracks: impl Into<std::vec::Vec<Id>>) -> &mut Self {
    self.tracks = tracks.into();
    self
  }

  /// In-place mutator for `total_scenes`.
  #[inline(always)]
  pub const fn set_total_scenes(&mut self, n: u32) -> &mut Self {
    self.total_scenes = n;
    self
  }

  /// In-place mutator for `track_progress`.
  #[inline(always)]
  pub const fn set_track_progress(&mut self, p: IndexProgress) -> &mut Self {
    self.track_progress = p;
    self
  }
}

/// Error returned when [`Video::try_new`] cannot uphold the non-nil-id
/// invariant. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum VideoError {
  /// The supplied `id` was the nil sentinel — not a real identity.
  #[error("Video facet id must not be the nil UUID")]
  NilId,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use crate::domain::vo::IndexProgressError;

  #[test]
  fn try_new_happy_path() {
    let id = Uuid7::new();
    let v = Video::try_new(id).unwrap();
    assert_eq!(v.id_ref(), &id);
    assert_eq!(v.total_scenes(), 0);
    assert!(v.tracks_slice().is_empty());
    assert_eq!(v.track_progress_ref(), &IndexProgress::new());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = Video::try_new(Uuid7::nil());
    assert_eq!(r.err(), Some(VideoError::NilId));
    assert!(VideoError::NilId.is_nil_id());
  }

  #[test]
  fn builders_and_setters_chain() {
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let p = IndexProgress::try_new(2, 1, 0).unwrap();
    let v = Video::try_new(Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![t1, t2])
      .with_total_scenes(7)
      .with_track_progress(p);
    assert_eq!(v.total_scenes(), 7);
    assert_eq!(v.tracks_slice().len(), 2);
    assert!(v.tracks_slice().contains(&t1));
    assert_eq!(v.track_progress_ref(), &p);

    let mut v = v;
    v.set_total_scenes(0);
    v.set_tracks(std::vec::Vec::<Uuid7>::new());
    v.set_track_progress(IndexProgress::new());
    assert_eq!(v.total_scenes(), 0);
    assert!(v.tracks_slice().is_empty());
    assert_eq!(v.track_progress_ref(), &IndexProgress::new());
  }

  #[test]
  fn fields_are_independent_across_mutators() {
    // Per validation-responsibility-boundary: replacing `tracks` does
    // NOT reset `total_scenes` / `track_progress`, and `total_scenes`
    // is accepted on an empty track list. The DB / app layer is the
    // source of truth for rollups; the domain stores what the caller
    // puts in.
    let mut v = Video::try_new(Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![Uuid7::new(), Uuid7::new()])
      .with_total_scenes(7)
      .with_track_progress(IndexProgress::try_new(2, 1, 1).unwrap());
    v.set_tracks(std::vec![Uuid7::new()]);
    assert_eq!(v.tracks_slice().len(), 1);
    // Rollups remain whatever the caller last set them to.
    assert_eq!(v.total_scenes(), 7);
    assert_eq!(
      v.track_progress_ref(),
      &IndexProgress::try_new(2, 1, 1).unwrap()
    );

    // `total_scenes` is accepted on an empty track list — no
    // tracks-imply-zero-scenes enforcement at the domain layer.
    let v2 = Video::try_new(Uuid7::new()).unwrap().with_total_scenes(3);
    assert_eq!(v2.total_scenes(), 3);
    assert!(v2.tracks_slice().is_empty());
  }

  #[test]
  fn index_progress_invariant_rejects_overcount() {
    let r = IndexProgress::try_new(2, 2, 1);
    assert_eq!(r.err(), Some(IndexProgressError::SumExceedsTotal));
    assert!(IndexProgressError::SumExceedsTotal.is_sum_exceeds_total());
  }

  #[test]
  fn index_progress_invariant_rejects_overflow() {
    let r = IndexProgress::try_new(u32::MAX, u32::MAX, 1);
    assert_eq!(r.err(), Some(IndexProgressError::SumOverflows));
    assert!(IndexProgressError::SumOverflows.is_sum_overflows());
  }

  #[test]
  fn index_progress_has_failures() {
    let none = IndexProgress::try_new(5, 5, 0).unwrap();
    let some = IndexProgress::try_new(5, 3, 2).unwrap();
    assert!(!none.has_failures());
    assert!(some.has_failures());
  }
}
