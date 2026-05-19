//! Mediaschema-owned bitflags companions (locked `schema/bitflags.md` r4).
//!
//! The wire keeps bare `u32`; the domain layer gets `bitflags!` companions.
//! **All bit values are verified against `findit-proto::database`** (the real
//! pipeline) — earlier doc-derived guesses were materially wrong.
//!
//! **Not here** — moved to `::mediaframe` per the descriptor re-scope:
//! `TrackDisposition` (FFmpeg `AV_DISPOSITION_*` — shared across all 3 track
//! types). It ships in the post-`0.1.0` mediaframe minor; consumers will
//! reference `::mediaframe::TrackDisposition`.
//!
//! **No per-track `error_status`** — error-state is **derived** from
//! stage-coded `index_errors: Vec<ErrorInfo>` + `index_status` (locked
//! decision). The `index_status` types below are the only per-track
//! "indexing bitflags" the domain models.

use bitflags::bitflags;

// ---------------------------------------------------------------------------
// MediaErrorFlags — root rollup (kept; locked)
// ---------------------------------------------------------------------------

bitflags! {
    /// Coarse per-kind error rollup on `Media`, **kept** as a deliberate
    /// list-query denormalization (same family as `track_progress` /
    /// `total_scenes`). A bit is set iff that kind's `track_progress.failed
    /// > 0` — drill down via the kind facet → `Track.index_errors`.
    ///
    /// `u16` chosen for future media kinds. Locked `schema/bitflags.md` r4.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct MediaErrorFlags: u16 {
        const VIDEO_ERROR    = 0x0001;
        const AUDIO_ERROR    = 0x0002;
        const SUBTITLE_ERROR = 0x0004;
        // Reserved bits 0x0008 .. for future kinds.
    }
}

// ---------------------------------------------------------------------------
// VideoIndexStatus — verified vs findit-proto::database::video
// ---------------------------------------------------------------------------

bitflags! {
    /// Per-`VideoTrack` indexing progress.
    ///
    /// **Authoritative bit values** from
    /// `findit-proto::database::video::VideoIndexStatus` (verified). A set
    /// bit asserts the stage **ran and its output landed** (incl. vectors
    /// pushed to LanceDB); the vector itself is not a domain field.
    ///
    /// Note distinct stages: `VLM_ANALYZED` ≠ `APPLE_VISION_ANALYZED` (two
    /// producers — locked `keyframe.md` producer-distinct model); two
    /// embedding stages (`TEXT_EMBEDDING_FINISHED` = EmbeddingGemma,
    /// `SCENE_EMBEDDING_FINISHED` = SigLIP2). Bit `0x08` is reserved.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct VideoIndexStatus: u32 {
        const PROBED                    = 0x01;
        const SCENE_DETECTED            = 0x02;
        const KEYFRAME_EXTRACTED        = 0x04;
        // 0x08 reserved.
        const VLM_ANALYZED              = 0x10;
        const APPLE_VISION_ANALYZED     = 0x20;
        const TEXT_EMBEDDING_FINISHED   = 0x40;
        const SCENE_EMBEDDING_FINISHED  = 0x80;
    }
}

impl VideoIndexStatus {
    /// The "fully indexed" mask — every stage bit set.
    #[inline]
    pub const fn fully_indexed_mask() -> Self {
        Self::from_bits_truncate(
            Self::PROBED.bits()
                | Self::SCENE_DETECTED.bits()
                | Self::KEYFRAME_EXTRACTED.bits()
                | Self::VLM_ANALYZED.bits()
                | Self::APPLE_VISION_ANALYZED.bits()
                | Self::TEXT_EMBEDDING_FINISHED.bits()
                | Self::SCENE_EMBEDDING_FINISHED.bits(),
        )
    }

    /// True iff every stage bit is set.
    #[inline]
    pub fn is_fully_indexed(&self) -> bool {
        self.contains(Self::fully_indexed_mask())
    }
}

// ---------------------------------------------------------------------------
// AudioIndexStatus — the verified real 11-bit `ProcessingStage`
// ---------------------------------------------------------------------------

