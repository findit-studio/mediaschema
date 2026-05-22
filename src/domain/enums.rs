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

use derive_more::IsVariant;

// The bitflags imports + `ErrorCode` / `ErrorInfo` are referenced only
// by the `{Video,Audio,Subtitle}IndexStage::from_status` impl blocks
// below, which are themselves `any(std, alloc)`-gated (they take
// `&[ErrorInfo]` and `ErrorInfo` is heap-tier). Gating the imports too
// keeps `--no-default-features` warning-clean under `-D warnings`.
#[cfg(any(feature = "std", feature = "alloc"))]
use crate::domain::{
  bitflags::{AudioIndexStatus, SubtitleIndexStatus, VideoIndexStatus},
  primitives::{ErrorCode, ErrorInfo},
};

// ===========================================================================
// MediaKind — the kind of media (drives which facets are created)
// ===========================================================================

/// Top-level media classification. **Closed** — `kind` is set at probe and
/// drives which facets (`Video`/`Audio`/`Subtitle`) the schema creates;
/// pre-probe is a different lifecycle, not an `Unknown` arm here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, IsVariant)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IsVariant)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IsVariant)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, IsVariant)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, IsVariant)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, IsVariant)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, IsVariant)]
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

// The stage-derivation methods take `&[ErrorInfo]`, which is itself
// `feature = "alloc"`-gated. Gate the whole impl block accordingly —
// the bare `VideoIndexStage` enum + `IsVariant` predicates remain
// available in pure no-std no-alloc.
#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
impl VideoIndexStage {
  /// Map a video-pipeline `ErrorCode` to its stage success bit, so a
  /// retained error whose corresponding bit is now set can be filtered
  /// out as **stale** (the retry succeeded after this error landed in
  /// the history). Returns `None` for errors with no clean stage
  /// mapping — those are always treated as live.
  fn error_stage_bit(code: ErrorCode) -> Option<VideoIndexStatus> {
    use VideoIndexStatus as S;
    match code {
      ErrorCode::ProbeCorrupt
      | ErrorCode::ProbeUnsupportedFormat
      | ErrorCode::ProbeNoVideoStream
      | ErrorCode::ProbeNoAudioStream => Some(S::PROBED),
      ErrorCode::SceneDetectionFailed | ErrorCode::SceneDetectionModelError => {
        Some(S::SCENE_DETECTED)
      }
      ErrorCode::VlmFailed | ErrorCode::VlmModelError => Some(S::VLM_ANALYZED),
      ErrorCode::AppleVisionFailed | ErrorCode::AppleVisionRequestFailed => {
        Some(S::APPLE_VISION_ANALYZED)
      }
      ErrorCode::EmbeddingFailed
      | ErrorCode::EmbeddingModelError
      | ErrorCode::EmbeddingModelLoadFailed
      | ErrorCode::EmbeddingPreprocessFailed
      | ErrorCode::EmbeddingInferenceFailed
      | ErrorCode::EmbeddingOutputInvalid => {
        // Either embedding stage clears this error (the locked schema
        // treats them as equivalent embedding completion signals).
        Some(S::TEXT_EMBEDDING_FINISHED | S::SCENE_EMBEDDING_FINISHED)
      }
      _ => None,
    }
  }

  /// Is the error currently **live** against `status`? An error is
  /// considered resolved (stale) iff its mapped stage success bit is set
  /// in `status` — i.e. a retry of that stage has since landed
  /// successfully.
  fn is_live(status: VideoIndexStatus, e: &ErrorInfo) -> bool {
    match Self::error_stage_bit(e.code()) {
      // For OR'd masks, "stage succeeded" iff *any* of the bits are
      // set; this matches the OR semantics of the analyzed /
      // embedded stages where either producer satisfies the stage.
      Some(bit) => !status.intersects(bit),
      None => true,
    }
  }

