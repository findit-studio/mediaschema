//! SQLite row shapes for the subtitle-cluster aggregates: the `Subtitle`
//! facet, `SubtitleTrack`, and `SubtitleCue`.
//!
//! UUIDs ride as 16-byte `BLOB`s. Integer-affinity columns (including
//! booleans, stored as `0`/`1`) ride as `i64`. Nested value-objects are
//! flattened into real columns; `Option<VO>` rides as a discriminating
//! column plus all-NULL payload columns when absent. Open descriptor
//! enums (`SubtitleCodec`, `Format`) ride as `TEXT` slugs; the closed
//! coded enum (`TrackOrigin`) and bitflags (`SubtitleIndexStatus` /
//! `TrackDisposition`) ride as integers. `Language` flattens to a BCP-47
//! `TEXT` column. Media-time values flatten to a PTS `INTEGER` +
//! timebase num/den.
//!
//! Collections ride in a child table (`subtitle_track_index_error`) with
//! an `ordinal` order column. The `Vec<Id>` reverse-FK fields are NOT
//! stored.

use indexmap::IndexMap;
use mediaframe::{
  codec::SubtitleCodec,
  disposition::TrackDisposition,
  lang::Language,
  subtitle::{Format, TrackOrigin},
};
use smol_str::SmolStr;

use crate::{
  domain::{
    aggregates::subtitle::{
      AssCue, AssData, AssStyle, Cea608Cue, Cea608Data, EbuStlCue, EbuStlData, LrcCue, LrcData,
      LrcMetadata, LrcWord, MicroDvdCue, MicroDvdData, PgsCue, PgsData, SamiCue, SamiData,
      SamiStyle, SbvCue, SbvData, SrtCue, SrtData, SubViewerCue, SubViewerData, SubtitleCueError,
      SubtitleCueKind, SubtitleError, SubtitleTrackError, TtmlCue, TtmlData, TtmlRegion, TtmlStyle,
      VobSubCue, VobSubData, VobSubPalette, VttCue, VttData, VttLineAlign, VttPositionAlign,
      VttRegion, VttStyleBlock, VttTextAlign, VttVertical,
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

use bytes::Bytes;

// ---------------------------------------------------------------------------
// SubtitleKind — closed enum, rides as a small integer
// ---------------------------------------------------------------------------

fn kind_to_i64(k: SubtitleKind) -> i64 {
  match k {
    SubtitleKind::FullDialogue => 0,
    SubtitleKind::ForcedNarrative => 1,
    SubtitleKind::CommentaryText => 2,
  }
}

fn kind_from_i64(n: i64) -> Result<SubtitleKind, SqlxError> {
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

/// SQLite row shape for the [`Subtitle`] facet.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleRow {
  pub id: std::vec::Vec<u8>,
  pub media_id: std::vec::Vec<u8>,
  pub track_progress_total: i64,
  pub track_progress_indexed: i64,
  pub track_progress_failed: i64,
}

impl From<&Subtitle<Uuid7>> for SqliteSubtitleRow {
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

impl TryFrom<SqliteSubtitleRow> for Subtitle<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteSubtitleRow) -> Result<Self, Self::Error> {
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

/// SQLite row shape for [`SubtitleTrack`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackRow {
  pub id: std::vec::Vec<u8>,
  pub subtitle_id: std::vec::Vec<u8>,
  pub stream_index: Option<i64>,
  pub container_track_id: Option<i64>,
  pub codec: String,
  pub format: String,
  pub origin: i64,
  pub language: Option<String>,
  pub title: String,
  pub disposition: i64,
  pub is_primary: i64,
  pub auto_selected: i64,
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
  pub bom_present: i64,
  pub is_sdh: i64,
  pub is_closed_caption: i64,
  pub is_translation: i64,
  pub kind: i64,
  pub coverage_ratio: Option<f32>,
  pub is_empty: i64,
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
pub struct SqliteSubtitleTrackIndexErrorRow {
  pub subtitle_track_id: std::vec::Vec<u8>,
  pub ordinal: i64,
  pub code: i64,
  pub message: String,
}

/// SQLite row for `subtitle_track_metadata`. Position in the per-
/// `subtitle_track_id` `ordinal` sequence IS the [`IndexMap`] insertion
/// order. `subtitle_track_from_rows` sorts by `ordinal` on decode.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackMetadataRow {
  pub subtitle_track_id: std::vec::Vec<u8>,
  pub ordinal: i64,
  pub key: String,
  pub value: String,
}

impl From<&SubtitleTrack<Uuid7>>
  for (
    SqliteSubtitleTrackRow,
    std::vec::Vec<SqliteSubtitleTrackIndexErrorRow>,
    std::vec::Vec<SqliteSubtitleTrackMetadataRow>,
  )
{
  fn from(t: &SubtitleTrack<Uuid7>) -> Self {
    let id = t.id_ref().as_bytes().to_vec();
    let prov = t.provenance_ref();
    let duration = t.duration_ref();
    let first_cue = t.first_cue_ref();
    let last_cue = t.last_cue_ref();
    let row = SqliteSubtitleTrackRow {
      id: id.clone(),
      subtitle_id: t.subtitle_id_ref().as_bytes().to_vec(),
      stream_index: t.stream_index().map(i64::from),
      container_track_id: t.container_track_id().map(|v| v as i64),
      codec: t.codec_ref().as_str().to_owned(),
      format: t.format_ref().as_str().to_owned(),
      origin: i64::from(t.origin_ref().to_u32()),
      language: language_to_slug(t.language_ref()),
      title: t.title().to_owned(),
      disposition: i64::from(t.disposition().bits()),
      is_primary: i64::from(t.is_primary()),
      auto_selected: i64::from(t.auto_selected()),
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
      bom_present: i64::from(t.bom_present()),
      is_sdh: i64::from(t.is_sdh()),
      is_closed_caption: i64::from(t.is_closed_caption()),
      is_translation: i64::from(t.is_translation()),
      kind: kind_to_i64(t.kind()),
      coverage_ratio: t.coverage_ratio(),
      is_empty: i64::from(t.is_empty()),
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
      .map(|(i, e)| SqliteSubtitleTrackIndexErrorRow {
        subtitle_track_id: id.clone(),
        ordinal: i as i64,
        code: i64::from(e.code().as_u32()),
        message: e.message().to_owned(),
      })
      .collect();
    let metadata = t
      .metadata_ref()
      .iter()
      .enumerate()
      .map(|(i, (k, v))| SqliteSubtitleTrackMetadataRow {
        subtitle_track_id: id.clone(),
        ordinal: i as i64,
        key: k.as_str().to_owned(),
        value: v.as_str().to_owned(),
      })
      .collect();
    (row, errors, metadata)
  }
}

impl
  TryFrom<(
    SqliteSubtitleTrackRow,
    std::vec::Vec<SqliteSubtitleTrackIndexErrorRow>,
    std::vec::Vec<SqliteSubtitleTrackMetadataRow>,
  )> for SubtitleTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, errors, metadata): (
      SqliteSubtitleTrackRow,
      std::vec::Vec<SqliteSubtitleTrackIndexErrorRow>,
      std::vec::Vec<SqliteSubtitleTrackMetadataRow>,
    ),
  ) -> Result<Self, Self::Error> {
    subtitle_track_from_rows(r, errors, metadata)
  }
}

/// Reconstruct a [`SubtitleTrack`] from its row, `index_errors` rows,
/// and `metadata` rows.
pub fn subtitle_track_from_rows(
  r: SqliteSubtitleTrackRow,
  mut errors: std::vec::Vec<SqliteSubtitleTrackIndexErrorRow>,
  mut metadata: std::vec::Vec<SqliteSubtitleTrackMetadataRow>,
) -> Result<SubtitleTrack<Uuid7>, SqlxError> {
  {
    let id = bytes_to_uuid7(&r.id)?;
    let subtitle_id = bytes_to_uuid7(&r.subtitle_id)?;
    let mut t = SubtitleTrack::try_new(id, subtitle_id)
      .map_err(|e: SubtitleTrackError| SqlxError::DomainConstructorRejected(e.to_string()))?;

    t = t
      .with_codec(parse_subtitle_codec(&r.codec))
      .with_format(parse_subtitle_format(&r.format))
      .with_origin(
        TrackOrigin::try_from_u32(u32_from_i64(r.origin, "SubtitleTrack.origin")?)
          .ok_or_else(|| SqlxError::UnknownDiscriminant(format!("TrackOrigin: {}", r.origin)))?,
      )
      .with_title(r.title)
      .with_disposition(TrackDisposition::from_bits_truncate(u32_from_i64(
        r.disposition,
        "SubtitleTrack.disposition",
      )?))
      .with_primary(r.is_primary != 0)
      .with_auto_selected(r.auto_selected != 0)
      .with_cue_count(u32_from_i64(r.cue_count, "SubtitleTrack.cue_count")?)
      .with_stream_index(opt_u32(r.stream_index, "SubtitleTrack.stream_index")?)
      .with_container_track_id(r.container_track_id.map(|v| v as u64))
      .with_character_encoding(r.character_encoding)
      .with_bom_present(r.bom_present != 0)
      .with_sdh(r.is_sdh != 0)
      .with_closed_caption(r.is_closed_caption != 0)
      .with_translation(r.is_translation != 0)
      .with_kind(kind_from_i64(r.kind)?)
      .with_coverage_ratio(r.coverage_ratio)
      .with_empty(r.is_empty != 0);

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
      let code = u32_from_i64(e.code, "SubtitleTrack.index_error.code")?;
      infos.push(ErrorInfo::new(ErrorCode::from_u32(code), e.message));
    }
    t = t.with_index_errors(infos);

    metadata.sort_by_key(|m| m.ordinal);
    let mut bag = IndexMap::with_capacity(metadata.len());
    for entry in metadata {
      if entry.subtitle_track_id != r.id {
        return Err(SqlxError::DomainConstructorRejected(
          "subtitle_track_metadata.subtitle_track_id does not match parent subtitle_track.id"
            .to_owned(),
        ));
      }
      bag.insert(SmolStr::from(entry.key), SmolStr::from(entry.value));
    }
    t = t.with_metadata(bag);

    Ok(t)
  }
}

// ===========================================================================
// SubtitleCue — polymorphic base + per-format detail + per-track aggregates
// ===========================================================================

