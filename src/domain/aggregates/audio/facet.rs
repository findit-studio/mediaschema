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
  /// `tracks` list starts empty and `total_segments` at zero; both are
  /// populated by builders/mutators as tracks are attached + segments
  /// rolled up.
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
  #[inline]
  pub fn with_tracks(mut self, tracks: impl Into<std::vec::Vec<Id>>) -> Self {
    self.tracks = tracks.into();
    self
  }

  /// Builder: replace `total_segments`.
  #[inline]
  pub const fn with_total_segments(mut self, total: u32) -> Self {
    self.total_segments = total;
    self
  }

  /// Builder: replace the `track_progress` rollup.
  #[inline]
  pub const fn with_track_progress(mut self, p: IndexProgress) -> Self {
    self.track_progress = p;
    self
  }

  /// In-place mutator for `tracks`.
  #[inline]
  pub fn set_tracks(&mut self, tracks: impl Into<std::vec::Vec<Id>>) {
    self.tracks = tracks.into();
  }

  /// In-place mutator for `total_segments`.
  #[inline]
  pub const fn set_total_segments(&mut self, total: u32) {
    self.total_segments = total;
  }

  /// In-place mutator for `track_progress`.
  #[inline]
  pub const fn set_track_progress(&mut self, p: IndexProgress) {
    self.track_progress = p;
  }
}

/// Error returned when [`Audio::try_new`] cannot uphold the non-nil-id
/// invariant. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum AudioError {
  /// Supplied `id` was the nil sentinel — not a real identity.
  #[error("Audio id must not be the nil UUID")]
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
      .with_track_progress(p);
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
    a.set_track_progress(IndexProgress::try_new(3, 2, 1).unwrap());
    assert_eq!(a.tracks().len(), 1);
    assert_eq!(a.total_segments(), 7);
    assert_eq!(a.track_progress().total(), 3);
    assert!(a.track_progress().has_failures());
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
