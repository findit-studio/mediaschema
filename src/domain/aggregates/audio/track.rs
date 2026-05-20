//! `AudioTrack` — one audio stream of an `Audio` facet.
//!
//! Locked `schema/audio_track.md` rev 3. A multi-track audio file = N
//! distinct recordings, so per-recording music metadata (tags + cover art)
//! lives **here**, not on a file/facet. Holds codec/stream descriptor,
//! per-track signal analysis (loudness/fingerprint), per-track indexing
//! state + provenance, the diarized-speaker set, and (A-loc per-track) the
//! per-track segments refs → `AudioSegment`.
//!
//! ## mediaframe placeholders
//!
//! The locked spec types several fields as `mediaframe::*` externs
//! (`AudioCodec`, `ChannelLayout`, `Language`, `TrackDisposition`,
//! `BitRateMode`, `Dimensions`). `mediaframe` is not yet a dependency of
//! `mediaschema` (see `schema/mediaframe-candidates.md`). Until that crate
//! is on crates.io we model those fields as wire-layer-compatible
//! placeholders (`SmolStr` / `u32` / `u64` / `Option<…>`) and flag each
//! site with `TODO(mediaframe)` so the substitution is mechanical when
//! the crate lands.

use derive_more::IsVariant;
use mediatime::Timestamp;
use smol_str::SmolStr;

use crate::domain::{
  bitflags::AudioIndexStatus, enums::AudioContentKind, primitives::ErrorInfo, vo::Provenance, Uuid7,
};

// ---------------------------------------------------------------------------
// Nested value objects (per locked spec §Nested value-objects)
// ---------------------------------------------------------------------------

/// Per-recording music tags (~16 flat ID3/Vorbis-style fields, grouped).
///
/// Locked `audio_track.md` §Nested value-objects. Free-text fields use
/// `SmolStr` (`""` = absent, never `Option`). Numeric counts use `u32`
/// (`0` = absent).
///
/// **Default convention**: `Default::default()` calls [`AudioTags::new`]
/// (the all-empty record).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioTags {
  title: SmolStr,
  artist: SmolStr,
  album_artist: SmolStr,
  album: SmolStr,
  genre: SmolStr,
  composer: SmolStr,
  performer: SmolStr,
  date: SmolStr,
  track_number: u32,
  total_tracks: u32,
  disc_number: u32,
  total_discs: u32,
  comment: SmolStr,
  lyrics: SmolStr,
  tag_types: std::vec::Vec<SmolStr>,
}

impl AudioTags {
  /// Canonical no-arg constructor — every field empty/zero.
  /// [`Default::default`] is `Self::new()`.
  #[inline]
  pub fn new() -> Self {
    Self {
      title: SmolStr::default(),
      artist: SmolStr::default(),
      album_artist: SmolStr::default(),
      album: SmolStr::default(),
      genre: SmolStr::default(),
      composer: SmolStr::default(),
      performer: SmolStr::default(),
      date: SmolStr::default(),
      track_number: 0,
      total_tracks: 0,
      disc_number: 0,
      total_discs: 0,
      comment: SmolStr::default(),
      lyrics: SmolStr::default(),
      tag_types: std::vec::Vec::new(),
    }
  }

  /// Track title (`""` = absent).
  #[inline]
  pub fn title(&self) -> &str {
    self.title.as_str()
  }

  /// Performing artist (`""` = absent).
  #[inline]
  pub fn artist(&self) -> &str {
    self.artist.as_str()
  }

  /// Album artist (`""` = absent).
  #[inline]
  pub fn album_artist(&self) -> &str {
    self.album_artist.as_str()
  }

  /// Album name (`""` = absent).
  #[inline]
  pub fn album(&self) -> &str {
    self.album.as_str()
  }

  /// Genre (`""` = absent).
  #[inline]
  pub fn genre(&self) -> &str {
    self.genre.as_str()
  }

  /// Composer (`""` = absent).
  #[inline]
  pub fn composer(&self) -> &str {
    self.composer.as_str()
  }

  /// Performer (`""` = absent).
  #[inline]
  pub fn performer(&self) -> &str {
    self.performer.as_str()
  }

  /// Release date / year (`""` = absent).
  #[inline]
  pub fn date(&self) -> &str {
    self.date.as_str()
  }

