//! `SubtitleTrack<Id>` — one subtitle stream of a `Subtitle` facet
//! (locked `schema/subtitle_track.md` r3). An external `.srt`/`.vtt` is
//! **one** `SubtitleTrack`; embedded subtitles are **N**. Holds the
//! per-track stream/codec descriptor, language/role/origin, the
//! parsed-cue aggregate refs, and per-track indexing state.
//!
//! ### mediaframe placeholders
//!
//! The locked doc references several types that live in the (not-yet-a-
//! dependency) `mediaframe` crate. For now they're represented by neutral
//! placeholder types so the domain compiles ahead of `mediaframe`
//! integration. Each is marked `TODO(mediaframe)`:
//!
//! - `mediaframe::SubtitleCodec` → `SmolStr` (free-text codec name).
//! - `mediaframe::SubtitleFormat` → `SmolStr` (text-vs-bitmap container form).
//! - `mediaframe::SubtitleTrackOrigin` → `SmolStr`
//!   (`"external"` / `"embedded"` / `"generated"`).
//! - `mediaframe::Language` (`LanguageCode`) → `SmolStr` (BCP-47 tag;
//!   `""` = absent — `Option` reserved for the future structured type).
//! - `mediaframe::TrackDisposition` (bitflags) → `u32` (raw FFmpeg
//!   `AV_DISPOSITION_*` bits).
//! - `mediatime::TrackTime` (per-track duration / cue positions) →
//!   `mediatime::Timestamp` (same path used by the locked `Speaker`).
//!
//! Once `mediaframe` is a `mediaschema` dep, these fields tighten in a
//! follow-up PR. `is_image_based` similarly stays as a stored `bool`
//! until the future `SubtitleCodec::is_image_based()` exists — at that
//! point the field becomes a derived helper.

use derive_more::IsVariant;
use mediatime::Timestamp;
use smol_str::SmolStr;

use crate::domain::{
  primitives::{ErrorInfo, FileChecksum, Location},
  vo::Provenance,
  SubtitleIndexStatus, SubtitleKind, Uuid7,
};

/// One subtitle stream. Generic over `Id` (default [`Uuid7`]).
///
/// **No `Default`** — a `SubtitleTrack` with nil `id`/`parent` would be
/// an orphaned stream with no addressable identity. Construct via
/// [`SubtitleTrack::try_new`]. Fields are private; access via getters
/// and `with_*` / `set_*` builders/mutators.
// `coverage_ratio: Option<f32>` rules out `Eq` / `Hash` (float NaN
// invariants). `PartialEq` is sufficient for the existing aggregate
// test surface (`assert_eq!`).
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleTrack<Id = Uuid7> {
  id: Id,
  parent: Id,

  // Source-locator (not identity).
  stream_index: Option<u32>,
  container_track_id: Option<u64>,

  // Codec / format / origin / language / title.
  // TODO(mediaframe): codec → `mediaframe::SubtitleCodec`.
  codec: SmolStr,
  // TODO(mediaframe): format → `mediaframe::SubtitleFormat`.
  format: SmolStr,
  // TODO(mediaframe): origin → `mediaframe::SubtitleTrackOrigin`.
  origin: SmolStr,
  // TODO(mediaframe): language → `mediaframe::Language` (BCP-47 newtype).
  // `""` = absent (string-rule: never `Option<SmolStr>`).
  language: SmolStr,
  title: SmolStr,

  /// TODO(mediaframe): once `mediaframe::SubtitleCodec` exists this
  /// becomes a derived helper (`codec.is_image_based()`); stored as a
  /// bool for now per the doc's "store derived or compute — open"
  /// resolution.
  is_image_based: bool,

  // TODO(mediaframe): disposition → `mediaframe::TrackDisposition`
  // (bitflags). Stored as raw `u32` of `AV_DISPOSITION_*` bits.
  disposition: u32,

  // Selection signals.
  is_primary: bool,
  auto_selected: bool,

  // TODO(mediaframe): duration → `mediatime::TrackTime` once available;
  // `mediatime::Timestamp` is the closest currently-exported fit (same
  // workaround as `Speaker::speech_duration`).
  duration: Option<Timestamp>,

  // Rollups / forward refs.
  cue_count: u32,
  cues: std::vec::Vec<Id>,

  // Shared per-track Provenance VO (PR #12 locked).
  provenance: Provenance,

  // Adopted rev 2 (ffmpeg-probe / parse / ingest obtainable).
  source_path: Option<Location<Id>>,
  source_checksum: Option<FileChecksum>,
  character_encoding: SmolStr,
  bom_present: bool,
  is_sdh: bool,
  is_closed_caption: bool,
  is_translation: bool,
  kind: SubtitleKind,
  coverage_ratio: Option<f32>,
  is_empty: bool,
  // TODO(mediaframe): first/last cue → `mediatime::TrackTime`.
  first_cue: Option<Timestamp>,
  last_cue: Option<Timestamp>,

  // Per-kind indexing.
  index_status: SubtitleIndexStatus,
  index_errors: std::vec::Vec<ErrorInfo>,
}

