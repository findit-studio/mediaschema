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
  aggregates::subtitle::{
    cue::{
      AssCue, AssData, AssStyle, LrcCue, LrcData, LrcMetadata, LrcWord, SrtCue, SrtData,
      SubtitleCue, SubtitleCueKind, VttCue, VttData, VttLineAlign, VttPositionAlign, VttRegion,
      VttStyleBlock, VttTextAlign, VttVertical,
    },
    facet::Subtitle,
    track::SubtitleTrack,
  },
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
// SubtitleCue — polymorphic per-format bson documents
//
// Each cue kind has its own bson `Document` impl. The `kind` SMALLINT-like
// `Int32` discriminator is stored on the document so callers can dispatch
// without a JOIN. Detail fields ride alongside the base fields on the
// same document (mongodb favours embedded shape over a join).
// ---------------------------------------------------------------------------

fn write_base<D>(d: &mut Document, c: &SubtitleCue<Uuid7, D>, kind: SubtitleCueKind) {
  d.insert("_id", uuid7_to_bson(*c.id_ref()));
  d.insert("subtitle_track_id", uuid7_to_bson(*c.subtitle_track_id_ref()));
  d.insert("ordinal", Bson::Int64(c.ordinal() as i64));
  d.insert("span", time_range_to_bson(c.span_ref()));
  d.insert("text", loc_text_to_bson(c.text_ref()));
  d.insert("kind", Bson::Int32(i32::from(kind.to_u8())));
}

fn read_base(
  d: &mut Document,
  expected: SubtitleCueKind,
) -> Result<
  (
    Uuid7,
    Uuid7,
    u32,
    mediatime::TimeRange,
    LocalizedText,
  ),
  MongoError,
> {
  let id = uuid7_from_bson(take(d, "_id")?, "_id")?;
  let subtitle_track_id = uuid7_from_bson(take(d, "subtitle_track_id")?, "subtitle_track_id")?;
  let ordinal = as_u32(take(d, "ordinal")?, "ordinal")?;
  let span = time_range_from_bson(take(d, "span")?, "span")?;
  let text = match take_opt(d, "text") {
    Some(b) => loc_text_from_bson(b, "text")?,
    None => LocalizedText::new(),
  };
  let kind_i = as_i64(take(d, "kind")?, "kind")?;
  let kind = u8::try_from(kind_i)
    .ok()
    .and_then(SubtitleCueKind::try_from_u8)
    .ok_or_else(|| MongoError::IntOutOfRange {
      field: SmolStr::from("kind"),
      value: kind_i,
    })?;
  if kind != expected {
    return Err(MongoError::IntOutOfRange {
      field: SmolStr::from("kind"),
      value: kind_i,
    });
  }
  Ok((id, subtitle_track_id, ordinal, span, text))
}

// --- SRT ---------------------------------------------------------------------

impl From<&SrtCue<Uuid7>> for Document {
  fn from(c: &SrtCue<Uuid7>) -> Self {
    let mut d = Document::new();
    write_base(&mut d, c, SubtitleCueKind::Srt);
    d
  }
}

impl TryFrom<Document> for SrtCue<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let (id, st_id, ordinal, span, text) = read_base(&mut d, SubtitleCueKind::Srt)?;
    Ok(SrtCue::try_new(id, st_id, ordinal, span, text, SrtData)?)
  }
}

// --- WebVTT ------------------------------------------------------------------

impl From<&VttCue<Uuid7>> for Document {
  fn from(c: &VttCue<Uuid7>) -> Self {
    let mut d = Document::new();
    write_base(&mut d, c, SubtitleCueKind::Vtt);
    let v = c.data_ref();
    d.insert("cue_identifier", Bson::String(v.cue_identifier().to_owned()));
    if let Some(x) = v.vertical() {
      d.insert("vertical", Bson::Int32(i32::from(x.to_u8())));
    }
    d.insert("line_value", Bson::String(v.line_value().to_owned()));
    if let Some(x) = v.line_align() {
      d.insert("line_align", Bson::Int32(i32::from(x.to_u8())));
    }
    d.insert("position_value", Bson::String(v.position_value().to_owned()));
    if let Some(x) = v.position_align() {
      d.insert("position_align", Bson::Int32(i32::from(x.to_u8())));
    }
    if let Some(x) = v.size_value() {
      d.insert("size_value", Bson::Double(x as f64));
    }
    if let Some(x) = v.text_align() {
      d.insert("text_align", Bson::Int32(i32::from(x.to_u8())));
    }
    if let Some(id) = v.region_id_ref() {
      d.insert("region_id", uuid7_to_bson(*id));
    }
    d.insert("voice", Bson::String(v.voice().to_owned()));
    d.insert("styled_text", Bson::String(v.styled_text().to_owned()));
    d
  }
}

