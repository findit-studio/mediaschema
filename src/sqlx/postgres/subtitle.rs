//! PostgreSQL row shapes for the subtitle-cluster aggregates: the
//! `Subtitle` facet, `SubtitleTrack`, and `SubtitleCue`.
//!
//! Identity / FK columns are native `uuid`. Nested value-objects
//! (`Provenance`, `LocalizedText`, `Location`) are flattened into real,
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

use mediaframe::{
  codec::SubtitleCodec,
  disposition::TrackDisposition,
  lang::Language,
  subtitle::{Format, TrackOrigin},
};
use uuid::Uuid;

use crate::{
  domain::{
    aggregates::subtitle::{SubtitleCueError, SubtitleError, SubtitleTrackError},
    primitives::{ErrorInfo, Location},
    vo::{IndexProgress, LocalizedText, Provenance},
    ErrorCode, Subtitle, SubtitleCue, SubtitleIndexStatus, SubtitleKind, SubtitleTrack, Uuid7,
  },
  sqlx::{
    dto::{
      bytes_to_checksum, time_range_from_parts, timestamp_from_parts, uuid7_to_uuid, uuid_to_uuid7,
    },
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

/// PostgreSQL row shape for the [`Subtitle`] facet.
///
/// `tracks` (a `Vec<Id>` reverse of `subtitle_track.subtitle_id`) is not
/// stored; the flattened `track_progress` rollup is.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleRow {
  pub id: Uuid,
  pub parent: Uuid,
  pub track_progress_total: i64,
  pub track_progress_indexed: i64,
  pub track_progress_failed: i64,
}

impl From<&Subtitle<Uuid7>> for PgSubtitleRow {
  fn from(s: &Subtitle<Uuid7>) -> Self {
    let p = s.track_progress_ref();
    Self {
      id: uuid7_to_uuid(*s.id_ref()),
      parent: uuid7_to_uuid(*s.parent_ref()),
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
    let parent = uuid_to_uuid7(r.parent)?;
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

/// PostgreSQL row shape for [`SubtitleTrack`].
///
/// `Location::Local` flattens to `source_path_volume` (`uuid`) +
/// `source_path` (`text`); `source_path_volume` NULL discriminates an
/// absent source-path. `FileChecksum` rides as `BYTEA` (32 bytes), NULL =
/// absent. `Provenance` flattens to the same four `provenance_*` columns
/// used in `audio_track`. `cues` reverse-FK is NOT stored.
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
  /// `Location::Local` of an external `.srt`/`.vtt` (`None` for
  /// embedded). `source_path_volume` NULL discriminates absence.
  pub source_path_volume: Option<Uuid>,
  /// `Location::Local` path components joined by `/`.
  pub source_path: Option<String>,
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
  pub subtitle_track: Uuid,
  pub ordinal: i32,
  pub code: i32,
  pub message: String,
}

impl From<&SubtitleTrack<Uuid7>>
  for (
    PgSubtitleTrackRow,
    std::vec::Vec<PgSubtitleTrackIndexErrorRow>,
  )
{
  fn from(t: &SubtitleTrack<Uuid7>) -> Self {
    let id = uuid7_to_uuid(*t.id_ref());
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
        (Some(uuid7_to_uuid(*local.volume_ref())), Some(path))
      }
    };
    let row = PgSubtitleTrackRow {
      id,
      subtitle_id: uuid7_to_uuid(*t.parent_ref()),
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
      source_path_volume,
      source_path,
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
        subtitle_track: id,
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
    PgSubtitleTrackRow,
    std::vec::Vec<PgSubtitleTrackIndexErrorRow>,
  )> for SubtitleTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, mut errors): (
      PgSubtitleTrackRow,
      std::vec::Vec<PgSubtitleTrackIndexErrorRow>,
    ),
  ) -> Result<Self, Self::Error> {
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

    if let Some(vol) = r.source_path_volume {
      let path = r.source_path.unwrap_or_default();
      let volume = uuid_to_uuid7(vol)?;
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
      let code = u32_from_i32(e.code, "SubtitleTrack.index_error.code")?;
      infos.push(ErrorInfo::new(ErrorCode::from_u32(code), e.message));
    }
    t = t.with_index_errors(infos);

    Ok(t)
  }
}

