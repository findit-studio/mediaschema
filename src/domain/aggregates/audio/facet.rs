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
//!
//! ### Validation-responsibility boundary
//!
//! The facet stores `tracks` (id refs), `total_segments`
//! (`Σ AudioTrack.segments.len()`), `total_sound_events`
//! (`Σ AudioTrack.sound_events.len()`), and `track_progress` (per-kind
//! rollup over `tracks`) as **independent fields**. Keeping them consistent —
//! e.g. `track_progress.total() == tracks.len()` — is a
//! cross-field/rollup-coupling concern owned by the application /
//! storage layer (the database is the source of truth for rollups; the
//! domain type is rebuilt from a row without filler-synthesizing a
//! `tracks` Vec to satisfy a derived count). The domain type enforces
//! only intrinsic single-value invariants (here: non-nil `id`).

use std::vec::Vec;

use derive_more::IsVariant;

use crate::domain::{vo::IndexProgress, Uuid7};

// ---------------------------------------------------------------------------
// Audio — the thin facet aggregate
// ---------------------------------------------------------------------------

/// Audio facet of a `Media`. FK `media_id → Media` (referenced by `Media.audio_id`).
///
/// Generic over `Id` (default [`Uuid7`]). The `tracks` vector holds refs to
/// child `AudioTrack`s; `total_segments` is a cheap rollup of
/// `Σ AudioTrack.segments.len()`; `total_sound_events` is the analogous
/// rollup of `Σ AudioTrack.sound_events.len()`; `track_progress` is the
/// per-kind index rollup over `tracks`. All are independent fields — see
/// the module-level note on the validation-responsibility boundary.
///
/// **No `Default`** — a facet with nil `id` would be an orphan record. Use
/// [`Audio::try_new`] for the canonical `Uuid7` identity type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Audio<Id = Uuid7> {
  id: Id,
  media_id: Id,
  tracks: Vec<Id>,
  total_segments: u32,
  total_sound_events: u32,
  track_progress: IndexProgress,
}

impl Audio<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (every aggregate row needs a real identity) and nil
  /// `media_id` (orphaned facet with no `Media` reference). The `tracks`
  /// list starts empty, `total_segments` / `total_sound_events` at zero,
  /// and `track_progress` as the empty rollup (`{0, 0, 0}`). All are
  /// populated by builders/mutators as tracks are attached + segments /
  /// sound events rolled up — or assembled directly from a database row.
  pub fn try_new(id: Uuid7, media_id: Uuid7) -> Result<Self, AudioError> {
    if id.is_nil() {
      return Err(AudioError::NilId);
    }
    if media_id.is_nil() {
      return Err(AudioError::NilMediaId);
    }
    Ok(Self {
      id,
      media_id,
      tracks: Vec::new(),
      total_segments: 0,
      total_sound_events: 0,
      track_progress: IndexProgress::new(),
    })
  }
}

impl<Id> Audio<Id> {
  /// Canonical identity (also referenced by `Media.audio_id`).
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// FK → `Media.id` — the `Media` this facet belongs to. Set at
  /// construction (identity-bearing — no `with_media_id` / `set_media_id`).
  #[inline(always)]
  pub const fn media_id_ref(&self) -> &Id {
    &self.media_id
  }

  /// Refs to child `AudioTrack`s.
  #[inline(always)]
  pub const fn tracks_slice(&self) -> &[Id] {
    self.tracks.as_slice()
  }

  /// Rollup `Σ AudioTrack.segments.len()` — cheap "how many segments under
  /// this media" facet. Truth = per-track `AudioTrack.segments`.
  #[inline(always)]
  pub const fn total_segments(&self) -> u32 {
    self.total_segments
  }

  /// Rollup `Σ AudioTrack.sound_events.len()` — cheap "how many sound
  /// events under this media" facet. Truth = per-track
  /// `AudioTrack.sound_events`.
  #[inline(always)]
  pub const fn total_sound_events(&self) -> u32 {
    self.total_sound_events
  }

  /// Per-kind indexing rollup over the facet's tracks.
  #[inline(always)]
  pub const fn track_progress_ref(&self) -> &IndexProgress {
    &self.track_progress
  }

  /// Builder: replace `tracks`.
  #[inline(always)]
  #[must_use]
  pub fn with_tracks(mut self, tracks: impl Into<Vec<Id>>) -> Self {
    self.tracks = tracks.into();
    self
  }

  /// Builder: replace `total_segments`.
  #[inline(always)]
  #[must_use]
  pub const fn with_total_segments(mut self, total: u32) -> Self {
    self.total_segments = total;
    self
  }

  /// Builder: replace `total_sound_events`.
  #[inline(always)]
  #[must_use]
  pub const fn with_total_sound_events(mut self, total: u32) -> Self {
    self.total_sound_events = total;
    self
  }

  /// Builder: replace the `track_progress` rollup.
  #[inline(always)]
  #[must_use]
  pub const fn with_track_progress(mut self, p: IndexProgress) -> Self {
    self.track_progress = p;
    self
  }

  /// In-place mutator for `tracks`.
  #[inline(always)]
  pub fn set_tracks(&mut self, tracks: impl Into<Vec<Id>>) -> &mut Self {
    self.tracks = tracks.into();
    self
  }

  /// In-place mutator for `total_segments`.
  #[inline(always)]
  pub const fn set_total_segments(&mut self, total: u32) -> &mut Self {
    self.total_segments = total;
    self
  }

  /// In-place mutator for `total_sound_events`.
  #[inline(always)]
  pub const fn set_total_sound_events(&mut self, total: u32) -> &mut Self {
    self.total_sound_events = total;
    self
  }