fn decode_small_vtt<T>(
  d: &mut Document,
  key: &'static str,
  decode: impl Fn(u8) -> Option<T>,
) -> Result<Option<T>, MongoError> {
  match take_opt(d, key) {
    None => Ok(None),
    Some(b) => {
      let i = as_i64(b, key)?;
      u8::try_from(i)
        .ok()
        .and_then(&decode)
        .map(Some)
        .ok_or_else(|| MongoError::IntOutOfRange {
          field: SmolStr::from(key),
          value: i,
        })
    }
  }
}

impl TryFrom<Document> for VttCue<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let (id, st_id, ordinal, span, text) = read_base(&mut d, SubtitleCueKind::Vtt)?;
    let cue_identifier = match take_opt(&mut d, "cue_identifier") {
      Some(b) => as_smol(b, "cue_identifier")?,
      None => SmolStr::default(),
    };
    let vertical = decode_small_vtt(&mut d, "vertical", VttVertical::try_from_u8)?;
    let line_value = match take_opt(&mut d, "line_value") {
      Some(b) => as_smol(b, "line_value")?,
      None => SmolStr::default(),
    };
    let line_align = decode_small_vtt(&mut d, "line_align", VttLineAlign::try_from_u8)?;
    let position_value = match take_opt(&mut d, "position_value") {
      Some(b) => as_smol(b, "position_value")?,
      None => SmolStr::default(),
    };
    let position_align =
      decode_small_vtt(&mut d, "position_align", VttPositionAlign::try_from_u8)?;
    let size_value = match take_opt(&mut d, "size_value") {
      Some(b) => Some(as_f32(b, "size_value")?),
      None => None,
    };
    let text_align = decode_small_vtt(&mut d, "text_align", VttTextAlign::try_from_u8)?;
    let region_id = match take_opt(&mut d, "region_id") {
      Some(b) => Some(uuid7_from_bson(b, "region_id")?),
      None => None,
    };
    let voice = match take_opt(&mut d, "voice") {
      Some(b) => as_smol(b, "voice")?,
      None => SmolStr::default(),
    };
    let styled_text = match take_opt(&mut d, "styled_text") {
      Some(b) => as_smol(b, "styled_text")?,
      None => SmolStr::default(),
    };
    let data = VttData::<Uuid7>::new()
      .with_cue_identifier(cue_identifier)
      .maybe_vertical(vertical)
      .with_line_value(line_value)
      .maybe_line_align(line_align)
      .with_position_value(position_value)
      .maybe_position_align(position_align)
      .maybe_size_value(size_value)
      .maybe_text_align(text_align)
      .maybe_region_id(region_id)
      .with_voice(voice)
      .with_styled_text(styled_text);
    Ok(VttCue::try_new(id, st_id, ordinal, span, text, data)?)
  }
}

// --- ASS / SSA ---------------------------------------------------------------

impl From<&AssCue<Uuid7>> for Document {
  fn from(c: &AssCue<Uuid7>) -> Self {
    let mut d = Document::new();
    write_base(&mut d, c, SubtitleCueKind::Ass);
    let a = c.data_ref();
    d.insert("layer", Bson::Int32(a.layer()));
    d.insert("style_id", uuid7_to_bson(*a.style_id_ref()));
    d.insert("name", Bson::String(a.name().to_owned()));
    d.insert("margin_l", Bson::Int32(a.margin_l()));
    d.insert("margin_r", Bson::Int32(a.margin_r()));
    d.insert("margin_v", Bson::Int32(a.margin_v()));
    d.insert("effect", Bson::String(a.effect().to_owned()));
    d.insert("styled_text", Bson::String(a.styled_text().to_owned()));
    d
  }
}

