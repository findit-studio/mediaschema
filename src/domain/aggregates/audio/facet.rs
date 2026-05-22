//! `Audio` — audio facet (thin aggregate).
//!
//! Locked `schema/audio.md` rev 8 — A-loc cascade resolved per-track. The
//! facet groups this media's audio tracks + an indexing rollup; the heavy
//! segmented-ML aggregate (`AudioSegment`) is **per-track** (on
//! `AudioTrack.segments`), mirroring locked `VideoTrack.scenes`. The facet
//! keeps only a `total_segments` rollup for cheap "how many segments under
//! this media" queries.
//!
//! `AudioFileRecord` is **dissolved** per the locked schema — there is no
//! separate file-record aggregate; per-recording tags + cover art live on
//! `AudioTrack` (multi-track audio files = N recordings).

use derive_more::IsVariant;

use crate::domain::Uuid7;

// ---------------------------------------------------------------------------
// IndexProgress — child-track rollup shared across all three facets
// ---------------------------------------------------------------------------

// NOTE: `IndexProgress` is the locked cross-cutting rollup (`schema/README.md`
// — "Indexing model-correction") shared by `Video`/`Audio`/`Subtitle` facets.
// It is duplicated here verbatim from the video cluster because the audio +
// video facet PRs ship in parallel; a follow-up reconcile PR will hoist the
// single canonical definition to `src/domain/vo.rs` and re-export from each
// facet. Until then, the video-cluster definition is the source of truth and
// this is a deliberate temporary copy.

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
  /// rollup invariant). `u32::checked_add` guards the overflow case.
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
// Audio — the thin facet aggregate
// ---------------------------------------------------------------------------

/// Audio facet of a `Media`. Parent → `Media` (referenced by `Media.audio`).
///
/// Generic over `Id` (default [`Uuid7`]). The `tracks` vector holds refs to
/// child `AudioTrack`s; `total_segments` is a cheap rollup of
/// `Σ AudioTrack.segments.len()`.
///
/// **Invariant**: `track_progress.total() == tracks.len()` — `track_progress`
/// is the rollup *over* `tracks`, so the two cannot be mutated
/// independently. Any change to `tracks` resets `track_progress` to
/// `{ total = tracks.len(), indexed = 0, failed = 0 }` (a changed track list
/// invalidates prior indexing progress); the `track_progress` setter is
/// fallible and rejects a `total` that disagrees with `tracks.len()`.
///
/// **No `Default`** — a facet with nil `id` would be an orphan record. Use
/// [`Audio::try_new`] for the canonical `Uuid7` identity type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Audio<Id = Uuid7> {
  id: Id,
  tracks: std::vec::Vec<Id>,
  total_segments: u32,
  track_progress: IndexProgress,
}

impl Audio<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (every aggregate row needs a real identity). The
  /// `tracks` list starts empty, `total_segments` at zero, and
  /// `track_progress` as the empty rollup (`{0, 0, 0}`) — consistent with
  /// the empty `tracks` list (`track_progress.total() == tracks.len()`).
  /// All three are populated by builders/mutators as tracks are attached +
  /// segments rolled up.
  pub fn try_new(id: Uuid7) -> Result<Self, AudioError> {
    if id.is_nil() {
      return Err(AudioError::NilId);
    }
    Ok(Self {
      id,
      tracks: std::vec::Vec::new(),
      total_segments: 0,
      track_progress: IndexProgress::new(),
    })
  }
}

impl<Id> Audio<Id> {
  /// Canonical identity (also referenced by `Media.audio`).
  #[inline]
  pub const fn id(&self) -> &Id {
    &self.id
  }

  /// Refs to child `AudioTrack`s.
  #[inline]
  pub const fn tracks(&self) -> &[Id] {
    self.tracks.as_slice()
  }

  /// Rollup `Σ AudioTrack.segments.len()` — cheap "how many segments under
  /// this media" facet. Truth = per-track `AudioTrack.segments`.
  #[inline]
  pub const fn total_segments(&self) -> u32 {
    self.total_segments
  }

  /// Per-kind indexing rollup over the facet's tracks.
  #[inline]
  pub const fn track_progress(&self) -> &IndexProgress {
    &self.track_progress
  }

  /// Builder: replace `tracks`.
  ///
  /// Replacing the track list with a **different** list invalidates the two
  /// rollups derived from it: `track_progress` is reset to
  /// `{ total = new tracks.len(), indexed = 0, failed = 0 }` (keeping the
  /// `track_progress.total() == tracks.len()` invariant) and `total_segments`
  /// — `Σ AudioTrack.segments.len()`, which cannot be recomputed from the
  /// id-only `tracks` list — is reset to `0`. Use
  /// [`Audio::try_with_track_progress`] / [`Audio::with_total_segments`]
  /// afterwards to record real values against the new list.
  ///
  /// Reapplying the **identical** track-id list (same ids, same order) is a
  /// no-op for both rollups, so an idempotent save/retry that re-sets the
  /// current list does not wipe valid progress.
  #[inline]
  #[must_use]
  pub fn with_tracks(mut self, tracks: impl Into<std::vec::Vec<Id>>) -> Self
  where
    Id: PartialEq,
  {
    self.replace_tracks(tracks.into());
    self
  }

