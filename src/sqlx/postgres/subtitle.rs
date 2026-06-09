//! PostgreSQL row shapes for the subtitle-cluster aggregates: the
//! `Subtitle` facet, `SubtitleTrack`, and `SubtitleCue`.
//!
//! Identity / FK columns are native `uuid`. Nested value-objects
//! (`Provenance`, `LocalizedText`) are flattened into real,
//! individually-typed columns — `Option<VO>` rides as a discriminating
//! column plus all-NULL payload columns when absent. The open descriptor
//! enums (`SubtitleCodec`, `Format`) ride as `text` slugs; the closed
//! coded enum (`TrackOrigin`) and bitflags (`SubtitleIndexStatus` /
//! `TrackDisposition`) ride as integers. `Language` flattens to a BCP-47
//! `text` column. Media-time values flatten to a PTS `BIGINT` + timebase
//! num/den. Wall-clock has no place here (subtitle-cluster carries only
//! media-time).
//!
//! Collections ride in child tables: `SubtitleTrack::index_errors` →
//! `subtitle_track_index_error`, with an `ordinal` order column,
//! mirroring `audio_track_index_error`. The `Vec<Id>` reverse-FK fields
//! (`Subtitle::tracks`, `SubtitleTrack::cues`) are NOT stored — they are
//! derived by querying the child table's FK.

use indexmap::IndexMap;
use mediaframe::{
  codec::SubtitleCodec,
  disposition::TrackDisposition,
  lang::Language,
  subtitle::{Format, TrackOrigin},
};
use smol_str::SmolStr;
use uuid::Uuid;

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
    dto::{bytes_to_checksum, timestamp_from_parts, uuid7_to_uuid, uuid_to_uuid7},
    SqlxError,
  },
};

use bytes::Bytes;

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

/// PostgreSQL row shape for the [`Subtitle`] facet.
///
/// `tracks` (a `Vec<Id>` reverse of `subtitle_track.subtitle_id`) is not
/// stored; the flattened `track_progress` rollup is.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleRow {
  pub id: Uuid,
  pub media_id: Uuid,
  pub track_progress_total: i64,
  pub track_progress_indexed: i64,
  pub track_progress_failed: i64,
}

impl From<&Subtitle<Uuid7>> for PgSubtitleRow {
  fn from(s: &Subtitle<Uuid7>) -> Self {
    let p = s.track_progress_ref();
    Self {
      id: uuid7_to_uuid(*s.id_ref()),
      media_id: uuid7_to_uuid(*s.media_id_ref()),
      track_progress_total: i64::from(p.total()),
      track_progress_indexed: i64::from(p.indexed()),
      track_progress_failed: i64::from(p.failed()),
    }
  }
}

impl TryFrom<PgSubtitleRow> for Subtitle<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgSubtitleRow) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let media_id = uuid_to_uuid7(r.media_id)?;
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

/// PostgreSQL row shape for [`SubtitleTrack`].
///
/// `FileChecksum` rides as `BYTEA` (32 bytes), NULL = absent.
/// `Provenance` flattens to the same four `provenance_*` columns used
/// in `audio_track`. `cues` reverse-FK is NOT stored. The pre-rev-4
/// `source_path_volume` / `source_path` columns were dropped in #67
/// — with the polymorphic-cue redesign the storage layer no longer
/// needs the source file path; only the `source_checksum` is kept for
/// FS-rescan change detection.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PgSubtitleTrackRow {
  pub id: Uuid,
  pub subtitle_id: Uuid,
  pub stream_index: Option<i64>,
  pub container_track_id: Option<i64>,
  /// `SubtitleCodec` open enum — FFmpeg short-name slug; `""` = absent.
  pub codec: String,
  /// `Format` open enum — FFmpeg-style slug; `""` = absent.
  pub format: String,
  /// `TrackOrigin::to_u32` (closed coded enum: `0=Embedded`, `1=Sidecar`,
  /// `2=External`).
  pub origin: i32,
  /// Declared `Language`, BCP-47; NULL = absent (`Language::default` /
  /// `und`).
  pub language: Option<String>,
  pub title: String,
  /// `TrackDisposition` bitflags `.bits()`.
  pub disposition: i64,
  pub is_primary: bool,
  pub auto_selected: bool,
  /// `duration` PTS tick + timebase; all-NULL = absent.
  pub duration_pts: Option<i64>,
  pub duration_tb_num: Option<i64>,
  pub duration_tb_den: Option<i64>,
  pub cue_count: i64,
  /// `Provenance` shared VO (`""` = absent per field).
  pub provenance_model_name: String,
  pub provenance_model_version: String,
  pub provenance_prompt_version: String,
  pub provenance_indexer_version: String,
  /// `FileChecksum` of the external file (32 bytes); NULL = absent.
  pub source_checksum: Option<std::vec::Vec<u8>>,
  pub character_encoding: String,
  pub bom_present: bool,
  pub is_sdh: bool,
  pub is_closed_caption: bool,
  pub is_translation: bool,
  /// `SubtitleKind` closed enum as small integer.
  pub kind: i16,
  pub coverage_ratio: Option<f32>,
  pub is_empty: bool,
  pub first_cue_pts: Option<i64>,
  pub first_cue_tb_num: Option<i64>,
  pub first_cue_tb_den: Option<i64>,
  pub last_cue_pts: Option<i64>,
  pub last_cue_tb_num: Option<i64>,
  pub last_cue_tb_den: Option<i64>,
  /// `SubtitleIndexStatus` bitflags `.bits()`.
  pub index_status: i64,
}

/// One `subtitle_track_index_error` child row: a single `ErrorInfo` from
/// `SubtitleTrack::index_errors`, with its `ordinal` position.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleTrackIndexErrorRow {
  pub subtitle_track_id: Uuid,
  pub ordinal: i32,
  pub code: i32,
  pub message: String,
}

/// PostgreSQL row for `subtitle_track_metadata`. Position in the per-
/// `subtitle_track_id` `ordinal` sequence IS the [`IndexMap`] insertion
/// order. `subtitle_track_from_rows` sorts by `ordinal` on decode.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PgSubtitleTrackMetadataRow {
  pub subtitle_track_id: Uuid,
  pub ordinal: i32,
  pub key: String,
  pub value: String,
}

impl From<&SubtitleTrack<Uuid7>>
  for (
    PgSubtitleTrackRow,
    std::vec::Vec<PgSubtitleTrackIndexErrorRow>,
    std::vec::Vec<PgSubtitleTrackMetadataRow>,
  )
{
  fn from(t: &SubtitleTrack<Uuid7>) -> Self {
    let id = uuid7_to_uuid(*t.id_ref());
    let prov = t.provenance_ref();
    let duration = t.duration_ref();
    let first_cue = t.first_cue_ref();
    let last_cue = t.last_cue_ref();
    let row = PgSubtitleTrackRow {
      id,
      subtitle_id: uuid7_to_uuid(*t.subtitle_id_ref()),
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
      .map(|(i, e)| PgSubtitleTrackIndexErrorRow {
        subtitle_track_id: id,
        ordinal: i as i32,
        code: e.code().as_u32() as i32,
        message: e.message().to_owned(),
      })
      .collect();
    let metadata = t
      .metadata_ref()
      .iter()
      .enumerate()
      .map(|(i, (k, v))| PgSubtitleTrackMetadataRow {
        subtitle_track_id: id,
        ordinal: i32::try_from(i).unwrap_or(i32::MAX),
        key: k.as_str().to_owned(),
        value: v.as_str().to_owned(),
      })
      .collect();
    (row, errors, metadata)
  }
}

impl
  TryFrom<(
    PgSubtitleTrackRow,
    std::vec::Vec<PgSubtitleTrackIndexErrorRow>,
    std::vec::Vec<PgSubtitleTrackMetadataRow>,
  )> for SubtitleTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, errors, metadata): (
      PgSubtitleTrackRow,
      std::vec::Vec<PgSubtitleTrackIndexErrorRow>,
      std::vec::Vec<PgSubtitleTrackMetadataRow>,
    ),
  ) -> Result<Self, Self::Error> {
    subtitle_track_from_rows(r, errors, metadata)
  }
}