fn as_i32(b: Bson, field: &'static str) -> Result<i32, MongoError> {
  let v = as_i64(b, field)?;
  i32::try_from(v).map_err(|_| MongoError::IntOutOfRange {
    field: SmolStr::from(field),
    value: v,
  })
}

impl TryFrom<Document> for AssCue<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let (id, st_id, ordinal, span, text) = read_base(&mut d, SubtitleCueKind::Ass)?;
    let layer = as_i32(take(&mut d, "layer")?, "layer")?;
    let style_id = uuid7_from_bson(take(&mut d, "style_id")?, "style_id")?;
    let name = match take_opt(&mut d, "name") {
      Some(b) => as_smol(b, "name")?,
      None => SmolStr::default(),
    };
    let margin_l = as_i32(take(&mut d, "margin_l")?, "margin_l")?;
    let margin_r = as_i32(take(&mut d, "margin_r")?, "margin_r")?;
    let margin_v = as_i32(take(&mut d, "margin_v")?, "margin_v")?;
    let effect = match take_opt(&mut d, "effect") {
      Some(b) => as_smol(b, "effect")?,
      None => SmolStr::default(),
    };
    let styled_text = match take_opt(&mut d, "styled_text") {
      Some(b) => as_smol(b, "styled_text")?,
      None => SmolStr::default(),
    };
    let data = AssData::<Uuid7>::new(style_id)
      .with_layer(layer)
      .with_name(name)
      .with_margin_l(margin_l)
      .with_margin_r(margin_r)
      .with_margin_v(margin_v)
      .with_effect(effect)
      .with_styled_text(styled_text);
    Ok(AssCue::try_new(id, st_id, ordinal, span, text, data)?)
  }
}

// --- LRC ---------------------------------------------------------------------

impl From<&LrcCue<Uuid7>> for Document {
  fn from(c: &LrcCue<Uuid7>) -> Self {
    let mut d = Document::new();
    write_base(&mut d, c, SubtitleCueKind::Lrc);
    d.insert("has_word_timing", Bson::Boolean(c.data_ref().has_word_timing()));
    d
  }
}

impl TryFrom<Document> for LrcCue<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let (id, st_id, ordinal, span, text) = read_base(&mut d, SubtitleCueKind::Lrc)?;
    let has_word_timing = match take_opt(&mut d, "has_word_timing") {
      Some(b) => as_bool(b, "has_word_timing")?,
      None => false,
    };
    let data = LrcData::new().maybe_word_timing(has_word_timing);
    Ok(LrcCue::try_new(id, st_id, ordinal, span, text, data)?)
  }
}

// --- LRC word ----------------------------------------------------------------

impl From<&LrcWord<Uuid7>> for Document {
  fn from(w: &LrcWord<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("subtitle_cue_id", uuid7_to_bson(*w.subtitle_cue_id_ref()));
    d.insert("ordinal", Bson::Int64(w.ordinal() as i64));
    d.insert("text", Bson::String(w.text().to_owned()));
    d.insert("start_pts", Bson::Int64(w.start_pts()));
    d
  }
}

impl TryFrom<Document> for LrcWord<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let subtitle_cue_id =
      uuid7_from_bson(take(&mut d, "subtitle_cue_id")?, "subtitle_cue_id")?;
    let ordinal = as_u32(take(&mut d, "ordinal")?, "ordinal")?;
    let text = match take_opt(&mut d, "text") {
      Some(b) => as_smol(b, "text")?,
      None => SmolStr::default(),
    };
    let start_pts = as_i64(take(&mut d, "start_pts")?, "start_pts")?;
    Ok(LrcWord::try_new(subtitle_cue_id, ordinal, text, start_pts)?)
  }
}

// --- Per-track WebVTT region ------------------------------------------------

