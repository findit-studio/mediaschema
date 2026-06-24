//! Attachment subtree: facet → tracks. Standalone field owners — no
//! embedded flat aggregates, no parent FKs, no id-vecs. Presence-only: an
//! attachment track has no analysis children below it.

use indexmap::IndexMap;
use mediaframe::disposition::TrackDisposition;
use smol_str::SmolStr;

use super::{parent_check, GraphError, NodeKind};
use crate::domain::{
  self,
  aggregates::attachment::{facet::AttachmentParts, track::AttachmentTrackParts, BlobRef},
  AttachmentIndexStatus, ErrorInfo, IndexProgress, Uuid7,
};

/// The attachment facet with its complete track subtrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment<Id = Uuid7> {
  id: Id,
  track_progress: IndexProgress,
  tracks: Vec<AttachmentTrack<Id>>,
}

impl Attachment<Uuid7> {
  /// Lift the flat facet; validates `media_id == expected_media`. Tracks
  /// arrive pre-lifted (their `attachment_id` was consumed by their lift).
  pub fn try_from_flat(
    expected_media: &Uuid7,
    facet: domain::Attachment<Uuid7>,
    tracks: Vec<AttachmentTrack<Uuid7>>,
  ) -> Result<Self, GraphError> {
    let AttachmentParts {
      id,
      media_id,
      track_progress,
      tracks: _,
    } = facet.into_parts();
    parent_check(NodeKind::AttachmentFacet, id, &media_id, expected_media)?;
    Ok(Self {
      id,
      track_progress,
      tracks,
    })
  }
}

impl<Id> Attachment<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  #[inline(always)]
  pub const fn track_progress_ref(&self) -> &IndexProgress {
    &self.track_progress
  }

  /// The track subtrees, in container stream order.
  #[inline(always)]
  pub const fn tracks_slice(&self) -> &[AttachmentTrack<Id>] {
    self.tracks.as_slice()
  }
}

/// One attachment track — every field of the flat `AttachmentTrack` except
/// `attachment_id` (implied by nesting).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentTrack<Id = Uuid7> {
  id: Id,
  stream_index: Option<u32>,
  codec: SmolStr,
  filename: SmolStr,
  mimetype: SmolStr,
  byte_size: u64,
  disposition: TrackDisposition,
  metadata: IndexMap<SmolStr, SmolStr>,
  index_status: AttachmentIndexStatus,
  index_errors: Vec<ErrorInfo>,
  blob: Option<BlobRef>,
}

impl AttachmentTrack<Uuid7> {
  /// Lift the flat track; validates `attachment_id == expected_attachment`.
  pub fn try_from_flat(
    expected_attachment: &Uuid7,
    track: domain::AttachmentTrack<Uuid7>,
  ) -> Result<Self, GraphError> {
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
    } = track.into_parts();
    parent_check(
      NodeKind::AttachmentTrack,
      id,
      &attachment_id,
      expected_attachment,
    )?;
    Ok(Self {
      id,
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
    })
  }
}

impl<Id> AttachmentTrack<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

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

  #[inline(always)]
  pub const fn byte_size(&self) -> u64 {
    self.byte_size
  }

  #[inline(always)]
  pub const fn disposition(&self) -> TrackDisposition {
    self.disposition
  }

  #[inline(always)]
  pub const fn metadata_ref(&self) -> &IndexMap<SmolStr, SmolStr> {
    &self.metadata
  }

  #[inline(always)]
  pub const fn index_status(&self) -> AttachmentIndexStatus {
    self.index_status
  }

  #[inline(always)]
  pub const fn index_errors_slice(&self) -> &[ErrorInfo] {
    self.index_errors.as_slice()
  }

  /// **Reserved** externalization handle — always `None` in v1.
  #[inline(always)]
  pub const fn blob_ref(&self) -> Option<&BlobRef> {
    self.blob.as_ref()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn coherent_attachment_subtree_lifts() {
    let media_id = Uuid7::new();
    let facet = domain::Attachment::try_new(Uuid7::new(), media_id).expect("valid facet");
    let facet_id = *facet.id_ref();
    let track = domain::AttachmentTrack::try_new(Uuid7::new(), facet_id)
      .expect("valid track")
      .with_codec("ttf")
      .with_filename("font.ttf");
    let g_track = AttachmentTrack::try_from_flat(&facet_id, track).expect("coherent");
    assert_eq!(g_track.filename(), "font.ttf");
    assert!(g_track.blob_ref().is_none());
    let g = Attachment::try_from_flat(&media_id, facet, vec![g_track]).expect("coherent");
    assert_eq!(g.tracks_slice().len(), 1);
  }

  #[test]
  fn track_under_wrong_facet_is_rejected() {
    let track = domain::AttachmentTrack::try_new(Uuid7::new(), Uuid7::new()).expect("valid track");
    let err = AttachmentTrack::try_from_flat(&Uuid7::new(), track).expect_err("incoherent");
    assert!(matches!(
      err,
      GraphError::ParentMismatch {
        kind: NodeKind::AttachmentTrack,
        ..
      }
    ));
  }

  #[test]
  fn facet_under_wrong_media_is_rejected() {
    let facet = domain::Attachment::try_new(Uuid7::new(), Uuid7::new()).expect("valid facet");
    let err = Attachment::try_from_flat(&Uuid7::new(), facet, vec![]).expect_err("incoherent");
    assert!(matches!(
      err,
      GraphError::ParentMismatch {
        kind: NodeKind::AttachmentFacet,
        ..
      }
    ));
  }
}