/// SQLite row shape for the base [`SubtitleCue`] table.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueBaseRow {
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
) -> SqliteSubtitleCueBaseRow {
  let span = c.span_ref();
  SqliteSubtitleCueBaseRow {
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
  r: &SqliteSubtitleCueBaseRow,
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
  let span = mediatime::TimeRange::try_new(r.span_start_pts, r.span_end_pts, parent_timebase)
    .ok_or_else(|| {
      SqlxError::DomainConstructorRejected(format!(
        "TimeRange start_pts ({}) must be <= end_pts ({})",
        r.span_start_pts, r.span_end_pts
      ))
    })?;
  let text = LocalizedText::from_src_translated(r.text_src.clone(), r.text_translated.clone());
  let kind = cue_kind_from_i64_v(r.kind)?;
  Ok((id, subtitle_track_id, ordinal, span, text, kind))
}

// --- SRT ---------------------------------------------------------------------

impl From<&SrtCue<Uuid7>> for SqliteSubtitleCueBaseRow {
  fn from(c: &SrtCue<Uuid7>) -> Self {
    base_row_from_cue(c, SubtitleCueKind::Srt)
  }
}

/// Rebuild a SubRip cue from its base row.
pub fn srt_cue_from_row(
  base: SqliteSubtitleCueBaseRow,
  parent_timebase: mediatime::Timebase,
) -> Result<SrtCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Srt {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Srt cue kind, got {kind:?}"
    )));
  }
  SrtCue::try_new(id, subtitle_track_id, ordinal, span, text, SrtData)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- WebVTT ------------------------------------------------------------------

/// SQLite detail row for a WebVTT cue.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteSubtitleCueVttRow {
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

impl From<&VttCue<Uuid7>> for (SqliteSubtitleCueBaseRow, SqliteSubtitleCueVttRow) {
  fn from(c: &VttCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Vtt);
    let d = c.data_ref();
    let detail = SqliteSubtitleCueVttRow {
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
  base: SqliteSubtitleCueBaseRow,
  detail: SqliteSubtitleCueVttRow,
  parent_timebase: mediatime::Timebase,
) -> Result<VttCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
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

/// SQLite detail row for an ASS/SSA cue.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueAssRow {
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

impl From<&AssCue<Uuid7>> for (SqliteSubtitleCueBaseRow, SqliteSubtitleCueAssRow) {
  fn from(c: &AssCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Ass);
    let d = c.data_ref();
    let detail = SqliteSubtitleCueAssRow {
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
  base: SqliteSubtitleCueBaseRow,
  detail: SqliteSubtitleCueAssRow,
  parent_timebase: mediatime::Timebase,
) -> Result<AssCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
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

/// SQLite detail row for an LRC cue.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueLrcRow {
  pub id: std::vec::Vec<u8>,
  pub has_word_timing: bool,
}

impl From<&LrcCue<Uuid7>> for (SqliteSubtitleCueBaseRow, SqliteSubtitleCueLrcRow) {
  fn from(c: &LrcCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Lrc);
    let detail = SqliteSubtitleCueLrcRow {
      id: base.id.clone(),
      has_word_timing: c.data_ref().has_word_timing(),
    };
    (base, detail)
  }
}

/// Rebuild an LRC cue from its (base, detail) rows.
pub fn lrc_cue_from_rows(
  base: SqliteSubtitleCueBaseRow,
  detail: SqliteSubtitleCueLrcRow,
  parent_timebase: mediatime::Timebase,
) -> Result<LrcCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
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
pub struct SqliteSubtitleCueLrcWordRow {
  pub subtitle_cue_id: std::vec::Vec<u8>,
  pub ordinal: i64,
  pub text: String,
  pub start_pts: i64,
}

impl From<&LrcWord<Uuid7>> for SqliteSubtitleCueLrcWordRow {
  fn from(w: &LrcWord<Uuid7>) -> Self {
    Self {
      subtitle_cue_id: w.subtitle_cue_id_ref().as_bytes().to_vec(),
      ordinal: i64::from(w.ordinal()),
      text: w.text().to_owned(),
      start_pts: w.start_pts(),
    }
  }
}

pub fn lrc_word_from_row(r: SqliteSubtitleCueLrcWordRow) -> Result<LrcWord<Uuid7>, SqlxError> {
  let subtitle_cue_id = bytes_to_uuid7(&r.subtitle_cue_id)?;
  let ordinal = u32_from_i64(r.ordinal, "LrcWord.ordinal")?;
  LrcWord::try_new(subtitle_cue_id, ordinal, r.text, r.start_pts)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- MicroDVD ----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueMicroDvdRow {
  pub id: std::vec::Vec<u8>,
  pub styled_text: String,
}

impl From<&MicroDvdCue<Uuid7>> for (SqliteSubtitleCueBaseRow, SqliteSubtitleCueMicroDvdRow) {
  fn from(c: &MicroDvdCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::MicroDvd);
    let detail = SqliteSubtitleCueMicroDvdRow {
      id: base.id.clone(),
      styled_text: c.data_ref().styled_text().to_owned(),
    };
    (base, detail)
  }
}

pub fn micro_dvd_cue_from_rows(
  base: SqliteSubtitleCueBaseRow,
  detail: SqliteSubtitleCueMicroDvdRow,
  parent_timebase: mediatime::Timebase,
) -> Result<MicroDvdCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::MicroDvd {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected MicroDvd cue kind, got {kind:?}"
    )));
  }
  let d = MicroDvdData::new(detail.styled_text);
  MicroDvdCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- SubViewer ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueSubViewerRow {
  pub id: std::vec::Vec<u8>,
  pub styled_text: String,
}

impl From<&SubViewerCue<Uuid7>> for (SqliteSubtitleCueBaseRow, SqliteSubtitleCueSubViewerRow) {
  fn from(c: &SubViewerCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::SubViewer);
    let detail = SqliteSubtitleCueSubViewerRow {
      id: base.id.clone(),
      styled_text: c.data_ref().styled_text().to_owned(),
    };
    (base, detail)
  }
}

pub fn sub_viewer_cue_from_rows(
  base: SqliteSubtitleCueBaseRow,
  detail: SqliteSubtitleCueSubViewerRow,
  parent_timebase: mediatime::Timebase,
) -> Result<SubViewerCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::SubViewer {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected SubViewer cue kind, got {kind:?}"
    )));
  }
  let d = SubViewerData::new(detail.styled_text);
  SubViewerCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- SBV ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueSbvRow {
  pub id: std::vec::Vec<u8>,
}

impl From<&SbvCue<Uuid7>> for (SqliteSubtitleCueBaseRow, SqliteSubtitleCueSbvRow) {
  fn from(c: &SbvCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Sbv);
    let detail = SqliteSubtitleCueSbvRow {
      id: base.id.clone(),
    };
    (base, detail)
  }
}

pub fn sbv_cue_from_rows(
  base: SqliteSubtitleCueBaseRow,
  _detail: SqliteSubtitleCueSbvRow,
  parent_timebase: mediatime::Timebase,
) -> Result<SbvCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Sbv {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Sbv cue kind, got {kind:?}"
    )));
  }
  SbvCue::try_new(id, subtitle_track_id, ordinal, span, text, SbvData::new())
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- TTML --------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueTtmlRow {
  pub id: std::vec::Vec<u8>,
  pub region_id: Option<std::vec::Vec<u8>>,
  pub style_id: Option<std::vec::Vec<u8>>,
  pub xml_id: String,
  pub styled_text: String,
}

impl From<&TtmlCue<Uuid7>> for (SqliteSubtitleCueBaseRow, SqliteSubtitleCueTtmlRow) {
  fn from(c: &TtmlCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Ttml);
    let d = c.data_ref();
    let detail = SqliteSubtitleCueTtmlRow {
      id: base.id.clone(),
      region_id: d.region_id_ref().map(|id| id.as_bytes().to_vec()),
      style_id: d.style_id_ref().map(|id| id.as_bytes().to_vec()),
      xml_id: d.xml_id().to_owned(),
      styled_text: d.styled_text().to_owned(),
    };
    (base, detail)
  }
}

pub fn ttml_cue_from_rows(
  base: SqliteSubtitleCueBaseRow,
  detail: SqliteSubtitleCueTtmlRow,
  parent_timebase: mediatime::Timebase,
) -> Result<TtmlCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Ttml {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Ttml cue kind, got {kind:?}"
    )));
  }
  let region_id = match detail.region_id.as_ref() {
    None => None,
    Some(b) => Some(bytes_to_uuid7(b)?),
  };
  let style_id = match detail.style_id.as_ref() {
    None => None,
    Some(b) => Some(bytes_to_uuid7(b)?),
  };
  let d = TtmlData::<Uuid7>::new()
    .maybe_region_id(region_id)
    .maybe_style_id(style_id)
    .with_xml_id(detail.xml_id)
    .with_styled_text(detail.styled_text);
  TtmlCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- SAMI --------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueSamiRow {
  pub id: std::vec::Vec<u8>,
  pub class_name: String,
  pub styled_text: String,
}

impl From<&SamiCue<Uuid7>> for (SqliteSubtitleCueBaseRow, SqliteSubtitleCueSamiRow) {
  fn from(c: &SamiCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Sami);
    let d = c.data_ref();
    let detail = SqliteSubtitleCueSamiRow {
      id: base.id.clone(),
      class_name: d.class_name().to_owned(),
      styled_text: d.styled_text().to_owned(),
    };
    (base, detail)
  }
}

pub fn sami_cue_from_rows(
  base: SqliteSubtitleCueBaseRow,
  detail: SqliteSubtitleCueSamiRow,
  parent_timebase: mediatime::Timebase,
) -> Result<SamiCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Sami {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Sami cue kind, got {kind:?}"
    )));
  }
  let d = SamiData::new()
    .with_class_name(detail.class_name)
    .with_styled_text(detail.styled_text);
  SamiCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- VobSub ------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueVobSubRow {
  pub id: std::vec::Vec<u8>,
  pub palette_id: std::vec::Vec<u8>,
  pub bitmap: std::vec::Vec<u8>,
  pub width: i64,
  pub height: i64,
  pub pos_x: i64,
  pub pos_y: i64,
  pub color_indices: i64,
  pub contrast_indices: i64,
}

fn pack_indices_i64(a: &[u8; 4]) -> i64 {
  (a[0] as i64) | ((a[1] as i64) << 8) | ((a[2] as i64) << 16) | ((a[3] as i64) << 24)
}

fn unpack_indices_i64(n: i64) -> Result<[u8; 4], SqlxError> {
  let v = u32::try_from(n)
    .map_err(|e| SqlxError::UnknownDiscriminant(format!("VobSub indices packing: {e}")))?;
  Ok([v as u8, (v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8])
}

fn i32_from_i64(v: i64, what: &str) -> Result<i32, SqlxError> {
  i32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
}

impl From<&VobSubCue<Uuid7>> for (SqliteSubtitleCueBaseRow, SqliteSubtitleCueVobSubRow) {
  fn from(c: &VobSubCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::VobSub);
    let d = c.data_ref();
    let detail = SqliteSubtitleCueVobSubRow {
      id: base.id.clone(),
      palette_id: d.palette_id_ref().as_bytes().to_vec(),
      bitmap: d.bitmap_ref().to_vec(),
      width: i64::from(d.width()),
      height: i64::from(d.height()),
      pos_x: i64::from(d.pos_x()),
      pos_y: i64::from(d.pos_y()),
      color_indices: pack_indices_i64(d.color_indices()),
      contrast_indices: pack_indices_i64(d.contrast_indices()),
    };
    (base, detail)
  }
}

