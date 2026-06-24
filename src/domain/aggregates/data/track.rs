//! `DataTrack` — one timed-metadata stream of a `Data` facet.
//!
//! Data tracks carry container-level timed-metadata streams (FFmpeg
//! `codec_type=data`): Sony rtmd, GoPro GPMF, MISB KLV, timecode. v1
//! records **presence + a codec/stream descriptor + container metadata
//! only** — no rtmd/GPMF/KLV sample parsing. The field vocabulary is the
//! `VideoTrack` stream descriptor minus everything visual; `mediaframe`
//! ships no `DataCodec`, so `codec` / `codec_tag` are plain [`SmolStr`]
//! slugs (`""` = unknown / absent).

use std::vec::Vec;

use derive_more::IsVariant;
use indexmap::IndexMap;
use mediaframe::disposition::TrackDisposition;
use mediatime::Timestamp;
use smol_str::SmolStr;

use crate::domain::{bitflags::DataIndexStatus, primitives::ErrorInfo, Uuid7};

/// `duration` is semantically a non-negative track-relative length. A
/// `mediatime::Timestamp` is negative iff its `pts()` is negative — the
/// timebase numerator/denominator are always positive, so the sign is
/// carried entirely by the PTS value. `None` (absent) is not negative.
/// Mirrors `VideoTrack`'s / `AudioTrack`'s `is_negative_duration` guard.
#[inline]
const fn is_negative_duration(d: Option<Timestamp>) -> bool {
  match d {
    None => false,
    Some(ts) => ts.pts() < 0,
  }
}

// ---------------------------------------------------------------------------
// DataTrack
// ---------------------------------------------------------------------------

/// One timed-metadata stream of a `Data` facet (`data_id → Data.id`).
///
/// Generic over `Id` (default [`Uuid7`]). Presence + descriptor + metadata
/// only — there is no sample payload aggregate.
///
/// **No `Default`** — defaulting to a nil id + nil data_id is an orphan
/// state. Use [`DataTrack::try_new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTrack<Id = Uuid7> {
  id: Id,
  data_id: Id,
  stream_index: Option<u32>,
  container_track_id: Option<u64>,
  /// Codec slug — `mediaframe` ships no `DataCodec`, so this is a plain
  /// [`SmolStr`] (`""` = unknown).
  codec: SmolStr,
  /// FourCC / handler tag (`""` = absent).
  codec_tag: SmolStr,
  start_pts: Option<Timestamp>,
  /// Non-negative track-time span (same convention as `VideoTrack.duration`).
  duration: Option<Timestamp>,
  nb_packets: Option<u64>,
  /// Total stream byte size (`0` = unknown).
  byte_size: u64,
  disposition: TrackDisposition,
  metadata: IndexMap<SmolStr, SmolStr>,
  index_status: DataIndexStatus,
  index_errors: Vec<ErrorInfo>,
}

impl DataTrack<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (every aggregate row needs a real identity) and nil
  /// `data_id` (orphan track with no `Data` facet). All descriptive fields
  /// start in their `""` / `None` / `0` neutral state and are filled by
  /// builders/mutators as the container probe runs.
  pub fn try_new(id: Uuid7, data_id: Uuid7) -> Result<Self, DataTrackError> {
    if id.is_nil() {
      return Err(DataTrackError::NilId);
    }
    if data_id.is_nil() {
      return Err(DataTrackError::NilDataId);
    }
    Ok(Self {
      id,
      data_id,
      stream_index: None,
      container_track_id: None,
      codec: SmolStr::default(),
      codec_tag: SmolStr::default(),
      start_pts: None,
      duration: None,
      nb_packets: None,
      byte_size: 0,
      disposition: TrackDisposition::empty(),
      metadata: IndexMap::new(),
      index_status: DataIndexStatus::empty(),
      index_errors: Vec::new(),
    })
  }
}

impl<Id> DataTrack<Id> {
  /// Canonical identity.
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// FK → `Data.id`.
  #[inline(always)]
  pub const fn data_id_ref(&self) -> &Id {
    &self.data_id
  }

  /// Source stream index (FFmpeg/container locator; not identity).
  #[inline(always)]
  pub const fn stream_index(&self) -> Option<u32> {
    self.stream_index
  }