/// Reconstruct a [`SubtitleTrack`] from its row, `index_errors` rows,
/// and `metadata` rows. The supplied collections may be in any order —
/// both are sorted by `ordinal` before insertion so the original `Vec` /
/// `IndexMap` ordering is recovered.
pub fn subtitle_track_from_rows(
  r: PgSubtitleTrackRow,
  mut errors: std::vec::Vec<PgSubtitleTrackIndexErrorRow>,
  mut metadata: std::vec::Vec<PgSubtitleTrackMetadataRow>,
) -> Result<SubtitleTrack<Uuid7>, SqlxError> {
  {
    let id = uuid_to_uuid7(r.id)?;
    let subtitle_id = uuid_to_uuid7(r.subtitle_id)?;
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

    metadata.sort_by_key(|m| m.ordinal);
    let mut bag = IndexMap::with_capacity(metadata.len());
    for entry in metadata {
      if entry.subtitle_track_id != r.id {
        return Err(SqlxError::DomainConstructorRejected(format!(
          "subtitle_track_metadata.subtitle_track_id ({}) does not match parent subtitle_track.id ({})",
          entry.subtitle_track_id, r.id
        )));
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
//
// Storage shape (see `schema/subtitle_cues.md` rev 5 + per-format detail
// docs): `subtitle_cue` is the format-agnostic base table. Per-format
// payload rides in a sibling detail table keyed by the cue id
// (`subtitle_cue_vtt` / `subtitle_cue_ass` / `subtitle_cue_lrc`).
// SubRip has no detail table. The full polymorphic cue is the JOIN of
// the base + the detail keyed by `id`. The `kind` SMALLINT on the base
// is the discriminant ([`SubtitleCueKind`]).
//
// Per-track aggregate tables (`subtitle_track_vtt_region`,
// `subtitle_track_vtt_style`, `subtitle_track_ass_style`,
// `subtitle_track_lrc_metadata`) are separate rows keyed by the parent
// track id.

/// PostgreSQL row shape for the base [`SubtitleCue`] table.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueBaseRow {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
  pub ordinal: i64,
  pub span_start_pts: i64,
  pub span_end_pts: i64,
  pub text_src: String,
  pub text_translated: String,
  pub kind: i16,
}

fn cue_kind_to_i16(k: SubtitleCueKind) -> i16 {
  k.to_u8() as i16
}

fn cue_kind_from_i16(n: i16) -> Result<SubtitleCueKind, SqlxError> {
  u8::try_from(n)
    .ok()
    .and_then(SubtitleCueKind::try_from_u8)
    .ok_or_else(|| SqlxError::UnknownDiscriminant(format!("SubtitleCueKind: {n}")))
}

fn base_row_from_cue<D>(c: &SubtitleCue<Uuid7, D>, kind: SubtitleCueKind) -> PgSubtitleCueBaseRow {
  let span = c.span_ref();
  PgSubtitleCueBaseRow {
    id: uuid7_to_uuid(*c.id_ref()),
    subtitle_track_id: uuid7_to_uuid(*c.subtitle_track_id_ref()),
    ordinal: i64::from(c.ordinal()),
    span_start_pts: span.start_pts(),
    span_end_pts: span.end_pts(),
    text_src: c.text_ref().src().to_owned(),
    text_translated: c.text_ref().translated().to_owned(),
    kind: cue_kind_to_i16(kind),
  }
}

fn base_row_to_parts(
  r: &PgSubtitleCueBaseRow,
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
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
  let ordinal = u32_from_i64(r.ordinal, "SubtitleCue.ordinal")?;
  let span = mediatime::TimeRange::try_new(r.span_start_pts, r.span_end_pts, parent_timebase)
    .ok_or_else(|| {
      SqlxError::DomainConstructorRejected(format!(
        "TimeRange start_pts ({}) must be <= end_pts ({})",
        r.span_start_pts, r.span_end_pts
      ))
    })?;
  let text = LocalizedText::from_src_translated(r.text_src.clone(), r.text_translated.clone());
  let kind = cue_kind_from_i16(r.kind)?;
  Ok((id, subtitle_track_id, ordinal, span, text, kind))
}

// --- SRT ---------------------------------------------------------------------

impl From<&SrtCue<Uuid7>> for PgSubtitleCueBaseRow {
  fn from(c: &SrtCue<Uuid7>) -> Self {
    base_row_from_cue(c, SubtitleCueKind::Srt)
  }
}

/// Rebuild a SubRip cue from its base row.
pub fn srt_cue_from_row(
  base: PgSubtitleCueBaseRow,
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

/// PostgreSQL detail row for a WebVTT cue (joined on `id`).
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PgSubtitleCueVttRow {
  pub id: Uuid,
  pub cue_identifier: String,
  pub vertical: Option<i16>,
  pub line_value: String,
  pub line_align: Option<i16>,
  pub position_value: String,
  pub position_align: Option<i16>,
  pub size_value: Option<f32>,
  pub text_align: Option<i16>,
  pub region_id: Option<Uuid>,
  pub voice: String,
  pub styled_text: String,
}

impl From<&VttCue<Uuid7>> for (PgSubtitleCueBaseRow, PgSubtitleCueVttRow) {
  fn from(c: &VttCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Vtt);
    let d = c.data_ref();
    let detail = PgSubtitleCueVttRow {
      id: base.id,
      cue_identifier: d.cue_identifier().to_owned(),
      vertical: d.vertical().map(|v| v.to_u8() as i16),
      line_value: d.line_value().to_owned(),
      line_align: d.line_align().map(|v| v.to_u8() as i16),
      position_value: d.position_value().to_owned(),
      position_align: d.position_align().map(|v| v.to_u8() as i16),
      size_value: d.size_value(),
      text_align: d.text_align().map(|v| v.to_u8() as i16),
      region_id: d.region_id_ref().map(|id| uuid7_to_uuid(*id)),
      voice: d.voice().to_owned(),
      styled_text: d.styled_text().to_owned(),
    };
    (base, detail)
  }
}

fn map_small<T>(
  v: Option<i16>,
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
  base: PgSubtitleCueBaseRow,
  detail: PgSubtitleCueVttRow,
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
    Some(u) => Some(uuid_to_uuid7(u)?),
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

/// PostgreSQL detail row for an ASS/SSA cue (joined on `id`).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueAssRow {
  pub id: Uuid,
  pub layer: i32,
  pub style_id: Uuid,
  pub name: String,
  pub margin_l: i32,
  pub margin_r: i32,
  pub margin_v: i32,
  pub effect: String,
  pub styled_text: String,
}

impl From<&AssCue<Uuid7>> for (PgSubtitleCueBaseRow, PgSubtitleCueAssRow) {
  fn from(c: &AssCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Ass);
    let d = c.data_ref();
    let detail = PgSubtitleCueAssRow {
      id: base.id,
      layer: d.layer(),
      style_id: uuid7_to_uuid(*d.style_id_ref()),
      name: d.name().to_owned(),
      margin_l: d.margin_l(),
      margin_r: d.margin_r(),
      margin_v: d.margin_v(),
      effect: d.effect().to_owned(),
      styled_text: d.styled_text().to_owned(),
    };
    (base, detail)
  }
}

/// Rebuild an ASS cue from its (base, detail) rows.
pub fn ass_cue_from_rows(
  base: PgSubtitleCueBaseRow,
  detail: PgSubtitleCueAssRow,
  parent_timebase: mediatime::Timebase,
) -> Result<AssCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Ass {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Ass cue kind, got {kind:?}"
    )));
  }
  let style_id = uuid_to_uuid7(detail.style_id)?;
  let d = AssData::<Uuid7>::new(style_id)
    .with_layer(detail.layer)
    .with_name(detail.name)
    .with_margin_l(detail.margin_l)
    .with_margin_r(detail.margin_r)
    .with_margin_v(detail.margin_v)
    .with_effect(detail.effect)
    .with_styled_text(detail.styled_text);
  AssCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- LRC ---------------------------------------------------------------------

/// PostgreSQL detail row for an LRC cue (joined on `id`).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueLrcRow {
  pub id: Uuid,
  pub has_word_timing: bool,
}

impl From<&LrcCue<Uuid7>> for (PgSubtitleCueBaseRow, PgSubtitleCueLrcRow) {
  fn from(c: &LrcCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Lrc);
    let detail = PgSubtitleCueLrcRow {
      id: base.id,
      has_word_timing: c.data_ref().has_word_timing(),
    };
    (base, detail)
  }
}

/// Rebuild an LRC cue from its (base, detail) rows.
pub fn lrc_cue_from_rows(
  base: PgSubtitleCueBaseRow,
  detail: PgSubtitleCueLrcRow,
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

/// PostgreSQL row for an LRC word (child of `subtitle_cue_lrc`).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueLrcWordRow {
  pub subtitle_cue_id: Uuid,
  pub ordinal: i32,
  pub text: String,
  pub start_pts: i64,
}

impl From<&LrcWord<Uuid7>> for PgSubtitleCueLrcWordRow {
  fn from(w: &LrcWord<Uuid7>) -> Self {
    Self {
      subtitle_cue_id: uuid7_to_uuid(*w.subtitle_cue_id_ref()),
      ordinal: w.ordinal() as i32,
      text: w.text().to_owned(),
      start_pts: w.start_pts(),
    }
  }
}

/// Rebuild an LRC word from its row.
pub fn lrc_word_from_row(r: PgSubtitleCueLrcWordRow) -> Result<LrcWord<Uuid7>, SqlxError> {
  let subtitle_cue_id = uuid_to_uuid7(r.subtitle_cue_id)?;
  LrcWord::try_new(subtitle_cue_id, r.ordinal as u32, r.text, r.start_pts)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- MicroDVD ----------------------------------------------------------------

/// PostgreSQL detail row for a MicroDVD cue (joined on `id`).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueMicroDvdRow {
  pub id: Uuid,
  pub styled_text: String,
}

impl From<&MicroDvdCue<Uuid7>> for (PgSubtitleCueBaseRow, PgSubtitleCueMicroDvdRow) {
  fn from(c: &MicroDvdCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::MicroDvd);
    let detail = PgSubtitleCueMicroDvdRow {
      id: base.id,
      styled_text: c.data_ref().styled_text().to_owned(),
    };
    (base, detail)
  }
}

/// Rebuild a MicroDVD cue from its (base, detail) rows.
pub fn micro_dvd_cue_from_rows(
  base: PgSubtitleCueBaseRow,
  detail: PgSubtitleCueMicroDvdRow,
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

/// PostgreSQL detail row for a SubViewer cue (joined on `id`).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueSubViewerRow {
  pub id: Uuid,
  pub styled_text: String,
}

impl From<&SubViewerCue<Uuid7>> for (PgSubtitleCueBaseRow, PgSubtitleCueSubViewerRow) {
  fn from(c: &SubViewerCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::SubViewer);
    let detail = PgSubtitleCueSubViewerRow {
      id: base.id,
      styled_text: c.data_ref().styled_text().to_owned(),
    };
    (base, detail)
  }
}

/// Rebuild a SubViewer cue from its (base, detail) rows.
pub fn sub_viewer_cue_from_rows(
  base: PgSubtitleCueBaseRow,
  detail: PgSubtitleCueSubViewerRow,
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
//
// SBV has no per-format detail; the row is just the FK PK. Owning a
// detail table keeps the dispatch surface uniform across formats.

/// PostgreSQL detail row for a YouTube SBV cue. Carries only the FK
/// (no payload columns); written to keep the per-format dispatch
/// uniform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueSbvRow {
  pub id: Uuid,
}

impl From<&SbvCue<Uuid7>> for (PgSubtitleCueBaseRow, PgSubtitleCueSbvRow) {
  fn from(c: &SbvCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Sbv);
    let detail = PgSubtitleCueSbvRow { id: base.id };
    (base, detail)
  }
}

/// Rebuild a SBV cue from its (base, detail) rows.
pub fn sbv_cue_from_rows(
  base: PgSubtitleCueBaseRow,
  _detail: PgSubtitleCueSbvRow,
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

/// PostgreSQL detail row for a TTML cue (joined on `id`).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueTtmlRow {
  pub id: Uuid,
  pub region_id: Option<Uuid>,
  pub style_id: Option<Uuid>,
  pub xml_id: String,
  pub styled_text: String,
}

impl From<&TtmlCue<Uuid7>> for (PgSubtitleCueBaseRow, PgSubtitleCueTtmlRow) {
  fn from(c: &TtmlCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Ttml);
    let d = c.data_ref();
    let detail = PgSubtitleCueTtmlRow {
      id: base.id,
      region_id: d.region_id_ref().map(|id| uuid7_to_uuid(*id)),
      style_id: d.style_id_ref().map(|id| uuid7_to_uuid(*id)),
      xml_id: d.xml_id().to_owned(),
      styled_text: d.styled_text().to_owned(),
    };
    (base, detail)
  }
}

/// Rebuild a TTML cue from its (base, detail) rows.
pub fn ttml_cue_from_rows(
  base: PgSubtitleCueBaseRow,
  detail: PgSubtitleCueTtmlRow,
  parent_timebase: mediatime::Timebase,
) -> Result<TtmlCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Ttml {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Ttml cue kind, got {kind:?}"
    )));
  }
  let region_id = match detail.region_id {
    None => None,
    Some(u) => Some(uuid_to_uuid7(u)?),
  };
  let style_id = match detail.style_id {
    None => None,
    Some(u) => Some(uuid_to_uuid7(u)?),
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

/// PostgreSQL detail row for a SAMI cue (joined on `id`).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueSamiRow {
  pub id: Uuid,
  pub class_name: String,
  pub styled_text: String,
}

impl From<&SamiCue<Uuid7>> for (PgSubtitleCueBaseRow, PgSubtitleCueSamiRow) {
  fn from(c: &SamiCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Sami);
    let d = c.data_ref();
    let detail = PgSubtitleCueSamiRow {
      id: base.id,
      class_name: d.class_name().to_owned(),
      styled_text: d.styled_text().to_owned(),
    };
    (base, detail)
  }
}

