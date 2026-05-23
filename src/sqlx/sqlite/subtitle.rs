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

use mediaframe::{
  codec::SubtitleCodec,
  disposition::TrackDisposition,
  lang::Language,
  subtitle::{Format, TrackOrigin},
};

use crate::{
  domain::{
    aggregates::subtitle::{SubtitleCueError, SubtitleError, SubtitleTrackError},
    primitives::{ErrorInfo, Location},
    vo::{IndexProgress, LocalizedText, Provenance},
    ErrorCode, Subtitle, SubtitleCue, SubtitleIndexStatus, SubtitleKind, SubtitleTrack, Uuid7,
  },
  sqlx::{
    dto::{bytes_to_checksum, bytes_to_uuid7, time_range_from_parts, timestamp_from_parts},
    SqlxError,
  },
};

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
  pub parent: std::vec::Vec<u8>,
  pub track_progress_total: i64,
  pub track_progress_indexed: i64,
  pub track_progress_failed: i64,
}

impl From<&Subtitle<Uuid7>> for SqliteSubtitleRow {
  fn from(s: &Subtitle<Uuid7>) -> Self {
    let p = s.track_progress_ref();
    Self {
      id: s.id_ref().as_bytes().to_vec(),
      parent: s.parent_ref().as_bytes().to_vec(),
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
    let parent = bytes_to_uuid7(&r.parent)?;
    let progress = IndexProgress::from_parts(
      u32_from_i64(r.track_progress_total, "Subtitle.track_progress_total")?,
      u32_from_i64(r.track_progress_indexed, "Subtitle.track_progress_indexed")?,
      u32_from_i64(r.track_progress_failed, "Subtitle.track_progress_failed")?,
    );
    let s = Subtitle::try_new(id, parent)
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
  pub source_path_volume: Option<std::vec::Vec<u8>>,
  pub source_path: Option<String>,
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
  pub subtitle_track: std::vec::Vec<u8>,
  pub ordinal: i64,
  pub code: i64,
  pub message: String,
}

impl From<&SubtitleTrack<Uuid7>>
  for (
    SqliteSubtitleTrackRow,
    std::vec::Vec<SqliteSubtitleTrackIndexErrorRow>,
  )
{
  fn from(t: &SubtitleTrack<Uuid7>) -> Self {
    let id = t.id_ref().as_bytes().to_vec();
    let prov = t.provenance_ref();
    let duration = t.duration_ref();
    let first_cue = t.first_cue_ref();
    let last_cue = t.last_cue_ref();
    let (source_path_volume, source_path) = match t.source_path_ref() {
      None => (None, None),
      Some(Location::Local(local)) => {
        let path = local
          .components_slice()
          .iter()
          .map(AsRef::as_ref)
          .collect::<std::vec::Vec<&str>>()
          .join("/");
        (Some(local.volume_ref().as_bytes().to_vec()), Some(path))
      }
    };
    let row = SqliteSubtitleTrackRow {
      id: id.clone(),
      subtitle_id: t.parent_ref().as_bytes().to_vec(),
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
      source_path_volume,
      source_path,
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
        subtitle_track: id.clone(),
        ordinal: i as i64,
        code: i64::from(e.code().as_u32()),
        message: e.message().to_owned(),
      })
      .collect();
    (row, errors)
  }
}

impl
  TryFrom<(
    SqliteSubtitleTrackRow,
    std::vec::Vec<SqliteSubtitleTrackIndexErrorRow>,
  )> for SubtitleTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, mut errors): (
      SqliteSubtitleTrackRow,
      std::vec::Vec<SqliteSubtitleTrackIndexErrorRow>,
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

    if let Some(vol) = r.source_path_volume {
      let path = r.source_path.unwrap_or_default();
      let volume = bytes_to_uuid7(&vol)?;
      let location = Location::try_local_uuid7(volume, path.split('/')).map_err(|e| {
        SqlxError::DomainConstructorRejected(format!("SubtitleTrack.source_path: {e}"))
      })?;
      t = t.with_source_path(Some(location));
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

    Ok(t)
  }
}

// ===========================================================================
// SubtitleCue
// ===========================================================================

/// SQLite row shape for [`SubtitleCue`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueRow {
  pub id: std::vec::Vec<u8>,
  pub parent: std::vec::Vec<u8>,
  pub index: i64,
  pub span_start_pts: i64,
  pub span_end_pts: i64,
  pub span_tb_num: i64,
  pub span_tb_den: i64,
  pub text_src: String,
  pub text_translated: String,
  pub styled_text: String,
  pub image: std::vec::Vec<u8>,
  pub ocr_text_src: String,
  pub ocr_text_translated: String,
}