  /// Track number on disc (`0` = absent).
  #[inline]
  pub const fn track_number(&self) -> u32 {
    self.track_number
  }

  /// Total tracks on disc (`0` = absent).
  #[inline]
  pub const fn total_tracks(&self) -> u32 {
    self.total_tracks
  }

  /// Disc number in set (`0` = absent).
  #[inline]
  pub const fn disc_number(&self) -> u32 {
    self.disc_number
  }

  /// Total discs in set (`0` = absent).
  #[inline]
  pub const fn total_discs(&self) -> u32 {
    self.total_discs
  }

  /// Free-text comment (`""` = absent).
  #[inline]
  pub fn comment(&self) -> &str {
    self.comment.as_str()
  }

  /// Lyrics (`""` = absent).
  #[inline]
  pub fn lyrics(&self) -> &str {
    self.lyrics.as_str()
  }

  /// Tag schemas seen in the source container (e.g. `ID3v2`, `Vorbis`).
  #[inline]
  pub fn tag_types(&self) -> &[SmolStr] {
    self.tag_types.as_slice()
  }

  /// Builder: replace `title`.
  #[inline]
  pub fn with_title(mut self, v: impl Into<SmolStr>) -> Self {
    self.title = v.into();
    self
  }

  /// Builder: replace `artist`.
  #[inline]
  pub fn with_artist(mut self, v: impl Into<SmolStr>) -> Self {
    self.artist = v.into();
    self
  }

  /// Builder: replace `album_artist`.
  #[inline]
  pub fn with_album_artist(mut self, v: impl Into<SmolStr>) -> Self {
    self.album_artist = v.into();
    self
  }

  /// Builder: replace `album`.
  #[inline]
  pub fn with_album(mut self, v: impl Into<SmolStr>) -> Self {
    self.album = v.into();
    self
  }

  /// Builder: replace `genre`.
  #[inline]
  pub fn with_genre(mut self, v: impl Into<SmolStr>) -> Self {
    self.genre = v.into();
    self
  }

  /// Builder: replace `composer`.
  #[inline]
  pub fn with_composer(mut self, v: impl Into<SmolStr>) -> Self {
    self.composer = v.into();
    self
  }

  /// Builder: replace `performer`.
  #[inline]
  pub fn with_performer(mut self, v: impl Into<SmolStr>) -> Self {
    self.performer = v.into();
    self
  }

  /// Builder: replace `date`.
  #[inline]
  pub fn with_date(mut self, v: impl Into<SmolStr>) -> Self {
    self.date = v.into();
    self
  }

  /// Builder: replace `track_number`.
  #[inline]
  pub const fn with_track_number(mut self, v: u32) -> Self {
    self.track_number = v;
    self
  }

  /// Builder: replace `total_tracks`.
  #[inline]
  pub const fn with_total_tracks(mut self, v: u32) -> Self {
    self.total_tracks = v;
    self
  }

  /// Builder: replace `disc_number`.
  #[inline]
  pub const fn with_disc_number(mut self, v: u32) -> Self {
    self.disc_number = v;
    self
  }

  /// Builder: replace `total_discs`.
  #[inline]
  pub const fn with_total_discs(mut self, v: u32) -> Self {
    self.total_discs = v;
    self
  }

  /// Builder: replace `comment`.
  #[inline]
  pub fn with_comment(mut self, v: impl Into<SmolStr>) -> Self {
    self.comment = v.into();
    self
  }

  /// Builder: replace `lyrics`.
  #[inline]
  pub fn with_lyrics(mut self, v: impl Into<SmolStr>) -> Self {
    self.lyrics = v.into();
    self
  }

  /// Builder: replace `tag_types`.
  #[inline]
  pub fn with_tag_types(mut self, v: impl Into<std::vec::Vec<SmolStr>>) -> Self {
    self.tag_types = v.into();
    self
  }
}

impl Default for AudioTags {
  #[inline]
  fn default() -> Self {
    Self::new()
  }
}