impl SubtitleTrack<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (track must be addressable; cues FK to it) and
  /// nil `parent` (orphaned track with no `Subtitle` facet reference).
  /// All other fields take sensible empty/zero defaults — callers
  /// populate via `with_*` / `set_*`.
  pub fn try_new(id: Uuid7, parent: Uuid7) -> Result<Self, SubtitleTrackError> {
    if id.is_nil() {
      return Err(SubtitleTrackError::NilId);
    }
    if parent.is_nil() {
      return Err(SubtitleTrackError::NilParent);
    }
    Ok(Self {
      id,
      parent,
      stream_index: None,
      container_track_id: None,
      codec: SmolStr::default(),
      format: SmolStr::default(),
      origin: SmolStr::default(),
      language: SmolStr::default(),
      title: SmolStr::default(),
      is_image_based: false,
      disposition: 0,
      is_primary: false,
      auto_selected: false,
      duration: None,
      cue_count: 0,
      cues: std::vec::Vec::new(),
      provenance: Provenance::new(),
      source_path: None,
      source_checksum: None,
      character_encoding: SmolStr::default(),
      bom_present: false,
      is_sdh: false,
      is_closed_caption: false,
      is_translation: false,
      kind: SubtitleKind::default(),
      coverage_ratio: None,
      is_empty: false,
      first_cue: None,
      last_cue: None,
      index_status: SubtitleIndexStatus::new(),
      index_errors: std::vec::Vec::new(),
    })
  }
}

impl<Id> SubtitleTrack<Id> {
  /// Canonical identity (cues FK to this).
  #[inline]
  pub const fn id(&self) -> &Id {
    &self.id
  }

  /// FK → `Subtitle.id`.
  #[inline]
  pub const fn parent(&self) -> &Id {
    &self.parent
  }

  /// Source-locator stream index (ffmpeg/WebCodecs); `None` for
  /// external files.
  #[inline]
  pub const fn stream_index(&self) -> Option<u32> {
    self.stream_index
  }

  /// Container-specific track id (kept only if the pipeline uses it).
  #[inline]
  pub const fn container_track_id(&self) -> Option<u64> {
    self.container_track_id
  }

  /// Codec (`""` = absent). TODO(mediaframe): tighten to
  /// `mediaframe::SubtitleCodec`.
  #[inline]
  pub fn codec(&self) -> &str {
    self.codec.as_str()
  }

  /// Text vs bitmap container form (`""` = absent).
  /// TODO(mediaframe): tighten to `mediaframe::SubtitleFormat`.
  #[inline]
  pub fn format(&self) -> &str {
    self.format.as_str()
  }

  /// `"external"` / `"embedded"` / `"generated"` (`""` = absent).
  /// TODO(mediaframe): tighten to `mediaframe::SubtitleTrackOrigin`.
  #[inline]
  pub fn origin(&self) -> &str {
    self.origin.as_str()
  }

  /// Language tag (`""` = absent). TODO(mediaframe): tighten to
  /// `mediaframe::Language` (BCP-47 newtype).
  #[inline]
  pub fn language(&self) -> &str {
    self.language.as_str()
  }

  /// Track title/label (`""` = absent — string-rule, no `Option`).
  #[inline]
  pub fn title(&self) -> &str {
    self.title.as_str()
  }

  /// Whether this codec requires OCR (PGS/DVBSUB/…). TODO(mediaframe):
  /// becomes a derived helper on `mediaframe::SubtitleCodec`.
  #[inline]
  pub const fn is_image_based(&self) -> bool {
    self.is_image_based
  }

  /// Raw FFmpeg `AV_DISPOSITION_*` bits. TODO(mediaframe): tighten to
  /// `mediaframe::TrackDisposition` (bitflags).
  #[inline]
  pub const fn disposition(&self) -> u32 {
    self.disposition
  }

