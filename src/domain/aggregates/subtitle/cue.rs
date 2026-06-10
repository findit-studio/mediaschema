//! Polymorphic `SubtitleCue<Id, D>` — one parsed cue of a `SubtitleTrack`,
//! parameterised by a per-format payload `D` (`SrtData`, `VttData<Id>`,
//! `AssData<Id>`, `LrcData`, …).
//!
//! See `schema/subtitle_cues.md` (rev 5 — polymorphic base) for the
//! design overview and the per-format detail docs
//! (`schema/subtitle_cue_{vtt,ass,lrc}.md`) for the implemented formats.
//!
//! ## Foundation
//!
//! - [`SubtitleCue`]`<Id, D>` — generic base. Carries the format-agnostic
//!   fields (id, parent track id, ordinal, span, plain `text`) and a
//!   format-specific payload `data: D`.
//! - [`SubtitleCueKind`] — closed enum discriminating every format we
//!   plan to support. All 13 variants exist from day one so the
//!   discriminator wire/SQL value is stable; deferred variants land in
//!   issue #56.
//! - Type aliases: [`SrtCue`], [`VttCue`], [`AssCue`], [`LrcCue`].
//! - Per-format `D` data types: [`SrtData`], [`VttData`], [`AssData`],
//!   [`LrcData`].
//! - Per-track aggregate types: [`VttRegion`], [`VttStyleBlock`],
//!   [`AssStyle`], [`LrcMetadata`], [`LrcWord`].
//!
//! ## Identity / invariants
//!
//! `id` and `subtitle_track_id` are validated non-nil at construction
//! (every cue needs a real LanceDB embedding key + a real parent FK).
//! `text` is `LocalizedText` — empty (`""`) is a legal value (an
//! un-OCR'd bitmap cue, or an ASS cue whose `styled_text` carries the
//! display text). The text-presence invariant of the rev-3 design
//! has been **lifted**: a polymorphic cue's content lives in `data`,
//! not always in `text`.

use bytes::Bytes;
use derive_more::{Display, IsVariant};
use mediatime::TimeRange;
use smol_str::SmolStr;

use crate::domain::{vo::LocalizedText, Uuid7};

// ===========================================================================
// SubtitleCueKind — closed discriminator (all 13 formats, day-1 stable)
// ===========================================================================

/// Closed discriminator for [`SubtitleCue`]'s `data` payload.
///
/// Every value in this enum exists from day one so the on-disk / on-wire
/// integer discriminant is stable across releases. Variants flagged
/// **reserved** have no associated `D` data type or row mapping yet —
/// implementation is tracked in GitHub issue #56.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IsVariant, Display)]
#[display("{}", self.as_str())]
#[non_exhaustive]
#[repr(u8)]
pub enum SubtitleCueKind {
  /// SubRip — line-only text, no detail table.
  Srt = 0,
  /// WebVTT — text + positioning + voice + region/style aggregates.
  Vtt = 1,
  /// Advanced SubStation Alpha / SubStation Alpha — `Dialogue` + Style
  /// aggregate.
  Ass = 2,
  /// MicroDVD — reserved; implementation deferred to issue #56.
  MicroDvd = 3,
  /// SubViewer — reserved; implementation deferred to issue #56.
  SubViewer = 4,
  /// YouTube SBV — reserved; implementation deferred to issue #56.
  Sbv = 5,
  /// LRC / Enhanced LRC — line + word-level lyric timing.
  Lrc = 6,
  /// Timed Text Markup Language — reserved; implementation deferred to
  /// issue #56.
  Ttml = 7,
  /// Synchronized Accessible Media Interchange — reserved; implementation
  /// deferred to issue #56.
  Sami = 8,
  /// DVD VobSub bitmap. Per-cue palette index + bitmap blob; per-track
  /// 16-entry RGB palette aggregate.
  VobSub = 9,
  /// Blu-ray PGS bitmap. Per-cue palette bytes + bitmap blob.
  Pgs = 10,
  /// CEA-608 line-21 captions. The auto-derived predicate would be
  /// `is_cea_608` (digit-snake-case); the hand-written
  /// [`Self::is_cea608`] uses the cleaner name (`Cea608` is the
  /// canonical industry spelling and can't be renamed away from the
  /// digit).
  #[is_variant(ignore)]
  Cea608 = 11,
  /// EBU STL teletext.
  EbuStl = 12,
}

impl SubtitleCueKind {
  /// True iff this is [`Self::Cea608`]. Hand-written to override the
  /// auto-derived `is_cea_608` (digit-snake-case is ugly; `Cea608` is
  /// the canonical industry name and can't be renamed).
  #[inline(always)]
  pub const fn is_cea608(&self) -> bool {
    matches!(self, Self::Cea608)
  }

  /// Numeric discriminant (stable across releases).
  #[inline(always)]
  pub const fn to_u8(self) -> u8 {
    self as u8
  }

  /// Decode the numeric discriminant.
  ///
  /// Returns `None` for an unknown value (the enum is `#[non_exhaustive]`
  /// but the wire value space is bounded by the discriminants reserved
  /// above).
  #[inline(always)]
  pub const fn try_from_u8(v: u8) -> Option<Self> {
    Some(match v {
      0 => Self::Srt,
      1 => Self::Vtt,
      2 => Self::Ass,
      3 => Self::MicroDvd,
      4 => Self::SubViewer,
      5 => Self::Sbv,
      6 => Self::Lrc,
      7 => Self::Ttml,
      8 => Self::Sami,
      9 => Self::VobSub,
      10 => Self::Pgs,
      11 => Self::Cea608,
      12 => Self::EbuStl,
      _ => return None,
    })
  }

  /// True when the kind has been implemented in this revision. All 13
  /// formats are now implemented; the predicate is kept for symmetry
  /// with future additions.
  #[inline(always)]
  pub const fn is_implemented(self) -> bool {
    matches!(
      self,
      Self::Srt
        | Self::Vtt
        | Self::Ass
        | Self::Lrc
        | Self::MicroDvd
        | Self::SubViewer
        | Self::Sbv
        | Self::Ttml
        | Self::Sami
        | Self::VobSub
        | Self::Pgs
        | Self::Cea608
        | Self::EbuStl,
    )
  }

  /// Stable snake_case slug — the canonical string form of every variant.
  ///
  /// Used for `Display`, serde tags, log keys, schema-doc references,
  /// and round-trip tests (`from_str(x.as_str()) == x` via [`from_str`](Self::from_str)).
  /// Slugs are stable across releases — the same wire/storage contract
  /// as [`to_u8`](Self::to_u8). For the two hyphenated standards
  /// (`CEA-608`, `EBU-STL`) the slug uses snake_case (`cea_608`,
  /// `ebu_stl`) for uniformity with the rest of the enum.
  #[inline(always)]
  pub const fn as_str(&self) -> &'static str {
    match self {
      Self::Srt => "srt",
      Self::Vtt => "vtt",
      Self::Ass => "ass",
      Self::MicroDvd => "micro_dvd",
      Self::SubViewer => "sub_viewer",
      Self::Sbv => "sbv",
      Self::Lrc => "lrc",
      Self::Ttml => "ttml",
      Self::Sami => "sami",
      Self::VobSub => "vob_sub",
      Self::Pgs => "pgs",
      Self::Cea608 => "cea_608",
      Self::EbuStl => "ebu_stl",
    }
  }

  /// Inverse of [`as_str`](Self::as_str). Returns `None` for any input
  /// that isn't an exact match of one of the slugs.
  #[inline]
  pub fn from_str(s: &str) -> Option<Self> {
    Some(match s {
      "srt" => Self::Srt,
      "vtt" => Self::Vtt,
      "ass" => Self::Ass,
      "micro_dvd" => Self::MicroDvd,
      "sub_viewer" => Self::SubViewer,
      "sbv" => Self::Sbv,
      "lrc" => Self::Lrc,
      "ttml" => Self::Ttml,
      "sami" => Self::Sami,
      "vob_sub" => Self::VobSub,
      "pgs" => Self::Pgs,
      "cea_608" => Self::Cea608,
      "ebu_stl" => Self::EbuStl,
      _ => return None,
    })
  }

  /// Primary file-on-disk extension (without the leading dot —
  /// `"srt"`, `"vtt"`, `"ass"`, …). Distinct from [`as_str`](Self::as_str),
  /// which returns the snake_case enum slug used for serialization /
  /// logging; this returns the conventional extension the format uses
  /// when written to disk as a standalone file:
  ///
  /// | variant | `as_str` | `as_extension` |
  /// |---|---|---|
  /// | `Srt` | `"srt"` | `"srt"` |
  /// | `Vtt` | `"vtt"` | `"vtt"` |
  /// | `Ass` | `"ass"` | `"ass"` |
  /// | `MicroDvd` | `"micro_dvd"` | `"sub"` |
  /// | `SubViewer` | `"sub_viewer"` | `"sub"` |
  /// | `Sbv` | `"sbv"` | `"sbv"` |
  /// | `Lrc` | `"lrc"` | `"lrc"` |
  /// | `Ttml` | `"ttml"` | `"ttml"` |
  /// | `Sami` | `"sami"` | `"smi"` |
  /// | `VobSub` | `"vob_sub"` | `"idx"` (index half of the `.idx`/`.sub` pair) |
  /// | `Pgs` | `"pgs"` | `"sup"` |
  /// | `Cea608` | `"cea_608"` | `""` (broadcast-embedded, no standalone extension) |
  /// | `EbuStl` | `"ebu_stl"` | `"stl"` |
  ///
  /// `MicroDvd` and `SubViewer` legitimately share `"sub"` — both write
  /// to that extension; the format is disambiguated by content sniffing,
  /// not by name. `Cea608` returns `""` because the captions are
  /// container-embedded (in MPEG-TS / line-21 broadcast) with no
  /// standalone disk form.
  #[inline(always)]
  pub const fn as_extension(&self) -> &'static str {
    match self {
      Self::Srt => "srt",
      Self::Vtt => "vtt",
      Self::Ass => "ass",
      Self::MicroDvd => "sub",
      Self::SubViewer => "sub",
      Self::Sbv => "sbv",
      Self::Lrc => "lrc",
      Self::Ttml => "ttml",
      Self::Sami => "smi",
      Self::VobSub => "idx",
      Self::Pgs => "sup",
      Self::Cea608 => "",
      Self::EbuStl => "stl",
    }
  }
}

// ===========================================================================
// CueData — sealed trait linking each `D` to its discriminant
// ===========================================================================

/// Sealed trait every per-format `D` payload implements; carries the
/// stable [`SubtitleCueKind`] discriminant alongside the format-specific
/// data. The sealed marker keeps the variant set closed to the crate so
/// adding a format is an additive change here, not a downstream impl.
pub trait CueData: private::Sealed {
  /// Stable discriminant for this payload.
  const KIND: SubtitleCueKind;
}

mod private {
  /// Sealed marker — only types in this module may impl [`CueData`].
  pub trait Sealed {}
}

// ===========================================================================
// SrtData — unit payload (SubRip line-only)
// ===========================================================================

/// Unit payload for SubRip cues — no per-format detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub struct SrtData;

impl SrtData {
  /// The single empty value.
  #[inline(always)]
  pub const fn new() -> Self {
    Self
  }
}

impl private::Sealed for SrtData {}
impl CueData for SrtData {
  const KIND: SubtitleCueKind = SubtitleCueKind::Srt;
}

// ===========================================================================
// VttData — WebVTT cue settings + region link + voice + styled markup
// ===========================================================================

/// WebVTT vertical direction (`vertical:lr` / `vertical:rl`). Absent
/// when the cue is horizontal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IsVariant, Display)]
#[display("{}", self.as_str())]
#[non_exhaustive]
#[repr(u8)]
pub enum VttVertical {
  /// `vertical:lr` (left-to-right column writing — Mongolian).
  Lr = 0,
  /// `vertical:rl` (right-to-left column writing — CJK).
  Rl = 1,
}

/// WebVTT line-align value (`line:<value>,<align>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IsVariant, Display)]
#[display("{}", self.as_str())]
#[non_exhaustive]
#[repr(u8)]
pub enum VttLineAlign {
  Start = 0,
  Center = 1,
  End = 2,
}

/// WebVTT position-align value (`position:<value>,<align>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IsVariant, Display)]
#[display("{}", self.as_str())]
#[non_exhaustive]
#[repr(u8)]
pub enum VttPositionAlign {
  Start = 0,
  Center = 1,
  End = 2,
  LineLeft = 3,
  LineRight = 4,
}