/// Per-recording embedded cover art (inline `Bytes`).
///
/// Locked `audio_track.md` §Nested value-objects. Mirrors locked
/// `Keyframe.data` (inline, no `Location`). `size` is dropped — derived
/// from `data.len()`.
///
/// TODO(mediaframe): `dimensions: Option<mediaframe::Dimensions>` — until
/// `mediaframe` ships, the field is dropped from the placeholder. Add it
/// back when `mediaframe::Dimensions` is available.
///
/// TODO(mediaframe): the locked spec types `data: Bytes`
/// (`bytes::Bytes` — currently only reachable via `buffa`'s re-export).
/// We use `std::vec::Vec<u8>` as the no-extra-dep placeholder; the
/// substitution is mechanical when a shared `bytes` dependency lands.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioCoverArt {
  data: std::vec::Vec<u8>,
  mime: SmolStr,
}

impl AudioCoverArt {
  /// Canonical no-arg constructor — empty payload.
  #[inline]
  pub fn new() -> Self {
    Self {
      data: std::vec::Vec::new(),
      mime: SmolStr::default(),
    }
  }

  /// Construct from explicit data + MIME type.
  #[inline]
  pub fn from_parts(data: impl Into<std::vec::Vec<u8>>, mime: impl Into<SmolStr>) -> Self {
    Self {
      data: data.into(),
      mime: mime.into(),
    }
  }

  /// Raw image bytes (inline; mirrors `Keyframe.data`).
  #[inline]
  pub fn data(&self) -> &[u8] {
    self.data.as_slice()
  }

  /// IANA MIME type (`""` = absent / unknown).
  #[inline]
  pub fn mime(&self) -> &str {
    self.mime.as_str()
  }

  /// Convenience: `data.len()` (the locked spec drops the explicit `size`
  /// field — derived).
  #[inline]
  pub const fn size(&self) -> usize {
    self.data.len()
  }

  /// Builder: replace `data`.
  #[inline]
  pub fn with_data(mut self, data: impl Into<std::vec::Vec<u8>>) -> Self {
    self.data = data.into();
    self
  }

  /// Builder: replace `mime`.
  #[inline]
  pub fn with_mime(mut self, mime: impl Into<SmolStr>) -> Self {
    self.mime = mime.into();
    self
  }
}

impl Default for AudioCoverArt {
  #[inline]
  fn default() -> Self {
    Self::new()
  }
}

/// EBU R128 loudness measurements (integrated LUFS + true peak + range LU).
///
/// Locked `audio_track.md` §Nested value-objects. All three values are
/// always present together when the EBU R128 stage has run; whole-VO
/// absence is modelled at the `AudioTrack` site via `Option<Loudness>`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loudness {
  integrated_lufs: f32,
  true_peak_dbtp: f32,
  loudness_range_lu: f32,
}

impl Loudness {
  /// Construct from the three EBU R128 values.
  #[inline]
  pub const fn new(integrated_lufs: f32, true_peak_dbtp: f32, loudness_range_lu: f32) -> Self {
    Self {
      integrated_lufs,
      true_peak_dbtp,
      loudness_range_lu,
    }
  }

  /// Integrated LUFS (program loudness).
  #[inline]
  pub const fn integrated_lufs(&self) -> f32 {
    self.integrated_lufs
  }

  /// True peak (dBTP).
  #[inline]
  pub const fn true_peak_dbtp(&self) -> f32 {
    self.true_peak_dbtp
  }

  /// Loudness range (LU).
  #[inline]
  pub const fn loudness_range_lu(&self) -> f32 {
    self.loudness_range_lu
  }
}

/// Acoustic fingerprint (chromaprint) for recording dedup / AcoustID
/// lookup.
///
/// Locked `audio_track.md` §Nested value-objects. `duration_s` is dropped
/// (locked: redundant with `AudioTrack.duration`).
///
/// TODO(mediaframe): the locked spec types `value: Bytes` (`bytes::Bytes`).
/// We use `std::vec::Vec<u8>` as the no-extra-dep placeholder.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioFingerprint {
  algo: SmolStr,
  value: std::vec::Vec<u8>,
}

impl AudioFingerprint {
  /// Construct from explicit algo + value.
  #[inline]
  pub fn from_parts(algo: impl Into<SmolStr>, value: impl Into<std::vec::Vec<u8>>) -> Self {
    Self {
      algo: algo.into(),
      value: value.into(),
    }
  }

  /// Algorithm name (e.g. `"chromaprint"`).
  #[inline]
  pub fn algo(&self) -> &str {
    self.algo.as_str()
  }

  /// Raw fingerprint bytes.
  #[inline]
  pub fn value(&self) -> &[u8] {
    self.value.as_slice()
  }
}

