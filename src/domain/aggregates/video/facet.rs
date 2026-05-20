//! `Video` — thin video facet aggregate (locked `schema/video.md` r8).
//!
//! Groups the file's video tracks + the per-kind indexing roll-up. **No
//! scalar stream metadata** of its own — file-level lives on `Media`,
//! stream/scene-level on `VideoTrack` (locked three-level data placement
//! rule). `scenes` was removed in rev 8 → moved to `VideoTrack.scenes`
//! (scene detection is per video stream); the facet keeps only the
//! `total_scenes` denormalized rollup.

use derive_more::IsVariant;

use crate::domain::Uuid7;

// ---------------------------------------------------------------------------
// IndexProgress — child-track rollup shared across all three facets
// ---------------------------------------------------------------------------

// NOTE: `IndexProgress` is the locked cross-cutting rollup (`schema/README.md`
// — "Indexing model-correction") shared by `Video`/`Audio`/`Subtitle` facets.
// It lives here because this PR ships the video cluster only; the audio +
// subtitle parallel PRs may temporarily duplicate it — a follow-up will hoist
// the single canonical definition to `src/domain/vo.rs` and re-export from
// each facet. Until then, treat the video-cluster definition as the source.

/// Per-kind facet rollup of `{total, indexed, failed}` over the facet's
/// child tracks. **Denormalised cache** — the source of truth lives on each
/// `*Track`'s `index_status` + `index_errors`; the facet maintains this so
/// list queries don't have to re-aggregate across tracks.
///
/// Invariant: `indexed + failed <= total`. Validated at the type boundary
/// via [`IndexProgress::try_new`]; mutators preserve the invariant by
/// rejecting any update that would violate it.
///
/// **Default convention**: `Default::default()` calls
/// [`IndexProgress::new`] — the empty rollup `{0, 0, 0}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndexProgress {
  total: u32,
  indexed: u32,
  failed: u32,
}

impl IndexProgress {
  /// Canonical no-arg constructor — the empty rollup (`{0, 0, 0}`).
  #[inline]
  pub const fn new() -> Self {
    Self {
      total: 0,
      indexed: 0,
      failed: 0,
    }
  }

  /// Validating constructor: rejects `indexed + failed > total` (the
  /// rollup invariant). `u32` saturates before the addition would
  /// overflow.
  pub const fn try_new(total: u32, indexed: u32, failed: u32) -> Result<Self, IndexProgressError> {
    // `u32::checked_add` is const fn since 1.61.
    let sum = match indexed.checked_add(failed) {
      Some(s) => s,
      None => return Err(IndexProgressError::SumOverflows),
    };
    if sum > total {
      return Err(IndexProgressError::SumExceedsTotal);
    }
    Ok(Self {
      total,
      indexed,
      failed,
    })
  }

  /// Total child tracks the facet owns.
  #[inline]
  pub const fn total(&self) -> u32 {
    self.total
  }

  /// Tracks that finished indexing successfully.
  #[inline]
  pub const fn indexed(&self) -> u32 {
    self.indexed
  }

  /// Tracks whose indexing failed (`index_errors` non-empty at the time
  /// of last rollup maintenance).
  #[inline]
  pub const fn failed(&self) -> u32 {
    self.failed
  }

  /// True iff the facet has at least one failed track — the locked
  /// "kind container's error signal" rule (`failed > 0` ⇒ drill down).
  #[inline]
  pub const fn has_failures(&self) -> bool {
    self.failed > 0
  }
}

impl Default for IndexProgress {
  #[inline]
  fn default() -> Self {
    Self::new()
  }
}

/// Error returned when [`IndexProgress::try_new`] cannot uphold the
/// `indexed + failed <= total` invariant. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum IndexProgressError {
  /// `indexed + failed > total` — would overcount.
  #[error("IndexProgress: indexed + failed must not exceed total")]
  SumExceedsTotal,
  /// `indexed + failed` overflows `u32` — definitely overcounts.
  #[error("IndexProgress: indexed + failed overflows u32")]
  SumOverflows,
}

// ---------------------------------------------------------------------------
// Video — the thin facet aggregate
// ---------------------------------------------------------------------------

/// The video facet of a `Media`. Holds the facet identity, child-track
/// id list, the `total_scenes` rollup, and the per-kind index progress.
/// **Generic over `Id`** (default [`Uuid7`]) — the parent FK lives on
/// `Media.video`, and each `VideoTrack` carries the back-reference.
///
/// Fields are private per the encapsulation rule; access via the
/// `id()` / `total_scenes()` / `tracks()` / `track_progress()` getters
/// and the `with_*` / `set_*` builders/mutators.
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
  /// (`total_scenes = 0`), and empty index progress; the indexer fills
  /// these in via `with_*` / `set_*` as tracks are created and
  /// processed.
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
  #[inline]
  pub const fn id(&self) -> &Id {
    &self.id
  }

  /// Total scenes across all child tracks — denormalised rollup
  /// (`Σ over its VideoTracks of scenes.len()`).
  #[inline]
  pub const fn total_scenes(&self) -> u32 {
    self.total_scenes
  }

  /// Refs → child [`VideoTrack`](super::track::VideoTrack)s.
  #[inline]
  pub const fn tracks(&self) -> &[Id] {
    self.tracks.as_slice()
  }

  /// Per-kind indexing rollup over the facet's tracks.
  #[inline]
  pub const fn track_progress(&self) -> &IndexProgress {
    &self.track_progress
  }

  /// Builder: replace `total_scenes`.
  #[inline]
  pub const fn with_total_scenes(mut self, n: u32) -> Self {
    self.total_scenes = n;
    self
  }

  /// Builder: replace the `tracks` id-list.
  #[inline]
  pub fn with_tracks(mut self, tracks: impl Into<std::vec::Vec<Id>>) -> Self {
    self.tracks = tracks.into();
    self
  }

  /// Builder: replace the `track_progress` rollup.
  #[inline]
  pub const fn with_track_progress(mut self, p: IndexProgress) -> Self {
    self.track_progress = p;
    self
  }

  /// In-place mutator for `total_scenes`.
  #[inline]
  pub const fn set_total_scenes(&mut self, n: u32) {
    self.total_scenes = n;
  }

  /// In-place mutator for `tracks`.
  #[inline]
  pub fn set_tracks(&mut self, tracks: impl Into<std::vec::Vec<Id>>) {
    self.tracks = tracks.into();
  }

  /// In-place mutator for `track_progress`.
  #[inline]
  pub const fn set_track_progress(&mut self, p: IndexProgress) {
    self.track_progress = p;
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

  #[test]
  fn try_new_happy_path() {
    let id = Uuid7::new();
    let v = Video::try_new(id).unwrap();
    assert_eq!(v.id(), &id);
    assert_eq!(v.total_scenes(), 0);
    assert!(v.tracks().is_empty());
    assert_eq!(v.track_progress(), &IndexProgress::new());
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
      .with_total_scenes(7)
      .with_tracks(std::vec![t1, t2])
      .with_track_progress(p);
    assert_eq!(v.total_scenes(), 7);
    assert_eq!(v.tracks().len(), 2);
    assert!(v.tracks().contains(&t1));
    assert_eq!(v.track_progress(), &p);

    let mut v = v;
    v.set_total_scenes(0);
    v.set_tracks(std::vec::Vec::<Uuid7>::new());
    v.set_track_progress(IndexProgress::new());
    assert_eq!(v.total_scenes(), 0);
    assert!(v.tracks().is_empty());
    assert_eq!(v.track_progress(), &IndexProgress::new());
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
