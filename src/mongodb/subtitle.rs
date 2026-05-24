//! `Subtitle` + `SubtitleTrack` + `SubtitleCue` ↔ bson `Document` mapping.

use ::bson::{Bson, Document};
use core::str::FromStr;
use mediaframe::{
  codec::SubtitleCodec,
  disposition::TrackDisposition,
  subtitle::{Format, TrackOrigin},
};
use smol_str::SmolStr;

use crate::domain::{
  aggregates::subtitle::{cue::SubtitleCue, facet::Subtitle, track::SubtitleTrack},
  bitflags::SubtitleIndexStatus,
  enums::SubtitleKind,
  vo::{IndexProgress, LocalizedText},
  Uuid7,
};

use super::{error::MongoError, util::*};

// ---------------------------------------------------------------------------
// SubtitleKind ↔ Int32
// ---------------------------------------------------------------------------

fn subtitle_kind_to_i32(k: SubtitleKind) -> i32 {
  match k {
    SubtitleKind::FullDialogue => 0,
    SubtitleKind::ForcedNarrative => 1,
    SubtitleKind::CommentaryText => 2,
  }
}

fn subtitle_kind_from_i64(v: i64, field: &'static str) -> Result<SubtitleKind, MongoError> {
  match v {
    0 => Ok(SubtitleKind::FullDialogue),
    1 => Ok(SubtitleKind::ForcedNarrative),
    2 => Ok(SubtitleKind::CommentaryText),
    _ => Err(MongoError::IntOutOfRange {
      field: SmolStr::from(field),
      value: v,
    }),
  }
}

// ---------------------------------------------------------------------------
// IndexProgress (subtitle copy — the one re-exported at `aggregates`)
// ---------------------------------------------------------------------------

pub(super) fn index_progress_to_bson(p: &IndexProgress) -> Bson {
  let mut d = Document::new();
  d.insert("total", Bson::Int64(p.total() as i64));
  d.insert("indexed", Bson::Int64(p.indexed() as i64));
  d.insert("failed", Bson::Int64(p.failed() as i64));
  Bson::Document(d)
}

pub(super) fn index_progress_from_bson(
  b: Bson,
  field: &'static str,
) -> Result<IndexProgress, MongoError> {
  let mut d = as_doc(b, field)?;
  let total = as_u32(take(&mut d, "total")?, "total")?;
  let indexed = as_u32(take(&mut d, "indexed")?, "indexed")?;
  let failed = as_u32(take(&mut d, "failed")?, "failed")?;
  Ok(IndexProgress::from_parts(total, indexed, failed))
}

// ---------------------------------------------------------------------------
// Subtitle facet
// ---------------------------------------------------------------------------

impl From<&Subtitle<Uuid7>> for Document {
  fn from(s: &Subtitle<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*s.id_ref()));
    d.insert("media_id", uuid7_to_bson(*s.media_id_ref()));
    d.insert("tracks", uuid7_vec_to_bson(s.tracks_slice()));
    d.insert(
      "track_progress",
      index_progress_to_bson(s.track_progress_ref()),
    );
    d
  }
}

impl TryFrom<Document> for Subtitle<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let media_id = uuid7_from_bson(take(&mut d, "media_id")?, "media_id")?;
    let mut s = Subtitle::try_new(id, media_id)?;
    if let Some(b) = take_opt(&mut d, "tracks") {
      s.set_tracks(uuid7_vec_from_bson(b, "tracks")?);
    }
    if let Some(b) = take_opt(&mut d, "track_progress") {
      s.set_track_progress(index_progress_from_bson(b, "track_progress")?);
    }
    Ok(s)
  }
}

// ---------------------------------------------------------------------------
// SubtitleTrack
// ---------------------------------------------------------------------------

