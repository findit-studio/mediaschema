//! `AudioTrack` — one audio stream of an `Audio` facet.
//!
//! Locked `schema/audio_track.md` rev 3. A multi-track audio file = N
//! distinct recordings, so per-recording music metadata (tags + cover art)
//! lives **here**, not on a file/facet. Holds codec/stream descriptor,
//! per-track signal analysis (loudness/fingerprint), per-track indexing
//! state + provenance, the diarized-speaker set, and (A-loc per-track) the
//! per-track segments refs → `AudioSegment`.
//!
//! ## mediaframe types
//!
//! The locked spec types several fields as `mediaframe::*` externs
//! (`codec::AudioCodec`, `audio::ChannelLayout`, `lang::Language`,
//! `disposition::TrackDisposition`, `audio::BitRateMode`); the
//! per-recording signal/metadata VOs (`audio::Loudness`,
//! `audio::Fingerprint`, `audio::Tags`, `audio::CoverArt`) likewise live
//! in `mediaframe`. These are wired through directly.

use derive_more::IsVariant;
use mediaframe::{
  audio::{BitRateMode, ChannelLayout, CoverArt, Fingerprint, Loudness, Tags},
  codec::AudioCodec,
  disposition::TrackDisposition,
  lang::Language,
};
use mediatime::Timestamp;
use smol_str::SmolStr;

use crate::domain::{
  bitflags::AudioIndexStatus, enums::AudioContentKind, primitives::ErrorInfo, vo::Provenance, Uuid7,
};

// ---------------------------------------------------------------------------
// AudioTrack
// ---------------------------------------------------------------------------

/// One audio stream of an `Audio` facet (`parent → Audio.id`).
///
/// Generic over `Id` (default [`Uuid7`]). See module docs for the
/// `mediaframe` descriptor / VO types used by its fields.
///
/// **No `Default`** — defaulting to a nil id + nil parent is an orphan
/// state. Use [`AudioTrack::try_new`].
#[derive(Debug, Clone, PartialEq)]
pub struct AudioTrack<Id = Uuid7> {
  id: Id,
  parent: Id,
  stream_index: Option<u32>,
  container_track_id: Option<u64>,
  codec: AudioCodec,
  profile: SmolStr,
  sample_rate: u32,
  channels: u16,
  channel_layout: ChannelLayout,
  bit_rate: u64,
  bit_rate_mode: Option<BitRateMode>,
  bits_per_sample: Option<u16>,
  is_lossless: bool,
  // TODO(mediaframe): `duration: Option<mediatime::TrackTime>` — `mediatime`
  // 0.1.6 publicly exports only `Timestamp`/`TimeRange`/`Timebase` (no
  // `TrackTime`). Same workaround as `Speaker.speech_duration`:
  // `mediatime::Timestamp` treated as a track-relative offset/duration.
  duration: Option<Timestamp>,
  start_pts: Option<Timestamp>,
  language: Option<Language>,
  detected_language: Option<Language>,
  language_mismatch: bool,
  disposition: TrackDisposition,
  is_primary: bool,
  auto_selected: bool,
  content: Option<AudioContentKind>,
  speech_ratio: Option<f32>,
  is_silent: bool,
  loudness: Option<Loudness>,
  fingerprint: Option<Fingerprint>,
  isrc: SmolStr,
  acoustid: SmolStr,
  musicbrainz_recording_id: SmolStr,
  speakers: std::vec::Vec<Id>,
  tags: Option<Tags>,
  cover_art: Option<CoverArt>,
  segments: std::vec::Vec<Id>,
  provenance: Provenance,
  index_status: AudioIndexStatus,
  index_errors: std::vec::Vec<ErrorInfo>,
}