// ---------------------------------------------------------------------------
// AudioTrack
// ---------------------------------------------------------------------------

/// One audio stream of an `Audio` facet (`parent → Audio.id`).
///
/// Generic over `Id` (default [`Uuid7`]). See module docs for the
/// `mediaframe`-extern placeholders.
///
/// **No `Default`** — defaulting to a nil id + nil parent is an orphan
/// state. Use [`AudioTrack::try_new`].
#[derive(Debug, Clone, PartialEq)]
pub struct AudioTrack<Id = Uuid7> {
  id: Id,
  parent: Id,
  stream_index: Option<u32>,
  container_track_id: Option<u64>,
  // TODO(mediaframe): `codec: mediaframe::AudioCodec` extern. Use `SmolStr`
  // placeholder (`""` = absent / `Other`-style fallback). Replace when
  // `mediaframe::AudioCodec` lands.
  codec: SmolStr,
  profile: SmolStr,
  sample_rate: u32,
  channels: u16,
  // TODO(mediaframe): `channel_layout: mediaframe::ChannelLayout` extern.
  channel_layout: SmolStr,
  bit_rate: u64,
  // TODO(mediaframe): `bit_rate_mode: Option<mediaframe::BitRateMode>` —
  // `Cbr`/`Vbr`/`Abr`. Placeholder = `Option<SmolStr>`.
  bit_rate_mode: Option<SmolStr>,
  bits_per_sample: Option<u16>,
  is_lossless: bool,
  // TODO(mediaframe): `duration: Option<mediatime::TrackTime>` — `mediatime`
  // 0.1.6 publicly exports only `Timestamp`/`TimeRange`/`Timebase` (no
  // `TrackTime`). Same workaround as `Speaker.speech_duration`:
  // `mediatime::Timestamp` treated as a track-relative offset/duration.
  duration: Option<Timestamp>,
  start_pts: Option<Timestamp>,
  // TODO(mediaframe): `language: Option<mediaframe::Language>` (BCP-47).
  // Placeholder = `Option<SmolStr>` (the raw BCP-47 string).
  language: Option<SmolStr>,
  // TODO(mediaframe): `detected_language: Option<mediaframe::Language>`.
  detected_language: Option<SmolStr>,
  language_mismatch: bool,
  // TODO(mediaframe): `disposition: mediaframe::TrackDisposition` (bitflags).
  // Placeholder = raw `u32` bits.
  disposition: u32,
  is_primary: bool,
  auto_selected: bool,
  content: Option<AudioContentKind>,
  speech_ratio: Option<f32>,
  is_silent: bool,
  loudness: Option<Loudness>,
  fingerprint: Option<AudioFingerprint>,
  isrc: SmolStr,
  acoustid: SmolStr,
  musicbrainz_recording_id: SmolStr,
  speakers: std::vec::Vec<Id>,
  tags: Option<AudioTags>,
  cover_art: Option<AudioCoverArt>,
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
      codec: SmolStr::default(),
      profile: SmolStr::default(),
      sample_rate: 0,
      channels: 0,
      channel_layout: SmolStr::default(),
      bit_rate: 0,
      bit_rate_mode: None,
      bits_per_sample: None,
      is_lossless: false,
      duration: None,
      start_pts: None,
      language: None,
      detected_language: None,
      language_mismatch: false,
      disposition: 0,
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

