//! `Audio` + `AudioTrack` + `AudioSegment` ↔ bson `Document` mapping.
//!
//! Includes the per-recording VOs (`AudioTags`, `AudioCoverArt`,
//! `Loudness`, `AudioFingerprint`) and the per-segment `Word` VO.

use ::bson::{Bson, Document};
use smol_str::SmolStr;

use crate::domain::{
  aggregates::audio::{
    facet::Audio,
    segment::{AudioSegment, Word},
    track::{AudioCoverArt, AudioFingerprint, AudioTags, AudioTrack, Loudness},
  },
  bitflags::AudioIndexStatus,
  enums::AudioContentKind,
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
// Audio facet
// ---------------------------------------------------------------------------

impl From<&Audio<Uuid7>> for Document {
  fn from(a: &Audio<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*a.id()));
    d.insert("tracks", uuid7_vec_to_bson(a.tracks()));
    d.insert("total_segments", Bson::Int64(a.total_segments() as i64));
    d
  }
}

impl TryFrom<Document> for Audio<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let mut a = Audio::try_new(id)?;
    if let Some(b) = take_opt(&mut d, "tracks") {
      a.set_tracks(uuid7_vec_from_bson(b, "tracks")?);
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

fn tags_to_bson(t: &AudioTags) -> Bson {
  let mut d = Document::new();
  d.insert("title", Bson::String(t.title().to_owned()));
  d.insert("artist", Bson::String(t.artist().to_owned()));
  d.insert("album_artist", Bson::String(t.album_artist().to_owned()));
  d.insert("album", Bson::String(t.album().to_owned()));
  d.insert("genre", Bson::String(t.genre().to_owned()));
  d.insert("composer", Bson::String(t.composer().to_owned()));
  d.insert("performer", Bson::String(t.performer().to_owned()));
  d.insert("date", Bson::String(t.date().to_owned()));
  d.insert("track_number", Bson::Int64(t.track_number() as i64));
  d.insert("total_tracks", Bson::Int64(t.total_tracks() as i64));
  d.insert("disc_number", Bson::Int64(t.disc_number() as i64));
  d.insert("total_discs", Bson::Int64(t.total_discs() as i64));
  d.insert("comment", Bson::String(t.comment().to_owned()));
  d.insert("lyrics", Bson::String(t.lyrics().to_owned()));
  d.insert("tag_types", smolstr_vec_to_bson(t.tag_types()));
  Bson::Document(d)
}

fn tags_from_bson(b: Bson, field: &'static str) -> Result<AudioTags, MongoError> {
  let mut d = as_doc(b, field)?;
  Ok(
    AudioTags::new()
      .with_title(as_smol(take(&mut d, "title")?, "title")?)
      .with_artist(as_smol(take(&mut d, "artist")?, "artist")?)
      .with_album_artist(as_smol(take(&mut d, "album_artist")?, "album_artist")?)
      .with_album(as_smol(take(&mut d, "album")?, "album")?)
      .with_genre(as_smol(take(&mut d, "genre")?, "genre")?)
      .with_composer(as_smol(take(&mut d, "composer")?, "composer")?)
      .with_performer(as_smol(take(&mut d, "performer")?, "performer")?)
      .with_date(as_smol(take(&mut d, "date")?, "date")?)
      .with_track_number(as_u32(take(&mut d, "track_number")?, "track_number")?)
      .with_total_tracks(as_u32(take(&mut d, "total_tracks")?, "total_tracks")?)
      .with_disc_number(as_u32(take(&mut d, "disc_number")?, "disc_number")?)
      .with_total_discs(as_u32(take(&mut d, "total_discs")?, "total_discs")?)
      .with_comment(as_smol(take(&mut d, "comment")?, "comment")?)
      .with_lyrics(as_smol(take(&mut d, "lyrics")?, "lyrics")?)
      .with_tag_types(smolstr_vec_from_bson(
        take(&mut d, "tag_types")?,
        "tag_types",
      )?),
  )
}

// ---------------------------------------------------------------------------
// AudioCoverArt
// ---------------------------------------------------------------------------

fn cover_art_to_bson(c: &AudioCoverArt) -> Bson {
  let mut d = Document::new();
  d.insert("data", bytes_to_bson(c.data()));
  d.insert("mime", Bson::String(c.mime().to_owned()));
  Bson::Document(d)
}

fn cover_art_from_bson(b: Bson, field: &'static str) -> Result<AudioCoverArt, MongoError> {
  let mut d = as_doc(b, field)?;
  let data = as_binary(take(&mut d, "data")?, "data")?;
  let mime = as_smol(take(&mut d, "mime")?, "mime")?;
  Ok(AudioCoverArt::from_parts(data, mime))
}

// ---------------------------------------------------------------------------
// Loudness
// ---------------------------------------------------------------------------

fn loudness_to_bson(l: &Loudness) -> Bson {
  let mut d = Document::new();
  d.insert("integrated_lufs", Bson::Double(l.integrated_lufs() as f64));
  d.insert("true_peak_dbtp", Bson::Double(l.true_peak_dbtp() as f64));
  d.insert(
    "loudness_range_lu",
    Bson::Double(l.loudness_range_lu() as f64),
  );
  Bson::Document(d)
}

fn loudness_from_bson(b: Bson, field: &'static str) -> Result<Loudness, MongoError> {
  let mut d = as_doc(b, field)?;
  let i = as_f32(take(&mut d, "integrated_lufs")?, "integrated_lufs")?;
  let p = as_f32(take(&mut d, "true_peak_dbtp")?, "true_peak_dbtp")?;
  let r = as_f32(take(&mut d, "loudness_range_lu")?, "loudness_range_lu")?;
  Ok(Loudness::new(i, p, r))
}

// ---------------------------------------------------------------------------
// AudioFingerprint
// ---------------------------------------------------------------------------

fn fingerprint_to_bson(fp: &AudioFingerprint) -> Bson {
  let mut d = Document::new();
  d.insert("algo", Bson::String(fp.algo().to_owned()));
  d.insert("value", bytes_to_bson(fp.value()));
  Bson::Document(d)
}

fn fingerprint_from_bson(b: Bson, field: &'static str) -> Result<AudioFingerprint, MongoError> {
  let mut d = as_doc(b, field)?;
  let algo = as_smol(take(&mut d, "algo")?, "algo")?;
  let value = as_binary(take(&mut d, "value")?, "value")?;
  Ok(AudioFingerprint::from_parts(algo, value))
}

// ---------------------------------------------------------------------------
// AudioTrack
// ---------------------------------------------------------------------------

impl From<&AudioTrack<Uuid7>> for Document {
  fn from(t: &AudioTrack<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*t.id()));
    d.insert("parent", uuid7_to_bson(*t.parent()));
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
    d.insert("codec", Bson::String(t.codec().to_owned()));
    d.insert("profile", Bson::String(t.profile().to_owned()));
    d.insert("sample_rate", Bson::Int64(t.sample_rate() as i64));
    d.insert("channels", Bson::Int32(t.channels() as i32));
    d.insert(
      "channel_layout",
      Bson::String(t.channel_layout().to_owned()),
    );
    d.insert("bit_rate", Bson::Int64(t.bit_rate() as i64));
    d.insert(
      "bit_rate_mode",
      t.bit_rate_mode()
        .map(|s| Bson::String(s.to_owned()))
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
      t.duration()
        .map(|v| media_ts_to_bson(*v))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "start_pts",
      t.start_pts()
        .map(|v| media_ts_to_bson(*v))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "language",
      t.language()
        .map(|s| Bson::String(s.to_owned()))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "detected_language",
      t.detected_language()
        .map(|s| Bson::String(s.to_owned()))
        .unwrap_or(Bson::Null),
    );
    d.insert("language_mismatch", Bson::Boolean(t.language_mismatch()));
    d.insert("disposition", Bson::Int64(t.disposition() as i64));
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
      t.loudness().map(loudness_to_bson).unwrap_or(Bson::Null),
    );
    d.insert(
      "fingerprint",
      t.fingerprint()
        .map(fingerprint_to_bson)
        .unwrap_or(Bson::Null),
    );
    d.insert("isrc", Bson::String(t.isrc().to_owned()));
    d.insert("acoustid", Bson::String(t.acoustid().to_owned()));
    d.insert(
      "musicbrainz_recording_id",
      Bson::String(t.musicbrainz_recording_id().to_owned()),
    );
    d.insert("speakers", uuid7_vec_to_bson(t.speakers()));
    d.insert("tags", t.tags().map(tags_to_bson).unwrap_or(Bson::Null));
    d.insert(
      "cover_art",
      t.cover_art().map(cover_art_to_bson).unwrap_or(Bson::Null),
    );
    d.insert("segments", uuid7_vec_to_bson(t.segments()));
    d.insert("provenance", provenance_to_bson(t.provenance()));
    d.insert("index_status", Bson::Int64(t.index_status().bits() as i64));
    d.insert("index_errors", error_info_vec_to_bson(t.index_errors()));
    d
  }
}