impl AudioTrack<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (every aggregate row needs a real identity) and nil
  /// `parent` (orphan track with no `Audio` facet). All descriptive fields
  /// start in their `""` / `None` / `0` / `false` neutral state and are
  /// filled by builders/mutators as the indexing pipeline runs.
  pub fn try_new(id: Uuid7, parent: Uuid7) -> Result<Self, AudioTrackError> {
    if id.is_nil() {
      return Err(AudioTrackError::NilId);
    }
    if parent.is_nil() {
      return Err(AudioTrackError::NilParent);
    }
    Ok(Self {
      id,
      parent,
      stream_index: None,
      container_track_id: None,
      codec: AudioCodec::Other(SmolStr::default()),
      profile: SmolStr::default(),
      sample_rate: 0,
      channels: 0,
      channel_layout: ChannelLayout::default(),
      bit_rate: 0,
      bit_rate_mode: None,
      bits_per_sample: None,
      is_lossless: false,
      duration: None,
      start_pts: None,
      language: None,
      detected_language: None,
      language_mismatch: false,
      disposition: TrackDisposition::empty(),
      is_primary: false,
      auto_selected: false,
      content: None,
      speech_ratio: None,
      is_silent: false,
      loudness: None,
      fingerprint: None,
      isrc: SmolStr::default(),
      acoustid: SmolStr::default(),
      musicbrainz_recording_id: SmolStr::default(),
      speakers: std::vec::Vec::new(),
      tags: None,
      cover_art: None,
      segments: std::vec::Vec::new(),
      provenance: Provenance::new(),
      index_status: AudioIndexStatus::empty(),
      index_errors: std::vec::Vec::new(),
    })
  }
}

impl<Id> AudioTrack<Id> {
  /// Canonical identity.
  #[inline]
  pub const fn id(&self) -> &Id {
    &self.id
  }

  /// FK → `Audio.id`.
  #[inline]
  pub const fn parent(&self) -> &Id {
    &self.parent
  }

  /// Source stream index (FFmpeg/container locator; not identity).
  #[inline]
  pub const fn stream_index(&self) -> Option<u32> {
    self.stream_index
  }

  /// Container-specific track id (Matroska TrackNumber etc.).
  #[inline]
  pub const fn container_track_id(&self) -> Option<u64> {
    self.container_track_id
  }

  /// Codec (`AudioCodec::Other("")` = absent).
  #[inline]
  pub const fn codec(&self) -> &AudioCodec {
    &self.codec
  }

  /// Codec profile (e.g. `LC` / `HE-AACv2`; `""` = absent).
  #[inline]
  pub fn profile(&self) -> &str {
    self.profile.as_str()
  }

  /// Sample rate (Hz; `0` = unknown).
  #[inline]
  pub const fn sample_rate(&self) -> u32 {
    self.sample_rate
  }

  /// Channel count (`0` = unknown).
  #[inline]
  pub const fn channels(&self) -> u16 {
    self.channels
  }

  /// Channel layout (`ChannelLayout::Other("")` = absent).
  #[inline]
  pub const fn channel_layout(&self) -> &ChannelLayout {
    &self.channel_layout
  }

  /// Bit rate (bits/s; `0` = unknown).
  #[inline]
  pub const fn bit_rate(&self) -> u64 {
    self.bit_rate
  }

  /// Bit-rate mode (`Cbr`/`Vbr`/`Abr`; `None` = unknown).
  #[inline]
  pub const fn bit_rate_mode(&self) -> Option<BitRateMode> {
    self.bit_rate_mode
  }

  /// PCM/lossless sample depth.
  #[inline]
  pub const fn bits_per_sample(&self) -> Option<u16> {
    self.bits_per_sample
  }

  /// Lossless flag (drives transcode/quality search facets).
  #[inline]
  pub const fn is_lossless(&self) -> bool {
    self.is_lossless
  }

  /// Per-track duration (track-relative offset/duration; see
  /// TODO(mediaframe) note on the field).
  #[inline]
  pub const fn duration(&self) -> Option<&Timestamp> {
    self.duration.as_ref()
  }

  /// First-PTS offset (audio rarely starts at 0; A/V sync/seek).
  #[inline]
  pub const fn start_pts(&self) -> Option<&Timestamp> {
    self.start_pts.as_ref()
  }

  /// Declared language (BCP-47; `None` = absent).
  #[inline]
  pub const fn language(&self) -> Option<Language> {
    self.language
  }

  /// Whisper-LID detected language (BCP-47; `None` = absent).
  #[inline]
  pub const fn detected_language(&self) -> Option<Language> {
    self.detected_language
  }

  /// `detected ≠ declared` (derived).
  #[inline]
  pub const fn language_mismatch(&self) -> bool {
    self.language_mismatch
  }

  /// Disposition flags (`AV_DISPOSITION_*` bitflags).
  #[inline]
  pub const fn disposition(&self) -> TrackDisposition {
    self.disposition
  }

  /// Primary-track flag.
  #[inline]
  pub const fn is_primary(&self) -> bool {
    self.is_primary
  }

