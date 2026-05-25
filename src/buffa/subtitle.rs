//! Wire ⇄ domain conversions for the polymorphic [`SubtitleCue`] family
//! and its per-track aggregate siblings ([`VttRegion`], [`VttStyleBlock`],
//! [`AssStyle`], [`LrcMetadata`], [`LrcWord`]).
//!
//! Locked `schema/subtitle_cues.md` (polymorphic base + per-format
//! detail). The wire `SubtitleCue` mirrors the domain `SubtitleCue<Id, D>`
//! by carrying:
//!
//! 1. **Base fields** (id, parent track id, ordinal, span PTS, text, kind).
//!    Span is PTS-only on the wire — timebase lives on the parent
//!    `SubtitleTrack` per the timebase-dedup rule. Decoding therefore
//!    needs the parent timebase passed externally
//!    ([`subtitle_cue_from_wire`]); a default-timebase convenience
//!    constructor exists too ([`subtitle_cue_from_wire_default_timebase`]).
//! 2. **Per-format detail** via a `data` oneof — one `SrtData` / `VttData`
//!    / `AssData` / `LrcData` arm per implemented kind. The `kind`
//!    discriminator on the base MUST match the present oneof variant;
//!    mismatch surfaces as [`BuffaError::SubtitleCueKindOneofMismatch`].
//!
//! ## Implemented vs reserved kinds
//!
//! `SubtitleCueKind` carries every value from day one (stable wire
//! discriminants), but only `Srt` / `Vtt` / `Ass` / `Lrc` have a data
//! arm and a domain payload. A wire frame whose `kind` is one of the
//! reserved discriminants (`MicroDvd`, `SubViewer`, `Sbv`, `Ttml`,
//! `Sami`, `VobSub`, `Pgs`, `Cea608`, `EbuStl`) surfaces as
//! [`BuffaError::UnimplementedSubtitleCueKind`] — tracked under issue
//! #56.
//!
//! ## Sibling aggregates (per-track)
//!
//! `VttRegion`, `VttStyleBlock`, `AssStyle` and `LrcMetadata` each
//! round-trip 1:1 against the wire message of the same name. `LrcWord`
//! is the child of an LRC cue and round-trips via its
//! `subtitle_cue_id` FK + `ordinal` ordering.

use ::buffa::bytes::Bytes;
use smol_str::SmolStr;

use crate::{
  buffa::error::BuffaError,
  domain::{
    aggregates::subtitle::{
      AssCue, AssData, AssStyle, LrcCue, LrcData, LrcMetadata, LrcWord, SrtCue, SrtData,
      SubtitleCueDetails, SubtitleCueError, SubtitleCueKind, VttCue, VttData, VttLineAlign,
      VttPositionAlign, VttRegion, VttStyleBlock, VttTextAlign, VttVertical,
    },
    vo::LocalizedText,
    SubtitleCue, Uuid7,
  },
  generated::media::v1 as wire,
};

#[cfg(all(not(feature = "std"), feature = "alloc"))]
#[allow(unused_imports)]
use std::{
  borrow::ToOwned,
  string::{String, ToString},
  vec::Vec,
};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn id_from_bytes(b: &Bytes) -> Result<Uuid7, BuffaError> {
  let arr: [u8; 16] = b
    .as_ref()
    .try_into()
    .map_err(|_| BuffaError::IdWrongLength(b.len()))?;
  Uuid7::try_from_bytes(arr).map_err(BuffaError::from)
}

fn id_to_bytes(id: &Uuid7) -> Bytes {
  Bytes::copy_from_slice(id.as_bytes())
}

fn opt_id_from_bytes(b: &Bytes) -> Result<Option<Uuid7>, BuffaError> {
  if b.as_ref().is_empty() {
    Ok(None)
  } else {
    id_from_bytes(b).map(Some)
  }
}

fn opt_id_to_bytes(id: Option<&Uuid7>) -> Bytes {
  match id {
    Some(id) => id_to_bytes(id),
    None => Bytes::new(),
  }
}

fn localized_text_to_wire_string(t: &LocalizedText) -> String {
  // Base `SubtitleCue.text` is the raw src text only — `translated`
  // rides on the wire `LocalizedText` form elsewhere. The per-format
  // detail messages keep their own untranslated `styled_text` field.
  t.src().to_owned()
}

fn localized_text_from_wire_string(s: &str) -> LocalizedText {
  if s.is_empty() {
    LocalizedText::new()
  } else {
    LocalizedText::from_src(SmolStr::from(s))
  }
}

fn subtitle_cue_error_as_buffa(e: SubtitleCueError) -> BuffaError {
  // Re-decode of previously-validated domain values rejecting an
  // invariant implies wire tampering. Map nil-id rejections to the
  // generic id-invalid variant; otherwise surface as a missing field.
  match e {
    SubtitleCueError::NilId => BuffaError::MissingRequiredField("SubtitleCue.id"),
    SubtitleCueError::NilSubtitleTrackId => {
      BuffaError::MissingRequiredField("SubtitleCue.subtitle_track_id")
    }
    SubtitleCueError::NilSubtitleCueId => {
      BuffaError::MissingRequiredField("LrcWord.subtitle_cue_id")
    }
    SubtitleCueError::EmptyAssStyleName => BuffaError::MissingRequiredField("AssStyle.name"),
    SubtitleCueError::UnimplementedFormat(k) => {
      BuffaError::UnimplementedSubtitleCueKind(i32::from(k.to_u8()))
    }
    SubtitleCueError::Other(_) => BuffaError::MissingRequiredField("SubtitleCue"),
  }
}

fn cue_kind_name(k: SubtitleCueKind) -> &'static str {
  match k {
    SubtitleCueKind::Srt => "Srt",
    SubtitleCueKind::Vtt => "Vtt",
    SubtitleCueKind::Ass => "Ass",
    SubtitleCueKind::MicroDvd => "MicroDvd",
    SubtitleCueKind::SubViewer => "SubViewer",
    SubtitleCueKind::Sbv => "Sbv",
    SubtitleCueKind::Lrc => "Lrc",
    SubtitleCueKind::Ttml => "Ttml",
    SubtitleCueKind::Sami => "Sami",
    SubtitleCueKind::VobSub => "VobSub",
    SubtitleCueKind::Pgs => "Pgs",
    SubtitleCueKind::Cea608 => "Cea608",
    SubtitleCueKind::EbuStl => "EbuStl",
  }
}

