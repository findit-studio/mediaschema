//! `Attachment` — attachment facet (thin aggregate).
//!
//! The facet groups this media's attachment (`codec_type=attachment`)
//! tracks plus an indexing rollup, symmetric with the `Audio` / `Subtitle`
//! / `Data` facets. Presence + metadata only — no attachment bytes.
//!
//! ### Validation-responsibility boundary
//!
//! The facet stores `tracks` (id refs) and `track_progress` (per-kind
//! rollup over `tracks`) as **independent fields** — keeping them
//! consistent is a cross-field concern owned by the application / storage
//! layer. The domain type enforces only intrinsic single-value invariants
//! (here: non-nil `id` / `media_id`).

use std::vec::Vec;

use derive_more::IsVariant;

use crate::domain::{vo::IndexProgress, Uuid7};

// ---------------------------------------------------------------------------
// Attachment — the thin facet aggregate
// ---------------------------------------------------------------------------

/// Attachment facet of a `Media`. FK `media_id → Media` (referenced by
/// `Media.attachment_id`).
///
/// Generic over `Id` (default [`Uuid7`]). The `tracks` vector holds refs to
/// child `AttachmentTrack`s; `track_progress` is the per-kind index rollup
/// over `tracks`. Both are independent fields — see the module-level note
/// on the validation-responsibility boundary.
///
/// **No `Default`** — a facet with nil `id` would be an orphan record. Use
/// [`Attachment::try_new`] for the canonical `Uuid7` identity type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Attachment<Id = Uuid7> {
  id: Id,
  media_id: Id,
  track_progress: IndexProgress,
  tracks: Vec<Id>,
}

impl Attachment<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (every aggregate row needs a real identity) and nil
  /// `media_id` (orphaned facet with no `Media` reference). The `tracks`
  /// list starts empty and `track_progress` as the empty rollup
  /// (`{0, 0, 0}`); both are populated by builders/mutators as tracks are
  /// attached — or assembled directly from a database row.
  pub fn try_new(id: Uuid7, media_id: Uuid7) -> Result<Self, AttachmentError> {
    if id.is_nil() {
      return Err(AttachmentError::NilId);
    }
    if media_id.is_nil() {
      return Err(AttachmentError::NilMediaId);
    }
    Ok(Self {
      id,
      media_id,
      track_progress: IndexProgress::new(),
      tracks: Vec::new(),
    })
  }
}

impl<Id> Attachment<Id> {
  /// Canonical identity (also referenced by `Media.attachment_id`).
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

  /// Refs to child `AttachmentTrack`s.
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

/// Error returned by [`Attachment::try_new`]. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum AttachmentError {
  /// Supplied `id` was the nil sentinel — not a real identity.
  #[error("Attachment facet id must not be the nil UUID")]
  NilId,
  /// Supplied `media_id` was the nil sentinel — orphaned facet with no
  /// `Media` reference.
  #[error("Attachment `media_id` (FK → Media) must not be the nil UUID")]
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
    let a = Attachment::try_new(id, media_id).expect("valid construction must succeed");
    assert_eq!(a.id_ref(), &id);
    assert_eq!(a.media_id_ref(), &media_id);
    assert!(a.tracks_slice().is_empty());
    assert_eq!(a.track_progress_ref(), &IndexProgress::new());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = Attachment::try_new(Uuid7::nil(), Uuid7::new());
    assert_eq!(r.err(), Some(AttachmentError::NilId));
    assert!(AttachmentError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_media_id() {
    let r = Attachment::try_new(Uuid7::new(), Uuid7::nil());
    assert_eq!(r.err(), Some(AttachmentError::NilMediaId));
    assert!(AttachmentError::NilMediaId.is_nil_media_id());
  }

  #[cfg(all(feature = "video", feature = "audio", feature = "subtitle"))]
  #[test]
  fn into_parts_and_rehydrate_round_trip() {
    let a = Attachment::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_tracks(std::vec![Uuid7::new()])
      .with_track_progress(IndexProgress::try_new(1, 1, 0).unwrap());
    let parts = a.clone().into_parts();
    let a2 = Attachment::rehydrate(parts);
    assert_eq!(a, a2);
  }
}

/// Exhaustive by-value decomposition of [`Attachment`] — every stored
/// field.
///
/// Public-field data-transfer struct (the conversion-boundary exception to
/// the encapsulation rule): cross-suite conversions (`crate::graph`)
/// destructure it exhaustively, so adding a field breaks them at compile
/// time instead of silently dropping data.
#[derive(Debug, Clone, PartialEq)]
pub struct AttachmentParts<Id = Uuid7> {
  pub id: Id,
  pub media_id: Id,
  pub track_progress: IndexProgress,
  pub tracks: Vec<Id>,
}

impl<Id> Attachment<Id> {
  /// Decompose into [`AttachmentParts`] — exhaustive, by value.
  #[inline(always)]
  pub fn into_parts(self) -> AttachmentParts<Id> {
    let Self {
      id,
      media_id,
      track_progress,
      tracks,
    } = self;
    AttachmentParts {
      id,
      media_id,
      track_progress,
      tracks,
    }
  }
}

impl<Id> Attachment<Id> {
  /// Invariant-carrying constructor from [`AttachmentParts`] — `pub(crate)`,
  /// reserved for in-crate conversions from already-validated values
  /// (`crate::graph`).
  #[cfg(all(
    feature = "std",
    feature = "video",
    feature = "audio",
    feature = "subtitle"
  ))]
  #[inline(always)]
  pub(crate) fn rehydrate(parts: AttachmentParts<Id>) -> Self {
    let AttachmentParts {
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