  /// Auto-selected flag.
  #[inline]
  pub const fn auto_selected(&self) -> bool {
    self.auto_selected
  }

  /// Coarse content classification (Speech/Music/Mixed/Silence).
  #[inline]
  pub const fn content(&self) -> Option<AudioContentKind> {
    self.content
  }

  /// Fraction-speech estimate (drives the pipeline).
  #[inline]
  pub const fn speech_ratio(&self) -> Option<f32> {
    self.speech_ratio
  }

  /// Cheap defect signal.
  #[inline]
  pub const fn is_silent(&self) -> bool {
    self.is_silent
  }

  /// EBU R128 loudness (`None` = stage not run yet).
  #[inline]
  pub const fn loudness(&self) -> Option<&Loudness> {
    self.loudness.as_ref()
  }

  /// Chromaprint fingerprint (`None` = stage not run yet).
  #[inline]
  pub const fn fingerprint(&self) -> Option<&Fingerprint> {
    self.fingerprint.as_ref()
  }

  /// ISRC recording code (`""` = absent).
  #[inline]
  pub fn isrc(&self) -> &str {
    self.isrc.as_str()
  }

  /// AcoustID id (`""` = absent).
  #[inline]
  pub fn acoustid(&self) -> &str {
    self.acoustid.as_str()
  }

  /// MusicBrainz recording id (`""` = absent).
  #[inline]
  pub fn musicbrainz_recording_id(&self) -> &str {
    self.musicbrainz_recording_id.as_str()
  }

  /// The track's diarized speaker set (`Speaker` ids; voiceprint → LanceDB).
  /// Distinct-count = `speakers().len()`.
  #[inline]
  pub const fn speakers(&self) -> &[Id] {
    self.speakers.as_slice()
  }

  /// Per-recording music tags (`None` = no tags read yet).
  #[inline]
  pub const fn tags(&self) -> Option<&Tags> {
    self.tags.as_ref()
  }

  /// Per-recording embedded cover art (`None` = no art).
  #[inline]
  pub const fn cover_art(&self) -> Option<&CoverArt> {
    self.cover_art.as_ref()
  }

  /// Per-track `AudioSegment` ids (`Audio.total_segments` rolls these up).
  #[inline]
  pub const fn segments(&self) -> &[Id] {
    self.segments.as_slice()
  }

  /// Analysis-run reproducibility (per-track, one per run).
  #[inline]
  pub const fn provenance(&self) -> &Provenance {
    &self.provenance
  }

  /// 11-bit indexing state (the verified `ProcessingStage`).
  #[inline]
  pub const fn index_status(&self) -> AudioIndexStatus {
    self.index_status
  }

  /// Per-track index errors (stage-coded `ErrorInfo.code`). Error-state is
  /// derived from `(index_status, index_errors)` via `AudioIndexStage`.
  #[inline]
  pub fn index_errors(&self) -> &[ErrorInfo] {
    self.index_errors.as_slice()
  }

  // ----- Builders ----------------------------------------------------------

  /// Builder: replace `stream_index`.
  #[inline]
  pub const fn with_stream_index(mut self, v: Option<u32>) -> Self {
    self.stream_index = v;
    self
  }

  /// Builder: replace `container_track_id`.
  #[inline]
  pub const fn with_container_track_id(mut self, v: Option<u64>) -> Self {
    self.container_track_id = v;
    self
  }

  /// Builder: replace `codec`.
  #[inline]
  pub fn with_codec(mut self, v: AudioCodec) -> Self {
    self.codec = v;
    self
  }

  /// Builder: replace `profile`.
  #[inline]
  pub fn with_profile(mut self, v: impl Into<SmolStr>) -> Self {
    self.profile = v.into();
    self
  }

  /// Builder: replace `sample_rate`.
  #[inline]
  pub const fn with_sample_rate(mut self, hz: u32) -> Self {
    self.sample_rate = hz;
    self
  }

  /// Builder: replace `channels`.
  #[inline]
  pub const fn with_channels(mut self, channels: u16) -> Self {
    self.channels = channels;
    self
  }

  /// Builder: replace `channel_layout`.
  #[inline]
  pub fn with_channel_layout(mut self, v: ChannelLayout) -> Self {
    self.channel_layout = v;
    self
  }