  /// Derive the coarse stage from the verified-bit status + the
  /// structured `index_errors` history. **Failed** precedence applies
  /// only to *live* errors (errors whose stage success bit isn't set —
  /// see [`VideoIndexStage::is_live`]); a successful retry clears a
  /// stale error without requiring the caller to mutate the history.
  ///
  /// The non-failed walk is **contiguous**: each next stage requires
  /// its *prerequisite* bits to be set, so an out-of-order bit (e.g.
  /// `TEXT_EMBEDDING_FINISHED` without `KEYFRAME_EXTRACTED`) does not
  /// jump to `Embedded` — it stays at the furthest contiguous stage.
  pub fn from_status(status: VideoIndexStatus, errors: &[ErrorInfo]) -> Self {
    use VideoIndexStatus as S;
    if errors.iter().any(|e| Self::is_live(status, e)) {
      return Self::Failed;
    }
    if status.is_fully_indexed() {
      return Self::Done;
    }
    // Contiguous stage walk — return at the first prerequisite gap.
    if !status.contains(S::PROBED) {
      return Self::Pending;
    }
    if !status.contains(S::SCENE_DETECTED) {
      return Self::Probed;
    }
    if !status.contains(S::KEYFRAME_EXTRACTED) {
      return Self::SceneDetected;
    }
    if !status.intersects(S::VLM_ANALYZED | S::APPLE_VISION_ANALYZED) {
      return Self::KeyframeExtracted;
    }
    if !status.intersects(S::TEXT_EMBEDDING_FINISHED | S::SCENE_EMBEDDING_FINISHED) {
      return Self::Analyzed;
    }
    Self::Embedded
  }
}

// ===========================================================================
// AudioIndexStage — derived from the verified 11-bit ProcessingStage
// ===========================================================================

/// Coarse derived stage for an `AudioTrack`'s indexing lifecycle.
///
/// Derived from [`AudioIndexStatus`] (the real 11-bit `ProcessingStage` from
/// `findit-proto::database::audio`) + `index_errors`. `Failed` precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, IsVariant)]
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

// Alloc-gated for the same reason as `VideoIndexStage`'s impl block.
#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
impl AudioIndexStage {
  /// Map an audio-pipeline `ErrorCode` to its stage success bit. Same
  /// stale-error filtering rationale as [`VideoIndexStage::error_stage_bit`].
  fn error_stage_bit(code: ErrorCode) -> Option<AudioIndexStatus> {
    use AudioIndexStatus as S;
    match code {
      ErrorCode::ProbeCorrupt
      | ErrorCode::ProbeUnsupportedFormat
      | ErrorCode::ProbeNoAudioStream
      | ErrorCode::ProbeNoVideoStream => Some(S::EXTRACTED),
      ErrorCode::CedFailed | ErrorCode::CedRequestFailed | ErrorCode::CedModelError => {
        Some(S::CED_DONE)
      }
      ErrorCode::TranscriptionFailed | ErrorCode::TranscriptionModelError => Some(S::STT_DONE),
      ErrorCode::EmbeddingFailed
      | ErrorCode::EmbeddingModelError
      | ErrorCode::EmbeddingModelLoadFailed
      | ErrorCode::EmbeddingPreprocessFailed
      | ErrorCode::EmbeddingInferenceFailed
      | ErrorCode::EmbeddingOutputInvalid => Some(S::TEXT_EMBED),
      _ => None,
    }
  }

  fn is_live(status: AudioIndexStatus, e: &ErrorInfo) -> bool {
    match Self::error_stage_bit(e.code()) {
      Some(bit) => !status.intersects(bit),
      None => true,
    }
  }

  /// Derive the coarse stage from the verified 11-bit status + structured
  /// `index_errors`. `Failed` precedence applies only to live errors; the
  /// non-failed walk is contiguous (each later stage requires its
  /// prerequisite stage's bits to be set, matching the locked
  /// pipeline order).
  pub fn from_status(status: AudioIndexStatus, errors: &[ErrorInfo]) -> Self {
    use AudioIndexStatus as S;
    if errors.iter().any(|e| Self::is_live(status, e)) {
      return Self::Failed;
    }
    if status.is_fully_indexed() {
      return Self::Done;
    }
    // Contiguous stage walk: EXTRACTED → (CLASSIFIED | VAD_DONE) →
    // STT_DONE → SPEAKER_DONE → TEXT_EMBED, with the secondary stages
    // (LLM/CED/CLAP/EBUR128/FPRINT) folded into Done. Each later
    // stage requires its predecessors.
    if !status.contains(S::EXTRACTED) {
      return Self::Pending;
    }
    if !status.intersects(S::CLASSIFIED | S::VAD_DONE) {
      return Self::Extracted;
    }
    if !status.contains(S::STT_DONE) {
      return Self::Analyzed;
    }
    if !status.contains(S::SPEAKER_DONE) {
      return Self::Transcribed;
    }
    if !status.contains(S::TEXT_EMBED) {
      return Self::Diarized;
    }
    Self::Embedded
  }
}