pub fn vob_sub_cue_from_rows(
  base: SqliteSubtitleCueBaseRow,
  detail: SqliteSubtitleCueVobSubRow,
  parent_timebase: mediatime::Timebase,
) -> Result<VobSubCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::VobSub {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected VobSub cue kind, got {kind:?}"
    )));
  }
  let palette_id = bytes_to_uuid7(&detail.palette_id)?;
  let d = VobSubData::<Uuid7>::new(palette_id)
    .with_bitmap(Bytes::from(detail.bitmap))
    .with_width(u32_from_i64(detail.width, "VobSubData.width")?)
    .with_height(u32_from_i64(detail.height, "VobSubData.height")?)
    .with_pos(
      i32_from_i64(detail.pos_x, "VobSubData.pos_x")?,
      i32_from_i64(detail.pos_y, "VobSubData.pos_y")?,
    )
    .with_color_indices(unpack_indices_i64(detail.color_indices)?)
    .with_contrast_indices(unpack_indices_i64(detail.contrast_indices)?);
  VobSubCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- PGS ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCuePgsRow {
  pub id: std::vec::Vec<u8>,
  pub bitmap: std::vec::Vec<u8>,
  pub width: i64,
  pub height: i64,
  pub pos_x: i64,
  pub pos_y: i64,
  pub palette_bytes: std::vec::Vec<u8>,
  pub composition_state: i64,
}

impl From<&PgsCue<Uuid7>> for (SqliteSubtitleCueBaseRow, SqliteSubtitleCuePgsRow) {
  fn from(c: &PgsCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Pgs);
    let d = c.data_ref();
    let detail = SqliteSubtitleCuePgsRow {
      id: base.id.clone(),
      bitmap: d.bitmap_ref().to_vec(),
      width: i64::from(d.width()),
      height: i64::from(d.height()),
      pos_x: i64::from(d.pos_x()),
      pos_y: i64::from(d.pos_y()),
      palette_bytes: d.palette_bytes_ref().to_vec(),
      composition_state: i64::from(d.composition_state()),
    };
    (base, detail)
  }
}

pub fn pgs_cue_from_rows(
  base: SqliteSubtitleCueBaseRow,
  detail: SqliteSubtitleCuePgsRow,
  parent_timebase: mediatime::Timebase,
) -> Result<PgsCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Pgs {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Pgs cue kind, got {kind:?}"
    )));
  }
  let composition_state = u8::try_from(detail.composition_state)
    .map_err(|e| SqlxError::UnknownDiscriminant(format!("PgsData.composition_state: {e}")))?;
  let d = PgsData::new()
    .with_bitmap(Bytes::from(detail.bitmap))
    .with_palette_bytes(Bytes::from(detail.palette_bytes))
    .with_width(u32_from_i64(detail.width, "PgsData.width")?)
    .with_height(u32_from_i64(detail.height, "PgsData.height")?)
    .with_pos(
      i32_from_i64(detail.pos_x, "PgsData.pos_x")?,
      i32_from_i64(detail.pos_y, "PgsData.pos_y")?,
    )
    .with_composition_state(composition_state);
  PgsCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- CEA-608 -----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueCea608Row {
  pub id: std::vec::Vec<u8>,
  pub channel: i64,
  pub pac_byte_pair: i64,
  pub styled_text: String,
}

impl From<&Cea608Cue<Uuid7>> for (SqliteSubtitleCueBaseRow, SqliteSubtitleCueCea608Row) {
  fn from(c: &Cea608Cue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Cea608);
    let d = c.data_ref();
    let detail = SqliteSubtitleCueCea608Row {
      id: base.id.clone(),
      channel: i64::from(d.channel()),
      pac_byte_pair: i64::from(d.pac_byte_pair()),
      styled_text: d.styled_text().to_owned(),
    };
    (base, detail)
  }
}

pub fn cea_608_cue_from_rows(
  base: SqliteSubtitleCueBaseRow,
  detail: SqliteSubtitleCueCea608Row,
  parent_timebase: mediatime::Timebase,
) -> Result<Cea608Cue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Cea608 {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Cea608 cue kind, got {kind:?}"
    )));
  }
  let channel = u8::try_from(detail.channel)
    .map_err(|e| SqlxError::UnknownDiscriminant(format!("Cea608Data.channel: {e}")))?;
  let pac = u32::try_from(detail.pac_byte_pair)
    .map_err(|e| SqlxError::UnknownDiscriminant(format!("Cea608Data.pac_byte_pair: {e}")))?;
  let d = Cea608Data::try_new(channel)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_pac_byte_pair(pac)
    .with_styled_text(detail.styled_text);
  Cea608Cue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- EBU STL -----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueEbuStlRow {
  pub id: std::vec::Vec<u8>,
  pub subtitle_number: i64,
  pub cumulative: bool,
  pub vertical_pos: i64,
  pub justification: i64,
  pub styled_text: String,
}

impl From<&EbuStlCue<Uuid7>> for (SqliteSubtitleCueBaseRow, SqliteSubtitleCueEbuStlRow) {
  fn from(c: &EbuStlCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::EbuStl);
    let d = c.data_ref();
    let detail = SqliteSubtitleCueEbuStlRow {
      id: base.id.clone(),
      subtitle_number: i64::from(d.subtitle_number()),
      cumulative: d.cumulative(),
      vertical_pos: i64::from(d.vertical_pos()),
      justification: i64::from(d.justification()),
      styled_text: d.styled_text().to_owned(),
    };
    (base, detail)
  }
}

pub fn ebu_stl_cue_from_rows(
  base: SqliteSubtitleCueBaseRow,
  detail: SqliteSubtitleCueEbuStlRow,
  parent_timebase: mediatime::Timebase,
) -> Result<EbuStlCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::EbuStl {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected EbuStl cue kind, got {kind:?}"
    )));
  }
  let justification = u8::try_from(detail.justification)
    .map_err(|e| SqlxError::UnknownDiscriminant(format!("EbuStlData.justification: {e}")))?;
  let subtitle_number = u32_from_i64(detail.subtitle_number, "EbuStlData.subtitle_number")?;
  let vertical_pos = i32_from_i64(detail.vertical_pos, "EbuStlData.vertical_pos")?;
  let d = EbuStlData::try_new(justification)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_subtitle_number(subtitle_number)
    .maybe_cumulative(detail.cumulative)
    .with_vertical_pos(vertical_pos)
    .with_styled_text(detail.styled_text);
  EbuStlCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// ===========================================================================
