//! Mediaschema-owned enums (locked `schema/enums.md` r4).
//!
//! **Boundary:** media-stream *descriptor* vocabulary (codec/format/layout
//! enums + `TrackDisposition`) lives in **`::mediaframe`**, not here — those
//! become extern types when mediaframe ships the post-`0.1.0` minor; this
//! module defines only the **findit pipeline / analysis** enums.
//!
//! `#[non_exhaustive]` is applied to enums with a future-extension story
//! (engines / pipelines may add stages). Closed enums (`MediaKind`,
//! `ScanStatus`, the coarse `*IndexStage`s, `SubtitleKind`,
//! `AudioContentKind`) are explicitly closed per the locked spec.

use crate::domain::bitflags::{AudioIndexStatus, SubtitleIndexStatus, VideoIndexStatus};

// ===========================================================================
// MediaKind — the kind of media (drives which facets are created)
// ===========================================================================

/// Top-level media classification. **Closed** — `kind` is set at probe and
/// drives which facets (`Video`/`Audio`/`Subtitle`) the schema creates;
/// pre-probe is a different lifecycle, not an `Unknown` arm here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MediaKind {
    #[default]
    Video,
    Audio,
}

// ===========================================================================
// SceneDetector — which of the `scenesdetect` engine modules produced a Scene
// ===========================================================================

/// Which `scenesdetect` engine module raised a `Scene` boundary. Mirrors the
/// 5 detection modules + a manual escape hatch. `#[non_exhaustive]` —
/// `scenesdetect` may add detectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SceneDetector {
    Histogram,
    Phash,
    Threshold,
    Content,
    Adaptive,
    /// User-created / imported boundary (not from a detector).
    Manual,
}

// ===========================================================================
// KeyframeExtractor — which extractor produced a Keyframe
// ===========================================================================

/// Which extractor produced a `Keyframe`. A scene-detector boundary frame is
/// a keyframe too, so this enum is a **superset** of [`SceneDetector`] plus
/// the dedicated keyframe-extractor variants. `#[non_exhaustive]`.
///
/// Replaces the dropped closed `KeyframeKind` (locked `enums.md` r4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KeyframeExtractor {
    // All SceneDetector variants — scene-boundary frames are also keyframes.
    Histogram,
    Phash,
    Threshold,
    Content,
    Adaptive,
    // Dedicated keyframe-extractor variants.
    CompositeQuality,
    Interval,
    IFrame,
    SceneRepresentative,
    Manual,
}

// ===========================================================================
// SubtitleKind — subtitle role (a findit selection/search facet)
// ===========================================================================

/// Subtitle role — *not* a raw stream property (those live in
/// `::mediaframe`); a findit selection/search facet used for default-track
/// picking and faceted UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SubtitleKind {
    #[default]
    FullDialogue,
    /// Forced narrative (FFmpeg `AV_DISPOSITION_FORCED`).
    ForcedNarrative,
    /// Commentary text (FFmpeg `AV_DISPOSITION_COMMENT`).
    CommentaryText,
}

// ===========================================================================
// AudioContentKind — coarse content classification (analyze stage output)
// ===========================================================================

/// Coarse audio-track content classification — drives whether to
/// transcribe/diarize this track at all. Output of the audio `CLASSIFIED`
/// stage; **closed** vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AudioContentKind {
    #[default]
    Speech,
    Music,
    Mixed,
    Silence,
}

// ===========================================================================
// ScanStatus — WatchedLocation reconcile-sweep status
// ===========================================================================

/// Status of a `WatchedLocation` reconcile sweep (bootstrap / after-downtime
/// / volume-remount catch-up). Closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ScanStatus {
    #[default]
    Ok,
    Partial,
    Failed,
}

// ===========================================================================
// VideoIndexStage — derived coarse stage from VideoIndexStatus + errors
// ===========================================================================