// ===========================================================================
// SubtitleIndexStage — derived
// ===========================================================================

/// Coarse derived stage for a `SubtitleTrack`'s indexing lifecycle. Derived
/// from [`SubtitleIndexStatus`] + `index_errors`; `Failed` precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, IsVariant)]
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

// Alloc-gated for the same reason as `VideoIndexStage`'s impl block.
#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
impl SubtitleIndexStage {
  /// Map a subtitle-pipeline `ErrorCode` to its stage success bit. Same
  /// stale-error filtering rationale as the video / audio mappings.
  /// OCR-related errors map to `OCR_DONE` only when the caller asserts
  /// the track *requires* OCR (see [`SubtitleIndexStage::from_status`]).
  fn error_stage_bit(code: ErrorCode, requires_ocr: bool) -> Option<SubtitleIndexStatus> {
    use SubtitleIndexStatus as S;
    match code {
      // Discovery errors clear once probing succeeds (locked schema
      // uses the same family of `Probe*` codes across all kinds).
      ErrorCode::ProbeCorrupt
      | ErrorCode::ProbeUnsupportedFormat
      | ErrorCode::ProbeNoVideoStream => Some(S::TRACKS_DISCOVERED),
      // Apple Vision OCR — only meaningful for image-based tracks.
      ErrorCode::AppleVisionFailed | ErrorCode::AppleVisionRequestFailed if requires_ocr => {
        Some(S::OCR_DONE)
      }
      _ => None,
    }
  }

  fn is_live(status: SubtitleIndexStatus, requires_ocr: bool, e: &ErrorInfo) -> bool {
    match Self::error_stage_bit(e.code(), requires_ocr) {
      Some(bit) => !status.intersects(bit),
      None => true,
    }
  }

  /// Derive the coarse stage from status + structured errors, taking
  /// `requires_ocr` into account.
  ///
  /// `requires_ocr` is `true` iff the subtitle codec is image-based
  /// (`mediaframe::SubtitleCodec::is_image_based() == Some(true)`).
  /// When `false`, `OCR_DONE` is **not** required for completion —
  /// a text subtitle that's `TRACKS_DISCOVERED | CUES_EXTRACTED |
  /// SEARCH_INDEXED` reaches `Done` without ever entering the
  /// `Ocr` stage. When `true`, OCR_DONE is required and the `Ocr`
  /// stage sits between `CuesExtracted` and `SearchIndexed`.
  ///
  /// Failed precedence applies only to *live* errors against the
  /// effective expected mask.
  ///
  /// `pub(crate)`: `requires_ocr` is an unbound caller-supplied bool, so
  /// exposing this publicly lets external code derive a `Done` stage for
  /// an unknown/image track without `OCR_DONE`. The only public stage
  /// path is [`SubtitleTrack::index_stage`], which binds `requires_ocr`
  /// from the track's codec/format internally.
  pub(crate) fn from_status(
    status: SubtitleIndexStatus,
    requires_ocr: bool,
    errors: &[ErrorInfo],
  ) -> Self {
    use SubtitleIndexStatus as S;
    if errors
      .iter()
      .any(|e| Self::is_live(status, requires_ocr, e))
    {
      return Self::Failed;
    }
    let mask = SubtitleIndexStatus::fully_indexed_mask(requires_ocr);
    if status.contains(mask) {
      return Self::Done;
    }
    // Contiguous walk. OCR_DONE is part of the chain only when
    // requires_ocr; otherwise the chain is
    // TRACKS_DISCOVERED → CUES_EXTRACTED → SEARCH_INDEXED.
    if !status.contains(S::TRACKS_DISCOVERED) {
      return Self::Pending;
    }
    if !status.contains(S::CUES_EXTRACTED) {
      return Self::TracksDiscovered;
    }
    if requires_ocr && !status.contains(S::OCR_DONE) {
      return Self::CuesExtracted;
    }
    if !status.contains(S::SEARCH_INDEXED) {
      // Without OCR in the chain, after CUES_EXTRACTED the next
      // stage is search indexing directly.
      return if requires_ocr {
        Self::Ocr
      } else {
        Self::CuesExtracted
      };
    }
    Self::SearchIndexed
  }
}

