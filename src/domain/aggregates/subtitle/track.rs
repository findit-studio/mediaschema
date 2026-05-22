//! `SubtitleTrack<Id>` — one subtitle stream of a `Subtitle` facet
//! (locked `schema/subtitle_track.md` r3). An external `.srt`/`.vtt` is
//! **one** `SubtitleTrack`; embedded subtitles are **N**. Holds the
//! per-track stream/codec descriptor, language/role/origin, the
//! parsed-cue aggregate refs, and per-track indexing state.
//!
//! The per-track codec / format / origin / language / disposition
//! descriptors are the published `mediaframe` types
//! ([`mediaframe::codec::SubtitleCodec`], [`mediaframe::subtitle::Format`],
//! [`mediaframe::subtitle::TrackOrigin`], [`mediaframe::lang::Language`],
//! [`mediaframe::disposition::TrackDisposition`]). The one remaining
//! placeholder is the per-track duration / cue positions:
//! `mediatime::TrackTime` (per-track time) → `mediatime::Timestamp`
//! (same path used by the locked `Speaker`) until that type exists.

use derive_more::IsVariant;
use mediaframe::{
  codec::SubtitleCodec,
  disposition::TrackDisposition,
  lang::Language,
  subtitle::{Format, TrackOrigin},
};
use mediatime::Timestamp;
use smol_str::SmolStr;

