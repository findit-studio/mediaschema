//! `Audio` + `AudioTrack` + `AudioSegment` ↔ bson `Document` mapping.
//!
//! Includes the per-recording VOs (`AudioTags`, `AudioCoverArt`,
//! `Loudness`, `AudioFingerprint`) and the per-segment `Word` VO.

use ::bson::{Bson, Document};
use core::str::FromStr;
use mediaframe::{
  audio::{ChannelLayout, CoverArt, Fingerprint, Loudness, ReplayGain, SampleFormat, Tags},
  codec::AudioCodec,
};
use smol_str::SmolStr;

use crate::domain::{
  aggregates::audio::{
    facet::Audio,
    segment::{AudioSegment, Word},
    sound_event::{SoundEvent, SoundEventError},
    track::AudioTrack,
  },
  bitflags::AudioIndexStatus,
  enums::{AudioContentKind, CedDetector},
  vo::IndexProgress,
  Uuid7,
};

use super::{error::MongoError, util::*};

// ---------------------------------------------------------------------------
// AudioContentKind ↔ Int32
// ---------------------------------------------------------------------------

fn content_kind_to_i32(k: AudioContentKind) -> i32 {
  match k {
    AudioContentKind::Speech => 0,
    AudioContentKind::Music => 1,
    AudioContentKind::Mixed => 2,
    AudioContentKind::Silence => 3,
  }
}

fn content_kind_from_i64(v: i64, field: &'static str) -> Result<AudioContentKind, MongoError> {
  match v {
    0 => Ok(AudioContentKind::Speech),
    1 => Ok(AudioContentKind::Music),
    2 => Ok(AudioContentKind::Mixed),
    3 => Ok(AudioContentKind::Silence),
    _ => Err(MongoError::IntOutOfRange {
      field: SmolStr::from(field),
      value: v,
    }),
  }
}

// ---------------------------------------------------------------------------
// CedDetector ↔ Int32
// ---------------------------------------------------------------------------

fn ced_detector_to_i32(d: CedDetector) -> i32 {
  match d {
    CedDetector::Ced => 0,
    CedDetector::Manual => 1,
  }
}

fn ced_detector_from_i64(v: i64, field: &'static str) -> Result<CedDetector, MongoError> {
  match v {
    0 => Ok(CedDetector::Ced),
    1 => Ok(CedDetector::Manual),
    _ => Err(MongoError::IntOutOfRange {
      field: SmolStr::from(field),
      value: v,
    }),
  }
}

// ---------------------------------------------------------------------------
// IndexProgress (audio copy — same `vo::IndexProgress` used across facets;
// kept module-private to mirror subtitle/video and keep the audio module
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
// Audio facet
// ---------------------------------------------------------------------------
//
// The `tracks` reverse-FK list is **not** stored — the `audio_tracks`
// collection's `parent` field drives the reverse lookup (mirrors the
// sqlx convention). Only the rollup fields are persisted on the facet
// document.

impl From<&Audio<Uuid7>> for Document {
  fn from(a: &Audio<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*a.id_ref()));
    d.insert("media_id", uuid7_to_bson(*a.media_id_ref()));
    d.insert(
      "track_progress",
      index_progress_to_bson(a.track_progress_ref()),
    );
    d.insert("total_segments", Bson::Int64(a.total_segments() as i64));
    d
  }
}

impl TryFrom<Document> for Audio<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let media_id = uuid7_from_bson(take(&mut d, "media_id")?, "media_id")?;
    let mut a = Audio::try_new(id, media_id)?;
    if let Some(b) = take_opt(&mut d, "track_progress") {
      a.set_track_progress(index_progress_from_bson(b, "track_progress")?);
    }
    if let Some(b) = take_opt(&mut d, "total_segments") {
      a.set_total_segments(as_u32(b, "total_segments")?);
    }
    Ok(a)
  }
}

// ---------------------------------------------------------------------------
// AudioTags
// ---------------------------------------------------------------------------

// `0`-means-absent `u16` → `Null` (absent) or `Int32` (present).
fn u16_opt_to_bson(v: u16) -> Bson {
  if v == 0 {
    Bson::Null
  } else {
    Bson::Int32(i32::from(v))
  }
}

