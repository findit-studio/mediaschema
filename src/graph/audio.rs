//! Audio subtree: facet → tracks → segments + speakers. Standalone field
//! owners — no embedded flat aggregates, no parent FKs, no id-vecs.

use indexmap::IndexMap;
use mediaframe::{
  audio::{
    BitRateMode, ChannelLayout, CoverArt, Fingerprint, Loudness, ReplayGain, SampleFormat, Tags,
  },
  codec::AudioCodec,
  disposition::TrackDisposition,
  lang::Language,
};
use mediatime::{TimeRange, Timestamp};
use smol_str::SmolStr;

use super::{parent_check, GraphError, NodeKind};
use crate::domain::{
  self,
  aggregates::{
    audio::{facet::AudioParts, segment::AudioSegmentParts, track::AudioTrackParts},
    speaker::SpeakerParts,
  },
  AudioContentKind, AudioIndexStatus, ErrorInfo, IndexProgress, LocalizedText, Provenance, Uuid7,
  VoiceFingerprint, Word,
};

/// The audio facet with its complete track subtrees.
#[derive(Debug, Clone, PartialEq)]
pub struct Audio<Id = Uuid7> {
  id: Id,
  total_segments: u32,
  track_progress: IndexProgress,
  tracks: Vec<AudioTrack<Id>>,
}

impl Audio<Uuid7> {
  /// Lift the flat facet; validates `media_id == expected_media`. Tracks
  /// arrive pre-lifted (their `audio_id` was consumed by their lift).
  pub fn try_from_flat(
    expected_media: &Uuid7,
    facet: domain::Audio<Uuid7>,
    tracks: Vec<AudioTrack<Uuid7>>,
  ) -> Result<Self, GraphError> {
    let AudioParts {
      id,
      media_id,
      tracks: _,
      total_segments,
      track_progress,
    } = facet.into_parts();
    parent_check(NodeKind::AudioFacet, id, &media_id, expected_media)?;
    Ok(Self {
      id,
      total_segments,
      track_progress,
      tracks,
    })
  }
}

impl<Id> Audio<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  #[inline(always)]
  pub const fn total_segments(&self) -> u32 {
    self.total_segments
  }

  #[inline(always)]
  pub const fn track_progress_ref(&self) -> &IndexProgress {
    &self.track_progress
  }

  /// The track subtrees, in container stream order.
  #[inline(always)]
  pub const fn tracks_slice(&self) -> &[AudioTrack<Id>] {
    self.tracks.as_slice()
  }
}

/// One audio track — every field of the flat `AudioTrack` except
/// `audio_id` and the segment/speaker id-vecs, plus the children
/// themselves.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioTrack<Id = Uuid7> {
  id: Id,
  stream_index: Option<u32>,
  container_track_id: Option<u64>,
  codec: AudioCodec,
  profile: SmolStr,
  sample_rate: u32,
  channels: u16,
  channel_layout: ChannelLayout,
  sample_format: SampleFormat,
  bit_rate: u64,
  bit_rate_mode: Option<BitRateMode>,
  bits_per_sample: Option<u16>,
  is_lossless: bool,
  duration: Option<Timestamp>,
  start_pts: Option<Timestamp>,
  language: Option<Language>,
  detected_language: Option<Language>,
  disposition: TrackDisposition,
  is_primary: bool,
  auto_selected: bool,
  content: Option<AudioContentKind>,
  speech_ratio: Option<f32>,
  is_silent: bool,
  loudness: Option<Loudness>,
  replay_gain: Option<ReplayGain>,
  fingerprint: Option<Fingerprint>,
  isrc: SmolStr,
  acoustid: SmolStr,
  musicbrainz_recording_id: SmolStr,
  speakers: Vec<Speaker<Id>>,
  tags: Option<Tags>,
  cover_art: Option<CoverArt>,
  segments: Vec<AudioSegment<Id>>,
  metadata: IndexMap<SmolStr, SmolStr>,
  provenance: Provenance,
  index_status: AudioIndexStatus,
  index_errors: Vec<ErrorInfo>,
}

