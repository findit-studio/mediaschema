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
      AssCue, AssData, AssStyle, Cea608Cue, Cea608Data, EbuStlCue, EbuStlData, LrcCue, LrcData,
      LrcMetadata, LrcWord, MicroDvdCue, MicroDvdData, PgsCue, PgsData, SamiCue, SamiData,
      SamiStyle, SbvCue, SbvData, SrtCue, SrtData, SubtitleCue, SubtitleCueDetails,
      SubtitleCueKind, SubViewerCue, SubViewerData, TtmlCue, TtmlData, TtmlRegion, TtmlStyle,
      VobSubCue, VobSubData, VobSubPalette, VttCue, VttData, VttLineAlign, VttPositionAlign,
      VttRegion, VttStyleBlock, VttTextAlign, VttVertical,
    },
    facet::Subtitle,
    track::SubtitleTrack,
  },
  bitflags::SubtitleIndexStatus,
  enums::SubtitleKind,
  vo::{IndexProgress, LocalizedText},
  Uuid7,
};

use ::bytes::Bytes;

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
// IndexProgress (subtitle copy — same `vo::IndexProgress` used across facets;
// kept module-private to mirror audio/video and keep the subtitle module
// self-contained).
// ---------------------------------------------------------------------------

fn index_progress_to_bson(p: &IndexProgress) -> Bson {
  let mut d = Document::new();
  d.insert("total", Bson::Int64(p.total() as i64));
  d.insert("indexed", Bson::Int64(p.indexed() as i64));
  d.insert("failed", Bson::Int64(p.failed() as i64));
  Bson::Document(d)
}

fn index_progress_from_bson(b: Bson, field: &'static str) -> Result<IndexProgress, MongoError> {
  let mut d = as_doc(b, field)?;
  let total = as_u32(take(&mut d, "total")?, "total")?;
  let indexed = as_u32(take(&mut d, "indexed")?, "indexed")?;
  let failed = as_u32(take(&mut d, "failed")?, "failed")?;
  Ok(IndexProgress::from_parts(total, indexed, failed))
}

// ---------------------------------------------------------------------------
// Subtitle facet
// ---------------------------------------------------------------------------
//
// The `tracks` reverse-FK list is **not** stored — the `subtitle_tracks`
// collection's `parent` field drives the reverse lookup (mirrors the
// sqlx convention). Only the rollup field (`track_progress`) is
// persisted on the facet document alongside the FK to `Media`.

impl From<&Subtitle<Uuid7>> for Document {
  fn from(s: &Subtitle<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*s.id_ref()));
    d.insert("media_id", uuid7_to_bson(*s.media_id_ref()));
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
    // `tracks` is a reverse-FK list — NOT stored. Discard any stale
    // value that may exist on legacy documents.
    let _ = take_opt(&mut d, "tracks");
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
    // `cues` is a reverse-FK list — NOT stored on the track document.
    // The `subtitle_cues` collection's `parent` field drives the
    // reverse lookup (consistent with the sqlx convention).
    d.insert("provenance", provenance_to_bson(t.provenance_ref()));
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
    // `cues` is a reverse-FK list — NOT stored. Discard any stale
    // value that may exist on legacy documents.
    let _ = take_opt(&mut d, "cues");
    if let Some(b) = take_opt(&mut d, "provenance") {
      t.set_provenance(provenance_from_bson(b, "provenance")?);
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

// --- MicroDVD ----------------------------------------------------------------

impl From<&MicroDvdCue<Uuid7>> for Document {
  fn from(c: &MicroDvdCue<Uuid7>) -> Self {
    let mut d = Document::new();
    write_base(&mut d, c, SubtitleCueKind::MicroDvd);
    d.insert("styled_text", Bson::String(c.data_ref().styled_text().to_owned()));
    d
  }
}

impl TryFrom<Document> for MicroDvdCue<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let (id, st_id, ordinal, span, text) = read_base(&mut d, SubtitleCueKind::MicroDvd)?;
    let styled_text = match take_opt(&mut d, "styled_text") {
      Some(b) => as_smol(b, "styled_text")?,
      None => SmolStr::default(),
    };
    Ok(MicroDvdCue::try_new(
      id,
      st_id,
      ordinal,
      span,
      text,
      MicroDvdData::new(styled_text),
    )?)
  }
}