  /// Container-specific track id (Matroska TrackNumber etc.).
  #[inline(always)]
  pub const fn container_track_id(&self) -> Option<u64> {
    self.container_track_id
  }

  /// Codec slug (`""` = unknown).
  #[inline(always)]
  pub fn codec(&self) -> &str {
    self.codec.as_str()
  }

  /// FourCC / handler tag (`""` = absent).
  #[inline(always)]
  pub fn codec_tag(&self) -> &str {
    self.codec_tag.as_str()
  }

  /// First-PTS offset (`None` = absent).
  #[inline(always)]
  pub const fn start_pts_ref(&self) -> Option<&Timestamp> {
    self.start_pts.as_ref()
  }

  /// Per-track duration (non-negative track-relative length).
  #[inline(always)]
  pub const fn duration_ref(&self) -> Option<&Timestamp> {
    self.duration.as_ref()
  }

  /// Packet count (`None` = unknown).
  #[inline(always)]
  pub const fn nb_packets(&self) -> Option<u64> {
    self.nb_packets
  }

  /// Total stream byte size (`0` = unknown).
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
  pub const fn index_status(&self) -> DataIndexStatus {
    self.index_status
  }

  /// Per-track index errors (stage-coded `ErrorInfo.code`).
  #[inline(always)]
  pub fn index_errors_slice(&self) -> &[ErrorInfo] {
    self.index_errors.as_slice()
  }

  // ----- Builders ----------------------------------------------------------

  /// Builder: replace `stream_index`.
  #[inline(always)]
  #[must_use]
  pub const fn with_stream_index(mut self, v: Option<u32>) -> Self {
    self.stream_index = v;
    self
  }

  /// Builder: replace `container_track_id`.
  #[inline(always)]
  #[must_use]
  pub const fn with_container_track_id(mut self, v: Option<u64>) -> Self {
    self.container_track_id = v;
    self
  }

  /// Builder: replace `codec`.
  #[inline(always)]
  #[must_use]
  pub fn with_codec(mut self, v: impl Into<SmolStr>) -> Self {
    self.codec = v.into();
    self
  }

  /// Builder: replace `codec_tag`.
  #[inline(always)]
  #[must_use]
  pub fn with_codec_tag(mut self, v: impl Into<SmolStr>) -> Self {
    self.codec_tag = v.into();
    self
  }

  /// Builder: replace `start_pts`.
  #[inline(always)]
  #[must_use]
  pub fn with_start_pts(mut self, v: Option<Timestamp>) -> Self {
    self.start_pts = v;
    self
  }

  /// Validating builder: replace `duration`.
  ///
  /// `duration` is semantically a non-negative track-relative length;
  /// although `mediatime::Timestamp` admits a negative PTS, a `Some(_)`
  /// with `pts() < 0` is rejected with [`DataTrackError::NegativeDuration`].
  /// `None` (absent) and a zero or positive `Timestamp` are accepted. On
  /// rejection `self` is returned unchanged inside the `Err`. (`start_pts`
  /// is left infallible — a negative first-PTS offset is legitimate.)
  #[inline]
  pub fn try_with_duration(mut self, v: Option<Timestamp>) -> Result<Self, DataTrackError> {
    if is_negative_duration(v) {
      return Err(DataTrackError::NegativeDuration);
    }
    self.duration = v;
    Ok(self)
  }