impl TryFrom<Document> for AudioTrack<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let parent = uuid7_from_bson(take(&mut d, "parent")?, "parent")?;
    let mut t = AudioTrack::try_new(id, parent)?;

    if let Some(b) = take_opt(&mut d, "stream_index") {
      t.set_stream_index(Some(as_u32(b, "stream_index")?));
    }
    if let Some(b) = take_opt(&mut d, "container_track_id") {
      t.set_container_track_id(Some(as_u64(b, "container_track_id")?));
    }
    if let Some(b) = take_opt(&mut d, "codec") {
      t.set_codec(as_smol(b, "codec")?);
    }
    if let Some(b) = take_opt(&mut d, "profile") {
      t.set_profile(as_smol(b, "profile")?);
    }
    if let Some(b) = take_opt(&mut d, "sample_rate") {
      t.set_sample_rate(as_u32(b, "sample_rate")?);
    }
    if let Some(b) = take_opt(&mut d, "channels") {
      t.set_channels(as_u16(b, "channels")?);
    }
    if let Some(b) = take_opt(&mut d, "channel_layout") {
      t.set_channel_layout(as_smol(b, "channel_layout")?);
    }
    if let Some(b) = take_opt(&mut d, "bit_rate") {
      t.set_bit_rate(as_u64(b, "bit_rate")?);
    }
    if let Some(b) = take_opt(&mut d, "bit_rate_mode") {
      t.set_bit_rate_mode(Some(as_smol(b, "bit_rate_mode")?));
    }
    if let Some(b) = take_opt(&mut d, "bits_per_sample") {
      t.set_bits_per_sample(Some(as_u16(b, "bits_per_sample")?));
    }
    if let Some(b) = take_opt(&mut d, "is_lossless") {
      t.set_lossless(as_bool(b, "is_lossless")?);
    }
    if let Some(b) = take_opt(&mut d, "duration") {
      t.set_duration(Some(media_ts_from_bson(b, "duration")?));
    }
    if let Some(b) = take_opt(&mut d, "start_pts") {
      t.set_start_pts(Some(media_ts_from_bson(b, "start_pts")?));
    }
    if let Some(b) = take_opt(&mut d, "language") {
      t.set_language(Some(as_smol(b, "language")?));
    }
    if let Some(b) = take_opt(&mut d, "detected_language") {
      t.set_detected_language(Some(as_smol(b, "detected_language")?));
    }
    if let Some(b) = take_opt(&mut d, "language_mismatch") {
      t.set_language_mismatch(as_bool(b, "language_mismatch")?);
    }
    if let Some(b) = take_opt(&mut d, "disposition") {
      t.set_disposition(as_u32(b, "disposition")?);
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
      t.set_speech_ratio(Some(as_f32(b, "speech_ratio")?));
    }
    if let Some(b) = take_opt(&mut d, "is_silent") {
      t.set_silent(as_bool(b, "is_silent")?);
    }
    if let Some(b) = take_opt(&mut d, "loudness") {
      t.set_loudness(Some(loudness_from_bson(b, "loudness")?));
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
    if let Some(b) = take_opt(&mut d, "speakers") {
      t.set_speakers(uuid7_vec_from_bson(b, "speakers")?);
    }
    if let Some(b) = take_opt(&mut d, "tags") {
      t.set_tags(Some(tags_from_bson(b, "tags")?));
    }
    if let Some(b) = take_opt(&mut d, "cover_art") {
      t.set_cover_art(Some(cover_art_from_bson(b, "cover_art")?));
    }
    if let Some(b) = take_opt(&mut d, "segments") {
      t.set_segments(uuid7_vec_from_bson(b, "segments")?);
    }
    if let Some(b) = take_opt(&mut d, "provenance") {
      t.set_provenance(provenance_from_bson(b, "provenance")?);
    }
    if let Some(b) = take_opt(&mut d, "index_status") {
      let bits = as_u64(b, "index_status")?;
      let bits32 = u32::try_from(bits).map_err(|_| MongoError::IntOutOfRange {
        field: SmolStr::from("index_status"),
        value: bits as i64,
      })?;
      t.set_index_status(AudioIndexStatus::from_bits_truncate(bits32));
    }
    if let Some(b) = take_opt(&mut d, "index_errors") {
      t.set_index_errors(error_info_vec_from_bson(b, "index_errors")?);
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
  d.insert("span", time_range_to_bson(w.span()));
  d.insert("score", Bson::Double(w.score() as f64));
  d.insert(
    "language",
    w.language()
      .map(|s| Bson::String(s.to_owned()))
      .unwrap_or(Bson::Null),
  );
  Bson::Document(d)
}

fn word_from_bson(b: Bson, field: &'static str) -> Result<Word, MongoError> {
  let mut d = as_doc(b, field)?;
  let text = as_smol(take(&mut d, "text")?, "text")?;
  let span = time_range_from_bson(take(&mut d, "span")?, "span")?;
  let score = as_f32(take(&mut d, "score")?, "score")?;
  let language = opt(take_opt(&mut d, "language"), |bb| as_smol(bb, "language"))?;
  Ok(Word::from_parts(text, span, score, language))
}

// ---------------------------------------------------------------------------
// AudioSegment
// ---------------------------------------------------------------------------

impl From<&AudioSegment<Uuid7>> for Document {
  fn from(s: &AudioSegment<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*s.id()));
    d.insert("parent", uuid7_to_bson(*s.parent()));
    d.insert("index", Bson::Int64(s.index() as i64));
    d.insert("span", time_range_to_bson(s.span()));
    d.insert(
      "speaker",
      s.speaker().map(|i| uuid7_to_bson(*i)).unwrap_or(Bson::Null),
    );
    d.insert("text", loc_text_to_bson(s.text()));
    d.insert(
      "language",
      s.language()
        .map(|s| Bson::String(s.to_owned()))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "words",
      Bson::Array(s.words().iter().map(word_to_bson).collect()),
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
    d
  }
}