  /// Builder: replace `bit_rate`.
  #[inline]
  pub const fn with_bit_rate(mut self, bps: u64) -> Self {
    self.bit_rate = bps;
    self
  }

  /// Builder: replace `bit_rate_mode`.
  #[inline]
  pub const fn with_bit_rate_mode(mut self, v: Option<BitRateMode>) -> Self {
    self.bit_rate_mode = v;
    self
  }

  /// Builder: replace `bits_per_sample`.
  #[inline]
  pub const fn with_bits_per_sample(mut self, v: Option<u16>) -> Self {
    self.bits_per_sample = v;
    self
  }

  /// Builder: replace `is_lossless`.
  #[inline]
  pub const fn with_lossless(mut self, v: bool) -> Self {
    self.is_lossless = v;
    self
  }

  /// Builder: replace `duration`.
  #[inline]
  pub fn with_duration(mut self, v: Option<Timestamp>) -> Self {
    self.duration = v;
    self
  }

  /// Builder: replace `start_pts`.
  #[inline]
  pub fn with_start_pts(mut self, v: Option<Timestamp>) -> Self {
    self.start_pts = v;
    self
  }

  /// Builder: replace `language`.
  #[inline]
  pub const fn with_language(mut self, v: Option<Language>) -> Self {
    self.language = v;
    self
  }

  /// Builder: replace `detected_language`.
  #[inline]
  pub const fn with_detected_language(mut self, v: Option<Language>) -> Self {
    self.detected_language = v;
    self
  }

  /// Builder: replace `language_mismatch`.
  #[inline]
  pub const fn with_language_mismatch(mut self, v: bool) -> Self {
    self.language_mismatch = v;
    self
  }

  /// Builder: replace `disposition` flags.
  #[inline]
  pub const fn with_disposition(mut self, v: TrackDisposition) -> Self {
    self.disposition = v;
    self
  }

  /// Builder: replace `is_primary`.
  #[inline]
  pub const fn with_primary(mut self, v: bool) -> Self {
    self.is_primary = v;
    self
  }

  /// Builder: replace `auto_selected`.
  #[inline]
  pub const fn with_auto_selected(mut self, v: bool) -> Self {
    self.auto_selected = v;
    self
  }

  /// Builder: replace `content`.
  #[inline]
  pub const fn with_content(mut self, v: Option<AudioContentKind>) -> Self {
    self.content = v;
    self
  }

  /// Builder: replace `speech_ratio`.
  #[inline]
  pub const fn with_speech_ratio(mut self, v: Option<f32>) -> Self {
    self.speech_ratio = v;
    self
  }

  /// Builder: replace `is_silent`.
  #[inline]
  pub const fn with_silent(mut self, v: bool) -> Self {
    self.is_silent = v;
    self
  }

  /// Builder: replace `loudness`.
  #[inline]
  pub const fn with_loudness(mut self, v: Option<Loudness>) -> Self {
    self.loudness = v;
    self
  }

  /// Builder: replace `fingerprint`.
  #[inline]
  pub fn with_fingerprint(mut self, v: Option<Fingerprint>) -> Self {
    self.fingerprint = v;
    self
  }

  /// Builder: replace `isrc`.
  #[inline]
  pub fn with_isrc(mut self, v: impl Into<SmolStr>) -> Self {
    self.isrc = v.into();
    self
  }

  /// Builder: replace `acoustid`.
  #[inline]
  pub fn with_acoustid(mut self, v: impl Into<SmolStr>) -> Self {
    self.acoustid = v.into();
    self
  }

  /// Builder: replace `musicbrainz_recording_id`.
  #[inline]
  pub fn with_musicbrainz_recording_id(mut self, v: impl Into<SmolStr>) -> Self {
    self.musicbrainz_recording_id = v.into();
    self
  }

  /// Builder: replace `speakers`.
  #[inline]
  pub fn with_speakers(mut self, v: impl Into<std::vec::Vec<Id>>) -> Self {
    self.speakers = v.into();
    self
  }

  /// Builder: replace `tags`.
  #[inline]
  pub fn with_tags(mut self, v: Option<Tags>) -> Self {
    self.tags = v;
    self
  }

  /// Builder: replace `cover_art`.
  #[inline]
  pub fn with_cover_art(mut self, v: Option<CoverArt>) -> Self {
    self.cover_art = v;
    self
  }