impl From<&SubtitleTrack<Uuid7>> for Document {
  fn from(t: &SubtitleTrack<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*t.id_ref()));
    d.insert("subtitle_id", uuid7_to_bson(*t.subtitle_id_ref()));
    d.insert(
      "stream_index",
      t.stream_index()
        .map(|v| Bson::Int64(v as i64))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "container_track_id",
      t.container_track_id()
        .map(|v| Bson::Int64(v as i64))
        .unwrap_or(Bson::Null),
    );
    d.insert("codec", Bson::String(t.codec_ref().as_str().to_owned()));
    d.insert("format", Bson::String(t.format_ref().as_str().to_owned()));
    // `TrackOrigin` is a closed enum (no `FromStr`) — wire it as its
    // stable `to_u32`/`from_u32` code, Int32.
    d.insert("origin", Bson::Int32(t.origin_ref().to_u32() as i32));
    d.insert("language", language_to_bson(t.language_ref()));
    d.insert("title", Bson::String(t.title().to_owned()));
    // `is_image_based` is derived from `format` (no setter); not
    // persisted — it round-trips for free once `format` is restored.
    d.insert("disposition", Bson::Int64(t.disposition().to_u32() as i64));
    d.insert("is_primary", Bson::Boolean(t.is_primary()));
    d.insert("auto_selected", Bson::Boolean(t.auto_selected()));
    d.insert(
      "duration",
      t.duration_ref()
        .map(|v| media_ts_to_bson(*v))
        .unwrap_or(Bson::Null),
    );
    d.insert("cue_count", Bson::Int64(t.cue_count() as i64));
    d.insert("cues", uuid7_vec_to_bson(t.cues_slice()));
    d.insert("provenance", provenance_to_bson(t.provenance_ref()));
    d.insert(
      "source_path",
      t.source_path_ref()
        .map(location_to_bson)
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "source_checksum",
      t.source_checksum_ref()
        .map(checksum_to_bson)
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "character_encoding",
      Bson::String(t.character_encoding().to_owned()),
    );
    d.insert("bom_present", Bson::Boolean(t.bom_present()));
    d.insert("is_sdh", Bson::Boolean(t.is_sdh()));
    d.insert("is_closed_caption", Bson::Boolean(t.is_closed_caption()));
    d.insert("is_translation", Bson::Boolean(t.is_translation()));
    d.insert("kind", Bson::Int32(subtitle_kind_to_i32(t.kind())));
    d.insert(
      "coverage_ratio",
      t.coverage_ratio()
        .map(|v| Bson::Double(v as f64))
        .unwrap_or(Bson::Null),
    );
    d.insert("is_empty", Bson::Boolean(t.is_empty()));
    d.insert(
      "first_cue",
      t.first_cue_ref()
        .map(|v| media_ts_to_bson(*v))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "last_cue",
      t.last_cue_ref()
        .map(|v| media_ts_to_bson(*v))
        .unwrap_or(Bson::Null),
    );
    d.insert("index_status", Bson::Int64(t.index_status().bits() as i64));
    d.insert(
      "index_errors",
      error_info_vec_to_bson(t.index_errors_slice()),
    );
    d
  }
}