impl AudioTrack<Uuid7> {
  /// Lift the flat track; validates `audio_id == expected_audio` and
  /// lifts the flat segments/speakers against this track's id.
  pub fn try_from_flat(
    expected_audio: &Uuid7,
    track: domain::AudioTrack<Uuid7>,
    segments: Vec<domain::AudioSegment<Uuid7>>,
    speakers: Vec<domain::Speaker<Uuid7>>,
  ) -> Result<Self, GraphError> {
    let AudioTrackParts {
      id,
      audio_id,
      stream_index,
      container_track_id,
      codec,
      profile,
      sample_rate,
      channels,
      channel_layout,
      sample_format,
      bit_rate,
      bit_rate_mode,
      bits_per_sample,
      is_lossless,
      duration,
      start_pts,
      language,
      detected_language,
      disposition,
      is_primary,
      auto_selected,
      content,
      speech_ratio,
      is_silent,
      loudness,
      replay_gain,
      fingerprint,
      isrc,
      acoustid,
      musicbrainz_recording_id,
      speakers: _,
      tags,
      cover_art,
      segments: _,
      metadata,
      provenance,
      index_status,
      index_errors,
    } = track.into_parts();
    parent_check(NodeKind::AudioTrack, id, &audio_id, expected_audio)?;
    let segments = segments
      .into_iter()
      .map(|s| AudioSegment::try_from_flat(&id, s))
      .collect::<Result<Vec<_>, _>>()?;
    let speakers = speakers
      .into_iter()
      .map(|s| Speaker::try_from_flat(&id, s))
      .collect::<Result<Vec<_>, _>>()?;
    Ok(Self {
      id,
      stream_index,
      container_track_id,
      codec,
      profile,
      sample_rate,
      channels,
      channel_layout,
      sample_format,
      bit_rate,
      bit_rate_mode,
      bits_per_sample,
      is_lossless,
      duration,
      start_pts,
      language,
      detected_language,
      disposition,
      is_primary,
      auto_selected,
      content,
      speech_ratio,
      is_silent,
      loudness,
      replay_gain,
      fingerprint,
      isrc,
      acoustid,
      musicbrainz_recording_id,
      speakers,
      tags,
      cover_art,
      segments,
      metadata,
      provenance,
      index_status,
      index_errors,
    })
  }
}

impl<Id> AudioTrack<Id> {
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
  pub const fn codec_ref(&self) -> &AudioCodec {
    &self.codec
  }

  /// Codec profile name (`""` = absent).
  #[inline(always)]
  pub fn profile(&self) -> &str {
    self.profile.as_str()
  }

  #[inline(always)]
  pub const fn sample_rate(&self) -> u32 {
    self.sample_rate
  }

  #[inline(always)]
  pub const fn channels(&self) -> u16 {
    self.channels
  }

  #[inline(always)]
  pub const fn channel_layout_ref(&self) -> &ChannelLayout {
    &self.channel_layout
  }

  #[inline(always)]
  pub const fn sample_format_ref(&self) -> &SampleFormat {
    &self.sample_format
  }

  #[inline(always)]
  pub const fn bit_rate(&self) -> u64 {
    self.bit_rate
  }

  #[inline(always)]
  pub const fn bit_rate_mode(&self) -> Option<BitRateMode> {
    self.bit_rate_mode
  }

  #[inline(always)]
  pub const fn bits_per_sample(&self) -> Option<u16> {
    self.bits_per_sample
  }

  #[inline(always)]
  pub const fn is_lossless(&self) -> bool {
    self.is_lossless
  }

  #[inline(always)]
  pub const fn duration_ref(&self) -> Option<&Timestamp> {
    self.duration.as_ref()
  }

  #[inline(always)]
  pub const fn start_pts_ref(&self) -> Option<&Timestamp> {
    self.start_pts.as_ref()
  }

  #[inline(always)]
  pub const fn language(&self) -> Option<Language> {
    self.language
  }

  #[inline(always)]
  pub const fn detected_language(&self) -> Option<Language> {
    self.detected_language
  }