// Per-track aggregates
// ===========================================================================

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackVttRegionRow {
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

impl From<&VttRegion<Uuid7>> for SqliteSubtitleTrackVttRegionRow {
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
  r: SqliteSubtitleTrackVttRegionRow,
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
pub struct SqliteSubtitleTrackVttStyleRow {
  pub id: std::vec::Vec<u8>,
  pub subtitle_track_id: std::vec::Vec<u8>,
  pub ordinal: i64,
  pub css_text: String,
}

impl From<&VttStyleBlock<Uuid7>> for SqliteSubtitleTrackVttStyleRow {
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
  r: SqliteSubtitleTrackVttStyleRow,
) -> Result<VttStyleBlock<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(&r.id)?;
  let subtitle_track_id = bytes_to_uuid7(&r.subtitle_track_id)?;
  let ordinal = u32_from_i64(r.ordinal, "VttStyleBlock.ordinal")?;
  VttStyleBlock::try_new(id, subtitle_track_id, ordinal, r.css_text)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackAssStyleRow {
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

impl From<&AssStyle<Uuid7>> for SqliteSubtitleTrackAssStyleRow {
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

pub fn ass_style_from_row(r: SqliteSubtitleTrackAssStyleRow) -> Result<AssStyle<Uuid7>, SqlxError> {
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
pub struct SqliteSubtitleTrackLrcMetadataRow {
  pub subtitle_track_id: std::vec::Vec<u8>,
  pub title: String,
  pub artist: String,
  pub album: String,
  pub author: String,
  pub creator: String,
  pub length: String,
  pub offset_ms: i64,
}

impl From<&LrcMetadata<Uuid7>> for SqliteSubtitleTrackLrcMetadataRow {
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
  r: SqliteSubtitleTrackLrcMetadataRow,
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

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackTtmlRegionRow {
  pub id: std::vec::Vec<u8>,
  pub subtitle_track_id: std::vec::Vec<u8>,
  pub xml_id: String,
  pub xml_attrs: String,
}

impl From<&TtmlRegion<Uuid7>> for SqliteSubtitleTrackTtmlRegionRow {
  fn from(r: &TtmlRegion<Uuid7>) -> Self {
    Self {
      id: r.id_ref().as_bytes().to_vec(),
      subtitle_track_id: r.subtitle_track_id_ref().as_bytes().to_vec(),
      xml_id: r.xml_id().to_owned(),
      xml_attrs: r.xml_attrs().to_owned(),
    }
  }
}

pub fn ttml_region_from_row(
  r: SqliteSubtitleTrackTtmlRegionRow,
) -> Result<TtmlRegion<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(&r.id)?;
  let subtitle_track_id = bytes_to_uuid7(&r.subtitle_track_id)?;
  Ok(
    TtmlRegion::try_new(id, subtitle_track_id, r.xml_id)
      .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_xml_attrs(r.xml_attrs),
  )
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackTtmlStyleRow {
  pub id: std::vec::Vec<u8>,
  pub subtitle_track_id: std::vec::Vec<u8>,
  pub xml_id: String,
  pub xml_attrs: String,
}

impl From<&TtmlStyle<Uuid7>> for SqliteSubtitleTrackTtmlStyleRow {
  fn from(s: &TtmlStyle<Uuid7>) -> Self {
    Self {
      id: s.id_ref().as_bytes().to_vec(),
      subtitle_track_id: s.subtitle_track_id_ref().as_bytes().to_vec(),
      xml_id: s.xml_id().to_owned(),
      xml_attrs: s.xml_attrs().to_owned(),
    }
  }
}

pub fn ttml_style_from_row(
  r: SqliteSubtitleTrackTtmlStyleRow,
) -> Result<TtmlStyle<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(&r.id)?;
  let subtitle_track_id = bytes_to_uuid7(&r.subtitle_track_id)?;
  Ok(
    TtmlStyle::try_new(id, subtitle_track_id, r.xml_id)
      .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_xml_attrs(r.xml_attrs),
  )
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackSamiStyleRow {
  pub id: std::vec::Vec<u8>,
  pub subtitle_track_id: std::vec::Vec<u8>,
  pub class_name: String,
  pub css_text: String,
}

impl From<&SamiStyle<Uuid7>> for SqliteSubtitleTrackSamiStyleRow {
  fn from(s: &SamiStyle<Uuid7>) -> Self {
    Self {
      id: s.id_ref().as_bytes().to_vec(),
      subtitle_track_id: s.subtitle_track_id_ref().as_bytes().to_vec(),
      class_name: s.class_name().to_owned(),
      css_text: s.css_text().to_owned(),
    }
  }
}

pub fn sami_style_from_row(
  r: SqliteSubtitleTrackSamiStyleRow,
) -> Result<SamiStyle<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(&r.id)?;
  let subtitle_track_id = bytes_to_uuid7(&r.subtitle_track_id)?;
  Ok(
    SamiStyle::try_new(id, subtitle_track_id, r.class_name)
      .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_css_text(r.css_text),
  )
}

/// SQLite row for a [`VobSubPalette`]. SQLite has no native array
/// type; the 16-entry palette LUT serialises into 16 separate
/// `INTEGER` columns (`entry00 … entry15`).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackVobSubPaletteRow {
  pub id: std::vec::Vec<u8>,
  pub subtitle_track_id: std::vec::Vec<u8>,
  pub entry00: i64,
  pub entry01: i64,
  pub entry02: i64,
  pub entry03: i64,
  pub entry04: i64,
  pub entry05: i64,
  pub entry06: i64,
  pub entry07: i64,
  pub entry08: i64,
  pub entry09: i64,
  pub entry10: i64,
  pub entry11: i64,
  pub entry12: i64,
  pub entry13: i64,
  pub entry14: i64,
  pub entry15: i64,
}

impl From<&VobSubPalette<Uuid7>> for SqliteSubtitleTrackVobSubPaletteRow {
  fn from(p: &VobSubPalette<Uuid7>) -> Self {
    let e = p.entries();
    Self {
      id: p.id_ref().as_bytes().to_vec(),
      subtitle_track_id: p.subtitle_track_id_ref().as_bytes().to_vec(),
      entry00: i64::from(e[0]),
      entry01: i64::from(e[1]),
      entry02: i64::from(e[2]),
      entry03: i64::from(e[3]),
      entry04: i64::from(e[4]),
      entry05: i64::from(e[5]),
      entry06: i64::from(e[6]),
      entry07: i64::from(e[7]),
      entry08: i64::from(e[8]),
      entry09: i64::from(e[9]),
      entry10: i64::from(e[10]),
      entry11: i64::from(e[11]),
      entry12: i64::from(e[12]),
      entry13: i64::from(e[13]),
      entry14: i64::from(e[14]),
      entry15: i64::from(e[15]),
    }
  }
}

pub fn vob_sub_palette_from_row(
  r: SqliteSubtitleTrackVobSubPaletteRow,
) -> Result<VobSubPalette<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(&r.id)?;
  let subtitle_track_id = bytes_to_uuid7(&r.subtitle_track_id)?;
  let to_u32 = |v: i64, what: &str| -> Result<u32, SqlxError> {
    u32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
  };
  let entries: [u32; 16] = [
    to_u32(r.entry00, "VobSubPalette.entry00")?,
    to_u32(r.entry01, "VobSubPalette.entry01")?,
    to_u32(r.entry02, "VobSubPalette.entry02")?,
    to_u32(r.entry03, "VobSubPalette.entry03")?,
    to_u32(r.entry04, "VobSubPalette.entry04")?,
    to_u32(r.entry05, "VobSubPalette.entry05")?,
    to_u32(r.entry06, "VobSubPalette.entry06")?,
    to_u32(r.entry07, "VobSubPalette.entry07")?,
    to_u32(r.entry08, "VobSubPalette.entry08")?,
    to_u32(r.entry09, "VobSubPalette.entry09")?,
    to_u32(r.entry10, "VobSubPalette.entry10")?,
    to_u32(r.entry11, "VobSubPalette.entry11")?,
    to_u32(r.entry12, "VobSubPalette.entry12")?,
    to_u32(r.entry13, "VobSubPalette.entry13")?,
    to_u32(r.entry14, "VobSubPalette.entry14")?,
    to_u32(r.entry15, "VobSubPalette.entry15")?,
  ];
  Ok(
    VobSubPalette::try_new(id, subtitle_track_id)
      .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_entries(entries),
  )
}

// ===========================================================================
// Borrowed-view siblings (`*RowRef<'r>`) — zero-copy decode from `&'r Row`.
// ===========================================================================

/// Borrowed view of [`SqliteSubtitleRow`] — zero-copy decode from `&'r Row`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleRowRef<'r> {
  pub id: &'r [u8],
  pub media_id: &'r [u8],
  pub track_progress_total: i64,
  pub track_progress_indexed: i64,
  pub track_progress_failed: i64,
}

/// Borrowed view of [`SqliteSubtitleTrackRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackRowRef<'r> {
  pub id: &'r [u8],
  pub subtitle_id: &'r [u8],
  pub stream_index: Option<i64>,
  pub container_track_id: Option<i64>,
  pub codec: &'r str,
  pub format: &'r str,
  pub origin: i64,
  pub language: Option<&'r str>,
  pub title: &'r str,
  pub disposition: i64,
  pub is_primary: i64,
  pub auto_selected: i64,
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
  pub bom_present: i64,
  pub is_sdh: i64,
  pub is_closed_caption: i64,
  pub is_translation: i64,
  pub kind: i64,
  pub coverage_ratio: Option<f32>,
  pub is_empty: i64,
  pub first_cue_pts: Option<i64>,
  pub first_cue_tb_num: Option<i64>,
  pub first_cue_tb_den: Option<i64>,
  pub last_cue_pts: Option<i64>,
  pub last_cue_tb_num: Option<i64>,
  pub last_cue_tb_den: Option<i64>,
  pub index_status: i64,
}

/// Borrowed view of [`SqliteSubtitleTrackIndexErrorRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackIndexErrorRowRef<'r> {
  pub subtitle_track_id: &'r [u8],
  pub ordinal: i64,
  pub code: i64,
  pub message: &'r str,
}

/// Borrowed view of [`SqliteSubtitleTrackMetadataRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackMetadataRowRef<'r> {
  pub subtitle_track_id: &'r [u8],
  pub ordinal: i64,
  pub key: &'r str,
  pub value: &'r str,
}

// --- Polymorphic subtitle_cue tables ----------------------------------------
//
// SQLite stores ids as `BLOB` (Vec<u8> owned, &'r [u8] borrowed); only the
// variable-length text + BLOB-byte columns flip away from `Copy` types.

/// Borrowed view of [`SqliteSubtitleCueBaseRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueBaseRowRef<'r> {
  pub id: &'r [u8],
  pub subtitle_track_id: &'r [u8],
  pub ordinal: i64,
  pub span_start_pts: i64,
  pub span_end_pts: i64,
  pub text_src: &'r str,
  pub text_translated: &'r str,
  pub kind: i64,
}

/// Borrowed view of [`SqliteSubtitleCueVttRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteSubtitleCueVttRowRef<'r> {
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

/// Borrowed view of [`SqliteSubtitleCueAssRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueAssRowRef<'r> {
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

/// Borrowed view of [`SqliteSubtitleCueLrcRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueLrcRowRef<'r> {
  pub id: &'r [u8],
  pub has_word_timing: bool,
}

/// Borrowed view of [`SqliteSubtitleCueLrcWordRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueLrcWordRowRef<'r> {
  pub subtitle_cue_id: &'r [u8],
  pub ordinal: i64,
  pub text: &'r str,
  pub start_pts: i64,
}

/// Borrowed view of [`SqliteSubtitleTrackVttRegionRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackVttRegionRowRef<'r> {
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

/// Borrowed view of [`SqliteSubtitleTrackVttStyleRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackVttStyleRowRef<'r> {
  pub id: &'r [u8],
  pub subtitle_track_id: &'r [u8],
  pub ordinal: i64,
  pub css_text: &'r str,
}

/// Borrowed view of [`SqliteSubtitleTrackAssStyleRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackAssStyleRowRef<'r> {
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

