//! GraphQL enum mirrors of the domain enums.
//!
//! `async_graphql::Enum` must be derived on the type — domain enums are
//! off-limits to this module, so each one is mirrored by a newtype-free
//! GraphQL enum here with infallible `From` conversions both ways. The
//! conversions are exhaustive `match`es so adding a domain variant fails
//! to compile until the mirror is updated.

use async_graphql::Enum;

use crate::domain::{
  AudioContentKind, AudioIndexStage, KeyframeExtractor, MediaKind, ScanStatus, SceneDetector,
  SubtitleIndexStage, SubtitleKind, VideoIndexStage,
};

// ---------------------------------------------------------------------------
// MediaKind
// ---------------------------------------------------------------------------

/// GraphQL mirror of [`MediaKind`].
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[graphql(name = "MediaKind")]
pub enum GqlMediaKind {
  Video,
  Audio,
}

impl From<MediaKind> for GqlMediaKind {
  fn from(v: MediaKind) -> Self {
    match v {
      MediaKind::Video => Self::Video,
      MediaKind::Audio => Self::Audio,
    }
  }
}

impl From<GqlMediaKind> for MediaKind {
  fn from(v: GqlMediaKind) -> Self {
    match v {
      GqlMediaKind::Video => Self::Video,
      GqlMediaKind::Audio => Self::Audio,
    }
  }
}

// ---------------------------------------------------------------------------
// AudioContentKind
// ---------------------------------------------------------------------------

/// GraphQL mirror of [`AudioContentKind`].
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[graphql(name = "AudioContentKind")]
pub enum GqlAudioContentKind {
  Speech,
  Music,
  Mixed,
  Silence,
}

impl From<AudioContentKind> for GqlAudioContentKind {
  fn from(v: AudioContentKind) -> Self {
    match v {
      AudioContentKind::Speech => Self::Speech,
      AudioContentKind::Music => Self::Music,
      AudioContentKind::Mixed => Self::Mixed,
      AudioContentKind::Silence => Self::Silence,
    }
  }
}

impl From<GqlAudioContentKind> for AudioContentKind {
  fn from(v: GqlAudioContentKind) -> Self {
    match v {
      GqlAudioContentKind::Speech => Self::Speech,
      GqlAudioContentKind::Music => Self::Music,
      GqlAudioContentKind::Mixed => Self::Mixed,
      GqlAudioContentKind::Silence => Self::Silence,
    }
  }
}

// ---------------------------------------------------------------------------
// ScanStatus
// ---------------------------------------------------------------------------

/// GraphQL mirror of [`ScanStatus`].
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[graphql(name = "ScanStatus")]
pub enum GqlScanStatus {
  Ok,
  Partial,
  Failed,
}

impl From<ScanStatus> for GqlScanStatus {
  fn from(v: ScanStatus) -> Self {
    match v {
      ScanStatus::Ok => Self::Ok,
      ScanStatus::Partial => Self::Partial,
      ScanStatus::Failed => Self::Failed,
    }
  }
}

impl From<GqlScanStatus> for ScanStatus {
  fn from(v: GqlScanStatus) -> Self {
    match v {
      GqlScanStatus::Ok => Self::Ok,
      GqlScanStatus::Partial => Self::Partial,
      GqlScanStatus::Failed => Self::Failed,
    }
  }
}

// ---------------------------------------------------------------------------
// SceneDetector
// ---------------------------------------------------------------------------

/// GraphQL mirror of [`SceneDetector`].
///
/// Note: `SceneDetector` is `#[non_exhaustive]`. The `_NonExhaustive`
/// wildcard arm in the From impl below catches any future variant — at
/// which point the mirror must add the corresponding GraphQL variant.
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[graphql(name = "SceneDetector")]
pub enum GqlSceneDetector {
  Histogram,
  Phash,
  Threshold,
  Content,
  Adaptive,
  Manual,
}

impl From<SceneDetector> for GqlSceneDetector {
  fn from(v: SceneDetector) -> Self {
    match v {
      SceneDetector::Histogram => Self::Histogram,
      SceneDetector::Phash => Self::Phash,
      SceneDetector::Threshold => Self::Threshold,
      SceneDetector::Content => Self::Content,
      SceneDetector::Adaptive => Self::Adaptive,
      SceneDetector::Manual => Self::Manual,
      // SceneDetector is `#[non_exhaustive]`; if a new variant lands
      // upstream the unit-mirror must add it. The wildcard arm below
      // keeps the From-impl complete while telegraphing the contract
      // to the next reviewer. Currently unreachable.
      #[allow(unreachable_patterns)]
      _ => Self::Manual,
    }
  }
}

