//! MySQL row shapes for the subtitle-cluster aggregates: the `Subtitle`
//! facet, `SubtitleTrack`, and `SubtitleCue`.
//!
//! UUIDs ride as `BINARY(16)` (`Vec<u8>`). Nested value-objects are
//! flattened into real columns; `Option<VO>` rides as a discriminating
//! column plus all-NULL payload columns when absent. Open descriptor
//! enums (`SubtitleCodec`, `Format`) ride as `text` slugs; the closed
//! coded enum (`TrackOrigin`) and bitflags (`SubtitleIndexStatus` /
//! `TrackDisposition`) ride as integers. `Language` flattens to a BCP-47
//! `text` column. Media-time values flatten to a PTS `BIGINT` + timebase
//! num/den.
//!
//! Collections ride in a child table (`subtitle_track_index_error`) with
//! an `ordinal` order column. The `Vec<Id>` reverse-FK fields
//! (`Subtitle::tracks`, `SubtitleTrack::cues`) are NOT stored.

use mediaframe::{
  codec::SubtitleCodec,
  disposition::TrackDisposition,
  lang::Language,
  subtitle::{Format, TrackOrigin},
};

use crate::{
  domain::{
    aggregates::subtitle::{
      AssCue, AssData, AssStyle, LrcCue, LrcData, LrcMetadata, LrcWord, SrtCue, SrtData,
      SubtitleCueError, SubtitleCueKind, SubtitleError, SubtitleTrackError, VttCue, VttData,
      VttLineAlign, VttPositionAlign, VttRegion, VttStyleBlock, VttTextAlign, VttVertical,
    },
    primitives::ErrorInfo,
    vo::{IndexProgress, LocalizedText, Provenance},
    ErrorCode, Subtitle, SubtitleCue, SubtitleIndexStatus, SubtitleKind, SubtitleTrack, Uuid7,
  },
  sqlx::{
    dto::{bytes_to_checksum, bytes_to_uuid7, timestamp_from_parts},
    SqlxError,
  },
};

// ---------------------------------------------------------------------------
// SubtitleKind — closed enum, rides as a small integer
// ---------------------------------------------------------------------------

fn kind_to_i16(k: SubtitleKind) -> i16 {
  match k {
    SubtitleKind::FullDialogue => 0,
    SubtitleKind::ForcedNarrative => 1,
    SubtitleKind::CommentaryText => 2,
  }
}

fn kind_from_i16(n: i16) -> Result<SubtitleKind, SqlxError> {
  match n {
    0 => Ok(SubtitleKind::FullDialogue),
    1 => Ok(SubtitleKind::ForcedNarrative),
    2 => Ok(SubtitleKind::CommentaryText),
    other => Err(SqlxError::UnknownDiscriminant(format!(
      "SubtitleKind: {other}"
    ))),
  }
}

// ===========================================================================
// Subtitle facet
// ===========================================================================

/// MySQL row shape for the [`Subtitle`] facet.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleRow {
  pub id: std::vec::Vec<u8>,
  pub media_id: std::vec::Vec<u8>,
  pub track_progress_total: i64,
  pub track_progress_indexed: i64,
  pub track_progress_failed: i64,
}

impl From<&Subtitle<Uuid7>> for MySqlSubtitleRow {
  fn from(s: &Subtitle<Uuid7>) -> Self {
    let p = s.track_progress_ref();
    Self {
      id: s.id_ref().as_bytes().to_vec(),
      media_id: s.media_id_ref().as_bytes().to_vec(),
      track_progress_total: i64::from(p.total()),
      track_progress_indexed: i64::from(p.indexed()),
      track_progress_failed: i64::from(p.failed()),
    }
  }
}

impl TryFrom<MySqlSubtitleRow> for Subtitle<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: MySqlSubtitleRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let media_id = bytes_to_uuid7(&r.media_id)?;
    let progress = IndexProgress::from_parts(
      u32_from_i64(r.track_progress_total, "Subtitle.track_progress_total")?,
      u32_from_i64(r.track_progress_indexed, "Subtitle.track_progress_indexed")?,
      u32_from_i64(r.track_progress_failed, "Subtitle.track_progress_failed")?,
    );
    let s = Subtitle::try_new(id, media_id)
      .map_err(|e: SubtitleError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    Ok(s.with_track_progress(progress))
  }
}

// ===========================================================================
// SubtitleTrack
// ===========================================================================

/// MySQL row shape for [`SubtitleTrack`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct MySqlSubtitleTrackRow {
  pub id: std::vec::Vec<u8>,
  pub subtitle_id: std::vec::Vec<u8>,
  pub stream_index: Option<i64>,
  pub container_track_id: Option<i64>,
  pub codec: String,
  pub format: String,
  pub origin: i32,
  pub language: Option<String>,
  pub title: String,
  pub disposition: i64,
  pub is_primary: bool,
  pub auto_selected: bool,
  pub duration_pts: Option<i64>,
  pub duration_tb_num: Option<i64>,
  pub duration_tb_den: Option<i64>,
  pub cue_count: i64,
  pub provenance_model_name: String,
  pub provenance_model_version: String,
  pub provenance_prompt_version: String,
  pub provenance_indexer_version: String,
  pub source_checksum: Option<std::vec::Vec<u8>>,
  pub character_encoding: String,
  pub bom_present: bool,
  pub is_sdh: bool,
  pub is_closed_caption: bool,
  pub is_translation: bool,
  pub kind: i16,
  pub coverage_ratio: Option<f32>,
  pub is_empty: bool,
  pub first_cue_pts: Option<i64>,
  pub first_cue_tb_num: Option<i64>,
  pub first_cue_tb_den: Option<i64>,
  pub last_cue_pts: Option<i64>,
  pub last_cue_tb_num: Option<i64>,
  pub last_cue_tb_den: Option<i64>,
  pub index_status: i64,
}

/// One `subtitle_track_index_error` child row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleTrackIndexErrorRow {
  pub subtitle_track_id: std::vec::Vec<u8>,
  pub ordinal: i32,
  pub code: i32,
  pub message: String,
}

impl From<&SubtitleTrack<Uuid7>>
  for (
    MySqlSubtitleTrackRow,
    std::vec::Vec<MySqlSubtitleTrackIndexErrorRow>,
  )
{
  fn from(t: &SubtitleTrack<Uuid7>) -> Self {
    let id = t.id_ref().as_bytes().to_vec();
    let prov = t.provenance_ref();
    let duration = t.duration_ref();
    let first_cue = t.first_cue_ref();
    let last_cue = t.last_cue_ref();
    let row = MySqlSubtitleTrackRow {
      id: id.clone(),
      subtitle_id: t.subtitle_id_ref().as_bytes().to_vec(),
      stream_index: t.stream_index().map(i64::from),
      container_track_id: t.container_track_id().map(|v| v as i64),
      codec: t.codec_ref().as_str().to_owned(),
      format: t.format_ref().as_str().to_owned(),
      origin: t.origin_ref().to_u32() as i32,
      language: language_to_slug(t.language_ref()),
      title: t.title().to_owned(),
      disposition: i64::from(t.disposition().bits()),
      is_primary: t.is_primary(),
      auto_selected: t.auto_selected(),
      duration_pts: duration.map(mediatime::Timestamp::pts),
      duration_tb_num: duration.map(|d| i64::from(d.timebase().num())),
      duration_tb_den: duration.map(|d| i64::from(d.timebase().den().get())),
      cue_count: i64::from(t.cue_count()),
      provenance_model_name: prov.model_name().to_owned(),
      provenance_model_version: prov.model_version().to_owned(),
      provenance_prompt_version: prov.prompt_version().to_owned(),
      provenance_indexer_version: prov.indexer_version().to_owned(),
      source_checksum: t.source_checksum_ref().map(|c| c.as_bytes().to_vec()),
      character_encoding: t.character_encoding().to_owned(),
      bom_present: t.bom_present(),
      is_sdh: t.is_sdh(),
      is_closed_caption: t.is_closed_caption(),
      is_translation: t.is_translation(),
      kind: kind_to_i16(t.kind()),
      coverage_ratio: t.coverage_ratio(),
      is_empty: t.is_empty(),
      first_cue_pts: first_cue.map(mediatime::Timestamp::pts),
      first_cue_tb_num: first_cue.map(|d| i64::from(d.timebase().num())),
      first_cue_tb_den: first_cue.map(|d| i64::from(d.timebase().den().get())),
      last_cue_pts: last_cue.map(mediatime::Timestamp::pts),
      last_cue_tb_num: last_cue.map(|d| i64::from(d.timebase().num())),
      last_cue_tb_den: last_cue.map(|d| i64::from(d.timebase().den().get())),
      index_status: i64::from(t.index_status().bits()),
    };
    let errors = t
      .index_errors_slice()
      .iter()
      .enumerate()
      .map(|(i, e)| MySqlSubtitleTrackIndexErrorRow {
        subtitle_track_id: id.clone(),
        ordinal: i as i32,
        code: e.code().as_u32() as i32,
        message: e.message().to_owned(),
      })
      .collect();
    (row, errors)
  }
}