/// Borrowed view of [`SqliteSubtitleTrackLrcMetadataRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackLrcMetadataRowRef<'r> {
  pub subtitle_track_id: &'r [u8],
  pub title: &'r str,
  pub artist: &'r str,
  pub album: &'r str,
  pub author: &'r str,
  pub creator: &'r str,
  pub length: &'r str,
  pub offset_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueMicroDvdRowRef<'r> {
  pub id: &'r [u8],
  pub styled_text: &'r str,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueSubViewerRowRef<'r> {
  pub id: &'r [u8],
  pub styled_text: &'r str,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueSbvRowRef<'r> {
  pub id: &'r [u8],
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueTtmlRowRef<'r> {
  pub id: &'r [u8],
  pub region_id: Option<&'r [u8]>,
  pub style_id: Option<&'r [u8]>,
  pub xml_id: &'r str,
  pub styled_text: &'r str,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueSamiRowRef<'r> {
  pub id: &'r [u8],
  pub class_name: &'r str,
  pub styled_text: &'r str,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueVobSubRowRef<'r> {
  pub id: &'r [u8],
  pub palette_id: &'r [u8],
  pub bitmap: &'r [u8],
  pub width: i64,
  pub height: i64,
  pub pos_x: i64,
  pub pos_y: i64,
  pub color_indices: i64,
  pub contrast_indices: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCuePgsRowRef<'r> {
  pub id: &'r [u8],
  pub bitmap: &'r [u8],
  pub width: i64,
  pub height: i64,
  pub pos_x: i64,
  pub pos_y: i64,
  pub palette_bytes: &'r [u8],
  pub composition_state: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueCea608RowRef<'r> {
  pub id: &'r [u8],
  pub channel: i64,
  pub pac_byte_pair: i64,
  pub styled_text: &'r str,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueEbuStlRowRef<'r> {
  pub id: &'r [u8],
  pub subtitle_number: i64,
  pub cumulative: bool,
  pub vertical_pos: i64,
  pub justification: i64,
  pub styled_text: &'r str,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackTtmlRegionRowRef<'r> {
  pub id: &'r [u8],
  pub subtitle_track_id: &'r [u8],
  pub xml_id: &'r str,
  pub xml_attrs: &'r str,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackTtmlStyleRowRef<'r> {
  pub id: &'r [u8],
  pub subtitle_track_id: &'r [u8],
  pub xml_id: &'r str,
  pub xml_attrs: &'r str,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackSamiStyleRowRef<'r> {
  pub id: &'r [u8],
  pub subtitle_track_id: &'r [u8],
  pub class_name: &'r str,
  pub css_text: &'r str,
}

/// Borrowed view of [`SqliteSubtitleTrackVobSubPaletteRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleTrackVobSubPaletteRowRef<'r> {
  pub id: &'r [u8],
  pub subtitle_track_id: &'r [u8],
  pub entry00: i64,
  pub entry01: i64,
  pub entry02: i64,
  pub entry03: i64,
  pub entry04: i64,
  pub entry05: i64,
  pub entry06: i64,
  pub entry07: i64,
  pub entry08: i64,
  pub entry09: i64,
  pub entry10: i64,
  pub entry11: i64,
  pub entry12: i64,
  pub entry13: i64,
  pub entry14: i64,
  pub entry15: i64,
}

impl SqliteSubtitleCueBaseRow {
  /// Cheap borrow — produces a [`SqliteSubtitleCueBaseRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSubtitleCueBaseRowRef<'_> {
    SqliteSubtitleCueBaseRowRef {
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

impl SqliteSubtitleCueVttRow {
  /// Cheap borrow — produces a [`SqliteSubtitleCueVttRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSubtitleCueVttRowRef<'_> {
    SqliteSubtitleCueVttRowRef {
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

impl SqliteSubtitleCueAssRow {
  /// Cheap borrow — produces a [`SqliteSubtitleCueAssRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSubtitleCueAssRowRef<'_> {
    SqliteSubtitleCueAssRowRef {
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

impl SqliteSubtitleCueLrcRow {
  /// Cheap borrow — produces a [`SqliteSubtitleCueLrcRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSubtitleCueLrcRowRef<'_> {
    SqliteSubtitleCueLrcRowRef {
      id: &self.id,
      has_word_timing: self.has_word_timing,
    }
  }
}

impl SqliteSubtitleCueLrcWordRow {
  /// Cheap borrow — produces a [`SqliteSubtitleCueLrcWordRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSubtitleCueLrcWordRowRef<'_> {
    SqliteSubtitleCueLrcWordRowRef {
      subtitle_cue_id: &self.subtitle_cue_id,
      ordinal: self.ordinal,
      text: &self.text,
      start_pts: self.start_pts,
    }
  }
}

impl SqliteSubtitleTrackVttRegionRow {
  /// Cheap borrow — produces a [`SqliteSubtitleTrackVttRegionRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSubtitleTrackVttRegionRowRef<'_> {
    SqliteSubtitleTrackVttRegionRowRef {
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

impl SqliteSubtitleTrackVttStyleRow {
  /// Cheap borrow — produces a [`SqliteSubtitleTrackVttStyleRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSubtitleTrackVttStyleRowRef<'_> {
    SqliteSubtitleTrackVttStyleRowRef {
      id: &self.id,
      subtitle_track_id: &self.subtitle_track_id,
      ordinal: self.ordinal,
      css_text: &self.css_text,
    }
  }
}

impl SqliteSubtitleTrackAssStyleRow {
  /// Cheap borrow — produces a [`SqliteSubtitleTrackAssStyleRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSubtitleTrackAssStyleRowRef<'_> {
    SqliteSubtitleTrackAssStyleRowRef {
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

impl SqliteSubtitleTrackLrcMetadataRow {
  /// Cheap borrow — produces a [`SqliteSubtitleTrackLrcMetadataRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSubtitleTrackLrcMetadataRowRef<'_> {
    SqliteSubtitleTrackLrcMetadataRowRef {
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

impl SqliteSubtitleCueMicroDvdRow {
  pub fn as_ref(&self) -> SqliteSubtitleCueMicroDvdRowRef<'_> {
    SqliteSubtitleCueMicroDvdRowRef {
      id: &self.id,
      styled_text: &self.styled_text,
    }
  }
}

impl SqliteSubtitleCueSubViewerRow {
  pub fn as_ref(&self) -> SqliteSubtitleCueSubViewerRowRef<'_> {
    SqliteSubtitleCueSubViewerRowRef {
      id: &self.id,
      styled_text: &self.styled_text,
    }
  }
}

impl SqliteSubtitleCueSbvRow {
  pub fn as_ref(&self) -> SqliteSubtitleCueSbvRowRef<'_> {
    SqliteSubtitleCueSbvRowRef { id: &self.id }
  }
}

impl SqliteSubtitleCueTtmlRow {
  pub fn as_ref(&self) -> SqliteSubtitleCueTtmlRowRef<'_> {
    SqliteSubtitleCueTtmlRowRef {
      id: &self.id,
      region_id: self.region_id.as_deref(),
      style_id: self.style_id.as_deref(),
      xml_id: &self.xml_id,
      styled_text: &self.styled_text,
    }
  }
}

impl SqliteSubtitleCueSamiRow {
  pub fn as_ref(&self) -> SqliteSubtitleCueSamiRowRef<'_> {
    SqliteSubtitleCueSamiRowRef {
      id: &self.id,
      class_name: &self.class_name,
      styled_text: &self.styled_text,
    }
  }
}

impl SqliteSubtitleCueVobSubRow {
  pub fn as_ref(&self) -> SqliteSubtitleCueVobSubRowRef<'_> {
    SqliteSubtitleCueVobSubRowRef {
      id: &self.id,
      palette_id: &self.palette_id,
      bitmap: &self.bitmap,
      width: self.width,
      height: self.height,
      pos_x: self.pos_x,
      pos_y: self.pos_y,
      color_indices: self.color_indices,
      contrast_indices: self.contrast_indices,
    }
  }
}

impl SqliteSubtitleCuePgsRow {
  pub fn as_ref(&self) -> SqliteSubtitleCuePgsRowRef<'_> {
    SqliteSubtitleCuePgsRowRef {
      id: &self.id,
      bitmap: &self.bitmap,
      width: self.width,
      height: self.height,
      pos_x: self.pos_x,
      pos_y: self.pos_y,
      palette_bytes: &self.palette_bytes,
      composition_state: self.composition_state,
    }
  }
}

impl SqliteSubtitleCueCea608Row {
  pub fn as_ref(&self) -> SqliteSubtitleCueCea608RowRef<'_> {
    SqliteSubtitleCueCea608RowRef {
      id: &self.id,
      channel: self.channel,
      pac_byte_pair: self.pac_byte_pair,
      styled_text: &self.styled_text,
    }
  }
}

impl SqliteSubtitleCueEbuStlRow {
  pub fn as_ref(&self) -> SqliteSubtitleCueEbuStlRowRef<'_> {
    SqliteSubtitleCueEbuStlRowRef {
      id: &self.id,
      subtitle_number: self.subtitle_number,
      cumulative: self.cumulative,
      vertical_pos: self.vertical_pos,
      justification: self.justification,
      styled_text: &self.styled_text,
    }
  }
}

impl SqliteSubtitleTrackTtmlRegionRow {
  pub fn as_ref(&self) -> SqliteSubtitleTrackTtmlRegionRowRef<'_> {
    SqliteSubtitleTrackTtmlRegionRowRef {
      id: &self.id,
      subtitle_track_id: &self.subtitle_track_id,
      xml_id: &self.xml_id,
      xml_attrs: &self.xml_attrs,
    }
  }
}

impl SqliteSubtitleTrackTtmlStyleRow {
  pub fn as_ref(&self) -> SqliteSubtitleTrackTtmlStyleRowRef<'_> {
    SqliteSubtitleTrackTtmlStyleRowRef {
      id: &self.id,
      subtitle_track_id: &self.subtitle_track_id,
      xml_id: &self.xml_id,
      xml_attrs: &self.xml_attrs,
    }
  }
}

impl SqliteSubtitleTrackSamiStyleRow {
  pub fn as_ref(&self) -> SqliteSubtitleTrackSamiStyleRowRef<'_> {
    SqliteSubtitleTrackSamiStyleRowRef {
      id: &self.id,
      subtitle_track_id: &self.subtitle_track_id,
      class_name: &self.class_name,
      css_text: &self.css_text,
    }
  }
}

impl SqliteSubtitleTrackVobSubPaletteRow {
  pub fn as_ref(&self) -> SqliteSubtitleTrackVobSubPaletteRowRef<'_> {
    SqliteSubtitleTrackVobSubPaletteRowRef {
      id: &self.id,
      subtitle_track_id: &self.subtitle_track_id,
      entry00: self.entry00,
      entry01: self.entry01,
      entry02: self.entry02,
      entry03: self.entry03,
      entry04: self.entry04,
      entry05: self.entry05,
      entry06: self.entry06,
      entry07: self.entry07,
      entry08: self.entry08,
      entry09: self.entry09,
      entry10: self.entry10,
      entry11: self.entry11,
      entry12: self.entry12,
      entry13: self.entry13,
      entry14: self.entry14,
      entry15: self.entry15,
    }
  }
}

// --- Borrowed-view promotion fns ---------------------------------------------

fn base_row_ref_to_parts<'r>(
  r: &SqliteSubtitleCueBaseRowRef<'r>,
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
  let span = mediatime::TimeRange::try_new(r.span_start_pts, r.span_end_pts, parent_timebase)
    .ok_or_else(|| {
      SqlxError::DomainConstructorRejected(format!(
        "TimeRange start_pts ({}) must be <= end_pts ({})",
        r.span_start_pts, r.span_end_pts
      ))
    })?;
  let text =
    LocalizedText::from_src_translated(r.text_src.to_owned(), r.text_translated.to_owned());
  let kind = cue_kind_from_i64_v(r.kind)?;
  Ok((id, subtitle_track_id, ordinal, span, text, kind))
}

/// Rebuild a SubRip cue from its borrowed base row.
pub fn srt_cue_from_row_ref<'r>(
  base: SqliteSubtitleCueBaseRowRef<'r>,
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
  base: SqliteSubtitleCueBaseRowRef<'r>,
  detail: SqliteSubtitleCueVttRowRef<'r>,
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
  base: SqliteSubtitleCueBaseRowRef<'r>,
  detail: SqliteSubtitleCueAssRowRef<'r>,
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
  base: SqliteSubtitleCueBaseRowRef<'r>,
  detail: SqliteSubtitleCueLrcRowRef<'r>,
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
  r: SqliteSubtitleCueLrcWordRowRef<'r>,
) -> Result<LrcWord<Uuid7>, SqlxError> {
  let subtitle_cue_id = bytes_to_uuid7(r.subtitle_cue_id)?;
  let ordinal = u32_from_i64(r.ordinal, "LrcWord.ordinal")?;
  LrcWord::try_new(subtitle_cue_id, ordinal, r.text, r.start_pts)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// Rebuild a [`VttRegion`] from its borrowed row.
pub fn vtt_region_from_row_ref<'r>(
  r: SqliteSubtitleTrackVttRegionRowRef<'r>,
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
  r: SqliteSubtitleTrackVttStyleRowRef<'r>,
) -> Result<VttStyleBlock<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(r.id)?;
  let subtitle_track_id = bytes_to_uuid7(r.subtitle_track_id)?;
  let ordinal = u32_from_i64(r.ordinal, "VttStyleBlock.ordinal")?;
  VttStyleBlock::try_new(id, subtitle_track_id, ordinal, r.css_text)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// Rebuild an [`AssStyle`] from its borrowed row.