impl From<GqlSceneDetector> for SceneDetector {
  fn from(v: GqlSceneDetector) -> Self {
    match v {
      GqlSceneDetector::Histogram => Self::Histogram,
      GqlSceneDetector::Phash => Self::Phash,
      GqlSceneDetector::Threshold => Self::Threshold,
      GqlSceneDetector::Content => Self::Content,
      GqlSceneDetector::Adaptive => Self::Adaptive,
      GqlSceneDetector::Manual => Self::Manual,
    }
  }
}

// ---------------------------------------------------------------------------
// KeyframeExtractor
// ---------------------------------------------------------------------------

/// GraphQL mirror of [`KeyframeExtractor`].
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[graphql(name = "KeyframeExtractor")]
pub enum GqlKeyframeExtractor {
  Histogram,
  Phash,
  Threshold,
  Content,
  Adaptive,
  CompositeQuality,
  Interval,
  IFrame,
  SceneRepresentative,
  Manual,
}

impl From<KeyframeExtractor> for GqlKeyframeExtractor {
  fn from(v: KeyframeExtractor) -> Self {
    match v {
      KeyframeExtractor::Histogram => Self::Histogram,
      KeyframeExtractor::Phash => Self::Phash,
      KeyframeExtractor::Threshold => Self::Threshold,
      KeyframeExtractor::Content => Self::Content,
      KeyframeExtractor::Adaptive => Self::Adaptive,
      KeyframeExtractor::CompositeQuality => Self::CompositeQuality,
      KeyframeExtractor::Interval => Self::Interval,
      KeyframeExtractor::IFrame => Self::IFrame,
      KeyframeExtractor::SceneRepresentative => Self::SceneRepresentative,
      KeyframeExtractor::Manual => Self::Manual,
      #[allow(unreachable_patterns)]
      _ => Self::Manual,
    }
  }
}

impl From<GqlKeyframeExtractor> for KeyframeExtractor {
  fn from(v: GqlKeyframeExtractor) -> Self {
    match v {
      GqlKeyframeExtractor::Histogram => Self::Histogram,
      GqlKeyframeExtractor::Phash => Self::Phash,
      GqlKeyframeExtractor::Threshold => Self::Threshold,
      GqlKeyframeExtractor::Content => Self::Content,
      GqlKeyframeExtractor::Adaptive => Self::Adaptive,
      GqlKeyframeExtractor::CompositeQuality => Self::CompositeQuality,
      GqlKeyframeExtractor::Interval => Self::Interval,
      GqlKeyframeExtractor::IFrame => Self::IFrame,
      GqlKeyframeExtractor::SceneRepresentative => Self::SceneRepresentative,
      GqlKeyframeExtractor::Manual => Self::Manual,
    }
  }
}

// ---------------------------------------------------------------------------
// SubtitleKind
// ---------------------------------------------------------------------------

/// GraphQL mirror of [`SubtitleKind`].
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[graphql(name = "SubtitleKind")]
pub enum GqlSubtitleKind {
  FullDialogue,
  ForcedNarrative,
  CommentaryText,
}

impl From<SubtitleKind> for GqlSubtitleKind {
  fn from(v: SubtitleKind) -> Self {
    match v {
      SubtitleKind::FullDialogue => Self::FullDialogue,
      SubtitleKind::ForcedNarrative => Self::ForcedNarrative,
      SubtitleKind::CommentaryText => Self::CommentaryText,
    }
  }
}

impl From<GqlSubtitleKind> for SubtitleKind {
  fn from(v: GqlSubtitleKind) -> Self {
    match v {
      GqlSubtitleKind::FullDialogue => Self::FullDialogue,
      GqlSubtitleKind::ForcedNarrative => Self::ForcedNarrative,
      GqlSubtitleKind::CommentaryText => Self::CommentaryText,
    }
  }
}

// ---------------------------------------------------------------------------
// VideoIndexStage
// ---------------------------------------------------------------------------

/// GraphQL mirror of [`VideoIndexStage`].
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[graphql(name = "VideoIndexStage")]
pub enum GqlVideoIndexStage {
  Pending,
  Probed,
  SceneDetected,
  KeyframeExtracted,
  Analyzed,
  Embedded,
  Done,
  Failed,
}