// `mediaframe::audio::Tags` field set (rev: narrower than the old local
// `AudioTags`): string fields use `""`-means-absent; numeric / language
// fields use `0`/`None`-means-absent, persisted as `Null` when absent.
fn tags_to_bson(t: &Tags) -> Bson {
  let mut d = Document::new();
  d.insert("title", Bson::String(t.title().to_owned()));
  d.insert("artist", Bson::String(t.artist().to_owned()));
  d.insert("album_artist", Bson::String(t.album_artist().to_owned()));
  d.insert("album", Bson::String(t.album().to_owned()));
  d.insert("composer", Bson::String(t.composer().to_owned()));
  d.insert("genre", Bson::String(t.genre().to_owned()));
  d.insert("comment", Bson::String(t.comment().to_owned()));
  // `mediaframe::audio::Tags` numeric getters return a bare `u16` with
  // `0`-means-absent; map `0` back to `Null` so the bson form stays sparse.
  d.insert("year", u16_opt_to_bson(t.year()));
  d.insert("track_number", u16_opt_to_bson(t.track_number()));
  d.insert("track_total", u16_opt_to_bson(t.track_total()));
  d.insert("disc_number", u16_opt_to_bson(t.disc_number()));
  d.insert("disc_total", u16_opt_to_bson(t.disc_total()));
  d.insert(
    "language",
    t.language().map_or(Bson::Null, |l| language_to_bson(&l)),
  );
  Bson::Document(d)
}

fn tags_from_bson(b: Bson, field: &'static str) -> Result<Tags, MongoError> {
  let mut d = as_doc(b, field)?;
  let mut t = Tags::new()
    .with_title(as_smol(take(&mut d, "title")?, "title")?)
    .with_artist(as_smol(take(&mut d, "artist")?, "artist")?)
    .with_album_artist(as_smol(take(&mut d, "album_artist")?, "album_artist")?)
    .with_album(as_smol(take(&mut d, "album")?, "album")?)
    .with_composer(as_smol(take(&mut d, "composer")?, "composer")?)
    .with_genre(as_smol(take(&mut d, "genre")?, "genre")?)
    .with_comment(as_smol(take(&mut d, "comment")?, "comment")?);
  if let Some(b) = take_opt(&mut d, "year") {
    t = t.with_year(as_u16(b, "year")?);
  }
  if let Some(b) = take_opt(&mut d, "track_number") {
    t = t.with_track_number(as_u16(b, "track_number")?);
  }
  if let Some(b) = take_opt(&mut d, "track_total") {
    t = t.with_track_total(as_u16(b, "track_total")?);
  }
  if let Some(b) = take_opt(&mut d, "disc_number") {
    t = t.with_disc_number(as_u16(b, "disc_number")?);
  }
  if let Some(b) = take_opt(&mut d, "disc_total") {
    t = t.with_disc_total(as_u16(b, "disc_total")?);
  }
  if let Some(b) = take_opt(&mut d, "language") {
    t = t.with_language(language_from_bson(b, "language")?);
  }
  Ok(t)
}

// ---------------------------------------------------------------------------
// AudioCoverArt
// ---------------------------------------------------------------------------

fn cover_art_to_bson(c: &CoverArt) -> Bson {
  let mut d = Document::new();
  d.insert("data", bytes_to_bson(c.data()));
  d.insert("mime", Bson::String(c.mime().to_owned()));
  Bson::Document(d)
}

fn cover_art_from_bson(b: Bson, field: &'static str) -> Result<CoverArt, MongoError> {
  let mut d = as_doc(b, field)?;
  let data = as_binary(take(&mut d, "data")?, "data")?;
  let mime = as_smol(take(&mut d, "mime")?, "mime")?;
  Ok(CoverArt::try_new(mime, data)?)
}

// ---------------------------------------------------------------------------
// Loudness
// ---------------------------------------------------------------------------

fn loudness_to_bson(l: &Loudness) -> Bson {
  let mut d = Document::new();
  d.insert("integrated_lufs", Bson::Double(l.integrated_lufs() as f64));
  d.insert("range_lu", Bson::Double(l.range_lu() as f64));
  d.insert("true_peak_dbtp", Bson::Double(l.true_peak_dbtp() as f64));
  d.insert(
    "sample_peak_dbfs",
    Bson::Double(l.sample_peak_dbfs() as f64),
  );
  Bson::Document(d)
}

fn loudness_from_bson(b: Bson, field: &'static str) -> Result<Loudness, MongoError> {
  let mut d = as_doc(b, field)?;
  let i = as_f32(take(&mut d, "integrated_lufs")?, "integrated_lufs")?;
  let r = as_f32(take(&mut d, "range_lu")?, "range_lu")?;
  let p = as_f32(take(&mut d, "true_peak_dbtp")?, "true_peak_dbtp")?;
  let s = as_f32(take(&mut d, "sample_peak_dbfs")?, "sample_peak_dbfs")?;
  Ok(Loudness::new(i, r, p, s))
}

// ---------------------------------------------------------------------------
// ReplayGain
// ---------------------------------------------------------------------------

