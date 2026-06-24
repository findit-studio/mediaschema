//! `AttachmentTrack` — one attachment stream of an `Attachment` facet.
//!
//! Attachment tracks carry container-level attachment streams (FFmpeg
//! `codec_type=attachment`): fonts, cover art, thumbnails. v1 records
//! **presence + a descriptor + container metadata only** — a reserved
//! `blob: Option<BlobRef>` slot is declared but **never populated** (no
//! attachment bytes are stored). `mediaframe` ships no attachment codec, so
//! `codec` / `filename` / `mimetype` are plain [`SmolStr`] slugs (`""` =
//! unknown / absent).

use std::vec::Vec;

use derive_more::IsVariant;
use indexmap::IndexMap;
use mediaframe::disposition::TrackDisposition;
use smol_str::SmolStr;

use super::blob::BlobRef;
use crate::domain::{bitflags::AttachmentIndexStatus, primitives::ErrorInfo, Uuid7};

// ---------------------------------------------------------------------------
// AttachmentTrack
// ---------------------------------------------------------------------------

/// One attachment stream of an `Attachment` facet
/// (`attachment_id → Attachment.id`).
///
/// Generic over `Id` (default [`Uuid7`]). Presence + descriptor + metadata
/// only — the `blob` slot is reserved and always `None` in v1.
///
/// **No `Default`** — defaulting to a nil id + nil attachment_id is an
/// orphan state. Use [`AttachmentTrack::try_new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentTrack<Id = Uuid7> {
  id: Id,
  attachment_id: Id,
  stream_index: Option<u32>,
  /// Codec slug (`""` = unknown).
  codec: SmolStr,
  /// Attachment filename (`""` = absent).
  filename: SmolStr,
  /// MIME type (`""` = absent).
  mimetype: SmolStr,
  /// Declared attachment size (`0` = unknown).
  byte_size: u64,
  disposition: TrackDisposition,
  metadata: IndexMap<SmolStr, SmolStr>,
  index_status: AttachmentIndexStatus,
  index_errors: Vec<ErrorInfo>,
  /// **RESERVED** — externalization handle for the attachment bytes; never
  /// `Some` in v1 (no attachment bytes are stored).
  blob: Option<BlobRef>,
}

impl AttachmentTrack<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (every aggregate row needs a real identity) and nil
  /// `attachment_id` (orphan track with no `Attachment` facet). All
  /// descriptive fields start in their `""` / `None` / `0` neutral state
  /// — `blob` starts `None` (its reserved-but-unset contract).
  pub fn try_new(id: Uuid7, attachment_id: Uuid7) -> Result<Self, AttachmentTrackError> {
    if id.is_nil() {
      return Err(AttachmentTrackError::NilId);
    }
    if attachment_id.is_nil() {
      return Err(AttachmentTrackError::NilAttachmentId);
    }
    Ok(Self {
      id,
      attachment_id,
      stream_index: None,
      codec: SmolStr::default(),
      filename: SmolStr::default(),
      mimetype: SmolStr::default(),
      byte_size: 0,
      disposition: TrackDisposition::empty(),
      metadata: IndexMap::new(),
      index_status: AttachmentIndexStatus::empty(),
      index_errors: Vec::new(),
      blob: None,
    })
  }
}

impl<Id> AttachmentTrack<Id> {
  /// Canonical identity.
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// FK → `Attachment.id`.
  #[inline(always)]
  pub const fn attachment_id_ref(&self) -> &Id {
    &self.attachment_id
  }

  /// Source stream index (FFmpeg/container locator; not identity).
  #[inline(always)]
  pub const fn stream_index(&self) -> Option<u32> {
    self.stream_index
  }

  /// Codec slug (`""` = unknown).
  #[inline(always)]
  pub fn codec(&self) -> &str {
    self.codec.as_str()
  }

  /// Attachment filename (`""` = absent).
  #[inline(always)]
  pub fn filename(&self) -> &str {
    self.filename.as_str()
  }