// ===========================================================================
// SubtitleCue
// ===========================================================================

/// PostgreSQL row shape for [`SubtitleCue`].
///
/// `span` flattens to `start_pts` / `end_pts` + timebase num/den; the
/// `LocalizedText` fields (`text`, `ocr_text`) each flatten to
/// `<field>_src` / `<field>_translated`. `image` is inline `BYTEA`
/// (mirrors `keyframe.data`).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSubtitleCueRow {
  pub id: Uuid,
  pub parent: Uuid,
  pub index: i64,
  pub span_start_pts: i64,
  pub span_end_pts: i64,
  pub span_tb_num: i64,
  pub span_tb_den: i64,
  pub text_src: String,
  pub text_translated: String,
  pub styled_text: String,
  /// Inline rendered cue bitmap (PGS/DVBSUB); empty = absent — mirrors
  /// the domain's `Bytes` (`""` = absent, no `Option`).
  pub image: std::vec::Vec<u8>,
  pub ocr_text_src: String,
  pub ocr_text_translated: String,
}

impl From<&SubtitleCue<Uuid7>> for PgSubtitleCueRow {
  fn from(c: &SubtitleCue<Uuid7>) -> Self {
    let span = c.span_ref();
    let tb = span.timebase();
    Self {
      id: uuid7_to_uuid(*c.id_ref()),
      parent: uuid7_to_uuid(*c.parent_ref()),
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

impl TryFrom<PgSubtitleCueRow> for SubtitleCue<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgSubtitleCueRow) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let parent = uuid_to_uuid7(r.parent)?;
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
  use smol_str::SmolStr;

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
    assert_eq!(s.parent_ref(), s2.parent_ref());
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
    ) = (&t).into();
    let t2: SubtitleTrack<Uuid7> = tuple.try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn subtitle_track_roundtrip_full() {
    let en = Language::from_bcp47("en").unwrap();
    let vol = Uuid7::new();
    let location = Location::try_local_uuid7(vol, ["Movies", "subs", "en.srt"]).unwrap();
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
      .with_source_path(Some(location))
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
    let (row, mut errs): (
      PgSubtitleTrackRow,
      std::vec::Vec<PgSubtitleTrackIndexErrorRow>,
    ) = (&t).into();
    errs.reverse();
    let t2: SubtitleTrack<Uuid7> = (row, errs).try_into().unwrap();
    assert_eq!(t2.index_errors_slice().len(), 3);
    assert_eq!(t2.index_errors_slice()[0].message(), "a");
    assert_eq!(t2.index_errors_slice()[2].message(), "c");
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
    let row: PgSubtitleCueRow = (&c).into();
    let c2: SubtitleCue<Uuid7> = row.try_into().unwrap();
    assert_eq!(c, c2);
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
    let row: PgSubtitleCueRow = (&c).into();
    assert_eq!(row.image, bitmap);
    let c2: SubtitleCue<Uuid7> = row.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn subtitle_cue_roundtrip_with_styled_text() {
    let c = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1_000, tb()),
      LocalizedText::from_src("hi"),
      "{\\b1}hi{\\b0}",
      Bytes::new(),
      LocalizedText::new(),
    )
    .unwrap();
    let row: PgSubtitleCueRow = (&c).into();
    assert_eq!(row.styled_text, "{\\b1}hi{\\b0}");
    let c2: SubtitleCue<Uuid7> = row.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn subtitle_track_row_with_nil_uuid_rejected() {
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    let (mut row, errs): (
      PgSubtitleTrackRow,
      std::vec::Vec<PgSubtitleTrackIndexErrorRow>,
    ) = (&t).into();
    row.id = Uuid::nil();
    assert!(SubtitleTrack::<Uuid7>::try_from((row, errs))
      .unwrap_err()
      .is_invalid_uuid());
  }
}