fn replay_gain_to_bson(rg: &ReplayGain) -> Bson {
  let mut d = Document::new();
  d.insert("track_gain_db", Bson::Double(rg.track_gain_db() as f64));
  d.insert("track_peak", Bson::Double(rg.track_peak() as f64));
  if let Some(v) = rg.album_gain_db() {
    d.insert("album_gain_db", Bson::Double(v as f64));
  }
  if let Some(v) = rg.album_peak() {
    d.insert("album_peak", Bson::Double(v as f64));
  }
  Bson::Document(d)
}

fn replay_gain_from_bson(b: Bson, field: &'static str) -> Result<ReplayGain, MongoError> {
  let mut d = as_doc(b, field)?;
  let tg = as_f32(take(&mut d, "track_gain_db")?, "track_gain_db")?;
  let tp = as_f32(take(&mut d, "track_peak")?, "track_peak")?;
  let ag = if let Some(b) = take_opt(&mut d, "album_gain_db") {
    Some(as_f32(b, "album_gain_db")?)
  } else {
    None
  };
  let ap = if let Some(b) = take_opt(&mut d, "album_peak") {
    Some(as_f32(b, "album_peak")?)
  } else {
    None
  };
  Ok(ReplayGain::new(tg, tp, ag, ap))
}

// ---------------------------------------------------------------------------
// AudioFingerprint
// ---------------------------------------------------------------------------

fn fingerprint_to_bson(fp: &Fingerprint) -> Bson {
  let mut d = Document::new();
  d.insert("algorithm", Bson::String(fp.algorithm().to_owned()));
  d.insert("value", bytes_to_bson(fp.value()));
  Bson::Document(d)
}

fn fingerprint_from_bson(b: Bson, field: &'static str) -> Result<Fingerprint, MongoError> {
  let mut d = as_doc(b, field)?;
  let algorithm = as_smol(take(&mut d, "algorithm")?, "algorithm")?;
  let value = as_binary(take(&mut d, "value")?, "value")?;
  Ok(Fingerprint::try_new(algorithm, value)?)
}

// ---------------------------------------------------------------------------
// AudioTrack
// ---------------------------------------------------------------------------

impl From<&AudioTrack<Uuid7>> for Document {
  fn from(t: &AudioTrack<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*t.id_ref()));
    d.insert("audio_id", uuid7_to_bson(*t.audio_id_ref()));
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
    d.insert("profile", Bson::String(t.profile().to_owned()));
    d.insert("sample_rate", Bson::Int64(t.sample_rate() as i64));
    d.insert("channels", Bson::Int32(t.channels() as i32));
    d.insert(
      "channel_layout",
      Bson::String(t.channel_layout_ref().as_str().to_owned()),
    );
    d.insert(
      "sample_format",
      Bson::Int64(i64::from(t.sample_format_ref().to_u32())),
    );
    d.insert("bit_rate", Bson::Int64(t.bit_rate() as i64));
    d.insert(
      "bit_rate_mode",
      t.bit_rate_mode()
        .map(|m| Bson::Int32(m.to_u32() as i32))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "bits_per_sample",
      t.bits_per_sample()
        .map(|v| Bson::Int32(v as i32))
        .unwrap_or(Bson::Null),
    );
    d.insert("is_lossless", Bson::Boolean(t.is_lossless()));
    d.insert(
      "duration",
      t.duration_ref()
        .map(|v| media_ts_to_bson(*v))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "start_pts",
      t.start_pts_ref()
        .map(|v| media_ts_to_bson(*v))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "language",
      t.language()
        .map(|l| language_to_bson(&l))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "detected_language",
      t.detected_language()
        .map(|l| language_to_bson(&l))
        .unwrap_or(Bson::Null),
    );
    d.insert("language_mismatch", Bson::Boolean(t.language_mismatch()));
    d.insert("disposition", Bson::Int64(t.disposition().to_u32() as i64));
    d.insert("is_primary", Bson::Boolean(t.is_primary()));
    d.insert("auto_selected", Bson::Boolean(t.auto_selected()));
    d.insert(
      "content",
      t.content()
        .map(|v| Bson::Int32(content_kind_to_i32(v)))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "speech_ratio",
      t.speech_ratio()
        .map(|v| Bson::Double(v as f64))
        .unwrap_or(Bson::Null),
    );
    d.insert("is_silent", Bson::Boolean(t.is_silent()));
    d.insert(
      "loudness",
      t.loudness_ref().map(loudness_to_bson).unwrap_or(Bson::Null),
    );
    d.insert(
      "replay_gain",
      t.replay_gain_ref()
        .map(replay_gain_to_bson)
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "fingerprint",
      t.fingerprint_ref()
        .map(fingerprint_to_bson)
        .unwrap_or(Bson::Null),
    );
    d.insert("isrc", Bson::String(t.isrc().to_owned()));
    d.insert("acoustid", Bson::String(t.acoustid().to_owned()));
    d.insert(
      "musicbrainz_recording_id",
      Bson::String(t.musicbrainz_recording_id().to_owned()),
    );
    // `speakers` + `segments` are reverse-FK lists — NOT stored on the
    // track document. The `speakers` collection's `parent` field and the
    // `audio_segments` collection's `parent` field drive the reverse
    // lookups (consistent with the sqlx convention).
    d.insert("tags", t.tags_ref().map(tags_to_bson).unwrap_or(Bson::Null));
    d.insert(
      "cover_art",
      t.cover_art_ref()
        .map(cover_art_to_bson)
        .unwrap_or(Bson::Null),
    );
    d.insert("provenance", provenance_to_bson(t.provenance_ref()));
    d.insert("vad_provenance", provenance_to_bson(t.vad_provenance_ref()));
    d.insert("ced_provenance", provenance_to_bson(t.ced_provenance_ref()));
    d.insert("index_status", Bson::Int64(t.index_status().bits() as i64));
    d.insert(
      "index_errors",
      error_info_vec_to_bson(t.index_errors_slice()),
    );
    d.insert("metadata", metadata_to_bson(t.metadata_ref()));
    d
  }
}

