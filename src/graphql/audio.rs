//! GraphQL exposure of the Audio aggregates.
//!
//! Each `#[Object]` impl is on a `Gql*` newtype wrapper around the
//! corresponding domain type (the domain types have inherent methods
//! with the same names as the GraphQL fields, so the macro's "inject
//! into impl block" approach collides with them).

use async_graphql::{Object, ID};

use crate::domain::{
  aggregates::audio::segment::Word, Audio, AudioCoverArt, AudioFingerprint, AudioSegment,
  AudioTags, AudioTrack, Loudness, Uuid7,
};

use super::{
  bitflags::GqlAudioIndexStatus,
  enums::GqlAudioContentKind,
  media::{GqlErrorInfo, GqlLocalizedText, GqlProvenance},
  scalars::{empty_as_none, GqlMediaTimeRange, GqlMediaTimestamp},
};

// ---------------------------------------------------------------------------
// Word
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`Word`].
#[derive(Debug, Clone, PartialEq)]
pub struct GqlWord(pub Word);

impl From<Word> for GqlWord {
  #[inline]
  fn from(v: Word) -> Self {
    Self(v)
  }
}
impl From<GqlWord> for Word {
  #[inline]
  fn from(v: GqlWord) -> Self {
    v.0
  }
}

#[Object(name = "Word")]
impl GqlWord {
  async fn text(&self) -> String {
    self.0.text().to_string()
  }
  async fn span(&self) -> GqlMediaTimeRange {
    GqlMediaTimeRange(*self.0.span())
  }
  async fn score(&self) -> f32 {
    self.0.score()
  }
  async fn language(&self) -> Option<String> {
    self.0.language().map(|s| s.to_string())
  }
}

// ---------------------------------------------------------------------------
// AudioTags
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`AudioTags`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GqlAudioTags(pub AudioTags);

impl From<AudioTags> for GqlAudioTags {
  #[inline]
  fn from(v: AudioTags) -> Self {
    Self(v)
  }
}
impl From<GqlAudioTags> for AudioTags {
  #[inline]
  fn from(v: GqlAudioTags) -> Self {
    v.0
  }
}

#[Object(name = "AudioTags")]
impl GqlAudioTags {
  async fn title(&self) -> Option<String> {
    empty_as_none(self.0.title())
  }
  async fn artist(&self) -> Option<String> {
    empty_as_none(self.0.artist())
  }
  async fn album_artist(&self) -> Option<String> {
    empty_as_none(self.0.album_artist())
  }
  async fn album(&self) -> Option<String> {
    empty_as_none(self.0.album())
  }
  async fn genre(&self) -> Option<String> {
    empty_as_none(self.0.genre())
  }
  async fn composer(&self) -> Option<String> {
    empty_as_none(self.0.composer())
  }
  async fn performer(&self) -> Option<String> {
    empty_as_none(self.0.performer())
  }
  async fn date(&self) -> Option<String> {
    empty_as_none(self.0.date())
  }
  async fn track_number(&self) -> u32 {
    self.0.track_number()
  }
  async fn total_tracks(&self) -> u32 {
    self.0.total_tracks()
  }
  async fn disc_number(&self) -> u32 {
    self.0.disc_number()
  }
  async fn total_discs(&self) -> u32 {
    self.0.total_discs()
  }
  async fn comment(&self) -> Option<String> {
    empty_as_none(self.0.comment())
  }
  async fn lyrics(&self) -> Option<String> {
    empty_as_none(self.0.lyrics())
  }
  async fn tag_types(&self) -> std::vec::Vec<String> {
    self.0.tag_types().iter().map(|s| s.to_string()).collect()
  }
}

// ---------------------------------------------------------------------------
// AudioCoverArt
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`AudioCoverArt`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GqlAudioCoverArt(pub AudioCoverArt);

impl From<AudioCoverArt> for GqlAudioCoverArt {
  #[inline]
  fn from(v: AudioCoverArt) -> Self {
    Self(v)
  }
}
impl From<GqlAudioCoverArt> for AudioCoverArt {
  #[inline]
  fn from(v: GqlAudioCoverArt) -> Self {
    v.0
  }
}

#[Object(name = "AudioCoverArt")]
impl GqlAudioCoverArt {
  async fn size(&self) -> usize {
    self.0.size()
  }
  async fn mime(&self) -> Option<String> {
    empty_as_none(self.0.mime())
  }
  /// Base64-encoded image bytes (`null` when no data is set).
  async fn data_base64(&self) -> Option<String> {
    if self.0.data().is_empty() {
      None
    } else {
      Some(base64_encode(self.0.data()))
    }
  }
}