impl
  TryFrom<(
    MySqlSubtitleTrackRow,
    std::vec::Vec<MySqlSubtitleTrackIndexErrorRow>,
  )> for SubtitleTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, mut errors): (
      MySqlSubtitleTrackRow,
      std::vec::Vec<MySqlSubtitleTrackIndexErrorRow>,
    ),
  ) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let subtitle_id = bytes_to_uuid7(&r.subtitle_id)?;
    let mut t = SubtitleTrack::try_new(id, subtitle_id)
      .map_err(|e: SubtitleTrackError| SqlxError::DomainConstructorRejected(e.to_string()))?;

    t = t
      .with_codec(parse_subtitle_codec(&r.codec))
      .with_format(parse_subtitle_format(&r.format))
      .with_origin(
        TrackOrigin::try_from_u32(u32_from_i32(r.origin, "SubtitleTrack.origin")?)
          .ok_or_else(|| SqlxError::UnknownDiscriminant(format!("TrackOrigin: {}", r.origin)))?,
      )
      .with_title(r.title)
      .with_disposition(TrackDisposition::from_bits_truncate(u32_from_i64(
        r.disposition,
        "SubtitleTrack.disposition",
      )?))
      .with_primary(r.is_primary)
      .with_auto_selected(r.auto_selected)
      .with_cue_count(u32_from_i64(r.cue_count, "SubtitleTrack.cue_count")?)
      .with_stream_index(opt_u32(r.stream_index, "SubtitleTrack.stream_index")?)
      .with_container_track_id(r.container_track_id.map(|v| v as u64))
      .with_character_encoding(r.character_encoding)
      .with_bom_present(r.bom_present)
      .with_sdh(r.is_sdh)
      .with_closed_caption(r.is_closed_caption)
      .with_translation(r.is_translation)
      .with_kind(kind_from_i16(r.kind)?)
      .with_coverage_ratio(r.coverage_ratio)
      .with_empty(r.is_empty);

    if let Some(s) = r.language {
      t = t.with_language(parse_language(&s)?);
    }

    if let Some(pts) = r.duration_pts {
      let (num, den) = require_timebase(
        r.duration_tb_num,
        r.duration_tb_den,
        "SubtitleTrack.duration",
      )?;
      t = t.with_duration(Some(timestamp_from_parts(pts, num, den)?));
    }
    if let Some(pts) = r.first_cue_pts {
      let (num, den) = require_timebase(
        r.first_cue_tb_num,
        r.first_cue_tb_den,
        "SubtitleTrack.first_cue",
      )?;
      t = t.with_first_cue(Some(timestamp_from_parts(pts, num, den)?));
    }
    if let Some(pts) = r.last_cue_pts {
      let (num, den) = require_timebase(
        r.last_cue_tb_num,
        r.last_cue_tb_den,
        "SubtitleTrack.last_cue",
      )?;
      t = t.with_last_cue(Some(timestamp_from_parts(pts, num, den)?));
    }

    if let Some(bytes) = r.source_checksum {
      t = t.with_source_checksum(Some(bytes_to_checksum(&bytes)?));
    }

    t = t.with_provenance(Provenance::from_parts(
      r.provenance_model_name,
      r.provenance_model_version,
      r.provenance_prompt_version,
      r.provenance_indexer_version,
    ));

    let status = SubtitleIndexStatus::from_bits_truncate(u32_from_i64(
      r.index_status,
      "SubtitleTrack.index_status",
    )?);
    t = t.with_index_status(status);

    errors.sort_by_key(|e| e.ordinal);
    let mut infos = std::vec::Vec::with_capacity(errors.len());
    for e in errors {
      let code = u32_from_i32(e.code, "SubtitleTrack.index_error.code")?;
      infos.push(ErrorInfo::new(ErrorCode::from_u32(code), e.message));
    }
    t = t.with_index_errors(infos);

    Ok(t)
  }
}

// ===========================================================================
// SubtitleCue — polymorphic base + per-format detail + per-track aggregates
// ===========================================================================

/// MySQL row shape for the base [`SubtitleCue`] table.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleCueBaseRow {
  pub id: std::vec::Vec<u8>,
  pub subtitle_track_id: std::vec::Vec<u8>,
  pub ordinal: i64,
  pub span_start_pts: i64,
  pub span_end_pts: i64,
  pub text_src: String,
  pub text_translated: String,
  pub kind: i64,
}

fn cue_kind_to_i64(k: SubtitleCueKind) -> i64 {
  i64::from(k.to_u8())
}

fn cue_kind_from_i64_v(n: i64) -> Result<SubtitleCueKind, SqlxError> {
  u8::try_from(n)
    .ok()
    .and_then(SubtitleCueKind::try_from_u8)
    .ok_or_else(|| SqlxError::UnknownDiscriminant(format!("SubtitleCueKind: {n}")))
}

fn base_row_from_cue<D>(
  c: &SubtitleCue<Uuid7, D>,
  kind: SubtitleCueKind,
) -> MySqlSubtitleCueBaseRow {
  let span = c.span_ref();
  MySqlSubtitleCueBaseRow {
    id: c.id_ref().as_bytes().to_vec(),
    subtitle_track_id: c.subtitle_track_id_ref().as_bytes().to_vec(),
    ordinal: i64::from(c.ordinal()),
    span_start_pts: span.start_pts(),
    span_end_pts: span.end_pts(),
    text_src: c.text_ref().src().to_owned(),
    text_translated: c.text_ref().translated().to_owned(),
    kind: cue_kind_to_i64(kind),
  }
}

fn base_row_to_parts(
  r: &MySqlSubtitleCueBaseRow,
  parent_timebase: mediatime::Timebase,
) -> Result<
  (
    Uuid7,
    Uuid7,
    u32,
    mediatime::TimeRange,
    LocalizedText,
    SubtitleCueKind,
  ),
  SqlxError,
> {
  let id = bytes_to_uuid7(&r.id)?;
  let subtitle_track_id = bytes_to_uuid7(&r.subtitle_track_id)?;
  let ordinal = u32_from_i64(r.ordinal, "SubtitleCue.ordinal")?;
  let span =
    mediatime::TimeRange::try_new(r.span_start_pts, r.span_end_pts, parent_timebase).ok_or_else(
      || {
        SqlxError::DomainConstructorRejected(format!(
          "TimeRange start_pts ({}) must be <= end_pts ({})",
          r.span_start_pts, r.span_end_pts
        ))
      },
    )?;
  let text = LocalizedText::from_src_translated(r.text_src.clone(), r.text_translated.clone());
  let kind = cue_kind_from_i64_v(r.kind)?;
  Ok((id, subtitle_track_id, ordinal, span, text, kind))
}

// --- SRT ---------------------------------------------------------------------

impl From<&SrtCue<Uuid7>> for MySqlSubtitleCueBaseRow {
  fn from(c: &SrtCue<Uuid7>) -> Self {
    base_row_from_cue(c, SubtitleCueKind::Srt)
  }
}

/// Rebuild a SubRip cue from its base row.
pub fn srt_cue_from_row(
  base: MySqlSubtitleCueBaseRow,
  parent_timebase: mediatime::Timebase,
) -> Result<SrtCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) = base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Srt {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Srt cue kind, got {kind:?}"
    )));
  }
  SrtCue::try_new(id, subtitle_track_id, ordinal, span, text, SrtData)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- WebVTT ------------------------------------------------------------------

/// MySQL detail row for a WebVTT cue.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct MySqlSubtitleCueVttRow {
  pub id: std::vec::Vec<u8>,
  pub cue_identifier: String,
  pub vertical: Option<i64>,
  pub line_value: String,
  pub line_align: Option<i64>,
  pub position_value: String,
  pub position_align: Option<i64>,
  pub size_value: Option<f32>,
  pub text_align: Option<i64>,
  pub region_id: Option<std::vec::Vec<u8>>,
  pub voice: String,
  pub styled_text: String,
}

impl From<&VttCue<Uuid7>> for (MySqlSubtitleCueBaseRow, MySqlSubtitleCueVttRow) {
  fn from(c: &VttCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Vtt);
    let d = c.data_ref();
    let detail = MySqlSubtitleCueVttRow {
      id: base.id.clone(),
      cue_identifier: d.cue_identifier().to_owned(),
      vertical: d.vertical().map(|v| i64::from(v.to_u8())),
      line_value: d.line_value().to_owned(),
      line_align: d.line_align().map(|v| i64::from(v.to_u8())),
      position_value: d.position_value().to_owned(),
      position_align: d.position_align().map(|v| i64::from(v.to_u8())),
      size_value: d.size_value(),
      text_align: d.text_align().map(|v| i64::from(v.to_u8())),
      region_id: d.region_id_ref().map(|id| id.as_bytes().to_vec()),
      voice: d.voice().to_owned(),
      styled_text: d.styled_text().to_owned(),
    };
    (base, detail)
  }
}

fn map_small<T>(
  v: Option<i64>,
  decode: impl Fn(u8) -> Option<T>,
  what: &str,
) -> Result<Option<T>, SqlxError> {
  match v {
    None => Ok(None),
    Some(x) => {
      let u = u8::try_from(x).ok();
      let t = u.and_then(&decode);
      t.map(Some)
        .ok_or_else(|| SqlxError::UnknownDiscriminant(format!("{what}: {x}")))
    }
  }
}