fn cue_kind_from_wire(
  v: &::buffa::EnumValue<wire::SubtitleCueKind>,
) -> Result<SubtitleCueKind, BuffaError> {
  match v {
    ::buffa::EnumValue::Known(k) => Ok(domain_cue_kind_from_wire(*k)),
    ::buffa::EnumValue::Unknown(n) => Err(BuffaError::UnknownEnumValue(*n)),
  }
}

fn domain_cue_kind_from_wire(k: wire::SubtitleCueKind) -> SubtitleCueKind {
  match k {
    wire::SubtitleCueKind::SUBTITLE_CUE_KIND_SRT => SubtitleCueKind::Srt,
    wire::SubtitleCueKind::SUBTITLE_CUE_KIND_VTT => SubtitleCueKind::Vtt,
    wire::SubtitleCueKind::SUBTITLE_CUE_KIND_ASS => SubtitleCueKind::Ass,
    wire::SubtitleCueKind::SUBTITLE_CUE_KIND_MICRO_DVD => SubtitleCueKind::MicroDvd,
    wire::SubtitleCueKind::SUBTITLE_CUE_KIND_SUB_VIEWER => SubtitleCueKind::SubViewer,
    wire::SubtitleCueKind::SUBTITLE_CUE_KIND_SBV => SubtitleCueKind::Sbv,
    wire::SubtitleCueKind::SUBTITLE_CUE_KIND_LRC => SubtitleCueKind::Lrc,
    wire::SubtitleCueKind::SUBTITLE_CUE_KIND_TTML => SubtitleCueKind::Ttml,
    wire::SubtitleCueKind::SUBTITLE_CUE_KIND_SAMI => SubtitleCueKind::Sami,
    wire::SubtitleCueKind::SUBTITLE_CUE_KIND_VOB_SUB => SubtitleCueKind::VobSub,
    wire::SubtitleCueKind::SUBTITLE_CUE_KIND_PGS => SubtitleCueKind::Pgs,
    wire::SubtitleCueKind::SUBTITLE_CUE_KIND_CEA_608 => SubtitleCueKind::Cea608,
    wire::SubtitleCueKind::SUBTITLE_CUE_KIND_EBU_STL => SubtitleCueKind::EbuStl,
  }
}

fn wire_cue_kind_from_domain(k: SubtitleCueKind) -> wire::SubtitleCueKind {
  match k {
    SubtitleCueKind::Srt => wire::SubtitleCueKind::SUBTITLE_CUE_KIND_SRT,
    SubtitleCueKind::Vtt => wire::SubtitleCueKind::SUBTITLE_CUE_KIND_VTT,
    SubtitleCueKind::Ass => wire::SubtitleCueKind::SUBTITLE_CUE_KIND_ASS,
    SubtitleCueKind::MicroDvd => wire::SubtitleCueKind::SUBTITLE_CUE_KIND_MICRO_DVD,
    SubtitleCueKind::SubViewer => wire::SubtitleCueKind::SUBTITLE_CUE_KIND_SUB_VIEWER,
    SubtitleCueKind::Sbv => wire::SubtitleCueKind::SUBTITLE_CUE_KIND_SBV,
    SubtitleCueKind::Lrc => wire::SubtitleCueKind::SUBTITLE_CUE_KIND_LRC,
    SubtitleCueKind::Ttml => wire::SubtitleCueKind::SUBTITLE_CUE_KIND_TTML,
    SubtitleCueKind::Sami => wire::SubtitleCueKind::SUBTITLE_CUE_KIND_SAMI,
    SubtitleCueKind::VobSub => wire::SubtitleCueKind::SUBTITLE_CUE_KIND_VOB_SUB,
    SubtitleCueKind::Pgs => wire::SubtitleCueKind::SUBTITLE_CUE_KIND_PGS,
    SubtitleCueKind::Cea608 => wire::SubtitleCueKind::SUBTITLE_CUE_KIND_CEA_608,
    SubtitleCueKind::EbuStl => wire::SubtitleCueKind::SUBTITLE_CUE_KIND_EBU_STL,
  }
}

// ---------------------------------------------------------------------------
// VTT cue-setting enums ⇄ wire enums
// ---------------------------------------------------------------------------

fn wire_vtt_vertical(v: VttVertical) -> wire::VttVertical {
  match v {
    VttVertical::Lr => wire::VttVertical::VTT_VERTICAL_LR,
    VttVertical::Rl => wire::VttVertical::VTT_VERTICAL_RL,
  }
}

fn domain_vtt_vertical(v: wire::VttVertical) -> VttVertical {
  match v {
    wire::VttVertical::VTT_VERTICAL_LR => VttVertical::Lr,
    wire::VttVertical::VTT_VERTICAL_RL => VttVertical::Rl,
  }
}

fn vtt_vertical_from_ev(
  v: &::buffa::EnumValue<wire::VttVertical>,
) -> Result<VttVertical, BuffaError> {
  match v {
    ::buffa::EnumValue::Known(k) => Ok(domain_vtt_vertical(*k)),
    ::buffa::EnumValue::Unknown(n) => Err(BuffaError::UnknownEnumValue(*n)),
  }
}

fn wire_vtt_line_align(v: VttLineAlign) -> wire::VttLineAlign {
  match v {
    VttLineAlign::Start => wire::VttLineAlign::VTT_LINE_ALIGN_START,
    VttLineAlign::Center => wire::VttLineAlign::VTT_LINE_ALIGN_CENTER,
    VttLineAlign::End => wire::VttLineAlign::VTT_LINE_ALIGN_END,
  }
}

fn domain_vtt_line_align(v: wire::VttLineAlign) -> VttLineAlign {
  match v {
    wire::VttLineAlign::VTT_LINE_ALIGN_START => VttLineAlign::Start,
    wire::VttLineAlign::VTT_LINE_ALIGN_CENTER => VttLineAlign::Center,
    wire::VttLineAlign::VTT_LINE_ALIGN_END => VttLineAlign::End,
  }
}

