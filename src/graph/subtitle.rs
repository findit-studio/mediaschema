//! Subtitle subtree: facet → tracks → cues. Standalone field owners — no
//! embedded flat aggregates, no parent FKs, no id-vecs.
//!
//! Cue payloads use the type-erased
//! [`SubtitleCueDetails`](crate::domain::aggregates::subtitle::SubtitleCueDetails)
//! union — the storage-shaped full form. Consumers wanting the typed
//! per-format cue payloads work with the flat aggregates.

use indexmap::IndexMap;
use mediaframe::{
  codec::SubtitleCodec,
  disposition::TrackDisposition,
  lang::Language,
  subtitle::{Format, TrackOrigin},
};
use mediatime::{TimeRange, Timestamp};
use smol_str::SmolStr;

use super::{parent_check, GraphError, NodeKind};
use crate::domain::{
  self, aggregates::subtitle::SubtitleCueDetails, ErrorInfo, FileChecksum, IndexProgress,
  LocalizedText, Provenance, SubtitleIndexStatus, SubtitleKind, Uuid7,
};

/// The subtitle facet with its complete track subtrees.
#[derive(Debug, Clone, PartialEq)]
pub struct Subtitle<Id = Uuid7> {
  id: Id,
  track_progress: IndexProgress,
  tracks: Vec<SubtitleTrack<Id>>,
}

impl Subtitle<Uuid7> {
  /// Lift the flat facet; validates `media_id == expected_media`. Tracks
  /// arrive pre-lifted (their `subtitle_id` was consumed by their lift).
  pub fn try_from_flat(
    expected_media: &Uuid7,
    facet: domain::Subtitle<Uuid7>,
    tracks: Vec<SubtitleTrack<Uuid7>>,
  ) -> Result<Self, GraphError> {
    parent_check(
      NodeKind::SubtitleFacet,
      *facet.id_ref(),
      facet.media_id_ref(),
      expected_media,
    )?;
    Ok(Self {
      id: *facet.id_ref(),
      track_progress: *facet.track_progress_ref(),
      tracks,
    })
  }
}

impl<Id> Subtitle<Id> {
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
  pub const fn tracks_slice(&self) -> &[SubtitleTrack<Id>] {
    self.tracks.as_slice()
  }
}

/// One subtitle track — every field of the flat `SubtitleTrack` except
/// `subtitle_id` and the cue-id vec, plus the cues themselves.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleTrack<Id = Uuid7> {
  id: Id,
  stream_index: Option<u32>,
  container_track_id: Option<u64>,
  codec: SubtitleCodec,
  format: Format,
  origin: TrackOrigin,
  language: Language,
  title: SmolStr,
  disposition: TrackDisposition,
  is_primary: bool,
  auto_selected: bool,
  duration: Option<Timestamp>,
  cue_count: u32,
  cues: Vec<SubtitleCue<Id>>,
  provenance: Provenance,
  source_checksum: Option<FileChecksum>,
  character_encoding: SmolStr,
  bom_present: bool,
  is_sdh: bool,
  is_closed_caption: bool,
  is_translation: bool,
  kind: SubtitleKind,
  coverage_ratio: Option<f32>,
  is_empty: bool,
  first_cue: Option<Timestamp>,
  last_cue: Option<Timestamp>,
  metadata: IndexMap<SmolStr, SmolStr>,
  index_status: SubtitleIndexStatus,
  index_errors: Vec<ErrorInfo>,
}