impl TryFrom<Document> for SubtitleTrack<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let subtitle_id = uuid7_from_bson(take(&mut d, "subtitle_id")?, "subtitle_id")?;
    let mut t = SubtitleTrack::try_new(id, subtitle_id)?;

    if let Some(b) = take_opt(&mut d, "stream_index") {
      t.set_stream_index(Some(as_u32(b, "stream_index")?));
    }
    if let Some(b) = take_opt(&mut d, "container_track_id") {
      t.set_container_track_id(Some(as_u64(b, "container_track_id")?));
    }
    if let Some(b) = take_opt(&mut d, "codec") {
      // `SubtitleCodec: FromStr<Err = Infallible>` (lossless via `Other`).
      let Ok(codec) = SubtitleCodec::from_str(&as_str(b, "codec")?);
      t.set_codec(codec);
    }
    if let Some(b) = take_opt(&mut d, "format") {
      // `Format: FromStr<Err = Infallible>` (lossless via `Other`).
      // `is_image_based` is derived from this and needs no separate field.
      let Ok(format) = Format::from_str(&as_str(b, "format")?);
      t.set_format(format);
    }
    if let Some(b) = take_opt(&mut d, "origin") {
      t.set_origin(TrackOrigin::from_u32(as_u32(b, "origin")?));
    }
    if let Some(b) = take_opt(&mut d, "language") {
      t.set_language(language_from_bson(b, "language")?);
    }
    if let Some(b) = take_opt(&mut d, "title") {
      t.set_title(as_smol(b, "title")?);
    }
    if let Some(b) = take_opt(&mut d, "disposition") {
      t.set_disposition(TrackDisposition::from_u32(as_u32(b, "disposition")?));
    }
    if let Some(b) = take_opt(&mut d, "is_primary") {
      t.set_primary(as_bool(b, "is_primary")?);
    }
    if let Some(b) = take_opt(&mut d, "auto_selected") {
      t.set_auto_selected(as_bool(b, "auto_selected")?);
    }
    if let Some(b) = take_opt(&mut d, "duration") {
      t.set_duration(Some(media_ts_from_bson(b, "duration")?));
    }
    if let Some(b) = take_opt(&mut d, "cue_count") {
      t.set_cue_count(as_u32(b, "cue_count")?);
    }
    if let Some(b) = take_opt(&mut d, "cues") {
      t.set_cues(uuid7_vec_from_bson(b, "cues")?);
    }
    if let Some(b) = take_opt(&mut d, "provenance") {
      t.set_provenance(provenance_from_bson(b, "provenance")?);
    }
    if let Some(b) = take_opt(&mut d, "source_path") {
      t.set_source_path(Some(location_from_bson(b, "source_path")?));
    }
    if let Some(b) = take_opt(&mut d, "source_checksum") {
      t.set_source_checksum(Some(checksum_from_bson(b, "source_checksum")?));
    }
    if let Some(b) = take_opt(&mut d, "character_encoding") {
      t.set_character_encoding(as_smol(b, "character_encoding")?);
    }
    if let Some(b) = take_opt(&mut d, "bom_present") {
      t.set_bom_present(as_bool(b, "bom_present")?);
    }
    if let Some(b) = take_opt(&mut d, "is_sdh") {
      t.set_sdh(as_bool(b, "is_sdh")?);
    }
    if let Some(b) = take_opt(&mut d, "is_closed_caption") {
      t.set_closed_caption(as_bool(b, "is_closed_caption")?);
    }
    if let Some(b) = take_opt(&mut d, "is_translation") {
      t.set_translation(as_bool(b, "is_translation")?);
    }
    if let Some(b) = take_opt(&mut d, "kind") {
      t.set_kind(subtitle_kind_from_i64(as_i64(b, "kind")?, "kind")?);
    }
    if let Some(b) = take_opt(&mut d, "coverage_ratio") {
      t.set_coverage_ratio(Some(as_f32(b, "coverage_ratio")?));
    }
    if let Some(b) = take_opt(&mut d, "is_empty") {
      t.set_empty(as_bool(b, "is_empty")?);
    }
    if let Some(b) = take_opt(&mut d, "first_cue") {
      t.set_first_cue(Some(media_ts_from_bson(b, "first_cue")?));
    }
    if let Some(b) = take_opt(&mut d, "last_cue") {
      t.set_last_cue(Some(media_ts_from_bson(b, "last_cue")?));
    }
    if let Some(b) = take_opt(&mut d, "index_status") {
      let bits = as_u64(b, "index_status")?;
      let bits32 = u32::try_from(bits).map_err(|_| MongoError::IntOutOfRange {
        field: SmolStr::from("index_status"),
        value: bits as i64,
      })?;
      t.set_index_status(SubtitleIndexStatus::from_bits_truncate(bits32));
    }
    if let Some(b) = take_opt(&mut d, "index_errors") {
      t.set_index_errors(error_info_vec_from_bson(b, "index_errors")?);
    }
    Ok(t)
  }
}

// ---------------------------------------------------------------------------
// SubtitleCue
// ---------------------------------------------------------------------------

impl From<&SubtitleCue<Uuid7>> for Document {
  fn from(c: &SubtitleCue<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*c.id_ref()));
    d.insert("subtitle_track_id", uuid7_to_bson(*c.subtitle_track_id_ref()));
    d.insert("index", Bson::Int64(c.index() as i64));
    d.insert("span", time_range_to_bson(c.span_ref()));
    d.insert("text", loc_text_to_bson(c.text_ref()));
    d.insert("styled_text", Bson::String(c.styled_text().to_owned()));
    d.insert("image", bytes_to_bson(c.image()));
    d.insert("ocr_text", loc_text_to_bson(c.ocr_text_ref()));
    d
  }
}