// --- SubViewer ---------------------------------------------------------------

impl From<&SubViewerCue<Uuid7>> for Document {
  fn from(c: &SubViewerCue<Uuid7>) -> Self {
    let mut d = Document::new();
    write_base(&mut d, c, SubtitleCueKind::SubViewer);
    d.insert("styled_text", Bson::String(c.data_ref().styled_text().to_owned()));
    d
  }
}

impl TryFrom<Document> for SubViewerCue<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let (id, st_id, ordinal, span, text) = read_base(&mut d, SubtitleCueKind::SubViewer)?;
    let styled_text = match take_opt(&mut d, "styled_text") {
      Some(b) => as_smol(b, "styled_text")?,
      None => SmolStr::default(),
    };
    Ok(SubViewerCue::try_new(
      id,
      st_id,
      ordinal,
      span,
      text,
      SubViewerData::new(styled_text),
    )?)
  }
}

// --- SBV ---------------------------------------------------------------------

impl From<&SbvCue<Uuid7>> for Document {
  fn from(c: &SbvCue<Uuid7>) -> Self {
    let mut d = Document::new();
    write_base(&mut d, c, SubtitleCueKind::Sbv);
    d
  }
}

impl TryFrom<Document> for SbvCue<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let (id, st_id, ordinal, span, text) = read_base(&mut d, SubtitleCueKind::Sbv)?;
    Ok(SbvCue::try_new(id, st_id, ordinal, span, text, SbvData::new())?)
  }
}

// --- TTML --------------------------------------------------------------------

impl From<&TtmlCue<Uuid7>> for Document {
  fn from(c: &TtmlCue<Uuid7>) -> Self {
    let mut d = Document::new();
    write_base(&mut d, c, SubtitleCueKind::Ttml);
    let t = c.data_ref();
    if let Some(id) = t.region_id_ref() {
      d.insert("region_id", uuid7_to_bson(*id));
    }
    if let Some(id) = t.style_id_ref() {
      d.insert("style_id", uuid7_to_bson(*id));
    }
    d.insert("xml_id", Bson::String(t.xml_id().to_owned()));
    d.insert("styled_text", Bson::String(t.styled_text().to_owned()));
    d
  }
}

impl TryFrom<Document> for TtmlCue<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let (id, st_id, ordinal, span, text) = read_base(&mut d, SubtitleCueKind::Ttml)?;
    let region_id = match take_opt(&mut d, "region_id") {
      Some(b) => Some(uuid7_from_bson(b, "region_id")?),
      None => None,
    };
    let style_id = match take_opt(&mut d, "style_id") {
      Some(b) => Some(uuid7_from_bson(b, "style_id")?),
      None => None,
    };
    let xml_id = match take_opt(&mut d, "xml_id") {
      Some(b) => as_smol(b, "xml_id")?,
      None => SmolStr::default(),
    };
    let styled_text = match take_opt(&mut d, "styled_text") {
      Some(b) => as_smol(b, "styled_text")?,
      None => SmolStr::default(),
    };
    let data = TtmlData::<Uuid7>::new()
      .maybe_region_id(region_id)
      .maybe_style_id(style_id)
      .with_xml_id(xml_id)
      .with_styled_text(styled_text);
    Ok(TtmlCue::try_new(id, st_id, ordinal, span, text, data)?)
  }
}

// --- SAMI --------------------------------------------------------------------

impl From<&SamiCue<Uuid7>> for Document {
  fn from(c: &SamiCue<Uuid7>) -> Self {
    let mut d = Document::new();
    write_base(&mut d, c, SubtitleCueKind::Sami);
    let s = c.data_ref();
    d.insert("class_name", Bson::String(s.class_name().to_owned()));
    d.insert("styled_text", Bson::String(s.styled_text().to_owned()));
    d
  }
}

impl TryFrom<Document> for SamiCue<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let (id, st_id, ordinal, span, text) = read_base(&mut d, SubtitleCueKind::Sami)?;
    let class_name = match take_opt(&mut d, "class_name") {
      Some(b) => as_smol(b, "class_name")?,
      None => SmolStr::default(),
    };
    let styled_text = match take_opt(&mut d, "styled_text") {
      Some(b) => as_smol(b, "styled_text")?,
      None => SmolStr::default(),
    };
    let data = SamiData::new()
      .with_class_name(class_name)
      .with_styled_text(styled_text);
    Ok(SamiCue::try_new(id, st_id, ordinal, span, text, data)?)
  }
}

