//! GraphQL object wrappers for the domain bitflag companions.
//!
//! Each wrapper exposes two fields:
//! - `bits`: the raw flag word (Int), suitable for round-trip storage.
//! - `flags`: a `[String]` of named flags currently set, for human use.
//!
//! The wrapper carries the underlying domain value by copy. `From`
//! conversions are infallible both ways.

use async_graphql::Object;

use mediaframe::disposition::TrackDisposition;

use crate::domain::{AudioIndexStatus, MediaErrorFlags, SubtitleIndexStatus, VideoIndexStatus};

/// Named flags currently set on a [`TrackDisposition`] (the shared
/// `mediaframe` FFmpeg `AV_DISPOSITION_*` bitflags). Shared by the
/// video / audio / subtitle track resolvers so each can expose the
/// disposition both as its raw `u32` and as a human-readable flag list.
pub(crate) fn disposition_flag_names(d: TrackDisposition) -> std::vec::Vec<String> {
  d.iter_names().map(|(name, _)| name.to_string()).collect()
}

// ---------------------------------------------------------------------------
// MediaErrorFlags
// ---------------------------------------------------------------------------

/// GraphQL wrapper around [`MediaErrorFlags`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GqlMediaErrorFlags(MediaErrorFlags);

impl From<MediaErrorFlags> for GqlMediaErrorFlags {
  #[inline]
  fn from(v: MediaErrorFlags) -> Self {
    Self(v)
  }
}

impl From<GqlMediaErrorFlags> for MediaErrorFlags {
  #[inline]
  fn from(v: GqlMediaErrorFlags) -> Self {
    v.0
  }
}

#[Object(name = "MediaErrorFlags")]
impl GqlMediaErrorFlags {
  /// Raw flag word — `u16` widened to `i32` for GraphQL's `Int`.
  async fn bits(&self) -> i32 {
    i32::from(self.0.bits())
  }

  /// Named flag set, sorted by bit value.
  async fn flags(&self) -> std::vec::Vec<String> {
    let mut out = std::vec::Vec::new();
    if self.0.contains(MediaErrorFlags::VIDEO_ERROR) {
      out.push("VIDEO_ERROR".into());
    }
    if self.0.contains(MediaErrorFlags::AUDIO_ERROR) {
      out.push("AUDIO_ERROR".into());
    }
    if self.0.contains(MediaErrorFlags::SUBTITLE_ERROR) {
      out.push("SUBTITLE_ERROR".into());
    }
    out
  }
}

// ---------------------------------------------------------------------------
// VideoIndexStatus
// ---------------------------------------------------------------------------

/// GraphQL wrapper around [`VideoIndexStatus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GqlVideoIndexStatus(VideoIndexStatus);

impl From<VideoIndexStatus> for GqlVideoIndexStatus {
  #[inline]
  fn from(v: VideoIndexStatus) -> Self {
    Self(v)
  }
}

impl From<GqlVideoIndexStatus> for VideoIndexStatus {
  #[inline]
  fn from(v: GqlVideoIndexStatus) -> Self {
    v.0
  }
}

#[Object(name = "VideoIndexStatus")]
impl GqlVideoIndexStatus {
  /// Raw flag word.
  async fn bits(&self) -> u32 {
    self.0.bits()
  }

  /// Named flag set, sorted by bit value.
  async fn flags(&self) -> std::vec::Vec<String> {
    let mut out = std::vec::Vec::new();
    if self.0.contains(VideoIndexStatus::PROBED) {
      out.push("PROBED".into());
    }
    if self.0.contains(VideoIndexStatus::SCENE_DETECTED) {
      out.push("SCENE_DETECTED".into());
    }
    if self.0.contains(VideoIndexStatus::KEYFRAME_EXTRACTED) {
      out.push("KEYFRAME_EXTRACTED".into());
    }
    if self.0.contains(VideoIndexStatus::VLM_ANALYZED) {
      out.push("VLM_ANALYZED".into());
    }
    if self.0.contains(VideoIndexStatus::APPLE_VISION_ANALYZED) {
      out.push("APPLE_VISION_ANALYZED".into());
    }
    if self.0.contains(VideoIndexStatus::TEXT_EMBEDDING_FINISHED) {
      out.push("TEXT_EMBEDDING_FINISHED".into());
    }
    if self.0.contains(VideoIndexStatus::SCENE_EMBEDDING_FINISHED) {
      out.push("SCENE_EMBEDDING_FINISHED".into());
    }
    out
  }

  /// `true` iff every stage bit in `VideoIndexStatus::fully_indexed_mask()` is set.
  async fn is_fully_indexed(&self) -> bool {
    self.0.is_fully_indexed()
  }
}

// ---------------------------------------------------------------------------
// AudioIndexStatus
// ---------------------------------------------------------------------------

/// GraphQL wrapper around [`AudioIndexStatus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GqlAudioIndexStatus(AudioIndexStatus);

impl From<AudioIndexStatus> for GqlAudioIndexStatus {
  #[inline]
  fn from(v: AudioIndexStatus) -> Self {
    Self(v)
  }
}