/// Rebuild a WebVTT cue from its (base, detail) rows.
pub fn vtt_cue_from_rows(
  base: MySqlSubtitleCueBaseRow,
  detail: MySqlSubtitleCueVttRow,
  parent_timebase: mediatime::Timebase,
) -> Result<VttCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) = base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Vtt {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Vtt cue kind, got {kind:?}"
    )));
  }
  let vertical = map_small(detail.vertical, VttVertical::try_from_u8, "VttVertical")?;
  let line_align = map_small(detail.line_align, VttLineAlign::try_from_u8, "VttLineAlign")?;
  let position_align = map_small(
    detail.position_align,
    VttPositionAlign::try_from_u8,
    "VttPositionAlign",
  )?;
  let text_align = map_small(detail.text_align, VttTextAlign::try_from_u8, "VttTextAlign")?;
  let region_id = match detail.region_id {
    None => None,
    Some(b) => Some(bytes_to_uuid7(&b)?),
  };
  let d = VttData::<Uuid7>::new()
    .with_cue_identifier(detail.cue_identifier)
    .maybe_vertical(vertical)
    .with_line_value(detail.line_value)
    .maybe_line_align(line_align)
    .with_position_value(detail.position_value)
    .maybe_position_align(position_align)
    .maybe_size_value(detail.size_value)
    .maybe_text_align(text_align)
    .maybe_region_id(region_id)
    .with_voice(detail.voice)
    .with_styled_text(detail.styled_text);
  VttCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- ASS / SSA ---------------------------------------------------------------

/// MySQL detail row for an ASS/SSA cue.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleCueAssRow {
  pub id: std::vec::Vec<u8>,
  pub layer: i64,
  pub style_id: std::vec::Vec<u8>,
  pub name: String,
  pub margin_l: i64,
  pub margin_r: i64,
  pub margin_v: i64,
  pub effect: String,
  pub styled_text: String,
}

impl From<&AssCue<Uuid7>> for (MySqlSubtitleCueBaseRow, MySqlSubtitleCueAssRow) {
  fn from(c: &AssCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Ass);
    let d = c.data_ref();
    let detail = MySqlSubtitleCueAssRow {
      id: base.id.clone(),
      layer: i64::from(d.layer()),
      style_id: d.style_id_ref().as_bytes().to_vec(),
      name: d.name().to_owned(),
      margin_l: i64::from(d.margin_l()),
      margin_r: i64::from(d.margin_r()),
      margin_v: i64::from(d.margin_v()),
      effect: d.effect().to_owned(),
      styled_text: d.styled_text().to_owned(),
    };
    (base, detail)
  }
}

/// Rebuild an ASS cue from its (base, detail) rows.
pub fn ass_cue_from_rows(
  base: MySqlSubtitleCueBaseRow,
  detail: MySqlSubtitleCueAssRow,
  parent_timebase: mediatime::Timebase,
) -> Result<AssCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) = base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Ass {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Ass cue kind, got {kind:?}"
    )));
  }
  let style_id = bytes_to_uuid7(&detail.style_id)?;
  let i32_of = |v: i64, what: &str| {
    i32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
  };
  let d = AssData::<Uuid7>::new(style_id)
    .with_layer(i32_of(detail.layer, "AssData.layer")?)
    .with_name(detail.name)
    .with_margin_l(i32_of(detail.margin_l, "AssData.margin_l")?)
    .with_margin_r(i32_of(detail.margin_r, "AssData.margin_r")?)
    .with_margin_v(i32_of(detail.margin_v, "AssData.margin_v")?)
    .with_effect(detail.effect)
    .with_styled_text(detail.styled_text);
  AssCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- LRC ---------------------------------------------------------------------

/// MySQL detail row for an LRC cue.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleCueLrcRow {
  pub id: std::vec::Vec<u8>,
  pub has_word_timing: bool,
}

impl From<&LrcCue<Uuid7>> for (MySqlSubtitleCueBaseRow, MySqlSubtitleCueLrcRow) {
  fn from(c: &LrcCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Lrc);
    let detail = MySqlSubtitleCueLrcRow {
      id: base.id.clone(),
      has_word_timing: c.data_ref().has_word_timing(),
    };
    (base, detail)
  }
}

/// Rebuild an LRC cue from its (base, detail) rows.
pub fn lrc_cue_from_rows(
  base: MySqlSubtitleCueBaseRow,
  detail: MySqlSubtitleCueLrcRow,
  parent_timebase: mediatime::Timebase,
) -> Result<LrcCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) = base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Lrc {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Lrc cue kind, got {kind:?}"
    )));
  }
  let d = LrcData::new().maybe_word_timing(detail.has_word_timing);
  LrcCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- LRC word ----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleCueLrcWordRow {
  pub subtitle_cue_id: std::vec::Vec<u8>,
  pub ordinal: i64,
  pub text: String,
  pub start_pts: i64,
}

impl From<&LrcWord<Uuid7>> for MySqlSubtitleCueLrcWordRow {
  fn from(w: &LrcWord<Uuid7>) -> Self {
    Self {
      subtitle_cue_id: w.subtitle_cue_id_ref().as_bytes().to_vec(),
      ordinal: i64::from(w.ordinal()),
      text: w.text().to_owned(),
      start_pts: w.start_pts(),
    }
  }
}