// --- VobSub ------------------------------------------------------------------

fn pack_indices_i64(a: &[u8; 4]) -> i64 {
  (a[0] as i64) | ((a[1] as i64) << 8) | ((a[2] as i64) << 16) | ((a[3] as i64) << 24)
}

fn unpack_indices_i64(n: i64) -> Result<[u8; 4], MongoError> {
  let v = u32::try_from(n).map_err(|_| MongoError::IntOutOfRange {
    field: SmolStr::from("vob_sub_indices"),
    value: n,
  })?;
  Ok([
    v as u8,
    (v >> 8) as u8,
    (v >> 16) as u8,
    (v >> 24) as u8,
  ])
}

fn bytes_to_bson(b: &Bytes) -> Bson {
  Bson::Binary(::bson::Binary {
    subtype: ::bson::spec::BinarySubtype::Generic,
    bytes: b.to_vec(),
  })
}

impl From<&VobSubCue<Uuid7>> for Document {
  fn from(c: &VobSubCue<Uuid7>) -> Self {
    let mut d = Document::new();
    write_base(&mut d, c, SubtitleCueKind::VobSub);
    let v = c.data_ref();
    d.insert("palette_id", uuid7_to_bson(*v.palette_id_ref()));
    d.insert("bitmap", bytes_to_bson(v.bitmap_ref()));
    d.insert("width", Bson::Int64(i64::from(v.width())));
    d.insert("height", Bson::Int64(i64::from(v.height())));
    d.insert("pos_x", Bson::Int32(v.pos_x()));
    d.insert("pos_y", Bson::Int32(v.pos_y()));
    d.insert("color_indices", Bson::Int64(pack_indices_i64(v.color_indices())));
    d.insert(
      "contrast_indices",
      Bson::Int64(pack_indices_i64(v.contrast_indices())),
    );
    d
  }
}

impl TryFrom<Document> for VobSubCue<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let (id, st_id, ordinal, span, text) = read_base(&mut d, SubtitleCueKind::VobSub)?;
    let palette_id = uuid7_from_bson(take(&mut d, "palette_id")?, "palette_id")?;
    let bitmap = as_binary(take(&mut d, "bitmap")?, "bitmap")?;
    let width = as_u32(take(&mut d, "width")?, "width")?;
    let height = as_u32(take(&mut d, "height")?, "height")?;
    let pos_x = as_i32(take(&mut d, "pos_x")?, "pos_x")?;
    let pos_y = as_i32(take(&mut d, "pos_y")?, "pos_y")?;
    let color_indices = unpack_indices_i64(as_i64(take(&mut d, "color_indices")?, "color_indices")?)?;
    let contrast_indices = unpack_indices_i64(as_i64(
      take(&mut d, "contrast_indices")?,
      "contrast_indices",
    )?)?;
    let data = VobSubData::<Uuid7>::new(palette_id)
      .with_bitmap(Bytes::from(bitmap))
      .with_width(width)
      .with_height(height)
      .with_pos(pos_x, pos_y)
      .with_color_indices(color_indices)
      .with_contrast_indices(contrast_indices);
    Ok(VobSubCue::try_new(id, st_id, ordinal, span, text, data)?)
  }
}

// --- PGS ---------------------------------------------------------------------

impl From<&PgsCue<Uuid7>> for Document {
  fn from(c: &PgsCue<Uuid7>) -> Self {
    let mut d = Document::new();
    write_base(&mut d, c, SubtitleCueKind::Pgs);
    let p = c.data_ref();
    d.insert("bitmap", bytes_to_bson(p.bitmap_ref()));
    d.insert("width", Bson::Int64(i64::from(p.width())));
    d.insert("height", Bson::Int64(i64::from(p.height())));
    d.insert("pos_x", Bson::Int32(p.pos_x()));
    d.insert("pos_y", Bson::Int32(p.pos_y()));
    d.insert("palette_bytes", bytes_to_bson(p.palette_bytes_ref()));
    d.insert("composition_state", Bson::Int32(i32::from(p.composition_state())));
    d
  }
}

