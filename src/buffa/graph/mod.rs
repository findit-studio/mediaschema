//! Wire ⇄ graph conversion bridge for the `media.v2` graph surface.
//!
//! `media.v2` mirrors [`crate::graph`] — the whole-record programming
//! shape: children are embedded (no parent-FK fields; the parent is the
//! message you are inside), each node keeps its own `id` plus the
//! cross-tree association ids (`Speaker.person_id`,
//! `AudioSegment.speaker_id`, `MediaFile.watched_location_id`).
//!
//! ## Directionality
//!
//! - **Encode** (`graph → wire`) is `From<&graph::X>` — infallible —
//!   except where the phase-A constraint applies: `media.v2` has no
//!   `Scene` / `Keyframe` messages yet, so [`Media`](crate::graph::Media)
//!   / [`Video`](crate::graph::Video) /
//!   [`VideoTrack`](crate::graph::VideoTrack) encode via `TryFrom` and a
//!   `graph::VideoTrack` with populated scenes fails with
//!   [`BuffaError::Unsupported`].
//! - **Decode** (`wire → graph`) is `TryFrom<&wire::X>`. Each node
//!   first reconstructs the **flat** `crate::domain` aggregate through
//!   its `try_new` constructor + (`try_`)`with_*` builder chain — the
//!   same validated door live application code uses — then lifts it via
//!   `graph::X::try_from_flat(...)`.
//!
//! ## Parent ids on decode
//!
//! The wire shape carries no parent FKs, but the flat constructors
//! require them. While decoding a *parent* message the already-parsed
//! parent id is threaded into each child's flat constructor (e.g. the
//! media's id becomes each flat `Chapter.media_id`). For the
//! **standalone** `TryFrom<&wire::X>` of a non-root node the parent FK
//! is synthesized from the node's **own id**: the lift's parent check
//! consumes it and the graph shape never stores it, so the result is
//! identical for any non-nil choice.
//!
//! ## Conventions (locked in `proto/media/v2/graph.proto`)
//!
//! | wire shape                          | domain/graph shape           | rule                                            |
//! | ----------------------------------- | ---------------------------- | ----------------------------------------------- |
//! | `bytes` id (16)                     | `Uuid7`                      | validating ([`BuffaError::IdWrongLength`] / [`BuffaError::IdInvalid`]) |
//! | `bytes` checksum (32)               | `FileChecksum`               | validating ([`BuffaError::ChecksumWrongLength`]) |
//! | `int64` epoch millis                | `jiff::Timestamp`            | proto3 `optional`; out-of-range ⇒ [`BuffaError::TimestampOutOfRange`] |
//! | `mediatime.v1.*` messages           | `mediatime::*`               | extern, self-contained timebase, `Copy`         |
//! | `mediaframe.v1.*` messages          | `mediaframe::*`              | extern; owned copies both ways                  |
//! | `uint32` raw bits                   | `bitflags!` companions       | encode `.bits()`; decode `from_bits_retain`     |
//! | `repeated media.v1.KeyValue`        | `IndexMap<SmolStr, SmolStr>` | insertion order preserved                       |
//! | codec slug `string`                 | `mediaframe::codec::*`       | `as_str()` out; total `FromStr` back            |
//! | vocabulary slug `string`            | `AudioContentKind` / `SubtitleKind` | `as_str()` out; unknown slug ⇒ [`BuffaError::DomainConstructorRejected`] |
//! | widened `uint32`                    | `u16` / `u8`                 | overflow ⇒ [`BuffaError::Unsupported`]          |
//!
//! Singular extern message fields backing a **non-optional** domain
//! field decode an unset slot as the domain default (proto3 "unset =
//! default"); fields backing an `Option<_>` decode unset as `None`.
//! `GraphError` from a lift (unreachable for frames produced by this
//! encoder — the parent ids are threaded, not transmitted) surfaces as
//! [`BuffaError::DomainConstructorRejected`].

use buffa::{bytes::Bytes, MessageField};
use indexmap::IndexMap;
use jiff::Timestamp as JiffTimestamp;
use smol_str::SmolStr;

use crate::{
  buffa::error::BuffaError,
  domain::{ErrorInfo, FileChecksum, IndexProgress, Provenance, Uuid7},
  generated::media::{v1 as wire1, v2 as wire},
  graph::GraphError,
};

pub mod audio;
pub mod media;
pub mod subtitle;
pub mod video;