pub fn lrc_word_from_row(r: MySqlSubtitleCueLrcWordRow) -> Result<LrcWord<Uuid7>, SqlxError> {
  let subtitle_cue_id = bytes_to_uuid7(&r.subtitle_cue_id)?;
  let ordinal = u32_from_i64(r.ordinal, "LrcWord.ordinal")?;
  LrcWord::try_new(subtitle_cue_id, ordinal, r.text, r.start_pts)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// ===========================================================================
// Per-track aggregates
// ===========================================================================

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct MySqlSubtitleTrackVttRegionRow {
  pub id: std::vec::Vec<u8>,
  pub subtitle_track_id: std::vec::Vec<u8>,
  pub name: String,
  pub width: f32,
  pub lines: i64,
  pub region_anchor_x: f32,
  pub region_anchor_y: f32,
  pub viewport_anchor_x: f32,
  pub viewport_anchor_y: f32,
  pub scroll_up: bool,
}

impl From<&VttRegion<Uuid7>> for MySqlSubtitleTrackVttRegionRow {
  fn from(r: &VttRegion<Uuid7>) -> Self {
    Self {
      id: r.id_ref().as_bytes().to_vec(),
      subtitle_track_id: r.subtitle_track_id_ref().as_bytes().to_vec(),
      name: r.name().to_owned(),
      width: r.width(),
      lines: i64::from(r.lines()),
      region_anchor_x: r.region_anchor_x(),
      region_anchor_y: r.region_anchor_y(),
      viewport_anchor_x: r.viewport_anchor_x(),
      viewport_anchor_y: r.viewport_anchor_y(),
      scroll_up: r.scroll_up(),
    }
  }
}

pub fn vtt_region_from_row(
  r: MySqlSubtitleTrackVttRegionRow,
) -> Result<VttRegion<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(&r.id)?;
  let subtitle_track_id = bytes_to_uuid7(&r.subtitle_track_id)?;
  let lines = u32_from_i64(r.lines, "VttRegion.lines")?;
  let region = VttRegion::try_new(id, subtitle_track_id, r.name)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_width(r.width)
    .with_lines(lines)
    .with_region_anchor(r.region_anchor_x, r.region_anchor_y)
    .with_viewport_anchor(r.viewport_anchor_x, r.viewport_anchor_y)
    .maybe_scroll_up(r.scroll_up);
  Ok(region)
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleTrackVttStyleRow {
  pub id: std::vec::Vec<u8>,
  pub subtitle_track_id: std::vec::Vec<u8>,
  pub ordinal: i64,
  pub css_text: String,
}

impl From<&VttStyleBlock<Uuid7>> for MySqlSubtitleTrackVttStyleRow {
  fn from(s: &VttStyleBlock<Uuid7>) -> Self {
    Self {
      id: s.id_ref().as_bytes().to_vec(),
      subtitle_track_id: s.subtitle_track_id_ref().as_bytes().to_vec(),
      ordinal: i64::from(s.ordinal()),
      css_text: s.css_text().to_owned(),
    }
  }
}

pub fn vtt_style_from_row(
  r: MySqlSubtitleTrackVttStyleRow,
) -> Result<VttStyleBlock<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(&r.id)?;
  let subtitle_track_id = bytes_to_uuid7(&r.subtitle_track_id)?;
  let ordinal = u32_from_i64(r.ordinal, "VttStyleBlock.ordinal")?;
  VttStyleBlock::try_new(id, subtitle_track_id, ordinal, r.css_text)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct MySqlSubtitleTrackAssStyleRow {
  pub id: std::vec::Vec<u8>,
  pub subtitle_track_id: std::vec::Vec<u8>,
  pub name: String,
  pub fontname: String,
  pub fontsize: f32,
  pub primary_colour: i64,
  pub secondary_colour: i64,
  pub outline_colour: i64,
  pub back_colour: i64,
  pub bold: bool,
  pub italic: bool,
  pub underline: bool,
  pub strikeout: bool,
  pub scale_x: i64,
  pub scale_y: i64,
  pub spacing: i64,
  pub angle: f32,
  pub border_style: i64,
  pub outline: f32,
  pub shadow: f32,
  pub alignment: i64,
  pub margin_l: i64,
  pub margin_r: i64,
  pub margin_v: i64,
  pub encoding: i64,
}

impl From<&AssStyle<Uuid7>> for MySqlSubtitleTrackAssStyleRow {
  fn from(s: &AssStyle<Uuid7>) -> Self {
    Self {
      id: s.id_ref().as_bytes().to_vec(),
      subtitle_track_id: s.subtitle_track_id_ref().as_bytes().to_vec(),
      name: s.name().to_owned(),
      fontname: s.fontname().to_owned(),
      fontsize: s.fontsize(),
      primary_colour: i64::from(s.primary_colour()),
      secondary_colour: i64::from(s.secondary_colour()),
      outline_colour: i64::from(s.outline_colour()),
      back_colour: i64::from(s.back_colour()),
      bold: s.bold(),
      italic: s.italic(),
      underline: s.underline(),
      strikeout: s.strikeout(),
      scale_x: i64::from(s.scale_x()),
      scale_y: i64::from(s.scale_y()),
      spacing: i64::from(s.spacing()),
      angle: s.angle(),
      border_style: i64::from(s.border_style()),
      outline: s.outline(),
      shadow: s.shadow(),
      alignment: i64::from(s.alignment()),
      margin_l: i64::from(s.margin_l()),
      margin_r: i64::from(s.margin_r()),
      margin_v: i64::from(s.margin_v()),
      encoding: i64::from(s.encoding()),
    }
  }
}

pub fn ass_style_from_row(r: MySqlSubtitleTrackAssStyleRow) -> Result<AssStyle<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(&r.id)?;
  let subtitle_track_id = bytes_to_uuid7(&r.subtitle_track_id)?;
  let to_u32 = |v: i64, what: &str| {
    u32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
  };
  let i16_of = |v: i64, what: &str| {
    i16::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
  };
  let i32_of = |v: i64, what: &str| {
    i32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
  };
  let s = AssStyle::try_new(id, subtitle_track_id, r.name)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_fontname(r.fontname)
    .with_fontsize(r.fontsize)
    .with_primary_colour(to_u32(r.primary_colour, "AssStyle.primary_colour")?)
    .with_secondary_colour(to_u32(r.secondary_colour, "AssStyle.secondary_colour")?)
    .with_outline_colour(to_u32(r.outline_colour, "AssStyle.outline_colour")?)
    .with_back_colour(to_u32(r.back_colour, "AssStyle.back_colour")?)
    .maybe_bold(r.bold)
    .maybe_italic(r.italic)
    .maybe_underline(r.underline)
    .maybe_strikeout(r.strikeout)
    .with_scale_x(i32_of(r.scale_x, "AssStyle.scale_x")?)
    .with_scale_y(i32_of(r.scale_y, "AssStyle.scale_y")?)
    .with_spacing(i32_of(r.spacing, "AssStyle.spacing")?)
    .with_angle(r.angle)
    .with_border_style(i16_of(r.border_style, "AssStyle.border_style")?)
    .with_outline(r.outline)
    .with_shadow(r.shadow)
    .with_alignment(i16_of(r.alignment, "AssStyle.alignment")?)
    .with_margin_l(i32_of(r.margin_l, "AssStyle.margin_l")?)
    .with_margin_r(i32_of(r.margin_r, "AssStyle.margin_r")?)
    .with_margin_v(i32_of(r.margin_v, "AssStyle.margin_v")?)
    .with_encoding(i32_of(r.encoding, "AssStyle.encoding")?);
  Ok(s)
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleTrackLrcMetadataRow {
  pub subtitle_track_id: std::vec::Vec<u8>,
  pub title: String,
  pub artist: String,
  pub album: String,
  pub author: String,
  pub creator: String,
  pub length: String,
  pub offset_ms: i64,
}

impl From<&LrcMetadata<Uuid7>> for MySqlSubtitleTrackLrcMetadataRow {
  fn from(m: &LrcMetadata<Uuid7>) -> Self {
    Self {
      subtitle_track_id: m.subtitle_track_id_ref().as_bytes().to_vec(),
      title: m.title().to_owned(),
      artist: m.artist().to_owned(),
      album: m.album().to_owned(),
      author: m.author().to_owned(),
      creator: m.creator().to_owned(),
      length: m.length().to_owned(),
      offset_ms: i64::from(m.offset_ms()),
    }
  }
}

pub fn lrc_metadata_from_row(
  r: MySqlSubtitleTrackLrcMetadataRow,
) -> Result<LrcMetadata<Uuid7>, SqlxError> {
  let subtitle_track_id = bytes_to_uuid7(&r.subtitle_track_id)?;
  let offset_ms = i32::try_from(r.offset_ms)
    .map_err(|e| SqlxError::UnknownDiscriminant(format!("LrcMetadata.offset_ms: {e}")))?;
  let m = LrcMetadata::try_new(subtitle_track_id)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_title(r.title)
    .with_artist(r.artist)
    .with_album(r.album)
    .with_author(r.author)
    .with_creator(r.creator)
    .with_length(r.length)
    .with_offset_ms(offset_ms);
  Ok(m)
}

// ===========================================================================
// Borrowed-view siblings (`*RowRef<'r>`) — zero-copy decode from `&'r Row`.
// ===========================================================================

/// Borrowed view of [`MySqlSubtitleRow`] — zero-copy decode from `&'r Row`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleRowRef<'r> {
  pub id: &'r [u8],
  pub media_id: &'r [u8],
  pub track_progress_total: i64,
  pub track_progress_indexed: i64,
  pub track_progress_failed: i64,
}

/// Borrowed view of [`MySqlSubtitleTrackRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct MySqlSubtitleTrackRowRef<'r> {
  pub id: &'r [u8],
  pub subtitle_id: &'r [u8],
  pub stream_index: Option<i64>,
  pub container_track_id: Option<i64>,
  pub codec: &'r str,
  pub format: &'r str,
  pub origin: i32,
  pub language: Option<&'r str>,
  pub title: &'r str,
  pub disposition: i64,
  pub is_primary: bool,
  pub auto_selected: bool,
  pub duration_pts: Option<i64>,
  pub duration_tb_num: Option<i64>,
  pub duration_tb_den: Option<i64>,
  pub cue_count: i64,
  pub provenance_model_name: &'r str,
  pub provenance_model_version: &'r str,
  pub provenance_prompt_version: &'r str,
  pub provenance_indexer_version: &'r str,
  pub source_checksum: Option<&'r [u8]>,
  pub character_encoding: &'r str,
  pub bom_present: bool,
  pub is_sdh: bool,
  pub is_closed_caption: bool,
  pub is_translation: bool,
  pub kind: i16,
  pub coverage_ratio: Option<f32>,
  pub is_empty: bool,
  pub first_cue_pts: Option<i64>,
  pub first_cue_tb_num: Option<i64>,
  pub first_cue_tb_den: Option<i64>,
  pub last_cue_pts: Option<i64>,
  pub last_cue_tb_num: Option<i64>,
  pub last_cue_tb_den: Option<i64>,
  pub index_status: i64,
}

/// Borrowed view of [`MySqlSubtitleTrackIndexErrorRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleTrackIndexErrorRowRef<'r> {
  pub subtitle_track_id: &'r [u8],
  pub ordinal: i32,
  pub code: i32,
  pub message: &'r str,
}

// --- Polymorphic subtitle_cue tables ----------------------------------------
//
// MySQL stores ids as `BINARY(16)` (Vec<u8> owned, &'r [u8] borrowed); only
// the variable-length text + BLOB-byte columns flip away from `Copy` types.

/// Borrowed view of [`MySqlSubtitleCueBaseRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleCueBaseRowRef<'r> {
  pub id: &'r [u8],
  pub subtitle_track_id: &'r [u8],
  pub ordinal: i64,
  pub span_start_pts: i64,
  pub span_end_pts: i64,
  pub text_src: &'r str,
  pub text_translated: &'r str,
  pub kind: i64,
}

/// Borrowed view of [`MySqlSubtitleCueVttRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct MySqlSubtitleCueVttRowRef<'r> {
  pub id: &'r [u8],
  pub cue_identifier: &'r str,
  pub vertical: Option<i64>,
  pub line_value: &'r str,
  pub line_align: Option<i64>,
  pub position_value: &'r str,
  pub position_align: Option<i64>,
  pub size_value: Option<f32>,
  pub text_align: Option<i64>,
  pub region_id: Option<&'r [u8]>,
  pub voice: &'r str,
  pub styled_text: &'r str,
}

/// Borrowed view of [`MySqlSubtitleCueAssRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleCueAssRowRef<'r> {
  pub id: &'r [u8],
  pub layer: i64,
  pub style_id: &'r [u8],
  pub name: &'r str,
  pub margin_l: i64,
  pub margin_r: i64,
  pub margin_v: i64,
  pub effect: &'r str,
  pub styled_text: &'r str,
}

/// Borrowed view of [`MySqlSubtitleCueLrcRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleCueLrcRowRef<'r> {
  pub id: &'r [u8],
  pub has_word_timing: bool,
}

/// Borrowed view of [`MySqlSubtitleCueLrcWordRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleCueLrcWordRowRef<'r> {
  pub subtitle_cue_id: &'r [u8],
  pub ordinal: i64,
  pub text: &'r str,
  pub start_pts: i64,
}

/// Borrowed view of [`MySqlSubtitleTrackVttRegionRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct MySqlSubtitleTrackVttRegionRowRef<'r> {
  pub id: &'r [u8],
  pub subtitle_track_id: &'r [u8],
  pub name: &'r str,
  pub width: f32,
  pub lines: i64,
  pub region_anchor_x: f32,
  pub region_anchor_y: f32,
  pub viewport_anchor_x: f32,
  pub viewport_anchor_y: f32,
  pub scroll_up: bool,
}