  /// Builder: replace `nb_packets`.
  #[inline(always)]
  #[must_use]
  pub const fn with_nb_packets(mut self, v: Option<u64>) -> Self {
    self.nb_packets = v;
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
  pub const fn with_index_status(mut self, v: DataIndexStatus) -> Self {
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

  // ----- Setters -----------------------------------------------------------

  /// In-place mutator for `stream_index`.
  #[inline(always)]
  pub const fn set_stream_index(&mut self, v: Option<u32>) -> &mut Self {
    self.stream_index = v;
    self
  }

  /// In-place mutator for `container_track_id`.
  #[inline(always)]
  pub const fn set_container_track_id(&mut self, v: Option<u64>) -> &mut Self {
    self.container_track_id = v;
    self
  }

  /// In-place mutator for `codec`.
  #[inline(always)]
  pub fn set_codec(&mut self, v: impl Into<SmolStr>) -> &mut Self {
    self.codec = v.into();
    self
  }

  /// In-place mutator for `codec_tag`.
  #[inline(always)]
  pub fn set_codec_tag(&mut self, v: impl Into<SmolStr>) -> &mut Self {
    self.codec_tag = v.into();
    self
  }

  /// In-place mutator for `start_pts`.
  #[inline(always)]
  pub fn set_start_pts(&mut self, v: Option<Timestamp>) -> &mut Self {
    self.start_pts = v;
    self
  }

  /// Validating in-place mutator for `duration`. Rejects a `Some(_)`
  /// carrying a negative `Timestamp` ([`DataTrackError::NegativeDuration`]);
  /// `None` and a zero or positive `Timestamp` are accepted. On rejection
  /// `self` is left unchanged.
  #[inline]
  pub const fn try_set_duration(
    &mut self,
    v: Option<Timestamp>,
  ) -> Result<&mut Self, DataTrackError> {
    if is_negative_duration(v) {
      return Err(DataTrackError::NegativeDuration);
    }
    self.duration = v;
    Ok(self)
  }

  /// In-place mutator for `nb_packets`.
  #[inline(always)]
  pub const fn set_nb_packets(&mut self, v: Option<u64>) -> &mut Self {
    self.nb_packets = v;
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
  pub const fn set_index_status(&mut self, v: DataIndexStatus) -> &mut Self {
    self.index_status = v;
    self
  }

  /// In-place mutator for `index_errors`.
  #[inline(always)]
  pub fn set_index_errors(&mut self, v: impl Into<Vec<ErrorInfo>>) -> &mut Self {
    self.index_errors = v.into();
    self
  }
}

/// Error returned by [`DataTrack::try_new`] and its duration mutators.
/// Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum DataTrackError {
  /// Supplied `id` was the nil sentinel.
  #[error("DataTrack id must not be the nil UUID")]
  NilId,
  /// Supplied `data_id` was the nil sentinel — orphaned track with no
  /// `Data` facet reference.
  #[error("DataTrack `data_id` (FK → Data) must not be the nil UUID")]
  NilDataId,
  /// A `Some(_)` `duration` carried a negative `Timestamp` — a track
  /// duration is semantically a non-negative length.
  #[error("DataTrack duration must not be negative")]
  NegativeDuration,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use crate::domain::{ErrorCode, ErrorInfo};
  use core::num::NonZeroU32;
  use mediatime::Timebase;

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).unwrap())
  }

  #[test]
  fn try_new_happy_path() {
    let data_id = Uuid7::new();
    let t = DataTrack::try_new(Uuid7::new(), data_id).expect("valid construction must succeed");
    assert_eq!(t.data_id_ref(), &data_id);
    assert!(t.codec().is_empty());
    assert!(t.codec_tag().is_empty());
    assert_eq!(t.byte_size(), 0);
    assert!(t.nb_packets().is_none());
    assert_eq!(t.index_status(), DataIndexStatus::empty());
    assert!(t.metadata_ref().is_empty());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = DataTrack::try_new(Uuid7::nil(), Uuid7::new());
    assert_eq!(r.err(), Some(DataTrackError::NilId));
    assert!(DataTrackError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_data_id() {
    let r = DataTrack::try_new(Uuid7::new(), Uuid7::nil());
    assert_eq!(r.err(), Some(DataTrackError::NilDataId));
    assert!(DataTrackError::NilDataId.is_nil_data_id());
  }

  #[test]
  fn builders_chain() {
    let t = DataTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_stream_index(Some(3))
      .with_container_track_id(Some(7))
      .with_codec("rtmd")
      .with_codec_tag("rtmd")
      .with_start_pts(Some(Timestamp::new(0, tb())))
      .try_with_duration(Some(Timestamp::new(90_000, tb())))
      .unwrap()
      .with_nb_packets(Some(2_700))
      .with_byte_size(1_024)
      .with_disposition(TrackDisposition::empty())
      .with_index_status(DataIndexStatus::PROBED)
      .with_index_errors(std::vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "x")]);
    assert_eq!(t.stream_index(), Some(3));
    assert_eq!(t.container_track_id(), Some(7));
    assert_eq!(t.codec(), "rtmd");
    assert_eq!(t.codec_tag(), "rtmd");
    assert_eq!(t.nb_packets(), Some(2_700));
    assert_eq!(t.byte_size(), 1_024);
    assert!(t.index_status().contains(DataIndexStatus::PROBED));
    assert_eq!(t.index_errors_slice().len(), 1);
  }

  #[test]
  fn try_with_duration_rejects_negative() {
    let r = DataTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .try_with_duration(Some(Timestamp::new(-1, tb())));
    assert_eq!(r.err(), Some(DataTrackError::NegativeDuration));
    assert!(DataTrackError::NegativeDuration.is_negative_duration());
  }

  #[test]
  fn setters_mutate_in_place() {
    let mut t = DataTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    t.set_codec("gpmf");
    t.set_byte_size(42);
    t.set_index_status(DataIndexStatus::PROBED);
    assert_eq!(t.codec(), "gpmf");
    assert_eq!(t.byte_size(), 42);
    assert!(t.index_status().contains(DataIndexStatus::PROBED));
  }

  #[test]
  fn into_parts_and_rehydrate_round_trip() {
    let t = DataTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec("klv")
      .with_byte_size(99)
      .with_index_status(DataIndexStatus::PROBED);
    let parts = t.clone().into_parts();
    let t2 = DataTrack::rehydrate(parts);
    assert_eq!(t, t2);
  }
}