  /// Primary subtitle for this `Subtitle` facet.
  #[inline]
  pub const fn is_primary(&self) -> bool {
    self.is_primary
  }

  /// Selected by the default-track selection heuristic.
  #[inline]
  pub const fn auto_selected(&self) -> bool {
    self.auto_selected
  }

  /// Per-track duration. TODO(mediaframe): switch to
  /// `mediatime::TrackTime` once available (see `Speaker` note).
  #[inline]
  pub const fn duration(&self) -> Option<&Timestamp> {
    self.duration.as_ref()
  }

  /// Σ of the cue aggregate's len (denormalised; truth = cue aggregate).
  #[inline]
  pub const fn cue_count(&self) -> u32 {
    self.cue_count
  }

  /// Forward refs to the per-track `SubtitleCue` segment aggregate.
  #[inline]
  pub const fn cues(&self) -> &[Id] {
    self.cues.as_slice()
  }

  /// Parse / OCR reproducibility (shared per-track [`Provenance`] VO).
  #[inline]
  pub const fn provenance(&self) -> &Provenance {
    &self.provenance
  }

  /// External `.srt`/`.vtt` location (`None` for embedded).
  #[inline]
  pub const fn source_path(&self) -> Option<&Location<Id>> {
    self.source_path.as_ref()
  }

  /// Checksum of the external file (`None` for embedded).
  #[inline]
  pub const fn source_checksum(&self) -> Option<&FileChecksum> {
    self.source_checksum.as_ref()
  }

  /// Charset (`""` = absent / detector-driven).
  #[inline]
  pub fn character_encoding(&self) -> &str {
    self.character_encoding.as_str()
  }

  /// BOM sniffed at parse time.
  #[inline]
  pub const fn bom_present(&self) -> bool {
    self.bom_present
  }

  /// SDH (deaf / hard-of-hearing) — finer than the disposition
  /// `HEARING_IMPAIRED` bit.
  #[inline]
  pub const fn is_sdh(&self) -> bool {
    self.is_sdh
  }

  /// CEA-608/708 closed-caption stream lifted to a track.
  #[inline]
  pub const fn is_closed_caption(&self) -> bool {
    self.is_closed_caption
  }

  /// Computed: subtitle language ≠ audio language.
  #[inline]
  pub const fn is_translation(&self) -> bool {
    self.is_translation
  }

  /// Subtitle role (selection / search facet).
  #[inline]
  pub const fn kind(&self) -> SubtitleKind {
    self.kind
  }

  /// Subtitled duration ÷ track duration (partial/truncated detection).
  #[inline]
  pub const fn coverage_ratio(&self) -> Option<f32> {
    self.coverage_ratio
  }

  /// Parsed but zero cues (a defect to surface).
  #[inline]
  pub const fn is_empty(&self) -> bool {
    self.is_empty
  }

  /// First cue start. TODO(mediaframe): switch to `mediatime::TrackTime`.
  #[inline]
  pub const fn first_cue(&self) -> Option<&Timestamp> {
    self.first_cue.as_ref()
  }

  /// Last cue start. TODO(mediaframe): switch to `mediatime::TrackTime`.
  #[inline]
  pub const fn last_cue(&self) -> Option<&Timestamp> {
    self.last_cue.as_ref()
  }

  /// Per-kind pipeline-stage bits (bit = stage succeeded).
  #[inline]
  pub const fn index_status(&self) -> SubtitleIndexStatus {
    self.index_status
  }

  /// Per-track error truth (stage-coded `ErrorInfo.code`). Drives
  /// `Media.error_flags.SUBTITLE_ERROR` rollup.
  #[inline]
  pub const fn index_errors(&self) -> &[ErrorInfo] {
    self.index_errors.as_slice()
  }

  // -------------------------------------------------------------------
  // Builders (`with_*` consume self and return Self).
  // -------------------------------------------------------------------

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
  pub fn with_codec(mut self, v: impl Into<SmolStr>) -> Self {
    self.codec = v.into();
    self
  }

  /// Builder: replace `format`.
  #[inline]
  pub fn with_format(mut self, v: impl Into<SmolStr>) -> Self {
    self.format = v.into();
    self
  }

  /// Builder: replace `origin`.
  #[inline]
  pub fn with_origin(mut self, v: impl Into<SmolStr>) -> Self {
    self.origin = v.into();
    self
  }