impl From<&VttRegion<Uuid7>> for Document {
  fn from(r: &VttRegion<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*r.id_ref()));
    d.insert("subtitle_track_id", uuid7_to_bson(*r.subtitle_track_id_ref()));
    d.insert("name", Bson::String(r.name().to_owned()));
    d.insert("width", Bson::Double(r.width() as f64));
    d.insert("lines", Bson::Int64(r.lines() as i64));
    d.insert("region_anchor_x", Bson::Double(r.region_anchor_x() as f64));
    d.insert("region_anchor_y", Bson::Double(r.region_anchor_y() as f64));
    d.insert("viewport_anchor_x", Bson::Double(r.viewport_anchor_x() as f64));
    d.insert("viewport_anchor_y", Bson::Double(r.viewport_anchor_y() as f64));
    d.insert("scroll_up", Bson::Boolean(r.scroll_up()));
    d
  }
}

impl TryFrom<Document> for VttRegion<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let st_id =
      uuid7_from_bson(take(&mut d, "subtitle_track_id")?, "subtitle_track_id")?;
    let name = match take_opt(&mut d, "name") {
      Some(b) => as_smol(b, "name")?,
      None => SmolStr::default(),
    };
    let r = VttRegion::try_new(id, st_id, name)?
      .with_width(as_f32(take(&mut d, "width")?, "width")?)
      .with_lines(as_u32(take(&mut d, "lines")?, "lines")?)
      .with_region_anchor(
        as_f32(take(&mut d, "region_anchor_x")?, "region_anchor_x")?,
        as_f32(take(&mut d, "region_anchor_y")?, "region_anchor_y")?,
      )
      .with_viewport_anchor(
        as_f32(take(&mut d, "viewport_anchor_x")?, "viewport_anchor_x")?,
        as_f32(take(&mut d, "viewport_anchor_y")?, "viewport_anchor_y")?,
      )
      .maybe_scroll_up(as_bool(take(&mut d, "scroll_up")?, "scroll_up")?);
    Ok(r)
  }
}

// --- Per-track WebVTT style block -------------------------------------------

impl From<&VttStyleBlock<Uuid7>> for Document {
  fn from(s: &VttStyleBlock<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*s.id_ref()));
    d.insert("subtitle_track_id", uuid7_to_bson(*s.subtitle_track_id_ref()));
    d.insert("ordinal", Bson::Int64(s.ordinal() as i64));
    d.insert("css_text", Bson::String(s.css_text().to_owned()));
    d
  }
}

impl TryFrom<Document> for VttStyleBlock<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let st_id =
      uuid7_from_bson(take(&mut d, "subtitle_track_id")?, "subtitle_track_id")?;
    let ordinal = as_u32(take(&mut d, "ordinal")?, "ordinal")?;
    let css_text = match take_opt(&mut d, "css_text") {
      Some(b) => as_smol(b, "css_text")?,
      None => SmolStr::default(),
    };
    Ok(VttStyleBlock::try_new(id, st_id, ordinal, css_text)?)
  }
}

// --- Per-track ASS style ----------------------------------------------------

impl From<&AssStyle<Uuid7>> for Document {
  fn from(s: &AssStyle<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*s.id_ref()));
    d.insert("subtitle_track_id", uuid7_to_bson(*s.subtitle_track_id_ref()));
    d.insert("name", Bson::String(s.name().to_owned()));
    d.insert("fontname", Bson::String(s.fontname().to_owned()));
    d.insert("fontsize", Bson::Double(s.fontsize() as f64));
    d.insert("primary_colour", Bson::Int64(i64::from(s.primary_colour())));
    d.insert(
      "secondary_colour",
      Bson::Int64(i64::from(s.secondary_colour())),
    );
    d.insert("outline_colour", Bson::Int64(i64::from(s.outline_colour())));
    d.insert("back_colour", Bson::Int64(i64::from(s.back_colour())));
    d.insert("bold", Bson::Boolean(s.bold()));
    d.insert("italic", Bson::Boolean(s.italic()));
    d.insert("underline", Bson::Boolean(s.underline()));
    d.insert("strikeout", Bson::Boolean(s.strikeout()));
    d.insert("scale_x", Bson::Int32(s.scale_x()));
    d.insert("scale_y", Bson::Int32(s.scale_y()));
    d.insert("spacing", Bson::Int32(s.spacing()));
    d.insert("angle", Bson::Double(s.angle() as f64));
    d.insert("border_style", Bson::Int32(i32::from(s.border_style())));
    d.insert("outline", Bson::Double(s.outline() as f64));
    d.insert("shadow", Bson::Double(s.shadow() as f64));
    d.insert("alignment", Bson::Int32(i32::from(s.alignment())));
    d.insert("margin_l", Bson::Int32(s.margin_l()));
    d.insert("margin_r", Bson::Int32(s.margin_r()));
    d.insert("margin_v", Bson::Int32(s.margin_v()));
    d.insert("encoding", Bson::Int32(s.encoding()));
    d
  }
}

