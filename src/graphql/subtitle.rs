//! GraphQL exposure of the Subtitle aggregates.

use async_graphql::{Object, ID};

use crate::domain::{FileChecksum, Subtitle, SubtitleCue, SubtitleTrack, Uuid7};

use super::{
  bitflags::{disposition_flag_names, GqlSubtitleIndexStatus},
  enums::GqlSubtitleKind,
  media::{GqlErrorInfo, GqlLocalizedText, GqlLocation, GqlProvenance},
  scalars::{empty_as_none, GqlMediaTimeRange, GqlMediaTimestamp},
};

// ---------------------------------------------------------------------------
// Subtitle.IndexProgress
// ---------------------------------------------------------------------------

/// Newtype wrapper for the subtitle-side `IndexProgress` (a distinct
/// type from the video facet's `IndexProgress`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GqlSubtitleIndexProgress(pub crate::domain::aggregates::subtitle::facet::IndexProgress);

impl From<crate::domain::aggregates::subtitle::facet::IndexProgress> for GqlSubtitleIndexProgress {
  #[inline]
  fn from(v: crate::domain::aggregates::subtitle::facet::IndexProgress) -> Self {
    Self(v)
  }
}
impl From<GqlSubtitleIndexProgress> for crate::domain::aggregates::subtitle::facet::IndexProgress {
  #[inline]
  fn from(v: GqlSubtitleIndexProgress) -> Self {
    v.0
  }
}

#[Object(name = "SubtitleIndexProgress")]
impl GqlSubtitleIndexProgress {
  async fn total(&self) -> u32 {
    self.0.total()
  }
  async fn indexed(&self) -> u32 {
    self.0.indexed()
  }
  async fn failed(&self) -> u32 {
    self.0.failed()
  }
}

// ---------------------------------------------------------------------------
// Subtitle facet
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`Subtitle`].
#[derive(Debug, Clone)]
pub struct GqlSubtitle(pub Subtitle<Uuid7>);

impl From<Subtitle<Uuid7>> for GqlSubtitle {
  #[inline]
  fn from(v: Subtitle<Uuid7>) -> Self {
    Self(v)
  }
}
impl From<GqlSubtitle> for Subtitle<Uuid7> {
  #[inline]
  fn from(v: GqlSubtitle) -> Self {
    v.0
  }
}

#[Object(name = "Subtitle")]
impl GqlSubtitle {
  async fn id(&self) -> ID {
    ID(self.0.id().to_string())
  }
  async fn parent(&self) -> ID {
    ID(self.0.parent().to_string())
  }
  async fn tracks(&self) -> std::vec::Vec<ID> {
    self
      .0
      .tracks()
      .iter()
      .map(|id| ID(id.to_string()))
      .collect()
  }
  async fn track_progress(&self) -> GqlSubtitleIndexProgress {
    GqlSubtitleIndexProgress(*self.0.track_progress())
  }
}

// ---------------------------------------------------------------------------
// SubtitleTrack
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`SubtitleTrack`].
#[derive(Debug, Clone)]
pub struct GqlSubtitleTrack(pub SubtitleTrack<Uuid7>);

impl From<SubtitleTrack<Uuid7>> for GqlSubtitleTrack {
  #[inline]
  fn from(v: SubtitleTrack<Uuid7>) -> Self {
    Self(v)
  }
}
impl From<GqlSubtitleTrack> for SubtitleTrack<Uuid7> {
  #[inline]
  fn from(v: GqlSubtitleTrack) -> Self {
    v.0
  }
}