fn vtt_line_align_from_ev(
  v: &::buffa::EnumValue<wire::VttLineAlign>,
) -> Result<VttLineAlign, BuffaError> {
  match v {
    ::buffa::EnumValue::Known(k) => Ok(domain_vtt_line_align(*k)),
    ::buffa::EnumValue::Unknown(n) => Err(BuffaError::UnknownEnumValue(*n)),
  }
}

fn wire_vtt_position_align(v: VttPositionAlign) -> wire::VttPositionAlign {
  match v {
    VttPositionAlign::Start => wire::VttPositionAlign::VTT_POSITION_ALIGN_START,
    VttPositionAlign::Center => wire::VttPositionAlign::VTT_POSITION_ALIGN_CENTER,
    VttPositionAlign::End => wire::VttPositionAlign::VTT_POSITION_ALIGN_END,
    VttPositionAlign::LineLeft => wire::VttPositionAlign::VTT_POSITION_ALIGN_LINE_LEFT,
    VttPositionAlign::LineRight => wire::VttPositionAlign::VTT_POSITION_ALIGN_LINE_RIGHT,
  }
}

fn domain_vtt_position_align(v: wire::VttPositionAlign) -> VttPositionAlign {
  match v {
    wire::VttPositionAlign::VTT_POSITION_ALIGN_START => VttPositionAlign::Start,
    wire::VttPositionAlign::VTT_POSITION_ALIGN_CENTER => VttPositionAlign::Center,
    wire::VttPositionAlign::VTT_POSITION_ALIGN_END => VttPositionAlign::End,
    wire::VttPositionAlign::VTT_POSITION_ALIGN_LINE_LEFT => VttPositionAlign::LineLeft,
    wire::VttPositionAlign::VTT_POSITION_ALIGN_LINE_RIGHT => VttPositionAlign::LineRight,
  }
}

fn vtt_position_align_from_ev(
  v: &::buffa::EnumValue<wire::VttPositionAlign>,
) -> Result<VttPositionAlign, BuffaError> {
  match v {
    ::buffa::EnumValue::Known(k) => Ok(domain_vtt_position_align(*k)),
    ::buffa::EnumValue::Unknown(n) => Err(BuffaError::UnknownEnumValue(*n)),
  }
}

fn wire_vtt_text_align(v: VttTextAlign) -> wire::VttTextAlign {
  match v {
    VttTextAlign::Start => wire::VttTextAlign::VTT_TEXT_ALIGN_START,
    VttTextAlign::Center => wire::VttTextAlign::VTT_TEXT_ALIGN_CENTER,
    VttTextAlign::End => wire::VttTextAlign::VTT_TEXT_ALIGN_END,
    VttTextAlign::Left => wire::VttTextAlign::VTT_TEXT_ALIGN_LEFT,
    VttTextAlign::Right => wire::VttTextAlign::VTT_TEXT_ALIGN_RIGHT,
  }
}

fn domain_vtt_text_align(v: wire::VttTextAlign) -> VttTextAlign {
  match v {
    wire::VttTextAlign::VTT_TEXT_ALIGN_START => VttTextAlign::Start,
    wire::VttTextAlign::VTT_TEXT_ALIGN_CENTER => VttTextAlign::Center,
    wire::VttTextAlign::VTT_TEXT_ALIGN_END => VttTextAlign::End,
    wire::VttTextAlign::VTT_TEXT_ALIGN_LEFT => VttTextAlign::Left,
    wire::VttTextAlign::VTT_TEXT_ALIGN_RIGHT => VttTextAlign::Right,
  }
}

fn vtt_text_align_from_ev(
  v: &::buffa::EnumValue<wire::VttTextAlign>,
) -> Result<VttTextAlign, BuffaError> {
  match v {
    ::buffa::EnumValue::Known(k) => Ok(domain_vtt_text_align(*k)),
    ::buffa::EnumValue::Unknown(n) => Err(BuffaError::UnknownEnumValue(*n)),
  }
}

// ---------------------------------------------------------------------------
// SrtData ⇄ wire::SrtData
// ---------------------------------------------------------------------------