  /// MIME type (`""` = absent).
  #[inline(always)]
  pub fn mimetype(&self) -> &str {
    self.mimetype.as_str()
  }

  /// Declared attachment size (`0` = unknown).
  #[inline(always)]
  pub const fn byte_size(&self) -> u64 {
    self.byte_size
  }

  /// Disposition flags (`AV_DISPOSITION_*` bitflags).
  #[inline(always)]
  pub const fn disposition(&self) -> TrackDisposition {
    self.disposition
  }

  /// Container `AVDictionary` entries. Insertion-ordered.
  #[inline(always)]
  pub const fn metadata_ref(&self) -> &IndexMap<SmolStr, SmolStr> {
    &self.metadata
  }

  /// Indexing state (single `PROBED` bit in v1).
  #[inline(always)]
  pub const fn index_status(&self) -> AttachmentIndexStatus {
    self.index_status
  }

  /// Per-track index errors (stage-coded `ErrorInfo.code`).
  #[inline(always)]
  pub fn index_errors_slice(&self) -> &[ErrorInfo] {
    self.index_errors.as_slice()
  }

  /// **Reserved** externalization handle — always `None` in v1 (no
  /// attachment bytes are stored).
  #[inline(always)]
  pub const fn blob_ref(&self) -> Option<&BlobRef> {
    self.blob.as_ref()
  }

  // ----- Builders ----------------------------------------------------------

  /// Builder: replace `stream_index`.
  #[inline(always)]
  #[must_use]
  pub const fn with_stream_index(mut self, v: Option<u32>) -> Self {
    self.stream_index = v;
    self
  }

  /// Builder: replace `codec`.
  #[inline(always)]
  #[must_use]
  pub fn with_codec(mut self, v: impl Into<SmolStr>) -> Self {
    self.codec = v.into();
    self
  }

  /// Builder: replace `filename`.
  #[inline(always)]
  #[must_use]
  pub fn with_filename(mut self, v: impl Into<SmolStr>) -> Self {
    self.filename = v.into();
    self
  }

  /// Builder: replace `mimetype`.
  #[inline(always)]
  #[must_use]
  pub fn with_mimetype(mut self, v: impl Into<SmolStr>) -> Self {
    self.mimetype = v.into();
    self
  }

  /// Builder: replace `byte_size`.
  #[inline(always)]
  #[must_use]
  pub const fn with_byte_size(mut self, v: u64) -> Self {
    self.byte_size = v;
    self
  }

  /// Builder: replace `disposition` flags.
  #[inline(always)]
  #[must_use]
  pub const fn with_disposition(mut self, v: TrackDisposition) -> Self {
    self.disposition = v;
    self
  }

  /// Builder: replace the container-`AVDictionary` metadata bag.
  #[inline(always)]
  #[must_use]
  pub fn with_metadata(mut self, v: IndexMap<SmolStr, SmolStr>) -> Self {
    self.metadata = v;
    self
  }

  /// Builder: replace `index_status`.
  #[inline(always)]
  #[must_use]
  pub const fn with_index_status(mut self, v: AttachmentIndexStatus) -> Self {
    self.index_status = v;
    self
  }

  /// Builder: replace `index_errors`.
  #[inline(always)]
  #[must_use]
  pub fn with_index_errors(mut self, v: impl Into<Vec<ErrorInfo>>) -> Self {
    self.index_errors = v.into();
    self
  }

  /// Builder: replace the **reserved** `blob` handle. v1 never populates
  /// it (no attachment bytes are stored); the setter exists only so the
  /// reserved slot is wired through the builder surface.
  #[inline(always)]
  #[must_use]
  pub fn with_blob(mut self, v: Option<BlobRef>) -> Self {
    self.blob = v;
    self
  }

  // ----- Setters -----------------------------------------------------------

  /// In-place mutator for `stream_index`.
  #[inline(always)]
  pub const fn set_stream_index(&mut self, v: Option<u32>) -> &mut Self {
    self.stream_index = v;
    self
  }

  /// In-place mutator for `codec`.
  #[inline(always)]
  pub fn set_codec(&mut self, v: impl Into<SmolStr>) -> &mut Self {
    self.codec = v.into();
    self
  }

