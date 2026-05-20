//! `VideoTrack` — one video stream of a `Video` facet
//! (locked `schema/video_track.md` r7).
//!
//! Owns stream/codec descriptors, the frame/pixel/colour vocabulary
//! (`::mediaframe` extern — currently **placeholders** pending the
//! batched post-`0.1.0` mediaframe minor; tagged `TODO(mediaframe)`),
//! the per-stream `Scene` id-list, the per-track indexing state, and
//! the per-track `Provenance` (rev 7 hoist — replaces per-`Scene`/
//! per-`Keyframe` provenance).

use derive_more::{IsVariant, TryUnwrap, Unwrap};
use mediatime::Timestamp;
use smol_str::SmolStr;

use crate::domain::{bitflags::VideoIndexStatus, primitives::ErrorInfo, vo::Provenance, Uuid7};

// ===========================================================================
// VideoCodec — mediaschema-owned (per task instructions)
// ===========================================================================
//
// NOTE: per the locked `schema/enums.md` r4 boundary, codec/profile/
// disposition descriptor enums move to `::mediaframe` (the post-`0.1.0`
// batched minor). The task instructions for this PR specify `VideoCodec`
// as mediaschema-owned for now; defining the type here (under the video
// cluster) keeps the scope tight pending the cross-cutting hoist /
// mediaframe rename. `Other(SmolStr)` is the open-vocab escape so any
// FFmpeg codec name survives the wire round-trip.

/// Video codec family. Locked open-vocab list (`schema/enums.md` r4 —
/// "VideoCodec + Other(SmolStr) escape"); `profile` and `level` live as
/// **separate** `VideoTrack` fields, not encoded into this enum.
///
/// Mixed enum (unit + one newtype variant) → derives `IsVariant` plus
/// `Unwrap` / `TryUnwrap` accessor families with shared/mut-ref flavours.
#[derive(Debug, Clone, PartialEq, Eq, Hash, IsVariant, Unwrap, TryUnwrap)]
#[unwrap(ref, ref_mut)]
#[try_unwrap(ref, ref_mut)]
#[non_exhaustive]
pub enum VideoCodec {
  H264,
  H265,
  H266,
  Vp8,
  Vp9,
  Av1,
  Mpeg2,
  Mpeg4,
  ProRes,
  Dnxhd,
  Theora,
  /// Unrecognised codec; preserves the wire string verbatim so a
  /// future codec value round-trips losslessly.
  Other(SmolStr),
}

// ===========================================================================
// VideoTrack
// ===========================================================================

/// One video stream of a [`Video`](super::facet::Video) facet.
///
/// Generic over `Id` (default [`Uuid7`]). **No `Default`** — defaulting
/// to nil `id`/`parent` would be indistinguishable from an orphan
/// stream. Construct via [`VideoTrack::try_new`] then chain `with_*` /
/// `set_*` mutations.
///
/// Field ordering matches `schema/video_track.md` r7 top-to-bottom.
/// Fields are private; access via the getter / `with_*` / `set_*`
/// accessors per the encapsulation rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoTrack<Id = Uuid7> {
  // --- identity ---
  id: Id,
  parent: Id,

  // --- source locators ---
  stream_index: Option<u32>,
  container_track_id: Option<u64>,

  // --- media-time ---
  start_pts: Option<Timestamp>,
  /// **Semantically a non-negative duration in the track timebase.**
  /// Schema names this `Option<TrackTime>` (`mediatime` alias); the
  /// crate currently exports only `Timestamp` / `TimeRange` / `Timebase`,
  /// so we use the same `Timestamp` workaround as `Speaker::speech_duration`.
  /// A proper `TrackTime` / `Duration` newtype is a tracked mediatime
  /// follow-up.
  duration: Option<Timestamp>,

  // --- codec ---
  codec: VideoCodec,
  profile: Option<SmolStr>,
  level: Option<u16>,

  // --- bitstream / signal ---
  bit_rate: u64,
  nb_frames: Option<u64>,
  has_b_frames: bool,
  closed_gop: Option<bool>,
  bits_per_raw_sample: Option<u8>,

  // --- frame / pixel / colour vocabulary (mediaframe placeholders) ---
  // TODO(mediaframe): `mediaframe::Dimensions` — width × height.
  // Coded W×H; deduplicates mediaschema's own once mediaframe lands.
  dimensions_width: u32,
  dimensions_height: u32,
  // TODO(mediaframe): `Option<mediaframe::Rect>` — clean-aperture / crop.
  visible_rect: Option<RectPlaceholder>,
  // TODO(mediaframe): `mediaframe::SampleAspectRatio` — display aspect /
  // anamorphic. Currently the rational num/den pair the wire carries.
  sample_aspect_ratio_num: u32,
  sample_aspect_ratio_den: u32,
  // TODO(mediaframe): `mediaframe::PixelFormat` — FFmpeg pixfmt (bit
  // depth encoded here).
  pixel_format: u32,
  // TODO(mediaframe): `mediaframe::ColorInfo` — primaries / transfer /
  // matrix / range / chroma_location. Currently the five wire `u32`s.
  color: ColorInfoPlaceholder,
  // TODO(mediaframe): `Option<mediaframe::HdrStaticMetadata>` —
  // MaxCLL / MaxFALL + mastering-display side-data.
  hdr_static: Option<HdrStaticMetadataPlaceholder>,
  // TODO(mediaframe): `mediaframe::Rotation` — display rotation.
  rotation: u32,
  // TODO(mediaframe): `mediaframe::FrameRate` (`{num,den,is_vfr}`).
  frame_rate_num: u32,
  frame_rate_den: u32,
  frame_rate_is_vfr: bool,
  // TODO(mediaframe): `mediaframe::FieldOrder` enum (progressive /
  // tff / bff).
  field_order: u32,
  // TODO(mediaframe): `Option<mediaframe::StereoMode>` — 3D / stereo
  // packing.
  stereo_mode: Option<u32>,
  // TODO(mediaframe): `Option<mediaframe::DolbyVisionConfig>` —
  // profile / level / rpu / el / bl-compat. **Not** the same as HDR10
  // static metadata.
  dovi: Option<DolbyVisionConfigPlaceholder>,

  // --- findit signals ---
  has_embedded_captions: bool,
  // TODO(mediaframe): `mediaframe::TrackDisposition` bitflags — the
  // shared FFmpeg `AV_DISPOSITION_*` set. Currently the bare wire u32.
  disposition: u32,
  is_primary: bool,
  auto_selected: bool,

  // --- per-stream segmented refs ---
  scenes: std::vec::Vec<Id>,

  // --- indexing state ---
  index_status: VideoIndexStatus,
  index_errors: std::vec::Vec<ErrorInfo>,

  // --- analysis-run reproducibility (rev 7 hoist) ---
  provenance: Provenance,
}