// ===========================================================================
// Tests
// ===========================================================================

// Tests exercise the `from_status` methods (which take `&[ErrorInfo]`)
// and therefore need heap-tier features. Gated on `any(std, alloc)`.
#[cfg(all(test, any(feature = "std", feature = "alloc")))]
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

  /// One stale `ProbeCorrupt` error that should be filtered by the
  /// live-error check once `PROBED` succeeds.
  fn probe_error() -> std::vec::Vec<ErrorInfo> {
    std::vec![ErrorInfo::code_only(ErrorCode::ProbeCorrupt)]
  }

  #[test]
  fn video_index_stage_progression() {
    use VideoIndexStatus as S;
    let no_err: &[ErrorInfo] = &[];
    assert_eq!(
      VideoIndexStage::from_status(S::empty(), no_err),
      VideoIndexStage::Pending
    );
    assert_eq!(
      VideoIndexStage::from_status(S::PROBED, no_err),
      VideoIndexStage::Probed
    );
    assert_eq!(
      VideoIndexStage::from_status(S::PROBED | S::SCENE_DETECTED, no_err),
      VideoIndexStage::SceneDetected
    );
    assert_eq!(
      VideoIndexStage::from_status(
        S::PROBED | S::SCENE_DETECTED | S::KEYFRAME_EXTRACTED,
        no_err
      ),
      VideoIndexStage::KeyframeExtracted
    );
    assert_eq!(
      VideoIndexStage::from_status(
        S::PROBED | S::SCENE_DETECTED | S::KEYFRAME_EXTRACTED | S::VLM_ANALYZED,
        no_err
      ),
      VideoIndexStage::Analyzed
    );
    assert_eq!(
      VideoIndexStage::from_status(
        S::PROBED
          | S::SCENE_DETECTED
          | S::KEYFRAME_EXTRACTED
          | S::APPLE_VISION_ANALYZED
          | S::TEXT_EMBEDDING_FINISHED,
        no_err
      ),
      VideoIndexStage::Embedded
    );
    assert_eq!(
      VideoIndexStage::from_status(S::fully_indexed_mask(), no_err),
      VideoIndexStage::Done
    );
  }

  #[test]
  fn video_index_stage_contiguous_walk_does_not_jump_on_out_of_order_bits() {
    // Regression for Codex round-1 finding #2: a stray
    // TEXT_EMBEDDING_FINISHED bit without the prerequisite chain
    // (PROBED | SCENE_DETECTED | KEYFRAME_EXTRACTED | analyzed) must
    // NOT report `Embedded` — the contiguous-walk rule pins the
    // stage at the furthest gap-free progression.
    use VideoIndexStatus as S;
    let no_err: &[ErrorInfo] = &[];
    let stray = S::TEXT_EMBEDDING_FINISHED; // nothing else set
    assert_eq!(
      VideoIndexStage::from_status(stray, no_err),
      VideoIndexStage::Pending,
      "stray late bit must not advance stage"
    );
    // PROBED set, but everything else is the stray late bit:
    assert_eq!(
      VideoIndexStage::from_status(S::PROBED | S::TEXT_EMBEDDING_FINISHED, no_err),
      VideoIndexStage::Probed,
    );
  }

  #[test]
  fn video_index_stage_failed_precedence_on_live_errors_only() {
    // Failed precedence applies to live errors (stage success bit
    // not set). A stale error whose stage has since succeeded must
    // NOT mark the track Failed.
    use VideoIndexStatus as S;
    // Live error: PROBED not set → ProbeCorrupt is live.
    assert_eq!(
      VideoIndexStage::from_status(S::empty(), &probe_error()),
      VideoIndexStage::Failed
    );
    // Stale error: PROBED is set → ProbeCorrupt has been resolved.
    assert_eq!(
      VideoIndexStage::from_status(S::PROBED, &probe_error()),
      VideoIndexStage::Probed,
      "successful retry must clear stale stage errors"
    );
    // Mix: stale ProbeCorrupt + live SceneDetectionFailed → Failed.
    let mixed = std::vec![
      ErrorInfo::code_only(ErrorCode::ProbeCorrupt),
      ErrorInfo::code_only(ErrorCode::SceneDetectionFailed),
    ];
    assert_eq!(
      VideoIndexStage::from_status(S::PROBED, &mixed),
      VideoIndexStage::Failed,
    );
  }

  #[test]
  fn audio_index_stage_progression() {
    use AudioIndexStatus as S;
    let no_err: &[ErrorInfo] = &[];
    assert_eq!(
      AudioIndexStage::from_status(S::empty(), no_err),
      AudioIndexStage::Pending
    );
    assert_eq!(
      AudioIndexStage::from_status(S::EXTRACTED, no_err),
      AudioIndexStage::Extracted
    );
    assert_eq!(
      AudioIndexStage::from_status(S::EXTRACTED | S::VAD_DONE, no_err),
      AudioIndexStage::Analyzed
    );
    assert_eq!(
      AudioIndexStage::from_status(
        S::EXTRACTED | S::CLASSIFIED | S::VAD_DONE | S::STT_DONE,
        no_err
      ),
      AudioIndexStage::Transcribed
    );
    assert_eq!(
      AudioIndexStage::from_status(
        S::EXTRACTED | S::CLASSIFIED | S::VAD_DONE | S::STT_DONE | S::SPEAKER_DONE,
        no_err
      ),
      AudioIndexStage::Diarized
    );
    assert_eq!(
      AudioIndexStage::from_status(S::fully_indexed_mask(), no_err),
      AudioIndexStage::Done
    );
    // Live error on empty status → Failed.
    assert_eq!(
      AudioIndexStage::from_status(S::empty(), &probe_error()),
      AudioIndexStage::Failed
    );
  }

  #[test]
  fn audio_index_stage_contiguous_walk() {
    // STT_DONE without EXTRACTED must not jump to Transcribed.
    use AudioIndexStatus as S;
    let no_err: &[ErrorInfo] = &[];
    assert_eq!(
      AudioIndexStage::from_status(S::STT_DONE, no_err),
      AudioIndexStage::Pending,
    );
  }

  #[test]
  fn subtitle_text_track_reaches_done_without_ocr() {
    // Regression for Codex PR #10 finding #1: a text subtitle
    // (requires_ocr = false) must reach Done when
    // TRACKS_DISCOVERED | CUES_EXTRACTED | SEARCH_INDEXED are set,
    // *without* OCR_DONE.
    use SubtitleIndexStatus as S;
    let no_err: &[ErrorInfo] = &[];
    let status = S::TRACKS_DISCOVERED | S::CUES_EXTRACTED | S::SEARCH_INDEXED;
    assert_eq!(
      SubtitleIndexStage::from_status(status, /* requires_ocr = */ false, no_err),
      SubtitleIndexStage::Done,
    );
    // Same bits + requires_ocr = true must NOT be Done (OCR is
    // genuinely missing for image-based tracks).
    assert_ne!(
      SubtitleIndexStage::from_status(status, /* requires_ocr = */ true, no_err),
      SubtitleIndexStage::Done,
    );
  }

  #[test]
  fn subtitle_index_stage_progression() {
    use SubtitleIndexStatus as S;
    let no_err: &[ErrorInfo] = &[];
    assert_eq!(
      SubtitleIndexStage::from_status(S::empty(), true, no_err),
      SubtitleIndexStage::Pending
    );
    assert_eq!(
      SubtitleIndexStage::from_status(S::TRACKS_DISCOVERED, true, no_err),
      SubtitleIndexStage::TracksDiscovered
    );
    assert_eq!(
      SubtitleIndexStage::from_status(S::TRACKS_DISCOVERED | S::CUES_EXTRACTED, true, no_err),
      SubtitleIndexStage::CuesExtracted
    );
    assert_eq!(
      SubtitleIndexStage::from_status(
        S::TRACKS_DISCOVERED | S::CUES_EXTRACTED | S::OCR_DONE,
        true,
        no_err
      ),
      SubtitleIndexStage::Ocr
    );
    assert_eq!(
      SubtitleIndexStage::from_status(S::fully_indexed_mask(true), true, no_err),
      SubtitleIndexStage::Done
    );
    assert_eq!(
      SubtitleIndexStage::from_status(S::empty(), true, &probe_error()),
      SubtitleIndexStage::Failed
    );
  }
}