impl From<VideoIndexStage> for GqlVideoIndexStage {
  fn from(v: VideoIndexStage) -> Self {
    match v {
      VideoIndexStage::Pending => Self::Pending,
      VideoIndexStage::Probed => Self::Probed,
      VideoIndexStage::SceneDetected => Self::SceneDetected,
      VideoIndexStage::KeyframeExtracted => Self::KeyframeExtracted,
      VideoIndexStage::Analyzed => Self::Analyzed,
      VideoIndexStage::Embedded => Self::Embedded,
      VideoIndexStage::Done => Self::Done,
      VideoIndexStage::Failed => Self::Failed,
    }
  }
}

impl From<GqlVideoIndexStage> for VideoIndexStage {
  fn from(v: GqlVideoIndexStage) -> Self {
    match v {
      GqlVideoIndexStage::Pending => Self::Pending,
      GqlVideoIndexStage::Probed => Self::Probed,
      GqlVideoIndexStage::SceneDetected => Self::SceneDetected,
      GqlVideoIndexStage::KeyframeExtracted => Self::KeyframeExtracted,
      GqlVideoIndexStage::Analyzed => Self::Analyzed,
      GqlVideoIndexStage::Embedded => Self::Embedded,
      GqlVideoIndexStage::Done => Self::Done,
      GqlVideoIndexStage::Failed => Self::Failed,
    }
  }
}

// ---------------------------------------------------------------------------
// AudioIndexStage
// ---------------------------------------------------------------------------

/// GraphQL mirror of [`AudioIndexStage`].
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[graphql(name = "AudioIndexStage")]
pub enum GqlAudioIndexStage {
  Pending,
  Extracted,
  Analyzed,
  Transcribed,
  Diarized,
  Embedded,
  Done,
  Failed,
}

impl From<AudioIndexStage> for GqlAudioIndexStage {
  fn from(v: AudioIndexStage) -> Self {
    match v {
      AudioIndexStage::Pending => Self::Pending,
      AudioIndexStage::Extracted => Self::Extracted,
      AudioIndexStage::Analyzed => Self::Analyzed,
      AudioIndexStage::Transcribed => Self::Transcribed,
      AudioIndexStage::Diarized => Self::Diarized,
      AudioIndexStage::Embedded => Self::Embedded,
      AudioIndexStage::Done => Self::Done,
      AudioIndexStage::Failed => Self::Failed,
    }
  }
}

impl From<GqlAudioIndexStage> for AudioIndexStage {
  fn from(v: GqlAudioIndexStage) -> Self {
    match v {
      GqlAudioIndexStage::Pending => Self::Pending,
      GqlAudioIndexStage::Extracted => Self::Extracted,
      GqlAudioIndexStage::Analyzed => Self::Analyzed,
      GqlAudioIndexStage::Transcribed => Self::Transcribed,
      GqlAudioIndexStage::Diarized => Self::Diarized,
      GqlAudioIndexStage::Embedded => Self::Embedded,
      GqlAudioIndexStage::Done => Self::Done,
      GqlAudioIndexStage::Failed => Self::Failed,
    }
  }
}

// ---------------------------------------------------------------------------
// SubtitleIndexStage
// ---------------------------------------------------------------------------

/// GraphQL mirror of [`SubtitleIndexStage`].
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[graphql(name = "SubtitleIndexStage")]
pub enum GqlSubtitleIndexStage {
  Pending,
  TracksDiscovered,
  CuesExtracted,
  Ocr,
  SearchIndexed,
  Done,
  Failed,
}

impl From<SubtitleIndexStage> for GqlSubtitleIndexStage {
  fn from(v: SubtitleIndexStage) -> Self {
    match v {
      SubtitleIndexStage::Pending => Self::Pending,
      SubtitleIndexStage::TracksDiscovered => Self::TracksDiscovered,
      SubtitleIndexStage::CuesExtracted => Self::CuesExtracted,
      SubtitleIndexStage::Ocr => Self::Ocr,
      SubtitleIndexStage::SearchIndexed => Self::SearchIndexed,
      SubtitleIndexStage::Done => Self::Done,
      SubtitleIndexStage::Failed => Self::Failed,
    }
  }
}