  /// Builder: replace `language`.
  #[inline]
  pub fn with_language(mut self, v: impl Into<SmolStr>) -> Self {
    self.language = v.into();
    self
  }

  /// Builder: replace `title`.
  #[inline]
  pub fn with_title(mut self, v: impl Into<SmolStr>) -> Self {
    self.title = v.into();
    self
  }

  /// Builder: replace `is_image_based`.
  #[inline]
  pub const fn with_image_based(mut self, v: bool) -> Self {
    self.is_image_based = v;
    self
  }

  /// Builder: replace `disposition`.
  #[inline]
  pub const fn with_disposition(mut self, v: u32) -> Self {
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

  /// Builder: replace `duration`.
  #[inline]
  pub fn with_duration(mut self, v: Option<Timestamp>) -> Self {
    self.duration = v;
    self
  }

  /// Builder: replace `cue_count`.
  #[inline]
  pub const fn with_cue_count(mut self, v: u32) -> Self {
    self.cue_count = v;
    self
  }

  /// Builder: replace `cues`.
  #[inline]
  pub fn with_cues(mut self, v: impl Into<std::vec::Vec<Id>>) -> Self {
    self.cues = v.into();
    self
  }

  /// Builder: replace `provenance`.
  #[inline]
  pub fn with_provenance(mut self, v: Provenance) -> Self {
    self.provenance = v;
    self
  }

  /// Builder: replace `source_path`.
  #[inline]
  pub fn with_source_path(mut self, v: Option<Location<Id>>) -> Self {
    self.source_path = v;
    self
  }

  /// Builder: replace `source_checksum`.
  #[inline]
  pub fn with_source_checksum(mut self, v: Option<FileChecksum>) -> Self {
    self.source_checksum = v;
    self
  }

  /// Builder: replace `character_encoding`.
  #[inline]
  pub fn with_character_encoding(mut self, v: impl Into<SmolStr>) -> Self {
    self.character_encoding = v.into();
    self
  }

  /// Builder: replace `bom_present`.
  #[inline]
  pub const fn with_bom_present(mut self, v: bool) -> Self {
    self.bom_present = v;
    self
  }

  /// Builder: replace `is_sdh`.
  #[inline]
  pub const fn with_sdh(mut self, v: bool) -> Self {
    self.is_sdh = v;
    self
  }

  /// Builder: replace `is_closed_caption`.
  #[inline]
  pub const fn with_closed_caption(mut self, v: bool) -> Self {
    self.is_closed_caption = v;
    self
  }

  /// Builder: replace `is_translation`.
  #[inline]
  pub const fn with_translation(mut self, v: bool) -> Self {
    self.is_translation = v;
    self
  }

  /// Builder: replace `kind`.
  #[inline]
  pub const fn with_kind(mut self, v: SubtitleKind) -> Self {
    self.kind = v;
    self
  }

  /// Builder: replace `coverage_ratio`.
  #[inline]
  pub const fn with_coverage_ratio(mut self, v: Option<f32>) -> Self {
    self.coverage_ratio = v;
    self
  }

  /// Builder: replace `is_empty`.
  #[inline]
  pub const fn with_empty(mut self, v: bool) -> Self {
    self.is_empty = v;
    self
  }

  /// Builder: replace `first_cue`.
  #[inline]
  pub fn with_first_cue(mut self, v: Option<Timestamp>) -> Self {
    self.first_cue = v;
    self
  }

  /// Builder: replace `last_cue`.
  #[inline]
  pub fn with_last_cue(mut self, v: Option<Timestamp>) -> Self {
    self.last_cue = v;
    self
  }

  /// Builder: replace `index_status`.
  #[inline]
  pub const fn with_index_status(mut self, v: SubtitleIndexStatus) -> Self {
    self.index_status = v;
    self
  }

  /// Builder: replace `index_errors`.
  #[inline]
  pub fn with_index_errors(mut self, v: impl Into<std::vec::Vec<ErrorInfo>>) -> Self {
    self.index_errors = v.into();
    self
  }

  // -------------------------------------------------------------------
  // In-place setters (`set_*` return ()).
  // -------------------------------------------------------------------

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
  pub fn set_codec(&mut self, v: impl Into<SmolStr>) {
    self.codec = v.into();
  }

  /// In-place mutator for `format`.
  #[inline]
  pub fn set_format(&mut self, v: impl Into<SmolStr>) {
    self.format = v.into();
  }

  /// In-place mutator for `origin`.
  #[inline]
  pub fn set_origin(&mut self, v: impl Into<SmolStr>) {
    self.origin = v.into();
  }

  /// In-place mutator for `language`.
  #[inline]
  pub fn set_language(&mut self, v: impl Into<SmolStr>) {
    self.language = v.into();
  }

  /// In-place mutator for `title`.
  #[inline]
  pub fn set_title(&mut self, v: impl Into<SmolStr>) {
    self.title = v.into();
  }

  /// In-place mutator for `is_image_based`.
  #[inline]
  pub const fn set_image_based(&mut self, v: bool) {
    self.is_image_based = v;
  }

  /// In-place mutator for `disposition`.
  #[inline]
  pub const fn set_disposition(&mut self, v: u32) {
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

  /// In-place mutator for `duration`.
  #[inline]
  pub fn set_duration(&mut self, v: Option<Timestamp>) {
    self.duration = v;
  }

  /// In-place mutator for `cue_count`.
  #[inline]
  pub const fn set_cue_count(&mut self, v: u32) {
    self.cue_count = v;
  }

  /// In-place mutator for `cues`.
  #[inline]
  pub fn set_cues(&mut self, v: impl Into<std::vec::Vec<Id>>) {
    self.cues = v.into();
  }

  /// In-place mutator for `provenance`.
  #[inline]
  pub fn set_provenance(&mut self, v: Provenance) {
    self.provenance = v;
  }

  /// In-place mutator for `source_path`.
  #[inline]
  pub fn set_source_path(&mut self, v: Option<Location<Id>>) {
    self.source_path = v;
  }

  /// In-place mutator for `source_checksum`.
  #[inline]
  pub fn set_source_checksum(&mut self, v: Option<FileChecksum>) {
    self.source_checksum = v;
  }

  /// In-place mutator for `character_encoding`.
  #[inline]
  pub fn set_character_encoding(&mut self, v: impl Into<SmolStr>) {
    self.character_encoding = v.into();
  }

  /// In-place mutator for `bom_present`.
  #[inline]
  pub const fn set_bom_present(&mut self, v: bool) {
    self.bom_present = v;
  }

  /// In-place mutator for `is_sdh`.
  #[inline]
  pub const fn set_sdh(&mut self, v: bool) {
    self.is_sdh = v;
  }

  /// In-place mutator for `is_closed_caption`.
  #[inline]
  pub const fn set_closed_caption(&mut self, v: bool) {
    self.is_closed_caption = v;
  }

  /// In-place mutator for `is_translation`.
  #[inline]
  pub const fn set_translation(&mut self, v: bool) {
    self.is_translation = v;
  }

  /// In-place mutator for `kind`.
  #[inline]
  pub const fn set_kind(&mut self, v: SubtitleKind) {
    self.kind = v;
  }

  /// In-place mutator for `coverage_ratio`.
  #[inline]
  pub const fn set_coverage_ratio(&mut self, v: Option<f32>) {
    self.coverage_ratio = v;
  }

  /// In-place mutator for `is_empty`.
  #[inline]
  pub const fn set_empty(&mut self, v: bool) {
    self.is_empty = v;
  }

  /// In-place mutator for `first_cue`.
  #[inline]
  pub fn set_first_cue(&mut self, v: Option<Timestamp>) {
    self.first_cue = v;
  }

  /// In-place mutator for `last_cue`.
  #[inline]
  pub fn set_last_cue(&mut self, v: Option<Timestamp>) {
    self.last_cue = v;
  }

  /// In-place mutator for `index_status`.
  #[inline]
  pub const fn set_index_status(&mut self, v: SubtitleIndexStatus) {
    self.index_status = v;
  }

  /// In-place mutator for `index_errors`.
  #[inline]
  pub fn set_index_errors(&mut self, v: impl Into<std::vec::Vec<ErrorInfo>>) {
    self.index_errors = v.into();
  }
}

/// Error returned when [`SubtitleTrack::try_new`] cannot uphold the
/// non-nil-id / non-nil-parent invariants. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum SubtitleTrackError {
  /// Supplied `id` was the nil sentinel — cues FK would be orphaned.
  #[error("SubtitleTrack id must not be the nil UUID")]
  NilId,
  /// Supplied `parent` was the nil sentinel — orphaned track with no
  /// `Subtitle` facet reference.
  #[error("SubtitleTrack parent (Subtitle) must not be the nil UUID")]
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
    let t = SubtitleTrack::try_new(Uuid7::new(), parent).expect("valid construction must succeed");
    assert_eq!(t.parent(), &parent);
    assert!(t.codec().is_empty());
    assert!(t.format().is_empty());
    assert!(t.origin().is_empty());
    assert!(t.language().is_empty());
    assert!(t.title().is_empty());
    assert!(!t.is_image_based());
    assert_eq!(t.disposition(), 0);
    assert!(!t.is_primary());
    assert!(!t.auto_selected());
    assert!(t.duration().is_none());
    assert_eq!(t.cue_count(), 0);
    assert!(t.cues().is_empty());
    assert_eq!(t.provenance(), &Provenance::new());
    assert!(t.source_path().is_none());
    assert!(t.source_checksum().is_none());
    assert!(t.character_encoding().is_empty());
    assert_eq!(t.index_status(), SubtitleIndexStatus::new());
    assert!(t.index_errors().is_empty());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = SubtitleTrack::try_new(Uuid7::nil(), Uuid7::new());
    assert_eq!(r.err(), Some(SubtitleTrackError::NilId));
    assert!(SubtitleTrackError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_parent() {
    let r = SubtitleTrack::try_new(Uuid7::new(), Uuid7::nil());
    assert_eq!(r.err(), Some(SubtitleTrackError::NilParent));
    assert!(SubtitleTrackError::NilParent.is_nil_parent());
  }

  #[test]
  fn descriptor_builders_chain() {
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec("subrip")
      .with_format("text")
      .with_origin("external")
      .with_language("en")
      .with_title("English (SDH)")
      .with_image_based(false)
      .with_disposition(0x0040)
      .with_primary(true)
      .with_auto_selected(true)
      .with_kind(SubtitleKind::ForcedNarrative)
      .with_sdh(true);
    assert_eq!(t.codec(), "subrip");
    assert_eq!(t.format(), "text");
    assert_eq!(t.origin(), "external");
    assert_eq!(t.language(), "en");
    assert_eq!(t.title(), "English (SDH)");
    assert!(!t.is_image_based());
    assert_eq!(t.disposition(), 0x0040);
    assert!(t.is_primary());
    assert!(t.auto_selected());
    assert!(t.kind().is_forced_narrative());
    assert!(t.is_sdh());
  }

  #[test]
  fn cue_rollup_builders() {
    let c1 = Uuid7::new();
    let c2 = Uuid7::new();
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_cues(std::vec![c1, c2])
      .with_cue_count(2);
    assert_eq!(t.cues().len(), 2);
    assert_eq!(t.cue_count(), 2);
    assert!(t.cues().contains(&c1));
    assert!(t.cues().contains(&c2));
  }

  #[test]
  fn provenance_is_per_track() {
    let prov = Provenance::from_parts("tesseract", "5.3.0", "", "indexer-0.4.2");
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_provenance(prov.clone());
    assert_eq!(t.provenance(), &prov);
  }

  #[test]
  fn index_state_builders() {
    let err = ErrorInfo::code_only(ErrorCode::ProbeCorrupt);
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_index_status(SubtitleIndexStatus::TRACKS_DISCOVERED)
      .with_index_errors(std::vec![err.clone()]);
    assert!(t
      .index_status()
      .contains(SubtitleIndexStatus::TRACKS_DISCOVERED));
    assert_eq!(t.index_errors().len(), 1);
    assert_eq!(t.index_errors()[0], err);
  }

  #[test]
  fn setters_mutate_in_place() {
    let mut t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    t.set_codec("ass");
    t.set_format("text");
    t.set_origin("embedded");
    t.set_language("ja");
    t.set_title("Japanese");
    t.set_disposition(0x0001);
    t.set_primary(true);
    t.set_kind(SubtitleKind::CommentaryText);
    t.set_index_status(SubtitleIndexStatus::CUES_EXTRACTED);
    assert_eq!(t.codec(), "ass");
    assert_eq!(t.format(), "text");
    assert_eq!(t.origin(), "embedded");
    assert_eq!(t.language(), "ja");
    assert_eq!(t.title(), "Japanese");
    assert_eq!(t.disposition(), 0x0001);
    assert!(t.is_primary());
    assert!(t.kind().is_commentary_text());
    assert!(t
      .index_status()
      .contains(SubtitleIndexStatus::CUES_EXTRACTED));
  }
}