// ---------------------------------------------------------------------------
// mediaframe placeholder VOs — tiny private structs so the field shape
// is honest about the wire components (the locked spec is "ColorInfo =
// primaries/transfer/matrix/range/chroma", "HdrStaticMetadata = MaxCLL +
// MaxFALL + mastering display", "DolbyVisionConfig = profile/level/rpu/
// el/bl-compat"). Each one is TODO(mediaframe) — replaced wholesale by
// the mediaframe extern once it ships.
// ---------------------------------------------------------------------------

/// TODO(mediaframe): replace with `mediaframe::Rect` — clean-aperture /
/// crop rectangle within the coded frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RectPlaceholder {
  pub(crate) x: u32,
  pub(crate) y: u32,
  pub(crate) width: u32,
  pub(crate) height: u32,
}

impl RectPlaceholder {
  /// Construct from `(x, y, w, h)`.
  #[inline]
  pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
    Self {
      x,
      y,
      width,
      height,
    }
  }

  /// `x` origin.
  #[inline]
  pub const fn x(&self) -> u32 {
    self.x
  }
  /// `y` origin.
  #[inline]
  pub const fn y(&self) -> u32 {
    self.y
  }
  /// Width.
  #[inline]
  pub const fn width(&self) -> u32 {
    self.width
  }
  /// Height.
  #[inline]
  pub const fn height(&self) -> u32 {
    self.height
  }
}

/// TODO(mediaframe): replace with `mediaframe::ColorInfo` (primaries /
/// transfer / matrix / range / chroma_location enums).
///
/// Wire shape: five `u32` discriminants exactly as ffmpeg / WebCodecs
/// reports them; the mediaframe types layer typed enums + an
/// `Unknown(u32)` escape on top of these wire ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ColorInfoPlaceholder {
  pub(crate) primaries: u32,
  pub(crate) transfer: u32,
  pub(crate) matrix: u32,
  pub(crate) range: u32,
  pub(crate) chroma_location: u32,
}

impl ColorInfoPlaceholder {
  /// Construct from the five wire-numbered components.
  #[inline]
  pub const fn new(
    primaries: u32,
    transfer: u32,
    matrix: u32,
    range: u32,
    chroma_location: u32,
  ) -> Self {
    Self {
      primaries,
      transfer,
      matrix,
      range,
      chroma_location,
    }
  }

  /// Colour primaries (FFmpeg `AVCOL_PRI_*`).
  #[inline]
  pub const fn primaries(&self) -> u32 {
    self.primaries
  }
  /// Transfer characteristic (FFmpeg `AVCOL_TRC_*`).
  #[inline]
  pub const fn transfer(&self) -> u32 {
    self.transfer
  }
  /// Matrix coefficients (FFmpeg `AVCOL_SPC_*`).
  #[inline]
  pub const fn matrix(&self) -> u32 {
    self.matrix
  }
  /// Colour range (FFmpeg `AVCOL_RANGE_*`).
  #[inline]
  pub const fn range(&self) -> u32 {
    self.range
  }
  /// Chroma sample location (FFmpeg `AVCHROMA_LOC_*`).
  #[inline]
  pub const fn chroma_location(&self) -> u32 {
    self.chroma_location
  }
}