/// Rebuild a SAMI cue from its (base, detail) rows.
pub fn sami_cue_from_rows(
  base: PgSubtitleCueBaseRow,
  detail: PgSubtitleCueSamiRow,
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

/// PostgreSQL detail row for a DVD VobSub cue. Bitmap blob rides as
/// `BYTEA`; 4-byte colour/contrast index arrays ride as packed i32 (LE
/// pack of the 4 u8 entries — matches the wire packing).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueVobSubRow {
  pub id: Uuid,
  pub palette_id: Uuid,
  pub bitmap: Vec<u8>,
  pub width: i64,
  pub height: i64,
  pub pos_x: i32,
  pub pos_y: i32,
  pub color_indices: i64,
  pub contrast_indices: i64,
}

fn pack_indices_i64(a: &[u8; 4]) -> i64 {
  (a[0] as i64) | ((a[1] as i64) << 8) | ((a[2] as i64) << 16) | ((a[3] as i64) << 24)
}

fn unpack_indices_i64(n: i64) -> Result<[u8; 4], SqlxError> {
  // Stored value must fit in 32 bits (4 × u8).
  let v = u32::try_from(n)
    .map_err(|e| SqlxError::UnknownDiscriminant(format!("VobSub indices packing: {e}")))?;
  Ok([v as u8, (v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8])
}

impl From<&VobSubCue<Uuid7>> for (PgSubtitleCueBaseRow, PgSubtitleCueVobSubRow) {
  fn from(c: &VobSubCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::VobSub);
    let d = c.data_ref();
    let detail = PgSubtitleCueVobSubRow {
      id: base.id,
      palette_id: uuid7_to_uuid(*d.palette_id_ref()),
      bitmap: d.bitmap_ref().to_vec(),
      width: i64::from(d.width()),
      height: i64::from(d.height()),
      pos_x: d.pos_x(),
      pos_y: d.pos_y(),
      color_indices: pack_indices_i64(d.color_indices()),
      contrast_indices: pack_indices_i64(d.contrast_indices()),
    };
    (base, detail)
  }
}

/// Rebuild a VobSub cue from its (base, detail) rows.
pub fn vob_sub_cue_from_rows(
  base: PgSubtitleCueBaseRow,
  detail: PgSubtitleCueVobSubRow,
  parent_timebase: mediatime::Timebase,
) -> Result<VobSubCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::VobSub {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected VobSub cue kind, got {kind:?}"
    )));
  }
  let palette_id = uuid_to_uuid7(detail.palette_id)?;
  let d = VobSubData::<Uuid7>::new(palette_id)
    .with_bitmap(Bytes::from(detail.bitmap))
    .with_width(u32_from_i64(detail.width, "VobSubData.width")?)
    .with_height(u32_from_i64(detail.height, "VobSubData.height")?)
    .with_pos(detail.pos_x, detail.pos_y)
    .with_color_indices(unpack_indices_i64(detail.color_indices)?)
    .with_contrast_indices(unpack_indices_i64(detail.contrast_indices)?);
  VobSubCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- PGS ---------------------------------------------------------------------

/// PostgreSQL detail row for a Blu-ray PGS cue. Bitmap + palette ride
/// as `BYTEA` blobs.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCuePgsRow {
  pub id: Uuid,
  pub bitmap: Vec<u8>,
  pub width: i64,
  pub height: i64,
  pub pos_x: i32,
  pub pos_y: i32,
  pub palette_bytes: Vec<u8>,
  pub composition_state: i16,
}

impl From<&PgsCue<Uuid7>> for (PgSubtitleCueBaseRow, PgSubtitleCuePgsRow) {
  fn from(c: &PgsCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Pgs);
    let d = c.data_ref();
    let detail = PgSubtitleCuePgsRow {
      id: base.id,
      bitmap: d.bitmap_ref().to_vec(),
      width: i64::from(d.width()),
      height: i64::from(d.height()),
      pos_x: d.pos_x(),
      pos_y: d.pos_y(),
      palette_bytes: d.palette_bytes_ref().to_vec(),
      composition_state: i16::from(d.composition_state()),
    };
    (base, detail)
  }
}

/// Rebuild a PGS cue from its (base, detail) rows.
pub fn pgs_cue_from_rows(
  base: PgSubtitleCueBaseRow,
  detail: PgSubtitleCuePgsRow,
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
    .with_pos(detail.pos_x, detail.pos_y)
    .with_composition_state(composition_state);
  PgsCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// --- CEA-608 -----------------------------------------------------------------

/// PostgreSQL detail row for a CEA-608 caption cue. `channel` is
/// validated `1..=4` by [`Cea608Data::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueCea608Row {
  pub id: Uuid,
  pub channel: i16,
  pub pac_byte_pair: i64,
  pub styled_text: String,
}

impl From<&Cea608Cue<Uuid7>> for (PgSubtitleCueBaseRow, PgSubtitleCueCea608Row) {
  fn from(c: &Cea608Cue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::Cea608);
    let d = c.data_ref();
    let detail = PgSubtitleCueCea608Row {
      id: base.id,
      channel: i16::from(d.channel()),
      pac_byte_pair: i64::from(d.pac_byte_pair()),
      styled_text: d.styled_text().to_owned(),
    };
    (base, detail)
  }
}

/// Rebuild a CEA-608 cue from its (base, detail) rows.
pub fn cea_608_cue_from_rows(
  base: PgSubtitleCueBaseRow,
  detail: PgSubtitleCueCea608Row,
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

/// PostgreSQL detail row for an EBU STL teletext cue.
/// `justification` is validated `1..=3` by [`EbuStlData::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueEbuStlRow {
  pub id: Uuid,
  pub subtitle_number: i64,
  pub cumulative: bool,
  pub vertical_pos: i32,
  pub justification: i16,
  pub styled_text: String,
}

impl From<&EbuStlCue<Uuid7>> for (PgSubtitleCueBaseRow, PgSubtitleCueEbuStlRow) {
  fn from(c: &EbuStlCue<Uuid7>) -> Self {
    let base = base_row_from_cue(c, SubtitleCueKind::EbuStl);
    let d = c.data_ref();
    let detail = PgSubtitleCueEbuStlRow {
      id: base.id,
      subtitle_number: i64::from(d.subtitle_number()),
      cumulative: d.cumulative(),
      vertical_pos: d.vertical_pos(),
      justification: i16::from(d.justification()),
      styled_text: d.styled_text().to_owned(),
    };
    (base, detail)
  }
}

/// Rebuild an EBU STL cue from its (base, detail) rows.
pub fn ebu_stl_cue_from_rows(
  base: PgSubtitleCueBaseRow,
  detail: PgSubtitleCueEbuStlRow,
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
  let d = EbuStlData::try_new(justification)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_subtitle_number(subtitle_number)
    .maybe_cumulative(detail.cumulative)
    .with_vertical_pos(detail.vertical_pos)
    .with_styled_text(detail.styled_text);
  EbuStlCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// ===========================================================================
// Per-track aggregates
// ===========================================================================

/// PostgreSQL row for a [`VttRegion`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PgSubtitleTrackVttRegionRow {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
  pub name: String,
  pub width: f32,
  pub lines: i64,
  pub region_anchor_x: f32,
  pub region_anchor_y: f32,
  pub viewport_anchor_x: f32,
  pub viewport_anchor_y: f32,
  pub scroll_up: bool,
}

impl From<&VttRegion<Uuid7>> for PgSubtitleTrackVttRegionRow {
  fn from(r: &VttRegion<Uuid7>) -> Self {
    Self {
      id: uuid7_to_uuid(*r.id_ref()),
      subtitle_track_id: uuid7_to_uuid(*r.subtitle_track_id_ref()),
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

/// Rebuild a [`VttRegion`] from its row.
pub fn vtt_region_from_row(r: PgSubtitleTrackVttRegionRow) -> Result<VttRegion<Uuid7>, SqlxError> {
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
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

/// PostgreSQL row for a [`VttStyleBlock`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleTrackVttStyleRow {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
  pub ordinal: i32,
  pub css_text: String,
}

impl From<&VttStyleBlock<Uuid7>> for PgSubtitleTrackVttStyleRow {
  fn from(s: &VttStyleBlock<Uuid7>) -> Self {
    Self {
      id: uuid7_to_uuid(*s.id_ref()),
      subtitle_track_id: uuid7_to_uuid(*s.subtitle_track_id_ref()),
      ordinal: s.ordinal() as i32,
      css_text: s.css_text().to_owned(),
    }
  }
}

/// Rebuild a [`VttStyleBlock`] from its row.
pub fn vtt_style_from_row(
  r: PgSubtitleTrackVttStyleRow,
) -> Result<VttStyleBlock<Uuid7>, SqlxError> {
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
  VttStyleBlock::try_new(id, subtitle_track_id, r.ordinal as u32, r.css_text)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// PostgreSQL row for an [`AssStyle`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PgSubtitleTrackAssStyleRow {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
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
  pub scale_x: i32,
  pub scale_y: i32,
  pub spacing: i32,
  pub angle: f32,
  pub border_style: i16,
  pub outline: f32,
  pub shadow: f32,
  pub alignment: i16,
  pub margin_l: i32,
  pub margin_r: i32,
  pub margin_v: i32,
  pub encoding: i32,
}

impl From<&AssStyle<Uuid7>> for PgSubtitleTrackAssStyleRow {
  fn from(s: &AssStyle<Uuid7>) -> Self {
    Self {
      id: uuid7_to_uuid(*s.id_ref()),
      subtitle_track_id: uuid7_to_uuid(*s.subtitle_track_id_ref()),
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
      scale_x: s.scale_x(),
      scale_y: s.scale_y(),
      spacing: s.spacing(),
      angle: s.angle(),
      border_style: s.border_style(),
      outline: s.outline(),
      shadow: s.shadow(),
      alignment: s.alignment(),
      margin_l: s.margin_l(),
      margin_r: s.margin_r(),
      margin_v: s.margin_v(),
      encoding: s.encoding(),
    }
  }
}

/// Rebuild an [`AssStyle`] from its row.
pub fn ass_style_from_row(r: PgSubtitleTrackAssStyleRow) -> Result<AssStyle<Uuid7>, SqlxError> {
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
  let to_u32 = |v: i64, what: &str| {
    u32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
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
    .with_scale_x(r.scale_x)
    .with_scale_y(r.scale_y)
    .with_spacing(r.spacing)
    .with_angle(r.angle)
    .with_border_style(r.border_style)
    .with_outline(r.outline)
    .with_shadow(r.shadow)
    .with_alignment(r.alignment)
    .with_margin_l(r.margin_l)
    .with_margin_r(r.margin_r)
    .with_margin_v(r.margin_v)
    .with_encoding(r.encoding);
  Ok(s)
}

/// PostgreSQL row for an [`LrcMetadata`] (1:1 with subtitle_track).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleTrackLrcMetadataRow {
  pub subtitle_track_id: Uuid,
  pub title: String,
  pub artist: String,
  pub album: String,
  pub author: String,
  pub creator: String,
  pub length: String,
  pub offset_ms: i32,
}

impl From<&LrcMetadata<Uuid7>> for PgSubtitleTrackLrcMetadataRow {
  fn from(m: &LrcMetadata<Uuid7>) -> Self {
    Self {
      subtitle_track_id: uuid7_to_uuid(*m.subtitle_track_id_ref()),
      title: m.title().to_owned(),
      artist: m.artist().to_owned(),
      album: m.album().to_owned(),
      author: m.author().to_owned(),
      creator: m.creator().to_owned(),
      length: m.length().to_owned(),
      offset_ms: m.offset_ms(),
    }
  }
}

/// Rebuild an [`LrcMetadata`] from its row.
pub fn lrc_metadata_from_row(
  r: PgSubtitleTrackLrcMetadataRow,
) -> Result<LrcMetadata<Uuid7>, SqlxError> {
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
  let m = LrcMetadata::try_new(subtitle_track_id)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_title(r.title)
    .with_artist(r.artist)
    .with_album(r.album)
    .with_author(r.author)
    .with_creator(r.creator)
    .with_length(r.length)
    .with_offset_ms(r.offset_ms);
  Ok(m)
}

/// PostgreSQL row for a [`TtmlRegion`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleTrackTtmlRegionRow {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
  pub xml_id: String,
  pub xml_attrs: String,
}

impl From<&TtmlRegion<Uuid7>> for PgSubtitleTrackTtmlRegionRow {
  fn from(r: &TtmlRegion<Uuid7>) -> Self {
    Self {
      id: uuid7_to_uuid(*r.id_ref()),
      subtitle_track_id: uuid7_to_uuid(*r.subtitle_track_id_ref()),
      xml_id: r.xml_id().to_owned(),
      xml_attrs: r.xml_attrs().to_owned(),
    }
  }
}

/// Rebuild a [`TtmlRegion`] from its row.
pub fn ttml_region_from_row(
  r: PgSubtitleTrackTtmlRegionRow,
) -> Result<TtmlRegion<Uuid7>, SqlxError> {
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
  let region = TtmlRegion::try_new(id, subtitle_track_id, r.xml_id)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_xml_attrs(r.xml_attrs);
  Ok(region)
}

/// PostgreSQL row for a [`TtmlStyle`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleTrackTtmlStyleRow {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
  pub xml_id: String,
  pub xml_attrs: String,
}

impl From<&TtmlStyle<Uuid7>> for PgSubtitleTrackTtmlStyleRow {
  fn from(s: &TtmlStyle<Uuid7>) -> Self {
    Self {
      id: uuid7_to_uuid(*s.id_ref()),
      subtitle_track_id: uuid7_to_uuid(*s.subtitle_track_id_ref()),
      xml_id: s.xml_id().to_owned(),
      xml_attrs: s.xml_attrs().to_owned(),
    }
  }
}

/// Rebuild a [`TtmlStyle`] from its row.
pub fn ttml_style_from_row(r: PgSubtitleTrackTtmlStyleRow) -> Result<TtmlStyle<Uuid7>, SqlxError> {
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
  let style = TtmlStyle::try_new(id, subtitle_track_id, r.xml_id)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_xml_attrs(r.xml_attrs);
  Ok(style)
}

/// PostgreSQL row for a [`SamiStyle`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleTrackSamiStyleRow {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
  pub class_name: String,
  pub css_text: String,
}