/// Standard base64 encoder (RFC 4648). Avoids pulling a new dep — the
/// encoding is small + self-contained.
fn base64_encode(data: &[u8]) -> String {
  const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
  let mut chunks = data.chunks_exact(3);
  for chunk in chunks.by_ref() {
    let b0 = chunk[0];
    let b1 = chunk[1];
    let b2 = chunk[2];
    out.push(ALPHABET[(b0 >> 2) as usize] as char);
    out.push(ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
    out.push(ALPHABET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
    out.push(ALPHABET[(b2 & 0x3f) as usize] as char);
  }
  let rem = chunks.remainder();
  match rem.len() {
    0 => {}
    1 => {
      let b0 = rem[0];
      out.push(ALPHABET[(b0 >> 2) as usize] as char);
      out.push(ALPHABET[((b0 & 0x03) << 4) as usize] as char);
      out.push('=');
      out.push('=');
    }
    2 => {
      let b0 = rem[0];
      let b1 = rem[1];
      out.push(ALPHABET[(b0 >> 2) as usize] as char);
      out.push(ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
      out.push(ALPHABET[((b1 & 0x0f) << 2) as usize] as char);
      out.push('=');
    }
    _ => unreachable!(),
  }
  out
}

// ---------------------------------------------------------------------------
// Loudness
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`Loudness`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GqlLoudness(pub Loudness);

impl From<Loudness> for GqlLoudness {
  #[inline]
  fn from(v: Loudness) -> Self {
    Self(v)
  }
}
impl From<GqlLoudness> for Loudness {
  #[inline]
  fn from(v: GqlLoudness) -> Self {
    v.0
  }
}

#[Object(name = "Loudness")]
impl GqlLoudness {
  async fn integrated_lufs(&self) -> f32 {
    self.0.integrated_lufs()
  }
  async fn true_peak_dbtp(&self) -> f32 {
    self.0.true_peak_dbtp()
  }
  async fn loudness_range_lu(&self) -> f32 {
    self.0.loudness_range_lu()
  }
}

// ---------------------------------------------------------------------------
// AudioFingerprint
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`AudioFingerprint`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GqlAudioFingerprint(pub AudioFingerprint);

impl From<AudioFingerprint> for GqlAudioFingerprint {
  #[inline]
  fn from(v: AudioFingerprint) -> Self {
    Self(v)
  }
}
impl From<GqlAudioFingerprint> for AudioFingerprint {
  #[inline]
  fn from(v: GqlAudioFingerprint) -> Self {
    v.0
  }
}

#[Object(name = "AudioFingerprint")]
impl GqlAudioFingerprint {
  async fn algo(&self) -> String {
    self.0.algo().to_string()
  }
  /// Base64-encoded fingerprint bytes (`null` if empty).
  async fn value_base64(&self) -> Option<String> {
    if self.0.value().is_empty() {
      None
    } else {
      Some(base64_encode(self.0.value()))
    }
  }
  async fn byte_len(&self) -> usize {
    self.0.value().len()
  }
}

// ---------------------------------------------------------------------------
// Audio facet
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`Audio`].
#[derive(Debug, Clone)]
pub struct GqlAudio(pub Audio<Uuid7>);

impl From<Audio<Uuid7>> for GqlAudio {
  #[inline]
  fn from(v: Audio<Uuid7>) -> Self {
    Self(v)
  }
}
impl From<GqlAudio> for Audio<Uuid7> {
  #[inline]
  fn from(v: GqlAudio) -> Self {
    v.0
  }
}

#[Object(name = "Audio")]
impl GqlAudio {
  async fn id(&self) -> ID {
    ID(self.0.id().to_string())
  }
  async fn tracks(&self) -> std::vec::Vec<ID> {
    self
      .0
      .tracks()
      .iter()
      .map(|id| ID(id.to_string()))
      .collect()
  }
  async fn total_segments(&self) -> u32 {
    self.0.total_segments()
  }
}

// ---------------------------------------------------------------------------
// AudioTrack
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`AudioTrack`].
#[derive(Debug, Clone)]
pub struct GqlAudioTrack(pub AudioTrack<Uuid7>);

impl From<AudioTrack<Uuid7>> for GqlAudioTrack {
  #[inline]
  fn from(v: AudioTrack<Uuid7>) -> Self {
    Self(v)
  }
}
impl From<GqlAudioTrack> for AudioTrack<Uuid7> {
  #[inline]
  fn from(v: GqlAudioTrack) -> Self {
    v.0
  }
}

#[Object(name = "AudioTrack")]
impl GqlAudioTrack {
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
  async fn codec(&self) -> Option<String> {
    empty_as_none(self.0.codec())
  }
  async fn profile(&self) -> Option<String> {
    empty_as_none(self.0.profile())
  }
  async fn sample_rate(&self) -> u32 {
    self.0.sample_rate()
  }
  async fn channels(&self) -> u32 {
    u32::from(self.0.channels())
  }
  async fn channel_layout(&self) -> Option<String> {
    empty_as_none(self.0.channel_layout())
  }
  async fn bit_rate(&self) -> String {
    self.0.bit_rate().to_string()
  }
  async fn bit_rate_mode(&self) -> Option<String> {
    self.0.bit_rate_mode().map(|s| s.to_string())
  }
  async fn bits_per_sample(&self) -> Option<u32> {
    self.0.bits_per_sample().map(u32::from)
  }
  async fn is_lossless(&self) -> bool {
    self.0.is_lossless()
  }
  async fn duration(&self) -> Option<GqlMediaTimestamp> {
    self.0.duration().copied().map(GqlMediaTimestamp)
  }
  async fn start_pts(&self) -> Option<GqlMediaTimestamp> {
    self.0.start_pts().copied().map(GqlMediaTimestamp)
  }
  async fn language(&self) -> Option<String> {
    self.0.language().map(|s| s.to_string())
  }
  async fn detected_language(&self) -> Option<String> {
    self.0.detected_language().map(|s| s.to_string())
  }
  async fn language_mismatch(&self) -> bool {
    self.0.language_mismatch()
  }
  async fn disposition(&self) -> u32 {
    self.0.disposition()
  }
  async fn is_primary(&self) -> bool {
    self.0.is_primary()
  }
  async fn auto_selected(&self) -> bool {
    self.0.auto_selected()
  }
  async fn content(&self) -> Option<GqlAudioContentKind> {
    self.0.content().map(Into::into)
  }
  async fn speech_ratio(&self) -> Option<f32> {
    self.0.speech_ratio()
  }
  async fn is_silent(&self) -> bool {
    self.0.is_silent()
  }
  async fn loudness(&self) -> Option<GqlLoudness> {
    self.0.loudness().copied().map(GqlLoudness)
  }
  async fn fingerprint(&self) -> Option<GqlAudioFingerprint> {
    self.0.fingerprint().cloned().map(GqlAudioFingerprint)
  }
  async fn isrc(&self) -> Option<String> {
    empty_as_none(self.0.isrc())
  }
  async fn acoustid(&self) -> Option<String> {
    empty_as_none(self.0.acoustid())
  }
  async fn musicbrainz_recording_id(&self) -> Option<String> {
    empty_as_none(self.0.musicbrainz_recording_id())
  }
  async fn speakers(&self) -> std::vec::Vec<ID> {
    self
      .0
      .speakers()
      .iter()
      .map(|id| ID(id.to_string()))
      .collect()
  }
  async fn tags(&self) -> Option<GqlAudioTags> {
    self.0.tags().cloned().map(GqlAudioTags)
  }
  async fn cover_art(&self) -> Option<GqlAudioCoverArt> {
    self.0.cover_art().cloned().map(GqlAudioCoverArt)
  }
  async fn segments(&self) -> std::vec::Vec<ID> {
    self
      .0
      .segments()
      .iter()
      .map(|id| ID(id.to_string()))
      .collect()
  }
  async fn provenance(&self) -> GqlProvenance {
    GqlProvenance(self.0.provenance().clone())
  }
  async fn index_status(&self) -> GqlAudioIndexStatus {
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
// AudioSegment
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`AudioSegment`].
#[derive(Debug, Clone)]
pub struct GqlAudioSegment(pub AudioSegment<Uuid7>);

impl From<AudioSegment<Uuid7>> for GqlAudioSegment {
  #[inline]
  fn from(v: AudioSegment<Uuid7>) -> Self {
    Self(v)
  }
}
impl From<GqlAudioSegment> for AudioSegment<Uuid7> {
  #[inline]
  fn from(v: GqlAudioSegment) -> Self {
    v.0
  }
}

#[Object(name = "AudioSegment")]
impl GqlAudioSegment {
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
  async fn speaker(&self) -> Option<ID> {
    self.0.speaker().map(|id| ID(id.to_string()))
  }
  async fn text(&self) -> GqlLocalizedText {
    GqlLocalizedText(self.0.text().clone())
  }
  async fn language(&self) -> Option<String> {
    self.0.language().map(|s| s.to_string())
  }
  async fn words(&self) -> std::vec::Vec<GqlWord> {
    self.0.words().iter().cloned().map(GqlWord).collect()
  }
  async fn no_speech_prob(&self) -> Option<f32> {
    self.0.no_speech_prob()
  }
  async fn avg_logprob(&self) -> Option<f32> {
    self.0.avg_logprob()
  }
  async fn temperature(&self) -> Option<f32> {
    self.0.temperature()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use core::num::NonZeroU32;

  #[test]
  fn audio_facet_resolvers_round_trip_through_domain() {
    let id = Uuid7::new();
    let a = Audio::try_new(id).unwrap();
    let g: GqlAudio = a.clone().into();
    let back: Audio<Uuid7> = g.into();
    assert_eq!(Audio::id(&back), Audio::id(&a));
  }

  #[test]
  fn base64_encode_known_vectors() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
    assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
  }

  #[test]
  fn word_wrapper_roundtrips() {
    let tb = mediatime::Timebase::new(1, NonZeroU32::new(1000).unwrap());
    let span = mediatime::TimeRange::new(0, 500, tb);
    let w = Word::new("hello", span, 0.9);
    let g: GqlWord = w.clone().into();
    let back: Word = g.into();
    assert_eq!(back, w);
  }
}