/// TODO(mediaframe): replace with `mediaframe::HdrStaticMetadata` —
/// MaxCLL / MaxFALL + mastering-display primaries + luminance bounds.
///
/// Carries `(max_cll, max_fall)` only as placeholder bookkeeping;
/// the full structure (mastering-display primaries / white-point /
/// luminance) lands with the mediaframe extern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct HdrStaticMetadataPlaceholder {
  pub(crate) max_cll: u32,
  pub(crate) max_fall: u32,
}

impl HdrStaticMetadataPlaceholder {
  /// Construct from `(max_cll, max_fall)`.
  #[inline]
  pub const fn new(max_cll: u32, max_fall: u32) -> Self {
    Self { max_cll, max_fall }
  }

  /// Maximum content light level (cd/m²).
  #[inline]
  pub const fn max_cll(&self) -> u32 {
    self.max_cll
  }
  /// Maximum frame-average light level (cd/m²).
  #[inline]
  pub const fn max_fall(&self) -> u32 {
    self.max_fall
  }
}

/// TODO(mediaframe): replace with `mediaframe::DolbyVisionConfig` —
/// `{ profile, level, rpu_present, el_present, bl_present, bl_signal_compatibility }`.
/// Distinct from HDR10 static metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DolbyVisionConfigPlaceholder {
  pub(crate) profile: u8,
  pub(crate) level: u8,
  pub(crate) rpu_present: bool,
  pub(crate) el_present: bool,
  pub(crate) bl_present: bool,
  pub(crate) bl_signal_compatibility_id: u8,
}

impl DolbyVisionConfigPlaceholder {
  /// Construct from all six wire components.
  #[inline]
  pub const fn new(
    profile: u8,
    level: u8,
    rpu_present: bool,
    el_present: bool,
    bl_present: bool,
    bl_signal_compatibility_id: u8,
  ) -> Self {
    Self {
      profile,
      level,
      rpu_present,
      el_present,
      bl_present,
      bl_signal_compatibility_id,
    }
  }

  /// Dolby Vision profile (e.g. 5, 7, 8).
  #[inline]
  pub const fn profile(&self) -> u8 {
    self.profile
  }
  /// Dolby Vision level.
  #[inline]
  pub const fn level(&self) -> u8 {
    self.level
  }
  /// Reference processing-unit metadata present.
  #[inline]
  pub const fn rpu_present(&self) -> bool {
    self.rpu_present
  }
  /// Enhancement layer present.
  #[inline]
  pub const fn el_present(&self) -> bool {
    self.el_present
  }
  /// Base layer present.
  #[inline]
  pub const fn bl_present(&self) -> bool {
    self.bl_present
  }
  /// Base-layer signal-compatibility id (HDR10 / SDR / HLG indicator).
  #[inline]
  pub const fn bl_signal_compatibility_id(&self) -> u8 {
    self.bl_signal_compatibility_id
  }
}

// ---------------------------------------------------------------------------
// VideoTrack — validating ctor + accessors
// ---------------------------------------------------------------------------