impl From<&SamiStyle<Uuid7>> for PgSubtitleTrackSamiStyleRow {
  fn from(s: &SamiStyle<Uuid7>) -> Self {
    Self {
      id: uuid7_to_uuid(*s.id_ref()),
      subtitle_track_id: uuid7_to_uuid(*s.subtitle_track_id_ref()),
      class_name: s.class_name().to_owned(),
      css_text: s.css_text().to_owned(),
    }
  }
}

/// Rebuild a [`SamiStyle`] from its row.
pub fn sami_style_from_row(r: PgSubtitleTrackSamiStyleRow) -> Result<SamiStyle<Uuid7>, SqlxError> {
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
  let s = SamiStyle::try_new(id, subtitle_track_id, r.class_name)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_css_text(r.css_text);
  Ok(s)
}

/// PostgreSQL row for a [`VobSubPalette`]. The 16-entry palette LUT
/// rides as a `BIGINT[]` array column (each `BIGINT` holds one
/// `0x00RRGGBB` u32).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleTrackVobSubPaletteRow {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
  pub entries: Vec<i64>,
}

impl From<&VobSubPalette<Uuid7>> for PgSubtitleTrackVobSubPaletteRow {
  fn from(p: &VobSubPalette<Uuid7>) -> Self {
    Self {
      id: uuid7_to_uuid(*p.id_ref()),
      subtitle_track_id: uuid7_to_uuid(*p.subtitle_track_id_ref()),
      entries: p.entries().iter().map(|&v| i64::from(v)).collect(),
    }
  }
}

/// Rebuild a [`VobSubPalette`] from its row. Rejects rows whose
/// `entries.len() != 16`.
pub fn vob_sub_palette_from_row(
  r: PgSubtitleTrackVobSubPaletteRow,
) -> Result<VobSubPalette<Uuid7>, SqlxError> {
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
  if r.entries.len() != 16 {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "VobSubPalette.entries.len must be 16, got {}",
      r.entries.len()
    )));
  }
  let mut entries = [0u32; 16];
  for (i, v) in r.entries.iter().enumerate() {
    entries[i] = u32::try_from(*v)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("VobSubPalette.entries[{i}]: {e}")))?;
  }
  let p = VobSubPalette::try_new(id, subtitle_track_id)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_entries(entries);
  Ok(p)
}

// ===========================================================================
// Borrowed-view siblings (`*RowRef<'r>`) — zero-copy decode from `&'r Row`.
//
// `PgSubtitleRow` is all-`Copy` (2 × Uuid + 3 × i64), so it has no `Ref`
// sibling.
// ===========================================================================

/// Borrowed view of [`PgSubtitleTrackRow`] — zero-copy decode from `&'r Row`.
///
/// Variable-length text/byte columns borrow from the underlying row;
/// promotion to the domain [`SubtitleTrack`] only allocates IF the caller
/// asks for it via `TryFrom`. See [`PgSubtitleTrackRow::as_ref`] for the
/// cheap-borrow path from an already-owned row.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PgSubtitleTrackRowRef<'r> {
  pub id: Uuid,
  pub subtitle_id: Uuid,
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

/// Borrowed view of [`PgSubtitleTrackIndexErrorRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleTrackIndexErrorRowRef<'r> {
  pub subtitle_track_id: Uuid,
  pub ordinal: i32,
  pub code: i32,
  pub message: &'r str,
}

/// Borrowed view of [`PgSubtitleTrackMetadataRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleTrackMetadataRowRef<'r> {
  pub subtitle_track_id: Uuid,
  pub ordinal: i32,
  pub key: &'r str,
  pub value: &'r str,
}

// --- Polymorphic subtitle_cue tables ----------------------------------------
//
// Each owned row from the base / per-format detail / per-track aggregate
// tables now has a borrowed-view `*RowRef<'r>` sibling. `Uuid` is `Copy`
// in postgres-land so id columns stay by-value; only the variable-length
// text + BLOB-bytea columns flip to `&'r str` / `&'r [u8]`.

/// Borrowed view of [`PgSubtitleCueBaseRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueBaseRowRef<'r> {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
  pub ordinal: i64,
  pub span_start_pts: i64,
  pub span_end_pts: i64,
  pub text_src: &'r str,
  pub text_translated: &'r str,
  pub kind: i16,
}

/// Borrowed view of [`PgSubtitleCueVttRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PgSubtitleCueVttRowRef<'r> {
  pub id: Uuid,
  pub cue_identifier: &'r str,
  pub vertical: Option<i16>,
  pub line_value: &'r str,
  pub line_align: Option<i16>,
  pub position_value: &'r str,
  pub position_align: Option<i16>,
  pub size_value: Option<f32>,
  pub text_align: Option<i16>,
  pub region_id: Option<Uuid>,
  pub voice: &'r str,
  pub styled_text: &'r str,
}

/// Borrowed view of [`PgSubtitleCueAssRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueAssRowRef<'r> {
  pub id: Uuid,
  pub layer: i32,
  pub style_id: Uuid,
  pub name: &'r str,
  pub margin_l: i32,
  pub margin_r: i32,
  pub margin_v: i32,
  pub effect: &'r str,
  pub styled_text: &'r str,
}

/// Borrowed view of [`PgSubtitleCueLrcRow`].
///
/// The owned row has only `Copy` fields (Uuid + bool); the `Ref` sibling
/// is structurally identical and exists for symmetry with the rest of
/// the cluster + so a `sqlx::query_as::<_, PgSubtitleCueLrcRowRef<'_>>`
/// over a borrowed `&'r Row` works without an intermediate
/// owned-row allocation. The `'r` lifetime is preserved on the type
/// signature (consistent with the other `*RowRef<'r>` siblings) via a
/// `PhantomData` field that `sqlx::FromRow` skips.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueLrcRowRef<'r> {
  pub id: Uuid,
  pub has_word_timing: bool,
  #[sqlx(skip)]
  _lt: core::marker::PhantomData<&'r ()>,
}

/// Borrowed view of [`PgSubtitleCueLrcWordRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueLrcWordRowRef<'r> {
  pub subtitle_cue_id: Uuid,
  pub ordinal: i32,
  pub text: &'r str,
  pub start_pts: i64,
}

/// Borrowed view of [`PgSubtitleTrackVttRegionRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PgSubtitleTrackVttRegionRowRef<'r> {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
  pub name: &'r str,
  pub width: f32,
  pub lines: i64,
  pub region_anchor_x: f32,
  pub region_anchor_y: f32,
  pub viewport_anchor_x: f32,
  pub viewport_anchor_y: f32,
  pub scroll_up: bool,
}

/// Borrowed view of [`PgSubtitleTrackVttStyleRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleTrackVttStyleRowRef<'r> {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
  pub ordinal: i32,
  pub css_text: &'r str,
}

/// Borrowed view of [`PgSubtitleTrackAssStyleRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PgSubtitleTrackAssStyleRowRef<'r> {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
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
  pub scale_x: i32,
  pub scale_y: i32,
  pub spacing: i32,
  pub angle: f32,
  pub border_style: i16,
  pub outline: f32,
  pub shadow: f32,
  pub alignment: i16,
  pub margin_l: i32,
  pub margin_r: i32,
  pub margin_v: i32,
  pub encoding: i32,
}

/// Borrowed view of [`PgSubtitleTrackLrcMetadataRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleTrackLrcMetadataRowRef<'r> {
  pub subtitle_track_id: Uuid,
  pub title: &'r str,
  pub artist: &'r str,
  pub album: &'r str,
  pub author: &'r str,
  pub creator: &'r str,
  pub length: &'r str,
  pub offset_ms: i32,
}