impl TryFrom<Document> for AudioSegment<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let parent = uuid7_from_bson(take(&mut d, "parent")?, "parent")?;
    let index = as_u32(take(&mut d, "index")?, "index")?;
    let span = time_range_from_bson(take(&mut d, "span")?, "span")?;
    let mut s = AudioSegment::try_new(id, parent, index, span)?;

    if let Some(b) = take_opt(&mut d, "speaker") {
      s.set_speaker(Some(uuid7_from_bson(b, "speaker")?));
    }
    if let Some(b) = take_opt(&mut d, "text") {
      s.set_text(loc_text_from_bson(b, "text")?);
    }
    if let Some(b) = take_opt(&mut d, "language") {
      s.set_language(Some(as_smol(b, "language")?));
    }
    if let Some(b) = take_opt(&mut d, "words") {
      let arr = as_array(b, "words")?;
      let mut vw = Vec::with_capacity(arr.len());
      for w in arr {
        vw.push(word_from_bson(w, "words[]")?);
      }
      s.set_words(vw);
    }
    if let Some(b) = take_opt(&mut d, "no_speech_prob") {
      s.set_no_speech_prob(Some(as_f32(b, "no_speech_prob")?));
    }
    if let Some(b) = take_opt(&mut d, "avg_logprob") {
      s.set_avg_logprob(Some(as_f32(b, "avg_logprob")?));
    }
    if let Some(b) = take_opt(&mut d, "temperature") {
      s.set_temperature(Some(as_f32(b, "temperature")?));
    }
    Ok(s)
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
    let a = Audio::try_new(Uuid7::new())
      .unwrap()
      .with_tracks(vec![Uuid7::new(), Uuid7::new()])
      .with_total_segments(7);
    let doc: Document = (&a).into();
    let a2: Audio<Uuid7> = doc.try_into().unwrap();
    assert_eq!(a, a2);
  }

  #[test]
  fn audio_track_roundtrip() {
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_stream_index(Some(0))
      .with_codec("aac")
      .with_profile("LC")
      .with_sample_rate(48_000)
      .with_channels(2)
      .with_channel_layout("stereo")
      .with_bit_rate(192_000)
      .with_bit_rate_mode(Some(SmolStr::from("CBR")))
      .with_bits_per_sample(Some(16))
      .with_lossless(false)
      .with_language(Some(SmolStr::from("en")))
      .with_detected_language(Some(SmolStr::from("en")))
      .with_disposition(0x21)
      .with_primary(true)
      .with_content(Some(AudioContentKind::Music))
      .with_speech_ratio(Some(0.42))
      .with_silent(false)
      .with_loudness(Some(Loudness::new(-23.0, -1.0, 7.5)))
      .with_fingerprint(Some(AudioFingerprint::from_parts(
        "chromaprint",
        vec![1u8, 2, 3, 4],
      )))
      .with_isrc("ISRC123")
      .with_acoustid("acoust-xyz")
      .with_musicbrainz_recording_id("mb-abc")
      .with_speakers(vec![Uuid7::new()])
      .with_tags(Some(
        AudioTags::new()
          .with_title("Song")
          .with_artist("X")
          .with_track_number(1)
          .with_tag_types(vec![SmolStr::from("ID3v2")]),
      ))
      .with_cover_art(Some(AudioCoverArt::from_parts(
        vec![0xFFu8, 0xD8, 0xFF],
        "image/jpeg",
      )))
      .with_segments(vec![Uuid7::new()])
      .with_provenance(Provenance::from_parts("asry", "1.0", "p", "idx"))
      .with_index_status(AudioIndexStatus::EXTRACTED | AudioIndexStatus::VAD_DONE)
      .with_index_errors(vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad header")]);
    let doc: Document = (&t).into();
    let t2: AudioTrack<Uuid7> = doc.try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn audio_segment_roundtrip() {
    let s = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, sp(0, 1500))
      .unwrap()
      .with_speaker(Some(Uuid7::new()))
      .with_text(LocalizedText::from_src_translated("hola", "hello"))
      .with_language(Some(SmolStr::from("es")))
      .with_words(vec![Word::from_parts(
        "hola",
        sp(0, 500),
        0.95,
        Some(SmolStr::from("es")),
      )])
      .with_no_speech_prob(Some(0.05))
      .with_avg_logprob(Some(-0.4))
      .with_temperature(Some(0.0));
    let doc: Document = (&s).into();
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
    d.insert("parent", uuid7_to_bson(Uuid7::new()));
    d.insert("index", Bson::Int64(0));
    d.insert("span", time_range_to_bson(&sp(0, 500)));
    let err = AudioSegment::<Uuid7>::try_from(d).unwrap_err();
    assert!(err.is_uuid_7());
  }
}