/// Coarse derived stage for a `VideoTrack`'s indexing lifecycle.
///
/// **Derived** from [`VideoIndexStatus`] + `index_errors` (the source of
/// truth) — *not* an independently stored field. `Failed` has precedence:
/// any non-empty `index_errors` short-circuits to `Failed`; otherwise the
/// stage advances through the locked progression matching the verified
/// 7-bit `VideoIndexStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VideoIndexStage {
    /// No stage bits set yet.
    #[default]
    Pending,
    /// `PROBED`.
    Probed,
    /// `PROBED | SCENE_DETECTED`.
    SceneDetected,
    /// + `KEYFRAME_EXTRACTED`.
    KeyframeExtracted,
    /// + at least one of `VLM_ANALYZED` / `APPLE_VISION_ANALYZED`.
    Analyzed,
    /// + at least one of `TEXT_EMBEDDING_FINISHED` / `SCENE_EMBEDDING_FINISHED`.
    Embedded,
    /// All `is_fully_indexed()` bits set.
    Done,
    /// `index_errors` non-empty (precedence over any progression).
    Failed,
}

impl VideoIndexStage {
    /// Derive the coarse stage from the verified-bit status + an
    /// `index_errors`-is-empty witness.
    pub fn from_status(status: VideoIndexStatus, has_errors: bool) -> Self {
        if has_errors {
            return Self::Failed;
        }
        if status.is_fully_indexed() {
            return Self::Done;
        }
        if status.intersects(
            VideoIndexStatus::TEXT_EMBEDDING_FINISHED | VideoIndexStatus::SCENE_EMBEDDING_FINISHED,
        ) {
            Self::Embedded
        } else if status
            .intersects(VideoIndexStatus::VLM_ANALYZED | VideoIndexStatus::APPLE_VISION_ANALYZED)
        {
            Self::Analyzed
        } else if status.contains(VideoIndexStatus::KEYFRAME_EXTRACTED) {
            Self::KeyframeExtracted
        } else if status.contains(VideoIndexStatus::SCENE_DETECTED) {
            Self::SceneDetected
        } else if status.contains(VideoIndexStatus::PROBED) {
            Self::Probed
        } else {
            Self::Pending
        }
    }
}

// ===========================================================================
// AudioIndexStage — derived from the verified 11-bit ProcessingStage
// ===========================================================================

/// Coarse derived stage for an `AudioTrack`'s indexing lifecycle.
///
/// Derived from [`AudioIndexStatus`] (the real 11-bit `ProcessingStage` from
/// `findit-proto::database::audio`) + `index_errors`. `Failed` precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AudioIndexStage {
    #[default]
    Pending,
    /// `EXTRACTED` set.
    Extracted,
    /// + at least one of `CLASSIFIED` / `VAD_DONE` (analyzed for content).
    Analyzed,
    /// + `STT_DONE` (whisper transcript landed).
    Transcribed,
    /// + `SPEAKER_DONE` (pyannote / dia diarization landed).
    Diarized,
    /// + `TEXT_EMBED` (embedding pushed to LanceDB).
    Embedded,
    /// All `is_fully_indexed()` bits set.
    Done,
    Failed,
}

impl AudioIndexStage {
    pub fn from_status(status: AudioIndexStatus, has_errors: bool) -> Self {
        if has_errors {
            return Self::Failed;
        }
        if status.is_fully_indexed() {
            return Self::Done;
        }
        if status.contains(AudioIndexStatus::TEXT_EMBED) {
            Self::Embedded
        } else if status.contains(AudioIndexStatus::SPEAKER_DONE) {
            Self::Diarized
        } else if status.contains(AudioIndexStatus::STT_DONE) {
            Self::Transcribed
        } else if status
            .intersects(AudioIndexStatus::CLASSIFIED | AudioIndexStatus::VAD_DONE)
        {
            Self::Analyzed
        } else if status.contains(AudioIndexStatus::EXTRACTED) {
            Self::Extracted
        } else {
            Self::Pending
        }
    }
}

// ===========================================================================
// SubtitleIndexStage — derived
// ===========================================================================