  /// Builder: replace `segments`.
  #[inline]
  pub fn with_segments(mut self, v: impl Into<std::vec::Vec<Id>>) -> Self {
    self.segments = v.into();
    self
  }

  /// Builder: replace `provenance`.
  #[inline]
  pub fn with_provenance(mut self, v: Provenance) -> Self {
    self.provenance = v;
    self
  }

  /// Builder: replace `index_status`.
  #[inline]
  pub const fn with_index_status(mut self, v: AudioIndexStatus) -> Self {
    self.index_status = v;
    self
  }

  /// Builder: replace `index_errors`.
  #[inline]
  pub fn with_index_errors(mut self, v: impl Into<std::vec::Vec<ErrorInfo>>) -> Self {
    self.index_errors = v.into();
    self
  }

  // ----- Setters -----------------------------------------------------------

  /// In-place mutator for `stream_index`.
  #[inline]
  pub const fn set_stream_index(&mut self, v: Option<u32>) {
    self.stream_index = v;
  }

  /// In-place mutator for `container_track_id`.
  #[inline]
  pub const fn set_container_track_id(&mut self, v: Option<u64>) {
    self.container_track_id = v;
  }

  /// In-place mutator for `codec`.
  #[inline]
  pub fn set_codec(&mut self, v: AudioCodec) {
    self.codec = v;
  }

  /// In-place mutator for `profile`.
  #[inline]
  pub fn set_profile(&mut self, v: impl Into<SmolStr>) {
    self.profile = v.into();
  }

  /// In-place mutator for `sample_rate`.
  #[inline]
  pub const fn set_sample_rate(&mut self, hz: u32) {
    self.sample_rate = hz;
  }

  /// In-place mutator for `channels`.
  #[inline]
  pub const fn set_channels(&mut self, channels: u16) {
    self.channels = channels;
  }

  /// In-place mutator for `channel_layout`.
  #[inline]
  pub fn set_channel_layout(&mut self, v: ChannelLayout) {
    self.channel_layout = v;
  }

  /// In-place mutator for `bit_rate`.
  #[inline]
  pub const fn set_bit_rate(&mut self, bps: u64) {
    self.bit_rate = bps;
  }

  /// In-place mutator for `bit_rate_mode`.
  #[inline]
  pub const fn set_bit_rate_mode(&mut self, v: Option<BitRateMode>) {
    self.bit_rate_mode = v;
  }

  /// In-place mutator for `bits_per_sample`.
  #[inline]
  pub const fn set_bits_per_sample(&mut self, v: Option<u16>) {
    self.bits_per_sample = v;
  }

  /// In-place mutator for `is_lossless`.
  #[inline]
  pub const fn set_lossless(&mut self, v: bool) {
    self.is_lossless = v;
  }

  /// In-place mutator for `duration`.
  #[inline]
  pub fn set_duration(&mut self, v: Option<Timestamp>) {
    self.duration = v;
  }

  /// In-place mutator for `start_pts`.
  #[inline]
  pub fn set_start_pts(&mut self, v: Option<Timestamp>) {
    self.start_pts = v;
  }

  /// In-place mutator for `language`.
  #[inline]
  pub const fn set_language(&mut self, v: Option<Language>) {
    self.language = v;
  }

  /// In-place mutator for `detected_language`.
  #[inline]
  pub const fn set_detected_language(&mut self, v: Option<Language>) {
    self.detected_language = v;
  }

  /// In-place mutator for `language_mismatch`.
  #[inline]
  pub const fn set_language_mismatch(&mut self, v: bool) {
    self.language_mismatch = v;
  }

  /// In-place mutator for `disposition`.
  #[inline]
  pub const fn set_disposition(&mut self, v: TrackDisposition) {
    self.disposition = v;
  }

  /// In-place mutator for `is_primary`.
  #[inline]
  pub const fn set_primary(&mut self, v: bool) {
    self.is_primary = v;
  }

  /// In-place mutator for `auto_selected`.
  #[inline]
  pub const fn set_auto_selected(&mut self, v: bool) {
    self.auto_selected = v;
  }

  /// In-place mutator for `content`.
  #[inline]
  pub const fn set_content(&mut self, v: Option<AudioContentKind>) {
    self.content = v;
  }

  /// In-place mutator for `speech_ratio`.
  #[inline]
  pub const fn set_speech_ratio(&mut self, v: Option<f32>) {
    self.speech_ratio = v;
  }