impl TryFrom<Document> for AudioTrack<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let audio_id = uuid7_from_bson(take(&mut d, "audio_id")?, "audio_id")?;
    let mut t = AudioTrack::try_new(id, audio_id)?;

    if let Some(b) = take_opt(&mut d, "stream_index") {
      t.set_stream_index(Some(as_u32(b, "stream_index")?));
    }
    if let Some(b) = take_opt(&mut d, "container_track_id") {
      t.set_container_track_id(Some(as_u64(b, "container_track_id")?));
    }
    if let Some(b) = take_opt(&mut d, "codec") {
      // `AudioCodec: FromStr<Err = Infallible>` — unknown slugs land in
      // `Other`, so the parse is total (the `unwrap` cannot panic).
      let s = as_str(b, "codec")?;
      let Ok(codec) = AudioCodec::from_str(&s);
      t.set_codec(codec);
    }
    if let Some(b) = take_opt(&mut d, "profile") {
      t.set_profile(as_smol(b, "profile")?);
    }
    if let Some(b) = take_opt(&mut d, "sample_rate") {
      t.try_set_sample_rate(as_u32(b, "sample_rate")?)?;
    }
    if let Some(b) = take_opt(&mut d, "channels") {
      t.try_set_channels(as_u16(b, "channels")?)?;
    }
    if let Some(b) = take_opt(&mut d, "channel_layout") {
      // `ChannelLayout: FromStr<Err = Infallible>` (lossless via `Other`).
      let s = as_str(b, "channel_layout")?;
      let Ok(layout) = ChannelLayout::from_str(&s);
      t.set_channel_layout(layout);
    }
    if let Some(b) = take_opt(&mut d, "sample_format") {
      t.set_sample_format(SampleFormat::from_u32(as_u32(b, "sample_format")?));
    }
    if let Some(b) = take_opt(&mut d, "bit_rate") {
      t.set_bit_rate(as_u64(b, "bit_rate")?);
    }
    if let Some(b) = take_opt(&mut d, "bit_rate_mode") {
      t.set_bit_rate_mode(Some(mediaframe::audio::BitRateMode::from_u32(as_u32(
        b,
        "bit_rate_mode",
      )?)));
    }
    if let Some(b) = take_opt(&mut d, "bits_per_sample") {
      t.set_bits_per_sample(Some(as_u16(b, "bits_per_sample")?));
    }
    if let Some(b) = take_opt(&mut d, "is_lossless") {
      t.set_lossless(as_bool(b, "is_lossless")?);
    }
    if let Some(b) = take_opt(&mut d, "duration") {
      t.try_set_duration(Some(media_ts_from_bson(b, "duration")?))?;
    }
    if let Some(b) = take_opt(&mut d, "start_pts") {
      t.set_start_pts(Some(media_ts_from_bson(b, "start_pts")?));
    }
    if let Some(b) = take_opt(&mut d, "language") {
      t.set_language(Some(language_from_bson(b, "language")?));
    }
    if let Some(b) = take_opt(&mut d, "detected_language") {
      t.set_detected_language(Some(language_from_bson(b, "detected_language")?));
    }
    // `language_mismatch` is a derived getter on `AudioTrack` (computed
    // from `language` vs `detected_language`); it is persisted only as a
    // queryable denormalized field. Drop any stored value on read — the
    // domain recomputes it.
    let _ = take_opt(&mut d, "language_mismatch");
    if let Some(b) = take_opt(&mut d, "disposition") {
      t.set_disposition(mediaframe::disposition::TrackDisposition::from_u32(as_u32(
        b,
        "disposition",
      )?));
    }
    if let Some(b) = take_opt(&mut d, "is_primary") {
      t.set_primary(as_bool(b, "is_primary")?);
    }
    if let Some(b) = take_opt(&mut d, "auto_selected") {
      t.set_auto_selected(as_bool(b, "auto_selected")?);
    }
    if let Some(b) = take_opt(&mut d, "content") {
      t.set_content(Some(content_kind_from_i64(
        as_i64(b, "content")?,
        "content",
      )?));
    }
    if let Some(b) = take_opt(&mut d, "speech_ratio") {
      t.try_set_speech_ratio(Some(as_f32(b, "speech_ratio")?))?;
    }
    if let Some(b) = take_opt(&mut d, "is_silent") {
      t.set_silent(as_bool(b, "is_silent")?);
    }
    if let Some(b) = take_opt(&mut d, "loudness") {
      t.set_loudness(Some(loudness_from_bson(b, "loudness")?));
    }
    if let Some(b) = take_opt(&mut d, "replay_gain") {
      t.set_replay_gain(Some(replay_gain_from_bson(b, "replay_gain")?));
    }
    if let Some(b) = take_opt(&mut d, "fingerprint") {
      t.set_fingerprint(Some(fingerprint_from_bson(b, "fingerprint")?));
    }
    if let Some(b) = take_opt(&mut d, "isrc") {
      t.set_isrc(as_smol(b, "isrc")?);
    }
    if let Some(b) = take_opt(&mut d, "acoustid") {
      t.set_acoustid(as_smol(b, "acoustid")?);
    }
    if let Some(b) = take_opt(&mut d, "musicbrainz_recording_id") {
      t.set_musicbrainz_recording_id(as_smol(b, "musicbrainz_recording_id")?);
    }
    // `speakers` + `segments` are reverse-FK lists — NOT stored. Discard
    // any stale values that may exist on legacy documents.
    let _ = take_opt(&mut d, "speakers");
    let _ = take_opt(&mut d, "segments");
    if let Some(b) = take_opt(&mut d, "tags") {
      t.set_tags(Some(tags_from_bson(b, "tags")?));
    }
    if let Some(b) = take_opt(&mut d, "cover_art") {
      t.set_cover_art(Some(cover_art_from_bson(b, "cover_art")?));
    }
    if let Some(b) = take_opt(&mut d, "provenance") {
      t.set_provenance(provenance_from_bson(b, "provenance")?);
    }
    if let Some(b) = take_opt(&mut d, "vad_provenance") {
      t.set_vad_provenance(provenance_from_bson(b, "vad_provenance")?);
    }
    if let Some(b) = take_opt(&mut d, "ced_provenance") {
      t.set_ced_provenance(provenance_from_bson(b, "ced_provenance")?);
    }
    if let Some(b) = take_opt(&mut d, "index_status") {
      let bits = as_u64(b, "index_status")?;
      let bits32 = u32::try_from(bits).map_err(|_| MongoError::IntOutOfRange {
        field: SmolStr::from("index_status"),
        value: bits as i64,
      })?;
      t.try_set_index_status(AudioIndexStatus::from_bits_truncate(bits32))?;
    }
    if let Some(b) = take_opt(&mut d, "index_errors") {
      t.set_index_errors(error_info_vec_from_bson(b, "index_errors")?);
    }
    if let Some(b) = take_opt(&mut d, "metadata") {
      t.set_metadata(metadata_from_bson(b, "metadata")?);
    }
    Ok(t)
  }
}

