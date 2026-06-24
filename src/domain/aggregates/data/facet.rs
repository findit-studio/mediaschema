//! `Data` — data facet (thin aggregate).
//!
//! The facet groups this media's timed-metadata (`codec_type=data`) tracks
//! plus an indexing rollup, symmetric with the `Audio` / `Subtitle`
//! facets. Presence + metadata only — no sample payloads.
//!
//! ### Validation-responsibility boundary
//!
//! The facet stores `tracks` (id refs) and `track_progress` (per-kind
//! rollup over `tracks`) as **independent fields** — keeping them
//! consistent is a cross-field concern owned by the application / storage
//! layer (the database is the source of truth for rollups). The domain
//! type enforces only intrinsic single-value invariants (here: non-nil
//! `id` / `media_id`).

use std::vec::Vec;

use derive_more::IsVariant;

use crate::domain::{vo::IndexProgress, Uuid7};

// ---------------------------------------------------------------------------
// Data — the thin facet aggregate
// ---------------------------------------------------------------------------

/// Data facet of a `Media`. FK `media_id → Media` (referenced by
/// `Media.data_id`).
///
/// Generic over `Id` (default [`Uuid7`]). The `tracks` vector holds refs to
/// child `DataTrack`s; `track_progress` is the per-kind index rollup over
/// `tracks`. Both are independent fields — see the module-level note on the
/// validation-responsibility boundary.
///
/// **No `Default`** — a facet with nil `id` would be an orphan record. Use
/// [`Data::try_new`] for the canonical `Uuid7` identity type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Data<Id = Uuid7> {
  id: Id,
  media_id: Id,
  track_progress: IndexProgress,
  tracks: Vec<Id>,
}

impl Data<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (every aggregate row needs a real identity) and nil
  /// `media_id` (orphaned facet with no `Media` reference). The `tracks`
  /// list starts empty and `track_progress` as the empty rollup
  /// (`{0, 0, 0}`); both are populated by builders/mutators as tracks are
  /// attached — or assembled directly from a database row.
  pub fn try_new(id: Uuid7, media_id: Uuid7) -> Result<Self, DataError> {
    if id.is_nil() {
      return Err(DataError::NilId);
    }
    if media_id.is_nil() {
      return Err(DataError::NilMediaId);
    }
    Ok(Self {
      id,
      media_id,
      track_progress: IndexProgress::new(),
      tracks: Vec::new(),
    })
  }
}

impl<Id> Data<Id> {
  /// Canonical identity (also referenced by `Media.data_id`).
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

  /// Per-kind indexing rollup over the facet's tracks.
  #[inline(always)]
  pub const fn track_progress_ref(&self) -> &IndexProgress {
    &self.track_progress
  }

  /// Refs to child `DataTrack`s.
  #[inline(always)]
  pub const fn tracks_slice(&self) -> &[Id] {
    self.tracks.as_slice()
  }

  /// Builder: replace the `track_progress` rollup.
  #[inline(always)]
  #[must_use]
  pub const fn with_track_progress(mut self, p: IndexProgress) -> Self {
    self.track_progress = p;
    self
  }

  /// Builder: replace `tracks`.
  #[inline(always)]
  #[must_use]
  pub fn with_tracks(mut self, tracks: impl Into<Vec<Id>>) -> Self {
    self.tracks = tracks.into();
    self
  }

  /// In-place mutator for `track_progress`.
  #[inline(always)]
  pub const fn set_track_progress(&mut self, p: IndexProgress) -> &mut Self {
    self.track_progress = p;
    self
  }

  /// In-place mutator for `tracks`.
  #[inline(always)]
  pub fn set_tracks(&mut self, tracks: impl Into<Vec<Id>>) -> &mut Self {
    self.tracks = tracks.into();
    self
  }
}

/// Error returned by [`Data::try_new`]. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum DataError {
  /// Supplied `id` was the nil sentinel — not a real identity.
  #[error("Data facet id must not be the nil UUID")]
  NilId,
  /// Supplied `media_id` was the nil sentinel — orphaned facet with no
  /// `Media` reference.
  #[error("Data `media_id` (FK → Media) must not be the nil UUID")]
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
    let id = Uuid7::new();
    let media_id = Uuid7::new();
    let d = Data::try_new(id, media_id).expect("valid construction must succeed");
    assert_eq!(d.id_ref(), &id);
    assert_eq!(d.media_id_ref(), &media_id);
    assert!(d.tracks_slice().is_empty());
    assert_eq!(d.track_progress_ref(), &IndexProgress::new());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = Data::try_new(Uuid7::nil(), Uuid7::new());
    assert_eq!(r.err(), Some(DataError::NilId));
    assert!(DataError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_media_id() {
    let r = Data::try_new(Uuid7::new(), Uuid7::nil());
    assert_eq!(r.err(), Some(DataError::NilMediaId));
    assert!(DataError::NilMediaId.is_nil_media_id());
  }

  #[test]
  fn builders_chain_tracks_and_rollup() {
    let t1 = Uuid7::new();
    let p = IndexProgress::try_new(1, 1, 0).unwrap();
    let d = Data::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![t1])
      .with_track_progress(p);
    assert_eq!(d.tracks_slice(), &[t1]);
    assert_eq!(d.track_progress_ref(), &p);
  }

  #[test]
  fn into_parts_and_rehydrate_round_trip() {
    let d = Data::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![Uuid7::new()]);
    let parts = d.clone().into_parts();
    let d2 = Data::rehydrate(parts);
    assert_eq!(d, d2);
  }
}

/// Exhaustive by-value decomposition of [`Data`] — every stored field.
///
/// Public-field data-transfer struct (the conversion-boundary exception to
/// the encapsulation rule): cross-suite conversions (`crate::graph`)
/// destructure it exhaustively, so adding a field breaks them at compile
/// time instead of silently dropping data.
#[derive(Debug, Clone, PartialEq)]
pub struct DataParts<Id = Uuid7> {
  pub id: Id,
  pub media_id: Id,
  pub track_progress: IndexProgress,
  pub tracks: Vec<Id>,
}

impl<Id> Data<Id> {
  /// Decompose into [`DataParts`] — exhaustive, by value.
  #[inline(always)]
  pub fn into_parts(self) -> DataParts<Id> {
    let Self {
      id,
      media_id,
      track_progress,
      tracks,
    } = self;
    DataParts {
      id,
      media_id,
      track_progress,
      tracks,
    }
  }
}

impl<Id> Data<Id> {
  /// Invariant-carrying constructor from [`DataParts`] — `pub(crate)`,
  /// reserved for in-crate conversions from already-validated values
  /// (`crate::graph`).
  #[cfg(all(
    feature = "std",
    feature = "video",
    feature = "audio",
    feature = "subtitle"
  ))]
  #[inline(always)]
  pub(crate) fn rehydrate(parts: DataParts<Id>) -> Self {
    let DataParts {
      id,
      media_id,
      track_progress,
      tracks,
    } = parts;
    Self {
      id,
      media_id,
      track_progress,
      tracks,
    }
  }
}