/// WebVTT text-align (`align:<value>` cue setting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IsVariant, Display)]
#[display("{}", self.as_str())]
#[non_exhaustive]
#[repr(u8)]
pub enum VttTextAlign {
  Start = 0,
  Center = 1,
  End = 2,
  Left = 3,
  Right = 4,
}

macro_rules! u8_codec {
  ($name:ident, $($variant:ident = $v:literal),+ $(,)?) => {
    impl $name {
      /// Stable numeric discriminant.
      #[inline(always)]
      pub const fn to_u8(self) -> u8 { self as u8 }
      /// Decode the numeric discriminant. `None` = unknown value.
      #[inline(always)]
      pub const fn try_from_u8(v: u8) -> Option<Self> {
        Some(match v {
          $($v => Self::$variant,)+
          _ => return None,
        })
      }
    }
  };
}

/// Same shape as [`u8_codec!`] for the string slug surface — generates
/// `as_str` (`const fn -> &'static str`) and `from_str` (exact-match
/// inverse, `None` for unknown). Pairs with `#[derive(Display)]` +
/// `#[display("{}", self.as_str())]` on each enum so Display routes
/// through `as_str` per rust-type-conventions §2.
macro_rules! slug_codec {
  ($name:ident, $($variant:ident = $slug:literal),+ $(,)?) => {
    impl $name {
      /// Canonical W3C-/spec-stable slug for this variant.
      #[inline(always)]
      pub const fn as_str(&self) -> &'static str {
        match self {
          $(Self::$variant => $slug,)+
        }
      }
      /// Inverse of [`as_str`](Self::as_str). `None` for any input
      /// that isn't an exact match of one of the slugs.
      #[inline]
      pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
          $($slug => Self::$variant,)+
          _ => return None,
        })
      }
    }
  };
}

u8_codec!(VttVertical, Lr = 0, Rl = 1);
u8_codec!(VttLineAlign, Start = 0, Center = 1, End = 2);
u8_codec!(
  VttPositionAlign,
  Start = 0,
  Center = 1,
  End = 2,
  LineLeft = 3,
  LineRight = 4
);
u8_codec!(
  VttTextAlign,
  Start = 0,
  Center = 1,
  End = 2,
  Left = 3,
  Right = 4
);

// W3C-canonical slugs for the VTT cue-setting tokens — these match the
// actual VTT syntax (`vertical:lr`, `vertical:rl`, `align:start`, …),
// including the hyphenated `line-left` / `line-right` for
// `VttPositionAlign` per the WebVTT spec.
slug_codec!(VttVertical, Lr = "lr", Rl = "rl");
slug_codec!(
  VttLineAlign,
  Start = "start",
  Center = "center",
  End = "end"
);
slug_codec!(
  VttPositionAlign,
  Start = "start",
  Center = "center",
  End = "end",
  LineLeft = "line-left",
  LineRight = "line-right"
);
slug_codec!(
  VttTextAlign,
  Start = "start",
  Center = "center",
  End = "end",
  Left = "left",
  Right = "right"
);

/// WebVTT per-cue payload — cue settings + voice + styled markup +
/// optional link to a region defined on the parent track.
///
/// `line_value` / `position_value` ride as raw `SmolStr` because the
/// WebVTT spec allows both percentages (`"50%"`) and line numbers
/// (`"-2"`). Empty `""` = absent (per the no-`Option`-for-string rule).
///
/// No `Eq`: `size_value: Option<f32>` is float-typed so structural
/// equality stops at `PartialEq`.
#[derive(Debug, Clone, PartialEq)]
pub struct VttData<Id = Uuid7> {
  cue_identifier: SmolStr,
  vertical: Option<VttVertical>,
  line_value: SmolStr,
  line_align: Option<VttLineAlign>,
  position_value: SmolStr,
  position_align: Option<VttPositionAlign>,
  size_value: Option<f32>,
  text_align: Option<VttTextAlign>,
  region_id: Option<Id>,
  voice: SmolStr,
  styled_text: SmolStr,
}

impl<Id> private::Sealed for VttData<Id> {}
impl<Id> CueData for VttData<Id> {
  const KIND: SubtitleCueKind = SubtitleCueKind::Vtt;
}

impl<Id> Default for VttData<Id> {
  fn default() -> Self {
    Self::new()
  }
}