  /// Codec name placeholder (`""` = absent). TODO(mediaframe).
  #[inline]
  pub fn codec(&self) -> &str {
    self.codec.as_str()
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

  /// Channel-layout placeholder (`""` = absent). TODO(mediaframe).
  #[inline]
  pub fn channel_layout(&self) -> &str {
    self.channel_layout.as_str()
  }

  /// Bit rate (bits/s; `0` = unknown).
  #[inline]
  pub const fn bit_rate(&self) -> u64 {
    self.bit_rate
  }

  /// Bit-rate mode placeholder (`Cbr`/`Vbr`/`Abr`; `None` = unknown).
  /// TODO(mediaframe).
  #[inline]
  pub fn bit_rate_mode(&self) -> Option<&str> {
    self.bit_rate_mode.as_deref()
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

  /// Declared language (BCP-47 placeholder string). TODO(mediaframe).
  #[inline]
  pub fn language(&self) -> Option<&str> {
    self.language.as_deref()
  }

  /// Whisper-LID detected language (BCP-47 placeholder). TODO(mediaframe).
  #[inline]
  pub fn detected_language(&self) -> Option<&str> {
    self.detected_language.as_deref()
  }

  /// `detected ≠ declared` (derived).
  #[inline]
  pub const fn language_mismatch(&self) -> bool {
    self.language_mismatch
  }

  /// Disposition flag bits (`mediaframe::TrackDisposition` placeholder).
  /// TODO(mediaframe).
  #[inline]
  pub const fn disposition(&self) -> u32 {
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
  pub const fn fingerprint(&self) -> Option<&AudioFingerprint> {
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
  pub const fn tags(&self) -> Option<&AudioTags> {
    self.tags.as_ref()
  }

  /// Per-recording embedded cover art (`None` = no art).
  #[inline]
  pub const fn cover_art(&self) -> Option<&AudioCoverArt> {
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

  /// Builder: replace `codec` placeholder. TODO(mediaframe).
  #[inline]
  pub fn with_codec(mut self, v: impl Into<SmolStr>) -> Self {
    self.codec = v.into();
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

  /// Builder: replace `channel_layout` placeholder. TODO(mediaframe).
  #[inline]
  pub fn with_channel_layout(mut self, v: impl Into<SmolStr>) -> Self {
    self.channel_layout = v.into();
    self
  }

  /// Builder: replace `bit_rate`.
  #[inline]
  pub const fn with_bit_rate(mut self, bps: u64) -> Self {
    self.bit_rate = bps;
    self
  }

  /// Builder: replace `bit_rate_mode` placeholder. TODO(mediaframe).
  #[inline]
  pub fn with_bit_rate_mode(mut self, v: Option<SmolStr>) -> Self {
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

  /// Builder: replace `language` placeholder. TODO(mediaframe).
  #[inline]
  pub fn with_language(mut self, v: Option<SmolStr>) -> Self {
    self.language = v;
    self
  }

  /// Builder: replace `detected_language` placeholder. TODO(mediaframe).
  #[inline]
  pub fn with_detected_language(mut self, v: Option<SmolStr>) -> Self {
    self.detected_language = v;
    self
  }

  /// Builder: replace `language_mismatch`.
  #[inline]
  pub const fn with_language_mismatch(mut self, v: bool) -> Self {
    self.language_mismatch = v;
    self
  }

  /// Builder: replace `disposition` bits. TODO(mediaframe).
  #[inline]
  pub const fn with_disposition(mut self, bits: u32) -> Self {
    self.disposition = bits;
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
  pub fn with_fingerprint(mut self, v: Option<AudioFingerprint>) -> Self {
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
  pub fn with_tags(mut self, v: Option<AudioTags>) -> Self {
    self.tags = v;
    self
  }

  /// Builder: replace `cover_art`.
  #[inline]
  pub fn with_cover_art(mut self, v: Option<AudioCoverArt>) -> Self {
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

  /// In-place mutator for `codec`. TODO(mediaframe).
  #[inline]
  pub fn set_codec(&mut self, v: impl Into<SmolStr>) {
    self.codec = v.into();
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

  /// In-place mutator for `channel_layout`. TODO(mediaframe).
  #[inline]
  pub fn set_channel_layout(&mut self, v: impl Into<SmolStr>) {
    self.channel_layout = v.into();
  }

  /// In-place mutator for `bit_rate`.
  #[inline]
  pub const fn set_bit_rate(&mut self, bps: u64) {
    self.bit_rate = bps;
  }

  /// In-place mutator for `bit_rate_mode`. TODO(mediaframe).
  #[inline]
  pub fn set_bit_rate_mode(&mut self, v: Option<SmolStr>) {
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

  /// In-place mutator for `language`. TODO(mediaframe).
  #[inline]
  pub fn set_language(&mut self, v: Option<SmolStr>) {
    self.language = v;
  }

  /// In-place mutator for `detected_language`. TODO(mediaframe).
  #[inline]
  pub fn set_detected_language(&mut self, v: Option<SmolStr>) {
    self.detected_language = v;
  }

  /// In-place mutator for `language_mismatch`.
  #[inline]
  pub const fn set_language_mismatch(&mut self, v: bool) {
    self.language_mismatch = v;
  }

  /// In-place mutator for `disposition`. TODO(mediaframe).
  #[inline]
  pub const fn set_disposition(&mut self, bits: u32) {
    self.disposition = bits;
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
  pub fn set_fingerprint(&mut self, v: Option<AudioFingerprint>) {
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
  pub fn set_tags(&mut self, v: Option<AudioTags>) {
    self.tags = v;
  }

  /// In-place mutator for `cover_art`.
  #[inline]
  pub fn set_cover_art(&mut self, v: Option<AudioCoverArt>) {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant)]
#[non_exhaustive]
pub enum AudioTrackError {
  /// Supplied `id` was the nil sentinel.
  NilId,
  /// Supplied `parent` was the nil sentinel — orphaned track with no
  /// `Audio` facet reference.
  NilParent,
}

impl core::fmt::Display for AudioTrackError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::NilId => f.write_str("AudioTrack id must not be the nil UUID"),
      Self::NilParent => f.write_str("AudioTrack parent (Audio) must not be the nil UUID"),
    }
  }
}

impl core::error::Error for AudioTrackError {}

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
    assert!(t.codec().is_empty());
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
      .with_codec("aac")
      .with_profile("LC")
      .with_sample_rate(48_000)
      .with_channels(2)
      .with_channel_layout("stereo")
      .with_bit_rate(192_000)
      .with_lossless(false)
      .with_primary(true);
    assert_eq!(t.codec(), "aac");
    assert_eq!(t.profile(), "LC");
    assert_eq!(t.sample_rate(), 48_000);
    assert_eq!(t.channels(), 2);
    assert_eq!(t.channel_layout(), "stereo");
    assert_eq!(t.bit_rate(), 192_000);
    assert!(!t.is_lossless());
    assert!(t.is_primary());
  }

  #[test]
  fn tags_and_cover_art_attach() {
    let tags = AudioTags::new()
      .with_title("Track 1")
      .with_artist("Artist A")
      .with_album("Album X")
      .with_track_number(1)
      .with_total_tracks(12)
      .with_tag_types(std::vec![SmolStr::from("ID3v2")]);
    let cover = AudioCoverArt::from_parts(std::vec![0xFFu8, 0xD8, 0xFF], "image/jpeg");
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_tags(Some(tags))
      .with_cover_art(Some(cover));
    let tags = t.tags().expect("tags attached");
    assert_eq!(tags.title(), "Track 1");
    assert_eq!(tags.artist(), "Artist A");
    assert_eq!(tags.track_number(), 1);
    assert_eq!(tags.tag_types(), &[SmolStr::from("ID3v2")]);
    let cover = t.cover_art().expect("cover attached");
    assert_eq!(cover.mime(), "image/jpeg");
    assert_eq!(cover.size(), 3);
    assert_eq!(cover.data(), &[0xFFu8, 0xD8, 0xFF]);
  }

  #[test]
  fn loudness_and_fingerprint_attach() {
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_loudness(Some(Loudness::new(-23.0, -1.0, 7.5)))
      .with_fingerprint(Some(AudioFingerprint::from_parts(
        "chromaprint",
        std::vec![1u8, 2, 3, 4],
      )));
    let l = t.loudness().expect("loudness present");
    assert!((l.integrated_lufs() - -23.0).abs() < f32::EPSILON);
    assert!((l.true_peak_dbtp() - -1.0).abs() < f32::EPSILON);
    assert!((l.loudness_range_lu() - 7.5).abs() < f32::EPSILON);
    let fp = t.fingerprint().expect("fingerprint present");
    assert_eq!(fp.algo(), "chromaprint");
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
    t.set_codec("opus");
    t.set_sample_rate(48_000);
    t.set_channels(2);
    t.set_lossless(false);
    t.set_silent(true);
    t.set_content(Some(AudioContentKind::Music));
    t.set_index_status(AudioIndexStatus::EXTRACTED);
    assert_eq!(t.codec(), "opus");
    assert_eq!(t.sample_rate(), 48_000);
    assert_eq!(t.channels(), 2);
    assert!(!t.is_lossless());
    assert!(t.is_silent());
    assert_eq!(t.content(), Some(AudioContentKind::Music));
    assert_eq!(t.index_status(), AudioIndexStatus::EXTRACTED);
  }
}