  /// In-place mutator for `filename`.
  #[inline(always)]
  pub fn set_filename(&mut self, v: impl Into<SmolStr>) -> &mut Self {
    self.filename = v.into();
    self
  }

  /// In-place mutator for `mimetype`.
  #[inline(always)]
  pub fn set_mimetype(&mut self, v: impl Into<SmolStr>) -> &mut Self {
    self.mimetype = v.into();
    self
  }

  /// In-place mutator for `byte_size`.
  #[inline(always)]
  pub const fn set_byte_size(&mut self, v: u64) -> &mut Self {
    self.byte_size = v;
    self
  }

  /// In-place mutator for `disposition`.
  #[inline(always)]
  pub const fn set_disposition(&mut self, v: TrackDisposition) -> &mut Self {
    self.disposition = v;
    self
  }

  /// In-place mutator for the container-`AVDictionary` metadata bag.
  #[inline(always)]
  pub fn set_metadata(&mut self, v: IndexMap<SmolStr, SmolStr>) -> &mut Self {
    self.metadata = v;
    self
  }

  /// In-place mutator for `index_status`.
  #[inline(always)]
  pub const fn set_index_status(&mut self, v: AttachmentIndexStatus) -> &mut Self {
    self.index_status = v;
    self
  }

  /// In-place mutator for `index_errors`.
  #[inline(always)]
  pub fn set_index_errors(&mut self, v: impl Into<Vec<ErrorInfo>>) -> &mut Self {
    self.index_errors = v.into();
    self
  }

  /// In-place mutator for the **reserved** `blob` handle (always `None` in
  /// v1 — see [`AttachmentTrack::with_blob`]).
  #[inline(always)]
  pub fn set_blob(&mut self, v: Option<BlobRef>) -> &mut Self {
    self.blob = v;
    self
  }
}

/// Error returned by [`AttachmentTrack::try_new`]. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum AttachmentTrackError {
  /// Supplied `id` was the nil sentinel.
  #[error("AttachmentTrack id must not be the nil UUID")]
  NilId,
  /// Supplied `attachment_id` was the nil sentinel — orphaned track with
  /// no `Attachment` facet reference.
  #[error("AttachmentTrack `attachment_id` (FK → Attachment) must not be the nil UUID")]
  NilAttachmentId,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use crate::domain::{ErrorCode, ErrorInfo};

  #[test]
  fn try_new_happy_path() {
    let attachment_id = Uuid7::new();
    let t = AttachmentTrack::try_new(Uuid7::new(), attachment_id)
      .expect("valid construction must succeed");
    assert_eq!(t.attachment_id_ref(), &attachment_id);
    assert!(t.codec().is_empty());
    assert!(t.filename().is_empty());
    assert!(t.mimetype().is_empty());
    assert_eq!(t.byte_size(), 0);
    assert!(t.blob_ref().is_none(), "blob defaults to None");
    assert_eq!(t.index_status(), AttachmentIndexStatus::empty());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = AttachmentTrack::try_new(Uuid7::nil(), Uuid7::new());
    assert_eq!(r.err(), Some(AttachmentTrackError::NilId));
    assert!(AttachmentTrackError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_attachment_id() {
    let r = AttachmentTrack::try_new(Uuid7::new(), Uuid7::nil());
    assert_eq!(r.err(), Some(AttachmentTrackError::NilAttachmentId));
    assert!(AttachmentTrackError::NilAttachmentId.is_nil_attachment_id());
  }

  #[test]
  fn builders_chain() {
    let t = AttachmentTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_stream_index(Some(4))
      .with_codec("ttf")
      .with_filename("font.ttf")
      .with_mimetype("font/ttf")
      .with_byte_size(4_096)
      .with_index_status(AttachmentIndexStatus::PROBED)
      .with_index_errors(std::vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "x")]);
    assert_eq!(t.stream_index(), Some(4));
    assert_eq!(t.codec(), "ttf");
    assert_eq!(t.filename(), "font.ttf");
    assert_eq!(t.mimetype(), "font/ttf");
    assert_eq!(t.byte_size(), 4_096);
    assert!(t.index_status().contains(AttachmentIndexStatus::PROBED));
    assert_eq!(t.index_errors_slice().len(), 1);
    // The reserved blob slot stays None even through the builder chain.
    assert!(t.blob_ref().is_none());
  }

  #[test]
  fn blob_slot_round_trips_when_set_and_when_none() {
    // Reserved-but-settable: a `Some(BlobRef)` survives into_parts/rehydrate
    // (so the contract is testable), and the default `None` does too.
    let blob = BlobRef::try_new("file:///x.ttf", 10, "font/ttf").unwrap();
    let t = AttachmentTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_blob(Some(blob.clone()));
    let t2 = AttachmentTrack::rehydrate(t.clone().into_parts());
    assert_eq!(t, t2);
    assert_eq!(t2.blob_ref(), Some(&blob));

    let none = AttachmentTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    let none2 = AttachmentTrack::rehydrate(none.clone().into_parts());
    assert_eq!(none, none2);
    assert!(none2.blob_ref().is_none());
  }

  #[test]
  fn setters_mutate_in_place() {
    let mut t = AttachmentTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    t.set_codec("otf");
    t.set_filename("a.otf");
    t.set_byte_size(7);
    assert_eq!(t.codec(), "otf");
    assert_eq!(t.filename(), "a.otf");
    assert_eq!(t.byte_size(), 7);
  }
}

