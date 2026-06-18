//! Wire ⇄ graph conversions for the audio subtree: `media.v2::Audio` ⇄
//! [`graph::Audio`], `media.v2::AudioTrack` ⇄ [`graph::AudioTrack`],
//! `media.v2::AudioSegment` ⇄ [`graph::AudioSegment`],
//! `media.v2::SoundEvent` ⇄ [`graph::SoundEvent`] and
//! `media.v2::Speaker` ⇄ [`graph::Speaker`].
//!
//! ## Field correspondence — `Audio`
//!
//! | wire field                       | graph field          | notes                 |
//! | -------------------------------- | -------------------- | --------------------- |
//! | `id` (bytes, 16)                 | `id`                 | validating            |
//! | `total_segments: uint32`         | `total_segments`     | denormalized rollup   |
//! | `total_sound_events: uint32`     | `total_sound_events` | denormalized rollup   |
//! | `track_progress: IndexProgress`  | `track_progress`     | unset ⇒ empty rollup  |
//! | `tracks: repeated AudioTrack`    | `tracks: Vec<_>`     | children embedded     |
//!
//! ## Field correspondence — `AudioTrack`
//!
//! | wire field                              | graph field                   | notes                                          |
//! | --------------------------------------- | ----------------------------- | ---------------------------------------------- |
//! | `id` (bytes, 16)                        | `id`                          | validating                                     |
//! | `stream_index` / `container_track_id`   | same                          |                                                |
//! | `codec: string`                         | `codec: AudioCodec`           | slug; total `FromStr`                          |
//! | `profile: string`                       | `profile: SmolStr`            | `""` = absent                                  |
//! | `sample_rate: uint32`                   | `sample_rate`                 | validating (descriptor invariant)              |
//! | `channels: uint32`                      | `channels: u16`               | widened; overflow ⇒ `Unsupported`; validating  |
//! | `channel_layout` / `sample_format`      | same                          | extern; unset ⇒ domain default                 |
//! | `bit_rate` / `bit_rate_mode` / `bits_per_sample` / `is_lossless` | same | `bits_per_sample` widened `u16`         |
//! | `duration` / `start_pts: Timestamp`     | `Option<Timestamp>`           | mediatime extern; negative duration rejected   |
//! | `language` / `detected_language`        | `Option<Language>`            | mediaframe extern; presence = `Some`           |
//! | `disposition: TrackDisposition`         | `disposition`                 | extern; unset ⇒ empty flags                    |
//! | `is_primary` / `auto_selected`          | same                          |                                                |
//! | `content: optional string`              | `content: Option<AudioContentKind>` | slug; unknown rejected                   |
//! | `speech_ratio: optional float`          | same                          | validating (`[0,1]`-finite)                    |
//! | `is_silent` / `loudness` / `replay_gain` / `fingerprint` | same         | externs; presence = `Some`                     |
//! | `isrc` / `acoustid` / `musicbrainz_recording_id` | same                 | `""` = absent                                  |
//! | `speakers: repeated Speaker`            | `speakers: Vec<_>`            | children embedded                              |
//! | `tags` / `cover_art`                    | `Option<_>`                   | externs; presence = `Some`                     |
//! | `segments: repeated AudioSegment`       | `segments: Vec<_>`            | children embedded                              |
//! | `sound_events: repeated SoundEvent`     | `sound_events: Vec<_>`        | children embedded                              |
//! | `metadata: repeated KeyValue`           | `metadata: IndexMap`          | insertion order preserved                      |
//! | `provenance: Provenance`                | `provenance`                  | unset ⇒ empty                                  |
//! | `vad_provenance: Provenance`            | `vad_provenance`              | unset ⇒ empty; VAD-model provenance, distinct from `provenance` |
//! | `ced_provenance: Provenance`            | `ced_provenance`              | unset ⇒ empty; CED-model provenance, distinct from `provenance` and `vad_provenance` |
//! | `index_status: uint32`                  | `index_status: AudioIndexStatus` | raw bits; decode re-runs the topology + descriptor invariants |
//! | `index_errors: repeated ErrorInfo`      | `index_errors: Vec<_>`        |                                                |
//!
//! ## Field correspondence — `AudioSegment`
//!
//! Same shape as the v1 [`audio_segment`](crate::buffa::audio_segment)
//! bridge minus the `audio_track_id` FK (implied by nesting), and with
//! `language` as the mediaframe extern message instead of the v1 bcp47
//! string form. `speaker_id` (cross-tree association into the sibling
//! speaker set) stays as an optional 16-byte id.
//!
//! ## Field correspondence — `SoundEvent`
//!
//! Same shape as the v1 [`sound_event`](crate::buffa::sound_event) bridge
//! minus the `audio_track_id` FK (implied by nesting); `detector` is the
//! producer slug (`"ced"` | `"manual"`) and the whole record reconstructs
//! in the single validating `try_new` (no nested children).
//!
//! ## Field correspondence — `Speaker`
//!
//! Same shape as the v1 [`speaker`](crate::buffa::speaker) bridge minus
//! the `audio_track_id` FK; `person_id` (cross-tree association) stays.