// ---------------------------------------------------------------------------
// Word
// ---------------------------------------------------------------------------

fn word_to_bson(w: &Word) -> Bson {
  let mut d = Document::new();
  d.insert("text", Bson::String(w.text().to_owned()));
  d.insert("span", time_range_to_bson(w.span_ref()));
  d.insert("score", Bson::Double(w.score() as f64));
  d.insert(
    "language",
    w.language()
      .map(|l| language_to_bson(&l))
      .unwrap_or(Bson::Null),
  );
  Bson::Document(d)
}

fn word_from_bson(b: Bson, field: &'static str) -> Result<Word, MongoError> {
  let mut d = as_doc(b, field)?;
  let text = as_smol(take(&mut d, "text")?, "text")?;
  let span = time_range_from_bson(take(&mut d, "span")?, "span")?;
  let score = as_f32(take(&mut d, "score")?, "score")?;
  let language = opt(take_opt(&mut d, "language"), |bb| {
    language_from_bson(bb, "language")
  })?;
  Ok(Word::try_from_parts(text, span, score, language)?)
}

// ---------------------------------------------------------------------------
// AudioSegment
// ---------------------------------------------------------------------------

impl From<&AudioSegment<Uuid7>> for Document {
  fn from(s: &AudioSegment<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*s.id_ref()));
    d.insert("audio_track_id", uuid7_to_bson(*s.audio_track_id_ref()));
    d.insert("index", Bson::Int64(s.index() as i64));
    d.insert("span", time_range_to_bson(s.span_ref()));
    d.insert(
      "speaker_id",
      s.speaker_id_ref()
        .map(|i| uuid7_to_bson(*i))
        .unwrap_or(Bson::Null),
    );
    d.insert("text", loc_text_to_bson(s.text_ref()));
    d.insert(
      "language",
      s.language()
        .map(|l| language_to_bson(&l))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "words",
      Bson::Array(s.words_slice().iter().map(word_to_bson).collect()),
    );
    d.insert(
      "no_speech_prob",
      s.no_speech_prob()
        .map(|v| Bson::Double(v as f64))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "avg_logprob",
      s.avg_logprob()
        .map(|v| Bson::Double(v as f64))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "temperature",
      s.temperature()
        .map(|v| Bson::Double(v as f64))
        .unwrap_or(Bson::Null),
    );
    // `voice_fingerprint` rides as an embedded sub-document (or `Null`).
    d.insert(
      "voice_fingerprint",
      s.voice_fingerprint_ref()
        .map(voice_fingerprint_to_bson)
        .unwrap_or(Bson::Null),
    );
    d
  }
}