/// Borrowed view of [`MySqlSubtitleTrackVttStyleRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleTrackVttStyleRowRef<'r> {
  pub id: &'r [u8],
  pub subtitle_track_id: &'r [u8],
  pub ordinal: i64,
  pub css_text: &'r str,
}

/// Borrowed view of [`MySqlSubtitleTrackAssStyleRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct MySqlSubtitleTrackAssStyleRowRef<'r> {
  pub id: &'r [u8],
  pub subtitle_track_id: &'r [u8],
  pub name: &'r str,
  pub fontname: &'r str,
  pub fontsize: f32,
  pub primary_colour: i64,
  pub secondary_colour: i64,
  pub outline_colour: i64,
  pub back_colour: i64,
  pub bold: bool,
  pub italic: bool,
  pub underline: bool,
  pub strikeout: bool,
  pub scale_x: i64,
  pub scale_y: i64,
  pub spacing: i64,
  pub angle: f32,
  pub border_style: i64,
  pub outline: f32,
  pub shadow: f32,
  pub alignment: i64,
  pub margin_l: i64,
  pub margin_r: i64,
  pub margin_v: i64,
  pub encoding: i64,
}

/// Borrowed view of [`MySqlSubtitleTrackLrcMetadataRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSubtitleTrackLrcMetadataRowRef<'r> {
  pub subtitle_track_id: &'r [u8],
  pub title: &'r str,
  pub artist: &'r str,
  pub album: &'r str,
  pub author: &'r str,
  pub creator: &'r str,
  pub length: &'r str,
  pub offset_ms: i64,
}

impl MySqlSubtitleCueBaseRow {
  /// Cheap borrow — produces a [`MySqlSubtitleCueBaseRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlSubtitleCueBaseRowRef<'_> {
    MySqlSubtitleCueBaseRowRef {
      id: &self.id,
      subtitle_track_id: &self.subtitle_track_id,
      ordinal: self.ordinal,
      span_start_pts: self.span_start_pts,
      span_end_pts: self.span_end_pts,
      text_src: &self.text_src,
      text_translated: &self.text_translated,
      kind: self.kind,
    }
  }
}

impl MySqlSubtitleCueVttRow {
  /// Cheap borrow — produces a [`MySqlSubtitleCueVttRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlSubtitleCueVttRowRef<'_> {
    MySqlSubtitleCueVttRowRef {
      id: &self.id,
      cue_identifier: &self.cue_identifier,
      vertical: self.vertical,
      line_value: &self.line_value,
      line_align: self.line_align,
      position_value: &self.position_value,
      position_align: self.position_align,
      size_value: self.size_value,
      text_align: self.text_align,
      region_id: self.region_id.as_deref(),
      voice: &self.voice,
      styled_text: &self.styled_text,
    }
  }
}

impl MySqlSubtitleCueAssRow {
  /// Cheap borrow — produces a [`MySqlSubtitleCueAssRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlSubtitleCueAssRowRef<'_> {
    MySqlSubtitleCueAssRowRef {
      id: &self.id,
      layer: self.layer,
      style_id: &self.style_id,
      name: &self.name,
      margin_l: self.margin_l,
      margin_r: self.margin_r,
      margin_v: self.margin_v,
      effect: &self.effect,
      styled_text: &self.styled_text,
    }
  }
}

impl MySqlSubtitleCueLrcRow {
  /// Cheap borrow — produces a [`MySqlSubtitleCueLrcRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlSubtitleCueLrcRowRef<'_> {
    MySqlSubtitleCueLrcRowRef {
      id: &self.id,
      has_word_timing: self.has_word_timing,
    }
  }
}

impl MySqlSubtitleCueLrcWordRow {
  /// Cheap borrow — produces a [`MySqlSubtitleCueLrcWordRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlSubtitleCueLrcWordRowRef<'_> {
    MySqlSubtitleCueLrcWordRowRef {
      subtitle_cue_id: &self.subtitle_cue_id,
      ordinal: self.ordinal,
      text: &self.text,
      start_pts: self.start_pts,
    }
  }
}

impl MySqlSubtitleTrackVttRegionRow {
  /// Cheap borrow — produces a [`MySqlSubtitleTrackVttRegionRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlSubtitleTrackVttRegionRowRef<'_> {
    MySqlSubtitleTrackVttRegionRowRef {
      id: &self.id,
      subtitle_track_id: &self.subtitle_track_id,
      name: &self.name,
      width: self.width,
      lines: self.lines,
      region_anchor_x: self.region_anchor_x,
      region_anchor_y: self.region_anchor_y,
      viewport_anchor_x: self.viewport_anchor_x,
      viewport_anchor_y: self.viewport_anchor_y,
      scroll_up: self.scroll_up,
    }
  }
}

impl MySqlSubtitleTrackVttStyleRow {
  /// Cheap borrow — produces a [`MySqlSubtitleTrackVttStyleRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlSubtitleTrackVttStyleRowRef<'_> {
    MySqlSubtitleTrackVttStyleRowRef {
      id: &self.id,
      subtitle_track_id: &self.subtitle_track_id,
      ordinal: self.ordinal,
      css_text: &self.css_text,
    }
  }
}

impl MySqlSubtitleTrackAssStyleRow {
  /// Cheap borrow — produces a [`MySqlSubtitleTrackAssStyleRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlSubtitleTrackAssStyleRowRef<'_> {
    MySqlSubtitleTrackAssStyleRowRef {
      id: &self.id,
      subtitle_track_id: &self.subtitle_track_id,
      name: &self.name,
      fontname: &self.fontname,
      fontsize: self.fontsize,
      primary_colour: self.primary_colour,
      secondary_colour: self.secondary_colour,
      outline_colour: self.outline_colour,
      back_colour: self.back_colour,
      bold: self.bold,
      italic: self.italic,
      underline: self.underline,
      strikeout: self.strikeout,
      scale_x: self.scale_x,
      scale_y: self.scale_y,
      spacing: self.spacing,
      angle: self.angle,
      border_style: self.border_style,
      outline: self.outline,
      shadow: self.shadow,
      alignment: self.alignment,
      margin_l: self.margin_l,
      margin_r: self.margin_r,
      margin_v: self.margin_v,
      encoding: self.encoding,
    }
  }
}

impl MySqlSubtitleTrackLrcMetadataRow {
  /// Cheap borrow — produces a [`MySqlSubtitleTrackLrcMetadataRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlSubtitleTrackLrcMetadataRowRef<'_> {
    MySqlSubtitleTrackLrcMetadataRowRef {
      subtitle_track_id: &self.subtitle_track_id,
      title: &self.title,
      artist: &self.artist,
      album: &self.album,
      author: &self.author,
      creator: &self.creator,
      length: &self.length,
      offset_ms: self.offset_ms,
    }
  }
}

// --- Borrowed-view promotion fns ---------------------------------------------

fn base_row_ref_to_parts<'r>(
  r: &MySqlSubtitleCueBaseRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<
  (
    Uuid7,
    Uuid7,
    u32,
    mediatime::TimeRange,
    LocalizedText,
    SubtitleCueKind,
  ),
  SqlxError,
> {
  let id = bytes_to_uuid7(r.id)?;
  let subtitle_track_id = bytes_to_uuid7(r.subtitle_track_id)?;
  let ordinal = u32_from_i64(r.ordinal, "SubtitleCue.ordinal")?;
  let span =
    mediatime::TimeRange::try_new(r.span_start_pts, r.span_end_pts, parent_timebase).ok_or_else(
      || {
        SqlxError::DomainConstructorRejected(format!(
          "TimeRange start_pts ({}) must be <= end_pts ({})",
          r.span_start_pts, r.span_end_pts
        ))
      },
    )?;
  let text = LocalizedText::from_src_translated(r.text_src.to_owned(), r.text_translated.to_owned());
  let kind = cue_kind_from_i64_v(r.kind)?;
  Ok((id, subtitle_track_id, ordinal, span, text, kind))
}

/// Rebuild a SubRip cue from its borrowed base row.
pub fn srt_cue_from_row_ref<'r>(
  base: MySqlSubtitleCueBaseRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<SrtCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_ref_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Srt {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Srt cue kind, got {kind:?}"
    )));
  }
  SrtCue::try_new(id, subtitle_track_id, ordinal, span, text, SrtData)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// Rebuild a WebVTT cue from its borrowed (base, detail) rows.
pub fn vtt_cue_from_row_refs<'r>(
  base: MySqlSubtitleCueBaseRowRef<'r>,
  detail: MySqlSubtitleCueVttRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<VttCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_ref_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Vtt {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Vtt cue kind, got {kind:?}"
    )));
  }
  let vertical = map_small(detail.vertical, VttVertical::try_from_u8, "VttVertical")?;
  let line_align = map_small(detail.line_align, VttLineAlign::try_from_u8, "VttLineAlign")?;
  let position_align = map_small(
    detail.position_align,
    VttPositionAlign::try_from_u8,
    "VttPositionAlign",
  )?;
  let text_align = map_small(detail.text_align, VttTextAlign::try_from_u8, "VttTextAlign")?;
  let region_id = match detail.region_id {
    None => None,
    Some(b) => Some(bytes_to_uuid7(b)?),
  };
  let d = VttData::<Uuid7>::new()
    .with_cue_identifier(detail.cue_identifier)
    .maybe_vertical(vertical)
    .with_line_value(detail.line_value)
    .maybe_line_align(line_align)
    .with_position_value(detail.position_value)
    .maybe_position_align(position_align)
    .maybe_size_value(detail.size_value)
    .maybe_text_align(text_align)
    .maybe_region_id(region_id)
    .with_voice(detail.voice)
    .with_styled_text(detail.styled_text);
  VttCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// Rebuild an ASS cue from its borrowed (base, detail) rows.