/// Borrowed view of [`PgSubtitleCueMicroDvdRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueMicroDvdRowRef<'r> {
  pub id: Uuid,
  pub styled_text: &'r str,
}

/// Borrowed view of [`PgSubtitleCueSubViewerRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueSubViewerRowRef<'r> {
  pub id: Uuid,
  pub styled_text: &'r str,
}

/// Borrowed view of [`PgSubtitleCueSbvRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueSbvRowRef<'r> {
  pub id: Uuid,
  #[sqlx(skip)]
  pub _lt: core::marker::PhantomData<&'r ()>,
}

/// Borrowed view of [`PgSubtitleCueTtmlRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueTtmlRowRef<'r> {
  pub id: Uuid,
  pub region_id: Option<Uuid>,
  pub style_id: Option<Uuid>,
  pub xml_id: &'r str,
  pub styled_text: &'r str,
}

/// Borrowed view of [`PgSubtitleCueSamiRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueSamiRowRef<'r> {
  pub id: Uuid,
  pub class_name: &'r str,
  pub styled_text: &'r str,
}

/// Borrowed view of [`PgSubtitleCueVobSubRow`]. Bitmap rides as
/// `&'r [u8]`; the 16-byte index arrays stay packed in `i64`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueVobSubRowRef<'r> {
  pub id: Uuid,
  pub palette_id: Uuid,
  pub bitmap: &'r [u8],
  pub width: i64,
  pub height: i64,
  pub pos_x: i32,
  pub pos_y: i32,
  pub color_indices: i64,
  pub contrast_indices: i64,
}

/// Borrowed view of [`PgSubtitleCuePgsRow`]. Bitmap + palette ride as
/// `&'r [u8]`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCuePgsRowRef<'r> {
  pub id: Uuid,
  pub bitmap: &'r [u8],
  pub width: i64,
  pub height: i64,
  pub pos_x: i32,
  pub pos_y: i32,
  pub palette_bytes: &'r [u8],
  pub composition_state: i16,
}

/// Borrowed view of [`PgSubtitleCueCea608Row`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueCea608RowRef<'r> {
  pub id: Uuid,
  pub channel: i16,
  pub pac_byte_pair: i64,
  pub styled_text: &'r str,
}

/// Borrowed view of [`PgSubtitleCueEbuStlRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueEbuStlRowRef<'r> {
  pub id: Uuid,
  pub subtitle_number: i64,
  pub cumulative: bool,
  pub vertical_pos: i32,
  pub justification: i16,
  pub styled_text: &'r str,
}

/// Borrowed view of [`PgSubtitleTrackTtmlRegionRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleTrackTtmlRegionRowRef<'r> {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
  pub xml_id: &'r str,
  pub xml_attrs: &'r str,
}

/// Borrowed view of [`PgSubtitleTrackTtmlStyleRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleTrackTtmlStyleRowRef<'r> {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
  pub xml_id: &'r str,
  pub xml_attrs: &'r str,
}

/// Borrowed view of [`PgSubtitleTrackSamiStyleRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleTrackSamiStyleRowRef<'r> {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
  pub class_name: &'r str,
  pub css_text: &'r str,
}

/// Borrowed view of [`PgSubtitleTrackVobSubPaletteRow`]. The `BIGINT[]`
/// array column rides as a borrowed slice.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleTrackVobSubPaletteRowRef<'r> {
  pub id: Uuid,
  pub subtitle_track_id: Uuid,
  pub entries: &'r [i64],
}

impl PgSubtitleCueBaseRow {
  /// Cheap borrow — produces a [`PgSubtitleCueBaseRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgSubtitleCueBaseRowRef<'_> {
    PgSubtitleCueBaseRowRef {
      id: self.id,
      subtitle_track_id: self.subtitle_track_id,
      ordinal: self.ordinal,
      span_start_pts: self.span_start_pts,
      span_end_pts: self.span_end_pts,
      text_src: &self.text_src,
      text_translated: &self.text_translated,
      kind: self.kind,
    }
  }
}

impl PgSubtitleCueVttRow {
  /// Cheap borrow — produces a [`PgSubtitleCueVttRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgSubtitleCueVttRowRef<'_> {
    PgSubtitleCueVttRowRef {
      id: self.id,
      cue_identifier: &self.cue_identifier,
      vertical: self.vertical,
      line_value: &self.line_value,
      line_align: self.line_align,
      position_value: &self.position_value,
      position_align: self.position_align,
      size_value: self.size_value,
      text_align: self.text_align,
      region_id: self.region_id,
      voice: &self.voice,
      styled_text: &self.styled_text,
    }
  }
}

impl PgSubtitleCueAssRow {
  /// Cheap borrow — produces a [`PgSubtitleCueAssRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgSubtitleCueAssRowRef<'_> {
    PgSubtitleCueAssRowRef {
      id: self.id,
      layer: self.layer,
      style_id: self.style_id,
      name: &self.name,
      margin_l: self.margin_l,
      margin_r: self.margin_r,
      margin_v: self.margin_v,
      effect: &self.effect,
      styled_text: &self.styled_text,
    }
  }
}

impl PgSubtitleCueLrcRow {
  /// Cheap borrow — produces a [`PgSubtitleCueLrcRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgSubtitleCueLrcRowRef<'_> {
    PgSubtitleCueLrcRowRef {
      id: self.id,
      has_word_timing: self.has_word_timing,
      _lt: core::marker::PhantomData,
    }
  }
}

impl PgSubtitleCueLrcWordRow {
  /// Cheap borrow — produces a [`PgSubtitleCueLrcWordRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgSubtitleCueLrcWordRowRef<'_> {
    PgSubtitleCueLrcWordRowRef {
      subtitle_cue_id: self.subtitle_cue_id,
      ordinal: self.ordinal,
      text: &self.text,
      start_pts: self.start_pts,
    }
  }
}

impl PgSubtitleTrackVttRegionRow {
  /// Cheap borrow — produces a [`PgSubtitleTrackVttRegionRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgSubtitleTrackVttRegionRowRef<'_> {
    PgSubtitleTrackVttRegionRowRef {
      id: self.id,
      subtitle_track_id: self.subtitle_track_id,
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

impl PgSubtitleTrackVttStyleRow {
  /// Cheap borrow — produces a [`PgSubtitleTrackVttStyleRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgSubtitleTrackVttStyleRowRef<'_> {
    PgSubtitleTrackVttStyleRowRef {
      id: self.id,
      subtitle_track_id: self.subtitle_track_id,
      ordinal: self.ordinal,
      css_text: &self.css_text,
    }
  }
}

impl PgSubtitleTrackAssStyleRow {
  /// Cheap borrow — produces a [`PgSubtitleTrackAssStyleRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgSubtitleTrackAssStyleRowRef<'_> {
    PgSubtitleTrackAssStyleRowRef {
      id: self.id,
      subtitle_track_id: self.subtitle_track_id,
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

impl PgSubtitleTrackLrcMetadataRow {
  /// Cheap borrow — produces a [`PgSubtitleTrackLrcMetadataRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgSubtitleTrackLrcMetadataRowRef<'_> {
    PgSubtitleTrackLrcMetadataRowRef {
      subtitle_track_id: self.subtitle_track_id,
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

impl PgSubtitleCueMicroDvdRow {
  pub fn as_ref(&self) -> PgSubtitleCueMicroDvdRowRef<'_> {
    PgSubtitleCueMicroDvdRowRef {
      id: self.id,
      styled_text: &self.styled_text,
    }
  }
}

impl PgSubtitleCueSubViewerRow {
  pub fn as_ref(&self) -> PgSubtitleCueSubViewerRowRef<'_> {
    PgSubtitleCueSubViewerRowRef {
      id: self.id,
      styled_text: &self.styled_text,
    }
  }
}

impl PgSubtitleCueSbvRow {
  pub fn as_ref(&self) -> PgSubtitleCueSbvRowRef<'_> {
    PgSubtitleCueSbvRowRef {
      id: self.id,
      _lt: core::marker::PhantomData,
    }
  }
}

impl PgSubtitleCueTtmlRow {
  pub fn as_ref(&self) -> PgSubtitleCueTtmlRowRef<'_> {
    PgSubtitleCueTtmlRowRef {
      id: self.id,
      region_id: self.region_id,
      style_id: self.style_id,
      xml_id: &self.xml_id,
      styled_text: &self.styled_text,
    }
  }
}

impl PgSubtitleCueSamiRow {
  pub fn as_ref(&self) -> PgSubtitleCueSamiRowRef<'_> {
    PgSubtitleCueSamiRowRef {
      id: self.id,
      class_name: &self.class_name,
      styled_text: &self.styled_text,
    }
  }
}

impl PgSubtitleCueVobSubRow {
  pub fn as_ref(&self) -> PgSubtitleCueVobSubRowRef<'_> {
    PgSubtitleCueVobSubRowRef {
      id: self.id,
      palette_id: self.palette_id,
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

impl PgSubtitleCuePgsRow {
  pub fn as_ref(&self) -> PgSubtitleCuePgsRowRef<'_> {
    PgSubtitleCuePgsRowRef {
      id: self.id,
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

impl PgSubtitleCueCea608Row {
  pub fn as_ref(&self) -> PgSubtitleCueCea608RowRef<'_> {
    PgSubtitleCueCea608RowRef {
      id: self.id,
      channel: self.channel,
      pac_byte_pair: self.pac_byte_pair,
      styled_text: &self.styled_text,
    }
  }
}

impl PgSubtitleCueEbuStlRow {
  pub fn as_ref(&self) -> PgSubtitleCueEbuStlRowRef<'_> {
    PgSubtitleCueEbuStlRowRef {
      id: self.id,
      subtitle_number: self.subtitle_number,
      cumulative: self.cumulative,
      vertical_pos: self.vertical_pos,
      justification: self.justification,
      styled_text: &self.styled_text,
    }
  }
}

impl PgSubtitleTrackTtmlRegionRow {
  pub fn as_ref(&self) -> PgSubtitleTrackTtmlRegionRowRef<'_> {
    PgSubtitleTrackTtmlRegionRowRef {
      id: self.id,
      subtitle_track_id: self.subtitle_track_id,
      xml_id: &self.xml_id,
      xml_attrs: &self.xml_attrs,
    }
  }
}

impl PgSubtitleTrackTtmlStyleRow {
  pub fn as_ref(&self) -> PgSubtitleTrackTtmlStyleRowRef<'_> {
    PgSubtitleTrackTtmlStyleRowRef {
      id: self.id,
      subtitle_track_id: self.subtitle_track_id,
      xml_id: &self.xml_id,
      xml_attrs: &self.xml_attrs,
    }
  }
}

impl PgSubtitleTrackSamiStyleRow {
  pub fn as_ref(&self) -> PgSubtitleTrackSamiStyleRowRef<'_> {
    PgSubtitleTrackSamiStyleRowRef {
      id: self.id,
      subtitle_track_id: self.subtitle_track_id,
      class_name: &self.class_name,
      css_text: &self.css_text,
    }
  }
}