// --- conversion traits: flat ⇄ graph ---------------------------------------

/// Trait form of [`Attachment::try_from_flat`] —
/// `(expected_media, facet, tracks)`.
impl
  TryFrom<(
    Uuid7,
    domain::Attachment<Uuid7>,
    Vec<AttachmentTrack<Uuid7>>,
  )> for Attachment<Uuid7>
{
  type Error = GraphError;

  #[inline(always)]
  fn try_from(
    (expected_media, facet, tracks): (
      Uuid7,
      domain::Attachment<Uuid7>,
      Vec<AttachmentTrack<Uuid7>>,
    ),
  ) -> Result<Self, Self::Error> {
    Self::try_from_flat(&expected_media, facet, tracks)
  }
}

/// Re-attach to `media_id` and rebuild the flat facet; the track-id vec is
/// re-derived from the embedded tracks, which are then dropped — convert
/// them first when persisting the tree.
impl From<(Uuid7, Attachment<Uuid7>)> for domain::Attachment<Uuid7> {
  fn from((media_id, g): (Uuid7, Attachment<Uuid7>)) -> Self {
    let Attachment {
      id,
      track_progress,
      tracks,
    } = g;
    domain::Attachment::rehydrate(AttachmentParts {
      id,
      media_id,
      track_progress,
      tracks: tracks.iter().map(|t| *t.id_ref()).collect(),
    })
  }
}

/// Trait form of [`AttachmentTrack::try_from_flat`] —
/// `(expected_attachment, track)`.
impl TryFrom<(Uuid7, domain::AttachmentTrack<Uuid7>)> for AttachmentTrack<Uuid7> {
  type Error = GraphError;

  #[inline(always)]
  fn try_from(
    (expected_attachment, track): (Uuid7, domain::AttachmentTrack<Uuid7>),
  ) -> Result<Self, Self::Error> {
    Self::try_from_flat(&expected_attachment, track)
  }
}

/// Re-attach to `attachment_id` and rebuild the flat track.
impl From<(Uuid7, AttachmentTrack<Uuid7>)> for domain::AttachmentTrack<Uuid7> {
  fn from((attachment_id, g): (Uuid7, AttachmentTrack<Uuid7>)) -> Self {
    let AttachmentTrack {
      id,
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
    } = g;
    domain::AttachmentTrack::rehydrate(AttachmentTrackParts {
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
    })
  }
}

#[cfg(test)]
mod conv_tests {
  use super::*;

  #[test]
  fn track_round_trips_through_graph() {
    let attachment_id = Uuid7::new();
    let flat = domain::AttachmentTrack::try_new(Uuid7::new(), attachment_id)
      .expect("valid track")
      .with_codec("ttf")
      .with_filename("a.ttf")
      .with_byte_size(42);
    let lifted: AttachmentTrack<Uuid7> =
      (attachment_id, flat.clone()).try_into().expect("coherent");
    let back: domain::AttachmentTrack<Uuid7> = (attachment_id, lifted).into();
    assert_eq!(back, flat);
  }

  #[test]
  fn facet_round_trips_through_graph() {
    let media_id = Uuid7::new();
    let flat = domain::Attachment::try_new(Uuid7::new(), media_id)
      .expect("valid facet")
      .with_track_progress(IndexProgress::try_new(1, 1, 0).unwrap());
    let lifted: Attachment<Uuid7> = (media_id, flat.clone(), vec![])
      .try_into()
      .expect("coherent");
    let back: domain::Attachment<Uuid7> = (media_id, lifted).into();
    assert_eq!(back, flat);
  }
}