bitflags! {
    /// Per-`AudioTrack` indexing progress.
    ///
    /// **The real 11-bit `ProcessingStage`** from
    /// `findit-proto::database::audio::ProcessingStage` — the user's earlier
    /// "VAD/STT/diarize/embed" 4-bit guess was wrong; this is the full
    /// pipeline (incl. CED sound-event detection, CLAP audio embedding,
    /// EBU-R128 loudness, chromaprint fingerprint).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct AudioIndexStatus: u32 {
        const EXTRACTED     = 0x001;
        const CLASSIFIED    = 0x002;
        const VAD_DONE      = 0x004;
        const STT_DONE      = 0x008;
        const SPEAKER_DONE  = 0x010;
        const LLM_DONE      = 0x020;
        const TEXT_EMBED    = 0x040;
        const CED_DONE      = 0x080;
        const CLAP_DONE     = 0x100;
        const EBUR128_DONE  = 0x200;
        const FPRINT_DONE   = 0x400;
    }
}

impl AudioIndexStatus {
    #[inline]
    pub const fn fully_indexed_mask() -> Self {
        Self::from_bits_truncate(
            Self::EXTRACTED.bits()
                | Self::CLASSIFIED.bits()
                | Self::VAD_DONE.bits()
                | Self::STT_DONE.bits()
                | Self::SPEAKER_DONE.bits()
                | Self::LLM_DONE.bits()
                | Self::TEXT_EMBED.bits()
                | Self::CED_DONE.bits()
                | Self::CLAP_DONE.bits()
                | Self::EBUR128_DONE.bits()
                | Self::FPRINT_DONE.bits(),
        )
    }

    #[inline]
    pub fn is_fully_indexed(&self) -> bool {
        self.contains(Self::fully_indexed_mask())
    }
}

// ---------------------------------------------------------------------------
// SubtitleIndexStatus — verified vs findit-proto::database::subtitle
// ---------------------------------------------------------------------------

bitflags! {
    /// Per-`SubtitleTrack` indexing progress.
    ///
    /// Authoritative names from `findit-proto::database::subtitle`
    /// (not the earlier doc-derived `PROBED/CUES_PARSED/...` guess).
    /// `OCR_DONE` only applies to image-based subs (PGS/DVBSUB).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct SubtitleIndexStatus: u32 {
        const TRACKS_DISCOVERED = 0x01;
        const CUES_EXTRACTED    = 0x02;
        const OCR_DONE          = 0x04;
        const SEARCH_INDEXED    = 0x08;
    }
}

impl SubtitleIndexStatus {
    #[inline]
    pub const fn fully_indexed_mask() -> Self {
        Self::from_bits_truncate(
            Self::TRACKS_DISCOVERED.bits()
                | Self::CUES_EXTRACTED.bits()
                | Self::OCR_DONE.bits()
                | Self::SEARCH_INDEXED.bits(),
        )
    }