impl From<&SubtitleCue<Uuid7>> for SqliteSubtitleCueRow {
  fn from(c: &SubtitleCue<Uuid7>) -> Self {
    let span = c.span_ref();
    let tb = span.timebase();
    Self {
      id: c.id_ref().as_bytes().to_vec(),
      parent: c.parent_ref().as_bytes().to_vec(),
      index: i64::from(c.index()),
      span_start_pts: span.start_pts(),
      span_end_pts: span.end_pts(),
      span_tb_num: i64::from(tb.num()),
      span_tb_den: i64::from(tb.den().get()),
      text_src: c.text_ref().src().to_owned(),
      text_translated: c.text_ref().translated().to_owned(),
      styled_text: c.styled_text().to_owned(),
      image: c.image().to_vec(),
      ocr_text_src: c.ocr_text_ref().src().to_owned(),
      ocr_text_translated: c.ocr_text_ref().translated().to_owned(),
    }
  }
}

impl TryFrom<SqliteSubtitleCueRow> for SubtitleCue<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteSubtitleCueRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let parent = bytes_to_uuid7(&r.parent)?;
    let index = u32_from_i64(r.index, "SubtitleCue.index")?;
    let span = time_range_from_parts(
      r.span_start_pts,
      r.span_end_pts,
      r.span_tb_num,
      r.span_tb_den,
    )?;
    let text = LocalizedText::from_src_translated(r.text_src, r.text_translated);
    let ocr_text = LocalizedText::from_src_translated(r.ocr_text_src, r.ocr_text_translated);
    SubtitleCue::try_new(
      id,
      parent,
      index,
      span,
      text,
      r.styled_text,
      r.image,
      ocr_text,
    )
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
  }
}

// ===========================================================================
// Borrowed-view siblings (`*RowRef<'r>`) — zero-copy decode from `&'r Row`.
// ===========================================================================

/// Borrowed view of [`SqliteSubtitleRow`] — zero-copy decode from `&'r Row`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleRowRef<'r> {
  pub id: &'r [u8],
  pub parent: &'r [u8],
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
  pub source_path_volume: Option<&'r [u8]>,
  pub source_path: Option<&'r str>,
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
  pub subtitle_track: &'r [u8],
  pub ordinal: i64,
  pub code: i64,
  pub message: &'r str,
}

/// Borrowed view of [`SqliteSubtitleCueRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSubtitleCueRowRef<'r> {
  pub id: &'r [u8],
  pub parent: &'r [u8],
  pub index: i64,
  pub span_start_pts: i64,
  pub span_end_pts: i64,
  pub span_tb_num: i64,
  pub span_tb_den: i64,
  pub text_src: &'r str,
  pub text_translated: &'r str,
  pub styled_text: &'r str,
  pub image: &'r [u8],
  pub ocr_text_src: &'r str,
  pub ocr_text_translated: &'r str,
}

impl SqliteSubtitleRow {
  /// Cheap borrow — produces a [`SqliteSubtitleRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSubtitleRowRef<'_> {
    SqliteSubtitleRowRef {
      id: &self.id,
      parent: &self.parent,
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
      source_path_volume: self.source_path_volume.as_deref(),
      source_path: self.source_path.as_deref(),
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
      subtitle_track: &self.subtitle_track,
      ordinal: self.ordinal,
      code: self.code,
      message: &self.message,
    }
  }
}

impl SqliteSubtitleCueRow {
  /// Cheap borrow — produces a [`SqliteSubtitleCueRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSubtitleCueRowRef<'_> {
    SqliteSubtitleCueRowRef {
      id: &self.id,
      parent: &self.parent,
      index: self.index,
      span_start_pts: self.span_start_pts,
      span_end_pts: self.span_end_pts,
      span_tb_num: self.span_tb_num,
      span_tb_den: self.span_tb_den,
      text_src: &self.text_src,
      text_translated: &self.text_translated,
      styled_text: &self.styled_text,
      image: &self.image,
      ocr_text_src: &self.ocr_text_src,
      ocr_text_translated: &self.ocr_text_translated,
    }
  }
}

impl<'r> TryFrom<SqliteSubtitleRowRef<'r>> for Subtitle<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteSubtitleRowRef<'r>) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
    let parent = bytes_to_uuid7(r.parent)?;
    let progress = IndexProgress::from_parts(
      u32_from_i64(r.track_progress_total, "Subtitle.track_progress_total")?,
      u32_from_i64(r.track_progress_indexed, "Subtitle.track_progress_indexed")?,
      u32_from_i64(r.track_progress_failed, "Subtitle.track_progress_failed")?,
    );
    let s = Subtitle::try_new(id, parent)
      .map_err(|e: SubtitleError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    Ok(s.with_track_progress(progress))
  }
}