pub fn ass_cue_from_row_refs<'r>(
  base: MySqlSubtitleCueBaseRowRef<'r>,
  detail: MySqlSubtitleCueAssRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<AssCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_ref_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Ass {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Ass cue kind, got {kind:?}"
    )));
  }
  let style_id = bytes_to_uuid7(detail.style_id)?;
  let i32_of = |v: i64, what: &str| {
    i32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
  };
  let d = AssData::<Uuid7>::new(style_id)
    .with_layer(i32_of(detail.layer, "AssData.layer")?)
    .with_name(detail.name)
    .with_margin_l(i32_of(detail.margin_l, "AssData.margin_l")?)
    .with_margin_r(i32_of(detail.margin_r, "AssData.margin_r")?)
    .with_margin_v(i32_of(detail.margin_v, "AssData.margin_v")?)
    .with_effect(detail.effect)
    .with_styled_text(detail.styled_text);
  AssCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// Rebuild an LRC cue from its borrowed (base, detail) rows.
pub fn lrc_cue_from_row_refs<'r>(
  base: MySqlSubtitleCueBaseRowRef<'r>,
  detail: MySqlSubtitleCueLrcRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<LrcCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_ref_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Lrc {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Lrc cue kind, got {kind:?}"
    )));
  }
  let d = LrcData::new().maybe_word_timing(detail.has_word_timing);
  LrcCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// Rebuild an LRC word from its borrowed row.
pub fn lrc_word_from_row_ref<'r>(
  r: MySqlSubtitleCueLrcWordRowRef<'r>,
) -> Result<LrcWord<Uuid7>, SqlxError> {
  let subtitle_cue_id = bytes_to_uuid7(r.subtitle_cue_id)?;
  let ordinal = u32_from_i64(r.ordinal, "LrcWord.ordinal")?;
  LrcWord::try_new(subtitle_cue_id, ordinal, r.text, r.start_pts)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// Rebuild a [`VttRegion`] from its borrowed row.
pub fn vtt_region_from_row_ref<'r>(
  r: MySqlSubtitleTrackVttRegionRowRef<'r>,
) -> Result<VttRegion<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(r.id)?;
  let subtitle_track_id = bytes_to_uuid7(r.subtitle_track_id)?;
  let lines = u32_from_i64(r.lines, "VttRegion.lines")?;
  let region = VttRegion::try_new(id, subtitle_track_id, r.name)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_width(r.width)
    .with_lines(lines)
    .with_region_anchor(r.region_anchor_x, r.region_anchor_y)
    .with_viewport_anchor(r.viewport_anchor_x, r.viewport_anchor_y)
    .maybe_scroll_up(r.scroll_up);
  Ok(region)
}

/// Rebuild a [`VttStyleBlock`] from its borrowed row.
pub fn vtt_style_from_row_ref<'r>(
  r: MySqlSubtitleTrackVttStyleRowRef<'r>,
) -> Result<VttStyleBlock<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(r.id)?;
  let subtitle_track_id = bytes_to_uuid7(r.subtitle_track_id)?;
  let ordinal = u32_from_i64(r.ordinal, "VttStyleBlock.ordinal")?;
  VttStyleBlock::try_new(id, subtitle_track_id, ordinal, r.css_text)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// Rebuild an [`AssStyle`] from its borrowed row.
pub fn ass_style_from_row_ref<'r>(
  r: MySqlSubtitleTrackAssStyleRowRef<'r>,
) -> Result<AssStyle<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(r.id)?;
  let subtitle_track_id = bytes_to_uuid7(r.subtitle_track_id)?;
  let to_u32 = |v: i64, what: &str| {
    u32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
  };
  let i16_of = |v: i64, what: &str| {
    i16::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
  };
  let i32_of = |v: i64, what: &str| {
    i32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
  };
  let s = AssStyle::try_new(id, subtitle_track_id, r.name)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_fontname(r.fontname)
    .with_fontsize(r.fontsize)
    .with_primary_colour(to_u32(r.primary_colour, "AssStyle.primary_colour")?)
    .with_secondary_colour(to_u32(r.secondary_colour, "AssStyle.secondary_colour")?)
    .with_outline_colour(to_u32(r.outline_colour, "AssStyle.outline_colour")?)
    .with_back_colour(to_u32(r.back_colour, "AssStyle.back_colour")?)
    .maybe_bold(r.bold)
    .maybe_italic(r.italic)
    .maybe_underline(r.underline)
    .maybe_strikeout(r.strikeout)
    .with_scale_x(i32_of(r.scale_x, "AssStyle.scale_x")?)
    .with_scale_y(i32_of(r.scale_y, "AssStyle.scale_y")?)
    .with_spacing(i32_of(r.spacing, "AssStyle.spacing")?)
    .with_angle(r.angle)
    .with_border_style(i16_of(r.border_style, "AssStyle.border_style")?)
    .with_outline(r.outline)
    .with_shadow(r.shadow)
    .with_alignment(i16_of(r.alignment, "AssStyle.alignment")?)
    .with_margin_l(i32_of(r.margin_l, "AssStyle.margin_l")?)
    .with_margin_r(i32_of(r.margin_r, "AssStyle.margin_r")?)
    .with_margin_v(i32_of(r.margin_v, "AssStyle.margin_v")?)
    .with_encoding(i32_of(r.encoding, "AssStyle.encoding")?);
  Ok(s)
}

/// Rebuild an [`LrcMetadata`] from its borrowed row.
pub fn lrc_metadata_from_row_ref<'r>(
  r: MySqlSubtitleTrackLrcMetadataRowRef<'r>,
) -> Result<LrcMetadata<Uuid7>, SqlxError> {
  let subtitle_track_id = bytes_to_uuid7(r.subtitle_track_id)?;
  let offset_ms = i32::try_from(r.offset_ms)
    .map_err(|e| SqlxError::UnknownDiscriminant(format!("LrcMetadata.offset_ms: {e}")))?;
  let m = LrcMetadata::try_new(subtitle_track_id)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_title(r.title)
    .with_artist(r.artist)
    .with_album(r.album)
    .with_author(r.author)
    .with_creator(r.creator)
    .with_length(r.length)
    .with_offset_ms(offset_ms);
  Ok(m)
}

impl MySqlSubtitleRow {
  /// Cheap borrow — produces a [`MySqlSubtitleRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlSubtitleRowRef<'_> {
    MySqlSubtitleRowRef {
      id: &self.id,
      media_id: &self.media_id,
      track_progress_total: self.track_progress_total,
      track_progress_indexed: self.track_progress_indexed,
      track_progress_failed: self.track_progress_failed,
    }
  }
}

impl MySqlSubtitleTrackRow {
  /// Cheap borrow — produces a [`MySqlSubtitleTrackRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlSubtitleTrackRowRef<'_> {
    MySqlSubtitleTrackRowRef {
      id: &self.id,
      subtitle_id: &self.subtitle_id,
      stream_index: self.stream_index,
      container_track_id: self.container_track_id,
      codec: &self.codec,
      format: &self.format,
      origin: self.origin,
      language: self.language.as_deref(),
      title: &self.title,
      disposition: self.disposition,
      is_primary: self.is_primary,
      auto_selected: self.auto_selected,
      duration_pts: self.duration_pts,
      duration_tb_num: self.duration_tb_num,
      duration_tb_den: self.duration_tb_den,
      cue_count: self.cue_count,
      provenance_model_name: &self.provenance_model_name,
      provenance_model_version: &self.provenance_model_version,
      provenance_prompt_version: &self.provenance_prompt_version,
      provenance_indexer_version: &self.provenance_indexer_version,
      source_checksum: self.source_checksum.as_deref(),
      character_encoding: &self.character_encoding,
      bom_present: self.bom_present,
      is_sdh: self.is_sdh,
      is_closed_caption: self.is_closed_caption,
      is_translation: self.is_translation,
      kind: self.kind,
      coverage_ratio: self.coverage_ratio,
      is_empty: self.is_empty,
      first_cue_pts: self.first_cue_pts,
      first_cue_tb_num: self.first_cue_tb_num,
      first_cue_tb_den: self.first_cue_tb_den,
      last_cue_pts: self.last_cue_pts,
      last_cue_tb_num: self.last_cue_tb_num,
      last_cue_tb_den: self.last_cue_tb_den,
      index_status: self.index_status,
    }
  }
}

impl MySqlSubtitleTrackIndexErrorRow {
  /// Cheap borrow — produces a [`MySqlSubtitleTrackIndexErrorRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlSubtitleTrackIndexErrorRowRef<'_> {
    MySqlSubtitleTrackIndexErrorRowRef {
      subtitle_track_id: &self.subtitle_track_id,
      ordinal: self.ordinal,
      code: self.code,
      message: &self.message,
    }
  }
}

impl<'r> TryFrom<MySqlSubtitleRowRef<'r>> for Subtitle<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: MySqlSubtitleRowRef<'r>) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
    let media_id = bytes_to_uuid7(r.media_id)?;
    let progress = IndexProgress::from_parts(
      u32_from_i64(r.track_progress_total, "Subtitle.track_progress_total")?,
      u32_from_i64(r.track_progress_indexed, "Subtitle.track_progress_indexed")?,
      u32_from_i64(r.track_progress_failed, "Subtitle.track_progress_failed")?,
    );
    let s = Subtitle::try_new(id, media_id)
      .map_err(|e: SubtitleError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    Ok(s.with_track_progress(progress))
  }
}