impl TryFrom<Document> for AudioSegment<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let audio_track_id = uuid7_from_bson(take(&mut d, "audio_track_id")?, "audio_track_id")?;
    let index = as_u32(take(&mut d, "index")?, "index")?;
    let span = time_range_from_bson(take(&mut d, "span")?, "span")?;
    let mut s = AudioSegment::try_new(id, audio_track_id, index, span)?;

    if let Some(b) = take_opt(&mut d, "speaker_id") {
      s.set_speaker_id(Some(uuid7_from_bson(b, "speaker_id")?));
    }
    if let Some(b) = take_opt(&mut d, "text") {
      s.set_text(loc_text_from_bson(b, "text")?);
    }
    if let Some(b) = take_opt(&mut d, "language") {
      s.set_language(Some(language_from_bson(b, "language")?));
    }
    if let Some(b) = take_opt(&mut d, "words") {
      let arr = as_array(b, "words")?;
      let mut vw = Vec::with_capacity(arr.len());
      for w in arr {
        vw.push(word_from_bson(w, "words[]")?);
      }
      s.try_set_words(vw)?;
    }
    if let Some(b) = take_opt(&mut d, "no_speech_prob") {
      s.try_set_no_speech_prob(Some(as_f32(b, "no_speech_prob")?))?;
    }
    if let Some(b) = take_opt(&mut d, "avg_logprob") {
      s.set_avg_logprob(Some(as_f32(b, "avg_logprob")?));
    }
    if let Some(b) = take_opt(&mut d, "temperature") {
      s.set_temperature(Some(as_f32(b, "temperature")?));
    }
    if let Some(b) = take_opt(&mut d, "voice_fingerprint") {
      s.set_voice_fingerprint(Some(voice_fingerprint_from_bson(b, "voice_fingerprint")?));
    }
    Ok(s)
  }
}

// ---------------------------------------------------------------------------
// SoundEvent
// ---------------------------------------------------------------------------
//
// The audio analog of `Scene`: a flat document with no nested collection.
// `detector` rides as Int32 (mirroring `Scene.detector`); `code`
// (`Option<u64>`) rides as Int64 or `Null`.

impl From<&SoundEvent<Uuid7>> for Document {
  fn from(e: &SoundEvent<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*e.id_ref()));
    d.insert("audio_track_id", uuid7_to_bson(*e.audio_track_id_ref()));
    d.insert("index", Bson::Int64(e.index() as i64));
    d.insert("span", time_range_to_bson(e.span_ref()));
    d.insert("label", Bson::String(e.label().to_owned()));
    d.insert(
      "code",
      e.code()
        .map(|c| Bson::Int64(c as i64))
        .unwrap_or(Bson::Null),
    );
    d.insert("score", Bson::Double(e.score() as f64));
    d.insert("detector", Bson::Int32(ced_detector_to_i32(e.detector())));
    d
  }
}