impl<'r>
  TryFrom<(
    SqliteSubtitleTrackRowRef<'r>,
    std::vec::Vec<SqliteSubtitleTrackIndexErrorRowRef<'r>>,
  )> for SubtitleTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, mut errors): (
      SqliteSubtitleTrackRowRef<'r>,
      std::vec::Vec<SqliteSubtitleTrackIndexErrorRowRef<'r>>,
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

    if let Some(vol) = r.source_path_volume {
      let path = r.source_path.unwrap_or_default();
      let volume = bytes_to_uuid7(vol)?;
      let location = Location::try_local_uuid7(volume, path.split('/')).map_err(|e| {
        SqlxError::DomainConstructorRejected(format!("SubtitleTrack.source_path: {e}"))
      })?;
      t = t.with_source_path(Some(location));
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

    Ok(t)
  }
}

impl<'r> TryFrom<SqliteSubtitleCueRowRef<'r>> for SubtitleCue<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteSubtitleCueRowRef<'r>) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
    let parent = bytes_to_uuid7(r.parent)?;
    let index = u32_from_i64(r.index, "SubtitleCue.index")?;
    let span = time_range_from_parts(
      r.span_start_pts,
      r.span_end_pts,
      r.span_tb_num,
      r.span_tb_den,
    )?;
    let text = LocalizedText::from_src_translated(r.text_src, r.text_translated);
    let ocr_text = LocalizedText::from_src_translated(r.ocr_text_src, r.ocr_text_translated);
    SubtitleCue::try_new(
      id,
      parent,
      index,
      span,
      text,
      r.styled_text,
      bytes::Bytes::copy_from_slice(r.image),
      ocr_text,
    )
    .map_err(|e: SubtitleCueError| SqlxError::DomainConstructorRejected(e.to_string()))
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
  use smol_str::SmolStr;

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
    ) = (&t).into();
    let t2: SubtitleTrack<Uuid7> = tuple.try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn subtitle_track_roundtrip_full() {
    let de = Language::from_bcp47("de").unwrap();
    let vol = Uuid7::new();
    let location = Location::try_local_uuid7(vol, ["clip.de.srt"]).unwrap();
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
      .with_source_path(Some(location))
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
    let (row, mut errs): (
      SqliteSubtitleTrackRow,
      std::vec::Vec<SqliteSubtitleTrackIndexErrorRow>,
    ) = (&t).into();
    errs.reverse();
    let t2: SubtitleTrack<Uuid7> = (row, errs).try_into().unwrap();
    assert_eq!(t2.index_errors_slice()[0].message(), "a");
    assert_eq!(t2.index_errors_slice()[2].message(), "c");
  }

  #[test]
  fn subtitle_cue_roundtrip_bitmap_with_ocr() {
    let bitmap = std::vec![1u8, 2, 3, 4];
    let c = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      5,
      TimeRange::new(500, 1_500, tb()),
      LocalizedText::new(),
      SmolStr::default(),
      Bytes::from(bitmap.clone()),
      LocalizedText::from_src("Hello (OCR)"),
    )
    .unwrap();
    let row: SqliteSubtitleCueRow = (&c).into();
    assert_eq!(row.image, bitmap);
    let c2: SubtitleCue<Uuid7> = row.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn subtitle_cue_roundtrip_text() {
    let c = SubtitleCue::try_new_text(
      Uuid7::new(),
      Uuid7::new(),
      3,
      TimeRange::new(1_000, 2_000, tb()),
      LocalizedText::from_src_translated("Hola", "Hello"),
    )
    .unwrap();
    let row: SqliteSubtitleCueRow = (&c).into();
    let c2: SubtitleCue<Uuid7> = row.try_into().unwrap();
    assert_eq!(c, c2);
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
    let vol = Uuid7::new();
    let location = Location::try_local_uuid7(vol, ["clip.de.srt"]).unwrap();
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
      .with_source_path(Some(location))
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
    let (row, errs): (
      SqliteSubtitleTrackRow,
      std::vec::Vec<SqliteSubtitleTrackIndexErrorRow>,
    ) = (&t).into();
    let err_refs: std::vec::Vec<SqliteSubtitleTrackIndexErrorRowRef<'_>> = errs
      .iter()
      .map(SqliteSubtitleTrackIndexErrorRow::as_ref)
      .collect();
    let t2: SubtitleTrack<Uuid7> = (row.as_ref(), err_refs).try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn subtitle_cue_ref_roundtrip() {
    let bitmap = std::vec![1u8, 2, 3, 4];
    let c = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      5,
      TimeRange::new(500, 1_500, tb()),
      LocalizedText::new(),
      SmolStr::default(),
      Bytes::from(bitmap),
      LocalizedText::from_src("Hello (OCR)"),
    )
    .unwrap();
    let row: SqliteSubtitleCueRow = (&c).into();
    let c2: SubtitleCue<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(c, c2);
  }
}