  /// Builder: replace `total_segments`.
  #[inline]
  #[must_use]
  pub const fn with_total_segments(mut self, total: u32) -> Self {
    self.total_segments = total;
    self
  }

  /// Validating builder: replace the `track_progress` rollup.
  ///
  /// Rejects a rollup whose `total()` disagrees with the current
  /// `tracks.len()` — `track_progress` must always be the rollup *over*
  /// this facet's tracks. On rejection `self` is returned unchanged inside
  /// the `Err`.
  #[inline]
  pub fn try_with_track_progress(mut self, p: IndexProgress) -> Result<Self, AudioError> {
    if p.total() != self.tracks_len() {
      return Err(AudioError::TrackProgressMismatch);
    }
    self.track_progress = p;
    Ok(self)
  }

  /// In-place mutator for `tracks`.
  ///
  /// As with [`Audio::with_tracks`], replacing the track list with a
  /// **different** list resets `track_progress` to
  /// `{ total = new tracks.len(), indexed = 0, failed = 0 }` and resets
  /// `total_segments` to `0`; reapplying the **identical** list leaves both
  /// rollups untouched.
  #[inline]
  pub fn set_tracks(&mut self, tracks: impl Into<std::vec::Vec<Id>>) -> &mut Self
  where
    Id: PartialEq,
  {
    self.replace_tracks(tracks.into());
    self
  }

  /// In-place mutator for `total_segments`.
  #[inline]
  pub const fn set_total_segments(&mut self, total: u32) -> &mut Self {
    self.total_segments = total;
    self
  }

  /// Validating in-place mutator for `track_progress`. Rejects a rollup
  /// whose `total()` disagrees with the current `tracks.len()`; on
  /// rejection `self` is left unchanged.
  #[inline]
  pub fn try_set_track_progress(&mut self, p: IndexProgress) -> Result<&mut Self, AudioError> {
    if p.total() != self.tracks_len() {
      return Err(AudioError::TrackProgressMismatch);
    }
    self.track_progress = p;
    Ok(self)
  }

  /// `tracks.len()` saturated into `u32` — the track count can never
  /// realistically exceed `u32::MAX`, and `IndexProgress::total` is `u32`.
  #[inline]
  fn tracks_len(&self) -> u32 {
    u32::try_from(self.tracks.len()).unwrap_or(u32::MAX)
  }

  /// Shared track-list replacement for [`Audio::with_tracks`] /
  /// [`Audio::set_tracks`].
  ///
  /// Reapplying the **identical** id list (same ids, same order) is treated
  /// as a no-op so an idempotent save/retry does not wipe valid rollups. A
  /// **different** list invalidates both rollups derived from `tracks`:
  /// `track_progress` is reset to the fresh rollup over the new list
  /// (`total = tracks.len()`, `indexed = 0`, `failed = 0`) and
  /// `total_segments` — `Σ AudioTrack.segments.len()`, not recomputable from
  /// the id-only list — is reset to `0`.
  #[inline]
  fn replace_tracks(&mut self, tracks: std::vec::Vec<Id>)
  where
    Id: PartialEq,
  {
    if self.tracks == tracks {
      // Identical list ⇒ rollups remain valid; nothing to invalidate.
      return;
    }
    self.tracks = tracks;
    // `indexed + failed = 0 <= total`, so the invariant always holds.
    self.track_progress =
      IndexProgress::try_new(self.tracks_len(), 0, 0).expect("0 + 0 never exceeds total");
    // `total_segments` cannot be recomputed from the id-only `tracks` list,
    // so a changed track list invalidates it — reset to 0.
    self.total_segments = 0;
  }
}

