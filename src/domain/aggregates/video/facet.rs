//! `Video` — thin video facet aggregate (locked `schema/video.md` r8).
//!
//! Groups the file's video tracks + the per-kind indexing roll-up. **No
//! scalar stream metadata** of its own — file-level lives on `Media`,
//! stream/scene-level on `VideoTrack` (locked three-level data placement
//! rule). `scenes` was removed in rev 8 → moved to `VideoTrack.scenes`
//! (scene detection is per video stream); the facet keeps only the
//! `total_scenes` denormalized rollup.

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
  /// (`total_scenes = 0`), and an empty index-progress rollup; the
  /// indexer fills these in via `with_*` / `set_*` as tracks are
  /// created and processed. Whenever the track list changes, the
  /// `track_progress` rollup is reset to `{total = tracks.len(),
  /// indexed = 0, failed = 0}` so the documented invariant
  /// `track_progress.total() == tracks.len()` always holds.
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

  /// Builder: replace the `tracks` id-list.
  ///
  /// A *changed* track list invalidates any prior indexing progress, so
  /// `track_progress` is reset to `{total = new tracks.len(), indexed =
  /// 0, failed = 0}` — keeping the invariant `track_progress.total() ==
  /// tracks.len()`. Any track-list change also resets `total_scenes` to
  /// 0 (the rollup sums scenes across the *current* tracks and is
  /// meaningless across a different track set).
  ///
  /// Reapplying the *identical* track-id list (same ids, same order) —
  /// an idempotent save/retry — leaves `track_progress` and
  /// `total_scenes` untouched, so completed/failed progress and the
  /// scene count survive a no-op re-set.
  #[must_use]
  #[inline(always)]
  pub fn with_tracks(mut self, tracks: impl Into<std::vec::Vec<Uuid7>>) -> Self {
    self.set_tracks(tracks);
    self
  }

  /// In-place mutator for `tracks`.
  ///
  /// A *changed* track list invalidates any prior indexing progress, so
  /// `track_progress` is reset to `{total = new tracks.len(), indexed =
  /// 0, failed = 0}` — keeping the invariant `track_progress.total() ==
  /// tracks.len()`. A track-list change also resets `total_scenes` to 0
  /// (the rollup sums scenes across the *current* tracks and is
  /// meaningless across a different track set).
  ///
  /// Reapplying the *identical* track-id list (same ids, same order) —
  /// an idempotent save/retry — leaves `track_progress` and
  /// `total_scenes` untouched, so completed/failed progress and the
  /// scene count survive a no-op re-set.
  #[inline]
  pub fn set_tracks(&mut self, tracks: impl Into<std::vec::Vec<Uuid7>>) -> &mut Self {
    let tracks = tracks.into();
    // An idempotent re-set with the identical id list (same ids, same
    // order) leaves the rollups alone — the invariant is "reset when
    // the track list *changes*", not "reset on every call".
    if self.tracks == tracks {
      return self;
    }
    self.tracks = tracks;
    // `tracks.len()` (`usize`) saturates into the `u32` rollup total;
    // `indexed`/`failed` are 0, so the `IndexProgress` invariant holds
    // and `try_new` cannot fail.
    let total = u32::try_from(self.tracks.len()).unwrap_or(u32::MAX);
    self.track_progress = IndexProgress::try_new(total, 0, 0)
      .expect("freshly-reset rollup {total, 0, 0} always upholds the IndexProgress invariant");
    // `total_scenes` is the sum of scenes across the *current* child
    // tracks — meaningless once the track set changes. Reset it on any
    // track-list change (not just emptying); this also upholds the
    // `tracks.is_empty() ⇒ total_scenes == 0` invariant.
    self.total_scenes = 0;
    self
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

  /// Fallible builder: replace `total_scenes`.
  ///
  /// `total_scenes` is a cross-aggregate rollup (scenes belong to
  /// tracks, and `tracks` holds only ids so it cannot be locally
  /// recomputed). The one invariant enforceable locally is
  /// `tracks.is_empty() ⇒ total_scenes == 0`: a facet with no tracks
  /// cannot have scenes. A non-zero `n` on an empty track list is
  /// rejected with [`VideoError::ScenesWithoutTracks`].
  #[inline]
  pub fn try_with_total_scenes(mut self, n: u32) -> Result<Self, VideoError> {
    self.try_set_total_scenes(n)?;
    Ok(self)
  }

  /// Fallible builder: replace the `track_progress` rollup.
  ///
  /// Rejects a rollup whose `total` does not equal `tracks.len()`
  /// ([`VideoError::TrackProgressMismatch`]) — progress is a rollup
  /// over exactly the facet's current tracks. On error `self` is
  /// returned unchanged inside the `Err`-free path is not possible, so
  /// the value is consumed; callers that need the original on failure
  /// should clone first.
  #[inline]
  pub fn try_with_track_progress(mut self, p: IndexProgress) -> Result<Self, VideoError> {
    self.try_set_track_progress(p)?;
    Ok(self)
  }

  /// Fallible in-place mutator for `total_scenes`, enforcing the local
  /// invariant `tracks.is_empty() ⇒ total_scenes == 0`.
  ///
  /// On success returns `&mut Self`; a non-zero `n` on an empty track
  /// list returns [`VideoError::ScenesWithoutTracks`] and leaves `self`
  /// unchanged.
  #[inline]
  pub fn try_set_total_scenes(&mut self, n: u32) -> Result<&mut Self, VideoError> {
    if n > 0 && self.tracks.is_empty() {
      return Err(VideoError::ScenesWithoutTracks);
    }
    self.total_scenes = n;
    Ok(self)
  }

  /// Fallible in-place mutator for `track_progress`, validating that
  /// the rollup's `total` equals the facet's current `tracks.len()`.
  ///
  /// On success returns `&mut Self`; on a mismatching `total` returns
  /// [`VideoError::TrackProgressMismatch`] and leaves `self` unchanged.
  #[inline]
  pub fn try_set_track_progress(&mut self, p: IndexProgress) -> Result<&mut Self, VideoError> {
    let expected = u32::try_from(self.tracks.len()).unwrap_or(u32::MAX);
    if p.total() != expected {
      return Err(VideoError::TrackProgressMismatch);
    }
    self.track_progress = p;
    Ok(self)
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
  /// A `track_progress` rollup was set whose `total` does not equal
  /// the facet's current `tracks.len()` — the rollup must aggregate
  /// over exactly the facet's tracks.
  #[error("Video track_progress total must equal tracks.len()")]
  TrackProgressMismatch,
  /// `total_scenes` was set to a non-zero count while `tracks` is
  /// empty — scenes belong to tracks, so a track-less facet has no
  /// scenes.
  #[error("Video total_scenes must be 0 when tracks is empty")]
  ScenesWithoutTracks,
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
    // A 2-track progress is valid only against a 2-track list.
    let p = IndexProgress::try_new(2, 1, 0).unwrap();
    // `total_scenes` must be set *after* a non-empty track list — the
    // `tracks.is_empty() ⇒ total_scenes == 0` invariant.
    let v = Video::try_new(Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![t1, t2])
      .try_with_total_scenes(7)
      .unwrap()
      .try_with_track_progress(p)
      .unwrap();
    assert_eq!(v.total_scenes(), 7);
    assert_eq!(v.tracks_slice().len(), 2);
    assert!(v.tracks_slice().contains(&t1));
    assert_eq!(v.track_progress_ref(), &p);

    let mut v = v;
    v.try_set_total_scenes(0).unwrap();
    v.set_tracks(std::vec::Vec::<Uuid7>::new());
    // Empty track list ⇒ `set_tracks` resets progress to the empty
    // rollup.
    v.try_set_track_progress(IndexProgress::new()).unwrap();
    assert_eq!(v.total_scenes(), 0);
    assert!(v.tracks_slice().is_empty());
    assert_eq!(v.track_progress_ref(), &IndexProgress::new());
  }

  #[test]
  fn with_tracks_resets_progress_to_match_track_count() {
    // rev-2 finding: `with_tracks` must not leave a stale `{0,0,0}`
    // rollup on a non-empty track list.
    let v = Video::try_new(Uuid7::new()).unwrap().with_tracks(std::vec![
      Uuid7::new(),
      Uuid7::new(),
      Uuid7::new()
    ]);
    assert_eq!(v.track_progress_ref().total(), 3);
    assert_eq!(v.track_progress_ref().indexed(), 0);
    assert_eq!(v.track_progress_ref().failed(), 0);

    // Changing the list re-syncs the rollup total.
    let mut v = v;
    v.set_tracks(std::vec![Uuid7::new()]);
    assert_eq!(v.track_progress_ref().total(), 1);
  }

  #[test]
  fn total_scenes_invariant_with_empty_tracks() {
    // rev-3 finding 1: a track-less facet cannot have scenes.
    let mut v = Video::try_new(Uuid7::new()).unwrap();
    assert_eq!(
      v.try_set_total_scenes(3).err(),
      Some(VideoError::ScenesWithoutTracks)
    );
    assert!(VideoError::ScenesWithoutTracks.is_scenes_without_tracks());
    // `0` is always fine on an empty list.
    assert!(v.try_set_total_scenes(0).is_ok());
    // `try_new` starts with `total_scenes == 0`, upholding the invariant.
    assert_eq!(v.total_scenes(), 0);
    // The consuming builder rejects identically.
    assert_eq!(
      Video::try_new(Uuid7::new())
        .unwrap()
        .try_with_total_scenes(1)
        .err(),
      Some(VideoError::ScenesWithoutTracks)
    );
  }

  #[test]
  fn emptying_tracks_resets_total_scenes() {
    // rev-3 finding 1: replacing the track list with an empty one must
    // reset `total_scenes` to 0 (scenes belong to tracks).
    let mut v = Video::try_new(Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![Uuid7::new(), Uuid7::new()])
      .try_with_total_scenes(9)
      .unwrap();
    assert_eq!(v.total_scenes(), 9);
    v.set_tracks(std::vec::Vec::<Uuid7>::new());
    assert!(v.tracks_slice().is_empty());
    assert_eq!(v.total_scenes(), 0);
  }

  #[test]
  fn replacing_tracks_resets_total_scenes() {
    // rev-5 finding 2: `total_scenes` sums scenes across the *current*
    // tracks — replacing the track set (non-empty → non-empty) must
    // reset the stale rollup to 0, not only when the list is emptied.
    let mut v = Video::try_new(Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![Uuid7::new()])
      .try_with_total_scenes(7)
      .unwrap();
    assert_eq!(v.total_scenes(), 7);
    // `[old]` → `[new]` is a different track set: the count is stale.
    v.set_tracks(std::vec![Uuid7::new()]);
    assert_eq!(v.tracks_slice().len(), 1);
    assert_eq!(v.total_scenes(), 0);

    // The consuming builder resets identically.
    let v = v
      .try_with_total_scenes(4)
      .unwrap()
      .with_tracks(std::vec![Uuid7::new(), Uuid7::new()]);
    assert_eq!(v.total_scenes(), 0);
  }

  #[test]
  fn idempotent_track_set_preserves_rollups() {
    // rev-6 finding 1: reapplying the *identical* track-id list (an
    // idempotent save/retry) must leave `track_progress` and
    // `total_scenes` untouched — the reset fires only on a real change.
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let mut v = Video::try_new(Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![t1, t2])
      .try_with_total_scenes(7)
      .unwrap()
      .try_with_track_progress(IndexProgress::try_new(2, 1, 1).unwrap())
      .unwrap();
    assert_eq!(v.total_scenes(), 7);
    assert_eq!(
      v.track_progress_ref(),
      &IndexProgress::try_new(2, 1, 1).unwrap()
    );

    // Same ids, same order ⇒ no-op: rollups survive.
    v.set_tracks(std::vec![t1, t2]);
    assert_eq!(v.total_scenes(), 7);
    assert_eq!(
      v.track_progress_ref(),
      &IndexProgress::try_new(2, 1, 1).unwrap()
    );

    // A genuinely different list ⇒ both rollups reset.
    v.set_tracks(std::vec![t2, t1]);
    assert_eq!(v.total_scenes(), 0);
    assert_eq!(
      v.track_progress_ref(),
      &IndexProgress::try_new(2, 0, 0).unwrap()
    );
  }

  #[test]
  fn set_track_progress_rejects_count_divergence() {
    // rev-2 finding: progress whose total != tracks.len() is rejected.
    let mut v = Video::try_new(Uuid7::new()).unwrap();
    // Empty track list — a `total = 5` rollup must not be accepted.
    assert_eq!(
      v.try_set_track_progress(IndexProgress::try_new(5, 0, 0).unwrap())
        .err(),
      Some(VideoError::TrackProgressMismatch)
    );
    assert!(VideoError::TrackProgressMismatch.is_track_progress_mismatch());
    // The empty rollup matches the empty list.
    assert!(v.try_set_track_progress(IndexProgress::new()).is_ok());

    // Two tracks — a 2-total rollup is accepted, a 1-total is not.
    v.set_tracks(std::vec![Uuid7::new(), Uuid7::new()]);
    assert!(v
      .try_set_track_progress(IndexProgress::try_new(2, 1, 1).unwrap())
      .is_ok());
    assert_eq!(
      v.try_with_track_progress(IndexProgress::try_new(1, 0, 0).unwrap())
        .err(),
      Some(VideoError::TrackProgressMismatch)
    );
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