impl TryFrom<Document> for AssStyle<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let st_id =
      uuid7_from_bson(take(&mut d, "subtitle_track_id")?, "subtitle_track_id")?;
    let name = as_smol(take(&mut d, "name")?, "name")?;
    let i16_of = |b: Bson, f: &'static str| -> Result<i16, MongoError> {
      let v = as_i64(b, f)?;
      i16::try_from(v).map_err(|_| MongoError::IntOutOfRange {
        field: SmolStr::from(f),
        value: v,
      })
    };
    let u32_of = |b: Bson, f: &'static str| -> Result<u32, MongoError> {
      let v = as_i64(b, f)?;
      u32::try_from(v).map_err(|_| MongoError::IntOutOfRange {
        field: SmolStr::from(f),
        value: v,
      })
    };
    let s = AssStyle::try_new(id, st_id, name)?
      .with_fontname(as_smol(take(&mut d, "fontname")?, "fontname")?)
      .with_fontsize(as_f32(take(&mut d, "fontsize")?, "fontsize")?)
      .with_primary_colour(u32_of(take(&mut d, "primary_colour")?, "primary_colour")?)
      .with_secondary_colour(u32_of(take(&mut d, "secondary_colour")?, "secondary_colour")?)
      .with_outline_colour(u32_of(take(&mut d, "outline_colour")?, "outline_colour")?)
      .with_back_colour(u32_of(take(&mut d, "back_colour")?, "back_colour")?)
      .maybe_bold(as_bool(take(&mut d, "bold")?, "bold")?)
      .maybe_italic(as_bool(take(&mut d, "italic")?, "italic")?)
      .maybe_underline(as_bool(take(&mut d, "underline")?, "underline")?)
      .maybe_strikeout(as_bool(take(&mut d, "strikeout")?, "strikeout")?)
      .with_scale_x(as_i32(take(&mut d, "scale_x")?, "scale_x")?)
      .with_scale_y(as_i32(take(&mut d, "scale_y")?, "scale_y")?)
      .with_spacing(as_i32(take(&mut d, "spacing")?, "spacing")?)
      .with_angle(as_f32(take(&mut d, "angle")?, "angle")?)
      .with_border_style(i16_of(take(&mut d, "border_style")?, "border_style")?)
      .with_outline(as_f32(take(&mut d, "outline")?, "outline")?)
      .with_shadow(as_f32(take(&mut d, "shadow")?, "shadow")?)
      .with_alignment(i16_of(take(&mut d, "alignment")?, "alignment")?)
      .with_margin_l(as_i32(take(&mut d, "margin_l")?, "margin_l")?)
      .with_margin_r(as_i32(take(&mut d, "margin_r")?, "margin_r")?)
      .with_margin_v(as_i32(take(&mut d, "margin_v")?, "margin_v")?)
      .with_encoding(as_i32(take(&mut d, "encoding")?, "encoding")?);
    Ok(s)
  }
}

// --- Per-track LRC metadata --------------------------------------------------

impl From<&LrcMetadata<Uuid7>> for Document {
  fn from(m: &LrcMetadata<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*m.subtitle_track_id_ref()));
    d.insert("title", Bson::String(m.title().to_owned()));
    d.insert("artist", Bson::String(m.artist().to_owned()));
    d.insert("album", Bson::String(m.album().to_owned()));
    d.insert("author", Bson::String(m.author().to_owned()));
    d.insert("creator", Bson::String(m.creator().to_owned()));
    d.insert("length", Bson::String(m.length().to_owned()));
    d.insert("offset_ms", Bson::Int32(m.offset_ms()));
    d
  }
}