impl<'r>
  TryFrom<(
    MySqlSubtitleTrackRowRef<'r>,
    std::vec::Vec<MySqlSubtitleTrackIndexErrorRowRef<'r>>,
  )> for SubtitleTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, mut errors): (
      MySqlSubtitleTrackRowRef<'r>,
      std::vec::Vec<MySqlSubtitleTrackIndexErrorRowRef<'r>>,
    ),
  ) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
    let subtitle_id = bytes_to_uuid7(r.subtitle_id)?;
    let mut t = SubtitleTrack::try_new(id, subtitle_id)
      .map_err(|e: SubtitleTrackError| SqlxError::DomainConstructorRejected(e.to_string()))?;

    t = t
      .with_codec(parse_subtitle_codec(r.codec))
      .with_format(parse_subtitle_format(r.format))
      .with_origin(
        TrackOrigin::try_from_u32(u32_from_i32(r.origin, "SubtitleTrack.origin")?)
          .ok_or_else(|| SqlxError::UnknownDiscriminant(format!("TrackOrigin: {}", r.origin)))?,
      )
      .with_title(r.title)
      .with_disposition(TrackDisposition::from_bits_truncate(u32_from_i64(
        r.disposition,
        "SubtitleTrack.disposition",
      )?))
      .with_primary(r.is_primary)
      .with_auto_selected(r.auto_selected)
      .with_cue_count(u32_from_i64(r.cue_count, "SubtitleTrack.cue_count")?)
      .with_stream_index(opt_u32(r.stream_index, "SubtitleTrack.stream_index")?)
      .with_container_track_id(r.container_track_id.map(|v| v as u64))
      .with_character_encoding(r.character_encoding)
      .with_bom_present(r.bom_present)
      .with_sdh(r.is_sdh)
      .with_closed_caption(r.is_closed_caption)
      .with_translation(r.is_translation)
      .with_kind(kind_from_i16(r.kind)?)
      .with_coverage_ratio(r.coverage_ratio)
      .with_empty(r.is_empty);

    if let Some(s) = r.language {
      t = t.with_language(parse_language(s)?);
    }

    if let Some(pts) = r.duration_pts {
      let (num, den) = require_timebase(
        r.duration_tb_num,
        r.duration_tb_den,
        "SubtitleTrack.duration",
      )?;
      t = t.with_duration(Some(timestamp_from_parts(pts, num, den)?));
    }
    if let Some(pts) = r.first_cue_pts {
      let (num, den) = require_timebase(
        r.first_cue_tb_num,
        r.first_cue_tb_den,
        "SubtitleTrack.first_cue",
      )?;
      t = t.with_first_cue(Some(timestamp_from_parts(pts, num, den)?));
    }
    if let Some(pts) = r.last_cue_pts {
      let (num, den) = require_timebase(
        r.last_cue_tb_num,
        r.last_cue_tb_den,
        "SubtitleTrack.last_cue",
      )?;
      t = t.with_last_cue(Some(timestamp_from_parts(pts, num, den)?));
    }

    if let Some(bytes) = r.source_checksum {
      t = t.with_source_checksum(Some(bytes_to_checksum(bytes)?));
    }

    t = t.with_provenance(Provenance::from_parts(
      r.provenance_model_name,
      r.provenance_model_version,
      r.provenance_prompt_version,
      r.provenance_indexer_version,
    ));

    let status = SubtitleIndexStatus::from_bits_truncate(u32_from_i64(
      r.index_status,
      "SubtitleTrack.index_status",
    )?);
    t = t.with_index_status(status);

    errors.sort_by_key(|e| e.ordinal);
    let mut infos = std::vec::Vec::with_capacity(errors.len());
    for e in errors {
      let code = u32_from_i32(e.code, "SubtitleTrack.index_error.code")?;
      infos.push(ErrorInfo::new(ErrorCode::from_u32(code), e.message));
    }
    t = t.with_index_errors(infos);

    Ok(t)
  }
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

fn parse_subtitle_codec(s: &str) -> SubtitleCodec {
  s.parse::<SubtitleCodec>()
    .unwrap_or_else(|_| SubtitleCodec::Other(s.into()))
}

fn parse_subtitle_format(s: &str) -> Format {
  s.parse::<Format>()
    .unwrap_or_else(|_| Format::Other(s.into()))
}

fn parse_language(s: &str) -> Result<Language, SqlxError> {
  Language::from_bcp47(s)
    .map_err(|e| SqlxError::DomainConstructorRejected(format!("Language `{s}`: {e}")))
}

fn language_to_slug(lang: &Language) -> Option<String> {
  let bcp47 = lang.to_bcp47();
  if lang == &Language::default() {
    None
  } else {
    Some(bcp47)
  }
}

fn u32_from_i64(v: i64, what: &str) -> Result<u32, SqlxError> {
  u32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
}

fn u32_from_i32(v: i32, what: &str) -> Result<u32, SqlxError> {
  u32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
}

fn opt_u32(v: Option<i64>, what: &str) -> Result<Option<u32>, SqlxError> {
  match v {
    None => Ok(None),
    Some(x) => Ok(Some(u32_from_i64(x, what)?)),
  }
}