    #[inline]
    pub fn is_fully_indexed(&self) -> bool {
        self.contains(Self::fully_indexed_mask())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_error_flags_bit_values() {
        assert_eq!(MediaErrorFlags::VIDEO_ERROR.bits(), 0x0001);
        assert_eq!(MediaErrorFlags::AUDIO_ERROR.bits(), 0x0002);
        assert_eq!(MediaErrorFlags::SUBTITLE_ERROR.bits(), 0x0004);
    }

    #[test]
    fn video_index_status_verified_vs_findit_proto() {
        // Exact values from findit-proto::database::video::VideoIndexStatus.
        assert_eq!(VideoIndexStatus::PROBED.bits(), 0x01);
        assert_eq!(VideoIndexStatus::SCENE_DETECTED.bits(), 0x02);
        assert_eq!(VideoIndexStatus::KEYFRAME_EXTRACTED.bits(), 0x04);
        // 0x08 is a reserved gap.
        assert_eq!(VideoIndexStatus::VLM_ANALYZED.bits(), 0x10);
        assert_eq!(VideoIndexStatus::APPLE_VISION_ANALYZED.bits(), 0x20);
        assert_eq!(VideoIndexStatus::TEXT_EMBEDDING_FINISHED.bits(), 0x40);
        assert_eq!(VideoIndexStatus::SCENE_EMBEDDING_FINISHED.bits(), 0x80);
        // Distinct stages (the schema-locked invariant):
        assert_ne!(
            VideoIndexStatus::VLM_ANALYZED,
            VideoIndexStatus::APPLE_VISION_ANALYZED
        );
        assert_ne!(
            VideoIndexStatus::TEXT_EMBEDDING_FINISHED,
            VideoIndexStatus::SCENE_EMBEDDING_FINISHED
        );
    }

    #[test]
    fn video_index_status_fully_indexed() {
        let mask = VideoIndexStatus::fully_indexed_mask();
        assert!(mask.is_fully_indexed());
        // Bit 0x08 is reserved — the mask MUST NOT include it.
        assert_eq!(mask.bits() & 0x08, 0);
        // Every named bit is in the mask.
        for bit in [
            VideoIndexStatus::PROBED,
            VideoIndexStatus::SCENE_DETECTED,
            VideoIndexStatus::KEYFRAME_EXTRACTED,
            VideoIndexStatus::VLM_ANALYZED,
            VideoIndexStatus::APPLE_VISION_ANALYZED,
            VideoIndexStatus::TEXT_EMBEDDING_FINISHED,
            VideoIndexStatus::SCENE_EMBEDDING_FINISHED,
        ] {
            assert!(mask.contains(bit), "fully_indexed_mask must contain {bit:?}");
        }
        // An empty status is not fully indexed.
        assert!(!VideoIndexStatus::empty().is_fully_indexed());
    }

    #[test]
    fn audio_index_status_verified_11_bit_processing_stage() {
        // The full 11-bit ProcessingStage from
        // findit-proto::database::audio::ProcessingStage.
        assert_eq!(AudioIndexStatus::EXTRACTED.bits(), 0x001);
        assert_eq!(AudioIndexStatus::CLASSIFIED.bits(), 0x002);
        assert_eq!(AudioIndexStatus::VAD_DONE.bits(), 0x004);
        assert_eq!(AudioIndexStatus::STT_DONE.bits(), 0x008);
        assert_eq!(AudioIndexStatus::SPEAKER_DONE.bits(), 0x010);
        assert_eq!(AudioIndexStatus::LLM_DONE.bits(), 0x020);
        assert_eq!(AudioIndexStatus::TEXT_EMBED.bits(), 0x040);
        assert_eq!(AudioIndexStatus::CED_DONE.bits(), 0x080);
        assert_eq!(AudioIndexStatus::CLAP_DONE.bits(), 0x100);
        assert_eq!(AudioIndexStatus::EBUR128_DONE.bits(), 0x200);
        assert_eq!(AudioIndexStatus::FPRINT_DONE.bits(), 0x400);
        assert_eq!(AudioIndexStatus::fully_indexed_mask().bits(), 0x7FF);
    }

    #[test]
    fn subtitle_index_status_verified_names_and_bits() {
        assert_eq!(SubtitleIndexStatus::TRACKS_DISCOVERED.bits(), 0x01);
        assert_eq!(SubtitleIndexStatus::CUES_EXTRACTED.bits(), 0x02);
        assert_eq!(SubtitleIndexStatus::OCR_DONE.bits(), 0x04);
        assert_eq!(SubtitleIndexStatus::SEARCH_INDEXED.bits(), 0x08);
        assert_eq!(SubtitleIndexStatus::fully_indexed_mask().bits(), 0x0F);
    }

    #[test]
    fn bitflags_default_is_empty() {
        assert_eq!(VideoIndexStatus::default(), VideoIndexStatus::empty());
        assert_eq!(AudioIndexStatus::default(), AudioIndexStatus::empty());
        assert_eq!(SubtitleIndexStatus::default(), SubtitleIndexStatus::empty());
        assert_eq!(MediaErrorFlags::default(), MediaErrorFlags::empty());
    }
}