/// Error returned by [`Audio`]'s validating constructor and `track_progress`
/// mutators. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum AudioError {
  /// Supplied `id` was the nil sentinel — not a real identity.
  #[error("Audio id must not be the nil UUID")]
  NilId,
  /// A supplied `track_progress` rollup's `total` disagreed with the
  /// facet's current `tracks.len()` — `track_progress` must be the rollup
  /// over `tracks`.
  #[error("Audio track_progress.total must equal tracks.len()")]
  TrackProgressMismatch,
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
    let a = Audio::try_new(id).expect("valid construction must succeed");
    assert_eq!(a.id(), &id);
    assert!(a.tracks().is_empty());
    assert_eq!(a.total_segments(), 0);
    assert_eq!(a.track_progress(), &IndexProgress::new());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = Audio::try_new(Uuid7::nil());
    assert_eq!(r.err(), Some(AudioError::NilId));
    assert!(AudioError::NilId.is_nil_id());
  }

  #[test]
  fn builders_chain_tracks_and_rollup() {
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let p = IndexProgress::try_new(2, 1, 0).unwrap();
    let a = Audio::try_new(Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![t1, t2])
      .with_total_segments(42)
      .try_with_track_progress(p)
      .unwrap();
    assert_eq!(a.tracks().len(), 2);
    assert!(a.tracks().contains(&t1));
    assert_eq!(a.total_segments(), 42);
    assert_eq!(a.track_progress(), &p);
  }

  #[test]
  fn setters_mutate_in_place() {
    let mut a = Audio::try_new(Uuid7::new()).unwrap();
    a.set_tracks(std::vec![Uuid7::new()]);
    a.set_total_segments(7);
    a.try_set_track_progress(IndexProgress::try_new(1, 1, 0).unwrap())
      .unwrap();
    assert_eq!(a.tracks().len(), 1);
    assert_eq!(a.total_segments(), 7);
    assert_eq!(a.track_progress().total(), 1);
    assert!(!a.track_progress().has_failures());
  }

  // --- Finding 3: track_progress / tracks coupling --------------------------

  #[test]
  fn fresh_facet_has_empty_rollup_consistent_with_empty_tracks() {
    let a = Audio::try_new(Uuid7::new()).unwrap();
    assert_eq!(a.track_progress().total() as usize, a.tracks().len());
    assert_eq!(a.track_progress(), &IndexProgress::new());
  }

  #[test]
  fn with_tracks_resets_track_progress_to_fresh_rollup() {
    let a = Audio::try_new(Uuid7::new()).unwrap().with_tracks(std::vec![
      Uuid7::new(),
      Uuid7::new(),
      Uuid7::new()
    ]);
    // changed track list ⇒ fresh rollup { total = 3, indexed = 0, failed = 0 }
    assert_eq!(
      a.track_progress(),
      &IndexProgress::try_new(3, 0, 0).unwrap()
    );
    assert_eq!(a.track_progress().total() as usize, a.tracks().len());
  }

  #[test]
  fn set_tracks_resets_prior_track_progress() {
    let mut a = Audio::try_new(Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![Uuid7::new(), Uuid7::new()])
      .try_with_track_progress(IndexProgress::try_new(2, 2, 0).unwrap())
      .unwrap();
    // replacing the track list invalidates the prior fully-indexed rollup
    a.set_tracks(std::vec![Uuid7::new()]);
    assert_eq!(
      a.track_progress(),
      &IndexProgress::try_new(1, 0, 0).unwrap()
    );
    assert_eq!(a.track_progress().total() as usize, a.tracks().len());
  }

  #[test]
  fn reapplying_identical_track_list_preserves_both_rollups() {
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let mut a = Audio::try_new(Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![t1, t2])
      .with_total_segments(17)
      .try_with_track_progress(IndexProgress::try_new(2, 1, 1).unwrap())
      .unwrap();
    // idempotent re-set of the SAME id list (same order) ⇒ no invalidation
    a.set_tracks(std::vec![t1, t2]);
    assert_eq!(a.total_segments(), 17);
    assert_eq!(
      a.track_progress(),
      &IndexProgress::try_new(2, 1, 1).unwrap()
    );
  }

  #[test]
  fn changing_track_list_invalidates_both_rollups() {
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let mut a = Audio::try_new(Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![t1, t2])
      .with_total_segments(17)
      .try_with_track_progress(IndexProgress::try_new(2, 1, 1).unwrap())
      .unwrap();
    // a DIFFERENT track list invalidates total_segments + track_progress
    a.set_tracks(std::vec![Uuid7::new()]);
    assert_eq!(a.total_segments(), 0);
    assert_eq!(a.track_progress().total(), 1);
    assert_eq!(a.track_progress().total() as usize, a.tracks().len());
    assert_eq!(a.track_progress().indexed(), 0);
    assert_eq!(a.track_progress().failed(), 0);
  }

  #[test]
  fn try_with_track_progress_rejects_total_mismatch() {
    let a = Audio::try_new(Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![Uuid7::new(), Uuid7::new()]);
    // total = 3 disagrees with tracks.len() = 2
    let r = a
      .clone()
      .try_with_track_progress(IndexProgress::try_new(3, 0, 0).unwrap());
    assert_eq!(r.err(), Some(AudioError::TrackProgressMismatch));
    assert!(AudioError::TrackProgressMismatch.is_track_progress_mismatch());
    // matching total is accepted
    assert!(a
      .try_with_track_progress(IndexProgress::try_new(2, 1, 1).unwrap())
      .is_ok());
  }

  #[test]
  fn try_set_track_progress_rejects_and_leaves_value_unchanged() {
    let mut a = Audio::try_new(Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![Uuid7::new(), Uuid7::new()]);
    let before = *a.track_progress();
    let r = a.try_set_track_progress(IndexProgress::try_new(5, 0, 0).unwrap());
    assert_eq!(r.err(), Some(AudioError::TrackProgressMismatch));
    // rejection leaves the prior rollup in place
    assert_eq!(a.track_progress(), &before);
    a.try_set_track_progress(IndexProgress::try_new(2, 2, 0).unwrap())
      .unwrap();
    assert_eq!(a.track_progress().indexed(), 2);
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