impl From<&SrtData> for wire::SrtData {
  fn from(_: &SrtData) -> Self {
    wire::SrtData {
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl From<&wire::SrtData> for SrtData {
  fn from(_: &wire::SrtData) -> Self {
    SrtData
  }
}

// ---------------------------------------------------------------------------
// VttData ⇄ wire::VttData
// ---------------------------------------------------------------------------

impl From<&VttData<Uuid7>> for wire::VttData {
  fn from(d: &VttData<Uuid7>) -> Self {
    wire::VttData {
      cue_identifier: d.cue_identifier().to_owned(),
      vertical: d
        .vertical()
        .map(|v| ::buffa::EnumValue::Known(wire_vtt_vertical(v))),
      line_value: d.line_value().to_owned(),
      line_align: d
        .line_align()
        .map(|v| ::buffa::EnumValue::Known(wire_vtt_line_align(v))),
      position_value: d.position_value().to_owned(),
      position_align: d
        .position_align()
        .map(|v| ::buffa::EnumValue::Known(wire_vtt_position_align(v))),
      size_value: d.size_value(),
      text_align: d
        .text_align()
        .map(|v| ::buffa::EnumValue::Known(wire_vtt_text_align(v))),
      region_id: opt_id_to_bytes(d.region_id_ref()),
      voice: d.voice().to_owned(),
      styled_text: d.styled_text().to_owned(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl TryFrom<&wire::VttData> for VttData<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::VttData) -> Result<Self, Self::Error> {
    let vertical = match w.vertical.as_ref() {
      Some(ev) => Some(vtt_vertical_from_ev(ev)?),
      None => None,
    };
    let line_align = match w.line_align.as_ref() {
      Some(ev) => Some(vtt_line_align_from_ev(ev)?),
      None => None,
    };
    let position_align = match w.position_align.as_ref() {
      Some(ev) => Some(vtt_position_align_from_ev(ev)?),
      None => None,
    };
    let text_align = match w.text_align.as_ref() {
      Some(ev) => Some(vtt_text_align_from_ev(ev)?),
      None => None,
    };
    let region_id = opt_id_from_bytes(&w.region_id)?;

    let mut d = VttData::<Uuid7>::new()
      .with_cue_identifier(w.cue_identifier.as_str())
      .maybe_vertical(vertical)
      .with_line_value(w.line_value.as_str())
      .maybe_line_align(line_align)
      .with_position_value(w.position_value.as_str())
      .maybe_position_align(position_align)
      .maybe_size_value(w.size_value)
      .maybe_text_align(text_align)
      .with_voice(w.voice.as_str())
      .with_styled_text(w.styled_text.as_str());
    if let Some(id) = region_id {
      d = d.with_region_id(id);
    }
    Ok(d)
  }
}

// ---------------------------------------------------------------------------
// AssData ⇄ wire::AssData
// ---------------------------------------------------------------------------

impl From<&AssData<Uuid7>> for wire::AssData {
  fn from(d: &AssData<Uuid7>) -> Self {
    wire::AssData {
      layer: d.layer(),
      style_id: id_to_bytes(d.style_id_ref()),
      name: d.name().to_owned(),
      margin_l: d.margin_l(),
      margin_r: d.margin_r(),
      margin_v: d.margin_v(),
      effect: d.effect().to_owned(),
      styled_text: d.styled_text().to_owned(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl TryFrom<&wire::AssData> for AssData<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::AssData) -> Result<Self, Self::Error> {
    let style_id = id_from_bytes(&w.style_id)?;
    Ok(
      AssData::<Uuid7>::new(style_id)
        .with_layer(w.layer)
        .with_name(w.name.as_str())
        .with_margin_l(w.margin_l)
        .with_margin_r(w.margin_r)
        .with_margin_v(w.margin_v)
        .with_effect(w.effect.as_str())
        .with_styled_text(w.styled_text.as_str()),
    )
  }
}

// ---------------------------------------------------------------------------
// LrcData ⇄ wire::LrcData
// ---------------------------------------------------------------------------

impl From<&LrcData> for wire::LrcData {
  fn from(d: &LrcData) -> Self {
    wire::LrcData {
      has_word_timing: d.has_word_timing(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl From<&wire::LrcData> for LrcData {
  fn from(w: &wire::LrcData) -> Self {
    LrcData::new().maybe_word_timing(w.has_word_timing)
  }
}

// ---------------------------------------------------------------------------
// SubtitleCue<Uuid7, D> -> wire::SubtitleCue (D-typed encode)
// ---------------------------------------------------------------------------

fn base_to_wire<D>(c: &SubtitleCue<Uuid7, D>, kind: SubtitleCueKind) -> wire::SubtitleCue {
  let span = c.span_ref();
  wire::SubtitleCue {
    id: id_to_bytes(c.id_ref()),
    subtitle_track_id: id_to_bytes(c.subtitle_track_id_ref()),
    ordinal: u64::from(c.ordinal()),
    span_start_pts: span.start_pts(),
    span_end_pts: span.end_pts(),
    text: localized_text_to_wire_string(c.text_ref()),
    kind: ::buffa::EnumValue::Known(wire_cue_kind_from_domain(kind)),
    data: None,
    __buffa_unknown_fields: Default::default(),
  }
}

impl From<&SrtCue<Uuid7>> for wire::SubtitleCue {
  fn from(c: &SrtCue<Uuid7>) -> Self {
    let mut w = base_to_wire(c, SubtitleCueKind::Srt);
    w.data = Some(wire::__buffa::oneof::subtitle_cue::Data::Srt(
      ::buffa::alloc::boxed::Box::new(wire::SrtData::from(c.data_ref())),
    ));
    w
  }
}

impl From<&VttCue<Uuid7>> for wire::SubtitleCue {
  fn from(c: &VttCue<Uuid7>) -> Self {
    let mut w = base_to_wire(c, SubtitleCueKind::Vtt);
    w.data = Some(wire::__buffa::oneof::subtitle_cue::Data::Vtt(
      ::buffa::alloc::boxed::Box::new(wire::VttData::from(c.data_ref())),
    ));
    w
  }
}

impl From<&AssCue<Uuid7>> for wire::SubtitleCue {
  fn from(c: &AssCue<Uuid7>) -> Self {
    let mut w = base_to_wire(c, SubtitleCueKind::Ass);
    w.data = Some(wire::__buffa::oneof::subtitle_cue::Data::Ass(
      ::buffa::alloc::boxed::Box::new(wire::AssData::from(c.data_ref())),
    ));
    w
  }
}

impl From<&LrcCue<Uuid7>> for wire::SubtitleCue {
  fn from(c: &LrcCue<Uuid7>) -> Self {
    let mut w = base_to_wire(c, SubtitleCueKind::Lrc);
    w.data = Some(wire::__buffa::oneof::subtitle_cue::Data::Lrc(
      ::buffa::alloc::boxed::Box::new(wire::LrcData::from(c.data_ref())),
    ));
    w
  }
}

impl From<&SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>>> for wire::SubtitleCue {
  fn from(c: &SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>>) -> Self {
    let kind = c.data_ref().kind();
    let mut w = base_to_wire(c, kind);
    w.data = Some(match c.data_ref() {
      SubtitleCueDetails::Srt(d) => wire::__buffa::oneof::subtitle_cue::Data::Srt(
        ::buffa::alloc::boxed::Box::new(wire::SrtData::from(d)),
      ),
      SubtitleCueDetails::Vtt(d) => wire::__buffa::oneof::subtitle_cue::Data::Vtt(
        ::buffa::alloc::boxed::Box::new(wire::VttData::from(d)),
      ),
      SubtitleCueDetails::Ass(d) => wire::__buffa::oneof::subtitle_cue::Data::Ass(
        ::buffa::alloc::boxed::Box::new(wire::AssData::from(d)),
      ),
      SubtitleCueDetails::Lrc(d) => wire::__buffa::oneof::subtitle_cue::Data::Lrc(
        ::buffa::alloc::boxed::Box::new(wire::LrcData::from(d)),
      ),
    });
    w
  }
}

// ---------------------------------------------------------------------------
// wire::SubtitleCue -> SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>>
// ---------------------------------------------------------------------------

/// Decode a polymorphic wire cue with the parent track's timebase.
///
/// The wire `SubtitleCue` carries PTS only — timebase lives on the
/// parent `SubtitleTrack` per the timebase-dedup rule. Pass the parent
/// track's timebase explicitly.
pub fn subtitle_cue_from_wire(
  w: &wire::SubtitleCue,
  parent_timebase: mediatime::Timebase,
) -> Result<SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>>, BuffaError> {
  let id = id_from_bytes(&w.id)?;
  let subtitle_track_id = id_from_bytes(&w.subtitle_track_id)?;
  let ordinal = u32::try_from(w.ordinal).map_err(|_| {
    BuffaError::SubtitleNumericOutOfRange(
      "SubtitleCue.ordinal",
      i32::try_from(w.ordinal).unwrap_or(i32::MAX),
    )
  })?;
  let span = mediatime::TimeRange::try_new(w.span_start_pts, w.span_end_pts, parent_timebase)
    .ok_or_else(|| BuffaError::MissingRequiredField("SubtitleCue.span"))?;
  let text = localized_text_from_wire_string(w.text.as_str());
  let kind = cue_kind_from_wire(&w.kind)?;

  let kind_name = cue_kind_name(kind);

  let details = match (kind, w.data.as_ref()) {
    (SubtitleCueKind::Srt, Some(wire::__buffa::oneof::subtitle_cue::Data::Srt(d))) => {
      SubtitleCueDetails::Srt(SrtData::from(d.as_ref()))
    }
    (SubtitleCueKind::Vtt, Some(wire::__buffa::oneof::subtitle_cue::Data::Vtt(d))) => {
      SubtitleCueDetails::Vtt(VttData::<Uuid7>::try_from(d.as_ref())?)
    }
    (SubtitleCueKind::Ass, Some(wire::__buffa::oneof::subtitle_cue::Data::Ass(d))) => {
      SubtitleCueDetails::Ass(AssData::<Uuid7>::try_from(d.as_ref())?)
    }
    (SubtitleCueKind::Lrc, Some(wire::__buffa::oneof::subtitle_cue::Data::Lrc(d))) => {
      SubtitleCueDetails::Lrc(LrcData::from(d.as_ref()))
    }
    // Implemented kind, but wrong oneof variant present.
    (
      SubtitleCueKind::Srt | SubtitleCueKind::Vtt | SubtitleCueKind::Ass | SubtitleCueKind::Lrc,
      Some(other),
    ) => {
      return Err(BuffaError::SubtitleCueKindOneofMismatch(
        kind_name,
        oneof_arm_name(other),
      ));
    }
    // Implemented kind, but no oneof set.
    (
      SubtitleCueKind::Srt | SubtitleCueKind::Vtt | SubtitleCueKind::Ass | SubtitleCueKind::Lrc,
      None,
    ) => {
      return Err(BuffaError::MissingSubtitleCueData(kind_name));
    }
    // Reserved discriminant (no domain payload type exists yet).
    (k, _) => {
      return Err(BuffaError::UnimplementedSubtitleCueKind(i32::from(
        k.to_u8(),
      )));
    }
  };

  SubtitleCue::try_new(id, subtitle_track_id, ordinal, span, text, details)
    .map_err(subtitle_cue_error_as_buffa)
}

/// Decode using a default timebase (`1/1000`). Convenience for the
/// common pure-millisecond-PTS case (e.g. when the parent track is
/// known to have been built with the default timebase).
pub fn subtitle_cue_from_wire_default_timebase(
  w: &wire::SubtitleCue,
) -> Result<SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>>, BuffaError> {
  subtitle_cue_from_wire(w, default_timebase())
}

fn default_timebase() -> mediatime::Timebase {
  // `1/1000` (ms ticks) is the ambient default used elsewhere
  // (mongodb tests, sqlx round-trip tests). Picked to keep the
  // convenience decoder useful for the common case without making
  // every caller construct a `Timebase` explicitly.
  mediatime::Timebase::new(1, core::num::NonZeroU32::new(1000).expect("nonzero"))
}

impl TryFrom<&wire::SubtitleCue> for SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>> {
  type Error = BuffaError;

  /// Decodes with the default `1/1000` timebase — use
  /// [`subtitle_cue_from_wire`] when the parent track's timebase
  /// differs.
  fn try_from(w: &wire::SubtitleCue) -> Result<Self, Self::Error> {
    subtitle_cue_from_wire_default_timebase(w)
  }
}

fn oneof_arm_name(d: &wire::__buffa::oneof::subtitle_cue::Data) -> &'static str {
  match d {
    wire::__buffa::oneof::subtitle_cue::Data::Srt(_) => "srt",
    wire::__buffa::oneof::subtitle_cue::Data::Vtt(_) => "vtt",
    wire::__buffa::oneof::subtitle_cue::Data::Ass(_) => "ass",
    wire::__buffa::oneof::subtitle_cue::Data::Lrc(_) => "lrc",
  }
}

// ---------------------------------------------------------------------------
// VttRegion ⇄ wire::VttRegion
// ---------------------------------------------------------------------------

impl From<&VttRegion<Uuid7>> for wire::VttRegion {
  fn from(r: &VttRegion<Uuid7>) -> Self {
    wire::VttRegion {
      id: id_to_bytes(r.id_ref()),
      subtitle_track_id: id_to_bytes(r.subtitle_track_id_ref()),
      name: r.name().to_owned(),
      width: r.width(),
      lines: r.lines(),
      region_anchor_x: r.region_anchor_x(),
      region_anchor_y: r.region_anchor_y(),
      viewport_anchor_x: r.viewport_anchor_x(),
      viewport_anchor_y: r.viewport_anchor_y(),
      scroll_up: r.scroll_up(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl TryFrom<&wire::VttRegion> for VttRegion<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::VttRegion) -> Result<Self, Self::Error> {
    let id = id_from_bytes(&w.id)?;
    let subtitle_track_id = id_from_bytes(&w.subtitle_track_id)?;
    Ok(
      VttRegion::try_new(id, subtitle_track_id, w.name.as_str())
        .map_err(subtitle_cue_error_as_buffa)?
        .with_width(w.width)
        .with_lines(w.lines)
        .with_region_anchor(w.region_anchor_x, w.region_anchor_y)
        .with_viewport_anchor(w.viewport_anchor_x, w.viewport_anchor_y)
        .maybe_scroll_up(w.scroll_up),
    )
  }
}

// ---------------------------------------------------------------------------
// VttStyleBlock ⇄ wire::VttStyleBlock
// ---------------------------------------------------------------------------

impl From<&VttStyleBlock<Uuid7>> for wire::VttStyleBlock {
  fn from(b: &VttStyleBlock<Uuid7>) -> Self {
    wire::VttStyleBlock {
      id: id_to_bytes(b.id_ref()),
      subtitle_track_id: id_to_bytes(b.subtitle_track_id_ref()),
      ordinal: b.ordinal(),
      css_text: b.css_text().to_owned(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl TryFrom<&wire::VttStyleBlock> for VttStyleBlock<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::VttStyleBlock) -> Result<Self, Self::Error> {
    let id = id_from_bytes(&w.id)?;
    let subtitle_track_id = id_from_bytes(&w.subtitle_track_id)?;
    VttStyleBlock::try_new(id, subtitle_track_id, w.ordinal, w.css_text.as_str())
      .map_err(subtitle_cue_error_as_buffa)
  }
}

// ---------------------------------------------------------------------------
// AssStyle ⇄ wire::AssStyle
// ---------------------------------------------------------------------------

impl From<&AssStyle<Uuid7>> for wire::AssStyle {
  fn from(s: &AssStyle<Uuid7>) -> Self {
    wire::AssStyle {
      id: id_to_bytes(s.id_ref()),
      subtitle_track_id: id_to_bytes(s.subtitle_track_id_ref()),
      name: s.name().to_owned(),
      fontname: s.fontname().to_owned(),
      fontsize: s.fontsize(),
      primary_colour: s.primary_colour(),
      secondary_colour: s.secondary_colour(),
      outline_colour: s.outline_colour(),
      back_colour: s.back_colour(),
      bold: s.bold(),
      italic: s.italic(),
      underline: s.underline(),
      strikeout: s.strikeout(),
      scale_x: s.scale_x(),
      scale_y: s.scale_y(),
      spacing: s.spacing(),
      angle: s.angle(),
      border_style: i32::from(s.border_style()),
      outline: s.outline(),
      shadow: s.shadow(),
      alignment: i32::from(s.alignment()),
      margin_l: s.margin_l(),
      margin_r: s.margin_r(),
      margin_v: s.margin_v(),
      encoding: s.encoding(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl TryFrom<&wire::AssStyle> for AssStyle<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::AssStyle) -> Result<Self, Self::Error> {
    let id = id_from_bytes(&w.id)?;
    let subtitle_track_id = id_from_bytes(&w.subtitle_track_id)?;
    let border_style = i16::try_from(w.border_style).map_err(|_| {
      BuffaError::SubtitleNumericOutOfRange("AssStyle.border_style", w.border_style)
    })?;
    let alignment = i16::try_from(w.alignment)
      .map_err(|_| BuffaError::SubtitleNumericOutOfRange("AssStyle.alignment", w.alignment))?;

    Ok(
      AssStyle::try_new(id, subtitle_track_id, w.name.as_str())
        .map_err(subtitle_cue_error_as_buffa)?
        .with_fontname(w.fontname.as_str())
        .with_fontsize(w.fontsize)
        .with_primary_colour(w.primary_colour)
        .with_secondary_colour(w.secondary_colour)
        .with_outline_colour(w.outline_colour)
        .with_back_colour(w.back_colour)
        .maybe_bold(w.bold)
        .maybe_italic(w.italic)
        .maybe_underline(w.underline)
        .maybe_strikeout(w.strikeout)
        .with_scale_x(w.scale_x)
        .with_scale_y(w.scale_y)
        .with_spacing(w.spacing)
        .with_angle(w.angle)
        .with_border_style(border_style)
        .with_outline(w.outline)
        .with_shadow(w.shadow)
        .with_alignment(alignment)
        .with_margin_l(w.margin_l)
        .with_margin_r(w.margin_r)
        .with_margin_v(w.margin_v)
        .with_encoding(w.encoding),
    )
  }
}

// ---------------------------------------------------------------------------
// LrcMetadata ⇄ wire::LrcMetadata
// ---------------------------------------------------------------------------

impl From<&LrcMetadata<Uuid7>> for wire::LrcMetadata {
  fn from(m: &LrcMetadata<Uuid7>) -> Self {
    wire::LrcMetadata {
      subtitle_track_id: id_to_bytes(m.subtitle_track_id_ref()),
      title: m.title().to_owned(),
      artist: m.artist().to_owned(),
      album: m.album().to_owned(),
      author: m.author().to_owned(),
      creator: m.creator().to_owned(),
      length: m.length().to_owned(),
      offset_ms: m.offset_ms(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl TryFrom<&wire::LrcMetadata> for LrcMetadata<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::LrcMetadata) -> Result<Self, Self::Error> {
    let subtitle_track_id = id_from_bytes(&w.subtitle_track_id)?;
    Ok(
      LrcMetadata::try_new(subtitle_track_id)
        .map_err(subtitle_cue_error_as_buffa)?
        .with_title(w.title.as_str())
        .with_artist(w.artist.as_str())
        .with_album(w.album.as_str())
        .with_author(w.author.as_str())
        .with_creator(w.creator.as_str())
        .with_length(w.length.as_str())
        .with_offset_ms(w.offset_ms),
    )
  }
}

// ---------------------------------------------------------------------------
// LrcWord ⇄ wire::LrcWord
// ---------------------------------------------------------------------------

impl From<&LrcWord<Uuid7>> for wire::LrcWord {
  fn from(w: &LrcWord<Uuid7>) -> Self {
    wire::LrcWord {
      subtitle_cue_id: id_to_bytes(w.subtitle_cue_id_ref()),
      ordinal: w.ordinal(),
      text: w.text().to_owned(),
      start_pts: w.start_pts(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

impl TryFrom<&wire::LrcWord> for LrcWord<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::LrcWord) -> Result<Self, Self::Error> {
    let cue_id = id_from_bytes(&w.subtitle_cue_id)?;
    LrcWord::try_new(cue_id, w.ordinal, w.text.as_str(), w.start_pts)
      .map_err(subtitle_cue_error_as_buffa)
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use core::num::NonZeroU32;
  use mediatime::{TimeRange, Timebase};

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).expect("nonzero"))
  }

  fn span(start: i64, end: i64) -> TimeRange {
    TimeRange::new(start, end, tb())
  }

  // ---- SubtitleCueKind ⇄ wire enum ------------------------------------------

  #[test]
  fn cue_kind_round_trips_all_discriminants() {
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
      let w = wire_cue_kind_from_domain(k);
      assert_eq!(domain_cue_kind_from_wire(w), k);
    }
  }

  // ---- SRT cue ---------------------------------------------------------------

  #[test]
  fn srt_cue_roundtrip() {
    let c = SrtCue::try_new_srt(
      Uuid7::new(),
      Uuid7::new(),
      3,
      span(1_000, 2_500),
      LocalizedText::from_src("Hello world"),
    )
    .unwrap();

    let w: wire::SubtitleCue = (&c).into();
    let c2 = subtitle_cue_from_wire(&w, tb()).unwrap();

    assert_eq!(c2.id_ref(), c.id_ref());
    assert_eq!(c2.subtitle_track_id_ref(), c.subtitle_track_id_ref());
    assert_eq!(c2.ordinal(), 3);
    assert_eq!(c2.span_ref().start_pts(), 1_000);
    assert_eq!(c2.span_ref().end_pts(), 2_500);
    assert_eq!(c2.text_ref().src(), "Hello world");
    assert_eq!(c2.data_ref().kind(), SubtitleCueKind::Srt);
    assert!(c2.data_ref().is_srt());
  }

  // ---- VTT cue ---------------------------------------------------------------

  #[test]
  fn vtt_cue_roundtrip_populated() {
    let region_id = Uuid7::new();
    let d = VttData::<Uuid7>::new()
      .with_cue_identifier("c1")
      .with_vertical(VttVertical::Rl)
      .with_line_value("50%")
      .with_line_align(VttLineAlign::Center)
      .with_position_value("25%")
      .with_position_align(VttPositionAlign::LineLeft)
      .with_size_value(80.0)
      .with_text_align(VttTextAlign::Start)
      .with_region_id(region_id)
      .with_voice("Alice")
      .with_styled_text("<b>hi</b>");
    let c: VttCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 1_000),
      LocalizedText::from_src("hi"),
      d,
    )
    .unwrap();

    let w: wire::SubtitleCue = (&c).into();
    let c2 = subtitle_cue_from_wire(&w, tb()).unwrap();
    assert_eq!(c2.data_ref().kind(), SubtitleCueKind::Vtt);

    let dd = match c2.data_ref() {
      SubtitleCueDetails::Vtt(v) => v,
      _ => panic!("expected Vtt data"),
    };
    assert_eq!(dd.cue_identifier(), "c1");
    assert_eq!(dd.vertical(), Some(VttVertical::Rl));
    assert_eq!(dd.line_align(), Some(VttLineAlign::Center));
    assert_eq!(dd.position_align(), Some(VttPositionAlign::LineLeft));
    assert_eq!(dd.size_value(), Some(80.0));
    assert_eq!(dd.text_align(), Some(VttTextAlign::Start));
    assert_eq!(dd.region_id_ref(), Some(&region_id));
    assert_eq!(dd.voice(), "Alice");
    assert_eq!(dd.styled_text(), "<b>hi</b>");
  }

  // ---- ASS cue ---------------------------------------------------------------

  #[test]
  fn ass_cue_roundtrip() {
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
      7,
      span(500, 1_500),
      LocalizedText::new(),
      d,
    )
    .unwrap();

    let w: wire::SubtitleCue = (&c).into();
    let c2 = subtitle_cue_from_wire(&w, tb()).unwrap();
    let dd = match c2.data_ref() {
      SubtitleCueDetails::Ass(v) => v,
      _ => panic!("expected Ass data"),
    };
    assert_eq!(dd.style_id_ref(), &style_id);
    assert_eq!(dd.layer(), 2);
    assert_eq!(dd.name(), "Alice");
    assert_eq!(dd.margin_l(), 10);
    assert_eq!(dd.margin_r(), 20);
    assert_eq!(dd.margin_v(), 30);
    assert_eq!(dd.effect(), "karaoke");
    assert_eq!(dd.styled_text(), "{\\b1}hi{\\b0}");
  }

  // ---- LRC cue ---------------------------------------------------------------

  #[test]
  fn lrc_cue_roundtrip_with_word_timing() {
    let c: LrcCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      4,
      span(0, 1_200),
      LocalizedText::from_src("la la la"),
      LrcData::new().with_word_timing(),
    )
    .unwrap();
    let w: wire::SubtitleCue = (&c).into();
    let c2 = subtitle_cue_from_wire(&w, tb()).unwrap();
    let dd = match c2.data_ref() {
      SubtitleCueDetails::Lrc(v) => v,
      _ => panic!("expected Lrc data"),
    };
    assert!(dd.has_word_timing());
  }

  // ---- VttRegion ------------------------------------------------------------

  #[test]
  fn vtt_region_roundtrip() {
    let r = VttRegion::try_new(Uuid7::new(), Uuid7::new(), "speakers")
      .unwrap()
      .with_width(72.5)
      .with_lines(3)
      .with_region_anchor(50.0, 100.0)
      .with_viewport_anchor(50.0, 90.0)
      .with_scroll_up();
    let w: wire::VttRegion = (&r).into();
    let r2 = VttRegion::try_from(&w).unwrap();
    assert_eq!(r, r2);
  }

  // ---- VttStyleBlock --------------------------------------------------------

  #[test]
  fn vtt_style_block_roundtrip() {
    let b = VttStyleBlock::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      "::cue { color: red; }",
    )
    .unwrap();
    let w: wire::VttStyleBlock = (&b).into();
    let b2 = VttStyleBlock::try_from(&w).unwrap();
    assert_eq!(b, b2);
  }

  // ---- AssStyle -------------------------------------------------------------

  #[test]
  fn ass_style_roundtrip() {
    let s = AssStyle::try_new(Uuid7::new(), Uuid7::new(), "Default")
      .unwrap()
      .with_fontname("Arial")
      .with_fontsize(20.0)
      .with_primary_colour(0x00FFFFFF)
      .with_bold()
      .with_alignment(2)
      .with_border_style(1)
      .with_outline(2.0)
      .with_shadow(0.0)
      .with_margin_l(10)
      .with_margin_r(10)
      .with_margin_v(10)
      .with_scale_x(100)
      .with_scale_y(100)
      .with_encoding(1);
    let w: wire::AssStyle = (&s).into();
    let s2 = AssStyle::try_from(&w).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn ass_style_border_style_out_of_range_errors() {
    let s = AssStyle::try_new(Uuid7::new(), Uuid7::new(), "Default").unwrap();
    let mut w: wire::AssStyle = (&s).into();
    w.border_style = i32::MAX;
    let e = AssStyle::try_from(&w).unwrap_err();
    assert!(matches!(
      e,
      BuffaError::SubtitleNumericOutOfRange("AssStyle.border_style", _)
    ));
  }

  // ---- LrcMetadata ----------------------------------------------------------

  #[test]
  fn lrc_metadata_roundtrip() {
    let m = LrcMetadata::try_new(Uuid7::new())
      .unwrap()
      .with_title("Song")
      .with_artist("Band")
      .with_album("Album")
      .with_author("Author")
      .with_creator("Creator")
      .with_length("3:30")
      .with_offset_ms(-500);
    let w: wire::LrcMetadata = (&m).into();
    let m2 = LrcMetadata::try_from(&w).unwrap();
    assert_eq!(m, m2);
  }

  // ---- LrcWord --------------------------------------------------------------

  #[test]
  fn lrc_word_roundtrip() {
    let lw = LrcWord::try_new(Uuid7::new(), 2, "la", 500).unwrap();
    let w: wire::LrcWord = (&lw).into();
    let lw2 = LrcWord::try_from(&w).unwrap();
    assert_eq!(lw, lw2);
  }

  // ---- Error cases ----------------------------------------------------------

  #[test]
  fn cue_kind_oneof_mismatch_errors() {
    // Build a VTT cue's wire frame, then swap the data oneof for an
    // SRT payload while leaving the kind = VTT.
    let d = VttData::<Uuid7>::new().with_styled_text("x");
    let c: VttCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 1),
      LocalizedText::new(),
      d,
    )
    .unwrap();
    let mut w: wire::SubtitleCue = (&c).into();
    w.data = Some(wire::__buffa::oneof::subtitle_cue::Data::Srt(
      ::buffa::alloc::boxed::Box::new(wire::SrtData::default()),
    ));
    let e = subtitle_cue_from_wire(&w, tb()).unwrap_err();
    assert!(matches!(
      e,
      BuffaError::SubtitleCueKindOneofMismatch("Vtt", "srt")
    ));
  }

  #[test]
  fn missing_oneof_for_implemented_kind_errors() {
    let c = SrtCue::try_new_srt(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 1),
      LocalizedText::new(),
    )
    .unwrap();
    let mut w: wire::SubtitleCue = (&c).into();
    w.data = None;
    let e = subtitle_cue_from_wire(&w, tb()).unwrap_err();
    assert!(matches!(
      e,
      BuffaError::MissingSubtitleCueData("Srt")
    ));
  }

  #[test]
  fn reserved_kind_is_unimplemented() {
    // Construct a synthetic wire frame whose kind is one of the
    // reserved discriminants. The decoder must surface the
    // `UnimplementedSubtitleCueKind` variant rather than try to
    // dispatch on a missing data type.
    let c = SrtCue::try_new_srt(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 1),
      LocalizedText::new(),
    )
    .unwrap();
    let mut w: wire::SubtitleCue = (&c).into();
    w.kind = ::buffa::EnumValue::Known(wire::SubtitleCueKind::SUBTITLE_CUE_KIND_TTML);
    w.data = None;
    let e = subtitle_cue_from_wire(&w, tb()).unwrap_err();
    assert!(matches!(
      e,
      BuffaError::UnimplementedSubtitleCueKind(7)
    ));
  }

  #[test]
  fn details_polymorphic_encode_dispatches_on_variant() {
    let style_id = Uuid7::new();
    let inner = AssData::<Uuid7>::new(style_id).with_name("X");
    let c: SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 100),
      LocalizedText::new(),
      SubtitleCueDetails::Ass(inner),
    )
    .unwrap();
    let w: wire::SubtitleCue = (&c).into();
    assert!(matches!(
      w.kind,
      ::buffa::EnumValue::Known(wire::SubtitleCueKind::SUBTITLE_CUE_KIND_ASS)
    ));
    assert!(matches!(
      w.data,
      Some(wire::__buffa::oneof::subtitle_cue::Data::Ass(_))
    ));
  }

  #[test]
  fn try_from_default_timebase_decodes_srt() {
    let c = SrtCue::try_new_srt(
      Uuid7::new(),
      Uuid7::new(),
      0,
      span(0, 200),
      LocalizedText::from_src("hi"),
    )
    .unwrap();
    let w: wire::SubtitleCue = (&c).into();
    // `TryFrom<&wire::SubtitleCue>` uses the default 1/1000 timebase
    // — same as the `tb()` helper.
    let c2 = SubtitleCue::<Uuid7, SubtitleCueDetails<Uuid7>>::try_from(&w).unwrap();
    assert!(c2.data_ref().is_srt());
    assert_eq!(c2.span_ref().start_pts(), 0);
    assert_eq!(c2.span_ref().end_pts(), 200);
  }
}