impl TryFrom<Document> for LrcMetadata<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let m = LrcMetadata::try_new(id)?
      .with_title(as_smol(take(&mut d, "title")?, "title")?)
      .with_artist(as_smol(take(&mut d, "artist")?, "artist")?)
      .with_album(as_smol(take(&mut d, "album")?, "album")?)
      .with_author(as_smol(take(&mut d, "author")?, "author")?)
      .with_creator(as_smol(take(&mut d, "creator")?, "creator")?)
      .with_length(as_smol(take(&mut d, "length")?, "length")?)
      .with_offset_ms(as_i32(take(&mut d, "offset_ms")?, "offset_ms")?);
    Ok(m)
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
  fn srt_cue_roundtrip() {
    let c = SrtCue::try_new_srt(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(1000, 2000),
      LocalizedText::from_src_translated("hola", "hello"),
    )
    .unwrap();
    let doc: Document = (&c).into();
    let c2: SrtCue<Uuid7> = doc.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn vtt_cue_roundtrip() {
    let data = VttData::<Uuid7>::new()
      .with_cue_identifier("c1")
      .with_voice("Speaker A")
      .with_styled_text("<b>hi</b>");
    let c: VttCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(0, 1000),
      LocalizedText::from_src("hi"),
      data,
    )
    .unwrap();
    let doc: Document = (&c).into();
    let c2: VttCue<Uuid7> = doc.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn ass_cue_roundtrip() {
    let data = AssData::<Uuid7>::new(Uuid7::new())
      .with_layer(1)
      .with_name("Alice")
      .with_styled_text("{\\b1}hi{\\b0}");
    let c: AssCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(0, 1000),
      LocalizedText::new(),
      data,
    )
    .unwrap();
    let doc: Document = (&c).into();
    let c2: AssCue<Uuid7> = doc.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn lrc_cue_roundtrip() {
    let c: LrcCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(0, 500),
      LocalizedText::from_src("la la"),
      LrcData::new().with_word_timing(),
    )
    .unwrap();
    let doc: Document = (&c).into();
    let c2: LrcCue<Uuid7> = doc.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn lrc_word_roundtrip() {
    let w = LrcWord::try_new(Uuid7::new(), 2, "la", 100).unwrap();
    let doc: Document = (&w).into();
    let w2: LrcWord<Uuid7> = doc.try_into().unwrap();
    assert_eq!(w, w2);
  }

  #[test]
  fn vtt_region_roundtrip() {
    let r = VttRegion::try_new(Uuid7::new(), Uuid7::new(), "footer")
      .unwrap()
      .with_lines(2);
    let doc: Document = (&r).into();
    let r2: VttRegion<Uuid7> = doc.try_into().unwrap();
    assert_eq!(r, r2);
  }

  #[test]
  fn vtt_style_roundtrip() {
    let s = VttStyleBlock::try_new(Uuid7::new(), Uuid7::new(), 0, "::cue { color: red }").unwrap();
    let doc: Document = (&s).into();
    let s2: VttStyleBlock<Uuid7> = doc.try_into().unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn ass_style_roundtrip() {
    let s = AssStyle::try_new(Uuid7::new(), Uuid7::new(), "Default")
      .unwrap()
      .with_fontname("Arial")
      .with_bold();
    let doc: Document = (&s).into();
    let s2: AssStyle<Uuid7> = doc.try_into().unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn lrc_metadata_roundtrip() {
    let m = LrcMetadata::try_new(Uuid7::new())
      .unwrap()
      .with_title("Song")
      .with_offset_ms(-500);
    let doc: Document = (&m).into();
    let m2: LrcMetadata<Uuid7> = doc.try_into().unwrap();
    assert_eq!(m, m2);
  }

  #[test]
  fn srt_cue_missing_span_errors() {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(Uuid7::new()));
    d.insert("subtitle_track_id", uuid7_to_bson(Uuid7::new()));
    d.insert("ordinal", Bson::Int64(0));
    d.insert("kind", Bson::Int32(0));
    let err = SrtCue::<Uuid7>::try_from(d).unwrap_err();
    assert!(err.is_missing_field());
  }
}