impl TryFrom<Document> for SubtitleCue<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let subtitle_track_id = uuid7_from_bson(take(&mut d, "subtitle_track_id")?, "subtitle_track_id")?;
    let index = as_u32(take(&mut d, "index")?, "index")?;
    let span = time_range_from_bson(take(&mut d, "span")?, "span")?;
    // `SubtitleCue::try_new` is a full-args constructor that validates
    // the text/image/ocr_text content invariant together — gather every
    // payload field first, then construct once.
    let text = match take_opt(&mut d, "text") {
      Some(b) => loc_text_from_bson(b, "text")?,
      None => LocalizedText::new(),
    };
    let styled_text = match take_opt(&mut d, "styled_text") {
      Some(b) => as_smol(b, "styled_text")?,
      None => SmolStr::default(),
    };
    let image = match take_opt(&mut d, "image") {
      Some(b) => as_binary(b, "image")?,
      None => Vec::new(),
    };
    let ocr_text = match take_opt(&mut d, "ocr_text") {
      Some(b) => loc_text_from_bson(b, "ocr_text")?,
      None => LocalizedText::new(),
    };
    Ok(SubtitleCue::try_new(
      id,
      subtitle_track_id,
      index,
      span,
      text,
      styled_text,
      image,
      ocr_text,
    )?)
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::{
    primitives::{ErrorCode, ErrorInfo, FileChecksum},
    vo::{LocalizedText, Provenance},
    Location,
  };
  use ::mediaframe::lang::Language;
  use core::num::NonZeroU32;
  use mediatime::{TimeRange, Timebase, Timestamp as MediaTimestamp};

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).unwrap())
  }

  fn sp(s: i64, e: i64) -> TimeRange {
    TimeRange::new(s, e, tb())
  }

  fn cs() -> FileChecksum {
    let mut b = [0u8; 32];
    b[0] = 7;
    FileChecksum::from_bytes(b)
  }

  #[test]
  fn subtitle_facet_roundtrip() {
    let s = Subtitle::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_tracks(vec![Uuid7::new(), Uuid7::new()])
      .with_track_progress(IndexProgress::from_parts(2, 1, 0));
    let doc: Document = (&s).into();
    let s2: Subtitle<Uuid7> = doc.try_into().unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn subtitle_track_roundtrip() {
    let vol = Uuid7::new();
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(SubtitleCodec::Subrip)
      .with_format(Format::Srt)
      .with_origin(TrackOrigin::External)
      .with_language(Language::from_bcp47("en").unwrap())
      .with_title("English (CC)")
      .with_disposition(TrackDisposition::from_u32(0x04))
      .with_primary(true)
      .with_auto_selected(false)
      .with_duration(Some(MediaTimestamp::new(60_000, tb())))
      .with_cue_count(500)
      .with_cues(vec![Uuid7::new()])
      .with_provenance(Provenance::from_parts("srt", "1.0", "p", "idx"))
      .with_source_path(Some(
        Location::try_local_uuid7(vol, ["Movies", "subs.srt"]).unwrap(),
      ))
      .with_source_checksum(Some(cs()))
      .with_character_encoding("utf-8")
      .with_bom_present(true)
      .with_sdh(true)
      .with_closed_caption(false)
      .with_translation(false)
      .with_kind(SubtitleKind::ForcedNarrative)
      .with_coverage_ratio(Some(0.98))
      .with_empty(false)
      .with_first_cue(Some(MediaTimestamp::new(1000, tb())))
      .with_last_cue(Some(MediaTimestamp::new(59_000, tb())))
      .with_index_status(SubtitleIndexStatus::all())
      .with_index_errors(vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad")]);
    let doc: Document = (&t).into();
    let t2: SubtitleTrack<Uuid7> = doc.try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn subtitle_cue_roundtrip() {
    let c = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(1000, 2000),
      LocalizedText::from_src_translated("hola", "hello"),
      "{\\b1}hello{\\b0}",
      vec![0u8, 1, 2, 3],
      LocalizedText::from_src("hello (OCR)"),
    )
    .unwrap();
    let doc: Document = (&c).into();
    let c2: SubtitleCue<Uuid7> = doc.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn subtitle_cue_missing_span_errors() {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(Uuid7::new()));
    d.insert("subtitle_track_id", uuid7_to_bson(Uuid7::new()));
    d.insert("index", Bson::Int64(0));
    let err = SubtitleCue::<Uuid7>::try_from(d).unwrap_err();
    assert!(err.is_missing_field());
  }
}
