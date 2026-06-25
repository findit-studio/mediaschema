//! Wire ⇄ graph conversions for the attachment subtree:
//! `media.v2::Attachment` ⇄ [`graph::Attachment`],
//! `media.v2::AttachmentTrack` ⇄ [`graph::AttachmentTrack`], and the
//! reserved `media.v2::BlobRef` ⇄ [`BlobRef`].
//!
//! Attachment tracks are flat presence-only streams (no Phase-B blocker),
//! so the encoders here are **infallible** `From<&graph::X>`. The reserved
//! `blob` slot is `None` in v1; it round-trips as unset.
//!
//! ## Field correspondence — `Attachment`
//!
//! | wire field                        | graph field          | notes                |
//! | --------------------------------- | -------------------- | -------------------- |
//! | `id` (bytes, 16)                  | `id`                 | validating           |
//! | `track_progress: IndexProgress`   | `track_progress`     | unset ⇒ empty rollup |
//! | `tracks: repeated AttachmentTrack`| `tracks: Vec<_>`     | children embedded    |
//!
//! ## Field correspondence — `AttachmentTrack`
//!
//! | wire field                            | graph field                         | notes                          |
//! | ------------------------------------- | ----------------------------------- | ------------------------------ |
//! | `id` (bytes, 16)                      | `id`                                | validating                     |
//! | `stream_index`                        | same                                |                                |
//! | `codec` / `filename` / `mimetype`     | `SmolStr`                           | `""` = unknown/absent          |
//! | `byte_size`                           | same                                |                                |
//! | `disposition: TrackDisposition`       | `disposition`                       | extern; unset ⇒ empty flags    |
//! | `metadata: repeated KeyValue`         | `metadata: IndexMap`                | insertion order preserved      |
//! | `index_status: uint32`                | `index_status: AttachmentIndexStatus` | raw bits / `from_bits_retain` |
//! | `index_errors: repeated ErrorInfo`    | `index_errors: Vec<_>`              |                                |
//! | `blob: BlobRef`                       | `blob: Option<BlobRef>`             | reserved; unset ⇒ `None`; empty `uri` rejected |

use buffa::MessageField;
use smol_str::SmolStr;

use super::{
  errors_from_wire, errors_to_wire, graph_err, id_from_wire, id_to_wire, index_progress_from_wire,
  index_progress_to_wire, metadata_from_wire, metadata_to_wire, rejected,
};
use crate::{
  buffa::error::BuffaError,
  domain::{self, AttachmentIndexStatus, BlobRef, Uuid7},
  generated::media::v2 as wire,
  graph,
};

// ---------------------------------------------------------------------------
// BlobRef ⇄ wire::BlobRef
// ---------------------------------------------------------------------------