  /// Declared and detected language disagree (both known).
  #[inline(always)]
  pub fn language_mismatch(&self) -> bool {
    matches!((self.language, self.detected_language), (Some(a), Some(b)) if a != b)
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
  pub const fn content(&self) -> Option<AudioContentKind> {
    self.content
  }

  #[inline(always)]
  pub const fn speech_ratio(&self) -> Option<f32> {
    self.speech_ratio
  }

  #[inline(always)]
  pub const fn is_silent(&self) -> bool {
    self.is_silent
  }

  #[inline(always)]
  pub const fn loudness_ref(&self) -> Option<&Loudness> {
    self.loudness.as_ref()
  }

  #[inline(always)]
  pub const fn replay_gain_ref(&self) -> Option<&ReplayGain> {
    self.replay_gain.as_ref()
  }

  #[inline(always)]
  pub const fn fingerprint_ref(&self) -> Option<&Fingerprint> {
    self.fingerprint.as_ref()
  }

  /// ISRC code (`""` = absent).
  #[inline(always)]
  pub fn isrc(&self) -> &str {
    self.isrc.as_str()
  }

  /// AcoustID (`""` = absent).
  #[inline(always)]
  pub fn acoustid(&self) -> &str {
    self.acoustid.as_str()
  }

  /// MusicBrainz recording id (`""` = absent).
  #[inline(always)]
  pub fn musicbrainz_recording_id(&self) -> &str {
    self.musicbrainz_recording_id.as_str()
  }

  /// The track's diarized speakers.
  #[inline(always)]
  pub const fn speakers_slice(&self) -> &[Speaker<Id>] {
    self.speakers.as_slice()
  }

  #[inline(always)]
  pub const fn tags_ref(&self) -> Option<&Tags> {
    self.tags.as_ref()
  }

  #[inline(always)]
  pub const fn cover_art_ref(&self) -> Option<&CoverArt> {
    self.cover_art.as_ref()
  }

  /// The track's transcript/diarization segments.
  #[inline(always)]
  pub const fn segments_slice(&self) -> &[AudioSegment<Id>] {
    self.segments.as_slice()
  }

  #[inline(always)]
  pub const fn metadata_ref(&self) -> &IndexMap<SmolStr, SmolStr> {
    &self.metadata
  }

  #[inline(always)]
  pub const fn provenance_ref(&self) -> &Provenance {
    &self.provenance
  }

  #[inline(always)]
  pub const fn index_status(&self) -> AudioIndexStatus {
    self.index_status
  }

  #[inline(always)]
  pub const fn index_errors_slice(&self) -> &[ErrorInfo] {
    self.index_errors.as_slice()
  }
}

/// One transcript/diarization segment — every field of the flat
/// `AudioSegment` except `audio_track_id` (implied by nesting).
/// `speaker_id` stays: it is a cross-tree association into the sibling
/// speaker set, not a containment edge.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioSegment<Id = Uuid7> {
  id: Id,
  index: u32,
  span: TimeRange,
  speaker_id: Option<Id>,
  text: LocalizedText,
  language: Option<Language>,
  words: Vec<Word>,
  no_speech_prob: Option<f32>,
  avg_logprob: Option<f32>,
  temperature: Option<f32>,
  voice_fingerprint: Option<VoiceFingerprint<Id>>,
}

impl AudioSegment<Uuid7> {
  /// Lift the flat segment; validates `audio_track_id == expected_track`.
  pub fn try_from_flat(
    expected_track: &Uuid7,
    segment: domain::AudioSegment<Uuid7>,
  ) -> Result<Self, GraphError> {
    let AudioSegmentParts {
      id,
      audio_track_id,
      index,
      span,
      speaker_id,
      text,
      language,
      words,
      no_speech_prob,
      avg_logprob,
      temperature,
      voice_fingerprint,
    } = segment.into_parts();
    parent_check(NodeKind::AudioSegment, id, &audio_track_id, expected_track)?;
    Ok(Self {
      id,
      index,
      span,
      speaker_id,
      text,
      language,
      words,
      no_speech_prob,
      avg_logprob,
      temperature,
      voice_fingerprint,
    })
  }
}

impl<Id> AudioSegment<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  #[inline(always)]
  pub const fn index(&self) -> u32 {
    self.index
  }

  #[inline(always)]
  pub const fn span_ref(&self) -> &TimeRange {
    &self.span
  }

  /// Cross-tree association → the diarized speaker, when attributed.
  #[inline(always)]
  pub const fn speaker_id_ref(&self) -> Option<&Id> {
    self.speaker_id.as_ref()
  }

  #[inline(always)]
  pub const fn text_ref(&self) -> &LocalizedText {
    &self.text
  }

  #[inline(always)]
  pub const fn language(&self) -> Option<Language> {
    self.language
  }

  #[inline(always)]
  pub const fn words_slice(&self) -> &[Word] {
    self.words.as_slice()
  }

  #[inline(always)]
  pub const fn no_speech_prob(&self) -> Option<f32> {
    self.no_speech_prob
  }

  #[inline(always)]
  pub const fn avg_logprob(&self) -> Option<f32> {
    self.avg_logprob
  }

  #[inline(always)]
  pub const fn temperature(&self) -> Option<f32> {
    self.temperature
  }

  #[inline(always)]
  pub const fn voice_fingerprint_ref(&self) -> Option<&VoiceFingerprint<Id>> {
    self.voice_fingerprint.as_ref()
  }
}