impl VideoTrack<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` and nil `parent` (orphan-stream guard).
  /// All other fields take sensible defaults (codec=`Other("")`,
  /// `bit_rate=0`, dimensions zero, every `Option = None`, every flag
  /// false, no scenes, no errors, empty progress, empty provenance);
  /// the indexer fills them in via `with_*` / `set_*` as probing /
  /// detection / analysis stages land.
  pub fn try_new(id: Uuid7, parent: Uuid7) -> Result<Self, VideoTrackError> {
    if id.is_nil() {
      return Err(VideoTrackError::NilId);
    }
    if parent.is_nil() {
      return Err(VideoTrackError::NilParent);
    }
    Ok(Self {
      id,
      parent,
      stream_index: None,
      container_track_id: None,
      start_pts: None,
      duration: None,
      codec: VideoCodec::Other(SmolStr::default()),
      profile: None,
      level: None,
      bit_rate: 0,
      nb_frames: None,
      has_b_frames: false,
      closed_gop: None,
      bits_per_raw_sample: None,
      dimensions_width: 0,
      dimensions_height: 0,
      visible_rect: None,
      sample_aspect_ratio_num: 1,
      sample_aspect_ratio_den: 1,
      pixel_format: 0,
      color: ColorInfoPlaceholder::default(),
      hdr_static: None,
      rotation: 0,
      frame_rate_num: 0,
      frame_rate_den: 1,
      frame_rate_is_vfr: false,
      field_order: 0,
      stereo_mode: None,
      dovi: None,
      has_embedded_captions: false,
      disposition: 0,
      is_primary: false,
      auto_selected: false,
      scenes: std::vec::Vec::new(),
      index_status: VideoIndexStatus::empty(),
      index_errors: std::vec::Vec::new(),
      provenance: Provenance::new(),
    })
  }
}

impl<Id> VideoTrack<Id> {
  /// Canonical identity.
  #[inline]
  pub const fn id(&self) -> &Id {
    &self.id
  }

  /// FK → `Video.id`.
  #[inline]
  pub const fn parent(&self) -> &Id {
    &self.parent
  }

  /// Source-locator: ffmpeg `stream_index` / WebCodecs index. Not
  /// identity.
  #[inline]
  pub const fn stream_index(&self) -> Option<u32> {
    self.stream_index
  }

  /// Container-level track id (kept only if the pipeline uses it).
  #[inline]
  pub const fn container_track_id(&self) -> Option<u64> {
    self.container_track_id
  }

  /// Stream start offset / first PTS (mediatime-represented).
  #[inline]
  pub const fn start_pts(&self) -> Option<&Timestamp> {
    self.start_pts.as_ref()
  }

  /// Per-track duration (mediatime placeholder — see `duration` field
  /// comment).
  #[inline]
  pub const fn duration(&self) -> Option<&Timestamp> {
    self.duration.as_ref()
  }

  /// Codec family (locked `VideoCodec` + `Other(SmolStr)` escape).
  #[inline]
  pub const fn codec(&self) -> &VideoCodec {
    &self.codec
  }

  /// Codec profile (`""` semantically absent, but expressed as
  /// `Option<SmolStr>` here because the locked schema names it
  /// `Option<SmolStr>` — distinct from the "empty-string=absent" rule
  /// for plain `SmolStr` fields).
  #[inline]
  pub fn profile(&self) -> Option<&str> {
    self.profile.as_deref()
  }

  /// Codec level (numeric).
  #[inline]
  pub const fn level(&self) -> Option<u16> {
    self.level
  }

  /// Per-track bitrate (0 = unknown).
  #[inline]
  pub const fn bit_rate(&self) -> u64 {
    self.bit_rate
  }

  /// Frame count (exact-duration / progress / VFR signal).
  #[inline]
  pub const fn nb_frames(&self) -> Option<u64> {
    self.nb_frames
  }

  /// Bitstream contains B-frames (seek/cut behaviour).
  #[inline]
  pub const fn has_b_frames(&self) -> bool {
    self.has_b_frames
  }

  /// Closed-GOP (seek/cut behaviour).
  #[inline]
  pub const fn closed_gop(&self) -> Option<bool> {
    self.closed_gop
  }

  /// Coded sample depth (may differ from pixfmt).
  #[inline]
  pub const fn bits_per_raw_sample(&self) -> Option<u8> {
    self.bits_per_raw_sample
  }

  /// Coded width × height. TODO(mediaframe): expose as
  /// `mediaframe::Dimensions`.
  #[inline]
  pub const fn dimensions(&self) -> (u32, u32) {
    (self.dimensions_width, self.dimensions_height)
  }

  /// Clean-aperture / crop rectangle.
  /// TODO(mediaframe): `Option<&mediaframe::Rect>`.
  #[inline]
  pub const fn visible_rect(&self) -> Option<&RectPlaceholder> {
    self.visible_rect.as_ref()
  }

  /// Display aspect / anamorphic ratio.
  /// TODO(mediaframe): `mediaframe::SampleAspectRatio` (currently the
  /// num/den pair the wire carries).
  #[inline]
  pub const fn sample_aspect_ratio(&self) -> (u32, u32) {
    (self.sample_aspect_ratio_num, self.sample_aspect_ratio_den)
  }

  /// FFmpeg pixfmt id. TODO(mediaframe): `mediaframe::PixelFormat`.
  #[inline]
  pub const fn pixel_format(&self) -> u32 {
    self.pixel_format
  }

  /// Colour primaries / transfer / matrix / range / chroma_location.
  /// TODO(mediaframe): `&mediaframe::ColorInfo`.
  #[inline]
  pub const fn color(&self) -> &ColorInfoPlaceholder {
    &self.color
  }

  /// HDR10 static metadata (MaxCLL / MaxFALL + mastering display).
  /// TODO(mediaframe): `Option<&mediaframe::HdrStaticMetadata>`.
  #[inline]
  pub const fn hdr_static(&self) -> Option<&HdrStaticMetadataPlaceholder> {
    self.hdr_static.as_ref()
  }

  /// Display rotation. TODO(mediaframe): `mediaframe::Rotation`.
  #[inline]
  pub const fn rotation(&self) -> u32 {
    self.rotation
  }

  /// Frame rate as `(num, den, is_vfr)`. TODO(mediaframe):
  /// `mediaframe::FrameRate` (NOT `mediatime::Timebase` — see the
  /// locked spec).
  #[inline]
  pub const fn frame_rate(&self) -> (u32, u32, bool) {
    (
      self.frame_rate_num,
      self.frame_rate_den,
      self.frame_rate_is_vfr,
    )
  }

  /// Field order. TODO(mediaframe): `mediaframe::FieldOrder` enum.
  #[inline]
  pub const fn field_order(&self) -> u32 {
    self.field_order
  }

  /// 3D / stereoscopic packing.
  /// TODO(mediaframe): `Option<mediaframe::StereoMode>`.
  #[inline]
  pub const fn stereo_mode(&self) -> Option<u32> {
    self.stereo_mode
  }

  /// Dolby Vision config.
  /// TODO(mediaframe): `Option<&mediaframe::DolbyVisionConfig>`.
  #[inline]
  pub const fn dovi(&self) -> Option<&DolbyVisionConfigPlaceholder> {
    self.dovi.as_ref()
  }

  /// CEA-608/708 captions detected in the bitstream.
  #[inline]
  pub const fn has_embedded_captions(&self) -> bool {
    self.has_embedded_captions
  }

  /// Disposition flags. TODO(mediaframe): `mediaframe::TrackDisposition`
  /// bitflags (the shared FFmpeg `AV_DISPOSITION_*` set).
  #[inline]
  pub const fn disposition(&self) -> u32 {
    self.disposition
  }

  /// Track selection signal — is this the primary video track for the
  /// containing media file?
  #[inline]
  pub const fn is_primary(&self) -> bool {
    self.is_primary
  }

  /// Track selection signal — was this track auto-selected?
  #[inline]
  pub const fn auto_selected(&self) -> bool {
    self.auto_selected
  }

  /// Refs → child [`Scene`](super::scene::Scene)s.
  #[inline]
  pub const fn scenes(&self) -> &[Id] {
    self.scenes.as_slice()
  }

  /// Per-track pipeline-stage status (bitflags).
  #[inline]
  pub const fn index_status(&self) -> VideoIndexStatus {
    self.index_status
  }

  /// Per-track error history (stage-coded).
  #[inline]
  pub const fn index_errors(&self) -> &[ErrorInfo] {
    self.index_errors.as_slice()
  }

  /// Analysis-run reproducibility for *this track-run*; every child
  /// `Scene` / `Keyframe` inherits this rather than carrying its own
  /// (rev 7 hoist).
  #[inline]
  pub const fn provenance(&self) -> &Provenance {
    &self.provenance
  }
}

// ---------------------------------------------------------------------------
// Builders + setters — one per mutable field, matching the
// `with_*` / `set_*` encapsulation rule.
// ---------------------------------------------------------------------------

impl<Id> VideoTrack<Id> {
  // --- source locators ---
  #[inline]
  pub const fn with_stream_index(mut self, v: Option<u32>) -> Self {
    self.stream_index = v;
    self
  }
  #[inline]
  pub const fn set_stream_index(&mut self, v: Option<u32>) {
    self.stream_index = v;
  }
  #[inline]
  pub const fn with_container_track_id(mut self, v: Option<u64>) -> Self {
    self.container_track_id = v;
    self
  }
  #[inline]
  pub const fn set_container_track_id(&mut self, v: Option<u64>) {
    self.container_track_id = v;
  }

  // --- mediatime ---
  #[inline]
  pub fn with_start_pts(mut self, v: Option<Timestamp>) -> Self {
    self.start_pts = v;
    self
  }
  #[inline]
  pub fn set_start_pts(&mut self, v: Option<Timestamp>) {
    self.start_pts = v;
  }
  #[inline]
  pub fn with_duration(mut self, v: Option<Timestamp>) -> Self {
    self.duration = v;
    self
  }
  #[inline]
  pub fn set_duration(&mut self, v: Option<Timestamp>) {
    self.duration = v;
  }

  // --- codec ---
  #[inline]
  pub fn with_codec(mut self, v: VideoCodec) -> Self {
    self.codec = v;
    self
  }
  #[inline]
  pub fn set_codec(&mut self, v: VideoCodec) {
    self.codec = v;
  }
  #[inline]
  pub fn with_profile(mut self, v: Option<SmolStr>) -> Self {
    self.profile = v;
    self
  }
  #[inline]
  pub fn set_profile(&mut self, v: Option<SmolStr>) {
    self.profile = v;
  }
  #[inline]
  pub const fn with_level(mut self, v: Option<u16>) -> Self {
    self.level = v;
    self
  }
  #[inline]
  pub const fn set_level(&mut self, v: Option<u16>) {
    self.level = v;
  }

  // --- bitstream / signal ---
  #[inline]
  pub const fn with_bit_rate(mut self, v: u64) -> Self {
    self.bit_rate = v;
    self
  }
  #[inline]
  pub const fn set_bit_rate(&mut self, v: u64) {
    self.bit_rate = v;
  }
  #[inline]
  pub const fn with_nb_frames(mut self, v: Option<u64>) -> Self {
    self.nb_frames = v;
    self
  }
  #[inline]
  pub const fn set_nb_frames(&mut self, v: Option<u64>) {
    self.nb_frames = v;
  }
  #[inline]
  pub const fn with_has_b_frames(mut self, v: bool) -> Self {
    self.has_b_frames = v;
    self
  }
  #[inline]
  pub const fn set_has_b_frames(&mut self, v: bool) {
    self.has_b_frames = v;
  }
  #[inline]
  pub const fn with_closed_gop(mut self, v: Option<bool>) -> Self {
    self.closed_gop = v;
    self
  }
  #[inline]
  pub const fn set_closed_gop(&mut self, v: Option<bool>) {
    self.closed_gop = v;
  }
  #[inline]
  pub const fn with_bits_per_raw_sample(mut self, v: Option<u8>) -> Self {
    self.bits_per_raw_sample = v;
    self
  }
  #[inline]
  pub const fn set_bits_per_raw_sample(&mut self, v: Option<u8>) {
    self.bits_per_raw_sample = v;
  }

  // --- frame / pixel / colour ---
  /// TODO(mediaframe): accept `mediaframe::Dimensions`.
  #[inline]
  pub const fn with_dimensions(mut self, width: u32, height: u32) -> Self {
    self.dimensions_width = width;
    self.dimensions_height = height;
    self
  }
  /// TODO(mediaframe): accept `mediaframe::Dimensions`.
  #[inline]
  pub const fn set_dimensions(&mut self, width: u32, height: u32) {
    self.dimensions_width = width;
    self.dimensions_height = height;
  }
  /// TODO(mediaframe): accept `Option<mediaframe::Rect>`.
  #[inline]
  pub const fn with_visible_rect(mut self, v: Option<RectPlaceholder>) -> Self {
    self.visible_rect = v;
    self
  }
  /// TODO(mediaframe): accept `Option<mediaframe::Rect>`.
  #[inline]
  pub const fn set_visible_rect(&mut self, v: Option<RectPlaceholder>) {
    self.visible_rect = v;
  }
  /// TODO(mediaframe): accept `mediaframe::SampleAspectRatio`.
  #[inline]
  pub const fn with_sample_aspect_ratio(mut self, num: u32, den: u32) -> Self {
    self.sample_aspect_ratio_num = num;
    self.sample_aspect_ratio_den = den;
    self
  }
  /// TODO(mediaframe): accept `mediaframe::SampleAspectRatio`.
  #[inline]
  pub const fn set_sample_aspect_ratio(&mut self, num: u32, den: u32) {
    self.sample_aspect_ratio_num = num;
    self.sample_aspect_ratio_den = den;
  }
  /// TODO(mediaframe): accept `mediaframe::PixelFormat`.
  #[inline]
  pub const fn with_pixel_format(mut self, v: u32) -> Self {
    self.pixel_format = v;
    self
  }
  /// TODO(mediaframe): accept `mediaframe::PixelFormat`.
  #[inline]
  pub const fn set_pixel_format(&mut self, v: u32) {
    self.pixel_format = v;
  }
  /// TODO(mediaframe): accept `mediaframe::ColorInfo`.
  #[inline]
  pub const fn with_color(mut self, v: ColorInfoPlaceholder) -> Self {
    self.color = v;
    self
  }
  /// TODO(mediaframe): accept `mediaframe::ColorInfo`.
  #[inline]
  pub const fn set_color(&mut self, v: ColorInfoPlaceholder) {
    self.color = v;
  }
  /// TODO(mediaframe): accept `Option<mediaframe::HdrStaticMetadata>`.
  #[inline]
  pub const fn with_hdr_static(mut self, v: Option<HdrStaticMetadataPlaceholder>) -> Self {
    self.hdr_static = v;
    self
  }
  /// TODO(mediaframe): accept `Option<mediaframe::HdrStaticMetadata>`.
  #[inline]
  pub const fn set_hdr_static(&mut self, v: Option<HdrStaticMetadataPlaceholder>) {
    self.hdr_static = v;
  }
  /// TODO(mediaframe): accept `mediaframe::Rotation`.
  #[inline]
  pub const fn with_rotation(mut self, v: u32) -> Self {
    self.rotation = v;
    self
  }
  /// TODO(mediaframe): accept `mediaframe::Rotation`.
  #[inline]
  pub const fn set_rotation(&mut self, v: u32) {
    self.rotation = v;
  }
  /// TODO(mediaframe): accept `mediaframe::FrameRate`.
  #[inline]
  pub const fn with_frame_rate(mut self, num: u32, den: u32, is_vfr: bool) -> Self {
    self.frame_rate_num = num;
    self.frame_rate_den = den;
    self.frame_rate_is_vfr = is_vfr;
    self
  }
  /// TODO(mediaframe): accept `mediaframe::FrameRate`.
  #[inline]
  pub const fn set_frame_rate(&mut self, num: u32, den: u32, is_vfr: bool) {
    self.frame_rate_num = num;
    self.frame_rate_den = den;
    self.frame_rate_is_vfr = is_vfr;
  }
  /// TODO(mediaframe): accept `mediaframe::FieldOrder`.
  #[inline]
  pub const fn with_field_order(mut self, v: u32) -> Self {
    self.field_order = v;
    self
  }
  /// TODO(mediaframe): accept `mediaframe::FieldOrder`.
  #[inline]
  pub const fn set_field_order(&mut self, v: u32) {
    self.field_order = v;
  }
  /// TODO(mediaframe): accept `Option<mediaframe::StereoMode>`.
  #[inline]
  pub const fn with_stereo_mode(mut self, v: Option<u32>) -> Self {
    self.stereo_mode = v;
    self
  }
  /// TODO(mediaframe): accept `Option<mediaframe::StereoMode>`.
  #[inline]
  pub const fn set_stereo_mode(&mut self, v: Option<u32>) {
    self.stereo_mode = v;
  }
  /// TODO(mediaframe): accept `Option<mediaframe::DolbyVisionConfig>`.
  #[inline]
  pub const fn with_dovi(mut self, v: Option<DolbyVisionConfigPlaceholder>) -> Self {
    self.dovi = v;
    self
  }
  /// TODO(mediaframe): accept `Option<mediaframe::DolbyVisionConfig>`.
  #[inline]
  pub const fn set_dovi(&mut self, v: Option<DolbyVisionConfigPlaceholder>) {
    self.dovi = v;
  }

  // --- findit signals ---
  #[inline]
  pub const fn with_has_embedded_captions(mut self, v: bool) -> Self {
    self.has_embedded_captions = v;
    self
  }
  #[inline]
  pub const fn set_has_embedded_captions(&mut self, v: bool) {
    self.has_embedded_captions = v;
  }
  /// TODO(mediaframe): accept `mediaframe::TrackDisposition`.
  #[inline]
  pub const fn with_disposition(mut self, v: u32) -> Self {
    self.disposition = v;
    self
  }
  /// TODO(mediaframe): accept `mediaframe::TrackDisposition`.
  #[inline]
  pub const fn set_disposition(&mut self, v: u32) {
    self.disposition = v;
  }
  #[inline]
  pub const fn with_is_primary(mut self, v: bool) -> Self {
    self.is_primary = v;
    self
  }
  #[inline]
  pub const fn set_is_primary(&mut self, v: bool) {
    self.is_primary = v;
  }
  #[inline]
  pub const fn with_auto_selected(mut self, v: bool) -> Self {
    self.auto_selected = v;
    self
  }
  #[inline]
  pub const fn set_auto_selected(&mut self, v: bool) {
    self.auto_selected = v;
  }

  // --- scenes / indexing / provenance ---
  #[inline]
  pub fn with_scenes(mut self, v: impl Into<std::vec::Vec<Id>>) -> Self {
    self.scenes = v.into();
    self
  }
  #[inline]
  pub fn set_scenes(&mut self, v: impl Into<std::vec::Vec<Id>>) {
    self.scenes = v.into();
  }
  #[inline]
  pub const fn with_index_status(mut self, v: VideoIndexStatus) -> Self {
    self.index_status = v;
    self
  }
  #[inline]
  pub const fn set_index_status(&mut self, v: VideoIndexStatus) {
    self.index_status = v;
  }
  #[inline]
  pub fn with_index_errors(mut self, v: impl Into<std::vec::Vec<ErrorInfo>>) -> Self {
    self.index_errors = v.into();
    self
  }
  #[inline]
  pub fn set_index_errors(&mut self, v: impl Into<std::vec::Vec<ErrorInfo>>) {
    self.index_errors = v.into();
  }
  #[inline]
  pub fn with_provenance(mut self, v: Provenance) -> Self {
    self.provenance = v;
    self
  }
  #[inline]
  pub fn set_provenance(&mut self, v: Provenance) {
    self.provenance = v;
  }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Error returned when [`VideoTrack::try_new`] cannot uphold the
/// non-nil-id / non-nil-parent invariants. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant)]
#[non_exhaustive]
pub enum VideoTrackError {
  /// Supplied `id` was the nil sentinel.
  NilId,
  /// Supplied `parent` was the nil sentinel — orphaned track with
  /// no `Video` facet.
  NilParent,
}

impl core::fmt::Display for VideoTrackError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::NilId => f.write_str("VideoTrack id must not be the nil UUID"),
      Self::NilParent => f.write_str("VideoTrack parent (Video facet) must not be the nil UUID"),
    }
  }
}

impl core::error::Error for VideoTrackError {}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use crate::domain::ErrorCode;

  #[test]
  fn try_new_happy_path() {
    let id = Uuid7::new();
    let parent = Uuid7::new();
    let t = VideoTrack::try_new(id, parent).unwrap();
    assert_eq!(t.id(), &id);
    assert_eq!(t.parent(), &parent);
    assert_eq!(t.bit_rate(), 0);
    assert!(t.codec().is_other());
    assert_eq!(t.dimensions(), (0, 0));
    assert!(t.scenes().is_empty());
    assert!(t.index_status().is_empty());
    assert!(t.index_errors().is_empty());
    assert!(t.provenance().is_empty());
  }

  #[test]
  fn try_new_rejects_nil_id_and_parent() {
    assert_eq!(
      VideoTrack::try_new(Uuid7::nil(), Uuid7::new()).err(),
      Some(VideoTrackError::NilId)
    );
    assert_eq!(
      VideoTrack::try_new(Uuid7::new(), Uuid7::nil()).err(),
      Some(VideoTrackError::NilParent)
    );
    assert!(VideoTrackError::NilId.is_nil_id());
    assert!(VideoTrackError::NilParent.is_nil_parent());
  }

  #[test]
  fn builders_and_setters_chain() {
    let s1 = Uuid7::new();
    let s2 = Uuid7::new();
    let t = VideoTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_stream_index(Some(0))
      .with_codec(VideoCodec::H265)
      .with_profile(Some(SmolStr::new("Main10")))
      .with_level(Some(150))
      .with_bit_rate(8_000_000)
      .with_dimensions(3840, 2160)
      .with_frame_rate(24000, 1001, false)
      .with_pixel_format(0x0a) // yuv420p10le-ish
      .with_has_b_frames(true)
      .with_is_primary(true)
      .with_scenes(std::vec![s1, s2])
      .with_index_status(VideoIndexStatus::PROBED)
      .with_index_errors(std::vec![ErrorInfo::code_only(ErrorCode::SceneDetectionFailed)])
      .with_provenance(Provenance::from_parts("qwen2-vl-7b", "v0.3.0", "p@1", "idx-0.1.0"));
    assert_eq!(t.stream_index(), Some(0));
    assert!(matches!(t.codec(), VideoCodec::H265));
    assert_eq!(t.profile(), Some("Main10"));
    assert_eq!(t.level(), Some(150));
    assert_eq!(t.bit_rate(), 8_000_000);
    assert_eq!(t.dimensions(), (3840, 2160));
    assert_eq!(t.frame_rate(), (24000, 1001, false));
    assert!(t.has_b_frames());
    assert!(t.is_primary());
    assert_eq!(t.scenes().len(), 2);
    assert!(t.scenes().contains(&s1));
    assert_eq!(t.index_status(), VideoIndexStatus::PROBED);
    assert_eq!(t.index_errors().len(), 1);
    assert_eq!(t.provenance().model_name(), "qwen2-vl-7b");

    let mut t = t;
    t.set_bit_rate(0);
    t.set_dimensions(0, 0);
    t.set_is_primary(false);
    t.set_index_status(VideoIndexStatus::empty());
    t.set_scenes(std::vec::Vec::<Uuid7>::new());
    assert_eq!(t.bit_rate(), 0);
    assert_eq!(t.dimensions(), (0, 0));
    assert!(!t.is_primary());
    assert!(t.index_status().is_empty());
    assert!(t.scenes().is_empty());
  }

  #[test]
  fn codec_other_preserves_wire_string() {
    let c = VideoCodec::Other(SmolStr::new("xyz-codec"));
    let s: SmolStr = c.try_unwrap_other().unwrap();
    assert_eq!(s.as_str(), "xyz-codec");
  }

  #[test]
  fn mediaframe_placeholders_round_trip() {
    let r = RectPlaceholder::new(0, 0, 1920, 1080);
    assert_eq!(r.width(), 1920);
    assert_eq!(r.height(), 1080);

    let c = ColorInfoPlaceholder::new(9, 16, 9, 1, 2);
    assert_eq!(c.primaries(), 9);
    assert_eq!(c.transfer(), 16);
    assert_eq!(c.matrix(), 9);
    assert_eq!(c.range(), 1);
    assert_eq!(c.chroma_location(), 2);

    let h = HdrStaticMetadataPlaceholder::new(4000, 400);
    assert_eq!(h.max_cll(), 4000);
    assert_eq!(h.max_fall(), 400);

    let d = DolbyVisionConfigPlaceholder::new(8, 9, true, false, true, 1);
    assert_eq!(d.profile(), 8);
    assert_eq!(d.level(), 9);
    assert!(d.rpu_present());
    assert!(!d.el_present());
    assert!(d.bl_present());
    assert_eq!(d.bl_signal_compatibility_id(), 1);
  }
}