/// Exhaustive by-value decomposition of [`DataTrack`] — every stored field.
///
/// Public-field data-transfer struct (the conversion-boundary exception to
/// the encapsulation rule): cross-suite conversions (`crate::graph`)
/// destructure it exhaustively, so adding a field breaks them at compile
/// time instead of silently dropping data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTrackParts<Id = Uuid7> {
  pub id: Id,
  pub data_id: Id,
  pub stream_index: Option<u32>,
  pub container_track_id: Option<u64>,
  pub codec: SmolStr,
  pub codec_tag: SmolStr,
  pub start_pts: Option<Timestamp>,
  pub duration: Option<Timestamp>,
  pub nb_packets: Option<u64>,
  pub byte_size: u64,
  pub disposition: TrackDisposition,
  pub metadata: IndexMap<SmolStr, SmolStr>,
  pub index_status: DataIndexStatus,
  pub index_errors: Vec<ErrorInfo>,
}

impl<Id> DataTrack<Id> {
  /// Decompose into [`DataTrackParts`] — exhaustive, by value.
  #[inline(always)]
  pub fn into_parts(self) -> DataTrackParts<Id> {
    let Self {
      id,
      data_id,
      stream_index,
      container_track_id,
      codec,
      codec_tag,
      start_pts,
      duration,
      nb_packets,
      byte_size,
      disposition,
      metadata,
      index_status,
      index_errors,
    } = self;
    DataTrackParts {
      id,
      data_id,
      stream_index,
      container_track_id,
      codec,
      codec_tag,
      start_pts,
      duration,
      nb_packets,
      byte_size,
      disposition,
      metadata,
      index_status,
      index_errors,
    }
  }
}

impl<Id> DataTrack<Id> {
  /// Invariant-carrying constructor from [`DataTrackParts`] — `pub(crate)`,
  /// reserved for in-crate conversions from already-validated values
  /// (`crate::graph`).
  #[cfg(all(
    feature = "std",
    feature = "video",
    feature = "audio",
    feature = "subtitle"
  ))]
  #[inline(always)]
  pub(crate) fn rehydrate(parts: DataTrackParts<Id>) -> Self {
    let DataTrackParts {
      id,
      data_id,
      stream_index,
      container_track_id,
      codec,
      codec_tag,
      start_pts,
      duration,
      nb_packets,
      byte_size,
      disposition,
      metadata,
      index_status,
      index_errors,
    } = parts;
    Self {
      id,
      data_id,
      stream_index,
      container_track_id,
      codec,
      codec_tag,
      start_pts,
      duration,
      nb_packets,
      byte_size,
      disposition,
      metadata,
      index_status,
      index_errors,
    }
  }
}