impl PgSubtitleTrackVobSubPaletteRow {
  pub fn as_ref(&self) -> PgSubtitleTrackVobSubPaletteRowRef<'_> {
    PgSubtitleTrackVobSubPaletteRowRef {
      id: self.id,
      subtitle_track_id: self.subtitle_track_id,
      entries: &self.entries,
    }
  }
}

// --- Borrowed-view promotion fns ---------------------------------------------
//
// Mirrors the owned-row `*_from_row(...)` / `*_from_rows(...)` shape. The
// base-row helper `base_row_ref_to_parts` factors out the shared
// id/track-id/span/text/kind decoding, matching `base_row_to_parts` for
// the owned path. Per-format functions take the same `parent_timebase`
// argument where applicable.

fn base_row_ref_to_parts<'r>(
  r: &PgSubtitleCueBaseRowRef<'r>,
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
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
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
  let kind = cue_kind_from_i16(r.kind)?;
  Ok((id, subtitle_track_id, ordinal, span, text, kind))
}

/// Rebuild a SubRip cue from its borrowed base row.
pub fn srt_cue_from_row_ref<'r>(
  base: PgSubtitleCueBaseRowRef<'r>,
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
  base: PgSubtitleCueBaseRowRef<'r>,
  detail: PgSubtitleCueVttRowRef<'r>,
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
    Some(u) => Some(uuid_to_uuid7(u)?),
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
  base: PgSubtitleCueBaseRowRef<'r>,
  detail: PgSubtitleCueAssRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<AssCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_ref_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::Ass {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected Ass cue kind, got {kind:?}"
    )));
  }
  let style_id = uuid_to_uuid7(detail.style_id)?;
  let d = AssData::<Uuid7>::new(style_id)
    .with_layer(detail.layer)
    .with_name(detail.name)
    .with_margin_l(detail.margin_l)
    .with_margin_r(detail.margin_r)
    .with_margin_v(detail.margin_v)
    .with_effect(detail.effect)
    .with_styled_text(detail.styled_text);
  AssCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// Rebuild an LRC cue from its borrowed (base, detail) rows.
pub fn lrc_cue_from_row_refs<'r>(
  base: PgSubtitleCueBaseRowRef<'r>,
  detail: PgSubtitleCueLrcRowRef<'r>,
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

/// Rebuild an LRC word from its borrowed row. Mirrors
/// [`lrc_word_from_row`] for the owned-row path; `parent_timebase` isn't
/// needed (an `LrcWord` carries a raw `start_pts` only).
pub fn lrc_word_from_row_ref<'r>(
  r: PgSubtitleCueLrcWordRowRef<'r>,
) -> Result<LrcWord<Uuid7>, SqlxError> {
  let subtitle_cue_id = uuid_to_uuid7(r.subtitle_cue_id)?;
  LrcWord::try_new(subtitle_cue_id, r.ordinal as u32, r.text, r.start_pts)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// Rebuild a [`VttRegion`] from its borrowed row.
pub fn vtt_region_from_row_ref<'r>(
  r: PgSubtitleTrackVttRegionRowRef<'r>,
) -> Result<VttRegion<Uuid7>, SqlxError> {
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
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
  r: PgSubtitleTrackVttStyleRowRef<'r>,
) -> Result<VttStyleBlock<Uuid7>, SqlxError> {
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
  VttStyleBlock::try_new(id, subtitle_track_id, r.ordinal as u32, r.css_text)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// Rebuild an [`AssStyle`] from its borrowed row.
pub fn ass_style_from_row_ref<'r>(
  r: PgSubtitleTrackAssStyleRowRef<'r>,
) -> Result<AssStyle<Uuid7>, SqlxError> {
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
  let to_u32 = |v: i64, what: &str| {
    u32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
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
    .with_scale_x(r.scale_x)
    .with_scale_y(r.scale_y)
    .with_spacing(r.spacing)
    .with_angle(r.angle)
    .with_border_style(r.border_style)
    .with_outline(r.outline)
    .with_shadow(r.shadow)
    .with_alignment(r.alignment)
    .with_margin_l(r.margin_l)
    .with_margin_r(r.margin_r)
    .with_margin_v(r.margin_v)
    .with_encoding(r.encoding);
  Ok(s)
}

/// Rebuild an [`LrcMetadata`] from its borrowed row.
pub fn lrc_metadata_from_row_ref<'r>(
  r: PgSubtitleTrackLrcMetadataRowRef<'r>,
) -> Result<LrcMetadata<Uuid7>, SqlxError> {
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
  let m = LrcMetadata::try_new(subtitle_track_id)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_title(r.title)
    .with_artist(r.artist)
    .with_album(r.album)
    .with_author(r.author)
    .with_creator(r.creator)
    .with_length(r.length)
    .with_offset_ms(r.offset_ms);
  Ok(m)
}

/// Rebuild a MicroDVD cue from its borrowed (base, detail) rows.
pub fn micro_dvd_cue_from_row_refs<'r>(
  base: PgSubtitleCueBaseRowRef<'r>,
  detail: PgSubtitleCueMicroDvdRowRef<'r>,
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

/// Rebuild a SubViewer cue from its borrowed (base, detail) rows.
pub fn sub_viewer_cue_from_row_refs<'r>(
  base: PgSubtitleCueBaseRowRef<'r>,
  detail: PgSubtitleCueSubViewerRowRef<'r>,
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

/// Rebuild a SBV cue from its borrowed (base, detail) rows.
pub fn sbv_cue_from_row_refs<'r>(
  base: PgSubtitleCueBaseRowRef<'r>,
  _detail: PgSubtitleCueSbvRowRef<'r>,
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

/// Rebuild a TTML cue from its borrowed (base, detail) rows.
pub fn ttml_cue_from_row_refs<'r>(
  base: PgSubtitleCueBaseRowRef<'r>,
  detail: PgSubtitleCueTtmlRowRef<'r>,
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
    Some(u) => Some(uuid_to_uuid7(u)?),
  };
  let style_id = match detail.style_id {
    None => None,
    Some(u) => Some(uuid_to_uuid7(u)?),
  };
  let d = TtmlData::<Uuid7>::new()
    .maybe_region_id(region_id)
    .maybe_style_id(style_id)
    .with_xml_id(detail.xml_id)
    .with_styled_text(detail.styled_text);
  TtmlCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// Rebuild a SAMI cue from its borrowed (base, detail) rows.
pub fn sami_cue_from_row_refs<'r>(
  base: PgSubtitleCueBaseRowRef<'r>,
  detail: PgSubtitleCueSamiRowRef<'r>,
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

/// Rebuild a VobSub cue from its borrowed (base, detail) rows.
pub fn vob_sub_cue_from_row_refs<'r>(
  base: PgSubtitleCueBaseRowRef<'r>,
  detail: PgSubtitleCueVobSubRowRef<'r>,
  parent_timebase: mediatime::Timebase,
) -> Result<VobSubCue<Uuid7>, SqlxError> {
  let (id, subtitle_track_id, ordinal, span, text, kind) =
    base_row_ref_to_parts(&base, parent_timebase)?;
  if kind != SubtitleCueKind::VobSub {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "expected VobSub cue kind, got {kind:?}"
    )));
  }
  let palette_id = uuid_to_uuid7(detail.palette_id)?;
  let d = VobSubData::<Uuid7>::new(palette_id)
    .with_bitmap(Bytes::copy_from_slice(detail.bitmap))
    .with_width(u32_from_i64(detail.width, "VobSubData.width")?)
    .with_height(u32_from_i64(detail.height, "VobSubData.height")?)
    .with_pos(detail.pos_x, detail.pos_y)
    .with_color_indices(unpack_indices_i64(detail.color_indices)?)
    .with_contrast_indices(unpack_indices_i64(detail.contrast_indices)?);
  VobSubCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// Rebuild a PGS cue from its borrowed (base, detail) rows.
pub fn pgs_cue_from_row_refs<'r>(
  base: PgSubtitleCueBaseRowRef<'r>,
  detail: PgSubtitleCuePgsRowRef<'r>,
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
    .with_pos(detail.pos_x, detail.pos_y)
    .with_composition_state(composition_state);
  PgsCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// Rebuild a CEA-608 cue from its borrowed (base, detail) rows.
pub fn cea_608_cue_from_row_refs<'r>(
  base: PgSubtitleCueBaseRowRef<'r>,
  detail: PgSubtitleCueCea608RowRef<'r>,
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

/// Rebuild an EBU STL cue from its borrowed (base, detail) rows.
pub fn ebu_stl_cue_from_row_refs<'r>(
  base: PgSubtitleCueBaseRowRef<'r>,
  detail: PgSubtitleCueEbuStlRowRef<'r>,
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
  let d = EbuStlData::try_new(justification)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_subtitle_number(subtitle_number)
    .maybe_cumulative(detail.cumulative)
    .with_vertical_pos(detail.vertical_pos)
    .with_styled_text(detail.styled_text);
  EbuStlCue::try_new(id, subtitle_track_id, ordinal, span, text, d)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
}

/// Rebuild a [`TtmlRegion`] from its borrowed row.
pub fn ttml_region_from_row_ref<'r>(
  r: PgSubtitleTrackTtmlRegionRowRef<'r>,
) -> Result<TtmlRegion<Uuid7>, SqlxError> {
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
  let region = TtmlRegion::try_new(id, subtitle_track_id, r.xml_id)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_xml_attrs(r.xml_attrs);
  Ok(region)
}

/// Rebuild a [`TtmlStyle`] from its borrowed row.
pub fn ttml_style_from_row_ref<'r>(
  r: PgSubtitleTrackTtmlStyleRowRef<'r>,
) -> Result<TtmlStyle<Uuid7>, SqlxError> {
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
  let style = TtmlStyle::try_new(id, subtitle_track_id, r.xml_id)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_xml_attrs(r.xml_attrs);
  Ok(style)
}

/// Rebuild a [`SamiStyle`] from its borrowed row.
pub fn sami_style_from_row_ref<'r>(
  r: PgSubtitleTrackSamiStyleRowRef<'r>,
) -> Result<SamiStyle<Uuid7>, SqlxError> {
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
  let s = SamiStyle::try_new(id, subtitle_track_id, r.class_name)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_css_text(r.css_text);
  Ok(s)
}

/// Rebuild a [`VobSubPalette`] from its borrowed row.
pub fn vob_sub_palette_from_row_ref<'r>(
  r: PgSubtitleTrackVobSubPaletteRowRef<'r>,
) -> Result<VobSubPalette<Uuid7>, SqlxError> {
  let id = uuid_to_uuid7(r.id)?;
  let subtitle_track_id = uuid_to_uuid7(r.subtitle_track_id)?;
  if r.entries.len() != 16 {
    return Err(SqlxError::DomainConstructorRejected(format!(
      "VobSubPalette.entries.len must be 16, got {}",
      r.entries.len()
    )));
  }
  let mut entries = [0u32; 16];
  for (i, v) in r.entries.iter().enumerate() {
    entries[i] = u32::try_from(*v)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("VobSubPalette.entries[{i}]: {e}")))?;
  }
  let p = VobSubPalette::try_new(id, subtitle_track_id)
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))?
    .with_entries(entries);
  Ok(p)
}