impl TryFrom<Document> for PgsCue<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let (id, st_id, ordinal, span, text) = read_base(&mut d, SubtitleCueKind::Pgs)?;
    let bitmap = as_binary(take(&mut d, "bitmap")?, "bitmap")?;
    let width = as_u32(take(&mut d, "width")?, "width")?;
    let height = as_u32(take(&mut d, "height")?, "height")?;
    let pos_x = as_i32(take(&mut d, "pos_x")?, "pos_x")?;
    let pos_y = as_i32(take(&mut d, "pos_y")?, "pos_y")?;
    let palette_bytes = as_binary(take(&mut d, "palette_bytes")?, "palette_bytes")?;
    let composition_state = as_u8(take(&mut d, "composition_state")?, "composition_state")?;
    let data = PgsData::new()
      .with_bitmap(Bytes::from(bitmap))
      .with_palette_bytes(Bytes::from(palette_bytes))
      .with_width(width)
      .with_height(height)
      .with_pos(pos_x, pos_y)
      .with_composition_state(composition_state);
    Ok(PgsCue::try_new(id, st_id, ordinal, span, text, data)?)
  }
}

// --- CEA-608 -----------------------------------------------------------------

impl From<&Cea608Cue<Uuid7>> for Document {
  fn from(c: &Cea608Cue<Uuid7>) -> Self {
    let mut d = Document::new();
    write_base(&mut d, c, SubtitleCueKind::Cea608);
    let v = c.data_ref();
    d.insert("channel", Bson::Int32(i32::from(v.channel())));
    d.insert("pac_byte_pair", Bson::Int64(i64::from(v.pac_byte_pair())));
    d.insert("styled_text", Bson::String(v.styled_text().to_owned()));
    d
  }
}

impl TryFrom<Document> for Cea608Cue<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let (id, st_id, ordinal, span, text) = read_base(&mut d, SubtitleCueKind::Cea608)?;
    let channel = as_u8(take(&mut d, "channel")?, "channel")?;
    let pac = as_u32(take(&mut d, "pac_byte_pair")?, "pac_byte_pair")?;
    let styled_text = match take_opt(&mut d, "styled_text") {
      Some(b) => as_smol(b, "styled_text")?,
      None => SmolStr::default(),
    };
    let data = Cea608Data::try_new(channel)?
      .with_pac_byte_pair(pac)
      .with_styled_text(styled_text);
    Ok(Cea608Cue::try_new(id, st_id, ordinal, span, text, data)?)
  }
}

// --- EBU STL -----------------------------------------------------------------

impl From<&EbuStlCue<Uuid7>> for Document {
  fn from(c: &EbuStlCue<Uuid7>) -> Self {
    let mut d = Document::new();
    write_base(&mut d, c, SubtitleCueKind::EbuStl);
    let e = c.data_ref();
    d.insert("subtitle_number", Bson::Int64(i64::from(e.subtitle_number())));
    d.insert("cumulative", Bson::Boolean(e.cumulative()));
    d.insert("vertical_pos", Bson::Int32(e.vertical_pos()));
    d.insert("justification", Bson::Int32(i32::from(e.justification())));
    d.insert("styled_text", Bson::String(e.styled_text().to_owned()));
    d
  }
}

impl TryFrom<Document> for EbuStlCue<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let (id, st_id, ordinal, span, text) = read_base(&mut d, SubtitleCueKind::EbuStl)?;
    let subtitle_number = as_u32(take(&mut d, "subtitle_number")?, "subtitle_number")?;
    let cumulative = as_bool(take(&mut d, "cumulative")?, "cumulative")?;
    let vertical_pos = as_i32(take(&mut d, "vertical_pos")?, "vertical_pos")?;
    let justification = as_u8(take(&mut d, "justification")?, "justification")?;
    let styled_text = match take_opt(&mut d, "styled_text") {
      Some(b) => as_smol(b, "styled_text")?,
      None => SmolStr::default(),
    };
    let data = EbuStlData::try_new(justification)?
      .with_subtitle_number(subtitle_number)
      .maybe_cumulative(cumulative)
      .with_vertical_pos(vertical_pos)
      .with_styled_text(styled_text);
    Ok(EbuStlCue::try_new(id, st_id, ordinal, span, text, data)?)
  }
}

// --- Polymorphic SubtitleCueDetails codec ------------------------------------