fn require_timebase(
  num: Option<i64>,
  den: Option<i64>,
  what: &str,
) -> Result<(i64, i64), SqlxError> {
  match (num, den) {
    (Some(n), Some(d)) => Ok((n, d)),
    _ => Err(SqlxError::DomainConstructorRejected(format!(
      "{what}: PTS present but timebase columns missing"
    ))),
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::FileChecksum;
  use bytes::Bytes;
  use core::num::NonZeroU32;
  use mediatime::{TimeRange, Timebase, Timestamp};
  use smol_str::SmolStr;

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).unwrap())
  }

  #[test]
  fn subtitle_facet_roundtrip() {
    let s = Subtitle::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_track_progress(IndexProgress::try_new(3, 2, 1).unwrap());
    let row: MySqlSubtitleRow = (&s).into();
    let s2: Subtitle<Uuid7> = row.try_into().unwrap();
    assert_eq!(s.id_ref(), s2.id_ref());
    assert_eq!(s.media_id_ref(), s2.media_id_ref());
    assert_eq!(s2.track_progress_ref().total(), 3);
  }

  #[test]
  fn subtitle_track_roundtrip_minimal() {
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    let tuple: (
      MySqlSubtitleTrackRow,
      std::vec::Vec<MySqlSubtitleTrackIndexErrorRow>,
    ) = (&t).into();
    let t2: SubtitleTrack<Uuid7> = tuple.try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn subtitle_track_roundtrip_full() {
    let en = Language::from_bcp47("en").unwrap();
    let mut bytes = [0u8; 32];
    bytes[0] = 1;
    let cs = FileChecksum::from_bytes(bytes);
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(SubtitleCodec::Subrip)
      .with_format(Format::Srt)
      .with_origin(TrackOrigin::External)
      .with_language(en)
      .with_title("English (SDH)")
      .with_disposition(TrackDisposition::from_u32(0x0040))
      .with_primary(true)
      .with_auto_selected(true)
      .with_duration(Some(Timestamp::new(120_000, tb())))
      .with_cue_count(42)
      .with_provenance(Provenance::from_parts(
        "tesseract",
        "5.3.0",
        "p1",
        "indexer-0.4.2",
      ))
      .with_source_checksum(Some(cs))
      .with_character_encoding("UTF-8")
      .with_bom_present(true)
      .with_sdh(true)
      .with_translation(true)
      .with_kind(SubtitleKind::ForcedNarrative)
      .with_coverage_ratio(Some(0.97))
      .with_first_cue(Some(Timestamp::new(500, tb())))
      .with_last_cue(Some(Timestamp::new(119_500, tb())))
      .with_index_status(
        SubtitleIndexStatus::TRACKS_DISCOVERED | SubtitleIndexStatus::CUES_EXTRACTED,
      )
      .with_index_errors(std::vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad")]);
    let tuple: (
      MySqlSubtitleTrackRow,
      std::vec::Vec<MySqlSubtitleTrackIndexErrorRow>,
    ) = (&t).into();
    let t2: SubtitleTrack<Uuid7> = tuple.try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn subtitle_track_index_errors_rebuild_in_ordinal_order() {
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_index_errors(std::vec![
        ErrorInfo::new(ErrorCode::ProbeCorrupt, "a"),
        ErrorInfo::new(ErrorCode::PathNotFound, "b"),
      ]);
    let (row, mut errs): (
      MySqlSubtitleTrackRow,
      std::vec::Vec<MySqlSubtitleTrackIndexErrorRow>,
    ) = (&t).into();
    errs.reverse();
    let t2: SubtitleTrack<Uuid7> = (row, errs).try_into().unwrap();
    assert_eq!(t2.index_errors_slice()[0].message(), "a");
    assert_eq!(t2.index_errors_slice()[1].message(), "b");
  }

  #[test]
  fn srt_cue_round_trip() {
    let c = SrtCue::try_new_srt(
      Uuid7::new(),
      Uuid7::new(),
      3,
      TimeRange::new(1_000, 2_000, tb()),
      LocalizedText::from_src_translated("Hola", "Hello"),
    )
    .unwrap();
    let base: MySqlSubtitleCueBaseRow = (&c).into();
    let c2 = srt_cue_from_row(base, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn vtt_cue_round_trip() {
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
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("hi"),
      d,
    )
    .unwrap();
    let (base, detail): (MySqlSubtitleCueBaseRow, MySqlSubtitleCueVttRow) = (&c).into();
    let c2 = vtt_cue_from_rows(base, detail, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn ass_cue_round_trip() {
    let style_id = Uuid7::new();
    let d = AssData::<Uuid7>::new(style_id)
      .with_layer(2)
      .with_name("Alice")
      .with_styled_text("{\\b1}hi{\\b0}");
    let c: AssCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::new(),
      d,
    )
    .unwrap();
    let (base, detail): (MySqlSubtitleCueBaseRow, MySqlSubtitleCueAssRow) = (&c).into();
    let c2 = ass_cue_from_rows(base, detail, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn lrc_cue_round_trip() {
    let c: LrcCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("la la"),
      LrcData::new().with_word_timing(),
    )
    .unwrap();
    let (base, detail): (MySqlSubtitleCueBaseRow, MySqlSubtitleCueLrcRow) = (&c).into();
    let c2 = lrc_cue_from_rows(base, detail, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn lrc_word_round_trip() {
    let w = LrcWord::try_new(Uuid7::new(), 3, "ma", 1234).unwrap();
    let row: MySqlSubtitleCueLrcWordRow = (&w).into();
    let w2 = lrc_word_from_row(row).unwrap();
    assert_eq!(w, w2);
  }

  #[test]
  fn vtt_region_round_trip() {
    let r = VttRegion::try_new(Uuid7::new(), Uuid7::new(), "footer")
      .unwrap()
      .with_width(80.0)
      .with_lines(2);
    let row: MySqlSubtitleTrackVttRegionRow = (&r).into();
    let r2 = vtt_region_from_row(row).unwrap();
    assert_eq!(r, r2);
  }

  #[test]
  fn vtt_style_round_trip() {
    let s = VttStyleBlock::try_new(Uuid7::new(), Uuid7::new(), 0, "::cue { color: red }").unwrap();
    let row: MySqlSubtitleTrackVttStyleRow = (&s).into();
    let s2 = vtt_style_from_row(row).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn ass_style_round_trip() {
    let s = AssStyle::try_new(Uuid7::new(), Uuid7::new(), "Default")
      .unwrap()
      .with_fontname("Arial")
      .with_bold();
    let row: MySqlSubtitleTrackAssStyleRow = (&s).into();
    let s2 = ass_style_from_row(row).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn lrc_metadata_round_trip() {
    let m = LrcMetadata::try_new(Uuid7::new())
      .unwrap()
      .with_title("Song")
      .with_offset_ms(-500);
    let row: MySqlSubtitleTrackLrcMetadataRow = (&m).into();
    let m2 = lrc_metadata_from_row(row).unwrap();
    assert_eq!(m, m2);
  }

  #[test]
  fn subtitle_facet_ref_roundtrip() {
    let s = Subtitle::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_track_progress(IndexProgress::try_new(3, 2, 1).unwrap());
    let row: MySqlSubtitleRow = (&s).into();
    let s2: Subtitle<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(s.id_ref(), s2.id_ref());
    assert_eq!(s.media_id_ref(), s2.media_id_ref());
    assert_eq!(s2.track_progress_ref().total(), 3);
  }

  #[test]
  fn subtitle_track_ref_roundtrip() {
    let en = Language::from_bcp47("en").unwrap();
    let mut bytes = [0u8; 32];
    bytes[0] = 1;
    let cs = FileChecksum::from_bytes(bytes);
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(SubtitleCodec::Subrip)
      .with_format(Format::Srt)
      .with_origin(TrackOrigin::External)
      .with_language(en)
      .with_title("English (SDH)")
      .with_disposition(TrackDisposition::from_u32(0x0040))
      .with_primary(true)
      .with_auto_selected(true)
      .with_duration(Some(Timestamp::new(120_000, tb())))
      .with_cue_count(42)
      .with_provenance(Provenance::from_parts(
        "tesseract",
        "5.3.0",
        "p1",
        "indexer-0.4.2",
      ))
      .with_source_checksum(Some(cs))
      .with_character_encoding("UTF-8")
      .with_bom_present(true)
      .with_sdh(true)
      .with_translation(true)
      .with_kind(SubtitleKind::ForcedNarrative)
      .with_coverage_ratio(Some(0.97))
      .with_first_cue(Some(Timestamp::new(500, tb())))
      .with_last_cue(Some(Timestamp::new(119_500, tb())))
      .with_index_status(
        SubtitleIndexStatus::TRACKS_DISCOVERED | SubtitleIndexStatus::CUES_EXTRACTED,
      )
      .with_index_errors(std::vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad")]);
    let (row, errs): (
      MySqlSubtitleTrackRow,
      std::vec::Vec<MySqlSubtitleTrackIndexErrorRow>,
    ) = (&t).into();
    let err_refs: std::vec::Vec<MySqlSubtitleTrackIndexErrorRowRef<'_>> = errs
      .iter()
      .map(MySqlSubtitleTrackIndexErrorRow::as_ref)
      .collect();
    let t2: SubtitleTrack<Uuid7> = (row.as_ref(), err_refs).try_into().unwrap();
    assert_eq!(t, t2);
  }

  // ---------------------------------------------------------------------------
  // Polymorphic subtitle_cue *_ref_roundtrip tests (mysql)
  // ---------------------------------------------------------------------------

  #[test]
  fn srt_cue_ref_roundtrip() {
    let c = SrtCue::try_new_srt(
      Uuid7::new(),
      Uuid7::new(),
      3,
      TimeRange::new(1_000, 2_000, tb()),
      LocalizedText::from_src_translated("Hola", "Hello"),
    )
    .unwrap();
    let base: MySqlSubtitleCueBaseRow = (&c).into();
    let c2 = srt_cue_from_row_ref(base.as_ref(), tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn vtt_cue_ref_roundtrip() {
    let d = VttData::<Uuid7>::new()
      .with_cue_identifier("c1")
      .with_vertical(VttVertical::Rl)
      .with_line_value("50%")
      .with_line_align(VttLineAlign::Center)
      .with_position_value("50%")
      .with_position_align(VttPositionAlign::Center)
      .with_size_value(80.0)
      .with_text_align(VttTextAlign::Start)
      .with_region_id(Uuid7::new())
      .with_voice("Speaker A")
      .with_styled_text("<b>hi</b>");
    let c: VttCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      1,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("hi"),
      d,
    )
    .unwrap();
    let (base, detail): (MySqlSubtitleCueBaseRow, MySqlSubtitleCueVttRow) = (&c).into();
    let c2 = vtt_cue_from_row_refs(base.as_ref(), detail.as_ref(), tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn ass_cue_ref_roundtrip() {
    let d = AssData::<Uuid7>::new(Uuid7::new())
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
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::new(),
      d,
    )
    .unwrap();
    let (base, detail): (MySqlSubtitleCueBaseRow, MySqlSubtitleCueAssRow) = (&c).into();
    let c2 = ass_cue_from_row_refs(base.as_ref(), detail.as_ref(), tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn lrc_cue_ref_roundtrip() {
    let c: LrcCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("la la"),
      LrcData::new().with_word_timing(),
    )
    .unwrap();
    let (base, detail): (MySqlSubtitleCueBaseRow, MySqlSubtitleCueLrcRow) = (&c).into();
    let c2 = lrc_cue_from_row_refs(base.as_ref(), detail.as_ref(), tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn lrc_word_ref_roundtrip() {
    let w = LrcWord::try_new(Uuid7::new(), 3, "ma", 1234).unwrap();
    let row: MySqlSubtitleCueLrcWordRow = (&w).into();
    let w2 = lrc_word_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(w, w2);
  }

  #[test]
  fn vtt_region_ref_roundtrip() {
    let r = VttRegion::try_new(Uuid7::new(), Uuid7::new(), "footer")
      .unwrap()
      .with_width(80.0)
      .with_lines(2)
      .with_region_anchor(50.0, 100.0)
      .with_viewport_anchor(50.0, 90.0)
      .with_scroll_up();
    let row: MySqlSubtitleTrackVttRegionRow = (&r).into();
    let r2 = vtt_region_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(r, r2);
  }

  #[test]
  fn vtt_style_ref_roundtrip() {
    let s = VttStyleBlock::try_new(Uuid7::new(), Uuid7::new(), 0, "::cue { color: red }").unwrap();
    let row: MySqlSubtitleTrackVttStyleRow = (&s).into();
    let s2 = vtt_style_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn ass_style_ref_roundtrip() {
    let s = AssStyle::try_new(Uuid7::new(), Uuid7::new(), "Default")
      .unwrap()
      .with_fontname("Arial")
      .with_fontsize(48.0)
      .with_bold()
      .with_outline(2.5);
    let row: MySqlSubtitleTrackAssStyleRow = (&s).into();
    let s2 = ass_style_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn lrc_metadata_ref_roundtrip() {
    let m = LrcMetadata::try_new(Uuid7::new())
      .unwrap()
      .with_title("Song")
      .with_artist("Band")
      .with_offset_ms(-500);
    let row: MySqlSubtitleTrackLrcMetadataRow = (&m).into();
    let m2 = lrc_metadata_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(m, m2);
  }
}