/// One diarized speaker — every field of the flat `Speaker` except
/// `audio_track_id` (implied by nesting). `person_id` stays: it is a
/// cross-tree association to the cross-file `Person` aggregate.
#[derive(Debug, Clone, PartialEq)]
pub struct Speaker<Id = Uuid7> {
  id: Id,
  cluster_id: u32,
  name: SmolStr,
  speech_duration: Option<Timestamp>,
  voiceprint: Option<VoiceFingerprint<Id>>,
  person_id: Option<Id>,
}

impl Speaker<Uuid7> {
  /// Lift the flat speaker; validates `audio_track_id == expected_track`.
  pub fn try_from_flat(
    expected_track: &Uuid7,
    speaker: domain::Speaker<Uuid7>,
  ) -> Result<Self, GraphError> {
    let SpeakerParts {
      id,
      audio_track_id,
      cluster_id,
      name,
      speech_duration,
      voiceprint,
      person_id,
    } = speaker.into_parts();
    parent_check(NodeKind::Speaker, id, &audio_track_id, expected_track)?;
    Ok(Self {
      id,
      cluster_id,
      name,
      speech_duration,
      voiceprint,
      person_id,
    })
  }
}

impl<Id> Speaker<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  #[inline(always)]
  pub const fn cluster_id(&self) -> u32 {
    self.cluster_id
  }

  /// Display name (`""` = unnamed cluster).
  #[inline(always)]
  pub fn name(&self) -> &str {
    self.name.as_str()
  }

  #[inline(always)]
  pub const fn speech_duration_ref(&self) -> Option<&Timestamp> {
    self.speech_duration.as_ref()
  }

  #[inline(always)]
  pub const fn voiceprint_ref(&self) -> Option<&VoiceFingerprint<Id>> {
    self.voiceprint.as_ref()
  }

  /// Cross-tree association → the cross-file `Person`, when identified.
  #[inline(always)]
  pub const fn person_id_ref(&self) -> Option<&Id> {
    self.person_id.as_ref()
  }
}

#[cfg(test)]
mod tests {
  use core::num::NonZeroU32;

  use mediatime::{TimeRange, Timebase};

  use super::*;

  fn span() -> TimeRange {
    TimeRange::new(0, 1000, Timebase::new(1, NonZeroU32::new(1000).unwrap()))
  }

  #[test]
  fn coherent_audio_subtree_lifts() {
    let audio_id = Uuid7::new();
    let track = domain::AudioTrack::try_new(Uuid7::new(), audio_id).expect("valid track");
    let track_id = *track.id_ref();
    let segment =
      domain::AudioSegment::try_new(Uuid7::new(), track_id, 0, span()).expect("valid segment");
    let speaker = domain::Speaker::try_new(Uuid7::new(), track_id, 0, "S1").expect("valid speaker");
    let node =
      AudioTrack::try_from_flat(&audio_id, track, vec![segment], vec![speaker]).expect("coherent");
    assert_eq!(node.segments_slice().len(), 1);
    assert_eq!(node.speakers_slice().len(), 1);
    assert_eq!(node.speakers_slice()[0].name(), "S1");
  }

  #[test]
  fn segment_under_wrong_track_is_rejected() {
    let audio_id = Uuid7::new();
    let track = domain::AudioTrack::try_new(Uuid7::new(), audio_id).expect("valid track");
    let segment =
      domain::AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span()).expect("valid segment");
    let err =
      AudioTrack::try_from_flat(&audio_id, track, vec![segment], vec![]).expect_err("incoherent");
    assert!(matches!(
      err,
      GraphError::ParentMismatch {
        kind: NodeKind::AudioSegment,
        ..
      }
    ));
  }

  #[test]
  fn speaker_under_wrong_track_is_rejected() {
    let audio_id = Uuid7::new();
    let track = domain::AudioTrack::try_new(Uuid7::new(), audio_id).expect("valid track");
    let speaker =
      domain::Speaker::try_new(Uuid7::new(), Uuid7::new(), 0, "S1").expect("valid speaker");
    let err =
      AudioTrack::try_from_flat(&audio_id, track, vec![], vec![speaker]).expect_err("incoherent");
    assert!(matches!(
      err,
      GraphError::ParentMismatch {
        kind: NodeKind::Speaker,
        ..
      }
    ));
  }
}