/// Encode any polymorphic subtitle cue (typed via the runtime-tagged
/// [`SubtitleCueDetails`] union) as a single bson `Document`. This is
/// the single-collection write path: callers don't need to know the
/// format at compile time. The on-document `kind` discriminator
/// matches the per-typed `From<&XxxCue>` shape, so a typed
/// `TryFrom<Document>` can decode back into the same kind.
impl From<&SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>>> for Document {
  fn from(c: &SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>>) -> Self {
    // Build a typed cue locally then encode via the existing per-kind
    // `From` impls — keeps the wire shape identical across the
    // typed-write + polymorphic-write paths.
    let id = *c.id_ref();
    let st = *c.subtitle_track_id_ref();
    let ord = c.ordinal();
    let span = *c.span_ref();
    let text = c.text_ref().clone();
    match c.data_ref().clone() {
      SubtitleCueDetails::Srt(d) => {
        Document::from(&SrtCue::try_new(id, st, ord, span, text, d).expect("rebuild Srt"))
      }
      SubtitleCueDetails::Vtt(d) => {
        Document::from(&VttCue::try_new(id, st, ord, span, text, d).expect("rebuild Vtt"))
      }
      SubtitleCueDetails::Ass(d) => {
        Document::from(&AssCue::try_new(id, st, ord, span, text, d).expect("rebuild Ass"))
      }
      SubtitleCueDetails::Lrc(d) => {
        Document::from(&LrcCue::try_new(id, st, ord, span, text, d).expect("rebuild Lrc"))
      }
      SubtitleCueDetails::MicroDvd(d) => Document::from(
        &MicroDvdCue::try_new(id, st, ord, span, text, d).expect("rebuild MicroDvd"),
      ),
      SubtitleCueDetails::SubViewer(d) => Document::from(
        &SubViewerCue::try_new(id, st, ord, span, text, d).expect("rebuild SubViewer"),
      ),
      SubtitleCueDetails::Sbv(d) => {
        Document::from(&SbvCue::try_new(id, st, ord, span, text, d).expect("rebuild Sbv"))
      }
      SubtitleCueDetails::Ttml(d) => {
        Document::from(&TtmlCue::try_new(id, st, ord, span, text, d).expect("rebuild Ttml"))
      }
      SubtitleCueDetails::Sami(d) => {
        Document::from(&SamiCue::try_new(id, st, ord, span, text, d).expect("rebuild Sami"))
      }
      SubtitleCueDetails::VobSub(d) => {
        Document::from(&VobSubCue::try_new(id, st, ord, span, text, d).expect("rebuild VobSub"))
      }
      SubtitleCueDetails::Pgs(d) => {
        Document::from(&PgsCue::try_new(id, st, ord, span, text, d).expect("rebuild Pgs"))
      }
      SubtitleCueDetails::Cea608(d) => {
        Document::from(&Cea608Cue::try_new(id, st, ord, span, text, d).expect("rebuild Cea608"))
      }
      SubtitleCueDetails::EbuStl(d) => {
        Document::from(&EbuStlCue::try_new(id, st, ord, span, text, d).expect("rebuild EbuStl"))
      }
    }
  }
}

/// Decode a polymorphic subtitle cue document by peeking the `kind`
/// discriminator and dispatching to the per-format `TryFrom<Document>`
/// impl.
impl TryFrom<Document> for SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>> {
  type Error = MongoError;

  fn try_from(d: Document) -> Result<Self, Self::Error> {
    let kind_i = d
      .get_i32("kind")
      .map_err(|_| MongoError::MissingField(SmolStr::from("kind")))?;
    let kind = u8::try_from(kind_i)
      .ok()
      .and_then(SubtitleCueKind::try_from_u8)
      .ok_or_else(|| MongoError::IntOutOfRange {
        field: SmolStr::from("kind"),
        value: i64::from(kind_i),
      })?;
    match kind {
      SubtitleCueKind::Srt => SrtCue::<Uuid7>::try_from(d).map(promote),
      SubtitleCueKind::Vtt => VttCue::<Uuid7>::try_from(d).map(promote),
      SubtitleCueKind::Ass => AssCue::<Uuid7>::try_from(d).map(promote),
      SubtitleCueKind::Lrc => LrcCue::<Uuid7>::try_from(d).map(promote),
      SubtitleCueKind::MicroDvd => MicroDvdCue::<Uuid7>::try_from(d).map(promote),
      SubtitleCueKind::SubViewer => SubViewerCue::<Uuid7>::try_from(d).map(promote),
      SubtitleCueKind::Sbv => SbvCue::<Uuid7>::try_from(d).map(promote),
      SubtitleCueKind::Ttml => TtmlCue::<Uuid7>::try_from(d).map(promote),
      SubtitleCueKind::Sami => SamiCue::<Uuid7>::try_from(d).map(promote),
      SubtitleCueKind::VobSub => VobSubCue::<Uuid7>::try_from(d).map(promote),
      SubtitleCueKind::Pgs => PgsCue::<Uuid7>::try_from(d).map(promote),
      SubtitleCueKind::Cea608 => Cea608Cue::<Uuid7>::try_from(d).map(promote),
      SubtitleCueKind::EbuStl => EbuStlCue::<Uuid7>::try_from(d).map(promote),
    }
  }
}