  /// In-place mutator for `is_silent`.
  #[inline]
  pub const fn set_silent(&mut self, v: bool) {
    self.is_silent = v;
  }

  /// In-place mutator for `loudness`.
  #[inline]
  pub const fn set_loudness(&mut self, v: Option<Loudness>) {
    self.loudness = v;
  }

  /// In-place mutator for `fingerprint`.
  #[inline]
  pub fn set_fingerprint(&mut self, v: Option<Fingerprint>) {
    self.fingerprint = v;
  }

  /// In-place mutator for `isrc`.
  #[inline]
  pub fn set_isrc(&mut self, v: impl Into<SmolStr>) {
    self.isrc = v.into();
  }

  /// In-place mutator for `acoustid`.
  #[inline]
  pub fn set_acoustid(&mut self, v: impl Into<SmolStr>) {
    self.acoustid = v.into();
  }

  /// In-place mutator for `musicbrainz_recording_id`.
  #[inline]
  pub fn set_musicbrainz_recording_id(&mut self, v: impl Into<SmolStr>) {
    self.musicbrainz_recording_id = v.into();
  }

  /// In-place mutator for `speakers`.
  #[inline]
  pub fn set_speakers(&mut self, v: impl Into<std::vec::Vec<Id>>) {
    self.speakers = v.into();
  }

  /// In-place mutator for `tags`.
  #[inline]
  pub fn set_tags(&mut self, v: Option<Tags>) {
    self.tags = v;
  }

  /// In-place mutator for `cover_art`.
  #[inline]
  pub fn set_cover_art(&mut self, v: Option<CoverArt>) {
    self.cover_art = v;
  }

  /// In-place mutator for `segments`.
  #[inline]
  pub fn set_segments(&mut self, v: impl Into<std::vec::Vec<Id>>) {
    self.segments = v.into();
  }

  /// In-place mutator for `provenance`.
  #[inline]
  pub fn set_provenance(&mut self, v: Provenance) {
    self.provenance = v;
  }

  /// In-place mutator for `index_status`.
  #[inline]
  pub const fn set_index_status(&mut self, v: AudioIndexStatus) {
    self.index_status = v;
  }

  /// In-place mutator for `index_errors`.
  #[inline]
  pub fn set_index_errors(&mut self, v: impl Into<std::vec::Vec<ErrorInfo>>) {
    self.index_errors = v.into();
  }
}