impl<Id> VttData<Id> {
  /// All-empty / all-absent value.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      cue_identifier: SmolStr::new_inline(""),
      vertical: None,
      line_value: SmolStr::new_inline(""),
      line_align: None,
      position_value: SmolStr::new_inline(""),
      position_align: None,
      size_value: None,
      text_align: None,
      region_id: None,
      voice: SmolStr::new_inline(""),
      styled_text: SmolStr::new_inline(""),
    }
  }

  #[inline(always)]
  pub fn cue_identifier(&self) -> &str {
    self.cue_identifier.as_str()
  }
  #[inline(always)]
  pub const fn vertical(&self) -> Option<VttVertical> {
    self.vertical
  }
  #[inline(always)]
  pub fn line_value(&self) -> &str {
    self.line_value.as_str()
  }
  #[inline(always)]
  pub const fn line_align(&self) -> Option<VttLineAlign> {
    self.line_align
  }
  #[inline(always)]
  pub fn position_value(&self) -> &str {
    self.position_value.as_str()
  }
  #[inline(always)]
  pub const fn position_align(&self) -> Option<VttPositionAlign> {
    self.position_align
  }
  #[inline(always)]
  pub const fn size_value(&self) -> Option<f32> {
    self.size_value
  }
  #[inline(always)]
  pub const fn text_align(&self) -> Option<VttTextAlign> {
    self.text_align
  }
  #[inline(always)]
  pub const fn region_id_ref(&self) -> Option<&Id> {
    self.region_id.as_ref()
  }
  #[inline(always)]
  pub fn voice(&self) -> &str {
    self.voice.as_str()
  }
  #[inline(always)]
  pub fn styled_text(&self) -> &str {
    self.styled_text.as_str()
  }

  // Builders (consuming, #[must_use]).

  #[must_use]
  #[inline(always)]
  pub fn with_cue_identifier(mut self, v: impl Into<SmolStr>) -> Self {
    self.cue_identifier = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_vertical(mut self, v: VttVertical) -> Self {
    self.vertical = Some(v);
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn maybe_vertical(mut self, v: Option<VttVertical>) -> Self {
    self.vertical = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_line_value(mut self, v: impl Into<SmolStr>) -> Self {
    self.line_value = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_line_align(mut self, v: VttLineAlign) -> Self {
    self.line_align = Some(v);
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn maybe_line_align(mut self, v: Option<VttLineAlign>) -> Self {
    self.line_align = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_position_value(mut self, v: impl Into<SmolStr>) -> Self {
    self.position_value = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_position_align(mut self, v: VttPositionAlign) -> Self {
    self.position_align = Some(v);
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn maybe_position_align(mut self, v: Option<VttPositionAlign>) -> Self {
    self.position_align = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_size_value(mut self, v: f32) -> Self {
    self.size_value = Some(v);
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn maybe_size_value(mut self, v: Option<f32>) -> Self {
    self.size_value = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_text_align(mut self, v: VttTextAlign) -> Self {
    self.text_align = Some(v);
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn maybe_text_align(mut self, v: Option<VttTextAlign>) -> Self {
    self.text_align = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_region_id(mut self, v: Id) -> Self {
    self.region_id = Some(v);
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn maybe_region_id(mut self, v: Option<Id>) -> Self {
    self.region_id = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_voice(mut self, v: impl Into<SmolStr>) -> Self {
    self.voice = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_styled_text(mut self, v: impl Into<SmolStr>) -> Self {
    self.styled_text = v.into();
    self
  }
}

// ===========================================================================
// AssData — ASS/SSA Dialogue payload
// ===========================================================================

/// ASS/SSA Dialogue payload. `style_id` is a strict FK to a row of the
/// `subtitle_track_ass_style` aggregate; parsers resolve the Style name
/// → id at parse time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssData<Id = Uuid7> {
  layer: i32,
  style_id: Id,
  name: SmolStr,
  margin_l: i32,
  margin_r: i32,
  margin_v: i32,
  effect: SmolStr,
  styled_text: SmolStr,
}

impl<Id> private::Sealed for AssData<Id> {}
impl<Id> CueData for AssData<Id> {
  const KIND: SubtitleCueKind = SubtitleCueKind::Ass;
}

impl<Id> AssData<Id> {
  /// Construct an `AssData` payload. `style_id` is mandatory — every ASS
  /// `Dialogue` line references exactly one Style. Validation of the FK
  /// is the application's responsibility (see [[validation-responsibility-boundary]]).
  #[inline]
  pub fn new(style_id: Id) -> Self {
    Self {
      layer: 0,
      style_id,
      name: SmolStr::new_inline(""),
      margin_l: 0,
      margin_r: 0,
      margin_v: 0,
      effect: SmolStr::new_inline(""),
      styled_text: SmolStr::new_inline(""),
    }
  }

  #[inline(always)]
  pub const fn layer(&self) -> i32 {
    self.layer
  }
  #[inline(always)]
  pub const fn style_id_ref(&self) -> &Id {
    &self.style_id
  }
  #[inline(always)]
  pub fn name(&self) -> &str {
    self.name.as_str()
  }
  #[inline(always)]
  pub const fn margin_l(&self) -> i32 {
    self.margin_l
  }
  #[inline(always)]
  pub const fn margin_r(&self) -> i32 {
    self.margin_r
  }
  #[inline(always)]
  pub const fn margin_v(&self) -> i32 {
    self.margin_v
  }
  #[inline(always)]
  pub fn effect(&self) -> &str {
    self.effect.as_str()
  }
  #[inline(always)]
  pub fn styled_text(&self) -> &str {
    self.styled_text.as_str()
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_layer(mut self, v: i32) -> Self {
    self.layer = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_style_id(mut self, v: Id) -> Self {
    self.style_id = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_name(mut self, v: impl Into<SmolStr>) -> Self {
    self.name = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_margin_l(mut self, v: i32) -> Self {
    self.margin_l = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_margin_r(mut self, v: i32) -> Self {
    self.margin_r = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_margin_v(mut self, v: i32) -> Self {
    self.margin_v = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_effect(mut self, v: impl Into<SmolStr>) -> Self {
    self.effect = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_styled_text(mut self, v: impl Into<SmolStr>) -> Self {
    self.styled_text = v.into();
    self
  }
}

// ===========================================================================
// LrcData — LRC line + Enhanced word-level flag
// ===========================================================================

/// LRC per-cue payload. `has_word_timing = true` means companion rows
/// exist in `subtitle_cue_lrc_word`; `false` = line-level only.
///
/// The actual word rows live as a separate child aggregate (see
/// [`LrcWord`]); they are not embedded in this payload because a cue
/// may have hundreds of words and SQL projections want them in a
/// dedicated table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub struct LrcData {
  has_word_timing: bool,
}

impl private::Sealed for LrcData {}
impl CueData for LrcData {
  const KIND: SubtitleCueKind = SubtitleCueKind::Lrc;
}

impl LrcData {
  /// Line-only LRC cue (`has_word_timing = false`).
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      has_word_timing: false,
    }
  }

  #[inline(always)]
  pub const fn has_word_timing(&self) -> bool {
    self.has_word_timing
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_word_timing(mut self) -> Self {
    self.has_word_timing = true;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn maybe_word_timing(mut self, v: bool) -> Self {
    self.has_word_timing = v;
    self
  }
  #[inline(always)]
  pub const fn set_word_timing(&mut self) -> &mut Self {
    self.has_word_timing = true;
    self
  }
  #[inline(always)]
  pub const fn update_word_timing(&mut self, v: bool) -> &mut Self {
    self.has_word_timing = v;
    self
  }
  #[inline(always)]
  pub const fn clear_word_timing(&mut self) -> &mut Self {
    self.has_word_timing = false;
    self
  }
}

// ===========================================================================
// MicroDvdData — inline `{y:i}` style codes
// ===========================================================================

/// MicroDVD per-cue payload. `styled_text` carries the inline frame-
/// based MicroDVD codes (`{y:i}`, `{c:$00FF00}`, …) verbatim; the
/// plain text rides on the base `SubtitleCue.text`.
///
/// MicroDVD has no per-track aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub struct MicroDvdData {
  styled_text: SmolStr,
}

impl private::Sealed for MicroDvdData {}
impl CueData for MicroDvdData {
  const KIND: SubtitleCueKind = SubtitleCueKind::MicroDvd;
}

impl MicroDvdData {
  /// Construct a MicroDVD payload with the given styled-text body
  /// (may be empty).
  #[inline]
  pub fn new(styled_text: impl Into<SmolStr>) -> Self {
    Self {
      styled_text: styled_text.into(),
    }
  }

  #[inline(always)]
  pub fn styled_text(&self) -> &str {
    self.styled_text.as_str()
  }

  #[must_use]
  #[inline(always)]
  pub fn with_styled_text(mut self, v: impl Into<SmolStr>) -> Self {
    self.styled_text = v.into();
    self
  }
}

// ===========================================================================
// SubViewerData — `[br]`/`[b]`/`[i]`/`[u]` inline tags
// ===========================================================================

/// SubViewer per-cue payload. `styled_text` carries the SubViewer
/// inline-tag body (`[br]`, `[b]`, `[i]`, `[u]`, `{y:i}` colour codes)
/// verbatim; plain text rides on the base `SubtitleCue.text`.
///
/// SubViewer has no per-track aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub struct SubViewerData {
  styled_text: SmolStr,
}

impl private::Sealed for SubViewerData {}
impl CueData for SubViewerData {
  const KIND: SubtitleCueKind = SubtitleCueKind::SubViewer;
}

impl SubViewerData {
  #[inline]
  pub fn new(styled_text: impl Into<SmolStr>) -> Self {
    Self {
      styled_text: styled_text.into(),
    }
  }

  #[inline(always)]
  pub fn styled_text(&self) -> &str {
    self.styled_text.as_str()
  }

  #[must_use]
  #[inline(always)]
  pub fn with_styled_text(mut self, v: impl Into<SmolStr>) -> Self {
    self.styled_text = v.into();
    self
  }
}

// ===========================================================================
// SbvData — unit marker (YouTube Sbv has no per-format detail)
// ===========================================================================

/// YouTube SBV unit marker payload. Plain text rides on the base
/// `SubtitleCue.text`. Stored as its own row even though empty so the
/// dispatch surface is uniform across formats (per user spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub struct SbvData;

impl SbvData {
  /// The single empty value.
  #[inline(always)]
  pub const fn new() -> Self {
    Self
  }
}

impl private::Sealed for SbvData {}
impl CueData for SbvData {
  const KIND: SubtitleCueKind = SubtitleCueKind::Sbv;
}

// ===========================================================================
// TtmlData — XML-fragment styled text + optional region / style refs
// ===========================================================================

/// TTML (Timed Text Markup Language) per-cue payload. `styled_text`
/// carries the inline XML fragment (`<span>` runs, `<br/>`, …) verbatim;
/// plain text rides on the base `SubtitleCue.text`.
///
/// `region_id` and `style_id` are optional FKs into per-track
/// [`TtmlRegion`] / [`TtmlStyle`] aggregates. `xml_id` is the cue's
/// `xml:id` attribute (the parser's stable cue-identifier handle); `""`
/// = absent (no `Option` per the empty-string-is-absent rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtmlData<Id = Uuid7> {
  region_id: Option<Id>,
  style_id: Option<Id>,
  xml_id: SmolStr,
  styled_text: SmolStr,
}

impl<Id> private::Sealed for TtmlData<Id> {}
impl<Id> CueData for TtmlData<Id> {
  const KIND: SubtitleCueKind = SubtitleCueKind::Ttml;
}

impl<Id> Default for TtmlData<Id> {
  fn default() -> Self {
    Self::new()
  }
}

impl<Id> TtmlData<Id> {
  /// All-empty / all-absent value.
  #[inline]
  pub fn new() -> Self {
    Self {
      region_id: None,
      style_id: None,
      xml_id: SmolStr::new_inline(""),
      styled_text: SmolStr::new_inline(""),
    }
  }

  #[inline(always)]
  pub const fn region_id_ref(&self) -> Option<&Id> {
    self.region_id.as_ref()
  }

  #[inline(always)]
  pub const fn style_id_ref(&self) -> Option<&Id> {
    self.style_id.as_ref()
  }

  #[inline(always)]
  pub fn xml_id(&self) -> &str {
    self.xml_id.as_str()
  }

  #[inline(always)]
  pub fn styled_text(&self) -> &str {
    self.styled_text.as_str()
  }

  #[must_use]
  #[inline]
  pub fn with_region_id(mut self, v: Id) -> Self {
    self.region_id = Some(v);
    self
  }

  #[must_use]
  #[inline]
  pub fn maybe_region_id(mut self, v: Option<Id>) -> Self {
    self.region_id = v;
    self
  }

  #[must_use]
  #[inline]
  pub fn with_style_id(mut self, v: Id) -> Self {
    self.style_id = Some(v);
    self
  }

  #[must_use]
  #[inline]
  pub fn maybe_style_id(mut self, v: Option<Id>) -> Self {
    self.style_id = v;
    self
  }

  #[must_use]
  #[inline]
  pub fn with_xml_id(mut self, v: impl Into<SmolStr>) -> Self {
    self.xml_id = v.into();
    self
  }

  #[must_use]
  #[inline]
  pub fn with_styled_text(mut self, v: impl Into<SmolStr>) -> Self {
    self.styled_text = v.into();
    self
  }
}

// ===========================================================================
// SamiData — HTML-like `<SYNC>`-block payload with per-class styling
// ===========================================================================

/// SAMI (Synchronized Accessible Media Interchange) per-cue payload.
/// `class_name` selects which per-track [`SamiStyle`] applies (e.g.
/// `ENCC` for English captions); `styled_text` carries the inline
/// HTML-like body of the `<SYNC>` block. Plain text rides on the base
/// `SubtitleCue.text`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub struct SamiData {
  class_name: SmolStr,
  styled_text: SmolStr,
}

impl private::Sealed for SamiData {}
impl CueData for SamiData {
  const KIND: SubtitleCueKind = SubtitleCueKind::Sami;
}

impl SamiData {
  #[inline]
  pub fn new() -> Self {
    Self::default()
  }

  #[inline(always)]
  pub fn class_name(&self) -> &str {
    self.class_name.as_str()
  }

  #[inline(always)]
  pub fn styled_text(&self) -> &str {
    self.styled_text.as_str()
  }

  #[must_use]
  #[inline]
  pub fn with_class_name(mut self, v: impl Into<SmolStr>) -> Self {
    self.class_name = v.into();
    self
  }

  #[must_use]
  #[inline]
  pub fn with_styled_text(mut self, v: impl Into<SmolStr>) -> Self {
    self.styled_text = v.into();
    self
  }
}

// ===========================================================================
// VobSubData — DVD VobSub bitmap payload + per-cue palette/contrast indices
// ===========================================================================

/// DVD VobSub per-cue payload. Carries the rendered bitmap blob plus
/// per-cue rendering metadata (`width`/`height`/origin offset, the
/// 4-entry colour/contrast indices into the per-track palette). The
/// bitmap is run-length-encoded VobSub payload (`Bytes` to keep the
/// raw blob exact; cheap-clone refcount).
///
/// `palette_id` FKs into the per-track [`VobSubPalette`] aggregate
/// (one palette is shared by many cues on the track). The base
/// `SubtitleCue.text` stays `""` until an OCR pipeline writes plain
/// text into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VobSubData<Id = Uuid7> {
  palette_id: Id,
  bitmap: Bytes,
  width: u32,
  height: u32,
  pos_x: i32,
  pos_y: i32,
  color_indices: [u8; 4],
  contrast_indices: [u8; 4],
}

impl<Id> private::Sealed for VobSubData<Id> {}
impl<Id> CueData for VobSubData<Id> {
  const KIND: SubtitleCueKind = SubtitleCueKind::VobSub;
}

impl<Id> VobSubData<Id> {
  /// All-empty payload with the given (required) palette FK.
  ///
  /// `width`/`height`/`pos_*`/colour-indices default to `0`; the
  /// bitmap blob defaults to empty. Use the builders to fill in.
  #[inline]
  pub fn new(palette_id: Id) -> Self {
    Self {
      palette_id,
      bitmap: Bytes::new(),
      width: 0,
      height: 0,
      pos_x: 0,
      pos_y: 0,
      color_indices: [0; 4],
      contrast_indices: [0; 4],
    }
  }

  #[inline(always)]
  pub const fn palette_id_ref(&self) -> &Id {
    &self.palette_id
  }

  #[inline(always)]
  pub const fn bitmap_ref(&self) -> &Bytes {
    &self.bitmap
  }

  #[inline(always)]
  pub const fn width(&self) -> u32 {
    self.width
  }

  #[inline(always)]
  pub const fn height(&self) -> u32 {
    self.height
  }

  #[inline(always)]
  pub const fn pos_x(&self) -> i32 {
    self.pos_x
  }

  #[inline(always)]
  pub const fn pos_y(&self) -> i32 {
    self.pos_y
  }

  #[inline(always)]
  pub const fn color_indices(&self) -> &[u8; 4] {
    &self.color_indices
  }

  #[inline(always)]
  pub const fn contrast_indices(&self) -> &[u8; 4] {
    &self.contrast_indices
  }

  #[must_use]
  #[inline]
  pub fn with_bitmap(mut self, v: impl Into<Bytes>) -> Self {
    self.bitmap = v.into();
    self
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_width(mut self, v: u32) -> Self {
    self.width = v;
    self
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_height(mut self, v: u32) -> Self {
    self.height = v;
    self
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_pos(mut self, x: i32, y: i32) -> Self {
    self.pos_x = x;
    self.pos_y = y;
    self
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_color_indices(mut self, v: [u8; 4]) -> Self {
    self.color_indices = v;
    self
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_contrast_indices(mut self, v: [u8; 4]) -> Self {
    self.contrast_indices = v;
    self
  }
}

// ===========================================================================
// PgsData — Blu-ray PGS bitmap payload + per-cue palette bytes
// ===========================================================================

/// Blu-ray PGS per-cue payload. Unlike VobSub, PGS embeds its palette
/// per-cue (no per-track aggregate). Carries the raw bitmap blob, the
/// per-cue palette bytes (`Y/Cr/Cb/A` quadruples — opaque to the schema
/// layer), the rendered dimensions / origin offset, and the PGS
/// `composition_state` byte (`0x00`/`0x40`/`0x80`).
///
/// The base `SubtitleCue.text` stays `""` until an OCR pipeline writes
/// plain text into it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PgsData {
  bitmap: Bytes,
  width: u32,
  height: u32,
  pos_x: i32,
  pos_y: i32,
  palette_bytes: Bytes,
  composition_state: u8,
}

impl private::Sealed for PgsData {}
impl CueData for PgsData {
  const KIND: SubtitleCueKind = SubtitleCueKind::Pgs;
}

impl PgsData {
  /// All-empty payload (zero geometry, empty bitmap + palette).
  #[inline]
  pub fn new() -> Self {
    Self::default()
  }

  #[inline(always)]
  pub const fn bitmap_ref(&self) -> &Bytes {
    &self.bitmap
  }

  #[inline(always)]
  pub const fn palette_bytes_ref(&self) -> &Bytes {
    &self.palette_bytes
  }

  #[inline(always)]
  pub const fn width(&self) -> u32 {
    self.width
  }

  #[inline(always)]
  pub const fn height(&self) -> u32 {
    self.height
  }

  #[inline(always)]
  pub const fn pos_x(&self) -> i32 {
    self.pos_x
  }

  #[inline(always)]
  pub const fn pos_y(&self) -> i32 {
    self.pos_y
  }

  #[inline(always)]
  pub const fn composition_state(&self) -> u8 {
    self.composition_state
  }

  #[must_use]
  #[inline]
  pub fn with_bitmap(mut self, v: impl Into<Bytes>) -> Self {
    self.bitmap = v.into();
    self
  }

  #[must_use]
  #[inline]
  pub fn with_palette_bytes(mut self, v: impl Into<Bytes>) -> Self {
    self.palette_bytes = v.into();
    self
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_width(mut self, v: u32) -> Self {
    self.width = v;
    self
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_height(mut self, v: u32) -> Self {
    self.height = v;
    self
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_pos(mut self, x: i32, y: i32) -> Self {
    self.pos_x = x;
    self.pos_y = y;
    self
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_composition_state(mut self, v: u8) -> Self {
    self.composition_state = v;
    self
  }
}

// ===========================================================================
// Cea608Data — CEA-608 line-21 caption payload
// ===========================================================================

/// CEA-608 per-cue payload. `channel` selects the captioning channel
/// (`1..=4` = CC1, CC2, CC3, CC4); `pac_byte_pair` holds the raw
/// Preamble Address Code byte pair (row/colour/underline encoded);
/// `styled_text` carries the decoded line text with any inline
/// CEA-608 style codes preserved.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cea608Data {
  channel: u8,
  pac_byte_pair: u32,
  styled_text: SmolStr,
}

impl private::Sealed for Cea608Data {}
impl CueData for Cea608Data {
  const KIND: SubtitleCueKind = SubtitleCueKind::Cea608;
}

impl Cea608Data {
  /// Validating constructor. Rejects `channel` outside `1..=4`.
  pub fn try_new(channel: u8) -> Result<Self, SubtitleCueError> {
    if !matches!(channel, 1..=4) {
      return Err(SubtitleCueError::Cea608ChannelOutOfRange(channel));
    }
    Ok(Self {
      channel,
      pac_byte_pair: 0,
      styled_text: SmolStr::new_inline(""),
    })
  }

  #[inline(always)]
  pub const fn channel(&self) -> u8 {
    self.channel
  }

  #[inline(always)]
  pub const fn pac_byte_pair(&self) -> u32 {
    self.pac_byte_pair
  }

  #[inline(always)]
  pub fn styled_text(&self) -> &str {
    self.styled_text.as_str()
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_pac_byte_pair(mut self, v: u32) -> Self {
    self.pac_byte_pair = v;
    self
  }

  #[must_use]
  #[inline]
  pub fn with_styled_text(mut self, v: impl Into<SmolStr>) -> Self {
    self.styled_text = v.into();
    self
  }
}

// ===========================================================================
// EbuStlData — EBU STL teletext payload
// ===========================================================================

/// EBU STL per-cue payload. Mirrors a TTI (Text and Timing
/// Information) block: `subtitle_number` is the SN field, `cumulative`
/// is the Cumulative-Subtitle flag (multi-row stacked subtitles),
/// `vertical_pos` is the VP field (line on screen), `justification`
/// is the JC field (`1` = left, `2` = centre, `3` = right —
/// validated), and `styled_text` carries the decoded line with any
/// inline STL control codes preserved.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EbuStlData {
  subtitle_number: u32,
  cumulative: bool,
  vertical_pos: i32,
  justification: u8,
  styled_text: SmolStr,
}

impl private::Sealed for EbuStlData {}
impl CueData for EbuStlData {
  const KIND: SubtitleCueKind = SubtitleCueKind::EbuStl;
}

impl EbuStlData {
  /// Validating constructor. Rejects `justification` outside `1..=3`.
  pub fn try_new(justification: u8) -> Result<Self, SubtitleCueError> {
    if !matches!(justification, 1..=3) {
      return Err(SubtitleCueError::EbuStlJustificationOutOfRange(
        justification,
      ));
    }
    Ok(Self {
      subtitle_number: 0,
      cumulative: false,
      vertical_pos: 0,
      justification,
      styled_text: SmolStr::new_inline(""),
    })
  }

  #[inline(always)]
  pub const fn subtitle_number(&self) -> u32 {
    self.subtitle_number
  }

  #[inline(always)]
  pub const fn cumulative(&self) -> bool {
    self.cumulative
  }

  #[inline(always)]
  pub const fn vertical_pos(&self) -> i32 {
    self.vertical_pos
  }

  #[inline(always)]
  pub const fn justification(&self) -> u8 {
    self.justification
  }

  #[inline(always)]
  pub fn styled_text(&self) -> &str {
    self.styled_text.as_str()
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_subtitle_number(mut self, v: u32) -> Self {
    self.subtitle_number = v;
    self
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_cumulative(mut self) -> Self {
    self.cumulative = true;
    self
  }

  #[must_use]
  #[inline(always)]
  pub const fn maybe_cumulative(mut self, v: bool) -> Self {
    self.cumulative = v;
    self
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_vertical_pos(mut self, v: i32) -> Self {
    self.vertical_pos = v;
    self
  }

  #[must_use]
  #[inline]
  pub fn with_styled_text(mut self, v: impl Into<SmolStr>) -> Self {
    self.styled_text = v.into();
    self
  }
}

// ===========================================================================
// SubtitleCue<Id, D> — polymorphic base
// ===========================================================================

/// One parsed subtitle cue, parameterised by per-format payload `D`.
///
/// Construct via [`SubtitleCue::try_new`]; for the line-only SRT case
/// see the [`SrtCue::try_new_srt`] convenience constructor through the
/// type alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleCue<Id, D> {
  id: Id,
  subtitle_track_id: Id,
  ordinal: u32,
  span: TimeRange,
  text: LocalizedText,
  data: D,
}

/// SubRip cue (no per-format detail).
pub type SrtCue<Id = Uuid7> = SubtitleCue<Id, SrtData>;
/// WebVTT cue.
pub type VttCue<Id = Uuid7> = SubtitleCue<Id, VttData<Id>>;
/// ASS/SSA Dialogue cue.
pub type AssCue<Id = Uuid7> = SubtitleCue<Id, AssData<Id>>;
/// LRC cue (line- or word-level).
pub type LrcCue<Id = Uuid7> = SubtitleCue<Id, LrcData>;
/// MicroDVD cue (frame-based inline `{y:i}` codes).
pub type MicroDvdCue<Id = Uuid7> = SubtitleCue<Id, MicroDvdData>;
/// SubViewer cue (inline `[br]`/`[b]`/`[i]`/`[u]` tags).
pub type SubViewerCue<Id = Uuid7> = SubtitleCue<Id, SubViewerData>;
/// YouTube SBV cue (unit marker, plain-text only).
pub type SbvCue<Id = Uuid7> = SubtitleCue<Id, SbvData>;
/// TTML cue.
pub type TtmlCue<Id = Uuid7> = SubtitleCue<Id, TtmlData<Id>>;
/// SAMI cue.
pub type SamiCue<Id = Uuid7> = SubtitleCue<Id, SamiData>;
/// DVD VobSub bitmap cue.
pub type VobSubCue<Id = Uuid7> = SubtitleCue<Id, VobSubData<Id>>;
/// Blu-ray PGS bitmap cue.
pub type PgsCue<Id = Uuid7> = SubtitleCue<Id, PgsData>;
/// CEA-608 caption cue.
pub type Cea608Cue<Id = Uuid7> = SubtitleCue<Id, Cea608Data>;
/// EBU STL teletext cue.
pub type EbuStlCue<Id = Uuid7> = SubtitleCue<Id, EbuStlData>;

// ===========================================================================
// SubtitleCueDetails<Id> — runtime-tagged union of every implemented payload
// ===========================================================================

/// Runtime-tagged union over every implemented per-format payload.
///
/// `SubtitleCue<Id, D>` is open over `D` so each format keeps its own
/// strongly-typed payload at the type level. Wire / sqlx / mongodb
/// decoders that don't know the format at compile time decode into
/// `SubtitleCue<Id, SubtitleCueDetails<Id>>`; callers `match` on the
/// variant to recover the typed payload.
///
/// Implements [`CueData`] but the [`CueData::KIND`] constant is
/// [`SubtitleCueKind::Srt`] — the constant is a per-type contract and
/// is not consulted for polymorphic cues. Use [`Self::kind`] (or
/// [`SubtitleCue::data_ref`] + [`Self::kind`]) at runtime instead.
#[derive(Debug, Clone, PartialEq, IsVariant)]
#[non_exhaustive]
pub enum SubtitleCueDetails<Id = Uuid7> {
  /// SubRip payload (unit marker).
  Srt(SrtData),
  /// WebVTT payload.
  Vtt(VttData<Id>),
  /// ASS/SSA Dialogue payload.
  Ass(AssData<Id>),
  /// LRC payload (line-level + word-timing flag).
  Lrc(LrcData),
  /// MicroDVD payload.
  MicroDvd(MicroDvdData),
  /// SubViewer payload.
  SubViewer(SubViewerData),
  /// YouTube SBV payload (unit marker).
  Sbv(SbvData),
  /// TTML payload.
  Ttml(TtmlData<Id>),
  /// SAMI payload.
  Sami(SamiData),
  /// DVD VobSub bitmap payload.
  VobSub(VobSubData<Id>),
  /// Blu-ray PGS bitmap payload.
  Pgs(PgsData),
  /// CEA-608 caption payload.
  Cea608(Cea608Data),
  /// EBU STL teletext payload.
  EbuStl(EbuStlData),
}

impl<Id> private::Sealed for SubtitleCueDetails<Id> {}
impl<Id> CueData for SubtitleCueDetails<Id> {
  // Placeholder — see the type-level doc; callers route through
  // `SubtitleCueDetails::kind` at runtime.
  const KIND: SubtitleCueKind = SubtitleCueKind::Srt;
}

impl<Id> SubtitleCueDetails<Id> {
  /// Runtime discriminant for this payload — the one that should be
  /// observed; the trait-level [`CueData::KIND`] is a placeholder for
  /// the polymorphic case (see the type-level doc).
  #[inline(always)]
  pub const fn kind(&self) -> SubtitleCueKind {
    match self {
      Self::Srt(_) => SubtitleCueKind::Srt,
      Self::Vtt(_) => SubtitleCueKind::Vtt,
      Self::Ass(_) => SubtitleCueKind::Ass,
      Self::Lrc(_) => SubtitleCueKind::Lrc,
      Self::MicroDvd(_) => SubtitleCueKind::MicroDvd,
      Self::SubViewer(_) => SubtitleCueKind::SubViewer,
      Self::Sbv(_) => SubtitleCueKind::Sbv,
      Self::Ttml(_) => SubtitleCueKind::Ttml,
      Self::Sami(_) => SubtitleCueKind::Sami,
      Self::VobSub(_) => SubtitleCueKind::VobSub,
      Self::Pgs(_) => SubtitleCueKind::Pgs,
      Self::Cea608(_) => SubtitleCueKind::Cea608,
      Self::EbuStl(_) => SubtitleCueKind::EbuStl,
    }
  }
}

impl<Id> From<MicroDvdData> for SubtitleCueDetails<Id> {
  #[inline]
  fn from(v: MicroDvdData) -> Self {
    Self::MicroDvd(v)
  }
}

impl<Id> From<SubViewerData> for SubtitleCueDetails<Id> {
  #[inline]
  fn from(v: SubViewerData) -> Self {
    Self::SubViewer(v)
  }
}

impl<Id> From<SbvData> for SubtitleCueDetails<Id> {
  #[inline]
  fn from(v: SbvData) -> Self {
    Self::Sbv(v)
  }
}

impl<Id> From<TtmlData<Id>> for SubtitleCueDetails<Id> {
  #[inline]
  fn from(v: TtmlData<Id>) -> Self {
    Self::Ttml(v)
  }
}

impl<Id> From<SamiData> for SubtitleCueDetails<Id> {
  #[inline]
  fn from(v: SamiData) -> Self {
    Self::Sami(v)
  }
}

impl<Id> From<SrtData> for SubtitleCueDetails<Id> {
  #[inline]
  fn from(v: SrtData) -> Self {
    Self::Srt(v)
  }
}

impl<Id> From<VttData<Id>> for SubtitleCueDetails<Id> {
  #[inline]
  fn from(v: VttData<Id>) -> Self {
    Self::Vtt(v)
  }
}

impl<Id> From<AssData<Id>> for SubtitleCueDetails<Id> {
  #[inline]
  fn from(v: AssData<Id>) -> Self {
    Self::Ass(v)
  }
}

impl<Id> From<LrcData> for SubtitleCueDetails<Id> {
  #[inline]
  fn from(v: LrcData) -> Self {
    Self::Lrc(v)
  }
}

impl<Id> From<VobSubData<Id>> for SubtitleCueDetails<Id> {
  #[inline]
  fn from(v: VobSubData<Id>) -> Self {
    Self::VobSub(v)
  }
}

impl<Id> From<PgsData> for SubtitleCueDetails<Id> {
  #[inline]
  fn from(v: PgsData) -> Self {
    Self::Pgs(v)
  }
}

impl<Id> From<Cea608Data> for SubtitleCueDetails<Id> {
  #[inline]
  fn from(v: Cea608Data) -> Self {
    Self::Cea608(v)
  }
}

impl<Id> From<EbuStlData> for SubtitleCueDetails<Id> {
  #[inline]
  fn from(v: EbuStlData) -> Self {
    Self::EbuStl(v)
  }
}

impl<D> SubtitleCue<Uuid7, D> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` and nil `subtitle_track_id`. `text` may be empty
  /// (a bitmap cue pre-OCR, or an ASS cue whose visible text rides in
  /// `data.styled_text()`).
  ///
  /// `mediatime::TimeRange::new` enforces `start <= end` by construction.
  pub fn try_new(
    id: Uuid7,
    subtitle_track_id: Uuid7,
    ordinal: u32,
    span: TimeRange,
    text: LocalizedText,
    data: D,
  ) -> Result<Self, SubtitleCueError> {
    if id.is_nil() {
      return Err(SubtitleCueError::NilId);
    }
    if subtitle_track_id.is_nil() {
      return Err(SubtitleCueError::NilSubtitleTrackId);
    }
    Ok(Self {
      id,
      subtitle_track_id,
      ordinal,
      span,
      text,
      data,
    })
  }
}

impl<Id, D> SubtitleCue<Id, D> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }
  #[inline(always)]
  pub const fn subtitle_track_id_ref(&self) -> &Id {
    &self.subtitle_track_id
  }
  #[inline(always)]
  pub const fn ordinal(&self) -> u32 {
    self.ordinal
  }
  #[inline(always)]
  pub const fn span_ref(&self) -> &TimeRange {
    &self.span
  }
  #[inline(always)]
  pub const fn text_ref(&self) -> &LocalizedText {
    &self.text
  }
  #[inline(always)]
  pub const fn data_ref(&self) -> &D {
    &self.data
  }
  #[inline(always)]
  pub const fn data_mut(&mut self) -> &mut D {
    &mut self.data
  }

  /// The stable kind discriminant for this cue's payload type.
  #[inline(always)]
  pub fn kind(&self) -> SubtitleCueKind
  where
    D: CueData,
  {
    D::KIND
  }

  // -------------------------------------------------------------------
  // Builders / setters — invariant-free fields.
  // -------------------------------------------------------------------

  #[must_use]
  #[inline(always)]
  pub const fn with_ordinal(mut self, v: u32) -> Self {
    self.ordinal = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_span(mut self, v: TimeRange) -> Self {
    self.span = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_text(mut self, v: LocalizedText) -> Self {
    self.text = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_data(mut self, v: D) -> Self {
    self.data = v;
    self
  }

  #[inline(always)]
  pub const fn set_ordinal(&mut self, v: u32) -> &mut Self {
    self.ordinal = v;
    self
  }
  #[inline(always)]
  pub const fn set_span(&mut self, v: TimeRange) -> &mut Self {
    self.span = v;
    self
  }
  #[inline(always)]
  pub fn set_text(&mut self, v: LocalizedText) -> &mut Self {
    self.text = v;
    self
  }
  #[inline(always)]
  pub fn set_data(&mut self, v: D) -> &mut Self {
    self.data = v;
    self
  }
}

impl SrtCue<Uuid7> {
  /// Convenience constructor for an SRT cue (no per-format detail).
  #[inline]
  pub fn try_new_srt(
    id: Uuid7,
    subtitle_track_id: Uuid7,
    ordinal: u32,
    span: TimeRange,
    text: LocalizedText,
  ) -> Result<Self, SubtitleCueError> {
    Self::try_new(id, subtitle_track_id, ordinal, span, text, SrtData)
  }
}

// ===========================================================================
// LrcWord — child of an LRC cue (word-level timing)
// ===========================================================================

/// One word of an LRC (Enhanced) cue. `start_pts` is a media-time PTS
/// tick in the parent track's timebase; the per-word end is the next
/// word's start (or the cue's `span.end`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LrcWord<Id = Uuid7> {
  subtitle_cue_id: Id,
  ordinal: u32,
  text: SmolStr,
  start_pts: i64,
}

impl LrcWord<Uuid7> {
  /// Validating constructor.
  ///
  /// Rejects nil `subtitle_cue_id`.
  pub fn try_new(
    subtitle_cue_id: Uuid7,
    ordinal: u32,
    text: impl Into<SmolStr>,
    start_pts: i64,
  ) -> Result<Self, SubtitleCueError> {
    if subtitle_cue_id.is_nil() {
      return Err(SubtitleCueError::NilSubtitleCueId);
    }
    Ok(Self {
      subtitle_cue_id,
      ordinal,
      text: text.into(),
      start_pts,
    })
  }
}

impl<Id> LrcWord<Id> {
  #[inline(always)]
  pub const fn subtitle_cue_id_ref(&self) -> &Id {
    &self.subtitle_cue_id
  }
  #[inline(always)]
  pub const fn ordinal(&self) -> u32 {
    self.ordinal
  }
  #[inline(always)]
  pub fn text(&self) -> &str {
    self.text.as_str()
  }
  #[inline(always)]
  pub const fn start_pts(&self) -> i64 {
    self.start_pts
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_ordinal(mut self, v: u32) -> Self {
    self.ordinal = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_text(mut self, v: impl Into<SmolStr>) -> Self {
    self.text = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_start_pts(mut self, v: i64) -> Self {
    self.start_pts = v;
    self
  }
}

// ===========================================================================
// VttRegion — per-track WebVTT region aggregate
// ===========================================================================

/// WebVTT region — a named viewport area cues can be assigned to via
/// `VttData::region_id`.
#[derive(Debug, Clone, PartialEq)]
pub struct VttRegion<Id = Uuid7> {
  id: Id,
  subtitle_track_id: Id,
  name: SmolStr,
  width: f32,
  lines: u32,
  region_anchor_x: f32,
  region_anchor_y: f32,
  viewport_anchor_x: f32,
  viewport_anchor_y: f32,
  scroll_up: bool,
}

impl VttRegion<Uuid7> {
  /// Validating constructor.
  ///
  /// Rejects nil `id` and nil `subtitle_track_id`.
  pub fn try_new(
    id: Uuid7,
    subtitle_track_id: Uuid7,
    name: impl Into<SmolStr>,
  ) -> Result<Self, SubtitleCueError> {
    if id.is_nil() {
      return Err(SubtitleCueError::NilId);
    }
    if subtitle_track_id.is_nil() {
      return Err(SubtitleCueError::NilSubtitleTrackId);
    }
    Ok(Self {
      id,
      subtitle_track_id,
      name: name.into(),
      width: 100.0,
      lines: 3,
      region_anchor_x: 0.0,
      region_anchor_y: 100.0,
      viewport_anchor_x: 0.0,
      viewport_anchor_y: 100.0,
      scroll_up: false,
    })
  }
}

impl<Id> VttRegion<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }
  #[inline(always)]
  pub const fn subtitle_track_id_ref(&self) -> &Id {
    &self.subtitle_track_id
  }
  #[inline(always)]
  pub fn name(&self) -> &str {
    self.name.as_str()
  }
  #[inline(always)]
  pub const fn width(&self) -> f32 {
    self.width
  }
  #[inline(always)]
  pub const fn lines(&self) -> u32 {
    self.lines
  }
  #[inline(always)]
  pub const fn region_anchor_x(&self) -> f32 {
    self.region_anchor_x
  }
  #[inline(always)]
  pub const fn region_anchor_y(&self) -> f32 {
    self.region_anchor_y
  }
  #[inline(always)]
  pub const fn viewport_anchor_x(&self) -> f32 {
    self.viewport_anchor_x
  }
  #[inline(always)]
  pub const fn viewport_anchor_y(&self) -> f32 {
    self.viewport_anchor_y
  }
  #[inline(always)]
  pub const fn scroll_up(&self) -> bool {
    self.scroll_up
  }

  #[must_use]
  #[inline(always)]
  pub fn with_name(mut self, v: impl Into<SmolStr>) -> Self {
    self.name = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_width(mut self, v: f32) -> Self {
    self.width = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_lines(mut self, v: u32) -> Self {
    self.lines = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_region_anchor(mut self, x: f32, y: f32) -> Self {
    self.region_anchor_x = x;
    self.region_anchor_y = y;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_viewport_anchor(mut self, x: f32, y: f32) -> Self {
    self.viewport_anchor_x = x;
    self.viewport_anchor_y = y;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_scroll_up(mut self) -> Self {
    self.scroll_up = true;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn maybe_scroll_up(mut self, v: bool) -> Self {
    self.scroll_up = v;
    self
  }
}

// ===========================================================================
// VttStyleBlock — per-track WebVTT STYLE block
// ===========================================================================

/// One WebVTT `STYLE` block of a track. Multiple style blocks per track
/// are allowed and rendered in `ordinal` order; the body is opaque CSS
/// text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VttStyleBlock<Id = Uuid7> {
  id: Id,
  subtitle_track_id: Id,
  ordinal: u32,
  css_text: SmolStr,
}

impl VttStyleBlock<Uuid7> {
  pub fn try_new(
    id: Uuid7,
    subtitle_track_id: Uuid7,
    ordinal: u32,
    css_text: impl Into<SmolStr>,
  ) -> Result<Self, SubtitleCueError> {
    if id.is_nil() {
      return Err(SubtitleCueError::NilId);
    }
    if subtitle_track_id.is_nil() {
      return Err(SubtitleCueError::NilSubtitleTrackId);
    }
    Ok(Self {
      id,
      subtitle_track_id,
      ordinal,
      css_text: css_text.into(),
    })
  }
}

impl<Id> VttStyleBlock<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }
  #[inline(always)]
  pub const fn subtitle_track_id_ref(&self) -> &Id {
    &self.subtitle_track_id
  }
  #[inline(always)]
  pub const fn ordinal(&self) -> u32 {
    self.ordinal
  }
  #[inline(always)]
  pub fn css_text(&self) -> &str {
    self.css_text.as_str()
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_ordinal(mut self, v: u32) -> Self {
    self.ordinal = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_css_text(mut self, v: impl Into<SmolStr>) -> Self {
    self.css_text = v.into();
    self
  }
}

// ===========================================================================
// AssStyle — per-track ASS V4+ Style row
// ===========================================================================

/// Per-track ASS/SSA `[V4+ Styles]` row. The full set of fields a
/// `Dialogue` line references via its Style name.
#[derive(Debug, Clone, PartialEq)]
pub struct AssStyle<Id = Uuid7> {
  id: Id,
  subtitle_track_id: Id,
  name: SmolStr,
  fontname: SmolStr,
  fontsize: f32,
  primary_colour: u32,
  secondary_colour: u32,
  outline_colour: u32,
  back_colour: u32,
  bold: bool,
  italic: bool,
  underline: bool,
  strikeout: bool,
  scale_x: i32,
  scale_y: i32,
  spacing: i32,
  angle: f32,
  border_style: i16,
  outline: f32,
  shadow: f32,
  alignment: i16,
  margin_l: i32,
  margin_r: i32,
  margin_v: i32,
  encoding: i32,
}

impl AssStyle<Uuid7> {
  /// Validating constructor with required `name` (non-empty, ASS Style
  /// names are the FK key parsers resolve `AssData::style_id` from).
  pub fn try_new(
    id: Uuid7,
    subtitle_track_id: Uuid7,
    name: impl Into<SmolStr>,
  ) -> Result<Self, SubtitleCueError> {
    if id.is_nil() {
      return Err(SubtitleCueError::NilId);
    }
    if subtitle_track_id.is_nil() {
      return Err(SubtitleCueError::NilSubtitleTrackId);
    }
    let name: SmolStr = name.into();
    if name.is_empty() {
      return Err(SubtitleCueError::EmptyAssStyleName);
    }
    Ok(Self {
      id,
      subtitle_track_id,
      name,
      fontname: SmolStr::new_inline("Arial"),
      fontsize: 20.0,
      primary_colour: 0x00FFFFFF,
      secondary_colour: 0x000000FF,
      outline_colour: 0x00000000,
      back_colour: 0x00000000,
      bold: false,
      italic: false,
      underline: false,
      strikeout: false,
      scale_x: 100,
      scale_y: 100,
      spacing: 0,
      angle: 0.0,
      border_style: 1,
      outline: 2.0,
      shadow: 0.0,
      alignment: 2,
      margin_l: 10,
      margin_r: 10,
      margin_v: 10,
      encoding: 1,
    })
  }
}

impl<Id> AssStyle<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }
  #[inline(always)]
  pub const fn subtitle_track_id_ref(&self) -> &Id {
    &self.subtitle_track_id
  }
  #[inline(always)]
  pub fn name(&self) -> &str {
    self.name.as_str()
  }
  #[inline(always)]
  pub fn fontname(&self) -> &str {
    self.fontname.as_str()
  }
  #[inline(always)]
  pub const fn fontsize(&self) -> f32 {
    self.fontsize
  }
  #[inline(always)]
  pub const fn primary_colour(&self) -> u32 {
    self.primary_colour
  }
  #[inline(always)]
  pub const fn secondary_colour(&self) -> u32 {
    self.secondary_colour
  }
  #[inline(always)]
  pub const fn outline_colour(&self) -> u32 {
    self.outline_colour
  }
  #[inline(always)]
  pub const fn back_colour(&self) -> u32 {
    self.back_colour
  }
  #[inline(always)]
  pub const fn bold(&self) -> bool {
    self.bold
  }
  #[inline(always)]
  pub const fn italic(&self) -> bool {
    self.italic
  }
  #[inline(always)]
  pub const fn underline(&self) -> bool {
    self.underline
  }
  #[inline(always)]
  pub const fn strikeout(&self) -> bool {
    self.strikeout
  }
  #[inline(always)]
  pub const fn scale_x(&self) -> i32 {
    self.scale_x
  }
  #[inline(always)]
  pub const fn scale_y(&self) -> i32 {
    self.scale_y
  }
  #[inline(always)]
  pub const fn spacing(&self) -> i32 {
    self.spacing
  }
  #[inline(always)]
  pub const fn angle(&self) -> f32 {
    self.angle
  }
  #[inline(always)]
  pub const fn border_style(&self) -> i16 {
    self.border_style
  }
  #[inline(always)]
  pub const fn outline(&self) -> f32 {
    self.outline
  }
  #[inline(always)]
  pub const fn shadow(&self) -> f32 {
    self.shadow
  }
  #[inline(always)]
  pub const fn alignment(&self) -> i16 {
    self.alignment
  }
  #[inline(always)]
  pub const fn margin_l(&self) -> i32 {
    self.margin_l
  }
  #[inline(always)]
  pub const fn margin_r(&self) -> i32 {
    self.margin_r
  }
  #[inline(always)]
  pub const fn margin_v(&self) -> i32 {
    self.margin_v
  }
  #[inline(always)]
  pub const fn encoding(&self) -> i32 {
    self.encoding
  }

  // Builders — typography group.
  #[must_use]
  #[inline(always)]
  pub fn with_fontname(mut self, v: impl Into<SmolStr>) -> Self {
    self.fontname = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_fontsize(mut self, v: f32) -> Self {
    self.fontsize = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_primary_colour(mut self, v: u32) -> Self {
    self.primary_colour = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_secondary_colour(mut self, v: u32) -> Self {
    self.secondary_colour = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_outline_colour(mut self, v: u32) -> Self {
    self.outline_colour = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_back_colour(mut self, v: u32) -> Self {
    self.back_colour = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_bold(mut self) -> Self {
    self.bold = true;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn maybe_bold(mut self, v: bool) -> Self {
    self.bold = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_italic(mut self) -> Self {
    self.italic = true;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn maybe_italic(mut self, v: bool) -> Self {
    self.italic = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_underline(mut self) -> Self {
    self.underline = true;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn maybe_underline(mut self, v: bool) -> Self {
    self.underline = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_strikeout(mut self) -> Self {
    self.strikeout = true;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn maybe_strikeout(mut self, v: bool) -> Self {
    self.strikeout = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_scale_x(mut self, v: i32) -> Self {
    self.scale_x = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_scale_y(mut self, v: i32) -> Self {
    self.scale_y = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_spacing(mut self, v: i32) -> Self {
    self.spacing = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_angle(mut self, v: f32) -> Self {
    self.angle = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_border_style(mut self, v: i16) -> Self {
    self.border_style = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_outline(mut self, v: f32) -> Self {
    self.outline = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_shadow(mut self, v: f32) -> Self {
    self.shadow = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_alignment(mut self, v: i16) -> Self {
    self.alignment = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_margin_l(mut self, v: i32) -> Self {
    self.margin_l = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_margin_r(mut self, v: i32) -> Self {
    self.margin_r = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_margin_v(mut self, v: i32) -> Self {
    self.margin_v = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_encoding(mut self, v: i32) -> Self {
    self.encoding = v;
    self
  }
}

// ===========================================================================
// TtmlRegion — per-track TTML `<region>` element
// ===========================================================================

/// Per-track TTML `<region>` element. Cues reference a region by `id`
/// via `TtmlData::region_id`. The free-form `xml_attrs` carries the
/// region's serialised XML attribute list (`tts:origin`,
/// `tts:extent`, …) verbatim — TTML's styling vocabulary is large
/// enough that pinning each attribute as a typed column would invent
/// a parser; storing the raw fragment keeps the round trip lossless.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtmlRegion<Id = Uuid7> {
  id: Id,
  subtitle_track_id: Id,
  xml_id: SmolStr,
  xml_attrs: SmolStr,
}

impl TtmlRegion<Uuid7> {
  pub fn try_new(
    id: Uuid7,
    subtitle_track_id: Uuid7,
    xml_id: impl Into<SmolStr>,
  ) -> Result<Self, SubtitleCueError> {
    if id.is_nil() {
      return Err(SubtitleCueError::NilId);
    }
    if subtitle_track_id.is_nil() {
      return Err(SubtitleCueError::NilSubtitleTrackId);
    }
    Ok(Self {
      id,
      subtitle_track_id,
      xml_id: xml_id.into(),
      xml_attrs: SmolStr::new_inline(""),
    })
  }
}

impl<Id> TtmlRegion<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }
  #[inline(always)]
  pub const fn subtitle_track_id_ref(&self) -> &Id {
    &self.subtitle_track_id
  }
  #[inline(always)]
  pub fn xml_id(&self) -> &str {
    self.xml_id.as_str()
  }
  #[inline(always)]
  pub fn xml_attrs(&self) -> &str {
    self.xml_attrs.as_str()
  }

  #[must_use]
  #[inline]
  pub fn with_xml_attrs(mut self, v: impl Into<SmolStr>) -> Self {
    self.xml_attrs = v.into();
    self
  }
}

// ===========================================================================
// TtmlStyle — per-track TTML `<style>` element
// ===========================================================================

/// Per-track TTML `<style>` element. Cues reference a style by `id`
/// via `TtmlData::style_id`. Like [`TtmlRegion`], the styling
/// vocabulary is stored verbatim in `xml_attrs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtmlStyle<Id = Uuid7> {
  id: Id,
  subtitle_track_id: Id,
  xml_id: SmolStr,
  xml_attrs: SmolStr,
}

impl TtmlStyle<Uuid7> {
  pub fn try_new(
    id: Uuid7,
    subtitle_track_id: Uuid7,
    xml_id: impl Into<SmolStr>,
  ) -> Result<Self, SubtitleCueError> {
    if id.is_nil() {
      return Err(SubtitleCueError::NilId);
    }
    if subtitle_track_id.is_nil() {
      return Err(SubtitleCueError::NilSubtitleTrackId);
    }
    Ok(Self {
      id,
      subtitle_track_id,
      xml_id: xml_id.into(),
      xml_attrs: SmolStr::new_inline(""),
    })
  }
}

impl<Id> TtmlStyle<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }
  #[inline(always)]
  pub const fn subtitle_track_id_ref(&self) -> &Id {
    &self.subtitle_track_id
  }
  #[inline(always)]
  pub fn xml_id(&self) -> &str {
    self.xml_id.as_str()
  }
  #[inline(always)]
  pub fn xml_attrs(&self) -> &str {
    self.xml_attrs.as_str()
  }

  #[must_use]
  #[inline]
  pub fn with_xml_attrs(mut self, v: impl Into<SmolStr>) -> Self {
    self.xml_attrs = v.into();
    self
  }
}

// ===========================================================================
// SamiStyle — per-track SAMI `<style>` class
// ===========================================================================

/// Per-track SAMI `<STYLE>` class. Cues reference a style by
/// `class_name` (the SAMI selector e.g. `ENCC`); the CSS body rides
/// in `css_text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SamiStyle<Id = Uuid7> {
  id: Id,
  subtitle_track_id: Id,
  class_name: SmolStr,
  css_text: SmolStr,
}

impl SamiStyle<Uuid7> {
  pub fn try_new(
    id: Uuid7,
    subtitle_track_id: Uuid7,
    class_name: impl Into<SmolStr>,
  ) -> Result<Self, SubtitleCueError> {
    if id.is_nil() {
      return Err(SubtitleCueError::NilId);
    }
    if subtitle_track_id.is_nil() {
      return Err(SubtitleCueError::NilSubtitleTrackId);
    }
    Ok(Self {
      id,
      subtitle_track_id,
      class_name: class_name.into(),
      css_text: SmolStr::new_inline(""),
    })
  }
}

impl<Id> SamiStyle<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }
  #[inline(always)]
  pub const fn subtitle_track_id_ref(&self) -> &Id {
    &self.subtitle_track_id
  }
  #[inline(always)]
  pub fn class_name(&self) -> &str {
    self.class_name.as_str()
  }
  #[inline(always)]
  pub fn css_text(&self) -> &str {
    self.css_text.as_str()
  }

  #[must_use]
  #[inline]
  pub fn with_css_text(mut self, v: impl Into<SmolStr>) -> Self {
    self.css_text = v.into();
    self
  }
}

// ===========================================================================
// VobSubPalette — per-track DVD palette aggregate (16 RGB entries)
// ===========================================================================

/// Per-track DVD VobSub palette — 16 fixed-position RGB entries. The
/// 4-entry colour/contrast indices on a `VobSubData` cue index into
/// these. Stored as a `[u32; 16]` where each `u32` is `0x00RRGGBB`
/// (alpha is conveyed via the cue's contrast indices).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VobSubPalette<Id = Uuid7> {
  id: Id,
  subtitle_track_id: Id,
  entries: [u32; 16],
}

impl VobSubPalette<Uuid7> {
  /// Validating constructor — rejects nil id / parent track id.
  pub fn try_new(id: Uuid7, subtitle_track_id: Uuid7) -> Result<Self, SubtitleCueError> {
    if id.is_nil() {
      return Err(SubtitleCueError::NilId);
    }
    if subtitle_track_id.is_nil() {
      return Err(SubtitleCueError::NilSubtitleTrackId);
    }
    Ok(Self {
      id,
      subtitle_track_id,
      entries: [0u32; 16],
    })
  }
}

impl<Id> VobSubPalette<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  #[inline(always)]
  pub const fn subtitle_track_id_ref(&self) -> &Id {
    &self.subtitle_track_id
  }

  #[inline(always)]
  pub const fn entries(&self) -> &[u32; 16] {
    &self.entries
  }

  #[must_use]
  #[inline(always)]
  pub const fn with_entries(mut self, v: [u32; 16]) -> Self {
    self.entries = v;
    self
  }
}

// ===========================================================================
// LrcMetadata — per-track LRC header tags (1:1 with subtitle_track)
// ===========================================================================

/// Per-track LRC header metadata. Carries the `[ti:]`/`[ar:]`/… tags
/// and a global playback offset. 1:1 with the parent `SubtitleTrack`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LrcMetadata<Id = Uuid7> {
  subtitle_track_id: Id,
  title: SmolStr,
  artist: SmolStr,
  album: SmolStr,
  author: SmolStr,
  creator: SmolStr,
  length: SmolStr,
  offset_ms: i32,
}

impl LrcMetadata<Uuid7> {
  /// Validating constructor. Rejects nil `subtitle_track_id`.
  pub fn try_new(subtitle_track_id: Uuid7) -> Result<Self, SubtitleCueError> {
    if subtitle_track_id.is_nil() {
      return Err(SubtitleCueError::NilSubtitleTrackId);
    }
    Ok(Self {
      subtitle_track_id,
      title: SmolStr::new_inline(""),
      artist: SmolStr::new_inline(""),
      album: SmolStr::new_inline(""),
      author: SmolStr::new_inline(""),
      creator: SmolStr::new_inline(""),
      length: SmolStr::new_inline(""),
      offset_ms: 0,
    })
  }
}

impl<Id> LrcMetadata<Id> {
  #[inline(always)]
  pub const fn subtitle_track_id_ref(&self) -> &Id {
    &self.subtitle_track_id
  }
  #[inline(always)]
  pub fn title(&self) -> &str {
    self.title.as_str()
  }
  #[inline(always)]
  pub fn artist(&self) -> &str {
    self.artist.as_str()
  }
  #[inline(always)]
  pub fn album(&self) -> &str {
    self.album.as_str()
  }
  #[inline(always)]
  pub fn author(&self) -> &str {
    self.author.as_str()
  }
  #[inline(always)]
  pub fn creator(&self) -> &str {
    self.creator.as_str()
  }
  #[inline(always)]
  pub fn length(&self) -> &str {
    self.length.as_str()
  }
  #[inline(always)]
  pub const fn offset_ms(&self) -> i32 {
    self.offset_ms
  }

  #[must_use]
  #[inline(always)]
  pub fn with_title(mut self, v: impl Into<SmolStr>) -> Self {
    self.title = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_artist(mut self, v: impl Into<SmolStr>) -> Self {
    self.artist = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_album(mut self, v: impl Into<SmolStr>) -> Self {
    self.album = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_author(mut self, v: impl Into<SmolStr>) -> Self {
    self.author = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_creator(mut self, v: impl Into<SmolStr>) -> Self {
    self.creator = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub fn with_length(mut self, v: impl Into<SmolStr>) -> Self {
    self.length = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_offset_ms(mut self, v: i32) -> Self {
    self.offset_ms = v;
    self
  }
}

// ===========================================================================
// Errors
// ===========================================================================

/// Returned when [`SubtitleCue::try_new`] or an aggregate constructor
/// cannot uphold an invariant.
#[derive(Debug, Clone, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum SubtitleCueError {
  /// Supplied `id` was the nil sentinel.
  #[error("SubtitleCue id must not be the nil UUID")]
  NilId,
  /// Supplied `subtitle_track_id` was the nil sentinel.
  #[error("`subtitle_track_id` (FK → SubtitleTrack) must not be the nil UUID")]
  NilSubtitleTrackId,
  /// Supplied `subtitle_cue_id` was the nil sentinel (LrcWord child FK).
  #[error("`subtitle_cue_id` (FK → SubtitleCue) must not be the nil UUID")]
  NilSubtitleCueId,
  /// AssStyle `name` was empty (the Style name is the parser's FK key).
  #[error("AssStyle name must be non-empty")]
  EmptyAssStyleName,
  /// `Cea608Data.channel` was outside the valid `1..=4` range.
  #[error("Cea608Data.channel must be 1..=4, got {0}")]
  Cea608ChannelOutOfRange(u8),
  /// `EbuStlData.justification` was outside the valid `1..=3` range.
  #[error("EbuStlData.justification must be 1..=3, got {0}")]
  EbuStlJustificationOutOfRange(u8),
  /// A row carried a [`SubtitleCueKind`] discriminant whose `D` payload
  /// type isn't implemented in this revision (reserved for issue #56).
  #[error("subtitle cue kind `{0}` not yet implemented (issue #56)")]
  UnimplementedFormat(SubtitleCueKind),
  /// Last-resort escape hatch for descriptive text that has no
  /// structured variant.
  #[error("{0}")]
  Other(std::borrow::Cow<'static, str>),
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use mediatime::Timebase;

  fn span() -> TimeRange {
    TimeRange::new(1000, 2000, Timebase::default())
  }

  #[test]
  fn kind_discriminants_round_trip() {
    for k in [
      SubtitleCueKind::Srt,
      SubtitleCueKind::Vtt,
      SubtitleCueKind::Ass,
      SubtitleCueKind::MicroDvd,
      SubtitleCueKind::SubViewer,
      SubtitleCueKind::Sbv,
      SubtitleCueKind::Lrc,
      SubtitleCueKind::Ttml,
      SubtitleCueKind::Sami,
      SubtitleCueKind::VobSub,
      SubtitleCueKind::Pgs,
      SubtitleCueKind::Cea608,
      SubtitleCueKind::EbuStl,
    ] {
      assert_eq!(SubtitleCueKind::try_from_u8(k.to_u8()), Some(k));
    }
    assert_eq!(SubtitleCueKind::try_from_u8(13), None);
  }

  #[test]
  fn kind_implemented_flags_match_brief() {
    // All 13 formats are now implemented (issue #56 closed): 4 rev-1
    // formats (Srt/Vtt/Ass/Lrc) + 5 text formats (MicroDvd/SubViewer/
    // Sbv/Ttml/Sami) + 2 bitmap (VobSub/Pgs) + 2 broadcast (Cea608/
    // EbuStl).
    for k in [
      SubtitleCueKind::Srt,
      SubtitleCueKind::Vtt,
      SubtitleCueKind::Ass,
      SubtitleCueKind::Lrc,
      SubtitleCueKind::MicroDvd,
      SubtitleCueKind::SubViewer,
      SubtitleCueKind::Sbv,
      SubtitleCueKind::Ttml,
      SubtitleCueKind::Sami,
      SubtitleCueKind::VobSub,
      SubtitleCueKind::Pgs,
      SubtitleCueKind::Cea608,
      SubtitleCueKind::EbuStl,
    ] {
      assert!(k.is_implemented(), "{k:?} should be implemented");
    }
  }

  #[test]
  fn kind_as_str_round_trips_and_displays() {
    for (k, expected) in [
      (SubtitleCueKind::Srt, "srt"),
      (SubtitleCueKind::Vtt, "vtt"),
      (SubtitleCueKind::Ass, "ass"),
      (SubtitleCueKind::MicroDvd, "micro_dvd"),
      (SubtitleCueKind::SubViewer, "sub_viewer"),
      (SubtitleCueKind::Sbv, "sbv"),
      (SubtitleCueKind::Lrc, "lrc"),
      (SubtitleCueKind::Ttml, "ttml"),
      (SubtitleCueKind::Sami, "sami"),
      (SubtitleCueKind::VobSub, "vob_sub"),
      (SubtitleCueKind::Pgs, "pgs"),
      (SubtitleCueKind::Cea608, "cea_608"),
      (SubtitleCueKind::EbuStl, "ebu_stl"),
    ] {
      assert_eq!(k.as_str(), expected, "{:?} slug mismatch", k);
      assert_eq!(SubtitleCueKind::from_str(expected), Some(k));
      assert_eq!(format!("{k}"), expected, "{:?} Display mismatch", k);
    }
    assert_eq!(SubtitleCueKind::from_str("unknown"), None);
    assert_eq!(SubtitleCueKind::from_str(""), None);
  }

  #[test]
  fn kind_as_extension_maps_to_disk_form() {
    for (k, expected) in [
      (SubtitleCueKind::Srt, "srt"),
      (SubtitleCueKind::Vtt, "vtt"),
      (SubtitleCueKind::Ass, "ass"),
      (SubtitleCueKind::MicroDvd, "sub"),
      (SubtitleCueKind::SubViewer, "sub"),
      (SubtitleCueKind::Sbv, "sbv"),
      (SubtitleCueKind::Lrc, "lrc"),
      (SubtitleCueKind::Ttml, "ttml"),
      (SubtitleCueKind::Sami, "smi"),
      (SubtitleCueKind::VobSub, "idx"),
      (SubtitleCueKind::Pgs, "sup"),
      (SubtitleCueKind::Cea608, ""),
      (SubtitleCueKind::EbuStl, "stl"),
    ] {
      assert_eq!(k.as_extension(), expected, "{:?} extension mismatch", k);
    }
    // Slug-form mismatch is intentional: as_str("micro_dvd") != as_extension("sub").
    assert_ne!(
      SubtitleCueKind::MicroDvd.as_str(),
      SubtitleCueKind::MicroDvd.as_extension()
    );
    assert_eq!(SubtitleCueKind::Cea608.as_extension(), "");
  }

  #[test]
  fn kind_cea608_predicate_is_hand_written() {
    // `Cea608` carries `#[is_variant(ignore)]` to skip the awkward
    // auto-derived `is_cea_608`; the hand-written `is_cea608` is the
    // public predicate name.
    assert!(SubtitleCueKind::Cea608.is_cea608());
    assert!(!SubtitleCueKind::Srt.is_cea608());
    assert!(!SubtitleCueKind::EbuStl.is_cea608());
  }

  #[test]
  fn vtt_vertical_slug_round_trips_and_displays() {
    for (v, slug) in [(VttVertical::Lr, "lr"), (VttVertical::Rl, "rl")] {
      assert_eq!(v.as_str(), slug);
      assert_eq!(VttVertical::from_str(slug), Some(v));
      assert_eq!(format!("{v}"), slug);
    }
    assert_eq!(VttVertical::from_str("unknown"), None);
    assert_eq!(VttVertical::from_str(""), None);
  }

  #[test]
  fn vtt_line_align_slug_round_trips_and_displays() {
    for (v, slug) in [
      (VttLineAlign::Start, "start"),
      (VttLineAlign::Center, "center"),
      (VttLineAlign::End, "end"),
    ] {
      assert_eq!(v.as_str(), slug);
      assert_eq!(VttLineAlign::from_str(slug), Some(v));
      assert_eq!(format!("{v}"), slug);
    }
    assert_eq!(VttLineAlign::from_str("middle"), None);
  }

  #[test]
  fn vtt_position_align_slug_round_trips_and_displays() {
    for (v, slug) in [
      (VttPositionAlign::Start, "start"),
      (VttPositionAlign::Center, "center"),
      (VttPositionAlign::End, "end"),
      (VttPositionAlign::LineLeft, "line-left"),
      (VttPositionAlign::LineRight, "line-right"),
    ] {
      assert_eq!(v.as_str(), slug);
      assert_eq!(VttPositionAlign::from_str(slug), Some(v));
      assert_eq!(format!("{v}"), slug);
    }
    // W3C uses hyphen — confirm we accept ONLY the hyphenated form.
    assert_eq!(VttPositionAlign::from_str("line_left"), None);
    assert_eq!(VttPositionAlign::from_str("lineleft"), None);
  }

  #[test]
  fn vtt_text_align_slug_round_trips_and_displays() {
    for (v, slug) in [
      (VttTextAlign::Start, "start"),
      (VttTextAlign::Center, "center"),
      (VttTextAlign::End, "end"),
      (VttTextAlign::Left, "left"),
      (VttTextAlign::Right, "right"),
    ] {
      assert_eq!(v.as_str(), slug);
      assert_eq!(VttTextAlign::from_str(slug), Some(v));
      assert_eq!(format!("{v}"), slug);
    }
  }

  #[test]
  fn srt_cue_constructs_and_carries_kind() {
    let c = SrtCue::try_new_srt(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("hello"),
    )
    .unwrap();
    assert_eq!(c.ordinal(), 0);
    assert_eq!(c.text_ref().src(), "hello");
    assert_eq!(c.kind(), SubtitleCueKind::Srt);
  }

  #[test]
  fn srt_cue_rejects_nil_id() {
    let e =
      SrtCue::try_new_srt(Uuid7::nil(), Uuid7::new(), 0, span(), LocalizedText::new()).unwrap_err();
    assert!(e.is_nil_id());
  }

  #[test]
  fn srt_cue_rejects_nil_subtitle_track_id() {
    let e =
      SrtCue::try_new_srt(Uuid7::new(), Uuid7::nil(), 0, span(), LocalizedText::new()).unwrap_err();
    assert!(e.is_nil_subtitle_track_id());
  }

  #[test]
  fn vtt_cue_constructs_and_builders_chain() {
    let region_id = Uuid7::new();
    let d = VttData::<Uuid7>::new()
      .with_cue_identifier("c1")
      .with_vertical(VttVertical::Rl)
      .with_line_value("50%")
      .with_line_align(VttLineAlign::Center)
      .with_position_value("50%")
      .with_position_align(VttPositionAlign::Center)
      .with_size_value(80.0)
      .with_text_align(VttTextAlign::Start)
      .with_region_id(region_id)
      .with_voice("Speaker A")
      .with_styled_text("<b>hi</b>");
    let c: VttCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      1,
      span(),
      LocalizedText::from_src("hi"),
      d,
    )
    .unwrap();
    assert_eq!(c.kind(), SubtitleCueKind::Vtt);
    assert_eq!(c.data_ref().cue_identifier(), "c1");
    assert_eq!(c.data_ref().vertical(), Some(VttVertical::Rl));
    assert_eq!(c.data_ref().region_id_ref(), Some(&region_id));
    assert_eq!(c.data_ref().voice(), "Speaker A");
  }

  #[test]
  fn ass_cue_constructs_with_style_id() {
    let style_id = Uuid7::new();
    let d = AssData::<Uuid7>::new(style_id)
      .with_layer(2)
      .with_name("Alice")
      .with_margin_l(10)
      .with_margin_r(20)
      .with_margin_v(30)
      .with_effect("karaoke")
      .with_styled_text("{\\b1}hi{\\b0}");
    let c: AssCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::new(),
      d,
    )
    .unwrap();
    assert_eq!(c.kind(), SubtitleCueKind::Ass);
    assert_eq!(c.data_ref().style_id_ref(), &style_id);
    assert_eq!(c.data_ref().layer(), 2);
    assert_eq!(c.data_ref().name(), "Alice");
  }

  #[test]
  fn lrc_cue_line_and_word_flag() {
    let line: LrcCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("la la la"),
      LrcData::new(),
    )
    .unwrap();
    assert!(!line.data_ref().has_word_timing());

    let word: LrcCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src(""),
      LrcData::new().with_word_timing(),
    )
    .unwrap();
    assert!(word.data_ref().has_word_timing());
  }

  #[test]
  fn lrc_word_rejects_nil_subtitle_cue_id() {
    let e = LrcWord::try_new(Uuid7::nil(), 0, "la", 0).unwrap_err();
    assert!(e.is_nil_subtitle_cue_id());
  }

  #[test]
  fn vtt_region_rejects_nil_ids() {
    assert!(VttRegion::try_new(Uuid7::nil(), Uuid7::new(), "r")
      .unwrap_err()
      .is_nil_id());
    assert!(VttRegion::try_new(Uuid7::new(), Uuid7::nil(), "r")
      .unwrap_err()
      .is_nil_subtitle_track_id());
  }

  #[test]
  fn vtt_style_block_rejects_nil_ids() {
    assert!(VttStyleBlock::try_new(Uuid7::nil(), Uuid7::new(), 0, "")
      .unwrap_err()
      .is_nil_id());
    assert!(VttStyleBlock::try_new(Uuid7::new(), Uuid7::nil(), 0, "")
      .unwrap_err()
      .is_nil_subtitle_track_id());
  }

  #[test]
  fn ass_style_rejects_empty_name() {
    let e = AssStyle::try_new(Uuid7::new(), Uuid7::new(), "").unwrap_err();
    assert!(e.is_empty_ass_style_name());
  }

  #[test]
  fn ass_style_builders_round_trip() {
    let s = AssStyle::try_new(Uuid7::new(), Uuid7::new(), "Default")
      .unwrap()
      .with_fontname("Arial")
      .with_fontsize(48.0)
      .with_primary_colour(0x00FFFFFF)
      .with_bold()
      .with_italic()
      .with_outline(2.5)
      .with_shadow(1.5)
      .with_alignment(2);
    assert_eq!(s.name(), "Default");
    assert_eq!(s.fontsize(), 48.0);
    assert!(s.bold());
    assert!(s.italic());
    assert_eq!(s.alignment(), 2);
  }

  #[test]
  fn lrc_metadata_round_trip() {
    let m = LrcMetadata::try_new(Uuid7::new())
      .unwrap()
      .with_title("Song")
      .with_artist("Band")
      .with_offset_ms(-500);
    assert_eq!(m.title(), "Song");
    assert_eq!(m.artist(), "Band");
    assert_eq!(m.offset_ms(), -500);
  }

  // ---- New text-format domain types ---------------------------------------

  #[test]
  fn micro_dvd_cue_builds_and_carries_kind() {
    let d = MicroDvdData::new("{y:i}hi").with_styled_text("{y:b}hello");
    let c: MicroDvdCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("hello"),
      d,
    )
    .unwrap();
    assert_eq!(c.kind(), SubtitleCueKind::MicroDvd);
    assert_eq!(c.data_ref().styled_text(), "{y:b}hello");
  }

  #[test]
  fn sub_viewer_cue_builds_and_carries_kind() {
    let d = SubViewerData::new("[b]hi[/b][br]line 2");
    let c: SubViewerCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("hi"),
      d,
    )
    .unwrap();
    assert_eq!(c.kind(), SubtitleCueKind::SubViewer);
    assert_eq!(c.data_ref().styled_text(), "[b]hi[/b][br]line 2");
  }

  #[test]
  fn sbv_cue_builds_and_carries_kind() {
    let c: SbvCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("plain"),
      SbvData::new(),
    )
    .unwrap();
    assert_eq!(c.kind(), SubtitleCueKind::Sbv);
  }

  #[test]
  fn ttml_cue_builds_with_region_and_style_refs() {
    let region_id = Uuid7::new();
    let style_id = Uuid7::new();
    let d = TtmlData::<Uuid7>::new()
      .with_region_id(region_id)
      .with_style_id(style_id)
      .with_xml_id("cue-1")
      .with_styled_text("<span tts:color=\"red\">hi</span>");
    let c: TtmlCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("hi"),
      d,
    )
    .unwrap();
    assert_eq!(c.kind(), SubtitleCueKind::Ttml);
    assert_eq!(c.data_ref().region_id_ref(), Some(&region_id));
    assert_eq!(c.data_ref().style_id_ref(), Some(&style_id));
    assert_eq!(c.data_ref().xml_id(), "cue-1");
  }

  #[test]
  fn sami_cue_builds_with_class_selector() {
    let d = SamiData::new()
      .with_class_name("ENCC")
      .with_styled_text("<P><B>Hello</B></P>");
    let c: SamiCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("Hello"),
      d,
    )
    .unwrap();
    assert_eq!(c.kind(), SubtitleCueKind::Sami);
    assert_eq!(c.data_ref().class_name(), "ENCC");
  }

  #[test]
  fn ttml_region_rejects_nil_ids() {
    assert!(TtmlRegion::try_new(Uuid7::nil(), Uuid7::new(), "r")
      .unwrap_err()
      .is_nil_id());
    assert!(TtmlRegion::try_new(Uuid7::new(), Uuid7::nil(), "r")
      .unwrap_err()
      .is_nil_subtitle_track_id());
  }

  #[test]
  fn ttml_region_builders_round_trip() {
    let r = TtmlRegion::try_new(Uuid7::new(), Uuid7::new(), "r1")
      .unwrap()
      .with_xml_attrs("tts:origin=\"10% 80%\" tts:extent=\"80% 20%\"");
    assert_eq!(r.xml_id(), "r1");
    assert!(r.xml_attrs().contains("tts:origin"));
  }

  #[test]
  fn ttml_style_rejects_nil_ids() {
    assert!(TtmlStyle::try_new(Uuid7::nil(), Uuid7::new(), "s")
      .unwrap_err()
      .is_nil_id());
    assert!(TtmlStyle::try_new(Uuid7::new(), Uuid7::nil(), "s")
      .unwrap_err()
      .is_nil_subtitle_track_id());
  }

  #[test]
  fn sami_style_rejects_nil_ids() {
    assert!(SamiStyle::try_new(Uuid7::nil(), Uuid7::new(), "ENCC")
      .unwrap_err()
      .is_nil_id());
    assert!(SamiStyle::try_new(Uuid7::new(), Uuid7::nil(), "ENCC")
      .unwrap_err()
      .is_nil_subtitle_track_id());
  }

  // ---- New bitmap / broadcast domain types --------------------------------

  #[test]
  fn vob_sub_cue_builds_and_carries_kind() {
    let palette_id = Uuid7::new();
    let d = VobSubData::<Uuid7>::new(palette_id)
      .with_bitmap(Bytes::from_static(b"\x01\x02\x03"))
      .with_width(720)
      .with_height(60)
      .with_pos(20, 540)
      .with_color_indices([1, 2, 3, 4])
      .with_contrast_indices([0, 0xF, 0xF, 0xF]);
    let c: VobSubCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::new(),
      d,
    )
    .unwrap();
    assert_eq!(c.kind(), SubtitleCueKind::VobSub);
    assert_eq!(c.data_ref().palette_id_ref(), &palette_id);
    assert_eq!(c.data_ref().bitmap_ref().as_ref(), b"\x01\x02\x03");
    assert_eq!(c.data_ref().width(), 720);
    assert_eq!(c.data_ref().pos_x(), 20);
    assert_eq!(c.data_ref().color_indices(), &[1, 2, 3, 4]);
  }

  #[test]
  fn pgs_cue_builds_and_carries_kind() {
    let d = PgsData::new()
      .with_bitmap(Bytes::from_static(b"\xAA\xBB"))
      .with_palette_bytes(Bytes::from_static(b"\x10\x20\x30\x40"))
      .with_width(1920)
      .with_height(80)
      .with_pos(0, 920)
      .with_composition_state(0x80);
    let c: PgsCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::new(),
      d,
    )
    .unwrap();
    assert_eq!(c.kind(), SubtitleCueKind::Pgs);
    assert_eq!(c.data_ref().bitmap_ref().as_ref(), b"\xAA\xBB");
    assert_eq!(
      c.data_ref().palette_bytes_ref().as_ref(),
      b"\x10\x20\x30\x40"
    );
    assert_eq!(c.data_ref().composition_state(), 0x80);
  }

  #[test]
  fn cea608_data_accepts_valid_channels() {
    for ch in 1u8..=4 {
      let d = Cea608Data::try_new(ch).unwrap();
      assert_eq!(d.channel(), ch);
    }
  }

  #[test]
  fn cea608_data_rejects_zero_channel() {
    let e = Cea608Data::try_new(0).unwrap_err();
    assert!(matches!(e, SubtitleCueError::Cea608ChannelOutOfRange(0)));
  }

  #[test]
  fn cea608_data_rejects_channel_above_4() {
    let e = Cea608Data::try_new(5).unwrap_err();
    assert!(matches!(e, SubtitleCueError::Cea608ChannelOutOfRange(5)));
  }

  #[test]
  fn cea608_cue_builds_and_carries_kind() {
    let d = Cea608Data::try_new(2)
      .unwrap()
      .with_pac_byte_pair(0x1170)
      .with_styled_text("Caption text");
    let c: Cea608Cue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("Caption text"),
      d,
    )
    .unwrap();
    assert_eq!(c.kind(), SubtitleCueKind::Cea608);
    assert_eq!(c.data_ref().channel(), 2);
    assert_eq!(c.data_ref().pac_byte_pair(), 0x1170);
  }

  #[test]
  fn ebu_stl_data_accepts_valid_justification() {
    for j in 1u8..=3 {
      let d = EbuStlData::try_new(j).unwrap();
      assert_eq!(d.justification(), j);
    }
  }

  #[test]
  fn ebu_stl_data_rejects_zero_justification() {
    let e = EbuStlData::try_new(0).unwrap_err();
    assert!(matches!(
      e,
      SubtitleCueError::EbuStlJustificationOutOfRange(0)
    ));
  }

  #[test]
  fn ebu_stl_data_rejects_justification_above_3() {
    let e = EbuStlData::try_new(4).unwrap_err();
    assert!(matches!(
      e,
      SubtitleCueError::EbuStlJustificationOutOfRange(4)
    ));
  }

  #[test]
  fn ebu_stl_cue_builds_and_carries_kind() {
    let d = EbuStlData::try_new(2)
      .unwrap()
      .with_subtitle_number(42)
      .with_cumulative()
      .with_vertical_pos(20);
    let c: EbuStlCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(),
      LocalizedText::from_src("Hi"),
      d,
    )
    .unwrap();
    assert_eq!(c.kind(), SubtitleCueKind::EbuStl);
    assert_eq!(c.data_ref().subtitle_number(), 42);
    assert!(c.data_ref().cumulative());
    assert_eq!(c.data_ref().vertical_pos(), 20);
  }

  #[test]
  fn vob_sub_palette_rejects_nil_ids() {
    assert!(VobSubPalette::try_new(Uuid7::nil(), Uuid7::new())
      .unwrap_err()
      .is_nil_id());
    assert!(VobSubPalette::try_new(Uuid7::new(), Uuid7::nil())
      .unwrap_err()
      .is_nil_subtitle_track_id());
  }

  #[test]
  fn vob_sub_palette_builders_round_trip() {
    let mut entries = [0u32; 16];
    entries[0] = 0x00FF0000; // red
    entries[1] = 0x0000FF00; // green
    let p = VobSubPalette::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_entries(entries);
    assert_eq!(p.entries()[0], 0x00FF0000);
    assert_eq!(p.entries()[1], 0x0000FF00);
  }
}

/// Exhaustive by-value decomposition of [`SubtitleCue`] — every stored
/// field, generic over the per-format payload `D` like the cue itself.
///
/// Public-field data-transfer struct (the conversion-boundary exception
/// to the encapsulation rule): cross-suite conversions (`crate::graph`)
/// destructure it exhaustively, so adding a field breaks them at compile
/// time instead of silently dropping data.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleCueParts<Id, D> {
  pub id: Id,
  pub subtitle_track_id: Id,
  pub ordinal: u32,
  pub span: TimeRange,
  pub text: LocalizedText,
  pub data: D,
}

impl<Id, D> SubtitleCue<Id, D> {
  /// Decompose into [`SubtitleCueParts`] — exhaustive, by value.
  #[inline(always)]
  pub fn into_parts(self) -> SubtitleCueParts<Id, D> {
    let Self {
      id,
      subtitle_track_id,
      ordinal,
      span,
      text,
      data,
    } = self;
    SubtitleCueParts {
      id,
      subtitle_track_id,
      ordinal,
      span,
      text,
      data,
    }
  }
}

impl<Id, D> SubtitleCue<Id, D> {
  /// Invariant-carrying constructor from [`SubtitleCueParts`] —
  /// `pub(crate)`, reserved for in-crate conversions from
  /// already-validated values (`crate::graph`).
  #[cfg(all(
    feature = "std",
    feature = "video",
    feature = "audio",
    feature = "subtitle"
  ))]
  #[inline(always)]
  pub(crate) fn rehydrate(parts: SubtitleCueParts<Id, D>) -> Self {
    let SubtitleCueParts {
      id,
      subtitle_track_id,
      ordinal,
      span,
      text,
      data,
    } = parts;
    Self {
      id,
      subtitle_track_id,
      ordinal,
      span,
      text,
      data,
    }
  }
}