  /// In-place mutator for `track_progress`.
  #[inline(always)]
  pub const fn set_track_progress(&mut self, p: IndexProgress) -> &mut Self {
    self.track_progress = p;
    self
  }
}

/// Error returned by [`Audio::try_new`]. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum AudioError {
  /// Supplied `id` was the nil sentinel — not a real identity.
  #[error("Audio id must not be the nil UUID")]
  NilId,
  /// Supplied `media_id` was the nil sentinel — orphaned facet with no
  /// `Media` reference.
  #[error("Audio `media_id` (FK → Media) must not be the nil UUID")]
  NilMediaId,
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
    let media_id = Uuid7::new();
    let a = Audio::try_new(id, media_id).expect("valid construction must succeed");
    assert_eq!(a.id_ref(), &id);
    assert_eq!(a.media_id_ref(), &media_id);
    assert!(a.tracks_slice().is_empty());
    assert_eq!(a.total_segments(), 0);
    assert_eq!(a.total_sound_events(), 0);
    assert_eq!(a.track_progress_ref(), &IndexProgress::new());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = Audio::try_new(Uuid7::nil(), Uuid7::new());
    assert_eq!(r.err(), Some(AudioError::NilId));
    assert!(AudioError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_media_id() {
    let r = Audio::try_new(Uuid7::new(), Uuid7::nil());
    assert_eq!(r.err(), Some(AudioError::NilMediaId));
    assert!(AudioError::NilMediaId.is_nil_media_id());
  }

  #[test]
  fn media_id_ref_returns_constructed_media_id() {
    let media_id = Uuid7::new();
    let a = Audio::try_new(Uuid7::new(), media_id).unwrap();
    assert_eq!(a.media_id_ref(), &media_id);
  }

  #[test]
  fn builders_chain_tracks_and_rollup() {
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let p = IndexProgress::try_new(2, 1, 0).unwrap();
    let a = Audio::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![t1, t2])
      .with_total_segments(42)
      .with_total_sound_events(13)
      .with_track_progress(p);
    assert_eq!(a.tracks_slice().len(), 2);
    assert!(a.tracks_slice().contains(&t1));
    assert_eq!(a.total_segments(), 42);
    assert_eq!(a.total_sound_events(), 13);
    assert_eq!(a.track_progress_ref(), &p);
  }

  #[test]
  fn setters_mutate_in_place() {
    let mut a = Audio::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    a.set_tracks(std::vec![Uuid7::new()]);
    a.set_total_segments(7);
    a.set_total_sound_events(3);
    a.set_track_progress(IndexProgress::try_new(1, 1, 0).unwrap());
    assert_eq!(a.tracks_slice().len(), 1);
    assert_eq!(a.total_segments(), 7);
    assert_eq!(a.total_sound_events(), 3);
    assert_eq!(a.track_progress_ref().total(), 1);
    assert!(!a.track_progress_ref().has_failures());
  }

  #[test]
  fn fields_are_independent_across_mutators() {
    // Per validation-responsibility-boundary: replacing `tracks` does
    // NOT reset `total_segments` / `track_progress`. The DB / app layer
    // is the source of truth for rollups; the domain stores what the
    // caller puts in.
    let mut a = Audio::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![Uuid7::new(), Uuid7::new()])
      .with_total_segments(17)
      .with_total_sound_events(9)
      .with_track_progress(IndexProgress::try_new(2, 1, 1).unwrap());
    a.set_tracks(std::vec![Uuid7::new()]);
    assert_eq!(a.tracks_slice().len(), 1);
    // Rollups remain whatever the caller last set them to.
    assert_eq!(a.total_segments(), 17);
    assert_eq!(a.total_sound_events(), 9);
    assert_eq!(
      a.track_progress_ref(),
      &IndexProgress::try_new(2, 1, 1).unwrap()
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

/// Exhaustive by-value decomposition of [`Audio`] — every stored field.
///
/// Public-field data-transfer struct (the conversion-boundary exception
/// to the encapsulation rule): cross-suite conversions (`crate::graph`)
/// destructure it exhaustively, so adding a field breaks them at compile
/// time instead of silently dropping data.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioParts<Id = Uuid7> {
  pub id: Id,
  pub media_id: Id,
  pub tracks: Vec<Id>,
  pub total_segments: u32,
  pub total_sound_events: u32,
  pub track_progress: IndexProgress,
}

impl<Id> Audio<Id> {
  /// Decompose into [`AudioParts`] — exhaustive, by value.
  #[inline(always)]
  pub fn into_parts(self) -> AudioParts<Id> {
    let Self {
      id,
      media_id,
      tracks,
      total_segments,
      total_sound_events,
      track_progress,
    } = self;
    AudioParts {
      id,
      media_id,
      tracks,
      total_segments,
      total_sound_events,
      track_progress,
    }
  }
}

impl<Id> Audio<Id> {
  /// Invariant-carrying constructor from [`AudioParts`] — `pub(crate)`,
  /// reserved for in-crate conversions from already-validated values
  /// (`crate::graph`).
  #[cfg(all(
    feature = "std",
    feature = "video",
    feature = "audio",
    feature = "subtitle"
  ))]
  #[inline(always)]
  pub(crate) fn rehydrate(parts: AudioParts<Id>) -> Self {
    let AudioParts {
      id,
      media_id,
      tracks,
      total_segments,
      total_sound_events,
      track_progress,
    } = parts;
    Self {
      id,
      media_id,
      tracks,
      total_segments,
      total_sound_events,
      track_progress,
    }
  }
}