impl TryFrom<Document> for SoundEvent<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let audio_track_id = uuid7_from_bson(take(&mut d, "audio_track_id")?, "audio_track_id")?;
    let index = as_u32(take(&mut d, "index")?, "index")?;
    let span = time_range_from_bson(take(&mut d, "span")?, "span")?;
    let label = as_smol(take(&mut d, "label")?, "label")?;
    let code = opt(take_opt(&mut d, "code"), |b| as_u64(b, "code"))?;
    let score = as_f32(take(&mut d, "score")?, "score")?;
    let detector =
      ced_detector_from_i64(as_i64(take(&mut d, "detector")?, "detector")?, "detector")?;
    SoundEvent::try_new(
      id,
      audio_track_id,
      index,
      span,
      label,
      code,
      score,
      detector,
    )
    .map_err(|e: SoundEventError| MongoError::DomainConstructorRejected(e.to_string()))
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::{
    primitives::{ErrorCode, ErrorInfo},
    vo::{LocalizedText, Provenance},
  };
  use ::mediaframe::{audio::BitRateMode, disposition::TrackDisposition, lang::Language};
  use core::num::NonZeroU32;
  use mediatime::{TimeRange, Timebase};

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).unwrap())
  }

  fn sp(s: i64, e: i64) -> TimeRange {
    TimeRange::new(s, e, tb())
  }

  #[test]
  fn audio_facet_roundtrip() {
    let a = Audio::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_total_segments(7)
      .with_track_progress(IndexProgress::try_new(3, 2, 1).unwrap());
    let doc: Document = (&a).into();
    let a2: Audio<Uuid7> = doc.try_into().unwrap();
    // The `tracks` reverse-FK list is intentionally not persisted — the
    // round-tripped facet has an empty `tracks` slice regardless of what
    // the source value held.
    assert!(a2.tracks_slice().is_empty());
    assert_eq!(a.total_segments(), a2.total_segments());
    assert_eq!(a.track_progress_ref(), a2.track_progress_ref());
  }

  #[test]
  fn audio_facet_drops_reverse_fk_tracks() {
    // A facet built with a non-empty `tracks` list round-trips to one
    // with an empty list — the document does not store the reverse FK.
    let a = Audio::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_tracks(vec![Uuid7::new(), Uuid7::new()])
      .with_total_segments(7);
    let doc: Document = (&a).into();
    assert!(!doc.contains_key("tracks"));
    let a2: Audio<Uuid7> = doc.try_into().unwrap();
    assert!(a2.tracks_slice().is_empty());
  }

  #[test]
  fn audio_track_roundtrip() {
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_stream_index(Some(0))
      .with_codec(AudioCodec::Aac)
      .with_profile("LC")
      .try_with_sample_rate(48_000)
      .unwrap()
      .try_with_channels(2)
      .unwrap()
      .with_channel_layout(ChannelLayout::Stereo)
      .with_bit_rate(192_000)
      .with_bit_rate_mode(Some(BitRateMode::Cbr))
      .with_bits_per_sample(Some(16))
      .with_lossless(false)
      .with_language(Some(Language::from_bcp47("en").unwrap()))
      .with_detected_language(Some(Language::from_bcp47("en").unwrap()))
      .with_disposition(TrackDisposition::from_u32(0x21))
      .with_primary(true)
      .with_content(Some(AudioContentKind::Music))
      .try_with_speech_ratio(Some(0.42))
      .unwrap()
      .with_silent(false)
      .with_loudness(Some(Loudness::new(-23.0, 7.5, -1.0, -3.0)))
      .with_fingerprint(Some(
        Fingerprint::try_new("chromaprint", vec![1u8, 2, 3, 4]).unwrap(),
      ))
      .with_isrc("ISRC123")
      .with_acoustid("acoust-xyz")
      .with_musicbrainz_recording_id("mb-abc")
      .with_tags(Some(
        Tags::new()
          .with_title("Song")
          .with_artist("X")
          .with_track_number(1)
          .with_year(2024)
          .with_language(Language::from_bcp47("en").unwrap()),
      ))
      .with_cover_art(Some(
        CoverArt::try_new("image/jpeg", vec![0xFFu8, 0xD8, 0xFF]).unwrap(),
      ))
      .with_provenance(Provenance::from_parts("asry", "1.0", "p", "idx"))
      .with_vad_provenance(Provenance::from_parts("silero", "v5", "p", "idx"))
      .with_ced_provenance(Provenance::from_parts("ced-net", "v2", "p", "idx"))
      .try_with_index_status(AudioIndexStatus::EXTRACTED | AudioIndexStatus::VAD_DONE)
      .unwrap()
      .with_index_errors(vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad header")]);
    let doc: Document = (&t).into();
    let t2: AudioTrack<Uuid7> = doc.try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn audio_track_drops_reverse_fk_lists() {
    // `speakers` + `segments` are reverse-FK lists; the document neither
    // writes them nor reads stale copies — the round-tripped track has
    // empty slices regardless of source state.
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_speakers(vec![Uuid7::new(), Uuid7::new()])
      .with_segments(vec![Uuid7::new()]);
    let doc: Document = (&t).into();
    assert!(!doc.contains_key("speakers"));
    assert!(!doc.contains_key("segments"));
    let t2: AudioTrack<Uuid7> = doc.try_into().unwrap();
    assert!(t2.speakers_slice().is_empty());
    assert!(t2.segments_slice().is_empty());
  }

  #[test]
  fn audio_segment_roundtrip() {
    let s = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, sp(0, 1500))
      .unwrap()
      .with_speaker_id(Some(Uuid7::new()))
      .with_text(LocalizedText::from_src_translated("hola", "hello"))
      .with_language(Some(Language::from_bcp47("es").unwrap()))
      .try_with_words(vec![Word::try_from_parts(
        "hola",
        sp(0, 500),
        0.95,
        Some(Language::from_bcp47("es").unwrap()),
      )
      .unwrap()])
      .unwrap()
      .try_with_no_speech_prob(Some(0.05))
      .unwrap()
      .with_avg_logprob(Some(-0.4))
      .with_temperature(Some(0.0));
    let doc: Document = (&s).into();
    // Absent voice_fingerprint serialises as Null.
    assert_eq!(doc.get("voice_fingerprint"), Some(&Bson::Null));
    let s2: AudioSegment<Uuid7> = doc.try_into().unwrap();
    assert_eq!(s, s2);
    assert!(s2.voice_fingerprint_ref().is_none());
  }

  #[test]
  fn audio_segment_roundtrip_with_voice_fingerprint() {
    use crate::domain::vo::VoiceFingerprint;
    let vfp = VoiceFingerprint::try_new(
      Uuid7::new(),
      192,
      jiff::Timestamp::from_millisecond(1_700_000_000_000).unwrap(),
      Some(0.83),
      Provenance::from_parts("ecapa-tdnn", "v1.0.0", "", "findit-indexer-0.1.0"),
    )
    .unwrap();
    let s = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, sp(0, 1500))
      .unwrap()
      .with_voice_fingerprint(Some(vfp));
    let doc: Document = (&s).into();
    // voice_fingerprint is an embedded sub-doc (not flattened).
    assert!(matches!(
      doc.get("voice_fingerprint"),
      Some(Bson::Document(_))
    ));
    let s2: AudioSegment<Uuid7> = doc.try_into().unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn audio_segment_nil_id_rejected() {
    let mut d = Document::new();
    d.insert(
      "_id",
      Bson::Binary(::bson::Binary {
        subtype: ::bson::spec::BinarySubtype::Uuid,
        bytes: vec![0u8; 16],
      }),
    );
    d.insert("audio_track_id", uuid7_to_bson(Uuid7::new()));
    d.insert("index", Bson::Int64(0));
    d.insert("span", time_range_to_bson(&sp(0, 500)));
    let err = AudioSegment::<Uuid7>::try_from(d).unwrap_err();
    assert!(err.is_uuid_7());
  }

  #[test]
  fn sound_event_roundtrip_minimal() {
    let e = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(0, 1500),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    )
    .unwrap();
    let doc: Document = (&e).into();
    // Absent code serialises as Null.
    assert_eq!(doc.get("code"), Some(&Bson::Null));
    let e2: SoundEvent<Uuid7> = doc.try_into().unwrap();
    assert_eq!(e, e2);
  }

  #[test]
  fn sound_event_roundtrip_full() {
    let e = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      7,
      sp(5_000, 10_000),
      "Siren",
      Some(316),
      0.87,
      CedDetector::Manual,
    )
    .unwrap();
    let doc: Document = (&e).into();
    assert_eq!(doc.get("code"), Some(&Bson::Int64(316)));
    assert_eq!(doc.get("detector"), Some(&Bson::Int32(1)));
    let e2: SoundEvent<Uuid7> = doc.try_into().unwrap();
    assert_eq!(e, e2);
  }

  #[test]
  fn sound_event_unknown_detector_rejected() {
    let e = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      sp(0, 100),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    )
    .unwrap();
    let mut doc: Document = (&e).into();
    doc.insert("detector", Bson::Int32(99));
    let err = SoundEvent::<Uuid7>::try_from(doc).unwrap_err();
    assert!(err.is_int_out_of_range());
  }

  #[test]
  fn sound_event_nil_id_rejected() {
    let mut d = Document::new();
    d.insert(
      "_id",
      Bson::Binary(::bson::Binary {
        subtype: ::bson::spec::BinarySubtype::Uuid,
        bytes: vec![0u8; 16],
      }),
    );
    d.insert("audio_track_id", uuid7_to_bson(Uuid7::new()));
    d.insert("index", Bson::Int64(0));
    d.insert("span", time_range_to_bson(&sp(0, 500)));
    d.insert("label", Bson::String("Speech".to_owned()));
    d.insert("code", Bson::Null);
    d.insert("score", Bson::Double(0.5));
    d.insert("detector", Bson::Int32(0));
    let err = SoundEvent::<Uuid7>::try_from(d).unwrap_err();
    assert!(err.is_uuid_7());
  }
}