// ---------------------------------------------------------------------------
// IndexProgress ⇄ wire::IndexProgress
// ---------------------------------------------------------------------------

/// Encode the facet rollup; field-for-field.
impl From<&IndexProgress> for wire::IndexProgress {
  fn from(d: &IndexProgress) -> Self {
    wire::IndexProgress {
      total: d.total(),
      indexed: d.indexed(),
      failed: d.failed(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Decode the facet rollup through the validating constructor
/// (`indexed + failed <= total`).
impl TryFrom<&wire::IndexProgress> for IndexProgress {
  type Error = BuffaError;

  fn try_from(w: &wire::IndexProgress) -> Result<Self, Self::Error> {
    IndexProgress::try_new(w.total, w.indexed, w.failed).map_err(rejected)
  }
}

/// Encode a facet's `track_progress` into its wire slot.
fn index_progress_to_wire(d: &IndexProgress) -> MessageField<wire::IndexProgress> {
  MessageField::some(wire::IndexProgress::from(d))
}

/// Decode a facet's `track_progress` slot. Unset ⇒ the empty rollup
/// (`{0, 0, 0}` — the facet constructor's default).
fn index_progress_from_wire(
  w: &MessageField<wire::IndexProgress>,
) -> Result<IndexProgress, BuffaError> {
  match w.as_option() {
    Some(v) => IndexProgress::try_from(v),
    None => Ok(IndexProgress::new()),
  }
}

// ---------------------------------------------------------------------------
// Shared field helpers
// ---------------------------------------------------------------------------

/// Decode a 16-byte wire id into the validating domain `Uuid7`.
fn id_from_wire(b: &Bytes, field: &'static str) -> Result<Uuid7, BuffaError> {
  let arr: [u8; 16] = b
    .as_ref()
    .try_into()
    .map_err(|_| BuffaError::IdWrongLength(b.len()))?;
  Uuid7::try_from_bytes(arr).map_err(|e| {
    // `field` is the source-side label; the typed error already carries
    // the specifics of *why* the layout failed.
    let _ = field;
    BuffaError::from(e)
  })
}

/// Encode a domain `Uuid7` as the inline 16-byte wire id.
fn id_to_wire(id: &Uuid7) -> Bytes {
  Bytes::copy_from_slice(id.as_bytes())
}

/// Decode a 32-byte wire checksum into the domain `FileChecksum`.
fn checksum_from_wire(b: &Bytes) -> Result<FileChecksum, BuffaError> {
  let arr: [u8; 32] = b
    .as_ref()
    .try_into()
    .map_err(|_| BuffaError::ChecksumWrongLength(b.len()))?;
  Ok(FileChecksum::from_bytes(arr))
}

/// Encode a domain `FileChecksum` as the inline 32-byte wire bytes.
fn checksum_to_wire(c: &FileChecksum) -> Bytes {
  Bytes::copy_from_slice(c.as_bytes())
}

/// `Option<T>` → singular message-field slot (`None` ⇒ unset).
fn opt_msg<T: Default>(v: Option<T>) -> MessageField<T> {
  match v {
    Some(v) => MessageField::some(v),
    None => MessageField::none(),
  }
}

/// Insertion-ordered metadata bag → `repeated media.v1.KeyValue`.
fn metadata_to_wire(d: &IndexMap<SmolStr, SmolStr>) -> Vec<wire1::KeyValue> {
  d.iter()
    .map(|(k, v)| wire1::KeyValue {
      key: SmolStr::from(k.as_str()),
      value: SmolStr::from(v.as_str()),
      __buffa_unknown_fields: Default::default(),
    })
    .collect()
}

/// `repeated media.v1.KeyValue` → insertion-ordered metadata bag.
fn metadata_from_wire(w: &[wire1::KeyValue]) -> IndexMap<SmolStr, SmolStr> {
  let mut bag = IndexMap::with_capacity(w.len());
  for kv in w {
    bag.insert(
      SmolStr::from(kv.key.as_str()),
      SmolStr::from(kv.value.as_str()),
    );
  }
  bag
}

/// `index_errors` list → `repeated media.v1.ErrorInfo`.
fn errors_to_wire(d: &[ErrorInfo]) -> Vec<wire1::ErrorInfo> {
  d.iter().map(wire1::ErrorInfo::from).collect()
}

/// `repeated media.v1.ErrorInfo` → `index_errors` list (lossless — wire
/// codes the domain doesn't know land in `ErrorCode::Unknown`).
fn errors_from_wire(w: &[wire1::ErrorInfo]) -> Vec<ErrorInfo> {
  w.iter().map(ErrorInfo::from).collect()
}

/// Encode a track's `provenance` into its wire slot.
fn provenance_to_wire(d: &Provenance) -> MessageField<wire1::Provenance> {
  MessageField::some(wire1::Provenance::from(d))
}

/// Decode a track's `provenance` slot. Unset ⇒ the all-empty
/// `Provenance` (the "not yet recorded" form).
fn provenance_from_wire(w: &MessageField<wire1::Provenance>) -> Provenance {
  w.as_option().map(Provenance::from).unwrap_or_default()
}

/// Narrow a wire-widened `uint32` back to the domain's `u16`. Overflow
/// implies a foreign/tampered frame — this encoder always writes a
/// widened domain value.
fn narrow_u16(v: u32, context: &'static str) -> Result<u16, BuffaError> {
  u16::try_from(v).map_err(|_| BuffaError::Unsupported { context })
}

/// Narrow a wire-widened `uint32` back to the domain's `u8`. See
/// [`narrow_u16`].
fn narrow_u8(v: u32, context: &'static str) -> Result<u8, BuffaError> {
  u8::try_from(v).map_err(|_| BuffaError::Unsupported { context })
}

/// Decode an epoch-millis instant into `jiff::Timestamp`.
fn ms_to_jiff(ms: i64) -> Result<JiffTimestamp, BuffaError> {
  JiffTimestamp::from_millisecond(ms).map_err(|_| BuffaError::TimestampOutOfRange(ms))
}

/// A domain `try_new` / `try_with_*` constructor (or validating VO
/// constructor) rejected the wire payload — surface its display message.
fn rejected<E: core::fmt::Display>(e: E) -> BuffaError {
  BuffaError::DomainConstructorRejected(SmolStr::from(e.to_string()))
}

/// A lift's coherence check failed. Unreachable for frames produced by
/// this module's encoders (parent ids are threaded, never transmitted),
/// but handled rather than panicked on.
fn graph_err(e: GraphError) -> BuffaError {
  BuffaError::DomainConstructorRejected(SmolStr::from(e.to_string()))
}

/// A closed string-slug vocabulary (`AudioContentKind` / `SubtitleKind`)
/// rejected the wire value.
fn unknown_slug(field: &'static str, slug: &str) -> BuffaError {
  BuffaError::DomainConstructorRejected(SmolStr::from(format!("{field}: unknown slug {slug:?}")))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn index_progress_round_trips() {
    let d = IndexProgress::try_new(5, 3, 1).unwrap();
    let w = wire::IndexProgress::from(&d);
    let d2 = IndexProgress::try_from(&w).unwrap();
    assert_eq!(d, d2);
  }

  #[test]
  fn index_progress_unset_slot_decodes_empty() {
    let d = index_progress_from_wire(&MessageField::none()).unwrap();
    assert_eq!(d, IndexProgress::new());
  }

  #[test]
  fn index_progress_invariant_violation_errors() {
    let w = wire::IndexProgress {
      total: 1,
      indexed: 2,
      failed: 2,
      __buffa_unknown_fields: Default::default(),
    };
    let err = IndexProgress::try_from(&w).unwrap_err();
    assert!(err.is_domain_constructor_rejected());
  }

  #[test]
  fn narrowing_overflow_is_unsupported() {
    assert_eq!(narrow_u16(7, "x").unwrap(), 7);
    assert_eq!(narrow_u8(7, "x").unwrap(), 7);
    assert!(matches!(
      narrow_u16(0x1_0000, "ctx").unwrap_err(),
      BuffaError::Unsupported { context: "ctx" }
    ));
    assert!(matches!(
      narrow_u8(0x100, "ctx").unwrap_err(),
      BuffaError::Unsupported { context: "ctx" }
    ));
  }

  #[test]
  fn metadata_round_trips_in_insertion_order() {
    let mut bag = IndexMap::new();
    bag.insert(SmolStr::from("zz"), SmolStr::from("1"));
    bag.insert(SmolStr::from("aa"), SmolStr::from("2"));
    let w = metadata_to_wire(&bag);
    assert_eq!(w[0].key, "zz");
    let back = metadata_from_wire(&w);
    assert_eq!(back, bag);
    assert!(back.get_index(0).is_some_and(|(k, _)| k == "zz"));
  }
}