pub fn ass_style_from_row_ref<'r>(
  r: SqliteSubtitleTrackAssStyleRowRef<'r>,
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
  r: SqliteSubtitleTrackLrcMetadataRowRef<'r>,
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

pub fn micro_dvd_cue_from_row_refs<'r>(
  base: SqliteSubtitleCueBaseRowRef<'r>,
  detail: SqliteSubtitleCueMicroDvdRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<MicroDvdCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_ref_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::MicroDvd {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected MicroDvd cue kind, got {kind:?}"
    )));
  }
  let d = MicroDvdData::new(detail.styled_text);
  MicroDvdCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

pub fn sub_viewer_cue_from_row_refs<'r>(
  base: SqliteSubtitleCueBaseRowRef<'r>,
  detail: SqliteSubtitleCueSubViewerRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<SubViewerCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_ref_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::SubViewer {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected SubViewer cue kind, got {kind:?}"
    )));
  }
  let d = SubViewerData::new(detail.styled_text);
  SubViewerCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

pub fn sbv_cue_from_row_refs<'r>(
  base: SqliteSubtitleCueBaseRowRef<'r>,
  _detail: SqliteSubtitleCueSbvRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<SbvCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_ref_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Sbv {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Sbv cue kind, got {kind:?}"
    )));
  }
  SbvCue::try_new(id, subtitle_track_id, ordinal, span, text, SbvData::new())
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

pub fn ttml_cue_from_row_refs<'r>(
  base: SqliteSubtitleCueBaseRowRef<'r>,
  detail: SqliteSubtitleCueTtmlRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<TtmlCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_ref_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Ttml {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Ttml cue kind, got {kind:?}"
    )));
  }
  let region_id = match detail.region_id {
    None => None,
    Some(b) => Some(bytes_to_uuid7(b)?),
  };
  let style_id = match detail.style_id {
    None => None,
    Some(b) => Some(bytes_to_uuid7(b)?),
  };
  let d = TtmlData::<Uuid7>::new()
    .maybe_region_id(region_id)
    .maybe_style_id(style_id)
    .with_xml_id(detail.xml_id)
    .with_styled_text(detail.styled_text);
  TtmlCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

pub fn sami_cue_from_row_refs<'r>(
  base: SqliteSubtitleCueBaseRowRef<'r>,
  detail: SqliteSubtitleCueSamiRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<SamiCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_ref_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Sami {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Sami cue kind, got {kind:?}"
    )));
  }
  let d = SamiData::new()
    .with_class_name(detail.class_name)
    .with_styled_text(detail.styled_text);
  SamiCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

pub fn vob_sub_cue_from_row_refs<'r>(
  base: SqliteSubtitleCueBaseRowRef<'r>,
  detail: SqliteSubtitleCueVobSubRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<VobSubCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_ref_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::VobSub {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected VobSub cue kind, got {kind:?}"
    )));
  }
  let palette_id = bytes_to_uuid7(detail.palette_id)?;
  let d = VobSubData::<Uuid7>::new(palette_id)
    .with_bitmap(Bytes::copy_from_slice(detail.bitmap))
    .with_width(u32_from_i64(detail.width, "VobSubData.width")?)
    .with_height(u32_from_i64(detail.height, "VobSubData.height")?)
    .with_pos(
      i32_from_i64(detail.pos_x, "VobSubData.pos_x")?,
      i32_from_i64(detail.pos_y, "VobSubData.pos_y")?,
    )
    .with_color_indices(unpack_indices_i64(detail.color_indices)?)
    .with_contrast_indices(unpack_indices_i64(detail.contrast_indices)?);
  VobSubCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

pub fn pgs_cue_from_row_refs<'r>(
  base: SqliteSubtitleCueBaseRowRef<'r>,
  detail: SqliteSubtitleCuePgsRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<PgsCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_ref_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Pgs {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Pgs cue kind, got {kind:?}"
    )));
  }
  let composition_state = u8::try_from(detail.composition_state)
    .map_err(|e| SqlxError::UnknownDiscriminant(format!("PgsData.composition_state: {e}")))?;
  let d = PgsData::new()
    .with_bitmap(Bytes::copy_from_slice(detail.bitmap))
    .with_palette_bytes(Bytes::copy_from_slice(detail.palette_bytes))
    .with_width(u32_from_i64(detail.width, "PgsData.width")?)
    .with_height(u32_from_i64(detail.height, "PgsData.height")?)
    .with_pos(
      i32_from_i64(detail.pos_x, "PgsData.pos_x")?,
      i32_from_i64(detail.pos_y, "PgsData.pos_y")?,
    )
    .with_composition_state(composition_state);
  PgsCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

pub fn cea_608_cue_from_row_refs<'r>(
  base: SqliteSubtitleCueBaseRowRef<'r>,
  detail: SqliteSubtitleCueCea608RowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<Cea608Cue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_ref_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Cea608 {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Cea608 cue kind, got {kind:?}"
    )));
  }
  let channel = u8::try_from(detail.channel)
    .map_err(|e| SqlxError::UnknownDiscriminant(format!("Cea608Data.channel: {e}")))?;
  let pac = u32::try_from(detail.pac_byte_pair)
    .map_err(|e| SqlxError::UnknownDiscriminant(format!("Cea608Data.pac_byte_pair: {e}")))?;
  let d = Cea608Data::try_new(channel)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_pac_byte_pair(pac)
    .with_styled_text(detail.styled_text);
  Cea608Cue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

pub fn ebu_stl_cue_from_row_refs<'r>(
  base: SqliteSubtitleCueBaseRowRef<'r>,
  detail: SqliteSubtitleCueEbuStlRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<EbuStlCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_ref_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::EbuStl {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected EbuStl cue kind, got {kind:?}"
    )));
  }
  let justification = u8::try_from(detail.justification)
    .map_err(|e| SqlxError::UnknownDiscriminant(format!("EbuStlData.justification: {e}")))?;
  let subtitle_number = u32_from_i64(detail.subtitle_number, "EbuStlData.subtitle_number")?;
  let vertical_pos = i32_from_i64(detail.vertical_pos, "EbuStlData.vertical_pos")?;
  let d = EbuStlData::try_new(justification)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_subtitle_number(subtitle_number)
    .maybe_cumulative(detail.cumulative)
    .with_vertical_pos(vertical_pos)
    .with_styled_text(detail.styled_text);
  EbuStlCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

pub fn ttml_region_from_row_ref<'r>(
  r: SqliteSubtitleTrackTtmlRegionRowRef<'r>,
) -> Result<TtmlRegion<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(r.id)?;
  let subtitle_track_id = bytes_to_uuid7(r.subtitle_track_id)?;
  Ok(
    TtmlRegion::try_new(id, subtitle_track_id, r.xml_id)
      .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_xml_attrs(r.xml_attrs),
  )
}

pub fn ttml_style_from_row_ref<'r>(
  r: SqliteSubtitleTrackTtmlStyleRowRef<'r>,
) -> Result<TtmlStyle<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(r.id)?;
  let subtitle_track_id = bytes_to_uuid7(r.subtitle_track_id)?;
  Ok(
    TtmlStyle::try_new(id, subtitle_track_id, r.xml_id)
      .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_xml_attrs(r.xml_attrs),
  )
}

pub fn sami_style_from_row_ref<'r>(
  r: SqliteSubtitleTrackSamiStyleRowRef<'r>,
) -> Result<SamiStyle<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(r.id)?;
  let subtitle_track_id = bytes_to_uuid7(r.subtitle_track_id)?;
  Ok(
    SamiStyle::try_new(id, subtitle_track_id, r.class_name)
      .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_css_text(r.css_text),
  )
}

pub fn vob_sub_palette_from_row_ref<'r>(
  r: SqliteSubtitleTrackVobSubPaletteRowRef<'r>,
) -> Result<VobSubPalette<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(r.id)?;
  let subtitle_track_id = bytes_to_uuid7(r.subtitle_track_id)?;
  let to_u32 = |v: i64, what: &str| -> Result<u32, SqlxError> {
    u32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
  };
  let entries: [u32; 16] = [
    to_u32(r.entry00, "VobSubPalette.entry00")?,
    to_u32(r.entry01, "VobSubPalette.entry01")?,
    to_u32(r.entry02, "VobSubPalette.entry02")?,
    to_u32(r.entry03, "VobSubPalette.entry03")?,
    to_u32(r.entry04, "VobSubPalette.entry04")?,
    to_u32(r.entry05, "VobSubPalette.entry05")?,
    to_u32(r.entry06, "VobSubPalette.entry06")?,
    to_u32(r.entry07, "VobSubPalette.entry07")?,
    to_u32(r.entry08, "VobSubPalette.entry08")?,
    to_u32(r.entry09, "VobSubPalette.entry09")?,
    to_u32(r.entry10, "VobSubPalette.entry10")?,
    to_u32(r.entry11, "VobSubPalette.entry11")?,
    to_u32(r.entry12, "VobSubPalette.entry12")?,
    to_u32(r.entry13, "VobSubPalette.entry13")?,
    to_u32(r.entry14, "VobSubPalette.entry14")?,
    to_u32(r.entry15, "VobSubPalette.entry15")?,
  ];
  Ok(
    VobSubPalette::try_new(id, subtitle_track_id)
      .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_entries(entries),
  )
}

impl SqliteSubtitleRow {
  /// Cheap borrow — produces a [`SqliteSubtitleRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSubtitleRowRef<'_> {
    SqliteSubtitleRowRef {
      id: &self.id,
      media_id: &self.media_id,
      track_progress_total: self.track_progress_total,
      track_progress_indexed: self.track_progress_indexed,
      track_progress_failed: self.track_progress_failed,
    }
  }
}

impl SqliteSubtitleTrackRow {
  /// Cheap borrow — produces a [`SqliteSubtitleTrackRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSubtitleTrackRowRef<'_> {
    SqliteSubtitleTrackRowRef {
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

impl SqliteSubtitleTrackIndexErrorRow {
  /// Cheap borrow — produces a [`SqliteSubtitleTrackIndexErrorRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSubtitleTrackIndexErrorRowRef<'_> {
    SqliteSubtitleTrackIndexErrorRowRef {
      subtitle_track_id: &self.subtitle_track_id,
      ordinal: self.ordinal,
      code: self.code,
      message: &self.message,
    }
  }
}

impl SqliteSubtitleTrackMetadataRow {
  /// Cheap borrow — produces a [`SqliteSubtitleTrackMetadataRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSubtitleTrackMetadataRowRef<'_> {
    SqliteSubtitleTrackMetadataRowRef {
      subtitle_track_id: &self.subtitle_track_id,
      ordinal: self.ordinal,
      key: &self.key,
      value: &self.value,
    }
  }
}