impl From<GqlAudioIndexStatus> for AudioIndexStatus {
  #[inline]
  fn from(v: GqlAudioIndexStatus) -> Self {
    v.0
  }
}

#[Object(name = "AudioIndexStatus")]
impl GqlAudioIndexStatus {
  async fn bits(&self) -> u32 {
    self.0.bits()
  }

  async fn flags(&self) -> std::vec::Vec<String> {
    let mut out = std::vec::Vec::new();
    if self.0.contains(AudioIndexStatus::EXTRACTED) {
      out.push("EXTRACTED".into());
    }
    if self.0.contains(AudioIndexStatus::CLASSIFIED) {
      out.push("CLASSIFIED".into());
    }
    if self.0.contains(AudioIndexStatus::VAD_DONE) {
      out.push("VAD_DONE".into());
    }
    if self.0.contains(AudioIndexStatus::STT_DONE) {
      out.push("STT_DONE".into());
    }
    if self.0.contains(AudioIndexStatus::SPEAKER_DONE) {
      out.push("SPEAKER_DONE".into());
    }
    if self.0.contains(AudioIndexStatus::LLM_DONE) {
      out.push("LLM_DONE".into());
    }
    if self.0.contains(AudioIndexStatus::TEXT_EMBED) {
      out.push("TEXT_EMBED".into());
    }
    if self.0.contains(AudioIndexStatus::CED_DONE) {
      out.push("CED_DONE".into());
    }
    if self.0.contains(AudioIndexStatus::CLAP_DONE) {
      out.push("CLAP_DONE".into());
    }
    if self.0.contains(AudioIndexStatus::EBUR128_DONE) {
      out.push("EBUR128_DONE".into());
    }
    if self.0.contains(AudioIndexStatus::FPRINT_DONE) {
      out.push("FPRINT_DONE".into());
    }
    out
  }

  async fn is_fully_indexed(&self) -> bool {
    self.0.is_fully_indexed()
  }
}

// ---------------------------------------------------------------------------
// SubtitleIndexStatus
// ---------------------------------------------------------------------------

/// GraphQL wrapper around [`SubtitleIndexStatus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GqlSubtitleIndexStatus(SubtitleIndexStatus);

impl From<SubtitleIndexStatus> for GqlSubtitleIndexStatus {
  #[inline]
  fn from(v: SubtitleIndexStatus) -> Self {
    Self(v)
  }
}

impl From<GqlSubtitleIndexStatus> for SubtitleIndexStatus {
  #[inline]
  fn from(v: GqlSubtitleIndexStatus) -> Self {
    v.0
  }
}

#[Object(name = "SubtitleIndexStatus")]
impl GqlSubtitleIndexStatus {
  async fn bits(&self) -> u32 {
    self.0.bits()
  }

  async fn flags(&self) -> std::vec::Vec<String> {
    let mut out = std::vec::Vec::new();
    if self.0.contains(SubtitleIndexStatus::TRACKS_DISCOVERED) {
      out.push("TRACKS_DISCOVERED".into());
    }
    if self.0.contains(SubtitleIndexStatus::CUES_EXTRACTED) {
      out.push("CUES_EXTRACTED".into());
    }
    if self.0.contains(SubtitleIndexStatus::OCR_DONE) {
      out.push("OCR_DONE".into());
    }
    if self.0.contains(SubtitleIndexStatus::SEARCH_INDEXED) {
      out.push("SEARCH_INDEXED".into());
    }
    out
  }

  /// `true` iff every stage bit in
  /// `SubtitleIndexStatus::fully_indexed_mask(requires_ocr)` is set.
  /// Pass `requires_ocr = true` for image-based codecs (PGS/DVBSUB).
  async fn is_fully_indexed(&self, requires_ocr: bool) -> bool {
    self.0.is_fully_indexed(requires_ocr)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn media_error_flags_roundtrips() {
    let v = MediaErrorFlags::VIDEO_ERROR | MediaErrorFlags::AUDIO_ERROR;
    let g: GqlMediaErrorFlags = v.into();
    let back: MediaErrorFlags = g.into();
    assert_eq!(back, v);
  }

  #[test]
  fn video_index_status_roundtrips() {
    let v = VideoIndexStatus::PROBED | VideoIndexStatus::SCENE_DETECTED;
    let g: GqlVideoIndexStatus = v.into();
    let back: VideoIndexStatus = g.into();
    assert_eq!(back, v);
  }

  #[test]
  fn audio_index_status_roundtrips() {
    let v = AudioIndexStatus::EXTRACTED | AudioIndexStatus::STT_DONE | AudioIndexStatus::TEXT_EMBED;
    let g: GqlAudioIndexStatus = v.into();
    let back: AudioIndexStatus = g.into();
    assert_eq!(back, v);
  }

  #[test]
  fn subtitle_index_status_roundtrips() {
    let v = SubtitleIndexStatus::TRACKS_DISCOVERED | SubtitleIndexStatus::SEARCH_INDEXED;
    let g: GqlSubtitleIndexStatus = v.into();
    let back: SubtitleIndexStatus = g.into();
    assert_eq!(back, v);
  }
}