#[cfg(any(feature = "std", feature = "alloc"))]
use crate::domain::SubtitleIndexStage;
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
  codec: SubtitleCodec,
  format: Format,
  origin: TrackOrigin,
  language: Language,
  title: SmolStr,

  disposition: TrackDisposition,

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
      // `SubtitleCodec` has no `Default`; the lossless "absent" value is
      // the empty `Other("")` escape (mirrors the `""` = absent rule).
      codec: SubtitleCodec::Other(SmolStr::default()),
      format: Format::default(),
      origin: TrackOrigin::default(),
      language: Language::default(),
      title: SmolStr::default(),
      disposition: TrackDisposition::default(),
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
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// FK → `Subtitle.id`.
  #[inline(always)]
  pub const fn parent_ref(&self) -> &Id {
    &self.parent
  }

  /// Source-locator stream index (ffmpeg/WebCodecs); `None` for
  /// external files.
  #[inline(always)]
  pub const fn stream_index(&self) -> Option<u32> {
    self.stream_index
  }

  /// Container-specific track id (kept only if the pipeline uses it).
  #[inline(always)]
  pub const fn container_track_id(&self) -> Option<u64> {
    self.container_track_id
  }

  /// Subtitle codec (`Other("")` = absent).
  #[inline(always)]
  pub const fn codec_ref(&self) -> &SubtitleCodec {
    &self.codec
  }

  /// Text vs bitmap container form ([`Format::default`] = absent).
  #[inline(always)]
  pub const fn format_ref(&self) -> &Format {
    &self.format
  }

  /// Where the bytes came from (embedded / sidecar / external).
  #[inline(always)]
  pub const fn origin_ref(&self) -> &TrackOrigin {
    &self.origin
  }

  /// Language tag ([`Language::default`] = `und` / undetermined).
  #[inline(always)]
  pub const fn language_ref(&self) -> &Language {
    &self.language
  }

  /// Track title/label (`""` = absent — string-rule, no `Option`).
  #[inline(always)]
  pub fn title(&self) -> &str {
    self.title.as_str()
  }

  /// Tri-state image-based classification, derived from **both**
  /// `codec` and `format`.
  ///
  /// - `Some(true)`: a known bitmap on either axis ⇒ image-based,
  ///   requires OCR. A known bitmap codec forces `true` even when the
  ///   `format` is a default / unclassified `Other`.
  /// - `Some(false)`: every classifiable axis says text-based and
  ///   neither says bitmap.
  /// - `None`: **both** axes are unknown / unclassified (`Other`) — the
  ///   track cannot be classified, so the caller must not assume
  ///   text-based.
  ///
  /// This is the lossless signal; [`Self::requires_ocr`] is the
  /// conservative `bool` projection used by the stage / progress
  /// rollups.
  #[inline]
  pub fn image_based(&self) -> Option<bool> {
    match (self.codec.is_image_based(), self.format.is_image_based()) {
      // A known bitmap on either axis is decisive.
      (Some(true), _) | (_, Some(true)) => Some(true),
      // Otherwise, if either axis classifies it as text-based, trust it.
      (Some(false), _) | (_, Some(false)) => Some(false),
      // Neither axis could classify the track.
      (None, None) => None,
    }
  }

  /// Whether this track requires an OCR pipeline stage — the
  /// **conservative** `bool` projection of [`Self::image_based`].
  ///
  /// An unknown / unclassified track (`image_based() == None`) maps to
  /// `true`: OCR is *required*. Under-requiring OCR would let a
  /// `SEARCH_INDEXED` completion check pass without `OCR_DONE`,
  /// silently skipping OCR for what may be image subtitles. Pass this
  /// into [`SubtitleIndexStatus::is_fully_indexed`] /
  /// `SubtitleIndexStage::from_status` so unknown never under-requires
  /// OCR.
  #[inline]
  pub fn requires_ocr(&self) -> bool {
    // `None` (unclassified) → `true`: conservative, never skip OCR.
    self.image_based().unwrap_or(true)
  }

  /// Whether this track's indexing pipeline is fully complete.
  ///
  /// The completion-facing public path. OCR gating is derived
  /// **internally** from [`Self::requires_ocr`] — callers cannot pass a
  /// wrong `bool` and accidentally mark an unclassified (possibly
  /// image-based) track as complete without `OCR_DONE`. An unknown
  /// codec + unknown format track conservatively requires OCR, so it can
  /// never report fully-indexed until `OCR_DONE` is set.
  #[inline]
  pub fn is_fully_indexed(&self) -> bool {
    self.index_status.is_fully_indexed(self.requires_ocr())
  }

  /// Coarse derived [`SubtitleIndexStage`] for this track's indexing
  /// lifecycle.
  ///
  /// The completion-facing public path. Like [`Self::is_fully_indexed`],
  /// OCR gating is derived **internally** from [`Self::requires_ocr`]
  /// (and the structured `index_errors`), so callers cannot reintroduce
  /// the OCR-bypass bug by passing a wrong `bool`.
  #[cfg(any(feature = "std", feature = "alloc"))]
  #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
  #[inline]
  pub fn index_stage(&self) -> SubtitleIndexStage {
    SubtitleIndexStage::from_status(self.index_status, self.requires_ocr(), &self.index_errors)
  }

  /// FFmpeg `AV_DISPOSITION_*` bits as a [`TrackDisposition`] bitflags.
  #[inline(always)]
  pub const fn disposition(&self) -> TrackDisposition {
    self.disposition
  }

  /// Primary subtitle for this `Subtitle` facet.
  #[inline(always)]
  pub const fn is_primary(&self) -> bool {
    self.is_primary
  }

  /// Selected by the default-track selection heuristic.
  #[inline(always)]
  pub const fn auto_selected(&self) -> bool {
    self.auto_selected
  }

  /// Per-track duration. TODO(mediaframe): switch to
  /// `mediatime::TrackTime` once available (see `Speaker` note).
  #[inline(always)]
  pub const fn duration_ref(&self) -> Option<&Timestamp> {
    self.duration.as_ref()
  }

  /// Σ of the cue aggregate's len (denormalised; truth = cue aggregate).
  #[inline(always)]
  pub const fn cue_count(&self) -> u32 {
    self.cue_count
  }

  /// Forward refs to the per-track `SubtitleCue` segment aggregate.
  #[inline(always)]
  pub const fn cues_slice(&self) -> &[Id] {
    self.cues.as_slice()
  }

  /// Parse / OCR reproducibility (shared per-track [`Provenance`] VO).
  #[inline(always)]
  pub const fn provenance_ref(&self) -> &Provenance {
    &self.provenance
  }

  /// External `.srt`/`.vtt` location (`None` for embedded).
  #[inline(always)]
  pub const fn source_path_ref(&self) -> Option<&Location<Id>> {
    self.source_path.as_ref()
  }

  /// Checksum of the external file (`None` for embedded).
  #[inline(always)]
  pub const fn source_checksum_ref(&self) -> Option<&FileChecksum> {
    self.source_checksum.as_ref()
  }

  /// Charset (`""` = absent / detector-driven).
  #[inline(always)]
  pub fn character_encoding(&self) -> &str {
    self.character_encoding.as_str()
  }

  /// BOM sniffed at parse time.
  #[inline(always)]
  pub const fn bom_present(&self) -> bool {
    self.bom_present
  }

  /// SDH (deaf / hard-of-hearing) — finer than the disposition
  /// `HEARING_IMPAIRED` bit.
  #[inline(always)]
  pub const fn is_sdh(&self) -> bool {
    self.is_sdh
  }

  /// CEA-608/708 closed-caption stream lifted to a track.
  #[inline(always)]
  pub const fn is_closed_caption(&self) -> bool {
    self.is_closed_caption
  }

  /// Computed: subtitle language ≠ audio language.
  #[inline(always)]
  pub const fn is_translation(&self) -> bool {
    self.is_translation
  }

  /// Subtitle role (selection / search facet).
  #[inline(always)]
  pub const fn kind(&self) -> SubtitleKind {
    self.kind
  }

  /// Subtitled duration ÷ track duration (partial/truncated detection).
  #[inline(always)]
  pub const fn coverage_ratio(&self) -> Option<f32> {
    self.coverage_ratio
  }

  /// Parsed but zero cues (a defect to surface).
  #[inline(always)]
  pub const fn is_empty(&self) -> bool {
    self.is_empty
  }

  /// First cue start. TODO(mediaframe): switch to `mediatime::TrackTime`.
  #[inline(always)]
  pub const fn first_cue_ref(&self) -> Option<&Timestamp> {
    self.first_cue.as_ref()
  }

  /// Last cue start. TODO(mediaframe): switch to `mediatime::TrackTime`.
  #[inline(always)]
  pub const fn last_cue_ref(&self) -> Option<&Timestamp> {
    self.last_cue.as_ref()
  }

  /// Per-kind pipeline-stage bits (bit = stage succeeded).
  #[inline(always)]
  pub const fn index_status(&self) -> SubtitleIndexStatus {
    self.index_status
  }

  /// Per-track error truth (stage-coded `ErrorInfo.code`). Drives
  /// `Media.error_flags.SUBTITLE_ERROR` rollup.
  #[inline(always)]
  pub const fn index_errors_slice(&self) -> &[ErrorInfo] {
    self.index_errors.as_slice()
  }

  // -------------------------------------------------------------------
  // Builders (`with_*` consume self and return Self).
  // -------------------------------------------------------------------

  /// Builder: replace `stream_index`.
  #[must_use]
  #[inline(always)]
  pub const fn with_stream_index(mut self, v: Option<u32>) -> Self {
    self.stream_index = v;
    self
  }

  /// Builder: replace `container_track_id`.
  #[must_use]
  #[inline(always)]
  pub const fn with_container_track_id(mut self, v: Option<u64>) -> Self {
    self.container_track_id = v;
    self
  }

  /// Builder: replace `codec`.
  #[must_use]
  #[inline(always)]
  pub fn with_codec(mut self, v: SubtitleCodec) -> Self {
    self.codec = v;
    self
  }

  /// Builder: replace `format`.
  #[must_use]
  #[inline(always)]
  pub fn with_format(mut self, v: Format) -> Self {
    self.format = v;
    self
  }

  /// Builder: replace `origin`.
  #[must_use]
  #[inline(always)]
  pub const fn with_origin(mut self, v: TrackOrigin) -> Self {
    self.origin = v;
    self
  }

  /// Builder: replace `language`.
  #[must_use]
  #[inline(always)]
  pub const fn with_language(mut self, v: Language) -> Self {
    self.language = v;
    self
  }

  /// Builder: replace `title`.
  #[must_use]
  #[inline(always)]
  pub fn with_title(mut self, v: impl Into<SmolStr>) -> Self {
    self.title = v.into();
    self
  }

  /// Builder: replace `disposition`.
  #[must_use]
  #[inline(always)]
  pub const fn with_disposition(mut self, v: TrackDisposition) -> Self {
    self.disposition = v;
    self
  }

  /// Builder: replace `is_primary`.
  #[must_use]
  #[inline(always)]
  pub const fn with_primary(mut self, v: bool) -> Self {
    self.is_primary = v;
    self
  }

  /// Builder: replace `auto_selected`.
  #[must_use]
  #[inline(always)]
  pub const fn with_auto_selected(mut self, v: bool) -> Self {
    self.auto_selected = v;
    self
  }

  /// Builder: replace `duration`.
  #[must_use]
  #[inline(always)]
  pub fn with_duration(mut self, v: Option<Timestamp>) -> Self {
    self.duration = v;
    self
  }

  /// Builder: replace `cue_count`.
  #[must_use]
  #[inline(always)]
  pub const fn with_cue_count(mut self, v: u32) -> Self {
    self.cue_count = v;
    self
  }

  /// Builder: replace `cues`.
  #[must_use]
  #[inline(always)]
  pub fn with_cues(mut self, v: impl Into<std::vec::Vec<Id>>) -> Self {
    self.cues = v.into();
    self
  }

  /// Builder: replace `provenance`.
  #[must_use]
  #[inline(always)]
  pub fn with_provenance(mut self, v: Provenance) -> Self {
    self.provenance = v;
    self
  }

  /// Builder: replace `source_path`.
  #[must_use]
  #[inline(always)]
  pub fn with_source_path(mut self, v: Option<Location<Id>>) -> Self {
    self.source_path = v;
    self
  }

  /// Builder: replace `source_checksum`.
  #[must_use]
  #[inline(always)]
  pub fn with_source_checksum(mut self, v: Option<FileChecksum>) -> Self {
    self.source_checksum = v;
    self
  }

  /// Builder: replace `character_encoding`.
  #[must_use]
  #[inline(always)]
  pub fn with_character_encoding(mut self, v: impl Into<SmolStr>) -> Self {
    self.character_encoding = v.into();
    self
  }

  /// Builder: replace `bom_present`.
  #[must_use]
  #[inline(always)]
  pub const fn with_bom_present(mut self, v: bool) -> Self {
    self.bom_present = v;
    self
  }

  /// Builder: replace `is_sdh`.
  #[must_use]
  #[inline(always)]
  pub const fn with_sdh(mut self, v: bool) -> Self {
    self.is_sdh = v;
    self
  }

  /// Builder: replace `is_closed_caption`.
  #[must_use]
  #[inline(always)]
  pub const fn with_closed_caption(mut self, v: bool) -> Self {
    self.is_closed_caption = v;
    self
  }

  /// Builder: replace `is_translation`.
  #[must_use]
  #[inline(always)]
  pub const fn with_translation(mut self, v: bool) -> Self {
    self.is_translation = v;
    self
  }

  /// Builder: replace `kind`.
  #[must_use]
  #[inline(always)]
  pub const fn with_kind(mut self, v: SubtitleKind) -> Self {
    self.kind = v;
    self
  }

  /// Builder: replace `coverage_ratio`.
  #[must_use]
  #[inline(always)]
  pub const fn with_coverage_ratio(mut self, v: Option<f32>) -> Self {
    self.coverage_ratio = v;
    self
  }

  /// Builder: replace `is_empty`.
  #[must_use]
  #[inline(always)]
  pub const fn with_empty(mut self, v: bool) -> Self {
    self.is_empty = v;
    self
  }

  /// Builder: replace `first_cue`.
  #[must_use]
  #[inline(always)]
  pub fn with_first_cue(mut self, v: Option<Timestamp>) -> Self {
    self.first_cue = v;
    self
  }

  /// Builder: replace `last_cue`.
  #[must_use]
  #[inline(always)]
  pub fn with_last_cue(mut self, v: Option<Timestamp>) -> Self {
    self.last_cue = v;
    self
  }

  /// Builder: replace `index_status`.
  #[must_use]
  #[inline(always)]
  pub const fn with_index_status(mut self, v: SubtitleIndexStatus) -> Self {
    self.index_status = v;
    self
  }

  /// Builder: replace `index_errors`.
  #[must_use]
  #[inline(always)]
  pub fn with_index_errors(mut self, v: impl Into<std::vec::Vec<ErrorInfo>>) -> Self {
    self.index_errors = v.into();
    self
  }

  // -------------------------------------------------------------------
  // In-place setters (`set_*` return `&mut Self` for chaining).
  // -------------------------------------------------------------------

  /// In-place mutator for `stream_index`.
  #[inline(always)]
  pub const fn set_stream_index(&mut self, v: Option<u32>) -> &mut Self {
    self.stream_index = v;
    self
  }

  /// In-place mutator for `container_track_id`.
  #[inline(always)]
  pub const fn set_container_track_id(&mut self, v: Option<u64>) -> &mut Self {
    self.container_track_id = v;
    self
  }

  /// In-place mutator for `codec`.
  #[inline(always)]
  pub fn set_codec(&mut self, v: SubtitleCodec) -> &mut Self {
    self.codec = v;
    self
  }

  /// In-place mutator for `format`.
  #[inline(always)]
  pub fn set_format(&mut self, v: Format) -> &mut Self {
    self.format = v;
    self
  }

  /// In-place mutator for `origin`.
  #[inline(always)]
  pub fn set_origin(&mut self, v: TrackOrigin) -> &mut Self {
    self.origin = v;
    self
  }

  /// In-place mutator for `language`.
  #[inline(always)]
  pub fn set_language(&mut self, v: Language) -> &mut Self {
    self.language = v;
    self
  }

  /// In-place mutator for `title`.
  #[inline(always)]
  pub fn set_title(&mut self, v: impl Into<SmolStr>) -> &mut Self {
    self.title = v.into();
    self
  }

  /// In-place mutator for `disposition`.
  #[inline(always)]
  pub fn set_disposition(&mut self, v: TrackDisposition) -> &mut Self {
    self.disposition = v;
    self
  }

  /// In-place mutator for `is_primary`.
  #[inline(always)]
  pub const fn set_primary(&mut self, v: bool) -> &mut Self {
    self.is_primary = v;
    self
  }

  /// In-place mutator for `auto_selected`.
  #[inline(always)]
  pub const fn set_auto_selected(&mut self, v: bool) -> &mut Self {
    self.auto_selected = v;
    self
  }

  /// In-place mutator for `duration`.
  #[inline(always)]
  pub fn set_duration(&mut self, v: Option<Timestamp>) -> &mut Self {
    self.duration = v;
    self
  }

  /// In-place mutator for `cue_count`.
  #[inline(always)]
  pub const fn set_cue_count(&mut self, v: u32) -> &mut Self {
    self.cue_count = v;
    self
  }

  /// In-place mutator for `cues`.
  #[inline(always)]
  pub fn set_cues(&mut self, v: impl Into<std::vec::Vec<Id>>) -> &mut Self {
    self.cues = v.into();
    self
  }

  /// In-place mutator for `provenance`.
  #[inline(always)]
  pub fn set_provenance(&mut self, v: Provenance) -> &mut Self {
    self.provenance = v;
    self
  }

  /// In-place mutator for `source_path`.
  #[inline(always)]
  pub fn set_source_path(&mut self, v: Option<Location<Id>>) -> &mut Self {
    self.source_path = v;
    self
  }

  /// In-place mutator for `source_checksum`.
  #[inline(always)]
  pub fn set_source_checksum(&mut self, v: Option<FileChecksum>) -> &mut Self {
    self.source_checksum = v;
    self
  }

  /// In-place mutator for `character_encoding`.
  #[inline(always)]
  pub fn set_character_encoding(&mut self, v: impl Into<SmolStr>) -> &mut Self {
    self.character_encoding = v.into();
    self
  }

  /// In-place mutator for `bom_present`.
  #[inline(always)]
  pub const fn set_bom_present(&mut self, v: bool) -> &mut Self {
    self.bom_present = v;
    self
  }

  /// In-place mutator for `is_sdh`.
  #[inline(always)]
  pub const fn set_sdh(&mut self, v: bool) -> &mut Self {
    self.is_sdh = v;
    self
  }

  /// In-place mutator for `is_closed_caption`.
  #[inline(always)]
  pub const fn set_closed_caption(&mut self, v: bool) -> &mut Self {
    self.is_closed_caption = v;
    self
  }

  /// In-place mutator for `is_translation`.
  #[inline(always)]
  pub const fn set_translation(&mut self, v: bool) -> &mut Self {
    self.is_translation = v;
    self
  }

  /// In-place mutator for `kind`.
  #[inline(always)]
  pub const fn set_kind(&mut self, v: SubtitleKind) -> &mut Self {
    self.kind = v;
    self
  }

  /// In-place mutator for `coverage_ratio`.
  #[inline(always)]
  pub const fn set_coverage_ratio(&mut self, v: Option<f32>) -> &mut Self {
    self.coverage_ratio = v;
    self
  }

  /// In-place mutator for `is_empty`.
  #[inline(always)]
  pub const fn set_empty(&mut self, v: bool) -> &mut Self {
    self.is_empty = v;
    self
  }

  /// In-place mutator for `first_cue`.
  #[inline(always)]
  pub fn set_first_cue(&mut self, v: Option<Timestamp>) -> &mut Self {
    self.first_cue = v;
    self
  }

  /// In-place mutator for `last_cue`.
  #[inline(always)]
  pub fn set_last_cue(&mut self, v: Option<Timestamp>) -> &mut Self {
    self.last_cue = v;
    self
  }

  /// In-place mutator for `index_status`.
  #[inline(always)]
  pub const fn set_index_status(&mut self, v: SubtitleIndexStatus) -> &mut Self {
    self.index_status = v;
    self
  }

  /// In-place mutator for `index_errors`.
  #[inline(always)]
  pub fn set_index_errors(&mut self, v: impl Into<std::vec::Vec<ErrorInfo>>) -> &mut Self {
    self.index_errors = v.into();
    self
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
    assert_eq!(t.parent_ref(), &parent);
    assert_eq!(t.codec_ref(), &SubtitleCodec::Other(SmolStr::default()));
    assert_eq!(t.format_ref(), &Format::default());
    assert_eq!(t.origin_ref(), &TrackOrigin::default());
    assert_eq!(t.language_ref(), &Language::default());
    assert!(t.title().is_empty());
    // Fresh track: unknown codec + unknown format ⇒ unclassified, so
    // OCR is conservatively required and it is not yet fully indexed.
    assert_eq!(t.image_based(), None);
    assert!(t.requires_ocr());
    assert!(!t.is_fully_indexed());
    assert!(t.index_stage().is_pending());
    assert_eq!(t.disposition(), TrackDisposition::default());
    assert!(!t.is_primary());
    assert!(!t.auto_selected());
    assert!(t.duration_ref().is_none());
    assert_eq!(t.cue_count(), 0);
    assert!(t.cues_slice().is_empty());
    assert_eq!(t.provenance_ref(), &Provenance::new());
    assert!(t.source_path_ref().is_none());
    assert!(t.source_checksum_ref().is_none());
    assert!(t.character_encoding().is_empty());
    assert_eq!(t.index_status(), SubtitleIndexStatus::new());
    assert!(t.index_errors_slice().is_empty());
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
    let lang = Language::from_bcp47("en").unwrap();
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(SubtitleCodec::Subrip)
      .with_format(Format::Srt)
      .with_origin(TrackOrigin::External)
      .with_language(lang)
      .with_title("English (SDH)")
      .with_disposition(TrackDisposition::from_u32(0x0040))
      .with_primary(true)
      .with_auto_selected(true)
      .with_kind(SubtitleKind::ForcedNarrative)
      .with_sdh(true);
    assert_eq!(t.codec_ref(), &SubtitleCodec::Subrip);
    assert_eq!(t.format_ref(), &Format::Srt);
    assert_eq!(t.origin_ref(), &TrackOrigin::External);
    assert_eq!(t.language_ref(), &lang);
    assert_eq!(t.title(), "English (SDH)");
    // `Format::Srt` + `SubtitleCodec::Subrip` are both text → not
    // image-based, OCR not required.
    assert_eq!(t.image_based(), Some(false));
    assert!(!t.requires_ocr());
    assert_eq!(t.disposition(), TrackDisposition::from_u32(0x0040));
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
    assert_eq!(t.cues_slice().len(), 2);
    assert_eq!(t.cue_count(), 2);
    assert!(t.cues_slice().contains(&c1));
    assert!(t.cues_slice().contains(&c2));
  }

  #[test]
  fn provenance_is_per_track() {
    let prov = Provenance::from_parts("tesseract", "5.3.0", "", "indexer-0.4.2");
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_provenance(prov.clone());
    assert_eq!(t.provenance_ref(), &prov);
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
    assert_eq!(t.index_errors_slice().len(), 1);
    assert_eq!(t.index_errors_slice()[0], err);
  }

  #[test]
  fn image_based_known_bitmap_codec_forces_ocr_despite_default_format() {
    // A PGS track with a known bitmap codec but a default / unclassified
    // `Other` format MUST still be treated as image-based — the bug was
    // that `is_image_based()` looked only at `format` and degraded the
    // `Other` to `false`, silently skipping OCR.
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(SubtitleCodec::HdmvPgsSubtitle)
      .with_format(Format::default());
    assert_eq!(t.format_ref(), &Format::default());
    assert_eq!(t.codec_ref().is_image_based(), Some(true));
    assert_eq!(t.image_based(), Some(true));
    assert!(t.requires_ocr(), "known bitmap codec must require OCR");
  }

  #[test]
  fn image_based_dvbsub_codec_with_other_format() {
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(SubtitleCodec::DvbSubtitle)
      .with_format(Format::Other(SmolStr::new("weird")));
    assert_eq!(t.image_based(), Some(true));
    assert!(t.requires_ocr());
  }

  #[test]
  fn image_based_text_codec_and_format_not_ocr() {
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(SubtitleCodec::Ass)
      .with_format(Format::Ass);
    assert_eq!(t.image_based(), Some(false));
    assert!(!t.requires_ocr());
  }

  #[test]
  fn image_based_unknown_on_both_axes_conservatively_requires_ocr() {
    // Neither axis classifies the track → `image_based()` is `None`,
    // and `requires_ocr()` is conservatively `true` so a completion
    // check can never pass without `OCR_DONE`.
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(SubtitleCodec::Other(SmolStr::new("mystery")))
      .with_format(Format::Other(SmolStr::new("mystery")));
    assert_eq!(t.image_based(), None);
    assert!(
      t.requires_ocr(),
      "an unclassified track must never under-require OCR"
    );
  }

  #[test]
  fn unknown_track_with_text_complete_bits_is_not_done() {
    // Regression (Codex round-2): an unknown-codec + unknown-format
    // track with every *text*-pipeline stage bit set must NOT report
    // fully-indexed / Done. `requires_ocr()` is conservatively `true`,
    // so `OCR_DONE` is part of the effective mask and is absent here.
    // The aggregate methods derive OCR gating internally — a caller
    // cannot pass a wrong `bool` to bypass it.
    use SubtitleIndexStatus as S;
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(SubtitleCodec::Other(SmolStr::new("mystery")))
      .with_format(Format::Other(SmolStr::new("mystery")))
      .with_index_status(S::TRACKS_DISCOVERED | S::CUES_EXTRACTED | S::SEARCH_INDEXED);
    assert!(t.requires_ocr());
    assert!(
      !t.is_fully_indexed(),
      "unclassified track without OCR_DONE must not be fully indexed"
    );
    assert_ne!(
      t.index_stage(),
      SubtitleIndexStage::Done,
      "unclassified track without OCR_DONE must not reach Done"
    );
    // Adding `OCR_DONE` completes it once OCR really ran.
    let done = t.with_index_status(S::fully_indexed_mask(true));
    assert!(done.is_fully_indexed());
    assert_eq!(done.index_stage(), SubtitleIndexStage::Done);
  }

  #[test]
  fn image_based_known_bitmap_format_with_other_codec() {
    // Mirror case: bitmap `format`, unknown `Other` codec.
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(SubtitleCodec::Other(SmolStr::default()))
      .with_format(Format::PgsSub);
    assert_eq!(t.image_based(), Some(true));
    assert!(t.requires_ocr());
  }

  #[test]
  fn setters_mutate_in_place() {
    let lang = Language::from_bcp47("ja").unwrap();
    let mut t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    t.set_codec(SubtitleCodec::Ass);
    t.set_format(Format::Ass);
    t.set_origin(TrackOrigin::Embedded);
    t.set_language(lang);
    t.set_title("Japanese");
    t.set_disposition(TrackDisposition::from_u32(0x0001));
    t.set_primary(true);
    t.set_kind(SubtitleKind::CommentaryText);
    t.set_index_status(SubtitleIndexStatus::CUES_EXTRACTED);
    assert_eq!(t.codec_ref(), &SubtitleCodec::Ass);
    assert_eq!(t.format_ref(), &Format::Ass);
    assert_eq!(t.origin_ref(), &TrackOrigin::Embedded);
    assert_eq!(t.language_ref(), &lang);
    assert_eq!(t.title(), "Japanese");
    assert_eq!(t.disposition(), TrackDisposition::from_u32(0x0001));
    assert!(t.is_primary());
    assert!(t.kind().is_commentary_text());
    assert!(t
      .index_status()
      .contains(SubtitleIndexStatus::CUES_EXTRACTED));
  }
}
