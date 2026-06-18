//! Audio subtree: facet → tracks → segments + sound events + speakers.
//! Standalone field owners — no embedded flat aggregates, no parent FKs,
//! no id-vecs.

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
    audio::{
      facet::AudioParts, segment::AudioSegmentParts, sound_event::SoundEventParts,
      track::AudioTrackParts,
    },
    speaker::SpeakerParts,
  },
  AudioContentKind, AudioIndexStatus, CedDetector, ErrorInfo, IndexProgress, LocalizedText,
  Provenance, Uuid7, VoiceFingerprint, Word,
};

/// The audio facet with its complete track subtrees.
#[derive(Debug, Clone, PartialEq)]
pub struct Audio<Id = Uuid7> {
  id: Id,
  total_segments: u32,
  total_sound_events: u32,
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
      total_sound_events,
      track_progress,
    } = facet.into_parts();
    parent_check(NodeKind::AudioFacet, id, &media_id, expected_media)?;
    Ok(Self {
      id,
      total_segments,
      total_sound_events,
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
  pub const fn total_sound_events(&self) -> u32 {
    self.total_sound_events
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
  sound_events: Vec<SoundEvent<Id>>,
  metadata: IndexMap<SmolStr, SmolStr>,
  provenance: Provenance,
  /// Provenance of the VAD (voice-activity) model that produced this
  /// track's `SpeechSegment`s — distinct from the general analysis
  /// `provenance`.
  vad_provenance: Provenance,
  index_status: AudioIndexStatus,
  index_errors: Vec<ErrorInfo>,
}

impl AudioTrack<Uuid7> {
  /// Lift the flat track; validates `audio_id == expected_audio` and
  /// lifts the flat segments/sound events/speakers against this track's id.
  pub fn try_from_flat(
    expected_audio: &Uuid7,
    track: domain::AudioTrack<Uuid7>,
    segments: Vec<domain::AudioSegment<Uuid7>>,
    sound_events: Vec<domain::SoundEvent<Uuid7>>,
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
      sound_events: _,
      metadata,
      provenance,
      vad_provenance,
      index_status,
      index_errors,
    } = track.into_parts();
    parent_check(NodeKind::AudioTrack, id, &audio_id, expected_audio)?;
    let segments = segments
      .into_iter()
      .map(|s| AudioSegment::try_from_flat(&id, s))
      .collect::<Result<Vec<_>, _>>()?;
    let sound_events = sound_events
      .into_iter()
      .map(|s| SoundEvent::try_from_flat(&id, s))
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
      sound_events,
      metadata,
      provenance,
      vad_provenance,
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

  /// The track's detected sound events.
  #[inline(always)]
  pub const fn sound_events_slice(&self) -> &[SoundEvent<Id>] {
    self.sound_events.as_slice()
  }

  #[inline(always)]
  pub const fn metadata_ref(&self) -> &IndexMap<SmolStr, SmolStr> {
    &self.metadata
  }

  #[inline(always)]
  pub const fn provenance_ref(&self) -> &Provenance {
    &self.provenance
  }

  /// Provenance of the VAD (voice-activity) model that produced this
  /// track's `SpeechSegment`s — distinct from the general analysis
  /// `provenance`.
  #[inline(always)]
  pub const fn vad_provenance_ref(&self) -> &Provenance {
    &self.vad_provenance
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

/// One detected sound event — every field of the flat `SoundEvent` except
/// `audio_track_id` (implied by nesting).
#[derive(Debug, Clone, PartialEq)]
pub struct SoundEvent<Id = Uuid7> {
  id: Id,
  index: u32,
  span: TimeRange,
  label: SmolStr,
  code: Option<u64>,
  score: f32,
  detector: CedDetector,
}

impl SoundEvent<Uuid7> {
  /// Lift the flat sound event; validates `audio_track_id == expected_track`.
  pub fn try_from_flat(
    expected_track: &Uuid7,
    sound_event: domain::SoundEvent<Uuid7>,
  ) -> Result<Self, GraphError> {
    let SoundEventParts {
      id,
      audio_track_id,
      index,
      span,
      label,
      code,
      score,
      detector,
    } = sound_event.into_parts();
    parent_check(NodeKind::SoundEvent, id, &audio_track_id, expected_track)?;
    Ok(Self {
      id,
      index,
      span,
      label,
      code,
      score,
      detector,
    })
  }
}

impl<Id> SoundEvent<Id> {
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

  /// CED class name (`""` = absent).
  #[inline(always)]
  pub fn label(&self) -> &str {
    self.label.as_str()
  }

  /// Stable soundevents dataset code (`None` = unmapped class).
  #[inline(always)]
  pub const fn code(&self) -> Option<u64> {
    self.code
  }

  /// `[0,1]` detection score (always finite).
  #[inline(always)]
  pub const fn score(&self) -> f32 {
    self.score
  }

  #[inline(always)]
  pub const fn detector(&self) -> CedDetector {
    self.detector
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
    let sound_event = domain::SoundEvent::try_new(
      Uuid7::new(),
      track_id,
      0,
      span(),
      "Speech",
      Some(0),
      0.9,
      crate::domain::CedDetector::Ced,
    )
    .expect("valid sound event");
    let speaker = domain::Speaker::try_new(Uuid7::new(), track_id, 0, "S1").expect("valid speaker");
    let node = AudioTrack::try_from_flat(
      &audio_id,
      track,
      vec![segment],
      vec![sound_event],
      vec![speaker],
    )
    .expect("coherent");
    assert_eq!(node.segments_slice().len(), 1);
    assert_eq!(node.sound_events_slice().len(), 1);
    assert_eq!(node.sound_events_slice()[0].label(), "Speech");
    assert_eq!(node.speakers_slice().len(), 1);
    assert_eq!(node.speakers_slice()[0].name(), "S1");
  }

  #[test]
  fn segment_under_wrong_track_is_rejected() {
    let audio_id = Uuid7::new();
    let track = domain::AudioTrack::try_new(Uuid7::new(), audio_id).expect("valid track");
    let segment =
      domain::AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span()).expect("valid segment");
    let err = AudioTrack::try_from_flat(&audio_id, track, vec![segment], vec![], vec![])
      .expect_err("incoherent");
    assert!(matches!(
      err,
      GraphError::ParentMismatch {
        kind: NodeKind::AudioSegment,
        ..
      }
    ));
  }

  #[test]
  fn sound_event_under_wrong_track_is_rejected_via_track_lift() {
    let audio_id = Uuid7::new();
    let track = domain::AudioTrack::try_new(Uuid7::new(), audio_id).expect("valid track");
    let sound_event = domain::SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      "Speech",
      None,
      0.9,
      crate::domain::CedDetector::Ced,
    )
    .expect("valid sound event");
    let err = AudioTrack::try_from_flat(&audio_id, track, vec![], vec![sound_event], vec![])
      .expect_err("incoherent");
    assert!(matches!(
      err,
      GraphError::ParentMismatch {
        kind: NodeKind::SoundEvent,
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
    let err = AudioTrack::try_from_flat(&audio_id, track, vec![], vec![], vec![speaker])
      .expect_err("incoherent");
    assert!(matches!(
      err,
      GraphError::ParentMismatch {
        kind: NodeKind::Speaker,
        ..
      }
    ));
  }

  #[test]
  fn sound_event_lifts_and_drops_fk() {
    let track_id = Uuid7::new();
    let flat = domain::SoundEvent::try_new(
      Uuid7::new(),
      track_id,
      0,
      span(),
      "Speech",
      Some(0),
      0.9,
      crate::domain::CedDetector::Ced,
    )
    .expect("valid sound event");
    let event_id = *flat.id_ref();
    let node = SoundEvent::try_from_flat(&track_id, flat).expect("coherent");
    assert_eq!(node.id_ref(), &event_id);
    assert_eq!(node.label(), "Speech");
    assert_eq!(node.code(), Some(0));
    assert!(node.detector().is_ced());
  }

  #[test]
  fn sound_event_under_wrong_track_is_rejected() {
    let track_id = Uuid7::new();
    let flat = domain::SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      "Speech",
      None,
      0.9,
      crate::domain::CedDetector::Ced,
    )
    .expect("valid sound event");
    let err = SoundEvent::try_from_flat(&track_id, flat).expect_err("incoherent");
    assert!(matches!(
      err,
      GraphError::ParentMismatch {
        kind: NodeKind::SoundEvent,
        ..
      }
    ));
  }
}

// --- conversion traits: flat ⇄ graph ---------------------------------------

/// Trait form of [`Audio::try_from_flat`] — `(expected_media, facet, tracks)`.
impl TryFrom<(Uuid7, domain::Audio<Uuid7>, Vec<AudioTrack<Uuid7>>)> for Audio<Uuid7> {
  type Error = GraphError;

  #[inline(always)]
  fn try_from(
    (expected_media, facet, tracks): (Uuid7, domain::Audio<Uuid7>, Vec<AudioTrack<Uuid7>>),
  ) -> Result<Self, Self::Error> {
    Self::try_from_flat(&expected_media, facet, tracks)
  }
}

/// Re-attach to `media_id` and rebuild the flat facet; the track-id vec
/// is re-derived from the embedded tracks, which are then dropped —
/// convert them first when persisting the tree.
impl From<(Uuid7, Audio<Uuid7>)> for domain::Audio<Uuid7> {
  fn from((media_id, g): (Uuid7, Audio<Uuid7>)) -> Self {
    let Audio {
      id,
      total_segments,
      total_sound_events,
      track_progress,
      tracks,
    } = g;
    domain::Audio::rehydrate(AudioParts {
      id,
      media_id,
      tracks: tracks.iter().map(|t| *t.id_ref()).collect(),
      total_segments,
      total_sound_events,
      track_progress,
    })
  }
}

/// Trait form of [`AudioTrack::try_from_flat`] —
/// `(expected_audio, track, segments, sound_events, speakers)`.
impl
  TryFrom<(
    Uuid7,
    domain::AudioTrack<Uuid7>,
    Vec<domain::AudioSegment<Uuid7>>,
    Vec<domain::SoundEvent<Uuid7>>,
    Vec<domain::Speaker<Uuid7>>,
  )> for AudioTrack<Uuid7>
{
  type Error = GraphError;

  #[inline(always)]
  fn try_from(
    (expected_audio, track, segments, sound_events, speakers): (
      Uuid7,
      domain::AudioTrack<Uuid7>,
      Vec<domain::AudioSegment<Uuid7>>,
      Vec<domain::SoundEvent<Uuid7>>,
      Vec<domain::Speaker<Uuid7>>,
    ),
  ) -> Result<Self, Self::Error> {
    Self::try_from_flat(&expected_audio, track, segments, sound_events, speakers)
  }
}

/// Re-attach to `audio_id` and rebuild the flat track; the segment- and
/// speaker-id vecs are re-derived from the embedded children, which are
/// then dropped — convert them first when persisting the tree.
impl From<(Uuid7, AudioTrack<Uuid7>)> for domain::AudioTrack<Uuid7> {
  fn from((audio_id, g): (Uuid7, AudioTrack<Uuid7>)) -> Self {
    let AudioTrack {
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
      sound_events,
      metadata,
      provenance,
      vad_provenance,
      index_status,
      index_errors,
    } = g;
    domain::AudioTrack::rehydrate(AudioTrackParts {
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
      speakers: speakers.iter().map(|s| *s.id_ref()).collect(),
      tags,
      cover_art,
      segments: segments.iter().map(|s| *s.id_ref()).collect(),
      sound_events: sound_events.iter().map(|s| *s.id_ref()).collect(),
      metadata,
      provenance,
      vad_provenance,
      index_status,
      index_errors,
    })
  }
}

/// Trait form of [`AudioSegment::try_from_flat`] — `(expected_track, flat)`.
impl TryFrom<(Uuid7, domain::AudioSegment<Uuid7>)> for AudioSegment<Uuid7> {
  type Error = GraphError;

  #[inline(always)]
  fn try_from(
    (expected_track, segment): (Uuid7, domain::AudioSegment<Uuid7>),
  ) -> Result<Self, Self::Error> {
    Self::try_from_flat(&expected_track, segment)
  }
}

/// Re-attach to `audio_track_id` and rebuild the flat segment.
impl From<(Uuid7, AudioSegment<Uuid7>)> for domain::AudioSegment<Uuid7> {
  fn from((audio_track_id, g): (Uuid7, AudioSegment<Uuid7>)) -> Self {
    let AudioSegment {
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
    } = g;
    domain::AudioSegment::rehydrate(AudioSegmentParts {
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
    })
  }
}

/// Trait form of [`SoundEvent::try_from_flat`] — `(expected_track, flat)`.
impl TryFrom<(Uuid7, domain::SoundEvent<Uuid7>)> for SoundEvent<Uuid7> {
  type Error = GraphError;

  #[inline(always)]
  fn try_from(
    (expected_track, sound_event): (Uuid7, domain::SoundEvent<Uuid7>),
  ) -> Result<Self, Self::Error> {
    Self::try_from_flat(&expected_track, sound_event)
  }
}

/// Re-attach to `audio_track_id` and rebuild the flat sound event.
impl From<(Uuid7, SoundEvent<Uuid7>)> for domain::SoundEvent<Uuid7> {
  fn from((audio_track_id, g): (Uuid7, SoundEvent<Uuid7>)) -> Self {
    let SoundEvent {
      id,
      index,
      span,
      label,
      code,
      score,
      detector,
    } = g;
    domain::SoundEvent::rehydrate(SoundEventParts {
      id,
      audio_track_id,
      index,
      span,
      label,
      code,
      score,
      detector,
    })
  }
}

/// Trait form of [`Speaker::try_from_flat`] — `(expected_track, flat)`.
impl TryFrom<(Uuid7, domain::Speaker<Uuid7>)> for Speaker<Uuid7> {
  type Error = GraphError;

  #[inline(always)]
  fn try_from(
    (expected_track, speaker): (Uuid7, domain::Speaker<Uuid7>),
  ) -> Result<Self, Self::Error> {
    Self::try_from_flat(&expected_track, speaker)
  }
}

/// Re-attach to `audio_track_id` and rebuild the flat speaker.
impl From<(Uuid7, Speaker<Uuid7>)> for domain::Speaker<Uuid7> {
  fn from((audio_track_id, g): (Uuid7, Speaker<Uuid7>)) -> Self {
    let Speaker {
      id,
      cluster_id,
      name,
      speech_duration,
      voiceprint,
      person_id,
    } = g;
    domain::Speaker::rehydrate(SpeakerParts {
      id,
      audio_track_id,
      cluster_id,
      name,
      speech_duration,
      voiceprint,
      person_id,
    })
  }
}

#[cfg(test)]
mod conv_tests {
  use core::num::NonZeroU32;

  use mediatime::Timebase;

  use super::*;

  #[test]
  fn segment_round_trips_through_graph() {
    let track_id = Uuid7::new();
    let tb = Timebase::new(1, NonZeroU32::new(1000).unwrap());
    let flat =
      domain::AudioSegment::try_new(Uuid7::new(), track_id, 0, TimeRange::new(0, 1000, tb))
        .expect("valid segment");
    let lifted: AudioSegment<Uuid7> = (track_id, flat.clone()).try_into().expect("coherent");
    let back: domain::AudioSegment<Uuid7> = (track_id, lifted).into();
    assert_eq!(back, flat);
  }

  #[test]
  fn sound_event_round_trips_through_graph() {
    let track_id = Uuid7::new();
    let tb = Timebase::new(1, NonZeroU32::new(1000).unwrap());
    let flat = domain::SoundEvent::try_new(
      Uuid7::new(),
      track_id,
      1,
      TimeRange::new(0, 1000, tb),
      "Music",
      Some(137),
      0.75,
      crate::domain::CedDetector::Ced,
    )
    .expect("valid sound event");
    let lifted: SoundEvent<Uuid7> = (track_id, flat.clone()).try_into().expect("coherent");
    let back: domain::SoundEvent<Uuid7> = (track_id, lifted).into();
    assert_eq!(back, flat);
  }
}
