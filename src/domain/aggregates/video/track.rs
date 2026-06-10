//! `VideoTrack` — one video stream of a `Video` facet
//! (locked `schema/video_track.md` r7).
//!
//! Owns stream/codec descriptors, the frame/pixel/colour vocabulary
//! (the `::mediaframe` extern — `codec` / `pixel_format` / `color` /
//! `frame` / `disposition` descriptor types), the per-stream `Scene`
//! id-list, the per-track indexing state, and the per-track
//! `Provenance` (rev 7 hoist — replaces per-`Scene`/per-`Keyframe`
//! provenance).

use std::vec::Vec;

use derive_more::IsVariant;
use mediatime::Timestamp;
use smol_str::SmolStr;

use indexmap::IndexMap;
use mediaframe::{
  codec::VideoCodec,
  color::{DolbyVisionConfig, HdrStaticMetadata, Info as ColorInfo},
  disposition::TrackDisposition,
  frame::{Dimensions, FieldOrder, FrameRate, Rect, Rotation, SampleAspectRatio, StereoMode},
  pixel_format::PixelFormat,
};

use crate::domain::{bitflags::VideoIndexStatus, primitives::ErrorInfo, vo::Provenance, Uuid7};

// ===========================================================================
// VideoTrack
// ===========================================================================

/// One video stream of a [`Video`](super::facet::Video) facet.
///
/// Generic over `Id` (default [`Uuid7`]). **No `Default`** — defaulting
/// to nil `id`/`video_id` would be indistinguishable from an orphan
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
  video_id: Id,

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

  // --- frame / pixel / colour vocabulary (`::mediaframe`) ---
  /// Coded width × height (`mediaframe::frame::Dimensions`).
  dimensions: Dimensions,
  /// Clean-aperture / crop (`mediaframe::frame::Rect`).
  visible_rect: Option<Rect>,
  /// Display aspect / anamorphic (`mediaframe::frame::SampleAspectRatio`).
  sample_aspect_ratio: SampleAspectRatio,
  /// FFmpeg pixfmt (`mediaframe::pixel_format::PixelFormat`).
  pixel_format: PixelFormat,
  /// Primaries / transfer / matrix / range / chroma_location
  /// (`mediaframe::color::Info`).
  color: ColorInfo,
  /// HDR10 static metadata (`mediaframe::color::HdrStaticMetadata`).
  hdr_static: Option<HdrStaticMetadata>,
  /// Display rotation (`mediaframe::frame::Rotation`).
  rotation: Rotation,
  /// Frame rate (`mediaframe::frame::FrameRate`). Corresponds to
  /// `AVStream.r_frame_rate` — the timebase reciprocal (true CFR frame
  /// rate; for VFR content, the LCD of all inter-frame intervals).
  frame_rate: FrameRate,
  /// Empirical / declared average frame rate
  /// (`AVStream.avg_frame_rate`). For CFR content this equals
  /// `frame_rate`; for VFR content the two diverge.
  /// Default = `FrameRate::default()` (`0/1`) — absent.
  avg_frame_rate: FrameRate,
  /// Field order (`mediaframe::frame::FieldOrder`).
  field_order: FieldOrder,
  /// 3D / stereo packing (`mediaframe::frame::StereoMode`).
  stereo_mode: Option<StereoMode>,
  /// Dolby Vision config (`mediaframe::color::DolbyVisionConfig`).
  /// **Not** the same as HDR10 static metadata.
  dovi: Option<DolbyVisionConfig>,

  // --- findit signals ---
  has_embedded_captions: bool,
  /// Disposition flags (`mediaframe::disposition::TrackDisposition`) —
  /// the shared FFmpeg `AV_DISPOSITION_*` set.
  disposition: TrackDisposition,
  is_primary: bool,
  auto_selected: bool,

  // --- per-stream segmented refs ---
  scenes: Vec<Id>,

  // --- container-level AVDictionary bag ---
  /// Container `AVDictionary` entries from this stream's metadata,
  /// **excluding** keys hoisted into dedicated columns (none today on
  /// VideoTrack — every entry rides here). Insertion-ordered.
  metadata: IndexMap<SmolStr, SmolStr>,

  // --- indexing state ---
  index_status: VideoIndexStatus,
  index_errors: Vec<ErrorInfo>,

  // --- analysis-run reproducibility (rev 7 hoist) ---
  provenance: Provenance,
}

// ---------------------------------------------------------------------------
// VideoTrack — validating ctor + accessors
// ---------------------------------------------------------------------------