impl<'r> TryFrom<SqliteSubtitleRowRef<'r>> for Subtitle<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteSubtitleRowRef<'r>) -> Result<Self, Self::Error> {
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
    SqliteSubtitleTrackRowRef<'r>,
    std::vec::Vec<SqliteSubtitleTrackIndexErrorRowRef<'r>>,
    std::vec::Vec<SqliteSubtitleTrackMetadataRowRef<'r>>,
  )> for SubtitleTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, mut errors, mut metadata): (
      SqliteSubtitleTrackRowRef<'r>,
      std::vec::Vec<SqliteSubtitleTrackIndexErrorRowRef<'r>>,
      std::vec::Vec<SqliteSubtitleTrackMetadataRowRef<'r>>,
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
        TrackOrigin::try_from_u32(u32_from_i64(r.origin, "SubtitleTrack.origin")?)
          .ok_or_else(|| SqlxError::UnknownDiscriminant(format!("TrackOrigin: {}", r.origin)))?,
      )
      .with_title(r.title)
      .with_disposition(TrackDisposition::from_bits_truncate(u32_from_i64(
        r.disposition,
        "SubtitleTrack.disposition",
      )?))
      .with_primary(r.is_primary != 0)
      .with_auto_selected(r.auto_selected != 0)
      .with_cue_count(u32_from_i64(r.cue_count, "SubtitleTrack.cue_count")?)
      .with_stream_index(opt_u32(r.stream_index, "SubtitleTrack.stream_index")?)
      .with_container_track_id(r.container_track_id.map(|v| v as u64))
      .with_character_encoding(r.character_encoding)
      .with_bom_present(r.bom_present != 0)
      .with_sdh(r.is_sdh != 0)
      .with_closed_caption(r.is_closed_caption != 0)
      .with_translation(r.is_translation != 0)
      .with_kind(kind_from_i64(r.kind)?)
      .with_coverage_ratio(r.coverage_ratio)
      .with_empty(r.is_empty != 0);

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
      let code = u32_from_i64(e.code, "SubtitleTrack.index_error.code")?;
      infos.push(ErrorInfo::new(ErrorCode::from_u32(code), e.message));
    }
    t = t.with_index_errors(infos);

    metadata.sort_by_key(|m| m.ordinal);
    let mut bag = IndexMap::with_capacity(metadata.len());
    for entry in metadata {
      if entry.subtitle_track_id != r.id {
        return Err(SqlxError::DomainConstructorRejected(
          "subtitle_track_metadata.subtitle_track_id does not match parent subtitle_track.id"
            .to_owned(),
        ));
      }
      bag.insert(SmolStr::from(entry.key), SmolStr::from(entry.value));
    }
    t = t.with_metadata(bag);

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

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).unwrap())
  }

  #[test]
  fn subtitle_facet_roundtrip() {
    let s = Subtitle::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_track_progress(IndexProgress::try_new(2, 1, 0).unwrap());
    let row: SqliteSubtitleRow = (&s).into();
    let s2: Subtitle<Uuid7> = row.try_into().unwrap();
    assert_eq!(s.id_ref(), s2.id_ref());
    assert_eq!(s2.track_progress_ref().indexed(), 1);
  }

  #[test]
  fn subtitle_track_roundtrip_minimal() {
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    let tuple: (
      SqliteSubtitleTrackRow,
      std::vec::Vec<SqliteSubtitleTrackIndexErrorRow>,
      std::vec::Vec<SqliteSubtitleTrackMetadataRow>,
    ) = (&t).into();
    let t2: SubtitleTrack<Uuid7> = tuple.try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn subtitle_track_metadata_roundtrip_preserves_insertion_order() {
    let mut meta = IndexMap::new();
    meta.insert(SmolStr::new("language_alt"), SmolStr::new("en-US"));
    meta.insert(SmolStr::new("encoding_origin"), SmolStr::new("scte35"));
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_metadata(meta);
    let (row, errs, mut metadata): (
      SqliteSubtitleTrackRow,
      std::vec::Vec<SqliteSubtitleTrackIndexErrorRow>,
      std::vec::Vec<SqliteSubtitleTrackMetadataRow>,
    ) = (&t).into();
    assert_eq!(metadata.len(), 2);
    metadata.reverse();
    let t2: SubtitleTrack<Uuid7> = (row, errs, metadata).try_into().unwrap();
    let keys: std::vec::Vec<&str> = t2.metadata_ref().keys().map(SmolStr::as_str).collect();
    assert_eq!(keys, std::vec!["language_alt", "encoding_origin"]);
  }

  #[test]
  fn subtitle_track_roundtrip_full() {
    let de = Language::from_bcp47("de").unwrap();
    let mut bytes = [0u8; 32];
    bytes[31] = 9;
    let cs = FileChecksum::from_bytes(bytes);
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(SubtitleCodec::HdmvPgsSubtitle)
      .with_format(Format::PgsSub)
      .with_origin(TrackOrigin::Sidecar)
      .with_language(de)
      .with_title("German")
      .with_disposition(TrackDisposition::from_u32(0x0001))
      .with_duration(Some(Timestamp::new(60_000, tb())))
      .with_cue_count(120)
      .with_provenance(Provenance::from_parts("tesseract", "5.3", "p", "idx"))
      .with_source_checksum(Some(cs))
      .with_character_encoding("UTF-8")
      .with_bom_present(false)
      .with_kind(SubtitleKind::CommentaryText)
      .with_coverage_ratio(Some(0.5))
      .with_first_cue(Some(Timestamp::new(0, tb())))
      .with_last_cue(Some(Timestamp::new(59_000, tb())))
      .with_stream_index(Some(0))
      .with_index_status(SubtitleIndexStatus::TRACKS_DISCOVERED)
      .with_index_errors(std::vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "x")]);
    let tuple: (
      SqliteSubtitleTrackRow,
      std::vec::Vec<SqliteSubtitleTrackIndexErrorRow>,
      std::vec::Vec<SqliteSubtitleTrackMetadataRow>,
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
        ErrorInfo::new(ErrorCode::TranscriptionFailed, "c"),
      ]);
    let (row, mut errs, meta): (
      SqliteSubtitleTrackRow,
      std::vec::Vec<SqliteSubtitleTrackIndexErrorRow>,
      std::vec::Vec<SqliteSubtitleTrackMetadataRow>,
    ) = (&t).into();
    errs.reverse();
    let t2: SubtitleTrack<Uuid7> = (row, errs, meta).try_into().unwrap();
    assert_eq!(t2.index_errors_slice()[0].message(), "a");
    assert_eq!(t2.index_errors_slice()[2].message(), "c");
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
    let base: SqliteSubtitleCueBaseRow = (&c).into();
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
    let (base, detail): (SqliteSubtitleCueBaseRow, SqliteSubtitleCueVttRow) = (&c).into();
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
    let (base, detail): (SqliteSubtitleCueBaseRow, SqliteSubtitleCueAssRow) = (&c).into();
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
    let (base, detail): (SqliteSubtitleCueBaseRow, SqliteSubtitleCueLrcRow) = (&c).into();
    let c2 = lrc_cue_from_rows(base, detail, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn lrc_word_round_trip() {
    let w = LrcWord::try_new(Uuid7::new(), 3, "ma", 1234).unwrap();
    let row: SqliteSubtitleCueLrcWordRow = (&w).into();
    let w2 = lrc_word_from_row(row).unwrap();
    assert_eq!(w, w2);
  }

  #[test]
  fn vtt_region_round_trip() {
    let r = VttRegion::try_new(Uuid7::new(), Uuid7::new(), "footer")
      .unwrap()
      .with_width(80.0)
      .with_lines(2);
    let row: SqliteSubtitleTrackVttRegionRow = (&r).into();
    let r2 = vtt_region_from_row(row).unwrap();
    assert_eq!(r, r2);
  }

  #[test]
  fn vtt_style_round_trip() {
    let s = VttStyleBlock::try_new(Uuid7::new(), Uuid7::new(), 0, "::cue { color: red }").unwrap();
    let row: SqliteSubtitleTrackVttStyleRow = (&s).into();
    let s2 = vtt_style_from_row(row).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn ass_style_round_trip() {
    let s = AssStyle::try_new(Uuid7::new(), Uuid7::new(), "Default")
      .unwrap()
      .with_fontname("Arial")
      .with_bold();
    let row: SqliteSubtitleTrackAssStyleRow = (&s).into();
    let s2 = ass_style_from_row(row).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn lrc_metadata_round_trip() {
    let m = LrcMetadata::try_new(Uuid7::new())
      .unwrap()
      .with_title("Song")
      .with_offset_ms(-500);
    let row: SqliteSubtitleTrackLrcMetadataRow = (&m).into();
    let m2 = lrc_metadata_from_row(row).unwrap();
    assert_eq!(m, m2);
  }

  #[test]
  fn subtitle_facet_ref_roundtrip() {
    let s = Subtitle::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_track_progress(IndexProgress::try_new(2, 1, 0).unwrap());
    let row: SqliteSubtitleRow = (&s).into();
    let s2: Subtitle<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(s.id_ref(), s2.id_ref());
    assert_eq!(s2.track_progress_ref().indexed(), 1);
  }

  #[test]
  fn subtitle_track_ref_roundtrip() {
    let de = Language::from_bcp47("de").unwrap();
    let mut bytes = [0u8; 32];
    bytes[31] = 9;
    let cs = FileChecksum::from_bytes(bytes);
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(SubtitleCodec::HdmvPgsSubtitle)
      .with_format(Format::PgsSub)
      .with_origin(TrackOrigin::Sidecar)
      .with_language(de)
      .with_title("German")
      .with_disposition(TrackDisposition::from_u32(0x0001))
      .with_duration(Some(Timestamp::new(60_000, tb())))
      .with_cue_count(120)
      .with_provenance(Provenance::from_parts("tesseract", "5.3", "p", "idx"))
      .with_source_checksum(Some(cs))
      .with_character_encoding("UTF-8")
      .with_bom_present(false)
      .with_kind(SubtitleKind::CommentaryText)
      .with_coverage_ratio(Some(0.5))
      .with_first_cue(Some(Timestamp::new(0, tb())))
      .with_last_cue(Some(Timestamp::new(59_000, tb())))
      .with_stream_index(Some(0))
      .with_index_status(SubtitleIndexStatus::TRACKS_DISCOVERED)
      .with_index_errors(std::vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "x")]);
    let (row, errs, meta): (
      SqliteSubtitleTrackRow,
      std::vec::Vec<SqliteSubtitleTrackIndexErrorRow>,
      std::vec::Vec<SqliteSubtitleTrackMetadataRow>,
    ) = (&t).into();
    let err_refs: std::vec::Vec<SqliteSubtitleTrackIndexErrorRowRef<'_>> = errs
      .iter()
      .map(SqliteSubtitleTrackIndexErrorRow::as_ref)
      .collect();
    let meta_refs: std::vec::Vec<SqliteSubtitleTrackMetadataRowRef<'_>> = meta
      .iter()
      .map(SqliteSubtitleTrackMetadataRow::as_ref)
      .collect();
    let t2: SubtitleTrack<Uuid7> = (row.as_ref(), err_refs, meta_refs).try_into().unwrap();
    assert_eq!(t, t2);
  }

  // ---------------------------------------------------------------------------
  // Polymorphic subtitle_cue *_ref_roundtrip tests (sqlite)
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
    let base: SqliteSubtitleCueBaseRow = (&c).into();
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
    let (base, detail): (SqliteSubtitleCueBaseRow, SqliteSubtitleCueVttRow) = (&c).into();
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
    let (base, detail): (SqliteSubtitleCueBaseRow, SqliteSubtitleCueAssRow) = (&c).into();
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
    let (base, detail): (SqliteSubtitleCueBaseRow, SqliteSubtitleCueLrcRow) = (&c).into();
    let c2 = lrc_cue_from_row_refs(base.as_ref(), detail.as_ref(), tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn lrc_word_ref_roundtrip() {
    let w = LrcWord::try_new(Uuid7::new(), 3, "ma", 1234).unwrap();
    let row: SqliteSubtitleCueLrcWordRow = (&w).into();
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
    let row: SqliteSubtitleTrackVttRegionRow = (&r).into();
    let r2 = vtt_region_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(r, r2);
  }

  #[test]
  fn vtt_style_ref_roundtrip() {
    let s = VttStyleBlock::try_new(Uuid7::new(), Uuid7::new(), 0, "::cue { color: red }").unwrap();
    let row: SqliteSubtitleTrackVttStyleRow = (&s).into();
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
    let row: SqliteSubtitleTrackAssStyleRow = (&s).into();
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
    let row: SqliteSubtitleTrackLrcMetadataRow = (&m).into();
    let m2 = lrc_metadata_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(m, m2);
  }

  // ---- Long-tail formats (#56) -------------------------------------------

  #[test]
  fn micro_dvd_cue_round_trip() {
    let c: MicroDvdCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("hi"),
      MicroDvdData::new("{y:b}hi"),
    )
    .unwrap();
    let (b, d): (SqliteSubtitleCueBaseRow, SqliteSubtitleCueMicroDvdRow) = (&c).into();
    let c2 = micro_dvd_cue_from_rows(b.clone(), d.clone(), tb()).unwrap();
    assert_eq!(c, c2);
    let c3 = micro_dvd_cue_from_row_refs(b.as_ref(), d.as_ref(), tb()).unwrap();
    assert_eq!(c, c3);
  }

  #[test]
  fn sub_viewer_cue_round_trip() {
    let c: SubViewerCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("hi"),
      SubViewerData::new("[b]hi[/b]"),
    )
    .unwrap();
    let (b, d): (SqliteSubtitleCueBaseRow, SqliteSubtitleCueSubViewerRow) = (&c).into();
    let c2 = sub_viewer_cue_from_rows(b.clone(), d.clone(), tb()).unwrap();
    assert_eq!(c, c2);
    let c3 = sub_viewer_cue_from_row_refs(b.as_ref(), d.as_ref(), tb()).unwrap();
    assert_eq!(c, c3);
  }

  #[test]
  fn sbv_cue_round_trip() {
    let c: SbvCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("plain"),
      SbvData::new(),
    )
    .unwrap();
    let (b, d): (SqliteSubtitleCueBaseRow, SqliteSubtitleCueSbvRow) = (&c).into();
    let c2 = sbv_cue_from_rows(b.clone(), d.clone(), tb()).unwrap();
    assert_eq!(c, c2);
    let c3 = sbv_cue_from_row_refs(b.as_ref(), d.as_ref(), tb()).unwrap();
    assert_eq!(c, c3);
  }

  #[test]
  fn ttml_cue_round_trip() {
    let d = TtmlData::<Uuid7>::new()
      .with_region_id(Uuid7::new())
      .with_xml_id("c-1")
      .with_styled_text("<span/>");
    let c: TtmlCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("hi"),
      d,
    )
    .unwrap();
    let (b, dr): (SqliteSubtitleCueBaseRow, SqliteSubtitleCueTtmlRow) = (&c).into();
    let c2 = ttml_cue_from_rows(b.clone(), dr.clone(), tb()).unwrap();
    assert_eq!(c, c2);
    let c3 = ttml_cue_from_row_refs(b.as_ref(), dr.as_ref(), tb()).unwrap();
    assert_eq!(c, c3);
  }

  #[test]
  fn sami_cue_round_trip() {
    let d = SamiData::new()
      .with_class_name("ENCC")
      .with_styled_text("<P>Hi</P>");
    let c: SamiCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("Hi"),
      d,
    )
    .unwrap();
    let (b, dr): (SqliteSubtitleCueBaseRow, SqliteSubtitleCueSamiRow) = (&c).into();
    let c2 = sami_cue_from_rows(b.clone(), dr.clone(), tb()).unwrap();
    assert_eq!(c, c2);
    let c3 = sami_cue_from_row_refs(b.as_ref(), dr.as_ref(), tb()).unwrap();
    assert_eq!(c, c3);
  }

  #[test]
  fn vob_sub_cue_round_trip() {
    let palette_id = Uuid7::new();
    let d = VobSubData::<Uuid7>::new(palette_id)
      .with_bitmap(Bytes::from_static(b"\x01\x02"))
      .with_width(720)
      .with_height(60)
      .with_pos(20, 540)
      .with_color_indices([1, 2, 3, 4]);
    let c: VobSubCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::new(),
      d,
    )
    .unwrap();
    let (b, dr): (SqliteSubtitleCueBaseRow, SqliteSubtitleCueVobSubRow) = (&c).into();
    let c2 = vob_sub_cue_from_rows(b.clone(), dr.clone(), tb()).unwrap();
    assert_eq!(c, c2);
    let c3 = vob_sub_cue_from_row_refs(b.as_ref(), dr.as_ref(), tb()).unwrap();
    assert_eq!(c, c3);
  }

  #[test]
  fn pgs_cue_round_trip() {
    let d = PgsData::new()
      .with_bitmap(Bytes::from_static(b"\xAA"))
      .with_palette_bytes(Bytes::from_static(b"\x10"))
      .with_composition_state(0x80);
    let c: PgsCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::new(),
      d,
    )
    .unwrap();
    let (b, dr): (SqliteSubtitleCueBaseRow, SqliteSubtitleCuePgsRow) = (&c).into();
    let c2 = pgs_cue_from_rows(b.clone(), dr.clone(), tb()).unwrap();
    assert_eq!(c, c2);
    let c3 = pgs_cue_from_row_refs(b.as_ref(), dr.as_ref(), tb()).unwrap();
    assert_eq!(c, c3);
  }

  #[test]
  fn cea_608_cue_round_trip() {
    let d = Cea608Data::try_new(2)
      .unwrap()
      .with_pac_byte_pair(0x1170)
      .with_styled_text("Hi");
    let c: Cea608Cue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("Hi"),
      d,
    )
    .unwrap();
    let (b, dr): (SqliteSubtitleCueBaseRow, SqliteSubtitleCueCea608Row) = (&c).into();
    let c2 = cea_608_cue_from_rows(b.clone(), dr.clone(), tb()).unwrap();
    assert_eq!(c, c2);
    let c3 = cea_608_cue_from_row_refs(b.as_ref(), dr.as_ref(), tb()).unwrap();
    assert_eq!(c, c3);
  }

  #[test]
  fn ebu_stl_cue_round_trip() {
    let d = EbuStlData::try_new(2)
      .unwrap()
      .with_subtitle_number(42)
      .with_cumulative()
      .with_vertical_pos(20);
    let c: EbuStlCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("Hi"),
      d,
    )
    .unwrap();
    let (b, dr): (SqliteSubtitleCueBaseRow, SqliteSubtitleCueEbuStlRow) = (&c).into();
    let c2 = ebu_stl_cue_from_rows(b.clone(), dr.clone(), tb()).unwrap();
    assert_eq!(c, c2);
    let c3 = ebu_stl_cue_from_row_refs(b.as_ref(), dr.as_ref(), tb()).unwrap();
    assert_eq!(c, c3);
  }

  #[test]
  fn ttml_region_round_trip() {
    let r = TtmlRegion::try_new(Uuid7::new(), Uuid7::new(), "r1")
      .unwrap()
      .with_xml_attrs("tts:origin=\"10% 80%\"");
    let row: SqliteSubtitleTrackTtmlRegionRow = (&r).into();
    let r2 = ttml_region_from_row(row.clone()).unwrap();
    assert_eq!(r, r2);
    let r3 = ttml_region_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(r, r3);
  }

  #[test]
  fn ttml_style_round_trip() {
    let s = TtmlStyle::try_new(Uuid7::new(), Uuid7::new(), "s1")
      .unwrap()
      .with_xml_attrs("tts:color=\"red\"");
    let row: SqliteSubtitleTrackTtmlStyleRow = (&s).into();
    let s2 = ttml_style_from_row(row.clone()).unwrap();
    assert_eq!(s, s2);
    let s3 = ttml_style_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(s, s3);
  }

  #[test]
  fn sami_style_round_trip() {
    let s = SamiStyle::try_new(Uuid7::new(), Uuid7::new(), "ENCC")
      .unwrap()
      .with_css_text("{color: yellow;}");
    let row: SqliteSubtitleTrackSamiStyleRow = (&s).into();
    let s2 = sami_style_from_row(row.clone()).unwrap();
    assert_eq!(s, s2);
    let s3 = sami_style_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(s, s3);
  }

  #[test]
  fn vob_sub_palette_round_trip() {
    let mut entries = [0u32; 16];
    entries[0] = 0x00_FF_00_00;
    entries[5] = 0x00_00_FF_00;
    let p = VobSubPalette::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_entries(entries);
    let row: SqliteSubtitleTrackVobSubPaletteRow = (&p).into();
    let p2 = vob_sub_palette_from_row(row.clone()).unwrap();
    assert_eq!(p, p2);
    let p3 = vob_sub_palette_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(p, p3);
  }
}