use buffa::{bytes::Bytes, MessageField};
use mediaframe::codec::AudioCodec;
use smol_str::SmolStr;

use super::{
  errors_from_wire, errors_to_wire, graph_err, id_from_wire, id_to_wire, index_progress_from_wire,
  index_progress_to_wire, metadata_from_wire, metadata_to_wire, narrow_u16, opt_msg,
  provenance_from_wire, provenance_to_wire, rejected, unknown_slug,
};
use crate::{
  buffa::{
    error::BuffaError,
    vo::localized_text_from_wire,
    voice_fingerprint::{voice_fingerprint_from_wire, voice_fingerprint_to_wire},
  },
  domain::{self, AudioContentKind, AudioIndexStatus, CedDetector, Uuid7, Word},
  generated::media::{v1 as wire1, v2 as wire},
  graph,
};

// ---------------------------------------------------------------------------
// graph::Audio ⇄ wire::Audio
// ---------------------------------------------------------------------------

impl From<&graph::Audio<Uuid7>> for wire::Audio {
  fn from(g: &graph::Audio<Uuid7>) -> Self {
    wire::Audio {
      id: id_to_wire(g.id_ref()),
      total_segments: g.total_segments(),
      total_sound_events: g.total_sound_events(),
      track_progress: index_progress_to_wire(g.track_progress_ref()),
      tracks: g
        .tracks_slice()
        .iter()
        .map(wire::AudioTrack::from)
        .collect(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Decode the facet and its track subtrees. The flat facet's `media_id`
/// is synthesized from the facet's own id (consumed by the lift).
impl TryFrom<&wire::Audio> for graph::Audio<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::Audio) -> Result<Self, Self::Error> {
    let id = id_from_wire(&w.id, "Audio.id")?;
    let tracks = w
      .tracks
      .iter()
      .map(|t| audio_track_from_wire(t, id))
      .collect::<Result<Vec<_>, _>>()?;
    let flat = domain::Audio::try_new(id, id)
      .map_err(rejected)?
      .with_total_segments(w.total_segments)
      .with_total_sound_events(w.total_sound_events)
      .with_track_progress(index_progress_from_wire(&w.track_progress)?);
    graph::Audio::try_from_flat(&id, flat, tracks).map_err(graph_err)
  }
}

// ---------------------------------------------------------------------------
// graph::AudioTrack ⇄ wire::AudioTrack
// ---------------------------------------------------------------------------

impl From<&graph::AudioTrack<Uuid7>> for wire::AudioTrack {
  fn from(g: &graph::AudioTrack<Uuid7>) -> Self {
    wire::AudioTrack {
      id: id_to_wire(g.id_ref()),
      stream_index: g.stream_index(),
      container_track_id: g.container_track_id(),
      codec: g.codec_ref().as_str().into(),
      profile: SmolStr::from(g.profile()),
      sample_rate: g.sample_rate(),
      channels: u32::from(g.channels()),
      channel_layout: MessageField::some(g.channel_layout_ref().clone()),
      sample_format: MessageField::some(g.sample_format_ref().clone()),
      bit_rate: g.bit_rate(),
      bit_rate_mode: opt_msg(g.bit_rate_mode()),
      bits_per_sample: g.bits_per_sample().map(u32::from),
      is_lossless: g.is_lossless(),
      duration: opt_msg(g.duration_ref().copied()),
      start_pts: opt_msg(g.start_pts_ref().copied()),
      language: opt_msg(g.language()),
      detected_language: opt_msg(g.detected_language()),
      disposition: MessageField::some(g.disposition()),
      is_primary: g.is_primary(),
      auto_selected: g.auto_selected(),
      content: g.content().map(|k| SmolStr::from(k.as_str())),
      speech_ratio: g.speech_ratio(),
      is_silent: g.is_silent(),
      loudness: opt_msg(g.loudness_ref().copied()),
      replay_gain: opt_msg(g.replay_gain_ref().copied()),
      fingerprint: opt_msg(g.fingerprint_ref().cloned()),
      isrc: SmolStr::from(g.isrc()),
      acoustid: SmolStr::from(g.acoustid()),
      musicbrainz_recording_id: SmolStr::from(g.musicbrainz_recording_id()),
      speakers: g.speakers_slice().iter().map(wire::Speaker::from).collect(),
      tags: opt_msg(g.tags_ref().cloned()),
      cover_art: opt_msg(g.cover_art_ref().cloned()),
      segments: g
        .segments_slice()
        .iter()
        .map(wire::AudioSegment::from)
        .collect(),
      sound_events: g
        .sound_events_slice()
        .iter()
        .map(wire::SoundEvent::from)
        .collect(),
      metadata: metadata_to_wire(g.metadata_ref()),
      provenance: provenance_to_wire(g.provenance_ref()),
      vad_provenance: provenance_to_wire(g.vad_provenance_ref()),
      ced_provenance: provenance_to_wire(g.ced_provenance_ref()),
      index_status: g.index_status().bits(),
      index_errors: errors_to_wire(g.index_errors_slice()),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Decode one track under the given parent facet id. Builder ordering
/// honours the domain invariants: `sample_rate` / `channels` land before
/// `try_with_index_status`, whose descriptor invariant reads them.
fn audio_track_from_wire(
  w: &wire::AudioTrack,
  audio_id: Uuid7,
) -> Result<graph::AudioTrack<Uuid7>, BuffaError> {
  let id = id_from_wire(&w.id, "AudioTrack.id")?;
  let Ok(codec) = w.codec.as_str().parse::<AudioCodec>();
  let content = w
    .content
    .as_ref()
    .map(|s| {
      AudioContentKind::from_str(s.as_str()).ok_or_else(|| unknown_slug("AudioTrack.content", s))
    })
    .transpose()?;

  let mut t = domain::AudioTrack::try_new(id, audio_id)
    .map_err(rejected)?
    .with_stream_index(w.stream_index)
    .with_container_track_id(w.container_track_id)
    .with_codec(codec)
    .with_profile(w.profile.as_str())
    .try_with_sample_rate(w.sample_rate)
    .map_err(rejected)?
    .try_with_channels(narrow_u16(w.channels, "AudioTrack.channels: u16")?)
    .map_err(rejected)?
    .with_bit_rate(w.bit_rate)
    .with_bit_rate_mode(w.bit_rate_mode.as_option().copied())
    .with_bits_per_sample(
      w.bits_per_sample
        .map(|v| narrow_u16(v, "AudioTrack.bits_per_sample: u16"))
        .transpose()?,
    )
    .with_lossless(w.is_lossless)
    .try_with_duration(w.duration.as_option().copied())
    .map_err(rejected)?
    .with_start_pts(w.start_pts.as_option().copied())
    .with_language(w.language.as_option().copied())
    .with_detected_language(w.detected_language.as_option().copied())
    .with_primary(w.is_primary)
    .with_auto_selected(w.auto_selected)
    .with_content(content)
    .with_silent(w.is_silent)
    .with_loudness(w.loudness.as_option().copied())
    .with_replay_gain(w.replay_gain.as_option().copied())
    .with_fingerprint(w.fingerprint.as_option().cloned())
    .with_isrc(w.isrc.as_str())
    .with_acoustid(w.acoustid.as_str())
    .with_musicbrainz_recording_id(w.musicbrainz_recording_id.as_str())
    .with_tags(w.tags.as_option().cloned())
    .with_cover_art(w.cover_art.as_option().cloned())
    .with_metadata(metadata_from_wire(&w.metadata))
    .with_provenance(provenance_from_wire(&w.provenance))
    .with_vad_provenance(provenance_from_wire(&w.vad_provenance))
    .with_ced_provenance(provenance_from_wire(&w.ced_provenance))
    .with_index_errors(errors_from_wire(&w.index_errors));
  if let Some(v) = w.channel_layout.as_option() {
    t = t.with_channel_layout(v.clone());
  }
  if let Some(v) = w.sample_format.as_option() {
    t = t.with_sample_format(v.clone());
  }
  if let Some(v) = w.disposition.as_option() {
    t = t.with_disposition(*v);
  }
  if w.speech_ratio.is_some() {
    t = t.try_with_speech_ratio(w.speech_ratio).map_err(rejected)?;
  }
  // Last: the status invariants read the descriptor fields set above.
  t = t
    .try_with_index_status(AudioIndexStatus::from_bits_retain(w.index_status))
    .map_err(rejected)?;

  let segments = w
    .segments
    .iter()
    .map(|s| flat_audio_segment(s, id))
    .collect::<Result<Vec<_>, _>>()?;
  let sound_events = w
    .sound_events
    .iter()
    .map(|s| flat_sound_event(s, id))
    .collect::<Result<Vec<_>, _>>()?;
  let speakers = w
    .speakers
    .iter()
    .map(|s| flat_speaker(s, id))
    .collect::<Result<Vec<_>, _>>()?;
  graph::AudioTrack::try_from_flat(&audio_id, t, segments, sound_events, speakers)
    .map_err(graph_err)
}

/// Standalone decode — the parent FK is synthesized from the track's
/// own id and consumed by the lift.
impl TryFrom<&wire::AudioTrack> for graph::AudioTrack<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::AudioTrack) -> Result<Self, Self::Error> {
    let synthetic_parent = id_from_wire(&w.id, "AudioTrack.id")?;
    audio_track_from_wire(w, synthetic_parent)
  }
}

// ---------------------------------------------------------------------------
// graph::AudioSegment ⇄ wire::AudioSegment
// ---------------------------------------------------------------------------

impl From<&graph::AudioSegment<Uuid7>> for wire::AudioSegment {
  fn from(g: &graph::AudioSegment<Uuid7>) -> Self {
    wire::AudioSegment {
      id: id_to_wire(g.id_ref()),
      index: g.index(),
      span: MessageField::some(*g.span_ref()),
      speaker_id: g.speaker_id_ref().map(id_to_wire),
      text: MessageField::some(wire1::LocalizedText::from(g.text_ref())),
      language: opt_msg(g.language()),
      words: g.words_slice().iter().map(wire1::Word::from).collect(),
      no_speech_prob: g.no_speech_prob(),
      avg_logprob: g.avg_logprob(),
      temperature: g.temperature(),
      voice_fingerprint: voice_fingerprint_to_wire(g.voice_fingerprint_ref()),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Reconstruct the flat segment under the given `audio_track_id` — the
/// same `try_new` + builder ordering as the v1 bridge (words and
/// `no_speech_prob` go through their validating builders).
fn flat_audio_segment(
  w: &wire::AudioSegment,
  audio_track_id: Uuid7,
) -> Result<domain::AudioSegment<Uuid7>, BuffaError> {
  let id = id_from_wire(&w.id, "AudioSegment.id")?;
  let span = *w
    .span
    .as_option()
    .ok_or(BuffaError::MissingRequiredField("AudioSegment.span"))?;
  let speaker_id = opt_id_from_wire(w.speaker_id.as_ref(), "AudioSegment.speaker_id")?;
  let words = w
    .words
    .iter()
    .map(Word::try_from)
    .collect::<Result<Vec<_>, _>>()?;
  let voice_fingerprint = voice_fingerprint_from_wire(&w.voice_fingerprint)?;

  let mut seg = domain::AudioSegment::try_new(id, audio_track_id, w.index, span)
    .map_err(rejected)?
    .with_speaker_id(speaker_id)
    .with_text(localized_text_from_wire(&w.text))
    .with_language(w.language.as_option().copied())
    .with_avg_logprob(w.avg_logprob)
    .with_temperature(w.temperature)
    .with_voice_fingerprint(voice_fingerprint);
  if !words.is_empty() {
    seg = seg.try_with_words(words).map_err(rejected)?;
  }
  if w.no_speech_prob.is_some() {
    seg = seg
      .try_with_no_speech_prob(w.no_speech_prob)
      .map_err(rejected)?;
  }
  Ok(seg)
}

/// Standalone decode — the parent FK is synthesized from the segment's
/// own id and consumed by the lift.
impl TryFrom<&wire::AudioSegment> for graph::AudioSegment<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::AudioSegment) -> Result<Self, Self::Error> {
    let synthetic_parent = id_from_wire(&w.id, "AudioSegment.id")?;
    let flat = flat_audio_segment(w, synthetic_parent)?;
    graph::AudioSegment::try_from_flat(&synthetic_parent, flat).map_err(graph_err)
  }
}

// ---------------------------------------------------------------------------
// graph::SoundEvent ⇄ wire::SoundEvent
// ---------------------------------------------------------------------------

impl From<&graph::SoundEvent<Uuid7>> for wire::SoundEvent {
  fn from(g: &graph::SoundEvent<Uuid7>) -> Self {
    wire::SoundEvent {
      id: id_to_wire(g.id_ref()),
      index: g.index(),
      span: MessageField::some(*g.span_ref()),
      label: SmolStr::from(g.label()),
      code: g.code(),
      score: g.score(),
      detector: SmolStr::from(g.detector().as_str()),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Reconstruct the flat sound event under the given `audio_track_id` — the
/// same `try_new` reconstruction as the v1 bridge (the whole record builds
/// in the single validating constructor; no `with_*` chain needed).
fn flat_sound_event(
  w: &wire::SoundEvent,
  audio_track_id: Uuid7,
) -> Result<domain::SoundEvent<Uuid7>, BuffaError> {
  let id = id_from_wire(&w.id, "SoundEvent.id")?;
  let span = *w
    .span
    .as_option()
    .ok_or(BuffaError::MissingRequiredField("SoundEvent.span"))?;
  // An unrecognized slug is only reachable via a tampered / out-of-contract
  // wire frame: a domain `CedDetector` always serializes to a canonical
  // slug. Surface it as the same generic "rejected" error the rest of this
  // bridge uses for out-of-contract payloads.
  let detector = CedDetector::from_str(w.detector.as_str())
    .ok_or_else(|| unknown_slug("SoundEvent.detector", w.detector.as_str()))?;
  domain::SoundEvent::try_new(
    id,
    audio_track_id,
    w.index,
    span,
    w.label.as_str(),
    w.code,
    w.score,
    detector,
  )
  .map_err(rejected)
}

/// Standalone decode — the parent FK is synthesized from the sound event's
/// own id and consumed by the lift.
impl TryFrom<&wire::SoundEvent> for graph::SoundEvent<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::SoundEvent) -> Result<Self, Self::Error> {
    let synthetic_parent = id_from_wire(&w.id, "SoundEvent.id")?;
    let flat = flat_sound_event(w, synthetic_parent)?;
    graph::SoundEvent::try_from_flat(&synthetic_parent, flat).map_err(graph_err)
  }
}

// ---------------------------------------------------------------------------
// graph::Speaker ⇄ wire::Speaker
// ---------------------------------------------------------------------------

impl From<&graph::Speaker<Uuid7>> for wire::Speaker {
  fn from(g: &graph::Speaker<Uuid7>) -> Self {
    wire::Speaker {
      id: id_to_wire(g.id_ref()),
      cluster_id: g.cluster_id(),
      name: SmolStr::from(g.name()),
      speech_duration: opt_msg(g.speech_duration_ref().copied()),
      voiceprint: voice_fingerprint_to_wire(g.voiceprint_ref()),
      person_id: g.person_id_ref().map(id_to_wire),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Reconstruct the flat speaker under the given `audio_track_id` — the
/// same `try_new` + builder chain as the v1 bridge.
fn flat_speaker(
  w: &wire::Speaker,
  audio_track_id: Uuid7,
) -> Result<domain::Speaker<Uuid7>, BuffaError> {
  let id = id_from_wire(&w.id, "Speaker.id")?;
  let voiceprint = voice_fingerprint_from_wire(&w.voiceprint)?;
  let person_id = opt_id_from_wire(w.person_id.as_ref(), "Speaker.person_id")?;
  domain::Speaker::try_new(
    id,
    audio_track_id,
    w.cluster_id,
    SmolStr::from(w.name.as_str()),
  )
  .map_err(rejected)?
  .try_with_speech_duration(w.speech_duration.as_option().copied())
  .map_err(rejected)
  .map(|s| s.maybe_voiceprint(voiceprint).maybe_person_id(person_id))
}

/// Standalone decode — the parent FK is synthesized from the speaker's
/// own id and consumed by the lift.
impl TryFrom<&wire::Speaker> for graph::Speaker<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::Speaker) -> Result<Self, Self::Error> {
    let synthetic_parent = id_from_wire(&w.id, "Speaker.id")?;
    let flat = flat_speaker(w, synthetic_parent)?;
    graph::Speaker::try_from_flat(&synthetic_parent, flat).map_err(graph_err)
  }
}

/// Decode an optional 16-byte cross-tree association id.
fn opt_id_from_wire(b: Option<&Bytes>, field: &'static str) -> Result<Option<Uuid7>, BuffaError> {
  match b {
    Some(b) => id_from_wire(b, field).map(Some),
    None => Ok(None),
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use core::num::NonZeroU32;

  use jiff::Timestamp as JiffTimestamp;
  use mediaframe::lang::Language;
  use mediatime::{TimeRange, Timebase, Timestamp};

  use super::*;
  use crate::domain::{IndexProgress, LocalizedText, Provenance, VoiceFingerprint};

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).expect("nonzero"))
  }

  fn span(start: i64, end: i64) -> TimeRange {
    TimeRange::new(start, end, tb())
  }

  fn vfp() -> VoiceFingerprint<Uuid7> {
    VoiceFingerprint::try_new(
      Uuid7::new(),
      192,
      JiffTimestamp::from_millisecond(1_700_000_000_000).expect("valid ts"),
      Some(0.83),
      Provenance::from_parts("ecapa-tdnn", "v1.0.0", "", "findit-indexer-0.1.0"),
    )
    .expect("valid voiceprint")
  }

  fn flat_segment(track_id: Uuid7, speaker_id: Uuid7) -> domain::AudioSegment<Uuid7> {
    let es = Language::from_bcp47("es").expect("valid tag");
    domain::AudioSegment::try_new(Uuid7::new(), track_id, 0, span(0, 400))
      .expect("valid segment")
      .with_speaker_id(Some(speaker_id))
      .with_text(LocalizedText::from_src_translated("hola", "hello"))
      .with_language(Some(es))
      .try_with_words(vec![Word::try_new("hola", span(0, 200), 0.95)
        .expect("valid word")
        .with_language(Some(es))])
      .expect("words fit")
      .try_with_no_speech_prob(Some(0.05))
      .expect("valid prob")
      .with_avg_logprob(Some(-0.4))
      .with_temperature(Some(0.0))
      .with_voice_fingerprint(Some(vfp()))
  }

  fn flat_speaker_fixture(track_id: Uuid7, speaker_id: Uuid7) -> domain::Speaker<Uuid7> {
    domain::Speaker::try_new(speaker_id, track_id, 2, "Jane")
      .expect("valid speaker")
      .try_with_speech_duration(Some(Timestamp::new(5_000, tb())))
      .expect("valid duration")
      .with_voiceprint(vfp())
      .with_person_id(Uuid7::new())
  }

  fn flat_sound_event(track_id: Uuid7) -> domain::SoundEvent<Uuid7> {
    domain::SoundEvent::try_new(
      Uuid7::new(),
      track_id,
      0,
      span(0, 400),
      "Siren",
      Some(316),
      0.42,
      CedDetector::Manual,
    )
    .expect("valid sound event")
  }

  fn rich_track(audio_id: Uuid7) -> domain::AudioTrack<Uuid7> {
    domain::AudioTrack::try_new(Uuid7::new(), audio_id)
      .expect("valid track")
      .with_stream_index(Some(1))
      .with_container_track_id(Some(2))
      .with_codec("aac".parse().expect("total"))
      .with_profile("LC")
      .try_with_sample_rate(48_000)
      .expect("valid rate")
      .try_with_channels(2)
      .expect("valid channels")
      .with_bit_rate(192_000)
      .with_bits_per_sample(Some(16))
      .with_lossless(false)
      .try_with_duration(Some(Timestamp::new(90_000, tb())))
      .expect("valid duration")
      .with_start_pts(Some(Timestamp::new(0, tb())))
      .with_language(Some(Language::from_bcp47("en").expect("valid tag")))
      .with_detected_language(Some(Language::from_bcp47("en-US").expect("valid tag")))
      .with_primary(true)
      .with_auto_selected(true)
      .with_content(Some(AudioContentKind::Speech))
      .try_with_speech_ratio(Some(0.85))
      .expect("valid ratio")
      .with_silent(false)
      .with_isrc("USRC17607839")
      .with_acoustid("acoustid-x")
      .with_musicbrainz_recording_id("mbid-y")
      .with_provenance(Provenance::from_parts(
        "whisper",
        "v3",
        "p-1",
        "indexer-0.1",
      ))
      .with_vad_provenance(Provenance::from_parts("silero", "v5", "p-1", "indexer-0.1"))
      .with_ced_provenance(Provenance::from_parts(
        "ced-net",
        "v2",
        "p-1",
        "indexer-0.1",
      ))
      .try_with_index_status(AudioIndexStatus::EXTRACTED | AudioIndexStatus::CLASSIFIED)
      .expect("valid status")
  }

  #[test]
  fn audio_segment_round_trips() {
    let track_id = Uuid7::new();
    let g = graph::AudioSegment::try_from_flat(&track_id, flat_segment(track_id, Uuid7::new()))
      .expect("coherent");
    let w = wire::AudioSegment::from(&g);
    let g2 = graph::AudioSegment::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }

  #[test]
  fn sound_event_round_trips() {
    let track_id = Uuid7::new();
    let g =
      graph::SoundEvent::try_from_flat(&track_id, flat_sound_event(track_id)).expect("coherent");
    let w = wire::SoundEvent::from(&g);
    let g2 = graph::SoundEvent::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }

  #[test]
  fn speaker_round_trips() {
    let track_id = Uuid7::new();
    let g = graph::Speaker::try_from_flat(&track_id, flat_speaker_fixture(track_id, Uuid7::new()))
      .expect("coherent");
    let w = wire::Speaker::from(&g);
    let g2 = graph::Speaker::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }

  #[test]
  fn audio_track_round_trips_with_children() {
    let audio_id = Uuid7::new();
    let track = rich_track(audio_id);
    let track_id = *track.id_ref();
    let speaker_id = Uuid7::new();
    let g = graph::AudioTrack::try_from_flat(
      &audio_id,
      track,
      vec![flat_segment(track_id, speaker_id)],
      vec![flat_sound_event(track_id)],
      vec![flat_speaker_fixture(track_id, speaker_id)],
    )
    .expect("coherent");
    let w = wire::AudioTrack::from(&g);
    let g2 = graph::AudioTrack::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
    // The cross-tree association survives the trip.
    assert_eq!(
      g2.segments_slice()[0].speaker_id_ref(),
      Some(g2.speakers_slice()[0].id_ref())
    );
    // The embedded sound event survives the trip.
    assert_eq!(g2.sound_events_slice().len(), 1);
    assert_eq!(g2.sound_events_slice()[0].label(), "Siren");
  }

  #[test]
  fn audio_facet_round_trips() {
    let media_id = Uuid7::new();
    let facet = domain::Audio::try_new(Uuid7::new(), media_id)
      .expect("valid facet")
      .with_total_segments(7)
      .with_total_sound_events(3)
      .with_track_progress(IndexProgress::try_new(1, 1, 0).expect("valid rollup"));
    let facet_id = *facet.id_ref();
    let track =
      graph::AudioTrack::try_from_flat(&facet_id, rich_track(facet_id), vec![], vec![], vec![])
        .expect("coherent");
    let g = graph::Audio::try_from_flat(&media_id, facet, vec![track]).expect("coherent");
    let w = wire::Audio::from(&g);
    let g2 = graph::Audio::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }

  #[test]
  fn audio_track_unknown_content_slug_errors() {
    let audio_id = Uuid7::new();
    let g =
      graph::AudioTrack::try_from_flat(&audio_id, rich_track(audio_id), vec![], vec![], vec![])
        .expect("coherent");
    let mut w = wire::AudioTrack::from(&g);
    w.content = Some(SmolStr::from("polka"));
    let err = graph::AudioTrack::try_from(&w).unwrap_err();
    assert!(err.is_domain_constructor_rejected());
  }

  #[test]
  fn audio_segment_missing_span_errors() {
    let track_id = Uuid7::new();
    let g = graph::AudioSegment::try_from_flat(&track_id, flat_segment(track_id, Uuid7::new()))
      .expect("coherent");
    let mut w = wire::AudioSegment::from(&g);
    w.span = MessageField::none();
    let err = graph::AudioSegment::try_from(&w).unwrap_err();
    assert!(err.is_missing_required_field());
  }
}