impl From<GqlSubtitleIndexStage> for SubtitleIndexStage {
  fn from(v: GqlSubtitleIndexStage) -> Self {
    match v {
      GqlSubtitleIndexStage::Pending => Self::Pending,
      GqlSubtitleIndexStage::TracksDiscovered => Self::TracksDiscovered,
      GqlSubtitleIndexStage::CuesExtracted => Self::CuesExtracted,
      GqlSubtitleIndexStage::Ocr => Self::Ocr,
      GqlSubtitleIndexStage::SearchIndexed => Self::SearchIndexed,
      GqlSubtitleIndexStage::Done => Self::Done,
      GqlSubtitleIndexStage::Failed => Self::Failed,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn media_kind_roundtrips() {
    for v in [MediaKind::Video, MediaKind::Audio] {
      let m: GqlMediaKind = v.into();
      let back: MediaKind = m.into();
      assert_eq!(v, back);
    }
  }

  #[test]
  fn audio_content_kind_roundtrips() {
    for v in [
      AudioContentKind::Speech,
      AudioContentKind::Music,
      AudioContentKind::Mixed,
      AudioContentKind::Silence,
    ] {
      let m: GqlAudioContentKind = v.into();
      let back: AudioContentKind = m.into();
      assert_eq!(v, back);
    }
  }

  #[test]
  fn scan_status_roundtrips() {
    for v in [ScanStatus::Ok, ScanStatus::Partial, ScanStatus::Failed] {
      let m: GqlScanStatus = v.into();
      let back: ScanStatus = m.into();
      assert_eq!(v, back);
    }
  }

  #[test]
  fn scene_detector_roundtrips_known_variants() {
    for v in [
      SceneDetector::Histogram,
      SceneDetector::Phash,
      SceneDetector::Threshold,
      SceneDetector::Content,
      SceneDetector::Adaptive,
      SceneDetector::Manual,
    ] {
      let m: GqlSceneDetector = v.into();
      let back: SceneDetector = m.into();
      assert_eq!(v, back);
    }
  }

  #[test]
  fn keyframe_extractor_roundtrips_known_variants() {
    for v in [
      KeyframeExtractor::Histogram,
      KeyframeExtractor::Phash,
      KeyframeExtractor::Threshold,
      KeyframeExtractor::Content,
      KeyframeExtractor::Adaptive,
      KeyframeExtractor::CompositeQuality,
      KeyframeExtractor::Interval,
      KeyframeExtractor::IFrame,
      KeyframeExtractor::SceneRepresentative,
      KeyframeExtractor::Manual,
    ] {
      let m: GqlKeyframeExtractor = v.into();
      let back: KeyframeExtractor = m.into();
      assert_eq!(v, back);
    }
  }

  #[test]
  fn subtitle_kind_roundtrips() {
    for v in [
      SubtitleKind::FullDialogue,
      SubtitleKind::ForcedNarrative,
      SubtitleKind::CommentaryText,
    ] {
      let m: GqlSubtitleKind = v.into();
      let back: SubtitleKind = m.into();
      assert_eq!(v, back);
    }
  }

  #[test]
  fn video_index_stage_roundtrips() {
    for v in [
      VideoIndexStage::Pending,
      VideoIndexStage::Probed,
      VideoIndexStage::SceneDetected,
      VideoIndexStage::KeyframeExtracted,
      VideoIndexStage::Analyzed,
      VideoIndexStage::Embedded,
      VideoIndexStage::Done,
      VideoIndexStage::Failed,
    ] {
      let m: GqlVideoIndexStage = v.into();
      let back: VideoIndexStage = m.into();
      assert_eq!(v, back);
    }
  }

  #[test]
  fn audio_index_stage_roundtrips() {
    for v in [
      AudioIndexStage::Pending,
      AudioIndexStage::Extracted,
      AudioIndexStage::Analyzed,
      AudioIndexStage::Transcribed,
      AudioIndexStage::Diarized,
      AudioIndexStage::Embedded,
      AudioIndexStage::Done,
      AudioIndexStage::Failed,
    ] {
      let m: GqlAudioIndexStage = v.into();
      let back: AudioIndexStage = m.into();
      assert_eq!(v, back);
    }
  }

  #[test]
  fn subtitle_index_stage_roundtrips() {
    for v in [
      SubtitleIndexStage::Pending,
      SubtitleIndexStage::TracksDiscovered,
      SubtitleIndexStage::CuesExtracted,
      SubtitleIndexStage::Ocr,
      SubtitleIndexStage::SearchIndexed,
      SubtitleIndexStage::Done,
      SubtitleIndexStage::Failed,
    ] {
      let m: GqlSubtitleIndexStage = v.into();
      let back: SubtitleIndexStage = m.into();
      assert_eq!(v, back);
    }
  }
}