fn promote<D>(c: SubtitleCue<Uuid7, D>) -> SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>>
where
  D: Clone + Into<SubtitleCueDetails<Uuid7>>,
{
  let id = *c.id_ref();
  let st = *c.subtitle_track_id_ref();
  let ord = c.ordinal();
  let span = *c.span_ref();
  let text = c.text_ref().clone();
  let details: SubtitleCueDetails<Uuid7> = c.data_ref().clone().into();
  SubtitleCue::try_new(id, st, ord, span, text, details).expect("promote")
}

// --- Per-track aggregates: TtmlRegion / TtmlStyle / SamiStyle / VobSubPalette --

impl From<&TtmlRegion<Uuid7>> for Document {
  fn from(r: &TtmlRegion<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*r.id_ref()));
    d.insert("subtitle_track_id", uuid7_to_bson(*r.subtitle_track_id_ref()));
    d.insert("xml_id", Bson::String(r.xml_id().to_owned()));
    d.insert("xml_attrs", Bson::String(r.xml_attrs().to_owned()));
    d
  }
}

impl TryFrom<Document> for TtmlRegion<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let st_id = uuid7_from_bson(take(&mut d, "subtitle_track_id")?, "subtitle_track_id")?;
    let xml_id = as_smol(take(&mut d, "xml_id")?, "xml_id")?;
    let xml_attrs = match take_opt(&mut d, "xml_attrs") {
      Some(b) => as_smol(b, "xml_attrs")?,
      None => SmolStr::default(),
    };
    Ok(TtmlRegion::try_new(id, st_id, xml_id)?.with_xml_attrs(xml_attrs))
  }
}

impl From<&TtmlStyle<Uuid7>> for Document {
  fn from(s: &TtmlStyle<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*s.id_ref()));
    d.insert("subtitle_track_id", uuid7_to_bson(*s.subtitle_track_id_ref()));
    d.insert("xml_id", Bson::String(s.xml_id().to_owned()));
    d.insert("xml_attrs", Bson::String(s.xml_attrs().to_owned()));
    d
  }
}

impl TryFrom<Document> for TtmlStyle<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let st_id = uuid7_from_bson(take(&mut d, "subtitle_track_id")?, "subtitle_track_id")?;
    let xml_id = as_smol(take(&mut d, "xml_id")?, "xml_id")?;
    let xml_attrs = match take_opt(&mut d, "xml_attrs") {
      Some(b) => as_smol(b, "xml_attrs")?,
      None => SmolStr::default(),
    };
    Ok(TtmlStyle::try_new(id, st_id, xml_id)?.with_xml_attrs(xml_attrs))
  }
}

impl From<&SamiStyle<Uuid7>> for Document {
  fn from(s: &SamiStyle<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*s.id_ref()));
    d.insert("subtitle_track_id", uuid7_to_bson(*s.subtitle_track_id_ref()));
    d.insert("class_name", Bson::String(s.class_name().to_owned()));
    d.insert("css_text", Bson::String(s.css_text().to_owned()));
    d
  }
}

impl TryFrom<Document> for SamiStyle<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let st_id = uuid7_from_bson(take(&mut d, "subtitle_track_id")?, "subtitle_track_id")?;
    let class_name = as_smol(take(&mut d, "class_name")?, "class_name")?;
    let css_text = match take_opt(&mut d, "css_text") {
      Some(b) => as_smol(b, "css_text")?,
      None => SmolStr::default(),
    };
    Ok(SamiStyle::try_new(id, st_id, class_name)?.with_css_text(css_text))
  }
}