/// Error returned when [`AudioTrack::try_new`] cannot uphold the
/// non-nil-id / non-nil-parent invariants. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum AudioTrackError {
  /// Supplied `id` was the nil sentinel.
  #[error("AudioTrack id must not be the nil UUID")]
  NilId,
  /// Supplied `parent` was the nil sentinel — orphaned track with no
  /// `Audio` facet reference.
  #[error("AudioTrack parent (Audio) must not be the nil UUID")]
  NilParent,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use crate::domain::ErrorCode;

  #[test]
  fn try_new_happy_path() {
    let parent = Uuid7::new();
    let t = AudioTrack::try_new(Uuid7::new(), parent).expect("valid construction must succeed");
    assert_eq!(t.parent(), &parent);
    assert_eq!(t.sample_rate(), 0);
    assert_eq!(t.channels(), 0);
    assert!(t.codec().as_str().is_empty());
    assert!(t.tags().is_none());
    assert!(t.cover_art().is_none());
    assert!(t.speakers().is_empty());
    assert!(t.segments().is_empty());
    assert_eq!(t.index_status(), AudioIndexStatus::empty());
    assert!(t.provenance().is_empty());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = AudioTrack::try_new(Uuid7::nil(), Uuid7::new());
    assert_eq!(r.err(), Some(AudioTrackError::NilId));
    assert!(AudioTrackError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_parent() {
    let r = AudioTrack::try_new(Uuid7::new(), Uuid7::nil());
    assert_eq!(r.err(), Some(AudioTrackError::NilParent));
    assert!(AudioTrackError::NilParent.is_nil_parent());
  }

  #[test]
  fn descriptor_builders_chain() {
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(AudioCodec::Aac)
      .with_profile("LC")
      .with_sample_rate(48_000)
      .with_channels(2)
      .with_channel_layout(ChannelLayout::Stereo)
      .with_bit_rate(192_000)
      .with_lossless(false)
      .with_primary(true);
    assert_eq!(t.codec(), &AudioCodec::Aac);
    assert_eq!(t.codec().as_str(), "aac");
    assert_eq!(t.profile(), "LC");
    assert_eq!(t.sample_rate(), 48_000);
    assert_eq!(t.channels(), 2);
    assert_eq!(t.channel_layout(), &ChannelLayout::Stereo);
    assert_eq!(t.channel_layout().as_str(), "stereo");
    assert_eq!(t.bit_rate(), 192_000);
    assert!(!t.is_lossless());
    assert!(t.is_primary());
  }

  #[test]
  fn tags_and_cover_art_attach() {
    let tags = Tags::new()
      .with_title("Track 1")
      .with_artist("Artist A")
      .with_album("Album X")
      .with_track_number(1)
      .with_track_total(12);
    let cover = CoverArt::try_new("image/jpeg", std::vec![0xFFu8, 0xD8, 0xFF]).unwrap();
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_tags(Some(tags))
      .with_cover_art(Some(cover));
    let tags = t.tags().expect("tags attached");
    assert_eq!(tags.title(), "Track 1");
    assert_eq!(tags.artist(), "Artist A");
    assert_eq!(tags.track_number(), Some(1));
    assert_eq!(tags.track_total(), Some(12));
    let cover = t.cover_art().expect("cover attached");
    assert_eq!(cover.mime(), "image/jpeg");
    assert_eq!(cover.data(), &[0xFFu8, 0xD8, 0xFF]);
  }

  #[test]
  fn loudness_and_fingerprint_attach() {
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_loudness(Some(Loudness::new(-23.0, 7.5, -1.0, -3.0)))
      .with_fingerprint(Some(
        Fingerprint::try_new("chromaprint", std::vec![1u8, 2, 3, 4]).unwrap(),
      ));
    let l = t.loudness().expect("loudness present");
    assert!((l.integrated_lufs() - -23.0).abs() < f32::EPSILON);
    assert!((l.true_peak_dbtp() - -1.0).abs() < f32::EPSILON);
    assert!((l.range_lu() - 7.5).abs() < f32::EPSILON);
    let fp = t.fingerprint().expect("fingerprint present");
    assert_eq!(fp.algorithm(), "chromaprint");
    assert_eq!(fp.value(), &[1u8, 2, 3, 4]);
  }

  #[test]
  fn provenance_is_per_track() {
    let prov = Provenance::from_parts("asry", "1.2.3", "v0", "indexer-0.4");
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_provenance(prov.clone());
    assert_eq!(t.provenance(), &prov);
    assert_eq!(t.provenance().model_name(), "asry");
  }

  #[test]
  fn index_status_and_errors_roundtrip() {
    let err = ErrorInfo::new(ErrorCode::ProbeCorrupt, "could not probe");
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_index_status(AudioIndexStatus::EXTRACTED | AudioIndexStatus::VAD_DONE)
      .with_index_errors(std::vec![err.clone()]);
    assert!(t.index_status().contains(AudioIndexStatus::EXTRACTED));
    assert!(t.index_status().contains(AudioIndexStatus::VAD_DONE));
    assert_eq!(t.index_errors().len(), 1);
    assert_eq!(t.index_errors()[0], err);
  }

  #[test]
  fn speakers_and_segments_lists() {
    let s1 = Uuid7::new();
    let g1 = Uuid7::new();
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_speakers(std::vec![s1])
      .with_segments(std::vec![g1]);
    assert_eq!(t.speakers(), &[s1]);
    assert_eq!(t.segments(), &[g1]);
  }

  #[test]
  fn setters_mutate_in_place() {
    let mut t = AudioTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    t.set_codec(AudioCodec::Opus);
    t.set_sample_rate(48_000);
    t.set_channels(2);
    t.set_lossless(false);
    t.set_silent(true);
    t.set_content(Some(AudioContentKind::Music));
    t.set_index_status(AudioIndexStatus::EXTRACTED);
    assert_eq!(t.codec(), &AudioCodec::Opus);
    assert_eq!(t.sample_rate(), 48_000);
    assert_eq!(t.channels(), 2);
    assert!(!t.is_lossless());
    assert!(t.is_silent());
    assert_eq!(t.content(), Some(AudioContentKind::Music));
    assert_eq!(t.index_status(), AudioIndexStatus::EXTRACTED);
  }
}