impl SubtitleTrack<Uuid7> {
  /// Lift the flat track; validates `subtitle_id == expected_subtitle`
  /// and lifts the flat cues against this track's id.
  pub fn try_from_flat(
    expected_subtitle: &Uuid7,
    track: domain::SubtitleTrack<Uuid7>,
    cues: Vec<domain::SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>>>,
  ) -> Result<Self, GraphError> {
    parent_check(
      NodeKind::SubtitleTrack,
      *track.id_ref(),
      track.subtitle_id_ref(),
      expected_subtitle,
    )?;
    let id = *track.id_ref();
    let cues = cues
      .into_iter()
      .map(|c| SubtitleCue::try_from_flat(&id, c))
      .collect::<Result<Vec<_>, _>>()?;
    Ok(Self {
      id,
      stream_index: track.stream_index(),
      container_track_id: track.container_track_id(),
      codec: track.codec_ref().clone(),
      format: track.format_ref().clone(),
      origin: *track.origin_ref(),
      language: *track.language_ref(),
      title: SmolStr::new(track.title()),
      disposition: track.disposition(),
      is_primary: track.is_primary(),
      auto_selected: track.auto_selected(),
      duration: track.duration_ref().cloned(),
      cue_count: track.cue_count(),
      cues,
      provenance: track.provenance_ref().clone(),
      source_checksum: track.source_checksum_ref().cloned(),
      character_encoding: SmolStr::new(track.character_encoding()),
      bom_present: track.bom_present(),
      is_sdh: track.is_sdh(),
      is_closed_caption: track.is_closed_caption(),
      is_translation: track.is_translation(),
      kind: track.kind(),
      coverage_ratio: track.coverage_ratio(),
      is_empty: track.is_empty(),
      first_cue: track.first_cue_ref().cloned(),
      last_cue: track.last_cue_ref().cloned(),
      metadata: track.metadata_ref().clone(),
      index_status: track.index_status(),
      index_errors: track.index_errors_slice().to_vec(),
    })
  }
}

impl<Id> SubtitleTrack<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  #[inline(always)]
  pub const fn stream_index(&self) -> Option<u32> {
    self.stream_index
  }

  #[inline(always)]
  pub const fn container_track_id(&self) -> Option<u64> {
    self.container_track_id
  }

  #[inline(always)]
  pub const fn codec_ref(&self) -> &SubtitleCodec {
    &self.codec
  }

  #[inline(always)]
  pub const fn format_ref(&self) -> &Format {
    &self.format
  }

  #[inline(always)]
  pub const fn origin_ref(&self) -> &TrackOrigin {
    &self.origin
  }

  #[inline(always)]
  pub const fn language_ref(&self) -> &Language {
    &self.language
  }

  /// Track title (`""` = absent).
  #[inline(always)]
  pub fn title(&self) -> &str {
    self.title.as_str()
  }

  #[inline(always)]
  pub const fn disposition(&self) -> TrackDisposition {
    self.disposition
  }

  #[inline(always)]
  pub const fn is_primary(&self) -> bool {
    self.is_primary
  }

  #[inline(always)]
  pub const fn auto_selected(&self) -> bool {
    self.auto_selected
  }

  #[inline(always)]
  pub const fn duration_ref(&self) -> Option<&Timestamp> {
    self.duration.as_ref()
  }

  #[inline(always)]
  pub const fn cue_count(&self) -> u32 {
    self.cue_count
  }

  /// The track's cues, in cue order.
  #[inline(always)]
  pub const fn cues_slice(&self) -> &[SubtitleCue<Id>] {
    self.cues.as_slice()
  }

  #[inline(always)]
  pub const fn provenance_ref(&self) -> &Provenance {
    &self.provenance
  }

  #[inline(always)]
  pub const fn source_checksum_ref(&self) -> Option<&FileChecksum> {
    self.source_checksum.as_ref()
  }

  /// Character encoding label (`""` = unknown).
  #[inline(always)]
  pub fn character_encoding(&self) -> &str {
    self.character_encoding.as_str()
  }

  #[inline(always)]
  pub const fn bom_present(&self) -> bool {
    self.bom_present
  }

  #[inline(always)]
  pub const fn is_sdh(&self) -> bool {
    self.is_sdh
  }

  #[inline(always)]
  pub const fn is_closed_caption(&self) -> bool {
    self.is_closed_caption
  }

  #[inline(always)]
  pub const fn is_translation(&self) -> bool {
    self.is_translation
  }

  #[inline(always)]
  pub const fn kind(&self) -> SubtitleKind {
    self.kind
  }

  #[inline(always)]
  pub const fn coverage_ratio(&self) -> Option<f32> {
    self.coverage_ratio
  }

  #[inline(always)]
  pub const fn is_empty(&self) -> bool {
    self.is_empty
  }

  #[inline(always)]
  pub const fn first_cue_ref(&self) -> Option<&Timestamp> {
    self.first_cue.as_ref()
  }

  #[inline(always)]
  pub const fn last_cue_ref(&self) -> Option<&Timestamp> {
    self.last_cue.as_ref()
  }

  #[inline(always)]
  pub const fn metadata_ref(&self) -> &IndexMap<SmolStr, SmolStr> {
    &self.metadata
  }

  #[inline(always)]
  pub const fn index_status(&self) -> SubtitleIndexStatus {
    self.index_status
  }

  #[inline(always)]
  pub const fn index_errors_slice(&self) -> &[ErrorInfo] {
    self.index_errors.as_slice()
  }
}