impl From<&VobSubPalette<Uuid7>> for Document {
  fn from(p: &VobSubPalette<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*p.id_ref()));
    d.insert("subtitle_track_id", uuid7_to_bson(*p.subtitle_track_id_ref()));
    let entries: Vec<Bson> = p.entries().iter().map(|&v| Bson::Int64(i64::from(v))).collect();
    d.insert("entries", Bson::Array(entries));
    d
  }
}

impl TryFrom<Document> for VobSubPalette<Uuid7> {
  type Error = MongoError;
  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let st_id = uuid7_from_bson(take(&mut d, "subtitle_track_id")?, "subtitle_track_id")?;
    let arr = as_array(take(&mut d, "entries")?, "entries")?;
    if arr.len() != 16 {
      return Err(MongoError::IntOutOfRange {
        field: SmolStr::from("entries.len"),
        value: i64::try_from(arr.len()).unwrap_or(i64::MAX),
      });
    }
    let mut entries = [0u32; 16];
    for (i, b) in arr.into_iter().enumerate() {
      entries[i] = as_u32(b, "entries[i]")?;
    }
    Ok(VobSubPalette::try_new(id, st_id)?.with_entries(entries))
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
      .with_track_progress(IndexProgress::from_parts(2, 1, 0));
    let doc: Document = (&s).into();
    let s2: Subtitle<Uuid7> = doc.try_into().unwrap();
    // The `tracks` reverse-FK list is intentionally not persisted — the
    // round-tripped facet has an empty `tracks` slice regardless of what
    // the source value held.
    assert!(s2.tracks_slice().is_empty());
    assert_eq!(s.media_id_ref(), s2.media_id_ref());
    assert_eq!(s.track_progress_ref(), s2.track_progress_ref());
  }

  #[test]
  fn subtitle_facet_drops_reverse_fk_tracks() {
    // A facet built with a non-empty `tracks` list round-trips to one
    // with an empty list — the document does not store the reverse FK.
    let s = Subtitle::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_tracks(vec![Uuid7::new(), Uuid7::new()])
      .with_track_progress(IndexProgress::from_parts(2, 1, 0));
    let doc: Document = (&s).into();
    assert!(!doc.contains_key("tracks"));
    let s2: Subtitle<Uuid7> = doc.try_into().unwrap();
    assert!(s2.tracks_slice().is_empty());
  }

  #[test]
  fn subtitle_track_roundtrip() {
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
      .with_provenance(Provenance::from_parts("srt", "1.0", "p", "idx"))
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
  fn subtitle_track_drops_reverse_fk_cues() {
    // `cues` is a reverse-FK list; the document neither writes it nor
    // reads stale copies — the round-tripped track has an empty slice
    // regardless of source state.
    let t = SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_cues(vec![Uuid7::new(), Uuid7::new()]);
    let doc: Document = (&t).into();
    assert!(!doc.contains_key("cues"));
    let t2: SubtitleTrack<Uuid7> = doc.try_into().unwrap();
    assert!(t2.cues_slice().is_empty());
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

  // ---- Long-tail formats (#56) -------------------------------------------

  #[test]
  fn micro_dvd_cue_roundtrip() {
    let c: MicroDvdCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(0, 500),
      LocalizedText::from_src("hi"),
      MicroDvdData::new("{y:b}hi"),
    )
    .unwrap();
    let doc: Document = (&c).into();
    let c2: MicroDvdCue<Uuid7> = doc.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn sub_viewer_cue_roundtrip() {
    let c: SubViewerCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(0, 500),
      LocalizedText::from_src("hi"),
      SubViewerData::new("[b]hi[/b]"),
    )
    .unwrap();
    let doc: Document = (&c).into();
    let c2: SubViewerCue<Uuid7> = doc.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn sbv_cue_roundtrip() {
    let c: SbvCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(0, 500),
      LocalizedText::from_src("plain"),
      SbvData::new(),
    )
    .unwrap();
    let doc: Document = (&c).into();
    let c2: SbvCue<Uuid7> = doc.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn ttml_cue_roundtrip() {
    let region_id = Uuid7::new();
    let style_id = Uuid7::new();
    let d = TtmlData::<Uuid7>::new()
      .with_region_id(region_id)
      .with_style_id(style_id)
      .with_xml_id("c-1")
      .with_styled_text("<span/>");
    let c: TtmlCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(0, 500),
      LocalizedText::from_src("hi"),
      d,
    )
    .unwrap();
    let doc: Document = (&c).into();
    let c2: TtmlCue<Uuid7> = doc.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn sami_cue_roundtrip() {
    let d = SamiData::new()
      .with_class_name("ENCC")
      .with_styled_text("<P>Hi</P>");
    let c: SamiCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(0, 500),
      LocalizedText::from_src("Hi"),
      d,
    )
    .unwrap();
    let doc: Document = (&c).into();
    let c2: SamiCue<Uuid7> = doc.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn vob_sub_cue_roundtrip() {
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
      sp(0, 500),
      LocalizedText::new(),
      d,
    )
    .unwrap();
    let doc: Document = (&c).into();
    let c2: VobSubCue<Uuid7> = doc.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn pgs_cue_roundtrip() {
    let d = PgsData::new()
      .with_bitmap(Bytes::from_static(b"\xAA"))
      .with_palette_bytes(Bytes::from_static(b"\x10"))
      .with_composition_state(0x80);
    let c: PgsCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(0, 500),
      LocalizedText::new(),
      d,
    )
    .unwrap();
    let doc: Document = (&c).into();
    let c2: PgsCue<Uuid7> = doc.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn cea_608_cue_roundtrip() {
    let d = Cea608Data::try_new(2)
      .unwrap()
      .with_pac_byte_pair(0x1170)
      .with_styled_text("Hi");
    let c: Cea608Cue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(0, 500),
      LocalizedText::from_src("Hi"),
      d,
    )
    .unwrap();
    let doc: Document = (&c).into();
    let c2: Cea608Cue<Uuid7> = doc.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn ebu_stl_cue_roundtrip() {
    let d = EbuStlData::try_new(2)
      .unwrap()
      .with_subtitle_number(42)
      .with_cumulative()
      .with_vertical_pos(20);
    let c: EbuStlCue<Uuid7> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(0, 500),
      LocalizedText::from_src("Hi"),
      d,
    )
    .unwrap();
    let doc: Document = (&c).into();
    let c2: EbuStlCue<Uuid7> = doc.try_into().unwrap();
    assert_eq!(c, c2);
  }

  #[test]
  fn ttml_region_roundtrip() {
    let r = TtmlRegion::try_new(Uuid7::new(), Uuid7::new(), "r1")
      .unwrap()
      .with_xml_attrs("tts:origin=\"10% 80%\"");
    let doc: Document = (&r).into();
    let r2: TtmlRegion<Uuid7> = doc.try_into().unwrap();
    assert_eq!(r, r2);
  }

  #[test]
  fn ttml_style_roundtrip() {
    let s = TtmlStyle::try_new(Uuid7::new(), Uuid7::new(), "s1")
      .unwrap()
      .with_xml_attrs("tts:color=\"red\"");
    let doc: Document = (&s).into();
    let s2: TtmlStyle<Uuid7> = doc.try_into().unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn sami_style_roundtrip() {
    let s = SamiStyle::try_new(Uuid7::new(), Uuid7::new(), "ENCC")
      .unwrap()
      .with_css_text("{color: yellow;}");
    let doc: Document = (&s).into();
    let s2: SamiStyle<Uuid7> = doc.try_into().unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn vob_sub_palette_roundtrip() {
    let mut entries = [0u32; 16];
    entries[0] = 0x00_FF_00_00;
    entries[5] = 0x00_00_FF_00;
    let p = VobSubPalette::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_entries(entries);
    let doc: Document = (&p).into();
    let p2: VobSubPalette<Uuid7> = doc.try_into().unwrap();
    assert_eq!(p, p2);
  }

  #[test]
  fn polymorphic_cue_roundtrip_dispatches_on_kind() {
    let style_id = Uuid7::new();
    let inner = AssData::<Uuid7>::new(style_id).with_name("X");
    let c: SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>> = SubtitleCue::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(0, 500),
      LocalizedText::new(),
      SubtitleCueDetails::Ass(inner),
    )
    .unwrap();
    let doc: Document = (&c).into();
    let c2: SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>> = doc.try_into().unwrap();
    assert_eq!(c2.data_ref().kind(), SubtitleCueKind::Ass);
  }
}
