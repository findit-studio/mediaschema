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
  // TODO(mediaframe): `track_progress: IndexProgress` rollup — `IndexProgress`
  // is a future shared VO; deferred until the shared progress type lands.
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
}

/// Error returned when [`Audio::try_new`] cannot uphold the non-nil-id
/// invariant. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant)]
#[non_exhaustive]
pub enum AudioError {
  /// Supplied `id` was the nil sentinel — not a real identity.
  NilId,
}

impl core::fmt::Display for AudioError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::NilId => f.write_str("Audio id must not be the nil UUID"),
    }
  }
}

impl core::error::Error for AudioError {}

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
    let a = Audio::try_new(Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![t1, t2])
      .with_total_segments(42);
    assert_eq!(a.tracks().len(), 2);
    assert!(a.tracks().contains(&t1));
    assert_eq!(a.total_segments(), 42);
  }

  #[test]
  fn setters_mutate_in_place() {
    let mut a = Audio::try_new(Uuid7::new()).unwrap();
    a.set_tracks(std::vec![Uuid7::new()]);
    a.set_total_segments(7);
    assert_eq!(a.tracks().len(), 1);
    assert_eq!(a.total_segments(), 7);
  }
}
