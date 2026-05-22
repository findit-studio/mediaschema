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
// Ratio validation — shared by `speech_ratio`'s validating mutators
// ---------------------------------------------------------------------------

/// An optional `[0,1]`-bounded fraction is valid iff it is absent, or its
/// `Some(_)` value is finite (no NaN / ±∞) and within the closed unit
/// interval. `f32::is_finite` already excludes NaN and infinities.
#[inline]
const fn is_valid_ratio(v: Option<f32>) -> bool {
  match v {
    None => true,
    Some(r) => r.is_finite() && r >= 0.0 && r <= 1.0,
  }
}

/// `duration` is semantically a non-negative track-relative length. A
/// `mediatime::Timestamp` is negative iff its `pts()` is negative — the
/// timebase numerator/denominator are always positive, so the sign is
/// carried entirely by the PTS value. `None` (absent) is not negative.
/// Mirrors `Speaker`'s `is_negative_duration` check. (`start_pts` is a
/// different field: negative offsets are valid there and it is not gated.)
#[inline]
const fn is_negative_duration(d: Option<Timestamp>) -> bool {
  match d {
    None => false,
    Some(ts) => ts.pts() < 0,
  }
}

/// A status that includes [`AudioIndexStatus::EXTRACTED`] (or any later
/// bit, which the pipeline only sets *after* extraction) asserts the track
/// has been probed — and the locked `audio_track.md` invariant requires a
/// probed track to carry a real descriptor (`sample_rate > 0`,
/// `channels > 0`). A status with no bit at or past `EXTRACTED` makes no
/// such claim. `EXTRACTED` is the lowest bit, so "extracted-or-later" is
/// simply "any bit set".
#[inline]
const fn status_asserts_descriptor(s: AudioIndexStatus) -> bool {
  !s.is_empty()
}

/// Validates that an [`AudioIndexStatus`] mask is *topologically* consistent
/// with the contiguous `AudioIndexStage` lifecycle.
///
/// `AudioIndexStage::from_status` treats a later stage bit set without its
/// prerequisites as if the track were still `Pending` — so a raw status that
/// sets, say, `STT_DONE` without `EXTRACTED`/`VAD_DONE` would silently
/// disagree with the derived stage. Persisting such a mask is rejected here.
///
/// The prerequisite chain mirrors `AudioIndexStage::from_status` exactly:
///
/// * any non-empty mask must include `EXTRACTED` (the probe bit every later
///   stage builds on);
/// * `STT_DONE` requires the analyzed stage — at least one of
///   `CLASSIFIED` / `VAD_DONE`;
/// * `SPEAKER_DONE` requires `STT_DONE`;
/// * `TEXT_EMBED` requires `SPEAKER_DONE`.
///
/// The secondary bits (`LLM_DONE` / `CED_DONE` / `CLAP_DONE` /
/// `EBUR128_DONE` / `FPRINT_DONE`) are folded into `Done` and gate nothing
/// in the contiguous walk beyond `EXTRACTED`, so only the `EXTRACTED`
/// requirement applies to them.
const fn validate_status_topology(s: AudioIndexStatus) -> Result<(), AudioTrackError> {
  use AudioIndexStatus as S;
  // Reject any bit outside the declared `AudioIndexStatus` mask before the
  // lifecycle checks. `bitflags` retains unknown bits on construction, so a
  // caller could otherwise smuggle a bit the domain does not understand
  // (e.g. `EXTRACTED | 0x800`) past every topology check below.
  if s.bits() & !S::all().bits() != 0 {
    return Err(AudioTrackError::UnknownStatusBits);
  }
  // The empty mask makes no lifecycle claim and is always valid.
  if s.is_empty() {
    return Ok(());
  }
  // Every non-empty mask must carry the probe bit.
  if !s.contains(S::EXTRACTED) {
    return Err(AudioTrackError::StatusOutOfOrder);
  }
  // `STT_DONE` requires the analyzed stage (CLASSIFIED or VAD_DONE).
  if s.contains(S::STT_DONE) && !s.intersects(S::CLASSIFIED.union(S::VAD_DONE)) {
    return Err(AudioTrackError::StatusOutOfOrder);
  }
  // `SPEAKER_DONE` requires `STT_DONE`.
  if s.contains(S::SPEAKER_DONE) && !s.contains(S::STT_DONE) {
    return Err(AudioTrackError::StatusOutOfOrder);
  }
  // `TEXT_EMBED` requires `SPEAKER_DONE`.
  if s.contains(S::TEXT_EMBED) && !s.contains(S::SPEAKER_DONE) {
    return Err(AudioTrackError::StatusOutOfOrder);
  }
  Ok(())
}

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

  /// Validating builder: replace the diarized `speakers` set.
  ///
  /// The speaker set is a *set*: the distinct-speaker count is
  /// `speakers().len()`, so a duplicated id would inflate it
  /// ([`AudioTrackError::DuplicateSpeaker`]). A nil `Uuid7` is not a real
  /// `Speaker` identity and is rejected too
  /// ([`AudioTrackError::NilSpeaker`]). On rejection `self` is returned
  /// unchanged inside the `Err`.
  pub fn try_with_speakers(
    mut self,
    v: impl Into<std::vec::Vec<Uuid7>>,
  ) -> Result<Self, AudioTrackError> {
    let speakers = v.into();
    validate_speaker_set(&speakers)?;
    self.speakers = speakers;
    Ok(self)
  }

  /// Validating in-place mutator for the diarized `speakers` set. Rejects
  /// a nil id ([`AudioTrackError::NilSpeaker`]) or a duplicate id
  /// ([`AudioTrackError::DuplicateSpeaker`]); on rejection `self` is left
  /// unchanged.
  pub fn try_set_speakers(
    &mut self,
    v: impl Into<std::vec::Vec<Uuid7>>,
  ) -> Result<&mut Self, AudioTrackError> {
    let speakers = v.into();
    validate_speaker_set(&speakers)?;
    self.speakers = speakers;
    Ok(self)
  }
}