#[Object(name = "SubtitleTrack")]
impl GqlSubtitleTrack {
  async fn id(&self) -> ID {
    ID(self.0.id().to_string())
  }
  async fn parent(&self) -> ID {
    ID(self.0.parent().to_string())
  }
  async fn stream_index(&self) -> Option<u32> {
    self.0.stream_index()
  }
  async fn container_track_id(&self) -> Option<String> {
    self.0.container_track_id().map(|v| v.to_string())
  }
  /// Subtitle codec short name (`as_str()`); `null` when the
  /// `Other("")` absent sentinel.
  async fn codec(&self) -> Option<String> {
    empty_as_none(self.0.codec().as_str())
  }
  /// Container form short name (`as_str()`, text vs bitmap); `null`
  /// when the `Other("")` absent sentinel.
  async fn format(&self) -> Option<String> {
    empty_as_none(self.0.format().as_str())
  }
  /// Track origin tag (`as_str()`, embedded / sidecar / external).
  async fn origin(&self) -> String {
    self.0.origin().as_str().to_string()
  }
  /// Language as a BCP-47 tag; `null` when undetermined (`und`).
  async fn language(&self) -> Option<String> {
    let lang = self.0.language();
    if lang.is_undetermined() {
      None
    } else {
      Some(lang.to_bcp47())
    }
  }
  async fn title(&self) -> Option<String> {
    empty_as_none(self.0.title())
  }
  async fn is_image_based(&self) -> bool {
    self.0.is_image_based()
  }
  /// Disposition flag word (`AV_DISPOSITION_*` bits via `to_u32()`).
  async fn disposition(&self) -> u32 {
    self.0.disposition().to_u32()
  }
  /// Named disposition flags currently set.
  async fn disposition_flags(&self) -> std::vec::Vec<String> {
    disposition_flag_names(self.0.disposition())
  }
  async fn is_primary(&self) -> bool {
    self.0.is_primary()
  }
  async fn auto_selected(&self) -> bool {
    self.0.auto_selected()
  }
  async fn duration(&self) -> Option<GqlMediaTimestamp> {
    self.0.duration().copied().map(GqlMediaTimestamp)
  }
  async fn cue_count(&self) -> u32 {
    self.0.cue_count()
  }
  async fn cues(&self) -> std::vec::Vec<ID> {
    self.0.cues().iter().map(|id| ID(id.to_string())).collect()
  }
  async fn provenance(&self) -> GqlProvenance {
    GqlProvenance(self.0.provenance().clone())
  }
  async fn source_path(&self) -> Option<GqlLocation> {
    self.0.source_path().cloned().map(Into::into)
  }
  async fn source_checksum(&self) -> Option<FileChecksum> {
    self.0.source_checksum().copied()
  }
  async fn character_encoding(&self) -> Option<String> {
    empty_as_none(self.0.character_encoding())
  }
  async fn bom_present(&self) -> bool {
    self.0.bom_present()
  }
  async fn is_sdh(&self) -> bool {
    self.0.is_sdh()
  }
  async fn is_closed_caption(&self) -> bool {
    self.0.is_closed_caption()
  }
  async fn is_translation(&self) -> bool {
    self.0.is_translation()
  }
  async fn kind(&self) -> GqlSubtitleKind {
    self.0.kind().into()
  }
  async fn coverage_ratio(&self) -> Option<f32> {
    self.0.coverage_ratio()
  }
  async fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
  async fn first_cue(&self) -> Option<GqlMediaTimestamp> {
    self.0.first_cue().copied().map(GqlMediaTimestamp)
  }
  async fn last_cue(&self) -> Option<GqlMediaTimestamp> {
    self.0.last_cue().copied().map(GqlMediaTimestamp)
  }
  async fn index_status(&self) -> GqlSubtitleIndexStatus {
    self.0.index_status().into()
  }
  async fn index_errors(&self) -> std::vec::Vec<GqlErrorInfo> {
    self
      .0
      .index_errors()
      .iter()
      .cloned()
      .map(GqlErrorInfo)
      .collect()
  }
}

// ---------------------------------------------------------------------------
// SubtitleCue
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`SubtitleCue`].
#[derive(Debug, Clone)]
pub struct GqlSubtitleCue(pub SubtitleCue<Uuid7>);

impl From<SubtitleCue<Uuid7>> for GqlSubtitleCue {
  #[inline]
  fn from(v: SubtitleCue<Uuid7>) -> Self {
    Self(v)
  }
}
impl From<GqlSubtitleCue> for SubtitleCue<Uuid7> {
  #[inline]
  fn from(v: GqlSubtitleCue) -> Self {
    v.0
  }
}

#[Object(name = "SubtitleCue")]
impl GqlSubtitleCue {
  async fn id(&self) -> ID {
    ID(self.0.id().to_string())
  }
  async fn parent(&self) -> ID {
    ID(self.0.parent().to_string())
  }
  async fn index(&self) -> u32 {
    self.0.index()
  }
  async fn span(&self) -> GqlMediaTimeRange {
    GqlMediaTimeRange(*self.0.span())
  }
  async fn text(&self) -> GqlLocalizedText {
    GqlLocalizedText(self.0.text().clone())
  }
  async fn styled_text(&self) -> Option<String> {
    empty_as_none(self.0.styled_text())
  }
  async fn image_byte_len(&self) -> usize {
    self.0.image().len()
  }
  async fn ocr_text(&self) -> GqlLocalizedText {
    GqlLocalizedText(self.0.ocr_text().clone())
  }
  async fn is_blank(&self) -> bool {
    self.0.is_blank()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use core::num::NonZeroU32;
  use mediatime::Timebase;

  #[test]
  fn subtitle_facet_roundtrips() {
    let id = Uuid7::new();
    let parent = Uuid7::new();
    let s = Subtitle::try_new(id, parent).unwrap();
    let g: GqlSubtitle = s.clone().into();
    let back: Subtitle<Uuid7> = g.into();
    assert_eq!(back.tracks().len(), s.tracks().len());
  }

  #[test]
  fn subtitle_cue_roundtrips() {
    let id = Uuid7::new();
    let parent = Uuid7::new();
    let tb = Timebase::new(1, NonZeroU32::new(1000).unwrap());
    let span = mediatime::TimeRange::new(0, 1000, tb);
    let c = SubtitleCue::try_new(id, parent, 0, span).unwrap();
    let g: GqlSubtitleCue = c.clone().into();
    let back: SubtitleCue<Uuid7> = g.into();
    assert_eq!(back.index(), c.index());
  }
}