impl PgSubtitleTrackRow {
  /// Cheap borrow — produces a [`PgSubtitleTrackRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgSubtitleTrackRowRef<'_> {
    PgSubtitleTrackRowRef {
      id: self.id,
      subtitle_id: self.subtitle_id,
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

impl PgSubtitleTrackIndexErrorRow {
  /// Cheap borrow — produces a [`PgSubtitleTrackIndexErrorRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgSubtitleTrackIndexErrorRowRef<'_> {
    PgSubtitleTrackIndexErrorRowRef {
      subtitle_track_id: self.subtitle_track_id,
      ordinal: self.ordinal,
      code: self.code,
      message: &self.message,
    }
  }
}

impl PgSubtitleTrackMetadataRow {
  /// Cheap borrow — produces a [`PgSubtitleTrackMetadataRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgSubtitleTrackMetadataRowRef<'_> {
    PgSubtitleTrackMetadataRowRef {
      subtitle_track_id: self.subtitle_track_id,
      ordinal: self.ordinal,
      key: &self.key,
      value: &self.value,
    }
  }
}

impl<'r>
  TryFrom<(
    PgSubtitleTrackRowRef<'r>,
    std::vec::Vec<PgSubtitleTrackIndexErrorRowRef<'r>>,
    std::vec::Vec<PgSubtitleTrackMetadataRowRef<'r>>,
  )> for SubtitleTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, mut errors, mut metadata): (
      PgSubtitleTrackRowRef<'r>,
      std::vec::Vec<PgSubtitleTrackIndexErrorRowRef<'r>>,
      std::vec::Vec<PgSubtitleTrackMetadataRowRef<'r>>,
    ),
  ) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let subtitle_id = uuid_to_uuid7(r.subtitle_id)?;
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

    metadata.sort_by_key(|m| m.ordinal);
    let mut bag = IndexMap::with_capacity(metadata.len());
    for entry in metadata {
      if entry.subtitle_track_id != r.id {
        return Err(SqlxError::DomainConstructorRejected(format!(
          "subtitle_track_metadata.subtitle_track_id ({}) does not match parent subtitle_track.id ({})",
          entry.subtitle_track_id, r.id
        )));
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

/// Parse an `SubtitleCodec` slug. The `FromStr` impl on
/// [`SubtitleCodec`] is total (unknown slugs land in `Other(slug)`); the
/// empty-string slug is the `""` = absent sentinel and maps to
/// `Other("")` — the same default value used by `SubtitleTrack::try_new`.
fn parse_subtitle_codec(s: &str) -> SubtitleCodec {
  s.parse::<SubtitleCodec>()
    .unwrap_or_else(|_| SubtitleCodec::Other(s.into()))
}

/// Parse a `Format` slug. The `FromStr` impl on [`Format`] is total
/// (unknown slugs land in `Other(slug)`).
fn parse_subtitle_format(s: &str) -> Format {
  s.parse::<Format>()
    .unwrap_or_else(|_| Format::Other(s.into()))
}

fn parse_language(s: &str) -> Result<Language, SqlxError> {
  Language::from_bcp47(s)
    .map_err(|e| SqlxError::DomainConstructorRejected(format!("Language `{s}`: {e}")))
}

/// `Language::default()` is the `und` / undetermined tag — store it as
/// NULL so a written-then-read row round-trips losslessly via the
/// `Option<String>` column.
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

/// A media-time value carries `(num, den)`; both columns must be present
/// together with the PTS column.
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
      .with_track_progress(IndexProgress::try_new(3, 2, 1).unwrap());
    let row: PgSubtitleRow = (&s).into();
    let s2: Subtitle<Uuid7> = row.try_into().unwrap();
    assert_eq!(s.id_ref(), s2.id_ref());
    assert_eq!(s.media_id_ref(), s2.media_id_ref());
    assert_eq!(s2.track_progress_ref().total(), 3);
    assert_eq!(s2.track_progress_ref().indexed(), 2);
    assert_eq!(s2.track_progress_ref().failed(), 1);
  }

  #[test]
  fn subtitle_track_roundtrip_minimal() {
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    let tuple: (
      PgSubtitleTrackRow,
      std::vec::Vec<PgSubtitleTrackIndexErrorRow>,
      std::vec::Vec<PgSubtitleTrackMetadataRow>,
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
      .with_closed_caption(false)
      .with_translation(true)
      .with_kind(SubtitleKind::ForcedNarrative)
      .with_coverage_ratio(Some(0.97))
      .with_empty(false)
      .with_first_cue(Some(Timestamp::new(500, tb())))
      .with_last_cue(Some(Timestamp::new(119_500, tb())))
      .with_stream_index(Some(2))
      .with_container_track_id(Some(7))
      .with_index_status(
        SubtitleIndexStatus::TRACKS_DISCOVERED | SubtitleIndexStatus::CUES_EXTRACTED,
      )
      .with_index_errors(std::vec![
        ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad"),
        ErrorInfo::new(ErrorCode::PathNotFound, "gone"),
      ]);
    let tuple: (
      PgSubtitleTrackRow,
      std::vec::Vec<PgSubtitleTrackIndexErrorRow>,
      std::vec::Vec<PgSubtitleTrackMetadataRow>,
    ) = (&t).into();
    assert_eq!(tuple.1.len(), 2);
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
      PgSubtitleTrackRow,
      std::vec::Vec<PgSubtitleTrackIndexErrorRow>,
      std::vec::Vec<PgSubtitleTrackMetadataRow>,
    ) = (&t).into();
    errs.reverse();
    let t2: SubtitleTrack<Uuid7> = (row, errs, meta).try_into().unwrap();
    assert_eq!(t2.index_errors_slice().len(), 3);
    assert_eq!(t2.index_errors_slice()[0].message(), "a");
    assert_eq!(t2.index_errors_slice()[2].message(), "c");
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
      PgSubtitleTrackRow,
      std::vec::Vec<PgSubtitleTrackIndexErrorRow>,
      std::vec::Vec<PgSubtitleTrackMetadataRow>,
    ) = (&t).into();
    assert_eq!(metadata.len(), 2);
    metadata.reverse();
    let t2: SubtitleTrack<Uuid7> = (row, errs, metadata).try_into().unwrap();
    let keys: std::vec::Vec<&str> = t2.metadata_ref().keys().map(SmolStr::as_str).collect();
    assert_eq!(keys, std::vec!["language_alt", "encoding_origin"]);
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
    let base: PgSubtitleCueBaseRow = (&c).into();
    assert_eq!(base.kind, SubtitleCueKind::Srt.to_u8() as i16);
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
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueVttRow) = (&c).into();
    assert_eq!(base.kind, SubtitleCueKind::Vtt.to_u8() as i16);
    let c2 = vtt_cue_from_rows(base, detail, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn ass_cue_round_trip() {
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
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::new(),
      d,
    )
    .unwrap();
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueAssRow) = (&c).into();
    assert_eq!(base.kind, SubtitleCueKind::Ass.to_u8() as i16);
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
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueLrcRow) = (&c).into();
    assert_eq!(base.kind, SubtitleCueKind::Lrc.to_u8() as i16);
    assert!(detail.has_word_timing);
    let c2 = lrc_cue_from_rows(base, detail, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn lrc_word_round_trip() {
    let w = LrcWord::try_new(Uuid7::new(), 3, "ma", 1234).unwrap();
    let row: PgSubtitleCueLrcWordRow = (&w).into();
    let w2 = lrc_word_from_row(row).unwrap();
    assert_eq!(w, w2);
  }

  #[test]
  fn vtt_region_round_trip() {
    let r = VttRegion::try_new(Uuid7::new(), Uuid7::new(), "footer")
      .unwrap()
      .with_width(80.0)
      .with_lines(2)
      .with_region_anchor(50.0, 100.0)
      .with_viewport_anchor(50.0, 90.0)
      .with_scroll_up();
    let row: PgSubtitleTrackVttRegionRow = (&r).into();
    let r2 = vtt_region_from_row(row).unwrap();
    assert_eq!(r, r2);
  }

  #[test]
  fn vtt_style_round_trip() {
    let s = VttStyleBlock::try_new(Uuid7::new(), Uuid7::new(), 0, "::cue { color: red }").unwrap();
    let row: PgSubtitleTrackVttStyleRow = (&s).into();
    let s2 = vtt_style_from_row(row).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn ass_style_round_trip() {
    let s = AssStyle::try_new(Uuid7::new(), Uuid7::new(), "Default")
      .unwrap()
      .with_fontname("Arial")
      .with_fontsize(48.0)
      .with_bold()
      .with_outline(2.5);
    let row: PgSubtitleTrackAssStyleRow = (&s).into();
    let s2 = ass_style_from_row(row).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn lrc_metadata_round_trip() {
    let m = LrcMetadata::try_new(Uuid7::new())
      .unwrap()
      .with_title("Song")
      .with_artist("Band")
      .with_offset_ms(-500);
    let row: PgSubtitleTrackLrcMetadataRow = (&m).into();
    let m2 = lrc_metadata_from_row(row).unwrap();
    assert_eq!(m, m2);
  }

  #[test]
  fn subtitle_track_row_with_nil_uuid_rejected() {
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    let (mut row, errs, meta): (
      PgSubtitleTrackRow,
      std::vec::Vec<PgSubtitleTrackIndexErrorRow>,
      std::vec::Vec<PgSubtitleTrackMetadataRow>,
    ) = (&t).into();
    row.id = Uuid::nil();
    assert!(SubtitleTrack::<Uuid7>::try_from((row, errs, meta))
      .unwrap_err()
      .is_invalid_uuid());
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
      .with_closed_caption(false)
      .with_translation(true)
      .with_kind(SubtitleKind::ForcedNarrative)
      .with_coverage_ratio(Some(0.97))
      .with_empty(false)
      .with_first_cue(Some(Timestamp::new(500, tb())))
      .with_last_cue(Some(Timestamp::new(119_500, tb())))
      .with_stream_index(Some(2))
      .with_container_track_id(Some(7))
      .with_index_status(
        SubtitleIndexStatus::TRACKS_DISCOVERED | SubtitleIndexStatus::CUES_EXTRACTED,
      )
      .with_index_errors(std::vec![
        ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad"),
        ErrorInfo::new(ErrorCode::PathNotFound, "gone"),
      ]);
    let (row, errs, meta): (
      PgSubtitleTrackRow,
      std::vec::Vec<PgSubtitleTrackIndexErrorRow>,
      std::vec::Vec<PgSubtitleTrackMetadataRow>,
    ) = (&t).into();
    let err_refs: std::vec::Vec<PgSubtitleTrackIndexErrorRowRef<'_>> = errs
      .iter()
      .map(PgSubtitleTrackIndexErrorRow::as_ref)
      .collect();
    let meta_refs: std::vec::Vec<PgSubtitleTrackMetadataRowRef<'_>> = meta
      .iter()
      .map(PgSubtitleTrackMetadataRow::as_ref)
      .collect();
    let t2: SubtitleTrack<Uuid7> = (row.as_ref(), err_refs, meta_refs).try_into().unwrap();
    assert_eq!(t, t2);
  }

  // ---------------------------------------------------------------------------
  // Polymorphic subtitle_cue *_ref_roundtrip tests
  //
  // For each new `*RowRef<'r>` sibling: build the owned row from a domain
  // value, borrow it via `.as_ref()`, hand the borrow to the per-format
  // `*_from_row_ref(...)` helper, and assert the domain reconstruction
  // matches. Per the fold-row-ref-into-owner convention this locks the
  // owned + Ref pair to the same round-trip semantics.
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
    let base: PgSubtitleCueBaseRow = (&c).into();
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
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueVttRow) = (&c).into();
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
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueAssRow) = (&c).into();
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
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueLrcRow) = (&c).into();
    let c2 = lrc_cue_from_row_refs(base.as_ref(), detail.as_ref(), tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn lrc_word_ref_roundtrip() {
    let w = LrcWord::try_new(Uuid7::new(), 3, "ma", 1234).unwrap();
    let row: PgSubtitleCueLrcWordRow = (&w).into();
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
    let row: PgSubtitleTrackVttRegionRow = (&r).into();
    let r2 = vtt_region_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(r, r2);
  }

  #[test]
  fn vtt_style_ref_roundtrip() {
    let s = VttStyleBlock::try_new(Uuid7::new(), Uuid7::new(), 0, "::cue { color: red }").unwrap();
    let row: PgSubtitleTrackVttStyleRow = (&s).into();
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
    let row: PgSubtitleTrackAssStyleRow = (&s).into();
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
    let row: PgSubtitleTrackLrcMetadataRow = (&m).into();
    let m2 = lrc_metadata_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(m, m2);
  }

  // ---- Long-tail formats (#56) -------------------------------------------

  #[test]
  fn micro_dvd_cue_round_trip() {
    let d = MicroDvdData::new("{y:b}hi");
    let c: MicroDvdCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("hi"),
      d,
    )
    .unwrap();
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueMicroDvdRow) = (&c).into();
    let c2 = micro_dvd_cue_from_rows(base, detail, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn micro_dvd_cue_ref_roundtrip() {
    let c: MicroDvdCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("hi"),
      MicroDvdData::new("{y:b}hi"),
    )
    .unwrap();
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueMicroDvdRow) = (&c).into();
    let c2 = micro_dvd_cue_from_row_refs(base.as_ref(), detail.as_ref(), tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn sub_viewer_cue_round_trip() {
    let d = SubViewerData::new("[b]hi[/b]");
    let c: SubViewerCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("hi"),
      d,
    )
    .unwrap();
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueSubViewerRow) = (&c).into();
    let c2 = sub_viewer_cue_from_rows(base, detail, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn sub_viewer_cue_ref_roundtrip() {
    let c: SubViewerCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("hi"),
      SubViewerData::new("[b]hi[/b]"),
    )
    .unwrap();
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueSubViewerRow) = (&c).into();
    let c2 = sub_viewer_cue_from_row_refs(base.as_ref(), detail.as_ref(), tb()).unwrap();
    assert_eq!(c, c2);
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
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueSbvRow) = (&c).into();
    let c2 = sbv_cue_from_rows(base, detail, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn sbv_cue_ref_roundtrip() {
    let c: SbvCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("plain"),
      SbvData::new(),
    )
    .unwrap();
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueSbvRow) = (&c).into();
    let c2 = sbv_cue_from_row_refs(base.as_ref(), detail.as_ref(), tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn ttml_cue_round_trip() {
    let region_id = Uuid7::new();
    let style_id = Uuid7::new();
    let d = TtmlData::<Uuid7>::new()
      .with_region_id(region_id)
      .with_style_id(style_id)
      .with_xml_id("c-1")
      .with_styled_text("<span tts:color=\"red\">hi</span>");
    let c: TtmlCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("hi"),
      d,
    )
    .unwrap();
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueTtmlRow) = (&c).into();
    let c2 = ttml_cue_from_rows(base, detail, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn ttml_cue_ref_roundtrip() {
    let d = TtmlData::<Uuid7>::new()
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
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueTtmlRow) = (&c).into();
    let c2 = ttml_cue_from_row_refs(base.as_ref(), detail.as_ref(), tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn sami_cue_round_trip() {
    let d = SamiData::new()
      .with_class_name("ENCC")
      .with_styled_text("<P><B>Hi</B></P>");
    let c: SamiCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("Hi"),
      d,
    )
    .unwrap();
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueSamiRow) = (&c).into();
    let c2 = sami_cue_from_rows(base, detail, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn sami_cue_ref_roundtrip() {
    let d = SamiData::new()
      .with_class_name("ENCC")
      .with_styled_text("<P><B>Hi</B></P>");
    let c: SamiCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("Hi"),
      d,
    )
    .unwrap();
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueSamiRow) = (&c).into();
    let c2 = sami_cue_from_row_refs(base.as_ref(), detail.as_ref(), tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn vob_sub_cue_round_trip() {
    let palette_id = Uuid7::new();
    let d = VobSubData::<Uuid7>::new(palette_id)
      .with_bitmap(Bytes::from_static(b"\x01\x02"))
      .with_width(720)
      .with_height(60)
      .with_pos(20, 540)
      .with_color_indices([1, 2, 3, 4])
      .with_contrast_indices([0, 0xF, 0xF, 0xF]);
    let c: VobSubCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::new(),
      d,
    )
    .unwrap();
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueVobSubRow) = (&c).into();
    let c2 = vob_sub_cue_from_rows(base, detail, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn vob_sub_cue_ref_roundtrip() {
    let d = VobSubData::<Uuid7>::new(Uuid7::new())
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
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueVobSubRow) = (&c).into();
    let c2 = vob_sub_cue_from_row_refs(base.as_ref(), detail.as_ref(), tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn pgs_cue_round_trip() {
    let d = PgsData::new()
      .with_bitmap(Bytes::from_static(b"\xAA\xBB"))
      .with_palette_bytes(Bytes::from_static(b"\x10\x20"))
      .with_width(1920)
      .with_height(80)
      .with_pos(0, 920)
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
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCuePgsRow) = (&c).into();
    let c2 = pgs_cue_from_rows(base, detail, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn pgs_cue_ref_roundtrip() {
    let d = PgsData::new()
      .with_bitmap(Bytes::from_static(b"\xAA"))
      .with_palette_bytes(Bytes::from_static(b"\x10"))
      .with_composition_state(0x40);
    let c: PgsCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::new(),
      d,
    )
    .unwrap();
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCuePgsRow) = (&c).into();
    let c2 = pgs_cue_from_row_refs(base.as_ref(), detail.as_ref(), tb()).unwrap();
    assert_eq!(c, c2);
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
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueCea608Row) = (&c).into();
    let c2 = cea_608_cue_from_rows(base, detail, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn cea_608_cue_ref_roundtrip() {
    let d = Cea608Data::try_new(3).unwrap().with_pac_byte_pair(0x1170);
    let c: Cea608Cue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("Hi"),
      d,
    )
    .unwrap();
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueCea608Row) = (&c).into();
    let c2 = cea_608_cue_from_row_refs(base.as_ref(), detail.as_ref(), tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn ebu_stl_cue_round_trip() {
    let d = EbuStlData::try_new(2)
      .unwrap()
      .with_subtitle_number(42)
      .with_cumulative()
      .with_vertical_pos(20)
      .with_styled_text("Hi");
    let c: EbuStlCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("Hi"),
      d,
    )
    .unwrap();
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueEbuStlRow) = (&c).into();
    let c2 = ebu_stl_cue_from_rows(base, detail, tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn ebu_stl_cue_ref_roundtrip() {
    let d = EbuStlData::try_new(2)
      .unwrap()
      .with_subtitle_number(42)
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
    let (base, detail): (PgSubtitleCueBaseRow, PgSubtitleCueEbuStlRow) = (&c).into();
    let c2 = ebu_stl_cue_from_row_refs(base.as_ref(), detail.as_ref(), tb()).unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn ttml_region_round_trip() {
    let r = TtmlRegion::try_new(Uuid7::new(), Uuid7::new(), "r1")
      .unwrap()
      .with_xml_attrs("tts:origin=\"10% 80%\"");
    let row: PgSubtitleTrackTtmlRegionRow = (&r).into();
    let r2 = ttml_region_from_row(row).unwrap();
    assert_eq!(r, r2);
  }

  #[test]
  fn ttml_region_ref_roundtrip() {
    let r = TtmlRegion::try_new(Uuid7::new(), Uuid7::new(), "r1")
      .unwrap()
      .with_xml_attrs("tts:origin=\"10% 80%\"");
    let row: PgSubtitleTrackTtmlRegionRow = (&r).into();
    let r2 = ttml_region_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(r, r2);
  }

  #[test]
  fn ttml_style_round_trip() {
    let s = TtmlStyle::try_new(Uuid7::new(), Uuid7::new(), "s1")
      .unwrap()
      .with_xml_attrs("tts:color=\"red\"");
    let row: PgSubtitleTrackTtmlStyleRow = (&s).into();
    let s2 = ttml_style_from_row(row).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn ttml_style_ref_roundtrip() {
    let s = TtmlStyle::try_new(Uuid7::new(), Uuid7::new(), "s1")
      .unwrap()
      .with_xml_attrs("tts:color=\"red\"");
    let row: PgSubtitleTrackTtmlStyleRow = (&s).into();
    let s2 = ttml_style_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn sami_style_round_trip() {
    let s = SamiStyle::try_new(Uuid7::new(), Uuid7::new(), "ENCC")
      .unwrap()
      .with_css_text("{color: yellow;}");
    let row: PgSubtitleTrackSamiStyleRow = (&s).into();
    let s2 = sami_style_from_row(row).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn sami_style_ref_roundtrip() {
    let s = SamiStyle::try_new(Uuid7::new(), Uuid7::new(), "ENCC")
      .unwrap()
      .with_css_text("{color: yellow;}");
    let row: PgSubtitleTrackSamiStyleRow = (&s).into();
    let s2 = sami_style_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn vob_sub_palette_round_trip() {
    let mut entries = [0u32; 16];
    entries[0] = 0x00_FF_00_00;
    entries[5] = 0x00_00_FF_00;
    let p = VobSubPalette::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_entries(entries);
    let row: PgSubtitleTrackVobSubPaletteRow = (&p).into();
    let p2 = vob_sub_palette_from_row(row).unwrap();
    assert_eq!(p, p2);
  }

  #[test]
  fn vob_sub_palette_ref_roundtrip() {
    let mut entries = [0u32; 16];
    entries[0] = 0x00_FF_00_00;
    let p = VobSubPalette::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_entries(entries);
    let row: PgSubtitleTrackVobSubPaletteRow = (&p).into();
    let p2 = vob_sub_palette_from_row_ref(row.as_ref()).unwrap();
    assert_eq!(p, p2);
  }
}