impl VideoTrack<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` and nil `video_id` (orphan-stream guard).
  /// All other fields take sensible defaults (codec=`Other("")`,
  /// `bit_rate=0`, dimensions zero, every `Option = None`, every flag
  /// false, no scenes, no errors, empty progress, empty provenance);
  /// the indexer fills them in via `with_*` / `set_*` as probing /
  /// detection / analysis stages land.
  pub fn try_new(id: Uuid7, video_id: Uuid7) -> Result<Self, VideoTrackError> {
    if id.is_nil() {
      return Err(VideoTrackError::NilId);
    }
    if video_id.is_nil() {
      return Err(VideoTrackError::NilVideoId);
    }
    Ok(Self {
      id,
      video_id,
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
      dimensions: Dimensions::new(0, 0),
      visible_rect: None,
      sample_aspect_ratio: SampleAspectRatio::default(),
      pixel_format: PixelFormat::default(),
      color: ColorInfo::default(),
      hdr_static: None,
      rotation: Rotation::default(),
      frame_rate: FrameRate::default(),
      avg_frame_rate: FrameRate::default(),
      field_order: FieldOrder::default(),
      stereo_mode: None,
      dovi: None,
      has_embedded_captions: false,
      disposition: TrackDisposition::empty(),
      is_primary: false,
      auto_selected: false,
      scenes: Vec::new(),
      metadata: IndexMap::new(),
      index_status: VideoIndexStatus::empty(),
      index_errors: Vec::new(),
      provenance: Provenance::new(),
    })
  }

  /// Builder: replace the `scenes` child-ref id-list.
  #[must_use]
  #[inline(always)]
  pub fn with_scenes(mut self, v: impl Into<Vec<Uuid7>>) -> Self {
    self.scenes = v.into();
    self
  }

  /// In-place mutator for the `scenes` child-ref id-list.
  #[inline(always)]
  pub fn set_scenes(&mut self, v: impl Into<Vec<Uuid7>>) -> &mut Self {
    self.scenes = v.into();
    self
  }
}

impl<Id> VideoTrack<Id> {
  /// Canonical identity.
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// FK → `Video.id`.
  #[inline(always)]
  pub const fn video_id_ref(&self) -> &Id {
    &self.video_id
  }

  /// Source-locator: ffmpeg `stream_index` / WebCodecs index. Not
  /// identity.
  #[inline(always)]
  pub const fn stream_index(&self) -> Option<u32> {
    self.stream_index
  }

  /// Container-level track id (kept only if the pipeline uses it).
  #[inline(always)]
  pub const fn container_track_id(&self) -> Option<u64> {
    self.container_track_id
  }

  /// Stream start offset / first PTS (mediatime-represented).
  #[inline(always)]
  pub const fn start_pts_ref(&self) -> Option<&Timestamp> {
    self.start_pts.as_ref()
  }

  /// Per-track duration (mediatime placeholder — see `duration` field
  /// comment).
  #[inline(always)]
  pub const fn duration_ref(&self) -> Option<&Timestamp> {
    self.duration.as_ref()
  }

  /// Codec family (locked `VideoCodec` + `Other(SmolStr)` escape).
  #[inline(always)]
  pub const fn codec_ref(&self) -> &VideoCodec {
    &self.codec
  }

  /// Codec profile (`""` semantically absent, but expressed as
  /// `Option<SmolStr>` here because the locked schema names it
  /// `Option<SmolStr>` — distinct from the "empty-string=absent" rule
  /// for plain `SmolStr` fields).
  #[inline(always)]
  pub fn profile(&self) -> Option<&str> {
    self.profile.as_deref()
  }

  /// Codec level (numeric).
  #[inline(always)]
  pub const fn level(&self) -> Option<u16> {
    self.level
  }

  /// Per-track bitrate (0 = unknown).
  #[inline(always)]
  pub const fn bit_rate(&self) -> u64 {
    self.bit_rate
  }

  /// Frame count (exact-duration / progress / VFR signal).
  #[inline(always)]
  pub const fn nb_frames(&self) -> Option<u64> {
    self.nb_frames
  }

  /// Bitstream contains B-frames (seek/cut behaviour).
  #[inline(always)]
  pub const fn has_b_frames(&self) -> bool {
    self.has_b_frames
  }

  /// Closed-GOP (seek/cut behaviour).
  #[inline(always)]
  pub const fn closed_gop(&self) -> Option<bool> {
    self.closed_gop
  }

  /// Coded sample depth (may differ from pixfmt).
  #[inline(always)]
  pub const fn bits_per_raw_sample(&self) -> Option<u8> {
    self.bits_per_raw_sample
  }

  /// Coded width × height (`mediaframe::frame::Dimensions`).
  #[inline(always)]
  pub const fn dimensions(&self) -> Dimensions {
    self.dimensions
  }

  /// Clean-aperture / crop rectangle (`mediaframe::frame::Rect`).
  #[inline(always)]
  pub const fn visible_rect(&self) -> Option<Rect> {
    self.visible_rect
  }

  /// Display aspect / anamorphic ratio
  /// (`mediaframe::frame::SampleAspectRatio`).
  #[inline(always)]
  pub const fn sample_aspect_ratio(&self) -> SampleAspectRatio {
    self.sample_aspect_ratio
  }

  /// FFmpeg pixfmt (`mediaframe::pixel_format::PixelFormat`).
  #[inline(always)]
  pub const fn pixel_format(&self) -> PixelFormat {
    self.pixel_format
  }

  /// Colour primaries / transfer / matrix / range / chroma_location
  /// (`mediaframe::color::Info`).
  #[inline(always)]
  pub const fn color_ref(&self) -> &ColorInfo {
    &self.color
  }

  /// HDR10 static metadata (`mediaframe::color::HdrStaticMetadata`).
  #[inline(always)]
  pub const fn hdr_static_ref(&self) -> Option<&HdrStaticMetadata> {
    self.hdr_static.as_ref()
  }

  /// Display rotation (`mediaframe::frame::Rotation`).
  #[inline(always)]
  pub const fn rotation(&self) -> Rotation {
    self.rotation
  }

  /// Frame rate (`mediaframe::frame::FrameRate` — NOT
  /// `mediatime::Timebase`, see the locked spec).
  #[inline(always)]
  pub const fn frame_rate(&self) -> FrameRate {
    self.frame_rate
  }

  /// `AVStream.avg_frame_rate` — empirical average frame rate. Equals
  /// `frame_rate()` for CFR content; diverges for VFR.
  #[inline(always)]
  pub const fn avg_frame_rate(&self) -> FrameRate {
    self.avg_frame_rate
  }

  /// Container-`AVDictionary` entries from this stream's metadata,
  /// insertion-ordered.
  #[inline(always)]
  pub const fn metadata_ref(&self) -> &IndexMap<SmolStr, SmolStr> {
    &self.metadata
  }

  /// Field order (`mediaframe::frame::FieldOrder`).
  #[inline(always)]
  pub const fn field_order(&self) -> FieldOrder {
    self.field_order
  }

  /// 3D / stereoscopic packing (`mediaframe::frame::StereoMode`).
  #[inline(always)]
  pub const fn stereo_mode(&self) -> Option<StereoMode> {
    self.stereo_mode
  }

  /// Dolby Vision config (`mediaframe::color::DolbyVisionConfig`).
  #[inline(always)]
  pub const fn dovi(&self) -> Option<DolbyVisionConfig> {
    self.dovi
  }

  /// CEA-608/708 captions detected in the bitstream.
  #[inline(always)]
  pub const fn has_embedded_captions(&self) -> bool {
    self.has_embedded_captions
  }

  /// Disposition flags (`mediaframe::disposition::TrackDisposition` —
  /// the shared FFmpeg `AV_DISPOSITION_*` set).
  #[inline(always)]
  pub const fn disposition(&self) -> TrackDisposition {
    self.disposition
  }

  /// Track selection signal — is this the primary video track for the
  /// containing media file?
  #[inline(always)]
  pub const fn is_primary(&self) -> bool {
    self.is_primary
  }

  /// Track selection signal — was this track auto-selected?
  #[inline(always)]
  pub const fn auto_selected(&self) -> bool {
    self.auto_selected
  }

  /// Refs → child [`Scene`](super::scene::Scene)s.
  #[inline(always)]
  pub const fn scenes_slice(&self) -> &[Id] {
    self.scenes.as_slice()
  }

  /// Per-track pipeline-stage status (bitflags).
  #[inline(always)]
  pub const fn index_status(&self) -> VideoIndexStatus {
    self.index_status
  }

  /// Per-track error history (stage-coded).
  #[inline(always)]
  pub const fn index_errors_slice(&self) -> &[ErrorInfo] {
    self.index_errors.as_slice()
  }

  /// Analysis-run reproducibility for *this track-run*; every child
  /// `Scene` / `Keyframe` inherits this rather than carrying its own
  /// (rev 7 hoist).
  #[inline(always)]
  pub const fn provenance_ref(&self) -> &Provenance {
    &self.provenance
  }
}

// ---------------------------------------------------------------------------
// Builders + setters — one per mutable field, matching the
// `with_*` / `set_*` encapsulation rule.
// ---------------------------------------------------------------------------

impl<Id> VideoTrack<Id> {
  // --- source locators ---
  #[must_use]
  #[inline(always)]
  pub const fn with_stream_index(mut self, v: Option<u32>) -> Self {
    self.stream_index = v;
    self
  }
  #[inline(always)]
  pub const fn set_stream_index(&mut self, v: Option<u32>) -> &mut Self {
    self.stream_index = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_container_track_id(mut self, v: Option<u64>) -> Self {
    self.container_track_id = v;
    self
  }
  #[inline(always)]
  pub const fn set_container_track_id(&mut self, v: Option<u64>) -> &mut Self {
    self.container_track_id = v;
    self
  }

  // --- mediatime ---
  #[must_use]
  #[inline(always)]
  pub fn with_start_pts(mut self, v: Option<Timestamp>) -> Self {
    self.start_pts = v;
    self
  }
  #[inline(always)]
  pub fn set_start_pts(&mut self, v: Option<Timestamp>) -> &mut Self {
    self.start_pts = v;
    self
  }
  /// Fallible builder for `duration`.
  ///
  /// `duration` is **semantically a non-negative track-time span** (see
  /// the field doc), but `mediatime::Timestamp` is a signed PTS, so a
  /// `Some(t)` with `t.pts() < 0` would be a nonsense duration. Rejects
  /// that with [`VideoTrackError::NegativeDuration`]; `None` (absent)
  /// and any non-negative `Some` are accepted.
  #[inline]
  pub fn try_with_duration(mut self, v: Option<Timestamp>) -> Result<Self, VideoTrackError> {
    if let Some(t) = v {
      if t.pts() < 0 {
        return Err(VideoTrackError::NegativeDuration);
      }
    }
    self.duration = v;
    Ok(self)
  }
  /// Fallible in-place mutator for `duration` — see
  /// [`VideoTrack::try_with_duration`]. On success returns `&mut Self`
  /// so it chains; on a negative duration returns
  /// [`VideoTrackError::NegativeDuration`] and leaves `self` unchanged.
  #[inline]
  pub fn try_set_duration(&mut self, v: Option<Timestamp>) -> Result<&mut Self, VideoTrackError> {
    if let Some(t) = v {
      if t.pts() < 0 {
        return Err(VideoTrackError::NegativeDuration);
      }
    }
    self.duration = v;
    Ok(self)
  }

  // --- codec ---
  #[must_use]
  #[inline(always)]
  pub fn with_codec(mut self, v: VideoCodec) -> Self {
    self.codec = v;
    self
  }
  #[inline(always)]
  pub fn set_codec(&mut self, v: VideoCodec) -> &mut Self {
    self.codec = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_profile(mut self, v: Option<SmolStr>) -> Self {
    self.profile = v;
    self
  }
  #[inline(always)]
  pub fn set_profile(&mut self, v: Option<SmolStr>) -> &mut Self {
    self.profile = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_level(mut self, v: Option<u16>) -> Self {
    self.level = v;
    self
  }
  #[inline(always)]
  pub const fn set_level(&mut self, v: Option<u16>) -> &mut Self {
    self.level = v;
    self
  }

  // --- bitstream / signal ---
  #[must_use]
  #[inline(always)]
  pub const fn with_bit_rate(mut self, v: u64) -> Self {
    self.bit_rate = v;
    self
  }
  #[inline(always)]
  pub const fn set_bit_rate(&mut self, v: u64) -> &mut Self {
    self.bit_rate = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_nb_frames(mut self, v: Option<u64>) -> Self {
    self.nb_frames = v;
    self
  }
  #[inline(always)]
  pub const fn set_nb_frames(&mut self, v: Option<u64>) -> &mut Self {
    self.nb_frames = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_has_b_frames(mut self, v: bool) -> Self {
    self.has_b_frames = v;
    self
  }
  #[inline(always)]
  pub const fn set_has_b_frames(&mut self, v: bool) -> &mut Self {
    self.has_b_frames = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_closed_gop(mut self, v: Option<bool>) -> Self {
    self.closed_gop = v;
    self
  }
  #[inline(always)]
  pub const fn set_closed_gop(&mut self, v: Option<bool>) -> &mut Self {
    self.closed_gop = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_bits_per_raw_sample(mut self, v: Option<u8>) -> Self {
    self.bits_per_raw_sample = v;
    self
  }
  #[inline(always)]
  pub const fn set_bits_per_raw_sample(&mut self, v: Option<u8>) -> &mut Self {
    self.bits_per_raw_sample = v;
    self
  }

  // --- frame / pixel / colour ---
  /// Fallible builder for `dimensions` (coded width × height).
  ///
  /// `dimensions` must be either the exact `0x0` "unknown" sentinel or
  /// have **both** axes non-zero — a partially-zero `Dimensions`
  /// (e.g. `0x1080`, `1920x0`) is rejected with
  /// [`VideoTrackError::PartialZeroDimensions`].
  ///
  /// `visible_rect` is the clean-aperture crop *within* the coded
  /// frame, so a new `dimensions` must still contain the current
  /// `visible_rect` (if any). Shrinking the coded frame below an
  /// existing crop is rejected with
  /// [`VideoTrackError::CropExceedsDimensions`].
  #[inline]
  pub fn try_with_dimensions(mut self, v: Dimensions) -> Result<Self, VideoTrackError> {
    self.try_set_dimensions(v)?;
    Ok(self)
  }
  /// Fallible in-place mutator for `dimensions` — see
  /// [`VideoTrack::try_with_dimensions`]. On success returns `&mut
  /// Self`; on a partially-zero `Dimensions` returns
  /// [`VideoTrackError::PartialZeroDimensions`], and if the current
  /// `visible_rect` would no longer fit returns
  /// [`VideoTrackError::CropExceedsDimensions`]; on either error
  /// `self` is left unchanged.
  #[inline]
  pub fn try_set_dimensions(&mut self, v: Dimensions) -> Result<&mut Self, VideoTrackError> {
    if !dimensions_valid(v) {
      return Err(VideoTrackError::PartialZeroDimensions);
    }
    if let Some(rect) = self.visible_rect {
      if !rect_fits_dimensions(rect, v) {
        return Err(VideoTrackError::CropExceedsDimensions);
      }
    }
    self.dimensions = v;
    Ok(self)
  }
  /// Fallible builder for `visible_rect` (clean-aperture crop).
  ///
  /// A `Some(rect)` crop must:
  /// - have non-zero `width` and `height` — a zero-extent crop is
  ///   rejected with [`VideoTrackError::ZeroExtentCrop`];
  /// - be set only when `dimensions` are *known* (not the `0x0`
  ///   sentinel) — a crop with unknown dimensions is rejected with
  ///   [`VideoTrackError::CropWithoutDimensions`];
  /// - fit within the coded `dimensions`: `x + width <=
  ///   dimensions.width` and `y + height <= dimensions.height`
  ///   (checked addition) — otherwise
  ///   [`VideoTrackError::CropExceedsDimensions`].
  ///
  /// `None` (no crop) is always accepted.
  #[inline]
  pub fn try_with_visible_rect(mut self, v: Option<Rect>) -> Result<Self, VideoTrackError> {
    self.try_set_visible_rect(v)?;
    Ok(self)
  }
  /// Fallible in-place mutator for `visible_rect` — see
  /// [`VideoTrack::try_with_visible_rect`]. On success returns `&mut
  /// Self`; on a zero-extent crop, a crop without known dimensions, or
  /// a crop past the coded `dimensions`, returns the matching
  /// [`VideoTrackError`] and leaves `self` unchanged.
  #[inline]
  pub fn try_set_visible_rect(&mut self, v: Option<Rect>) -> Result<&mut Self, VideoTrackError> {
    if let Some(rect) = v {
      if rect.width() == 0 || rect.height() == 0 {
        return Err(VideoTrackError::ZeroExtentCrop);
      }
      if !dimensions_known(self.dimensions) {
        return Err(VideoTrackError::CropWithoutDimensions);
      }
      if !rect_fits_dimensions(rect, self.dimensions) {
        return Err(VideoTrackError::CropExceedsDimensions);
      }
    }
    self.visible_rect = v;
    Ok(self)
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_sample_aspect_ratio(mut self, v: SampleAspectRatio) -> Self {
    self.sample_aspect_ratio = v;
    self
  }
  #[inline(always)]
  pub const fn set_sample_aspect_ratio(&mut self, v: SampleAspectRatio) -> &mut Self {
    self.sample_aspect_ratio = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_pixel_format(mut self, v: PixelFormat) -> Self {
    self.pixel_format = v;
    self
  }
  #[inline(always)]
  pub const fn set_pixel_format(&mut self, v: PixelFormat) -> &mut Self {
    self.pixel_format = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_color(mut self, v: ColorInfo) -> Self {
    self.color = v;
    self
  }
  #[inline(always)]
  pub const fn set_color(&mut self, v: ColorInfo) -> &mut Self {
    self.color = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_hdr_static(mut self, v: Option<HdrStaticMetadata>) -> Self {
    self.hdr_static = v;
    self
  }
  #[inline(always)]
  pub const fn set_hdr_static(&mut self, v: Option<HdrStaticMetadata>) -> &mut Self {
    self.hdr_static = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_rotation(mut self, v: Rotation) -> Self {
    self.rotation = v;
    self
  }
  #[inline(always)]
  pub const fn set_rotation(&mut self, v: Rotation) -> &mut Self {
    self.rotation = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_frame_rate(mut self, v: FrameRate) -> Self {
    self.frame_rate = v;
    self
  }
  #[inline(always)]
  pub const fn set_frame_rate(&mut self, v: FrameRate) -> &mut Self {
    self.frame_rate = v;
    self
  }
  /// Builder: replace `avg_frame_rate`.
  #[must_use]
  #[inline(always)]
  pub const fn with_avg_frame_rate(mut self, v: FrameRate) -> Self {
    self.avg_frame_rate = v;
    self
  }
  /// In-place mutator: replace `avg_frame_rate`.
  #[inline(always)]
  pub const fn set_avg_frame_rate(&mut self, v: FrameRate) -> &mut Self {
    self.avg_frame_rate = v;
    self
  }
  /// Builder: replace the container-`AVDictionary` metadata bag.
  #[must_use]
  #[inline(always)]
  pub fn with_metadata(mut self, v: IndexMap<SmolStr, SmolStr>) -> Self {
    self.metadata = v;
    self
  }
  /// In-place mutator: replace the container-`AVDictionary` metadata bag.
  #[inline(always)]
  pub fn set_metadata(&mut self, v: IndexMap<SmolStr, SmolStr>) -> &mut Self {
    self.metadata = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_field_order(mut self, v: FieldOrder) -> Self {
    self.field_order = v;
    self
  }
  #[inline(always)]
  pub const fn set_field_order(&mut self, v: FieldOrder) -> &mut Self {
    self.field_order = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_stereo_mode(mut self, v: Option<StereoMode>) -> Self {
    self.stereo_mode = v;
    self
  }
  #[inline(always)]
  pub const fn set_stereo_mode(&mut self, v: Option<StereoMode>) -> &mut Self {
    self.stereo_mode = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_dovi(mut self, v: Option<DolbyVisionConfig>) -> Self {
    self.dovi = v;
    self
  }
  #[inline(always)]
  pub const fn set_dovi(&mut self, v: Option<DolbyVisionConfig>) -> &mut Self {
    self.dovi = v;
    self
  }

  // --- findit signals ---
  #[must_use]
  #[inline(always)]
  pub const fn with_has_embedded_captions(mut self, v: bool) -> Self {
    self.has_embedded_captions = v;
    self
  }
  #[inline(always)]
  pub const fn set_has_embedded_captions(&mut self, v: bool) -> &mut Self {
    self.has_embedded_captions = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_disposition(mut self, v: TrackDisposition) -> Self {
    self.disposition = v;
    self
  }
  #[inline(always)]
  pub const fn set_disposition(&mut self, v: TrackDisposition) -> &mut Self {
    self.disposition = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_is_primary(mut self, v: bool) -> Self {
    self.is_primary = v;
    self
  }
  #[inline(always)]
  pub const fn set_is_primary(&mut self, v: bool) -> &mut Self {
    self.is_primary = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_auto_selected(mut self, v: bool) -> Self {
    self.auto_selected = v;
    self
  }
  #[inline(always)]
  pub const fn set_auto_selected(&mut self, v: bool) -> &mut Self {
    self.auto_selected = v;
    self
  }

  // --- indexing / provenance ---
  #[must_use]
  #[inline(always)]
  pub const fn with_index_status(mut self, v: VideoIndexStatus) -> Self {
    self.index_status = v;
    self
  }
  #[inline(always)]
  pub const fn set_index_status(&mut self, v: VideoIndexStatus) -> &mut Self {
    self.index_status = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_index_errors(mut self, v: impl Into<Vec<ErrorInfo>>) -> Self {
    self.index_errors = v.into();
    self
  }
  #[inline(always)]
  pub fn set_index_errors(&mut self, v: impl Into<Vec<ErrorInfo>>) -> &mut Self {
    self.index_errors = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_provenance(mut self, v: Provenance) -> Self {
    self.provenance = v;
    self
  }
  #[inline(always)]
  pub fn set_provenance(&mut self, v: Provenance) -> &mut Self {
    self.provenance = v;
    self
  }
}

// ---------------------------------------------------------------------------
// Crop-geometry helper
// ---------------------------------------------------------------------------

/// True iff `dims` is a *valid* coded `Dimensions`: either the exact
/// `0x0` "unknown" sentinel, or **both** axes non-zero. A partially-zero
/// `Dimensions` (`0x1080`, `1920x0`) is rejected — it is neither a real
/// frame size nor the unknown sentinel.
#[inline]
const fn dimensions_valid(dims: Dimensions) -> bool {
  let (w, h) = (dims.width(), dims.height());
  (w == 0 && h == 0) || (w != 0 && h != 0)
}

/// True iff `dims` is *known* — i.e. not the `0x0` "unknown" sentinel.
/// A `visible_rect` crop is meaningful only against known dimensions.
#[inline]
const fn dimensions_known(dims: Dimensions) -> bool {
  dims.width() != 0 && dims.height() != 0
}

/// True iff `rect` (clean-aperture crop) fits entirely within `dims`
/// (coded frame): `rect.x + rect.width <= dims.width` and `rect.y +
/// rect.height <= dims.height`, using checked addition so an
/// `x + width` overflow is treated as out-of-bounds.
#[inline]
const fn rect_fits_dimensions(rect: Rect, dims: Dimensions) -> bool {
  let right = match rect.x().checked_add(rect.width()) {
    Some(v) => v,
    None => return false,
  };
  let bottom = match rect.y().checked_add(rect.height()) {
    Some(v) => v,
    None => return false,
  };
  right <= dims.width() && bottom <= dims.height()
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Error returned when a [`VideoTrack`] constructor or fallible mutator
/// cannot uphold an invariant. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum VideoTrackError {
  /// Supplied `id` was the nil sentinel.
  #[error("VideoTrack id must not be the nil UUID")]
  NilId,
  /// Supplied `video_id` was the nil sentinel — orphaned track with
  /// no `Video` facet.
  #[error("VideoTrack video_id (Video facet) must not be the nil UUID")]
  NilVideoId,
  /// Supplied `duration` was `Some(t)` with `t.pts() < 0` — a duration
  /// is semantically non-negative (see the `duration` field doc).
  #[error("VideoTrack duration must not be negative")]
  NegativeDuration,
  /// The clean-aperture crop (`visible_rect`) does not fit within the
  /// coded `dimensions` — either a crop extends past the coded frame
  /// (`x + width > dimensions.width` / `y + height >
  /// dimensions.height`), or `dimensions` was shrunk below an existing
  /// crop.
  #[error("VideoTrack visible_rect crop must fit within the coded dimensions")]
  CropExceedsDimensions,
  /// `dimensions` had exactly one axis zero (e.g. `0x1080`, `1920x0`).
  /// Coded dimensions must be either the `0x0` "unknown" sentinel or
  /// have both axes non-zero.
  #[error("VideoTrack dimensions must be 0x0 (unknown) or have both axes non-zero")]
  PartialZeroDimensions,
  /// A `visible_rect` crop had zero `width` or `height` — a zero-extent
  /// crop is degenerate.
  #[error("VideoTrack visible_rect crop must have non-zero width and height")]
  ZeroExtentCrop,
  /// A `visible_rect` crop was set while `dimensions` were unknown (the
  /// `0x0` sentinel) — a crop is only meaningful against known coded
  /// dimensions.
  #[error("VideoTrack visible_rect crop requires known (non-zero) dimensions")]
  CropWithoutDimensions,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use crate::domain::ErrorCode;
  use core::num::NonZeroU32;
  use mediaframe::frame::Rational;
  use mediatime::Timebase;

  #[test]
  fn try_new_happy_path() {
    let id = Uuid7::new();
    let video_id = Uuid7::new();
    let t = VideoTrack::try_new(id, video_id).unwrap();
    assert_eq!(t.id_ref(), &id);
    assert_eq!(t.video_id_ref(), &video_id);
    assert_eq!(t.bit_rate(), 0);
    assert!(t.codec_ref().is_other());
    assert_eq!(t.dimensions(), Dimensions::new(0, 0));
    assert!(t.scenes_slice().is_empty());
    assert!(t.index_status().is_empty());
    assert!(t.index_errors_slice().is_empty());
    assert!(t.provenance_ref().is_empty());
  }

  #[test]
  fn try_new_rejects_nil_id_and_parent() {
    assert_eq!(
      VideoTrack::try_new(Uuid7::nil(), Uuid7::new()).err(),
      Some(VideoTrackError::NilId)
    );
    assert_eq!(
      VideoTrack::try_new(Uuid7::new(), Uuid7::nil()).err(),
      Some(VideoTrackError::NilVideoId)
    );
    assert!(VideoTrackError::NilId.is_nil_id());
    assert!(VideoTrackError::NilVideoId.is_nil_video_id());
  }

  #[test]
  fn builders_and_setters_chain() {
    let s1 = Uuid7::new();
    let s2 = Uuid7::new();
    let fr = FrameRate::new(Rational::new(24000, NonZeroU32::new(1001).unwrap()), false);
    let t = VideoTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_stream_index(Some(0))
      .with_codec(VideoCodec::Hevc)
      .with_profile(Some(SmolStr::new("Main10")))
      .with_level(Some(150))
      .with_bit_rate(8_000_000)
      .try_with_dimensions(Dimensions::new(3840, 2160))
      .unwrap()
      .with_frame_rate(fr)
      .with_pixel_format(PixelFormat::from_u32(0x0a)) // yuv420p10le-ish
      .with_has_b_frames(true)
      .with_is_primary(true)
      .with_scenes(std::vec![s1, s2])
      .with_index_status(VideoIndexStatus::PROBED)
      .with_index_errors(std::vec![ErrorInfo::code_only(ErrorCode::SceneDetectionFailed)])
      .with_provenance(Provenance::from_parts("qwen2-vl-7b", "v0.3.0", "p@1", "idx-0.1.0"));
    assert_eq!(t.stream_index(), Some(0));
    assert!(matches!(t.codec_ref(), VideoCodec::Hevc));
    assert_eq!(t.profile(), Some("Main10"));
    assert_eq!(t.level(), Some(150));
    assert_eq!(t.bit_rate(), 8_000_000);
    assert_eq!(t.dimensions(), Dimensions::new(3840, 2160));
    assert_eq!(t.frame_rate(), fr);
    assert!(t.has_b_frames());
    assert!(t.is_primary());
    assert_eq!(t.scenes_slice().len(), 2);
    assert!(t.scenes_slice().contains(&s1));
    assert_eq!(t.index_status(), VideoIndexStatus::PROBED);
    assert_eq!(t.index_errors_slice().len(), 1);
    assert_eq!(t.provenance_ref().model_name(), "qwen2-vl-7b");

    let mut t = t;
    t.set_bit_rate(0);
    t.try_set_dimensions(Dimensions::new(0, 0)).unwrap();
    t.set_is_primary(false);
    t.set_index_status(VideoIndexStatus::empty());
    t.set_scenes(Vec::<Uuid7>::new());
    assert_eq!(t.bit_rate(), 0);
    assert_eq!(t.dimensions(), Dimensions::new(0, 0));
    assert!(!t.is_primary());
    assert!(t.index_status().is_empty());
    assert!(t.scenes_slice().is_empty());
  }

  #[test]
  fn duration_rejects_negative_timestamp() {
    let tb = Timebase::new(1, NonZeroU32::new(1000).unwrap());
    let t = VideoTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();

    // Negative duration is rejected through the consuming builder...
    assert_eq!(
      t.clone()
        .try_with_duration(Some(Timestamp::new(-1, tb)))
        .err(),
      Some(VideoTrackError::NegativeDuration)
    );

    // ...and through the in-place setter, which leaves `self` unchanged.
    let mut t = t;
    assert_eq!(
      t.try_set_duration(Some(Timestamp::new(-5_000, tb))).err(),
      Some(VideoTrackError::NegativeDuration)
    );
    assert!(t.duration_ref().is_none());
    assert!(VideoTrackError::NegativeDuration.is_negative_duration());

    // Zero and positive durations, and `None`, are accepted.
    let t = t
      .try_with_duration(Some(Timestamp::new(0, tb)))
      .unwrap()
      .try_with_duration(Some(Timestamp::new(48_000, tb)))
      .unwrap();
    assert_eq!(t.duration_ref().map(Timestamp::pts), Some(48_000));
    let mut t = t;
    t.try_set_duration(None).unwrap();
    assert!(t.duration_ref().is_none());
  }

  #[test]
  fn codec_other_preserves_wire_string() {
    let c = VideoCodec::Other(SmolStr::new("xyz-codec"));
    assert!(c.is_other());
    assert_eq!(c.as_str(), "xyz-codec");
  }

  #[test]
  fn mediaframe_descriptors_flow_through() {
    let t = VideoTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .try_with_dimensions(Dimensions::new(1920, 1080))
      .unwrap()
      .try_with_visible_rect(Some(Rect::new(0, 0, 1920, 1080)))
      .unwrap()
      .with_sample_aspect_ratio(SampleAspectRatio::new(16, NonZeroU32::new(9).unwrap()))
      .with_rotation(Rotation::D90)
      .with_field_order(FieldOrder::Progressive)
      .with_stereo_mode(Some(StereoMode::SideBySide))
      .with_color(ColorInfo::UNSPECIFIED)
      .with_hdr_static(Some(HdrStaticMetadata::new(None, None)))
      .with_dovi(Some(DolbyVisionConfig::new(8, 9, true, false, 1)))
      .with_disposition(TrackDisposition::empty());
    assert_eq!(t.visible_rect().unwrap().width(), 1920);
    assert_eq!(t.sample_aspect_ratio().num(), 16);
    assert!(matches!(t.rotation(), Rotation::D90));
    assert!(matches!(t.field_order(), FieldOrder::Progressive));
    assert!(matches!(t.stereo_mode(), Some(StereoMode::SideBySide)));
    assert_eq!(t.color_ref(), &ColorInfo::UNSPECIFIED);
    assert!(t.hdr_static_ref().is_some());
    assert_eq!(t.dovi().unwrap().profile(), 8);
    assert!(t.disposition().is_empty());
  }

  #[test]
  fn crop_must_fit_within_coded_dimensions() {
    // rev-3 finding 3: `dimensions` and `visible_rect` were
    // independently settable — the crop could exceed the coded frame.
    let t = VideoTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .try_with_dimensions(Dimensions::new(1920, 1080))
      .unwrap();

    // Direction 1: a crop that runs past the coded frame is rejected.
    assert_eq!(
      t.clone()
        .try_with_visible_rect(Some(Rect::new(100, 0, 1900, 1080)))
        .err(),
      Some(VideoTrackError::CropExceedsDimensions)
    );
    assert_eq!(
      t.clone()
        .try_with_visible_rect(Some(Rect::new(0, 100, 1920, 1000)))
        .err(),
      Some(VideoTrackError::CropExceedsDimensions)
    );
    assert!(VideoTrackError::CropExceedsDimensions.is_crop_exceeds_dimensions());

    // A crop flush against the coded edge is allowed.
    let t = t
      .try_with_visible_rect(Some(Rect::new(0, 0, 1920, 1080)))
      .unwrap();

    // Direction 2: shrinking `dimensions` below the existing crop is
    // rejected, and the in-place setter leaves `self` unchanged.
    let mut t = t;
    assert_eq!(
      t.try_set_dimensions(Dimensions::new(1280, 720)).err(),
      Some(VideoTrackError::CropExceedsDimensions)
    );
    assert_eq!(t.dimensions(), Dimensions::new(1920, 1080));
    assert_eq!(t.visible_rect(), Some(Rect::new(0, 0, 1920, 1080)));

    // The in-place crop setter also rejects + leaves `self` unchanged.
    assert_eq!(
      t.try_set_visible_rect(Some(Rect::new(0, 0, 4000, 4000)))
        .err(),
      Some(VideoTrackError::CropExceedsDimensions)
    );
    assert_eq!(t.visible_rect(), Some(Rect::new(0, 0, 1920, 1080)));

    // Growing `dimensions` keeps the crop valid; `None` crop is always
    // fine.
    t.try_set_dimensions(Dimensions::new(3840, 2160)).unwrap();
    t.try_set_visible_rect(None).unwrap();
    assert!(t.visible_rect().is_none());
    // With no crop, any `dimensions` is accepted.
    t.try_set_dimensions(Dimensions::new(0, 0)).unwrap();
  }

  #[test]
  fn dimensions_reject_partial_zero() {
    // rev-4 finding 2: `Dimensions::new(0, 1080)` (one axis zero)
    // passed the round-3 crop check.
    let mut t = VideoTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    assert_eq!(
      t.try_set_dimensions(Dimensions::new(0, 1080)).err(),
      Some(VideoTrackError::PartialZeroDimensions)
    );
    assert_eq!(
      t.try_set_dimensions(Dimensions::new(1920, 0)).err(),
      Some(VideoTrackError::PartialZeroDimensions)
    );
    assert_eq!(t.dimensions(), Dimensions::new(0, 0));
    assert!(VideoTrackError::PartialZeroDimensions.is_partial_zero_dimensions());
    // `0x0` (unknown) and fully-non-zero are both accepted.
    t.try_set_dimensions(Dimensions::new(0, 0)).unwrap();
    t.try_set_dimensions(Dimensions::new(1920, 1080)).unwrap();
  }

  #[test]
  fn crop_rejects_zero_extent_and_unknown_dimensions() {
    // rev-4 finding 2: a zero-extent rect, and a crop with no known
    // dimensions, were both accepted by the round-3 containment check.
    let t = VideoTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();

    // A crop set while dimensions are unknown (`0x0`) is rejected.
    assert_eq!(
      t.clone()
        .try_with_visible_rect(Some(Rect::new(0, 0, 100, 100)))
        .err(),
      Some(VideoTrackError::CropWithoutDimensions)
    );
    assert!(VideoTrackError::CropWithoutDimensions.is_crop_without_dimensions());

    let t = t.try_with_dimensions(Dimensions::new(1920, 1080)).unwrap();

    // A zero-width / zero-height crop is rejected.
    assert_eq!(
      t.clone()
        .try_with_visible_rect(Some(Rect::new(0, 0, 0, 1080)))
        .err(),
      Some(VideoTrackError::ZeroExtentCrop)
    );
    assert_eq!(
      t.clone()
        .try_with_visible_rect(Some(Rect::new(0, 0, 1920, 0)))
        .err(),
      Some(VideoTrackError::ZeroExtentCrop)
    );
    assert!(VideoTrackError::ZeroExtentCrop.is_zero_extent_crop());

    // A normal non-zero crop within known dimensions still passes.
    let t = t
      .try_with_visible_rect(Some(Rect::new(10, 10, 100, 100)))
      .unwrap();
    assert_eq!(t.visible_rect(), Some(Rect::new(10, 10, 100, 100)));
  }
}

/// Exhaustive by-value decomposition of [`VideoTrack`] — every stored
/// field.
///
/// Public-field data-transfer struct (the conversion-boundary exception
/// to the encapsulation rule): cross-suite conversions (`crate::graph`)
/// destructure it exhaustively, so adding a field breaks them at compile
/// time instead of silently dropping data.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoTrackParts<Id = Uuid7> {
  pub id: Id,
  pub video_id: Id,
  pub stream_index: Option<u32>,
  pub container_track_id: Option<u64>,
  pub start_pts: Option<Timestamp>,
  pub duration: Option<Timestamp>,
  pub codec: VideoCodec,
  pub profile: Option<SmolStr>,
  pub level: Option<u16>,
  pub bit_rate: u64,
  pub nb_frames: Option<u64>,
  pub has_b_frames: bool,
  pub closed_gop: Option<bool>,
  pub bits_per_raw_sample: Option<u8>,
  pub dimensions: Dimensions,
  pub visible_rect: Option<Rect>,
  pub sample_aspect_ratio: SampleAspectRatio,
  pub pixel_format: PixelFormat,
  pub color: ColorInfo,
  pub hdr_static: Option<HdrStaticMetadata>,
  pub rotation: Rotation,
  pub frame_rate: FrameRate,
  pub avg_frame_rate: FrameRate,
  pub field_order: FieldOrder,
  pub stereo_mode: Option<StereoMode>,
  pub dovi: Option<DolbyVisionConfig>,
  pub has_embedded_captions: bool,
  pub disposition: TrackDisposition,
  pub is_primary: bool,
  pub auto_selected: bool,
  pub scenes: Vec<Id>,
  pub metadata: IndexMap<SmolStr, SmolStr>,
  pub index_status: VideoIndexStatus,
  pub index_errors: Vec<ErrorInfo>,
  pub provenance: Provenance,
}

impl<Id> VideoTrack<Id> {
  /// Decompose into [`VideoTrackParts`] — exhaustive, by value.
  #[inline(always)]
  pub fn into_parts(self) -> VideoTrackParts<Id> {
    let Self {
      id,
      video_id,
      stream_index,
      container_track_id,
      start_pts,
      duration,
      codec,
      profile,
      level,
      bit_rate,
      nb_frames,
      has_b_frames,
      closed_gop,
      bits_per_raw_sample,
      dimensions,
      visible_rect,
      sample_aspect_ratio,
      pixel_format,
      color,
      hdr_static,
      rotation,
      frame_rate,
      avg_frame_rate,
      field_order,
      stereo_mode,
      dovi,
      has_embedded_captions,
      disposition,
      is_primary,
      auto_selected,
      scenes,
      metadata,
      index_status,
      index_errors,
      provenance,
    } = self;
    VideoTrackParts {
      id,
      video_id,
      stream_index,
      container_track_id,
      start_pts,
      duration,
      codec,
      profile,
      level,
      bit_rate,
      nb_frames,
      has_b_frames,
      closed_gop,
      bits_per_raw_sample,
      dimensions,
      visible_rect,
      sample_aspect_ratio,
      pixel_format,
      color,
      hdr_static,
      rotation,
      frame_rate,
      avg_frame_rate,
      field_order,
      stereo_mode,
      dovi,
      has_embedded_captions,
      disposition,
      is_primary,
      auto_selected,
      scenes,
      metadata,
      index_status,
      index_errors,
      provenance,
    }
  }
}

impl<Id> VideoTrack<Id> {
  /// Invariant-carrying constructor from [`VideoTrackParts`] —
  /// `pub(crate)`, reserved for in-crate conversions from
  /// already-validated values (`crate::graph`).
  #[cfg(all(
    feature = "std",
    feature = "video",
    feature = "audio",
    feature = "subtitle"
  ))]
  #[inline(always)]
  pub(crate) fn rehydrate(parts: VideoTrackParts<Id>) -> Self {
    let VideoTrackParts {
      id,
      video_id,
      stream_index,
      container_track_id,
      start_pts,
      duration,
      codec,
      profile,
      level,
      bit_rate,
      nb_frames,
      has_b_frames,
      closed_gop,
      bits_per_raw_sample,
      dimensions,
      visible_rect,
      sample_aspect_ratio,
      pixel_format,
      color,
      hdr_static,
      rotation,
      frame_rate,
      avg_frame_rate,
      field_order,
      stereo_mode,
      dovi,
      has_embedded_captions,
      disposition,
      is_primary,
      auto_selected,
      scenes,
      metadata,
      index_status,
      index_errors,
      provenance,
    } = parts;
    Self {
      id,
      video_id,
      stream_index,
      container_track_id,
      start_pts,
      duration,
      codec,
      profile,
      level,
      bit_rate,
      nb_frames,
      has_b_frames,
      closed_gop,
      bits_per_raw_sample,
      dimensions,
      visible_rect,
      sample_aspect_ratio,
      pixel_format,
      color,
      hdr_static,
      rotation,
      frame_rate,
      avg_frame_rate,
      field_order,
      stereo_mode,
      dovi,
      has_embedded_captions,
      disposition,
      is_primary,
      auto_selected,
      scenes,
      metadata,
      index_status,
      index_errors,
      provenance,
    }
  }
}