/// Validates a diarized speaker set for the canonical `Uuid7` identity:
/// no nil ids, no duplicates. `O(n²)` is fine — a track's distinct speaker
/// count is small.
fn validate_speaker_set(speakers: &[Uuid7]) -> Result<(), AudioTrackError> {
  for (i, id) in speakers.iter().enumerate() {
    if id.is_nil() {
      return Err(AudioTrackError::NilSpeaker);
    }
    if speakers[..i].contains(id) {
      return Err(AudioTrackError::DuplicateSpeaker);
    }
  }
  Ok(())
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

  /// `detected ≠ declared` — **derived**, not stored.
  ///
  /// True iff both `language` and `detected_language` are present and
  /// differ. If either side is absent there is nothing to compare, so the
  /// answer is `false` (no *known* mismatch). Computing this on demand
  /// makes it impossible for the flag to contradict the two language
  /// fields it is derived from.
  #[inline]
  pub fn language_mismatch(&self) -> bool {
    match (self.language, self.detected_language) {
      (Some(declared), Some(detected)) => declared != detected,
      _ => false,
    }
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

  /// Validating builder: replace `sample_rate`.
  ///
  /// A probed track (`index_status` includes `EXTRACTED` or later) must
  /// keep `sample_rate > 0`; resetting it to `0` once the descriptor has
  /// been asserted is rejected with
  /// [`AudioTrackError::ProbedDescriptorCleared`]. On rejection `self` is
  /// returned unchanged inside the `Err`.
  #[inline]
  pub fn try_with_sample_rate(mut self, hz: u32) -> Result<Self, AudioTrackError> {
    if hz == 0 && status_asserts_descriptor(self.index_status) {
      return Err(AudioTrackError::ProbedDescriptorCleared);
    }
    self.sample_rate = hz;
    Ok(self)
  }

  /// Validating builder: replace `channels`.
  ///
  /// A probed track (`index_status` includes `EXTRACTED` or later) must
  /// keep `channels > 0`; resetting it to `0` once the descriptor has been
  /// asserted is rejected with
  /// [`AudioTrackError::ProbedDescriptorCleared`]. On rejection `self` is
  /// returned unchanged inside the `Err`.
  #[inline]
  pub fn try_with_channels(mut self, channels: u16) -> Result<Self, AudioTrackError> {
    if channels == 0 && status_asserts_descriptor(self.index_status) {
      return Err(AudioTrackError::ProbedDescriptorCleared);
    }
    self.channels = channels;
    Ok(self)
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

  /// Validating builder: replace `duration`.
  ///
  /// `duration` is semantically a non-negative track-relative length;
  /// although `mediatime::Timestamp` admits a negative PTS, a `Some(_)`
  /// with `pts() < 0` is rejected with
  /// [`AudioTrackError::NegativeDuration`]. `None` (absent) and a zero or
  /// positive `Timestamp` are accepted. On rejection `self` is returned
  /// unchanged inside the `Err`. (`start_pts` is left infallible — a
  /// negative first-PTS offset is legitimate.)
  #[inline]
  pub fn try_with_duration(mut self, v: Option<Timestamp>) -> Result<Self, AudioTrackError> {
    if is_negative_duration(v) {
      return Err(AudioTrackError::NegativeDuration);
    }
    self.duration = v;
    Ok(self)
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

  /// Validating builder: replace `speech_ratio`.
  ///
  /// `speech_ratio` is a fraction that drives pipeline decisions, so a
  /// `Some(_)` value must be finite (no NaN / ±∞) and within `[0,1]`;
  /// `None` (absent) is always accepted. On rejection `self` is returned
  /// unchanged inside the `Err`.
  ///
  /// Not `const` — the error path drops `self`, which is not permitted in
  /// a `const fn`.
  #[inline]
  pub fn try_with_speech_ratio(mut self, v: Option<f32>) -> Result<Self, AudioTrackError> {
    if !is_valid_ratio(v) {
      return Err(AudioTrackError::SpeechRatioOutOfRange);
    }
    self.speech_ratio = v;
    Ok(self)
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

  /// Validating builder: replace `index_status`.
  ///
  /// Two invariants are enforced:
  ///
  /// * **Topology** — the mask must be consistent with the contiguous
  ///   `AudioIndexStage` lifecycle (`validate_status_topology`): a later
  ///   stage bit set without its prerequisites is rejected with
  ///   [`AudioTrackError::StatusOutOfOrder`], so the raw status cannot
  ///   disagree with the derived stage.
  /// * **Descriptor** — a status that includes `EXTRACTED` (or any later
  ///   pipeline bit) asserts the track has been probed, which the locked
  ///   invariant ties to a real descriptor: it is rejected with
  ///   [`AudioTrackError::ExtractedWithoutDescriptor`] while `sample_rate`
  ///   or `channels` is still `0`.
  ///
  /// On rejection `self` is returned unchanged inside the `Err`.
  #[inline]
  pub fn try_with_index_status(mut self, v: AudioIndexStatus) -> Result<Self, AudioTrackError> {
    if let Err(e) = validate_status_topology(v) {
      return Err(e);
    }
    if status_asserts_descriptor(v) && (self.sample_rate == 0 || self.channels == 0) {
      return Err(AudioTrackError::ExtractedWithoutDescriptor);
    }
    self.index_status = v;
    Ok(self)
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

  /// Validating in-place mutator for `sample_rate`. Rejects clearing the
  /// rate to `0` once `index_status` asserts the descriptor
  /// (`EXTRACTED`-or-later); on rejection `self` is left unchanged.
  #[inline]
  pub const fn try_set_sample_rate(&mut self, hz: u32) -> Result<&mut Self, AudioTrackError> {
    if hz == 0 && status_asserts_descriptor(self.index_status) {
      return Err(AudioTrackError::ProbedDescriptorCleared);
    }
    self.sample_rate = hz;
    Ok(self)
  }

  /// Validating in-place mutator for `channels`. Rejects clearing the
  /// channel count to `0` once `index_status` asserts the descriptor
  /// (`EXTRACTED`-or-later); on rejection `self` is left unchanged.
  #[inline]
  pub const fn try_set_channels(&mut self, channels: u16) -> Result<&mut Self, AudioTrackError> {
    if channels == 0 && status_asserts_descriptor(self.index_status) {
      return Err(AudioTrackError::ProbedDescriptorCleared);
    }
    self.channels = channels;
    Ok(self)
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

  /// Validating in-place mutator for `duration`. Rejects a `Some(_)`
  /// carrying a negative `Timestamp` ([`AudioTrackError::NegativeDuration`]);
  /// `None` and a zero or positive `Timestamp` are accepted. On rejection
  /// `self` is left unchanged.
  #[inline]
  pub const fn try_set_duration(
    &mut self,
    v: Option<Timestamp>,
  ) -> Result<&mut Self, AudioTrackError> {
    if is_negative_duration(v) {
      return Err(AudioTrackError::NegativeDuration);
    }
    self.duration = v;
    Ok(self)
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

  /// Validating in-place mutator for `speech_ratio`. A `Some(_)` value
  /// must be finite and within `[0,1]`; on rejection `self` is left
  /// unchanged.
  #[inline]
  pub const fn try_set_speech_ratio(
    &mut self,
    v: Option<f32>,
  ) -> Result<&mut Self, AudioTrackError> {
    if !is_valid_ratio(v) {
      return Err(AudioTrackError::SpeechRatioOutOfRange);
    }
    self.speech_ratio = v;
    Ok(self)
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

  /// Validating in-place mutator for `index_status`. Rejects a mask that is
  /// topologically inconsistent with the `AudioIndexStage` lifecycle (a
  /// later stage bit without its prerequisites,
  /// [`AudioTrackError::StatusOutOfOrder`]) and an `EXTRACTED`-or-later
  /// status while `sample_rate` or `channels` is still `0`
  /// ([`AudioTrackError::ExtractedWithoutDescriptor`]); on rejection `self`
  /// is left unchanged.
  #[inline]
  pub const fn try_set_index_status(
    &mut self,
    v: AudioIndexStatus,
  ) -> Result<&mut Self, AudioTrackError> {
    if let Err(e) = validate_status_topology(v) {
      return Err(e);
    }
    if status_asserts_descriptor(v) && (self.sample_rate == 0 || self.channels == 0) {
      return Err(AudioTrackError::ExtractedWithoutDescriptor);
    }
    self.index_status = v;
    Ok(self)
  }

  /// In-place mutator for `index_errors`.
  #[inline]
  pub fn set_index_errors(&mut self, v: impl Into<std::vec::Vec<ErrorInfo>>) {
    self.index_errors = v.into();
  }
}

/// Error returned by [`AudioTrack`]'s validating constructor and
/// fraction-valued mutators when an invariant cannot be upheld.
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
  /// A `Some(_)` `speech_ratio` was non-finite (NaN / ±∞) or outside the
  /// closed `[0,1]` interval.
  #[error("AudioTrack speech_ratio must be finite and within [0, 1]")]
  SpeechRatioOutOfRange,
  /// An `index_status` at or past `EXTRACTED` was set while the track
  /// still has no descriptor (`sample_rate == 0` or `channels == 0`) — a
  /// probed track must carry a real descriptor.
  #[error("AudioTrack index_status reached EXTRACTED while sample_rate/channels are still 0")]
  ExtractedWithoutDescriptor,
  /// `sample_rate` or `channels` was cleared to `0` while `index_status`
  /// already asserts the descriptor (`EXTRACTED`-or-later) — a probed
  /// track must keep `sample_rate > 0` and `channels > 0`.
  #[error("AudioTrack sample_rate/channels cannot be cleared to 0 once the track is EXTRACTED")]
  ProbedDescriptorCleared,
  /// An `index_status` mask set a later `AudioIndexStage` bit without its
  /// prerequisite stage bits (e.g. `STT_DONE` without `EXTRACTED` /
  /// `VAD_DONE`) — the contiguous lifecycle would treat it as `Pending`,
  /// so the raw status and derived stage would disagree.
  #[error(
    "AudioTrack index_status mask is out of order: a stage bit is set without its prerequisites"
  )]
  StatusOutOfOrder,
  /// The diarized `speakers` set contained the same id twice — duplicates
  /// would inflate the distinct-speaker count `speakers().len()`.
  #[error("AudioTrack speakers set must not contain duplicate ids")]
  DuplicateSpeaker,
  /// The diarized `speakers` set contained the nil `Uuid7`, which is not a
  /// real `Speaker` identity.
  #[error("AudioTrack speakers set must not contain the nil UUID")]
  NilSpeaker,
  /// An `index_status` mask carried a bit outside the declared
  /// `AudioIndexStatus` set — `bitflags` retains unknown bits on
  /// construction, and the domain cannot reason about a status it does not
  /// understand.
  #[error("AudioTrack index_status mask contains unknown bits outside AudioIndexStatus")]
  UnknownStatusBits,
  /// A `Some(_)` `duration` carried a negative `Timestamp` — a track
  /// duration is semantically a non-negative length.
  #[error("AudioTrack duration must not be negative")]
  NegativeDuration,
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
      .try_with_sample_rate(48_000)
      .unwrap()
      .try_with_channels(2)
      .unwrap()
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
    assert_eq!(tags.track_number(), 1);
    assert_eq!(tags.track_total(), 12);
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
      .try_with_sample_rate(48_000)
      .unwrap()
      .try_with_channels(2)
      .unwrap()
      .try_with_index_status(AudioIndexStatus::EXTRACTED | AudioIndexStatus::VAD_DONE)
      .unwrap()
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
      .try_with_speakers(std::vec![s1])
      .unwrap()
      .with_segments(std::vec![g1]);
    assert_eq!(t.speakers(), &[s1]);
    assert_eq!(t.segments(), &[g1]);
  }

  #[test]
  fn setters_mutate_in_place() {
    let mut t = AudioTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    t.set_codec(AudioCodec::Opus);
    t.try_set_sample_rate(48_000).unwrap();
    t.try_set_channels(2).unwrap();
    t.set_lossless(false);
    t.set_silent(true);
    t.set_content(Some(AudioContentKind::Music));
    t.try_set_index_status(AudioIndexStatus::EXTRACTED).unwrap();
    assert_eq!(t.codec(), &AudioCodec::Opus);
    assert_eq!(t.sample_rate(), 48_000);
    assert_eq!(t.channels(), 2);
    assert!(!t.is_lossless());
    assert!(t.is_silent());
    assert_eq!(t.content(), Some(AudioContentKind::Music));
    assert_eq!(t.index_status(), AudioIndexStatus::EXTRACTED);
  }

  #[test]
  fn try_with_speech_ratio_rejects_non_finite_or_out_of_range() {
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.01, 1.5] {
      let r = t.clone().try_with_speech_ratio(Some(bad));
      assert_eq!(r.err(), Some(AudioTrackError::SpeechRatioOutOfRange));
    }
    // boundary + absent values are accepted
    assert!(t.clone().try_with_speech_ratio(Some(0.0)).is_ok());
    assert!(t.clone().try_with_speech_ratio(Some(1.0)).is_ok());
    assert!(t.clone().try_with_speech_ratio(None).is_ok());
    let t = t.try_with_speech_ratio(Some(0.6)).unwrap();
    assert!((t.speech_ratio().unwrap() - 0.6).abs() < f32::EPSILON);
    assert!(AudioTrackError::SpeechRatioOutOfRange.is_speech_ratio_out_of_range());
  }

  #[test]
  fn try_set_speech_ratio_rejects_and_leaves_value_unchanged() {
    let mut t = AudioTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    t.try_set_speech_ratio(Some(0.4)).unwrap();
    assert_eq!(
      t.try_set_speech_ratio(Some(f32::NAN)).err(),
      Some(AudioTrackError::SpeechRatioOutOfRange)
    );
    // rejection leaves the prior valid value in place
    assert!((t.speech_ratio().unwrap() - 0.4).abs() < f32::EPSILON);
    assert_eq!(
      t.try_set_speech_ratio(Some(2.0)).err(),
      Some(AudioTrackError::SpeechRatioOutOfRange)
    );
    assert!((t.speech_ratio().unwrap() - 0.4).abs() < f32::EPSILON);
    t.try_set_speech_ratio(None).unwrap();
    assert!(t.speech_ratio().is_none());
  }

  // --- Finding 1: index_status gated on descriptor --------------------------

  #[test]
  fn extracted_status_rejected_without_descriptor() {
    // Fresh track has 0/0 descriptor — EXTRACTED must be rejected.
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    assert_eq!(
      t.clone()
        .try_with_index_status(AudioIndexStatus::EXTRACTED)
        .err(),
      Some(AudioTrackError::ExtractedWithoutDescriptor)
    );
    // A topologically-valid later mask (CLASSIFIED → STT_DONE chain) still
    // asserts a probed track and is rejected for the missing descriptor.
    assert_eq!(
      t.clone()
        .try_with_index_status(
          AudioIndexStatus::EXTRACTED | AudioIndexStatus::VAD_DONE | AudioIndexStatus::STT_DONE
        )
        .err(),
      Some(AudioTrackError::ExtractedWithoutDescriptor)
    );
    // Only sample_rate set — channels still 0 — still rejected.
    let half = t.try_with_sample_rate(48_000).unwrap();
    assert_eq!(
      half
        .try_with_index_status(AudioIndexStatus::EXTRACTED)
        .err(),
      Some(AudioTrackError::ExtractedWithoutDescriptor)
    );
    assert!(AudioTrackError::ExtractedWithoutDescriptor.is_extracted_without_descriptor());
  }

  #[test]
  fn extracted_status_accepted_with_descriptor() {
    // Boundary acceptance: a full descriptor admits EXTRACTED.
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .try_with_sample_rate(48_000)
      .unwrap()
      .try_with_channels(2)
      .unwrap()
      .try_with_index_status(AudioIndexStatus::EXTRACTED)
      .unwrap();
    assert_eq!(t.index_status(), AudioIndexStatus::EXTRACTED);
    // The empty status makes no descriptor claim and is always accepted.
    let mut fresh = AudioTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    fresh
      .try_set_index_status(AudioIndexStatus::empty())
      .unwrap();
  }

  #[test]
  fn clearing_descriptor_on_extracted_track_rejected() {
    let mut t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .try_with_sample_rate(48_000)
      .unwrap()
      .try_with_channels(2)
      .unwrap()
      .try_with_index_status(AudioIndexStatus::EXTRACTED)
      .unwrap();
    // Resetting sample_rate / channels to 0 on a probed track is rejected,
    // and leaves the prior value in place.
    assert_eq!(
      t.try_set_sample_rate(0).err(),
      Some(AudioTrackError::ProbedDescriptorCleared)
    );
    assert_eq!(t.sample_rate(), 48_000);
    assert_eq!(
      t.try_set_channels(0).err(),
      Some(AudioTrackError::ProbedDescriptorCleared)
    );
    assert_eq!(t.channels(), 2);
    // The builder form rejects the same way.
    assert_eq!(
      t.clone().try_with_sample_rate(0).err(),
      Some(AudioTrackError::ProbedDescriptorCleared)
    );
    // A non-zero replacement is still accepted on a probed track.
    t.try_set_sample_rate(44_100).unwrap();
    assert_eq!(t.sample_rate(), 44_100);
    // On an unprobed track, clearing to 0 is fine.
    let mut fresh = AudioTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    fresh.try_set_sample_rate(0).unwrap();
    fresh.try_set_channels(0).unwrap();
    assert!(AudioTrackError::ProbedDescriptorCleared.is_probed_descriptor_cleared());
  }

  // --- Finding 1 (round 4): index_status topology ---------------------------

  /// A fully-probed track, ready to receive an `index_status` mask so the
  /// descriptor gate never masks a topology rejection.
  fn probed_track() -> AudioTrack<Uuid7> {
    AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .try_with_sample_rate(48_000)
      .unwrap()
      .try_with_channels(2)
      .unwrap()
  }

  #[test]
  fn index_status_rejects_stage_bit_without_prerequisites() {
    use AudioIndexStatus as S;
    let t = probed_track();
    // STT_DONE without EXTRACTED — missing the probe bit entirely.
    assert_eq!(
      t.clone().try_with_index_status(S::STT_DONE).err(),
      Some(AudioTrackError::StatusOutOfOrder)
    );
    // STT_DONE with EXTRACTED but no analyzed bit (CLASSIFIED | VAD_DONE).
    assert_eq!(
      t.clone()
        .try_with_index_status(S::EXTRACTED | S::STT_DONE)
        .err(),
      Some(AudioTrackError::StatusOutOfOrder)
    );
    // TEXT_EMBED without its SPEAKER_DONE prerequisite.
    assert_eq!(
      t.clone()
        .try_with_index_status(S::EXTRACTED | S::VAD_DONE | S::STT_DONE | S::TEXT_EMBED)
        .err(),
      Some(AudioTrackError::StatusOutOfOrder)
    );
    // SPEAKER_DONE without its STT_DONE prerequisite.
    assert_eq!(
      t.clone()
        .try_with_index_status(S::EXTRACTED | S::VAD_DONE | S::SPEAKER_DONE)
        .err(),
      Some(AudioTrackError::StatusOutOfOrder)
    );
    // A secondary bit without the probe bit is rejected too.
    assert_eq!(
      t.try_with_index_status(S::FPRINT_DONE).err(),
      Some(AudioTrackError::StatusOutOfOrder)
    );
    assert!(AudioTrackError::StatusOutOfOrder.is_status_out_of_order());
  }

  #[test]
  fn index_status_set_rejects_out_of_order_and_leaves_value_unchanged() {
    use AudioIndexStatus as S;
    let mut t = probed_track();
    t.try_set_index_status(S::EXTRACTED | S::VAD_DONE).unwrap();
    // An out-of-order mask is rejected and the prior value is kept.
    assert_eq!(
      t.try_set_index_status(S::STT_DONE).err(),
      Some(AudioTrackError::StatusOutOfOrder)
    );
    assert_eq!(t.index_status(), S::EXTRACTED | S::VAD_DONE);
  }

  #[test]
  fn index_status_accepts_valid_contiguous_masks() {
    use AudioIndexStatus as S;
    let t = probed_track();
    // Every prefix of the contiguous lifecycle is accepted.
    for mask in [
      S::empty(),
      S::EXTRACTED,
      S::EXTRACTED | S::CLASSIFIED,
      S::EXTRACTED | S::VAD_DONE,
      S::EXTRACTED | S::CLASSIFIED | S::VAD_DONE | S::STT_DONE,
      S::EXTRACTED | S::VAD_DONE | S::STT_DONE | S::SPEAKER_DONE,
      S::EXTRACTED | S::VAD_DONE | S::STT_DONE | S::SPEAKER_DONE | S::TEXT_EMBED,
      S::fully_indexed_mask(),
    ] {
      assert!(
        t.clone().try_with_index_status(mask).is_ok(),
        "valid contiguous mask {mask:?} must be accepted"
      );
    }
  }

  // --- Finding 2: speaker set invariant -------------------------------------

  #[test]
  fn try_with_speakers_rejects_duplicates() {
    let s1 = Uuid7::new();
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    assert_eq!(
      t.try_with_speakers(std::vec![s1, s1]).err(),
      Some(AudioTrackError::DuplicateSpeaker)
    );
    assert!(AudioTrackError::DuplicateSpeaker.is_duplicate_speaker());
  }

  #[test]
  fn try_with_speakers_rejects_nil() {
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    assert_eq!(
      t.try_with_speakers(std::vec![Uuid7::nil()]).err(),
      Some(AudioTrackError::NilSpeaker)
    );
    assert!(AudioTrackError::NilSpeaker.is_nil_speaker());
  }

  #[test]
  fn try_set_speakers_rejects_and_leaves_value_unchanged() {
    let s1 = Uuid7::new();
    let s2 = Uuid7::new();
    let mut t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .try_with_speakers(std::vec![s1, s2])
      .unwrap();
    // Distinct set accepted.
    assert_eq!(t.speakers(), &[s1, s2]);
    // Rejected duplicate leaves the prior value in place.
    assert_eq!(
      t.try_set_speakers(std::vec![s1, s1]).err(),
      Some(AudioTrackError::DuplicateSpeaker)
    );
    assert_eq!(t.speakers(), &[s1, s2]);
    // Rejected nil leaves the prior value in place.
    assert_eq!(
      t.try_set_speakers(std::vec![Uuid7::nil()]).err(),
      Some(AudioTrackError::NilSpeaker)
    );
    assert_eq!(t.speakers(), &[s1, s2]);
    // A fresh valid set replaces it.
    t.try_set_speakers(std::vec![s2]).unwrap();
    assert_eq!(t.speakers(), &[s2]);
  }

  // --- Finding 1 (round 5): unknown index_status bits -----------------------

  /// A bit not declared by any `AudioIndexStatus` flag. The declared mask
  /// occupies the low 11 bits, so `0x800` is guaranteed unknown.
  fn unknown_bit() -> AudioIndexStatus {
    let b = AudioIndexStatus::from_bits_retain(0x800);
    assert!(
      b.bits() & !AudioIndexStatus::all().bits() != 0,
      "0x800 must lie outside the declared mask"
    );
    b
  }

  #[test]
  fn index_status_rejects_unknown_bits() {
    let t = probed_track();
    // An unknown-only mask is rejected.
    assert_eq!(
      t.clone().try_with_index_status(unknown_bit()).err(),
      Some(AudioTrackError::UnknownStatusBits)
    );
    // EXTRACTED smuggling an unknown bit alongside is rejected — the unknown
    // check runs before the lifecycle/topology checks.
    assert_eq!(
      t.clone()
        .try_with_index_status(AudioIndexStatus::EXTRACTED | unknown_bit())
        .err(),
      Some(AudioTrackError::UnknownStatusBits)
    );
    // The in-place mutator rejects it too, leaving the prior value unchanged.
    let mut m = probed_track();
    m.try_set_index_status(AudioIndexStatus::EXTRACTED).unwrap();
    assert_eq!(
      m.try_set_index_status(AudioIndexStatus::EXTRACTED | unknown_bit())
        .err(),
      Some(AudioTrackError::UnknownStatusBits)
    );
    assert_eq!(m.index_status(), AudioIndexStatus::EXTRACTED);
    assert!(AudioTrackError::UnknownStatusBits.is_unknown_status_bits());
  }

  #[test]
  fn index_status_accepts_all_known_masks() {
    use AudioIndexStatus as S;
    let t = probed_track();
    // Every all-known valid mask is still accepted after the unknown-bit gate.
    for mask in [
      S::empty(),
      S::EXTRACTED,
      S::EXTRACTED | S::VAD_DONE,
      S::EXTRACTED | S::CLASSIFIED | S::VAD_DONE | S::STT_DONE | S::SPEAKER_DONE,
      S::all(),
    ] {
      assert!(
        t.clone().try_with_index_status(mask).is_ok(),
        "all-known mask {mask:?} must be accepted"
      );
    }
  }

  // --- Finding 2 (round 5): non-negative track duration ---------------------

  /// A standard 1/1000 (millisecond) timebase for duration tests.
  fn tb() -> mediatime::Timebase {
    mediatime::Timebase::new(1, core::num::NonZeroU32::new(1000).expect("nonzero"))
  }

  #[test]
  fn try_with_duration_rejects_negative() {
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    assert_eq!(
      t.clone()
        .try_with_duration(Some(Timestamp::new(-1, tb())))
        .err(),
      Some(AudioTrackError::NegativeDuration)
    );
    assert!(AudioTrackError::NegativeDuration.is_negative_duration());
    // zero / positive / None are accepted.
    let z = t
      .clone()
      .try_with_duration(Some(Timestamp::new(0, tb())))
      .expect("zero accepted");
    assert_eq!(z.duration().unwrap().pts(), 0);
    let p = t
      .clone()
      .try_with_duration(Some(Timestamp::new(5000, tb())))
      .expect("positive accepted");
    assert_eq!(p.duration().unwrap().pts(), 5000);
    let n = t.try_with_duration(None).expect("None accepted");
    assert!(n.duration().is_none());
  }

  #[test]
  fn try_set_duration_rejects_and_leaves_value_unchanged() {
    let mut t = AudioTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    t.try_set_duration(Some(Timestamp::new(3000, tb())))
      .unwrap();
    assert_eq!(
      t.try_set_duration(Some(Timestamp::new(-10, tb()))).err(),
      Some(AudioTrackError::NegativeDuration)
    );
    // rejection leaves the prior valid value in place.
    assert_eq!(t.duration().unwrap().pts(), 3000);
    t.try_set_duration(Some(Timestamp::new(0, tb()))).unwrap();
    assert_eq!(t.duration().unwrap().pts(), 0);
    t.try_set_duration(None).unwrap();
    assert!(t.duration().is_none());
  }

  // --- Finding 3: language_mismatch is derived ------------------------------

  #[test]
  fn language_mismatch_is_derived_from_languages() {
    use mediaframe::lang::Language;
    let en = Language::from_bcp47("en").unwrap();
    let fr = Language::from_bcp47("fr").unwrap();
    // Either side absent → no known mismatch.
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    assert!(!t.language_mismatch());
    let t = t.with_language(Some(en));
    assert!(!t.language_mismatch());
    // Both present and equal → no mismatch.
    let t = t.with_detected_language(Some(en));
    assert!(!t.language_mismatch());
    // Both present and differing → mismatch, with no way to contradict it.
    let t = t.with_detected_language(Some(fr));
    assert!(t.language_mismatch());
  }
}