/// Coarse derived stage for a `SubtitleTrack`'s indexing lifecycle. Derived
/// from [`SubtitleIndexStatus`] + `index_errors`; `Failed` precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SubtitleIndexStage {
    #[default]
    Pending,
    /// `TRACKS_DISCOVERED`.
    TracksDiscovered,
    /// + `CUES_EXTRACTED`.
    CuesExtracted,
    /// + `OCR_DONE` (image-based only).
    Ocr,
    /// + `SEARCH_INDEXED`.
    SearchIndexed,
    /// All `is_fully_indexed()` bits set.
    Done,
    Failed,
}

impl SubtitleIndexStage {
    pub fn from_status(status: SubtitleIndexStatus, has_errors: bool) -> Self {
        if has_errors {
            return Self::Failed;
        }
        if status.is_fully_indexed() {
            return Self::Done;
        }
        if status.contains(SubtitleIndexStatus::SEARCH_INDEXED) {
            Self::SearchIndexed
        } else if status.contains(SubtitleIndexStatus::OCR_DONE) {
            Self::Ocr
        } else if status.contains(SubtitleIndexStatus::CUES_EXTRACTED) {
            Self::CuesExtracted
        } else if status.contains(SubtitleIndexStatus::TRACKS_DISCOVERED) {
            Self::TracksDiscovered
        } else {
            Self::Pending
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_kind_default_video() {
        assert_eq!(MediaKind::default(), MediaKind::Video);
    }

    #[test]
    fn keyframe_extractor_superset_of_scene_detector() {
        // KeyframeExtractor must carry every SceneDetector variant — a
        // scene-detector boundary frame is also a keyframe.
        let _ = SceneDetector::Histogram;
        let _ = KeyframeExtractor::Histogram;
        let _ = KeyframeExtractor::Phash;
        let _ = KeyframeExtractor::Threshold;
        let _ = KeyframeExtractor::Content;
        let _ = KeyframeExtractor::Adaptive;
        let _ = KeyframeExtractor::Manual;
        // ...plus its own variants:
        let _ = KeyframeExtractor::CompositeQuality;
        let _ = KeyframeExtractor::Interval;
        let _ = KeyframeExtractor::IFrame;
        let _ = KeyframeExtractor::SceneRepresentative;
    }

    #[test]
    fn video_index_stage_progression() {
        assert_eq!(
            VideoIndexStage::from_status(VideoIndexStatus::empty(), false),
            VideoIndexStage::Pending
        );
        assert_eq!(
            VideoIndexStage::from_status(VideoIndexStatus::PROBED, false),
            VideoIndexStage::Probed
        );
        assert_eq!(
            VideoIndexStage::from_status(
                VideoIndexStatus::PROBED | VideoIndexStatus::SCENE_DETECTED,
                false
            ),
            VideoIndexStage::SceneDetected
        );
        assert_eq!(
            VideoIndexStage::from_status(
                VideoIndexStatus::PROBED
                    | VideoIndexStatus::SCENE_DETECTED
                    | VideoIndexStatus::KEYFRAME_EXTRACTED,
                false
            ),
            VideoIndexStage::KeyframeExtracted
        );
        assert_eq!(
            VideoIndexStage::from_status(
                VideoIndexStatus::PROBED
                    | VideoIndexStatus::SCENE_DETECTED
                    | VideoIndexStatus::KEYFRAME_EXTRACTED
                    | VideoIndexStatus::VLM_ANALYZED,
                false
            ),
            VideoIndexStage::Analyzed
        );
        assert_eq!(
            VideoIndexStage::from_status(
                VideoIndexStatus::PROBED
                    | VideoIndexStatus::SCENE_DETECTED
                    | VideoIndexStatus::KEYFRAME_EXTRACTED
                    | VideoIndexStatus::APPLE_VISION_ANALYZED
                    | VideoIndexStatus::TEXT_EMBEDDING_FINISHED,
                false
            ),
            VideoIndexStage::Embedded
        );
        assert_eq!(
            VideoIndexStage::from_status(VideoIndexStatus::fully_indexed_mask(), false),
            VideoIndexStage::Done
        );
    }

    #[test]
    fn video_index_stage_failed_precedence() {
        // Failed precedence: any error → Failed, regardless of progression.
        assert_eq!(
            VideoIndexStage::from_status(VideoIndexStatus::fully_indexed_mask(), true),
            VideoIndexStage::Failed
        );
        assert_eq!(
            VideoIndexStage::from_status(VideoIndexStatus::empty(), true),
            VideoIndexStage::Failed
        );
    }

    #[test]
    fn audio_index_stage_progression() {
        assert_eq!(
            AudioIndexStage::from_status(AudioIndexStatus::empty(), false),
            AudioIndexStage::Pending
        );
        assert_eq!(
            AudioIndexStage::from_status(AudioIndexStatus::EXTRACTED, false),
            AudioIndexStage::Extracted
        );
        assert_eq!(
            AudioIndexStage::from_status(
                AudioIndexStatus::EXTRACTED | AudioIndexStatus::VAD_DONE,
                false
            ),
            AudioIndexStage::Analyzed
        );
        assert_eq!(
            AudioIndexStage::from_status(
                AudioIndexStatus::EXTRACTED
                    | AudioIndexStatus::CLASSIFIED
                    | AudioIndexStatus::VAD_DONE
                    | AudioIndexStatus::STT_DONE,
                false
            ),
            AudioIndexStage::Transcribed
        );
        assert_eq!(
            AudioIndexStage::from_status(
                AudioIndexStatus::EXTRACTED
                    | AudioIndexStatus::CLASSIFIED
                    | AudioIndexStatus::VAD_DONE
                    | AudioIndexStatus::STT_DONE
                    | AudioIndexStatus::SPEAKER_DONE,
                false
            ),
            AudioIndexStage::Diarized
        );
        assert_eq!(
            AudioIndexStage::from_status(
                AudioIndexStatus::EXTRACTED
                    | AudioIndexStatus::STT_DONE
                    | AudioIndexStatus::SPEAKER_DONE
                    | AudioIndexStatus::TEXT_EMBED,
                false
            ),
            AudioIndexStage::Embedded
        );
        assert_eq!(
            AudioIndexStage::from_status(AudioIndexStatus::fully_indexed_mask(), false),
            AudioIndexStage::Done
        );
        assert_eq!(
            AudioIndexStage::from_status(AudioIndexStatus::empty(), true),
            AudioIndexStage::Failed
        );
    }

    #[test]
    fn subtitle_index_stage_progression() {
        assert_eq!(
            SubtitleIndexStage::from_status(SubtitleIndexStatus::empty(), false),
            SubtitleIndexStage::Pending
        );
        assert_eq!(
            SubtitleIndexStage::from_status(SubtitleIndexStatus::TRACKS_DISCOVERED, false),
            SubtitleIndexStage::TracksDiscovered
        );
        assert_eq!(
            SubtitleIndexStage::from_status(
                SubtitleIndexStatus::TRACKS_DISCOVERED | SubtitleIndexStatus::CUES_EXTRACTED,
                false
            ),
            SubtitleIndexStage::CuesExtracted
        );
        assert_eq!(
            SubtitleIndexStage::from_status(
                SubtitleIndexStatus::TRACKS_DISCOVERED
                    | SubtitleIndexStatus::CUES_EXTRACTED
                    | SubtitleIndexStatus::OCR_DONE,
                false
            ),
            SubtitleIndexStage::Ocr
        );
        assert_eq!(
            SubtitleIndexStage::from_status(SubtitleIndexStatus::fully_indexed_mask(), false),
            SubtitleIndexStage::Done
        );
        assert_eq!(
            SubtitleIndexStage::from_status(SubtitleIndexStatus::empty(), true),
            SubtitleIndexStage::Failed
        );
    }
}