impl From<&BlobRef> for wire::BlobRef {
  fn from(b: &BlobRef) -> Self {
    wire::BlobRef {
      uri: SmolStr::from(b.uri()),
      byte_size: b.byte_size(),
      content_type: SmolStr::from(b.content_type()),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Decode the reserved `blob` slot. Unset ⇒ `None` (the v1 contract);
/// a present `BlobRef` runs through the validating `try_new` (empty `uri`
/// rejected).
fn blob_from_wire(w: &MessageField<wire::BlobRef>) -> Result<Option<BlobRef>, BuffaError> {
  match w.as_option() {
    Some(b) => BlobRef::try_new(b.uri.as_str(), b.byte_size, b.content_type.as_str())
      .map(Some)
      .map_err(rejected),
    None => Ok(None),
  }
}

// ---------------------------------------------------------------------------
// graph::Attachment ⇄ wire::Attachment
// ---------------------------------------------------------------------------

impl From<&graph::Attachment<Uuid7>> for wire::Attachment {
  fn from(g: &graph::Attachment<Uuid7>) -> Self {
    wire::Attachment {
      id: id_to_wire(g.id_ref()),
      track_progress: index_progress_to_wire(g.track_progress_ref()),
      tracks: g
        .tracks_slice()
        .iter()
        .map(wire::AttachmentTrack::from)
        .collect(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Decode the facet and its track subtrees. The flat facet's `media_id`
/// is synthesized from the facet's own id (consumed by the lift).
impl TryFrom<&wire::Attachment> for graph::Attachment<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::Attachment) -> Result<Self, Self::Error> {
    let id = id_from_wire(&w.id, "Attachment.id")?;
    let tracks = w
      .tracks
      .iter()
      .map(|t| attachment_track_from_wire(t, id))
      .collect::<Result<Vec<_>, _>>()?;
    let flat = domain::Attachment::try_new(id, id)
      .map_err(rejected)?
      .with_track_progress(index_progress_from_wire(&w.track_progress)?);
    graph::Attachment::try_from_flat(&id, flat, tracks).map_err(graph_err)
  }
}

// ---------------------------------------------------------------------------
// graph::AttachmentTrack ⇄ wire::AttachmentTrack
// ---------------------------------------------------------------------------

impl From<&graph::AttachmentTrack<Uuid7>> for wire::AttachmentTrack {
  fn from(g: &graph::AttachmentTrack<Uuid7>) -> Self {
    wire::AttachmentTrack {
      id: id_to_wire(g.id_ref()),
      stream_index: g.stream_index(),
      codec: SmolStr::from(g.codec()),
      filename: SmolStr::from(g.filename()),
      mimetype: SmolStr::from(g.mimetype()),
      byte_size: g.byte_size(),
      disposition: MessageField::some(g.disposition()),
      metadata: metadata_to_wire(g.metadata_ref()),
      index_status: g.index_status().bits(),
      index_errors: errors_to_wire(g.index_errors_slice()),
      blob: match g.blob_ref() {
        Some(b) => MessageField::some(wire::BlobRef::from(b)),
        None => MessageField::none(),
      },
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Decode one track under the given parent facet id.
fn attachment_track_from_wire(
  w: &wire::AttachmentTrack,
  attachment_id: Uuid7,
) -> Result<graph::AttachmentTrack<Uuid7>, BuffaError> {
  let id = id_from_wire(&w.id, "AttachmentTrack.id")?;
  let mut t = domain::AttachmentTrack::try_new(id, attachment_id)
    .map_err(rejected)?
    .with_stream_index(w.stream_index)
    .with_codec(w.codec.as_str())
    .with_filename(w.filename.as_str())
    .with_mimetype(w.mimetype.as_str())
    .with_byte_size(w.byte_size)
    .with_index_status(AttachmentIndexStatus::from_bits_retain(w.index_status))
    .with_metadata(metadata_from_wire(&w.metadata))
    .with_index_errors(errors_from_wire(&w.index_errors))
    .with_blob(blob_from_wire(&w.blob)?);
  if let Some(v) = w.disposition.as_option() {
    t = t.with_disposition(*v);
  }
  graph::AttachmentTrack::try_from_flat(&attachment_id, t).map_err(graph_err)
}

/// Standalone decode — the parent FK is synthesized from the track's own
/// id and consumed by the lift.
impl TryFrom<&wire::AttachmentTrack> for graph::AttachmentTrack<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::AttachmentTrack) -> Result<Self, Self::Error> {
    let synthetic_parent = id_from_wire(&w.id, "AttachmentTrack.id")?;
    attachment_track_from_wire(w, synthetic_parent)
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::{ErrorCode, ErrorInfo, IndexProgress};

  fn rich_attachment_track(attachment_id: Uuid7) -> domain::AttachmentTrack<Uuid7> {
    domain::AttachmentTrack::try_new(Uuid7::new(), attachment_id)
      .expect("valid track")
      .with_stream_index(Some(4))
      .with_codec("ttf")
      .with_filename("font.ttf")
      .with_mimetype("font/ttf")
      .with_byte_size(4_096)
      .with_index_status(AttachmentIndexStatus::PROBED)
      .with_index_errors(vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "glitch")])
      .with_metadata({
        let mut bag = indexmap::IndexMap::new();
        bag.insert(SmolStr::from("filename"), SmolStr::from("font.ttf"));
        bag
      })
  }

  #[test]
  fn attachment_track_round_trips_blob_none() {
    let attachment_id = Uuid7::new();
    let g =
      graph::AttachmentTrack::try_from_flat(&attachment_id, rich_attachment_track(attachment_id))
        .expect("coherent");
    assert!(g.blob_ref().is_none());
    let w = wire::AttachmentTrack::from(&g);
    assert!(w.blob.is_unset(), "reserved blob must be wire-absent");
    let g2 = graph::AttachmentTrack::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
    assert!(g2.blob_ref().is_none());
  }

  #[test]
  fn attachment_track_round_trips_blob_some() {
    // The reserved slot is wire-capable even though v1 never populates it.
    let attachment_id = Uuid7::new();
    let blob = BlobRef::try_new("file:///x.ttf", 10, "font/ttf").expect("valid blob");
    let flat = rich_attachment_track(attachment_id).with_blob(Some(blob.clone()));
    let g = graph::AttachmentTrack::try_from_flat(&attachment_id, flat).expect("coherent");
    let w = wire::AttachmentTrack::from(&g);
    assert!(w.blob.as_option().is_some());
    let g2 = graph::AttachmentTrack::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
    assert_eq!(g2.blob_ref(), Some(&blob));
  }

  #[test]
  fn attachment_facet_round_trips() {
    let media_id = Uuid7::new();
    let facet = domain::Attachment::try_new(Uuid7::new(), media_id)
      .expect("valid facet")
      .with_track_progress(IndexProgress::try_new(1, 1, 0).expect("valid rollup"));
    let facet_id = *facet.id_ref();
    let track = graph::AttachmentTrack::try_from_flat(&facet_id, rich_attachment_track(facet_id))
      .expect("coherent");
    let g = graph::Attachment::try_from_flat(&media_id, facet, vec![track]).expect("coherent");
    let w = wire::Attachment::from(&g);
    let g2 = graph::Attachment::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }
}