/// Exhaustive by-value decomposition of [`AttachmentTrack`] — every stored
/// field.
///
/// Public-field data-transfer struct (the conversion-boundary exception to
/// the encapsulation rule): cross-suite conversions (`crate::graph`)
/// destructure it exhaustively, so adding a field breaks them at compile
/// time instead of silently dropping data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentTrackParts<Id = Uuid7> {
  pub id: Id,
  pub attachment_id: Id,
  pub stream_index: Option<u32>,
  pub codec: SmolStr,
  pub filename: SmolStr,
  pub mimetype: SmolStr,
  pub byte_size: u64,
  pub disposition: TrackDisposition,
  pub metadata: IndexMap<SmolStr, SmolStr>,
  pub index_status: AttachmentIndexStatus,
  pub index_errors: Vec<ErrorInfo>,
  pub blob: Option<BlobRef>,
}

impl<Id> AttachmentTrack<Id> {
  /// Decompose into [`AttachmentTrackParts`] — exhaustive, by value.
  #[inline(always)]
  pub fn into_parts(self) -> AttachmentTrackParts<Id> {
    let Self {
      id,
      attachment_id,
      stream_index,
      codec,
      filename,
      mimetype,
      byte_size,
      disposition,
      metadata,
      index_status,
      index_errors,
      blob,
    } = self;
    AttachmentTrackParts {
      id,
      attachment_id,
      stream_index,
      codec,
      filename,
      mimetype,
      byte_size,
      disposition,
      metadata,
      index_status,
      index_errors,
      blob,
    }
  }
}

impl<Id> AttachmentTrack<Id> {
  /// Invariant-carrying constructor from [`AttachmentTrackParts`] —
  /// `pub(crate)`, reserved for in-crate conversions from already-validated
  /// values (`crate::graph`).
  #[cfg(all(
    feature = "std",
    feature = "video",
    feature = "audio",
    feature = "subtitle"
  ))]
  #[inline(always)]
  pub(crate) fn rehydrate(parts: AttachmentTrackParts<Id>) -> Self {
    let AttachmentTrackParts {
      id,
      attachment_id,
      stream_index,
      codec,
      filename,
      mimetype,
      byte_size,
      disposition,
      metadata,
      index_status,
      index_errors,
      blob,
    } = parts;
    Self {
      id,
      attachment_id,
      stream_index,
      codec,
      filename,
      mimetype,
      byte_size,
      disposition,
      metadata,
      index_status,
      index_errors,
      blob,
    }
  }
}