/// One cue — every field of the flat `SubtitleCue` except
/// `subtitle_track_id` (implied by nesting), with the type-erased
/// payload form.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleCue<Id = Uuid7> {
  id: Id,
  ordinal: u32,
  span: TimeRange,
  text: LocalizedText,
  data: SubtitleCueDetails<Id>,
}

impl SubtitleCue<Uuid7> {
  /// Lift the flat cue; validates `subtitle_track_id == expected_track`.
  pub fn try_from_flat(
    expected_track: &Uuid7,
    cue: domain::SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>>,
  ) -> Result<Self, GraphError> {
    parent_check(
      NodeKind::SubtitleCue,
      *cue.id_ref(),
      cue.subtitle_track_id_ref(),
      expected_track,
    )?;
    Ok(Self {
      id: *cue.id_ref(),
      ordinal: cue.ordinal(),
      span: *cue.span_ref(),
      text: cue.text_ref().clone(),
      data: cue.data_ref().clone(),
    })
  }
}

impl<Id> SubtitleCue<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  #[inline(always)]
  pub const fn ordinal(&self) -> u32 {
    self.ordinal
  }

  #[inline(always)]
  pub const fn span_ref(&self) -> &TimeRange {
    &self.span
  }

  #[inline(always)]
  pub const fn text_ref(&self) -> &LocalizedText {
    &self.text
  }

  /// Type-erased per-format payload.
  #[inline(always)]
  pub const fn data_ref(&self) -> &SubtitleCueDetails<Id> {
    &self.data
  }
}

#[cfg(test)]
mod tests {
  use core::num::NonZeroU32;

  use mediatime::{TimeRange, Timebase};

  use super::*;
  use crate::domain::aggregates::SrtData;

  fn span() -> TimeRange {
    TimeRange::new(0, 1000, Timebase::new(1, NonZeroU32::new(1000).unwrap()))
  }

  fn flat_cue(track_id: Uuid7) -> domain::SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>> {
    domain::SubtitleCue::try_new(
      Uuid7::new(),
      track_id,
      0,
      span(),
      LocalizedText::new(),
      SubtitleCueDetails::Srt(SrtData::new()),
    )
    .expect("valid cue")
  }

  #[test]
  fn coherent_subtitle_subtree_lifts() {
    let subtitle_id = Uuid7::new();
    let track = domain::SubtitleTrack::try_new(Uuid7::new(), subtitle_id).expect("valid track");
    let track_id = *track.id_ref();
    let node = SubtitleTrack::try_from_flat(&subtitle_id, track, vec![flat_cue(track_id)])
      .expect("coherent");
    assert_eq!(node.cues_slice().len(), 1);
    assert_eq!(node.cues_slice()[0].ordinal(), 0);
  }

  #[test]
  fn cue_under_wrong_track_is_rejected() {
    let subtitle_id = Uuid7::new();
    let track = domain::SubtitleTrack::try_new(Uuid7::new(), subtitle_id).expect("valid track");
    let err = SubtitleTrack::try_from_flat(&subtitle_id, track, vec![flat_cue(Uuid7::new())])
      .expect_err("incoherent");
    assert!(matches!(
      err,
      GraphError::ParentMismatch {
        kind: NodeKind::SubtitleCue,
        ..
      }
    ));
  }
}
