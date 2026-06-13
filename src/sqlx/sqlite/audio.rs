//! SQLite row shapes for the audio-cluster aggregates: the `Audio` facet,
//! `AudioTrack`, and `AudioSegment` (+ the `Word` child table).
//!
//! UUIDs ride as 16-byte `BLOB`s. Integer-affinity columns (including
//! booleans, stored as `0`/`1`) ride as `i64`. Nested value-objects are
//! flattened into real columns; `Option<VO>` rides as a discriminating
//! column plus all-NULL payload columns when absent. Open descriptor
//! enums (`AudioCodec`, `ChannelLayout`) ride as `TEXT` slugs; coded
//! enums (`BitRateMode`) and bitflags (`AudioIndexStatus`) ride as
//! integers. `Language` flattens to a BCP-47 `TEXT` column. Media-time
//! values flatten to a PTS `INTEGER` + timebase num/den.
//!
//! Collections ride in child tables (`audio_segment_word`,
//! `audio_track_index_error`) with an `ordinal` order column. The
//! `Vec<Id>` reverse-FK fields are NOT stored.

use std::vec::Vec;

use indexmap::IndexMap;
use mediaframe::{
  audio::{
    BitRateMode, ChannelLayout, CoverArt, Fingerprint, Loudness, ReplayGain, SampleFormat, Tags,
  },
  codec::AudioCodec,
  disposition::TrackDisposition,
  lang::Language,
};
use smol_str::SmolStr;

use crate::{
  domain::{
    aggregates::audio::{
      segment::WordError, sound_event::SoundEventError, AudioError, AudioSegmentError,
      AudioTrackError, Word,
    },
    vo::{IndexProgress, Provenance, VoiceFingerprint},
    Audio, AudioContentKind, AudioIndexStatus, AudioSegment, AudioTrack, CedDetector, ErrorCode,
    ErrorInfo, SoundEvent, Uuid7,
  },
  sqlx::{
    dto::{bytes_to_uuid7, millis_to_timestamp, timestamp_from_parts, timestamp_to_millis},
    SqlxError,
  },
};

// ---------------------------------------------------------------------------
// AudioContentKind — closed enum, rides as a small integer
// ---------------------------------------------------------------------------

fn content_kind_to_i64(k: AudioContentKind) -> i64 {
  match k {
    AudioContentKind::Speech => 0,
    AudioContentKind::Music => 1,
    AudioContentKind::Mixed => 2,
    AudioContentKind::Silence => 3,
  }
}

fn content_kind_from_i64(n: i64) -> Result<AudioContentKind, SqlxError> {
  match n {
    0 => Ok(AudioContentKind::Speech),
    1 => Ok(AudioContentKind::Music),
    2 => Ok(AudioContentKind::Mixed),
    3 => Ok(AudioContentKind::Silence),
    other => Err(SqlxError::UnknownDiscriminant(format!(
      "AudioContentKind: {other}"
    ))),
  }
}

// ===========================================================================
// Audio facet
// ===========================================================================

/// SQLite row shape for the [`Audio`] facet.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteAudioRow {
  pub id: Vec<u8>,
  pub media_id: Vec<u8>,
  pub total_segments: i64,
  pub track_progress_total: i64,
  pub track_progress_indexed: i64,
  pub track_progress_failed: i64,
}

impl From<&Audio<Uuid7>> for SqliteAudioRow {
  fn from(a: &Audio<Uuid7>) -> Self {
    let p = a.track_progress_ref();
    Self {
      id: a.id_ref().as_bytes().to_vec(),
      media_id: a.media_id_ref().as_bytes().to_vec(),
      total_segments: i64::from(a.total_segments()),
      track_progress_total: i64::from(p.total()),
      track_progress_indexed: i64::from(p.indexed()),
      track_progress_failed: i64::from(p.failed()),
    }
  }
}

impl TryFrom<SqliteAudioRow> for Audio<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteAudioRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let media_id = bytes_to_uuid7(&r.media_id)?;
    let total_segments = u32_from_i64(r.total_segments, "Audio.total_segments")?;
    let progress = IndexProgress::from_parts(
      u32_from_i64(r.track_progress_total, "Audio.track_progress_total")?,
      u32_from_i64(r.track_progress_indexed, "Audio.track_progress_indexed")?,
      u32_from_i64(r.track_progress_failed, "Audio.track_progress_failed")?,
    );
    let a = Audio::try_new(id, media_id)
      .map_err(|e: AudioError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    Ok(restore_rollups(a, total_segments, progress))
  }
}

/// Restore the stored `total_segments` + `track_progress` rollups onto a
/// row-reconstructed `Audio` facet — see the PostgreSQL audio module for
/// the rationale (`tracks` is the reverse-FK on `audio_track.audio_id`,
/// not stored on `audio`; the application re-derives it from the child query).
fn restore_rollups(a: Audio<Uuid7>, total_segments: u32, progress: IndexProgress) -> Audio<Uuid7> {
  a.with_total_segments(total_segments)
    .with_track_progress(progress)
}

// ===========================================================================
// AudioTrack
// ===========================================================================

/// SQLite row shape for [`AudioTrack`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteAudioTrackRow {
  pub id: Vec<u8>,
  pub audio_id: Vec<u8>,
  pub stream_index: Option<i64>,
  pub container_track_id: Option<i64>,
  pub codec: String,
  pub profile: String,
  pub sample_rate: i64,
  pub channels: i64,
  pub channel_layout: String,
  /// `SampleFormat::to_u32` (FFmpeg `AV_SAMPLE_FMT_*`).
  pub sample_format: i64,
  pub bit_rate: i64,
  pub bit_rate_mode: Option<i64>,
  pub bits_per_sample: Option<i64>,
  pub is_lossless: i64,
  pub duration_pts: Option<i64>,
  pub duration_tb_num: Option<i64>,
  pub duration_tb_den: Option<i64>,
  pub start_pts: Option<i64>,
  pub start_pts_tb_num: Option<i64>,
  pub start_pts_tb_den: Option<i64>,
  pub language: Option<String>,
  pub detected_language: Option<String>,
  pub disposition: i64,
  pub is_primary: i64,
  pub auto_selected: i64,
  pub content: Option<i64>,
  pub speech_ratio: Option<f32>,
  pub is_silent: i64,
  pub has_loudness: i64,
  pub loudness_integrated_lufs: Option<f32>,
  pub loudness_range_lu: Option<f32>,
  pub loudness_true_peak_dbtp: Option<f32>,
  pub loudness_sample_peak_dbfs: Option<f32>,
  pub has_replay_gain: i64,
  pub replay_gain_track_gain_db: Option<f32>,
  pub replay_gain_track_peak: Option<f32>,
  pub replay_gain_album_gain_db: Option<f32>,
  pub replay_gain_album_peak: Option<f32>,
  pub fingerprint_algo: Option<String>,
  pub fingerprint_value: Option<Vec<u8>>,
  pub isrc: String,
  pub acoustid: String,
  pub musicbrainz_recording_id: String,
  pub has_tags: i64,
  pub tags_title: Option<String>,
  pub tags_artist: Option<String>,
  pub tags_album_artist: Option<String>,
  pub tags_album: Option<String>,
  pub tags_composer: Option<String>,
  pub tags_genre: Option<String>,
  pub tags_comment: Option<String>,
  pub tags_year: Option<i64>,
  pub tags_track_number: Option<i64>,
  pub tags_track_total: Option<i64>,
  pub tags_disc_number: Option<i64>,
  pub tags_disc_total: Option<i64>,
  pub tags_language: Option<String>,
  pub cover_art_mime: Option<String>,
  pub cover_art_data: Option<Vec<u8>>,
  pub provenance_model_name: String,
  pub provenance_model_version: String,
  pub provenance_prompt_version: String,
  pub provenance_indexer_version: String,
  pub index_status: i64,
}

/// One `audio_track_index_error` child row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteAudioTrackIndexErrorRow {
  pub audio_track_id: Vec<u8>,
  pub ordinal: i64,
  pub code: i64,
  pub message: String,
}

/// SQLite row for `audio_track_metadata`. Position in the per-
/// `audio_track_id` `ordinal` sequence IS the [`IndexMap`] insertion
/// order. `audio_track_from_rows` sorts by `ordinal` on decode.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteAudioTrackMetadataRow {
  pub audio_track_id: Vec<u8>,
  pub ordinal: i64,
  pub key: String,
  pub value: String,
}

impl From<&AudioTrack<Uuid7>>
  for (
    SqliteAudioTrackRow,
    Vec<SqliteAudioTrackIndexErrorRow>,
    Vec<SqliteAudioTrackMetadataRow>,
  )
{
  fn from(t: &AudioTrack<Uuid7>) -> Self {
    let id = t.id_ref().as_bytes().to_vec();
    let loudness = t.loudness_ref();
    let replay_gain = t.replay_gain_ref();
    let fingerprint = t.fingerprint_ref();
    let cover = t.cover_art_ref();
    let tags = t.tags_ref();
    let prov = t.provenance_ref();
    let duration = t.duration_ref();
    let start_pts = t.start_pts_ref();
    let row = SqliteAudioTrackRow {
      id: id.clone(),
      audio_id: t.audio_id_ref().as_bytes().to_vec(),
      stream_index: t.stream_index().map(i64::from),
      container_track_id: t.container_track_id().map(|v| v as i64),
      codec: t.codec_ref().as_str().to_owned(),
      profile: t.profile().to_owned(),
      sample_rate: i64::from(t.sample_rate()),
      channels: i64::from(t.channels()),
      channel_layout: t.channel_layout_ref().as_str().to_owned(),
      sample_format: i64::from(t.sample_format_ref().to_u32()),
      bit_rate: t.bit_rate() as i64,
      bit_rate_mode: t.bit_rate_mode().map(|m| i64::from(m.to_u32())),
      bits_per_sample: t.bits_per_sample().map(i64::from),
      is_lossless: i64::from(t.is_lossless()),
      duration_pts: duration.map(mediatime::Timestamp::pts),
      duration_tb_num: duration.map(|d| i64::from(d.timebase().num())),
      duration_tb_den: duration.map(|d| i64::from(d.timebase().den().get())),
      start_pts: start_pts.map(mediatime::Timestamp::pts),
      start_pts_tb_num: start_pts.map(|d| i64::from(d.timebase().num())),
      start_pts_tb_den: start_pts.map(|d| i64::from(d.timebase().den().get())),
      language: t.language().map(|l| l.to_bcp47()),
      detected_language: t.detected_language().map(|l| l.to_bcp47()),
      disposition: i64::from(t.disposition().bits()),
      is_primary: i64::from(t.is_primary()),
      auto_selected: i64::from(t.auto_selected()),
      content: t.content().map(content_kind_to_i64),
      speech_ratio: t.speech_ratio(),
      is_silent: i64::from(t.is_silent()),
      has_loudness: i64::from(loudness.is_some()),
      loudness_integrated_lufs: loudness.map(Loudness::integrated_lufs),
      loudness_range_lu: loudness.map(Loudness::range_lu),
      loudness_true_peak_dbtp: loudness.map(Loudness::true_peak_dbtp),
      loudness_sample_peak_dbfs: loudness.map(Loudness::sample_peak_dbfs),
      has_replay_gain: i64::from(replay_gain.is_some()),
      replay_gain_track_gain_db: replay_gain.map(ReplayGain::track_gain_db),
      replay_gain_track_peak: replay_gain.map(ReplayGain::track_peak),
      replay_gain_album_gain_db: replay_gain.and_then(ReplayGain::album_gain_db),
      replay_gain_album_peak: replay_gain.and_then(ReplayGain::album_peak),
      fingerprint_algo: fingerprint.map(|f| f.algorithm().to_owned()),
      fingerprint_value: fingerprint.map(|f| f.value().to_vec()),
      isrc: t.isrc().to_owned(),
      acoustid: t.acoustid().to_owned(),
      musicbrainz_recording_id: t.musicbrainz_recording_id().to_owned(),
      has_tags: i64::from(tags.is_some()),
      tags_title: tags.map(|x| x.title().to_owned()),
      tags_artist: tags.map(|x| x.artist().to_owned()),
      tags_album_artist: tags.map(|x| x.album_artist().to_owned()),
      tags_album: tags.map(|x| x.album().to_owned()),
      tags_composer: tags.map(|x| x.composer().to_owned()),
      tags_genre: tags.map(|x| x.genre().to_owned()),
      tags_comment: tags.map(|x| x.comment().to_owned()),
      tags_year: tags.map(|x| i64::from(x.year())),
      tags_track_number: tags.map(|x| i64::from(x.track_number())),
      tags_track_total: tags.map(|x| i64::from(x.track_total())),
      tags_disc_number: tags.map(|x| i64::from(x.disc_number())),
      tags_disc_total: tags.map(|x| i64::from(x.disc_total())),
      tags_language: tags.and_then(|x| x.language()).map(|l| l.to_bcp47()),
      cover_art_mime: cover.map(|c| c.mime().to_owned()),
      cover_art_data: cover.map(|c| c.data().to_vec()),
      provenance_model_name: prov.model_name().to_owned(),
      provenance_model_version: prov.model_version().to_owned(),
      provenance_prompt_version: prov.prompt_version().to_owned(),
      provenance_indexer_version: prov.indexer_version().to_owned(),
      index_status: i64::from(t.index_status().bits()),
    };
    let errors = t
      .index_errors_slice()
      .iter()
      .enumerate()
      .map(|(i, e)| SqliteAudioTrackIndexErrorRow {
        audio_track_id: id.clone(),
        ordinal: i as i64,
        code: i64::from(e.code().as_u32()),
        message: e.message().to_owned(),
      })
      .collect();
    let metadata = t
      .metadata_ref()
      .iter()
      .enumerate()
      .map(|(i, (k, v))| SqliteAudioTrackMetadataRow {
        audio_track_id: id.clone(),
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
    SqliteAudioTrackRow,
    Vec<SqliteAudioTrackIndexErrorRow>,
    Vec<SqliteAudioTrackMetadataRow>,
  )> for AudioTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, errors, metadata): (
      SqliteAudioTrackRow,
      Vec<SqliteAudioTrackIndexErrorRow>,
      Vec<SqliteAudioTrackMetadataRow>,
    ),
  ) -> Result<Self, Self::Error> {
    audio_track_from_rows(r, errors, metadata)
  }
}

/// Reconstruct an [`AudioTrack`] from its row, `index_errors` rows, and
/// `metadata` rows.
pub fn audio_track_from_rows(
  r: SqliteAudioTrackRow,
  mut errors: Vec<SqliteAudioTrackIndexErrorRow>,
  mut metadata: Vec<SqliteAudioTrackMetadataRow>,
) -> Result<AudioTrack<Uuid7>, SqlxError> {
  {
    let id = bytes_to_uuid7(&r.id)?;
    let audio_id = bytes_to_uuid7(&r.audio_id)?;
    let mut t = AudioTrack::try_new(id, audio_id)
      .map_err(|e: AudioTrackError| SqlxError::DomainConstructorRejected(e.to_string()))?;

    t = t
      .with_codec(parse_audio_codec(&r.codec))
      .with_profile(r.profile)
      .with_channel_layout(parse_channel_layout(&r.channel_layout))
      .with_sample_format(SampleFormat::from_u32(u32_from_i64(
        r.sample_format,
        "AudioTrack.sample_format",
      )?))
      .with_bit_rate(r.bit_rate as u64)
      .with_lossless(r.is_lossless != 0)
      .with_primary(r.is_primary != 0)
      .with_auto_selected(r.auto_selected != 0)
      .with_silent(r.is_silent != 0)
      .with_stream_index(opt_u32(r.stream_index, "AudioTrack.stream_index")?)
      .with_container_track_id(r.container_track_id.map(|v| v as u64))
      .with_bits_per_sample(opt_u16(r.bits_per_sample, "AudioTrack.bits_per_sample")?)
      .with_disposition(TrackDisposition::from_bits_truncate(u32_from_i64(
        r.disposition,
        "AudioTrack.disposition",
      )?))
      .with_isrc(r.isrc)
      .with_acoustid(r.acoustid)
      .with_musicbrainz_recording_id(r.musicbrainz_recording_id);

    t = t
      .try_with_sample_rate(u32_from_i64(r.sample_rate, "AudioTrack.sample_rate")?)
      .map_err(track_err)?
      .try_with_channels(u16_from_i64(r.channels, "AudioTrack.channels")?)
      .map_err(track_err)?;

    if let Some(m) = r.bit_rate_mode {
      let raw = u32_from_i64(m, "AudioTrack.bit_rate_mode")?;
      let mode = BitRateMode::try_from_u32(raw)
        .ok_or_else(|| SqlxError::UnknownDiscriminant(format!("BitRateMode: {raw}")))?;
      t = t.with_bit_rate_mode(Some(mode));
    }

    if let Some(pts) = r.duration_pts {
      let (num, den) =
        require_timebase(r.duration_tb_num, r.duration_tb_den, "AudioTrack.duration")?;
      t = t
        .try_with_duration(Some(timestamp_from_parts(pts, num, den)?))
        .map_err(track_err)?;
    }
    if let Some(pts) = r.start_pts {
      let (num, den) = require_timebase(
        r.start_pts_tb_num,
        r.start_pts_tb_den,
        "AudioTrack.start_pts",
      )?;
      t = t.with_start_pts(Some(timestamp_from_parts(pts, num, den)?));
    }

    if let Some(s) = r.language {
      t = t.with_language(Some(parse_language(&s)?));
    }
    if let Some(s) = r.detected_language {
      t = t.with_detected_language(Some(parse_language(&s)?));
    }
    if let Some(c) = r.content {
      t = t.with_content(Some(content_kind_from_i64(c)?));
    }
    if let Some(v) = r.speech_ratio {
      t = t.try_with_speech_ratio(Some(v)).map_err(track_err)?;
    }

    if r.has_loudness != 0 {
      t = t.with_loudness(Some(Loudness::new(
        r.loudness_integrated_lufs.unwrap_or_default(),
        r.loudness_range_lu.unwrap_or_default(),
        r.loudness_true_peak_dbtp.unwrap_or_default(),
        r.loudness_sample_peak_dbfs.unwrap_or_default(),
      )));
    }
    if r.has_replay_gain != 0 {
      t = t.with_replay_gain(Some(ReplayGain::new(
        r.replay_gain_track_gain_db.unwrap_or_default(),
        r.replay_gain_track_peak.unwrap_or_default(),
        r.replay_gain_album_gain_db,
        r.replay_gain_album_peak,
      )));
    }
    if let Some(algo) = r.fingerprint_algo {
      let value = r.fingerprint_value.unwrap_or_default();
      t = t.with_fingerprint(Some(Fingerprint::try_new(algo, value).map_err(|e| {
        SqlxError::DomainConstructorRejected(format!("AudioFingerprint: {e}"))
      })?));
    }
    if let Some(mime) = r.cover_art_mime {
      let data = r.cover_art_data.unwrap_or_default();
      t = t.with_cover_art(Some(CoverArt::try_new(mime, data).map_err(|e| {
        SqlxError::DomainConstructorRejected(format!("AudioCoverArt: {e}"))
      })?));
    }
    if r.has_tags != 0 {
      let mut tags = Tags::new()
        .with_title(r.tags_title.unwrap_or_default())
        .with_artist(r.tags_artist.unwrap_or_default())
        .with_album_artist(r.tags_album_artist.unwrap_or_default())
        .with_album(r.tags_album.unwrap_or_default())
        .with_composer(r.tags_composer.unwrap_or_default())
        .with_genre(r.tags_genre.unwrap_or_default())
        .with_comment(r.tags_comment.unwrap_or_default())
        .with_year(u16_from_i64_opt(r.tags_year, "Tags.year")?)
        .with_track_number(u16_from_i64_opt(r.tags_track_number, "Tags.track_number")?)
        .with_track_total(u16_from_i64_opt(r.tags_track_total, "Tags.track_total")?)
        .with_disc_number(u16_from_i64_opt(r.tags_disc_number, "Tags.disc_number")?)
        .with_disc_total(u16_from_i64_opt(r.tags_disc_total, "Tags.disc_total")?);
      if let Some(s) = r.tags_language {
        tags = tags.with_language(parse_language(&s)?);
      }
      t = t.with_tags(Some(tags));
    }

    t = t.with_provenance(crate::domain::vo::Provenance::from_parts(
      r.provenance_model_name,
      r.provenance_model_version,
      r.provenance_prompt_version,
      r.provenance_indexer_version,
    ));

    let status = AudioIndexStatus::from_bits_truncate(u32_from_i64(
      r.index_status,
      "AudioTrack.index_status",
    )?);
    t = t.try_with_index_status(status).map_err(track_err)?;

    errors.sort_by_key(|e| e.ordinal);
    let mut infos = Vec::with_capacity(errors.len());
    for e in errors {
      let code = u32_from_i64(e.code, "AudioTrack.index_error.code")?;
      infos.push(ErrorInfo::new(ErrorCode::from_u32(code), e.message));
    }
    t = t.with_index_errors(infos);

    metadata.sort_by_key(|m| m.ordinal);
    let mut bag = IndexMap::with_capacity(metadata.len());
    for entry in metadata {
      if entry.audio_track_id != r.id {
        return Err(SqlxError::DomainConstructorRejected(
          "audio_track_metadata.audio_track_id does not match parent audio_track.id".to_owned(),
        ));
      }
      bag.insert(SmolStr::from(entry.key), SmolStr::from(entry.value));
    }
    t = t.with_metadata(bag);

    Ok(t)
  }
}

// ===========================================================================
// AudioSegment + Word child table
// ===========================================================================

/// SQLite row shape for [`AudioSegment`].
///
/// `span` flattens to `start_pts` / `end_pts` PTS ticks only; the timebase
/// lives on the parent `audio_track` (a single stream has one timebase for
/// all of its segments + words).
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteAudioSegmentRow {
  pub id: Vec<u8>,
  pub audio_track_id: Vec<u8>,
  pub index: i64,
  pub span_start_pts: i64,
  pub span_end_pts: i64,
  pub speaker_id: Option<Vec<u8>>,
  pub text_src: String,
  pub text_translated: String,
  pub language: Option<String>,
  pub no_speech_prob: Option<f32>,
  pub avg_logprob: Option<f32>,
  pub temperature: Option<f32>,
  /// Per-segment voice embedding — discriminator for the flattened
  /// `VoiceFingerprint` VO (`Some` = present; `None` = all NULL).
  pub voice_fingerprint_vector_id: Option<Vec<u8>>,
  pub voice_fingerprint_dimensions: Option<i64>,
  pub voice_fingerprint_extracted_at_ms: Option<i64>,
  pub voice_fingerprint_confidence: Option<f32>,
  pub voice_fingerprint_provenance_model_name: Option<String>,
  pub voice_fingerprint_provenance_model_version: Option<String>,
  pub voice_fingerprint_provenance_prompt_version: Option<String>,
  pub voice_fingerprint_provenance_indexer_version: Option<String>,
}

/// One `audio_segment_word` child row. Like its parent row, the timebase
/// is inherited from `audio_track` and is not stored per word.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteAudioSegmentWordRow {
  pub audio_segment_id: Vec<u8>,
  pub ordinal: i64,
  pub text: String,
  pub span_start_pts: i64,
  pub span_end_pts: i64,
  pub score: f32,
  pub language: Option<String>,
}

impl From<&AudioSegment<Uuid7>> for (SqliteAudioSegmentRow, Vec<SqliteAudioSegmentWordRow>) {
  fn from(s: &AudioSegment<Uuid7>) -> Self {
    let id = s.id_ref().as_bytes().to_vec();
    let span = s.span_ref();
    let vfp = s.voice_fingerprint_ref();
    let prov = vfp.map(|v| v.provenance_ref());
    let row = SqliteAudioSegmentRow {
      id: id.clone(),
      audio_track_id: s.audio_track_id_ref().as_bytes().to_vec(),
      index: i64::from(s.index()),
      span_start_pts: span.start_pts(),
      span_end_pts: span.end_pts(),
      speaker_id: s.speaker_id_ref().map(|id| id.as_bytes().to_vec()),
      text_src: s.text_ref().src().to_owned(),
      text_translated: s.text_ref().translated().to_owned(),
      language: s.language().map(|l| l.to_bcp47()),
      no_speech_prob: s.no_speech_prob(),
      avg_logprob: s.avg_logprob(),
      temperature: s.temperature(),
      voice_fingerprint_vector_id: vfp.map(|v| v.vector_id_ref().as_bytes().to_vec()),
      voice_fingerprint_dimensions: vfp.map(|v| i64::from(v.dimensions())),
      voice_fingerprint_extracted_at_ms: vfp.map(|v| timestamp_to_millis(v.extracted_at())),
      voice_fingerprint_confidence: vfp.and_then(|v| v.confidence()),
      voice_fingerprint_provenance_model_name: prov.map(|p| p.model_name().to_owned()),
      voice_fingerprint_provenance_model_version: prov.map(|p| p.model_version().to_owned()),
      voice_fingerprint_provenance_prompt_version: prov.map(|p| p.prompt_version().to_owned()),
      voice_fingerprint_provenance_indexer_version: prov.map(|p| p.indexer_version().to_owned()),
    };
    let words = s
      .words_slice()
      .iter()
      .enumerate()
      .map(|(i, w)| {
        let wspan = w.span_ref();
        SqliteAudioSegmentWordRow {
          audio_segment_id: id.clone(),
          ordinal: i as i64,
          text: w.text().to_owned(),
          span_start_pts: wspan.start_pts(),
          span_end_pts: wspan.end_pts(),
          score: w.score(),
          language: w.language().map(|l| l.to_bcp47()),
        }
      })
      .collect();
    (row, words)
  }
}

/// Rebuild an [`AudioSegment`] from a stored segment row + its word rows.
///
/// The `parent_timebase` is the per-stream timebase carried on the parent
/// `audio_track` row (`duration_tb_num/den` or `start_pts_tb_num/den` —
/// both flatten the same `Timebase` for a given stream). Segment and word
/// spans are reconstructed by stamping their stored PTS ticks with this
/// timebase.
pub fn audio_segment_from_rows(
  r: SqliteAudioSegmentRow,
  mut words: Vec<SqliteAudioSegmentWordRow>,
  parent_timebase: mediatime::Timebase,
) -> Result<AudioSegment<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(&r.id)?;
  let audio_track_id = bytes_to_uuid7(&r.audio_track_id)?;
  let index = u32_from_i64(r.index, "AudioSegment.index")?;
  let span = mediatime::TimeRange::try_new(r.span_start_pts, r.span_end_pts, parent_timebase)
    .ok_or_else(|| {
      SqlxError::DomainConstructorRejected(format!(
        "TimeRange start_pts ({}) must be <= end_pts ({})",
        r.span_start_pts, r.span_end_pts
      ))
    })?;
  let mut s = AudioSegment::try_new(id, audio_track_id, index, span)
    .map_err(|e: AudioSegmentError| SqlxError::DomainConstructorRejected(e.to_string()))?;

  if let Some(sp) = r.speaker_id {
    s = s.with_speaker_id(Some(bytes_to_uuid7(&sp)?));
  }
  if let Some(vid) = r.voice_fingerprint_vector_id {
    let vector_id = bytes_to_uuid7(&vid)?;
    let dimensions = u32::try_from(r.voice_fingerprint_dimensions.unwrap_or(0)).map_err(|e| {
      SqlxError::UnknownDiscriminant(format!("AudioSegment.voice_fingerprint_dimensions: {e}"))
    })?;
    let extracted_at = millis_to_timestamp(r.voice_fingerprint_extracted_at_ms.unwrap_or(0))?;
    let provenance = Provenance::from_parts(
      r.voice_fingerprint_provenance_model_name
        .unwrap_or_default(),
      r.voice_fingerprint_provenance_model_version
        .unwrap_or_default(),
      r.voice_fingerprint_provenance_prompt_version
        .unwrap_or_default(),
      r.voice_fingerprint_provenance_indexer_version
        .unwrap_or_default(),
    );
    s = s.with_voice_fingerprint(Some(VoiceFingerprint::from_parts(
      vector_id,
      dimensions,
      extracted_at,
      r.voice_fingerprint_confidence,
      provenance,
    )));
  }
  s = s
    .with_text(crate::domain::vo::LocalizedText::from_src_translated(
      r.text_src,
      r.text_translated,
    ))
    .with_avg_logprob(r.avg_logprob)
    .with_temperature(r.temperature);
  if let Some(l) = r.language {
    s = s.with_language(Some(parse_language(&l)?));
  }
  s = s
    .try_with_no_speech_prob(r.no_speech_prob)
    .map_err(seg_err)?;

  words.sort_by_key(|w| w.ordinal);
  let mut built = Vec::with_capacity(words.len());
  for w in words {
    let wspan = mediatime::TimeRange::try_new(w.span_start_pts, w.span_end_pts, parent_timebase)
      .ok_or_else(|| {
        SqlxError::DomainConstructorRejected(format!(
          "TimeRange start_pts ({}) must be <= end_pts ({})",
          w.span_start_pts, w.span_end_pts
        ))
      })?;
    let language = match w.language {
      Some(l) => Some(parse_language(&l)?),
      None => None,
    };
    built.push(
      Word::try_from_parts(w.text, wspan, w.score, language)
        .map_err(|e: WordError| SqlxError::DomainConstructorRejected(e.to_string()))?,
    );
  }
  s = s.try_with_words(built).map_err(seg_err)?;

  Ok(s)
}

// ===========================================================================
// SoundEvent
// ===========================================================================

/// SQLite row shape for [`SoundEvent`] — the audio analog of
/// [`SqliteSceneRow`](crate::sqlx::sqlite::video::SqliteSceneRow).
///
/// UUIDs ride as 16-byte `BLOB` (`Vec<u8>`). `span` flattens to `start_pts`
/// / `end_pts` PTS ticks only; the timebase lives on the parent
/// `audio_track` (a single stream has one timebase for all of its sound
/// events). `code` (`Option<u64>`) rides as a nullable `INTEGER`.
/// `detector` ([`CedDetector`]) rides as a `text` slug. There is no nested
/// collection, so unlike `AudioSegment` there is no child table.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteSoundEventRow {
  pub id: Vec<u8>,
  pub audio_track_id: Vec<u8>,
  pub index: i64,
  pub span_start_pts: i64,
  pub span_end_pts: i64,
  pub label: String,
  /// Stable soundevents dataset code; NULL = unmapped class.
  pub code: Option<i64>,
  pub score: f32,
  pub detector: String,
}

impl From<&SoundEvent<Uuid7>> for SqliteSoundEventRow {
  fn from(e: &SoundEvent<Uuid7>) -> Self {
    let span = e.span_ref();
    Self {
      id: e.id_ref().as_bytes().to_vec(),
      audio_track_id: e.audio_track_id_ref().as_bytes().to_vec(),
      index: i64::from(e.index()),
      span_start_pts: span.start_pts(),
      span_end_pts: span.end_pts(),
      label: e.label().to_owned(),
      code: e.code().map(|c| c as i64),
      score: e.score(),
      detector: ced_detector_to_slug(e.detector()).to_owned(),
    }
  }
}

/// Rebuild a [`SoundEvent`] from a stored row. The `parent_timebase` is the
/// per-stream timebase carried on the parent `audio_track` row — see
/// [`audio_segment_from_rows`] for the same pattern.
pub fn sound_event_from_row(
  r: SqliteSoundEventRow,
  parent_timebase: mediatime::Timebase,
) -> Result<SoundEvent<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(&r.id)?;
  let audio_track_id = bytes_to_uuid7(&r.audio_track_id)?;
  let index = u32_from_i64(r.index, "SoundEvent.index")?;
  let span = mediatime::TimeRange::try_new(r.span_start_pts, r.span_end_pts, parent_timebase)
    .ok_or_else(|| {
      SqlxError::DomainConstructorRejected(format!(
        "TimeRange start_pts ({}) must be <= end_pts ({})",
        r.span_start_pts, r.span_end_pts
      ))
    })?;
  let detector = parse_ced_detector(&r.detector)?;
  SoundEvent::try_new(
    id,
    audio_track_id,
    index,
    span,
    r.label,
    r.code.map(|c| c as u64),
    r.score,
    detector,
  )
  .map_err(|e: SoundEventError| SqlxError::DomainConstructorRejected(e.to_string()))
}

// ===========================================================================
// Borrowed-view siblings (`*RowRef<'r>`) — zero-copy decode from `&'r Row`.
// ===========================================================================

/// Borrowed view of [`SqliteAudioRow`] — zero-copy decode from `&'r Row`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteAudioRowRef<'r> {
  pub id: &'r [u8],
  pub media_id: &'r [u8],
  pub total_segments: i64,
  pub track_progress_total: i64,
  pub track_progress_indexed: i64,
  pub track_progress_failed: i64,
}

impl SqliteAudioRow {
  /// Cheap borrow — produces a [`SqliteAudioRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteAudioRowRef<'_> {
    SqliteAudioRowRef {
      id: &self.id,
      media_id: &self.media_id,
      total_segments: self.total_segments,
      track_progress_total: self.track_progress_total,
      track_progress_indexed: self.track_progress_indexed,
      track_progress_failed: self.track_progress_failed,
    }
  }
}

impl<'r> TryFrom<SqliteAudioRowRef<'r>> for Audio<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteAudioRowRef<'r>) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
    let media_id = bytes_to_uuid7(r.media_id)?;
    let total_segments = u32_from_i64(r.total_segments, "Audio.total_segments")?;
    let progress = IndexProgress::from_parts(
      u32_from_i64(r.track_progress_total, "Audio.track_progress_total")?,
      u32_from_i64(r.track_progress_indexed, "Audio.track_progress_indexed")?,
      u32_from_i64(r.track_progress_failed, "Audio.track_progress_failed")?,
    );
    let a = Audio::try_new(id, media_id)
      .map_err(|e: AudioError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    Ok(restore_rollups(a, total_segments, progress))
  }
}

/// Borrowed view of [`SqliteAudioTrackRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteAudioTrackRowRef<'r> {
  pub id: &'r [u8],
  pub audio_id: &'r [u8],
  pub stream_index: Option<i64>,
  pub container_track_id: Option<i64>,
  pub codec: &'r str,
  pub profile: &'r str,
  pub sample_rate: i64,
  pub channels: i64,
  pub channel_layout: &'r str,
  pub sample_format: i64,
  pub bit_rate: i64,
  pub bit_rate_mode: Option<i64>,
  pub bits_per_sample: Option<i64>,
  pub is_lossless: i64,
  pub duration_pts: Option<i64>,
  pub duration_tb_num: Option<i64>,
  pub duration_tb_den: Option<i64>,
  pub start_pts: Option<i64>,
  pub start_pts_tb_num: Option<i64>,
  pub start_pts_tb_den: Option<i64>,
  pub language: Option<&'r str>,
  pub detected_language: Option<&'r str>,
  pub disposition: i64,
  pub is_primary: i64,
  pub auto_selected: i64,
  pub content: Option<i64>,
  pub speech_ratio: Option<f32>,
  pub is_silent: i64,
  pub has_loudness: i64,
  pub loudness_integrated_lufs: Option<f32>,
  pub loudness_range_lu: Option<f32>,
  pub loudness_true_peak_dbtp: Option<f32>,
  pub loudness_sample_peak_dbfs: Option<f32>,
  pub has_replay_gain: i64,
  pub replay_gain_track_gain_db: Option<f32>,
  pub replay_gain_track_peak: Option<f32>,
  pub replay_gain_album_gain_db: Option<f32>,
  pub replay_gain_album_peak: Option<f32>,
  pub fingerprint_algo: Option<&'r str>,
  pub fingerprint_value: Option<&'r [u8]>,
  pub isrc: &'r str,
  pub acoustid: &'r str,
  pub musicbrainz_recording_id: &'r str,
  pub has_tags: i64,
  pub tags_title: Option<&'r str>,
  pub tags_artist: Option<&'r str>,
  pub tags_album_artist: Option<&'r str>,
  pub tags_album: Option<&'r str>,
  pub tags_composer: Option<&'r str>,
  pub tags_genre: Option<&'r str>,
  pub tags_comment: Option<&'r str>,
  pub tags_year: Option<i64>,
  pub tags_track_number: Option<i64>,
  pub tags_track_total: Option<i64>,
  pub tags_disc_number: Option<i64>,
  pub tags_disc_total: Option<i64>,
  pub tags_language: Option<&'r str>,
  pub cover_art_mime: Option<&'r str>,
  pub cover_art_data: Option<&'r [u8]>,
  pub provenance_model_name: &'r str,
  pub provenance_model_version: &'r str,
  pub provenance_prompt_version: &'r str,
  pub provenance_indexer_version: &'r str,
  pub index_status: i64,
}

/// Borrowed view of [`SqliteAudioTrackIndexErrorRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteAudioTrackIndexErrorRowRef<'r> {
  pub audio_track_id: &'r [u8],
  pub ordinal: i64,
  pub code: i64,
  pub message: &'r str,
}

/// Borrowed view of [`SqliteAudioTrackMetadataRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteAudioTrackMetadataRowRef<'r> {
  pub audio_track_id: &'r [u8],
  pub ordinal: i64,
  pub key: &'r str,
  pub value: &'r str,
}

impl SqliteAudioTrackRow {
  /// Cheap borrow — produces a [`SqliteAudioTrackRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteAudioTrackRowRef<'_> {
    SqliteAudioTrackRowRef {
      id: &self.id,
      audio_id: &self.audio_id,
      stream_index: self.stream_index,
      container_track_id: self.container_track_id,
      codec: &self.codec,
      profile: &self.profile,
      sample_rate: self.sample_rate,
      channels: self.channels,
      channel_layout: &self.channel_layout,
      sample_format: self.sample_format,
      bit_rate: self.bit_rate,
      bit_rate_mode: self.bit_rate_mode,
      bits_per_sample: self.bits_per_sample,
      is_lossless: self.is_lossless,
      duration_pts: self.duration_pts,
      duration_tb_num: self.duration_tb_num,
      duration_tb_den: self.duration_tb_den,
      start_pts: self.start_pts,
      start_pts_tb_num: self.start_pts_tb_num,
      start_pts_tb_den: self.start_pts_tb_den,
      language: self.language.as_deref(),
      detected_language: self.detected_language.as_deref(),
      disposition: self.disposition,
      is_primary: self.is_primary,
      auto_selected: self.auto_selected,
      content: self.content,
      speech_ratio: self.speech_ratio,
      is_silent: self.is_silent,
      has_loudness: self.has_loudness,
      loudness_integrated_lufs: self.loudness_integrated_lufs,
      loudness_range_lu: self.loudness_range_lu,
      loudness_true_peak_dbtp: self.loudness_true_peak_dbtp,
      loudness_sample_peak_dbfs: self.loudness_sample_peak_dbfs,
      has_replay_gain: self.has_replay_gain,
      replay_gain_track_gain_db: self.replay_gain_track_gain_db,
      replay_gain_track_peak: self.replay_gain_track_peak,
      replay_gain_album_gain_db: self.replay_gain_album_gain_db,
      replay_gain_album_peak: self.replay_gain_album_peak,
      fingerprint_algo: self.fingerprint_algo.as_deref(),
      fingerprint_value: self.fingerprint_value.as_deref(),
      isrc: &self.isrc,
      acoustid: &self.acoustid,
      musicbrainz_recording_id: &self.musicbrainz_recording_id,
      has_tags: self.has_tags,
      tags_title: self.tags_title.as_deref(),
      tags_artist: self.tags_artist.as_deref(),
      tags_album_artist: self.tags_album_artist.as_deref(),
      tags_album: self.tags_album.as_deref(),
      tags_composer: self.tags_composer.as_deref(),
      tags_genre: self.tags_genre.as_deref(),
      tags_comment: self.tags_comment.as_deref(),
      tags_year: self.tags_year,
      tags_track_number: self.tags_track_number,
      tags_track_total: self.tags_track_total,
      tags_disc_number: self.tags_disc_number,
      tags_disc_total: self.tags_disc_total,
      tags_language: self.tags_language.as_deref(),
      cover_art_mime: self.cover_art_mime.as_deref(),
      cover_art_data: self.cover_art_data.as_deref(),
      provenance_model_name: &self.provenance_model_name,
      provenance_model_version: &self.provenance_model_version,
      provenance_prompt_version: &self.provenance_prompt_version,
      provenance_indexer_version: &self.provenance_indexer_version,
      index_status: self.index_status,
    }
  }
}

impl SqliteAudioTrackIndexErrorRow {
  /// Cheap borrow — produces a [`SqliteAudioTrackIndexErrorRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteAudioTrackIndexErrorRowRef<'_> {
    SqliteAudioTrackIndexErrorRowRef {
      audio_track_id: &self.audio_track_id,
      ordinal: self.ordinal,
      code: self.code,
      message: &self.message,
    }
  }
}

impl SqliteAudioTrackMetadataRow {
  /// Cheap borrow — produces a [`SqliteAudioTrackMetadataRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteAudioTrackMetadataRowRef<'_> {
    SqliteAudioTrackMetadataRowRef {
      audio_track_id: &self.audio_track_id,
      ordinal: self.ordinal,
      key: &self.key,
      value: &self.value,
    }
  }
}

impl<'r>
  TryFrom<(
    SqliteAudioTrackRowRef<'r>,
    Vec<SqliteAudioTrackIndexErrorRowRef<'r>>,
    Vec<SqliteAudioTrackMetadataRowRef<'r>>,
  )> for AudioTrack<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, mut errors, mut metadata): (
      SqliteAudioTrackRowRef<'r>,
      Vec<SqliteAudioTrackIndexErrorRowRef<'r>>,
      Vec<SqliteAudioTrackMetadataRowRef<'r>>,
    ),
  ) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
    let audio_id = bytes_to_uuid7(r.audio_id)?;
    let mut t = AudioTrack::try_new(id, audio_id)
      .map_err(|e: AudioTrackError| SqlxError::DomainConstructorRejected(e.to_string()))?;

    t = t
      .with_codec(parse_audio_codec(r.codec))
      .with_profile(r.profile)
      .with_channel_layout(parse_channel_layout(r.channel_layout))
      .with_sample_format(SampleFormat::from_u32(u32_from_i64(
        r.sample_format,
        "AudioTrack.sample_format",
      )?))
      .with_bit_rate(r.bit_rate as u64)
      .with_lossless(r.is_lossless != 0)
      .with_primary(r.is_primary != 0)
      .with_auto_selected(r.auto_selected != 0)
      .with_silent(r.is_silent != 0)
      .with_stream_index(opt_u32(r.stream_index, "AudioTrack.stream_index")?)
      .with_container_track_id(r.container_track_id.map(|v| v as u64))
      .with_bits_per_sample(opt_u16(r.bits_per_sample, "AudioTrack.bits_per_sample")?)
      .with_disposition(TrackDisposition::from_bits_truncate(u32_from_i64(
        r.disposition,
        "AudioTrack.disposition",
      )?))
      .with_isrc(r.isrc)
      .with_acoustid(r.acoustid)
      .with_musicbrainz_recording_id(r.musicbrainz_recording_id);

    t = t
      .try_with_sample_rate(u32_from_i64(r.sample_rate, "AudioTrack.sample_rate")?)
      .map_err(track_err)?
      .try_with_channels(u16_from_i64(r.channels, "AudioTrack.channels")?)
      .map_err(track_err)?;

    if let Some(m) = r.bit_rate_mode {
      let raw = u32_from_i64(m, "AudioTrack.bit_rate_mode")?;
      let mode = BitRateMode::try_from_u32(raw)
        .ok_or_else(|| SqlxError::UnknownDiscriminant(format!("BitRateMode: {raw}")))?;
      t = t.with_bit_rate_mode(Some(mode));
    }

    if let Some(pts) = r.duration_pts {
      let (num, den) =
        require_timebase(r.duration_tb_num, r.duration_tb_den, "AudioTrack.duration")?;
      t = t
        .try_with_duration(Some(timestamp_from_parts(pts, num, den)?))
        .map_err(track_err)?;
    }
    if let Some(pts) = r.start_pts {
      let (num, den) = require_timebase(
        r.start_pts_tb_num,
        r.start_pts_tb_den,
        "AudioTrack.start_pts",
      )?;
      t = t.with_start_pts(Some(timestamp_from_parts(pts, num, den)?));
    }

    if let Some(s) = r.language {
      t = t.with_language(Some(parse_language(s)?));
    }
    if let Some(s) = r.detected_language {
      t = t.with_detected_language(Some(parse_language(s)?));
    }
    if let Some(c) = r.content {
      t = t.with_content(Some(content_kind_from_i64(c)?));
    }
    if let Some(v) = r.speech_ratio {
      t = t.try_with_speech_ratio(Some(v)).map_err(track_err)?;
    }

    if r.has_loudness != 0 {
      t = t.with_loudness(Some(Loudness::new(
        r.loudness_integrated_lufs.unwrap_or_default(),
        r.loudness_range_lu.unwrap_or_default(),
        r.loudness_true_peak_dbtp.unwrap_or_default(),
        r.loudness_sample_peak_dbfs.unwrap_or_default(),
      )));
    }
    if r.has_replay_gain != 0 {
      t = t.with_replay_gain(Some(ReplayGain::new(
        r.replay_gain_track_gain_db.unwrap_or_default(),
        r.replay_gain_track_peak.unwrap_or_default(),
        r.replay_gain_album_gain_db,
        r.replay_gain_album_peak,
      )));
    }
    if let Some(algo) = r.fingerprint_algo {
      let value = r.fingerprint_value.unwrap_or_default().to_vec();
      t = t.with_fingerprint(Some(Fingerprint::try_new(algo, value).map_err(|e| {
        SqlxError::DomainConstructorRejected(format!("AudioFingerprint: {e}"))
      })?));
    }
    if let Some(mime) = r.cover_art_mime {
      let data = r.cover_art_data.unwrap_or_default().to_vec();
      t = t.with_cover_art(Some(CoverArt::try_new(mime, data).map_err(|e| {
        SqlxError::DomainConstructorRejected(format!("AudioCoverArt: {e}"))
      })?));
    }
    if r.has_tags != 0 {
      let mut tags = Tags::new()
        .with_title(r.tags_title.unwrap_or_default())
        .with_artist(r.tags_artist.unwrap_or_default())
        .with_album_artist(r.tags_album_artist.unwrap_or_default())
        .with_album(r.tags_album.unwrap_or_default())
        .with_composer(r.tags_composer.unwrap_or_default())
        .with_genre(r.tags_genre.unwrap_or_default())
        .with_comment(r.tags_comment.unwrap_or_default())
        .with_year(u16_from_i64_opt(r.tags_year, "Tags.year")?)
        .with_track_number(u16_from_i64_opt(r.tags_track_number, "Tags.track_number")?)
        .with_track_total(u16_from_i64_opt(r.tags_track_total, "Tags.track_total")?)
        .with_disc_number(u16_from_i64_opt(r.tags_disc_number, "Tags.disc_number")?)
        .with_disc_total(u16_from_i64_opt(r.tags_disc_total, "Tags.disc_total")?);
      if let Some(s) = r.tags_language {
        tags = tags.with_language(parse_language(s)?);
      }
      t = t.with_tags(Some(tags));
    }

    t = t.with_provenance(crate::domain::vo::Provenance::from_parts(
      r.provenance_model_name,
      r.provenance_model_version,
      r.provenance_prompt_version,
      r.provenance_indexer_version,
    ));

    let status = AudioIndexStatus::from_bits_truncate(u32_from_i64(
      r.index_status,
      "AudioTrack.index_status",
    )?);
    t = t.try_with_index_status(status).map_err(track_err)?;

    errors.sort_by_key(|e| e.ordinal);
    let mut infos = Vec::with_capacity(errors.len());
    for e in errors {
      let code = u32_from_i64(e.code, "AudioTrack.index_error.code")?;
      infos.push(ErrorInfo::new(ErrorCode::from_u32(code), e.message));
    }
    t = t.with_index_errors(infos);

    metadata.sort_by_key(|m| m.ordinal);
    let mut bag = IndexMap::with_capacity(metadata.len());
    for entry in metadata {
      if entry.audio_track_id != r.id {
        return Err(SqlxError::DomainConstructorRejected(
          "audio_track_metadata.audio_track_id does not match parent audio_track.id".to_owned(),
        ));
      }
      bag.insert(SmolStr::from(entry.key), SmolStr::from(entry.value));
    }
    t = t.with_metadata(bag);

    Ok(t)
  }
}

/// Borrowed view of [`SqliteAudioSegmentRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteAudioSegmentRowRef<'r> {
  pub id: &'r [u8],
  pub audio_track_id: &'r [u8],
  pub index: i64,
  pub span_start_pts: i64,
  pub span_end_pts: i64,
  pub speaker_id: Option<&'r [u8]>,
  pub text_src: &'r str,
  pub text_translated: &'r str,
  pub language: Option<&'r str>,
  pub no_speech_prob: Option<f32>,
  pub avg_logprob: Option<f32>,
  pub temperature: Option<f32>,
  pub voice_fingerprint_vector_id: Option<&'r [u8]>,
  pub voice_fingerprint_dimensions: Option<i64>,
  pub voice_fingerprint_extracted_at_ms: Option<i64>,
  pub voice_fingerprint_confidence: Option<f32>,
  pub voice_fingerprint_provenance_model_name: Option<&'r str>,
  pub voice_fingerprint_provenance_model_version: Option<&'r str>,
  pub voice_fingerprint_provenance_prompt_version: Option<&'r str>,
  pub voice_fingerprint_provenance_indexer_version: Option<&'r str>,
}

/// Borrowed view of [`SqliteAudioSegmentWordRow`].
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteAudioSegmentWordRowRef<'r> {
  pub audio_segment_id: &'r [u8],
  pub ordinal: i64,
  pub text: &'r str,
  pub span_start_pts: i64,
  pub span_end_pts: i64,
  pub score: f32,
  pub language: Option<&'r str>,
}

impl SqliteAudioSegmentRow {
  /// Cheap borrow — produces a [`SqliteAudioSegmentRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteAudioSegmentRowRef<'_> {
    SqliteAudioSegmentRowRef {
      id: &self.id,
      audio_track_id: &self.audio_track_id,
      index: self.index,
      span_start_pts: self.span_start_pts,
      span_end_pts: self.span_end_pts,
      speaker_id: self.speaker_id.as_deref(),
      text_src: &self.text_src,
      text_translated: &self.text_translated,
      language: self.language.as_deref(),
      no_speech_prob: self.no_speech_prob,
      avg_logprob: self.avg_logprob,
      temperature: self.temperature,
      voice_fingerprint_vector_id: self.voice_fingerprint_vector_id.as_deref(),
      voice_fingerprint_dimensions: self.voice_fingerprint_dimensions,
      voice_fingerprint_extracted_at_ms: self.voice_fingerprint_extracted_at_ms,
      voice_fingerprint_confidence: self.voice_fingerprint_confidence,
      voice_fingerprint_provenance_model_name: self
        .voice_fingerprint_provenance_model_name
        .as_deref(),
      voice_fingerprint_provenance_model_version: self
        .voice_fingerprint_provenance_model_version
        .as_deref(),
      voice_fingerprint_provenance_prompt_version: self
        .voice_fingerprint_provenance_prompt_version
        .as_deref(),
      voice_fingerprint_provenance_indexer_version: self
        .voice_fingerprint_provenance_indexer_version
        .as_deref(),
    }
  }
}

impl SqliteAudioSegmentWordRow {
  /// Cheap borrow — produces a [`SqliteAudioSegmentWordRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteAudioSegmentWordRowRef<'_> {
    SqliteAudioSegmentWordRowRef {
      audio_segment_id: &self.audio_segment_id,
      ordinal: self.ordinal,
      text: &self.text,
      span_start_pts: self.span_start_pts,
      span_end_pts: self.span_end_pts,
      score: self.score,
      language: self.language.as_deref(),
    }
  }
}

/// Rebuild an [`AudioSegment`] from a borrowed segment row + its borrowed
/// word rows. The `parent_timebase` is the per-stream timebase carried on
/// the parent `audio_track` row — see [`audio_segment_from_rows`] for the
/// owned counterpart.
pub fn audio_segment_from_row_refs<'r>(
  r: SqliteAudioSegmentRowRef<'r>,
  mut words: Vec<SqliteAudioSegmentWordRowRef<'r>>,
  parent_timebase: mediatime::Timebase,
) -> Result<AudioSegment<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(r.id)?;
  let audio_track_id = bytes_to_uuid7(r.audio_track_id)?;
  let index = u32_from_i64(r.index, "AudioSegment.index")?;
  let span = mediatime::TimeRange::try_new(r.span_start_pts, r.span_end_pts, parent_timebase)
    .ok_or_else(|| {
      SqlxError::DomainConstructorRejected(format!(
        "TimeRange start_pts ({}) must be <= end_pts ({})",
        r.span_start_pts, r.span_end_pts
      ))
    })?;
  let mut s = AudioSegment::try_new(id, audio_track_id, index, span)
    .map_err(|e: AudioSegmentError| SqlxError::DomainConstructorRejected(e.to_string()))?;

  if let Some(sp) = r.speaker_id {
    s = s.with_speaker_id(Some(bytes_to_uuid7(sp)?));
  }
  if let Some(vid) = r.voice_fingerprint_vector_id {
    let vector_id = bytes_to_uuid7(vid)?;
    let dimensions = u32::try_from(r.voice_fingerprint_dimensions.unwrap_or(0)).map_err(|e| {
      SqlxError::UnknownDiscriminant(format!("AudioSegment.voice_fingerprint_dimensions: {e}"))
    })?;
    let extracted_at = millis_to_timestamp(r.voice_fingerprint_extracted_at_ms.unwrap_or(0))?;
    let provenance = Provenance::from_parts(
      r.voice_fingerprint_provenance_model_name
        .unwrap_or_default(),
      r.voice_fingerprint_provenance_model_version
        .unwrap_or_default(),
      r.voice_fingerprint_provenance_prompt_version
        .unwrap_or_default(),
      r.voice_fingerprint_provenance_indexer_version
        .unwrap_or_default(),
    );
    s = s.with_voice_fingerprint(Some(VoiceFingerprint::from_parts(
      vector_id,
      dimensions,
      extracted_at,
      r.voice_fingerprint_confidence,
      provenance,
    )));
  }
  s = s
    .with_text(crate::domain::vo::LocalizedText::from_src_translated(
      r.text_src,
      r.text_translated,
    ))
    .with_avg_logprob(r.avg_logprob)
    .with_temperature(r.temperature);
  if let Some(l) = r.language {
    s = s.with_language(Some(parse_language(l)?));
  }
  s = s
    .try_with_no_speech_prob(r.no_speech_prob)
    .map_err(seg_err)?;

  words.sort_by_key(|w| w.ordinal);
  let mut built = Vec::with_capacity(words.len());
  for w in words {
    let wspan = mediatime::TimeRange::try_new(w.span_start_pts, w.span_end_pts, parent_timebase)
      .ok_or_else(|| {
        SqlxError::DomainConstructorRejected(format!(
          "TimeRange start_pts ({}) must be <= end_pts ({})",
          w.span_start_pts, w.span_end_pts
        ))
      })?;
    let language = match w.language {
      Some(l) => Some(parse_language(l)?),
      None => None,
    };
    built.push(
      Word::try_from_parts(w.text, wspan, w.score, language)
        .map_err(|e: WordError| SqlxError::DomainConstructorRejected(e.to_string()))?,
    );
  }
  s = s.try_with_words(built).map_err(seg_err)?;

  Ok(s)
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

fn parse_audio_codec(s: &str) -> AudioCodec {
  s.parse::<AudioCodec>()
    .unwrap_or_else(|_| AudioCodec::Other(s.into()))
}

fn parse_channel_layout(s: &str) -> ChannelLayout {
  s.parse::<ChannelLayout>()
    .unwrap_or_else(|_| ChannelLayout::Other(s.into()))
}

fn parse_language(s: &str) -> Result<Language, SqlxError> {
  Language::from_bcp47(s)
    .map_err(|e| SqlxError::DomainConstructorRejected(format!("Language `{s}`: {e}")))
}

fn track_err(e: AudioTrackError) -> SqlxError {
  SqlxError::DomainConstructorRejected(e.to_string())
}

fn seg_err(e: AudioSegmentError) -> SqlxError {
  SqlxError::DomainConstructorRejected(e.to_string())
}

fn ced_detector_to_slug(d: CedDetector) -> &'static str {
  match d {
    CedDetector::Ced => "ced",
    CedDetector::Manual => "manual",
  }
}

fn parse_ced_detector(s: &str) -> Result<CedDetector, SqlxError> {
  Ok(match s {
    "ced" => CedDetector::Ced,
    "manual" => CedDetector::Manual,
    other => {
      return Err(SqlxError::UnknownDiscriminant(format!(
        "CedDetector slug: {other}"
      )))
    }
  })
}

fn u32_from_i64(v: i64, what: &str) -> Result<u32, SqlxError> {
  u32::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
}

fn u16_from_i64(v: i64, what: &str) -> Result<u16, SqlxError> {
  u16::try_from(v).map_err(|e| SqlxError::UnknownDiscriminant(format!("{what}: {e}")))
}

fn u16_from_i64_opt(v: Option<i64>, what: &str) -> Result<u16, SqlxError> {
  match v {
    None => Ok(0),
    Some(x) => u16_from_i64(x, what),
  }
}

fn opt_u32(v: Option<i64>, what: &str) -> Result<Option<u32>, SqlxError> {
  match v {
    None => Ok(None),
    Some(x) => Ok(Some(u32_from_i64(x, what)?)),
  }
}

fn opt_u16(v: Option<i64>, what: &str) -> Result<Option<u16>, SqlxError> {
  match v {
    None => Ok(None),
    Some(x) => Ok(Some(u16_from_i64(x, what)?)),
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
  use core::num::NonZeroU32;
  use mediatime::{TimeRange, Timebase, Timestamp};

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).unwrap())
  }

  #[test]
  fn audio_facet_roundtrip() {
    let a = Audio::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_total_segments(5)
      .with_track_progress(IndexProgress::try_new(1, 1, 0).unwrap());
    let row: SqliteAudioRow = (&a).into();
    let a2: Audio<Uuid7> = row.try_into().unwrap();
    assert_eq!(a.id_ref(), a2.id_ref());
    assert_eq!(a2.total_segments(), 5);
    assert_eq!(a2.track_progress_ref().indexed(), 1);
  }

  #[test]
  fn audio_track_roundtrip_minimal() {
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new()).unwrap();
    let tuple: (
      SqliteAudioTrackRow,
      Vec<SqliteAudioTrackIndexErrorRow>,
      Vec<SqliteAudioTrackMetadataRow>,
    ) = (&t).into();
    let t2: AudioTrack<Uuid7> = tuple.try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn audio_track_metadata_roundtrip_preserves_insertion_order() {
    let mut meta = IndexMap::new();
    meta.insert(SmolStr::new("encoder"), SmolStr::new("flac 1.4.3"));
    meta.insert(SmolStr::new("compatible_brands"), SmolStr::new("isom"));
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_metadata(meta)
      .with_sample_format(SampleFormat::Fltp);
    let (row, errs, mut metadata): (
      SqliteAudioTrackRow,
      Vec<SqliteAudioTrackIndexErrorRow>,
      Vec<SqliteAudioTrackMetadataRow>,
    ) = (&t).into();
    assert_eq!(metadata.len(), 2);
    assert_eq!(row.sample_format, i64::from(SampleFormat::Fltp.to_u32()));
    metadata.reverse();
    let t2: AudioTrack<Uuid7> = (row, errs, metadata).try_into().unwrap();
    let keys: Vec<&str> = t2.metadata_ref().keys().map(SmolStr::as_str).collect();
    assert_eq!(keys, std::vec!["encoder", "compatible_brands"]);
    assert_eq!(t2.sample_format_ref(), &SampleFormat::Fltp);
  }

  #[test]
  fn audio_track_roundtrip_full() {
    let en = Language::from_bcp47("en").unwrap();
    let tags = Tags::new()
      .with_title("Song")
      .with_artist("Band")
      .with_track_number(3)
      .with_year(2020);
    let cover = CoverArt::try_new("image/png", std::vec![1u8, 2, 3]).unwrap();
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(AudioCodec::Opus)
      .with_profile("LC")
      .try_with_sample_rate(44_100)
      .unwrap()
      .try_with_channels(2)
      .unwrap()
      .with_channel_layout(ChannelLayout::Stereo)
      .with_bit_rate(128_000)
      .with_bit_rate_mode(Some(BitRateMode::Cbr))
      .with_bits_per_sample(Some(16))
      .with_lossless(false)
      .try_with_duration(Some(Timestamp::new(60_000, tb())))
      .unwrap()
      .with_start_pts(Some(Timestamp::new(0, tb())))
      .with_language(Some(en))
      .with_content(Some(AudioContentKind::Speech))
      .try_with_speech_ratio(Some(0.9))
      .unwrap()
      .with_silent(false)
      .with_loudness(Some(Loudness::new(-16.0, 5.0, -2.0, -4.0)))
      .with_fingerprint(Some(
        Fingerprint::try_new("chromaprint", std::vec![1u8, 2]).unwrap(),
      ))
      .with_isrc("ISRC1")
      .with_tags(Some(tags))
      .with_cover_art(Some(cover))
      .with_provenance(crate::domain::vo::Provenance::from_parts(
        "asry", "1.0", "p", "idx",
      ))
      .try_with_index_status(AudioIndexStatus::EXTRACTED)
      .unwrap()
      .with_index_errors(std::vec![
        ErrorInfo::new(ErrorCode::ProbeCorrupt, "x"),
        ErrorInfo::new(ErrorCode::TranscriptionFailed, "y"),
      ]);
    let tuple: (
      SqliteAudioTrackRow,
      Vec<SqliteAudioTrackIndexErrorRow>,
      Vec<SqliteAudioTrackMetadataRow>,
    ) = (&t).into();
    assert_eq!(tuple.1.len(), 2);
    let t2: AudioTrack<Uuid7> = tuple.try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn audio_segment_roundtrip_full() {
    let de = Language::from_bcp47("de").unwrap();
    let w1 = Word::try_from_parts("hallo", TimeRange::new(0, 300, tb()), 0.95, Some(de)).unwrap();
    let s = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 1, TimeRange::new(0, 800, tb()))
      .unwrap()
      .with_speaker_id(Some(Uuid7::new()))
      .with_text(crate::domain::vo::LocalizedText::from_src("hallo welt"))
      .with_language(Some(de))
      .try_with_no_speech_prob(Some(0.1))
      .unwrap()
      .with_temperature(Some(0.2))
      .try_with_words(std::vec![w1])
      .unwrap();
    let (row, words): (SqliteAudioSegmentRow, Vec<SqliteAudioSegmentWordRow>) = (&s).into();
    let s2 = audio_segment_from_rows(row, words, tb()).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn audio_segment_roundtrip_with_voice_fingerprint() {
    let vfp = VoiceFingerprint::try_new(
      Uuid7::new(),
      192,
      jiff::Timestamp::from_millisecond(1_700_000_000_000).unwrap(),
      Some(0.83),
      Provenance::from_parts("ecapa-tdnn", "v1.0.0", "", "findit-indexer-0.1.0"),
    )
    .unwrap();
    let s = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, TimeRange::new(0, 1000, tb()))
      .unwrap()
      .with_voice_fingerprint(Some(vfp.clone()));
    let (row, words): (SqliteAudioSegmentRow, Vec<SqliteAudioSegmentWordRow>) = (&s).into();
    assert!(row.voice_fingerprint_vector_id.is_some());
    let s2 = audio_segment_from_rows(row, words, tb()).unwrap();
    assert_eq!(s2.voice_fingerprint_ref(), Some(&vfp));
  }

  #[test]
  fn audio_segment_words_rebuild_in_ordinal_order() {
    let w1 = Word::try_new("one", TimeRange::new(0, 100, tb()), 0.9).unwrap();
    let w2 = Word::try_new("two", TimeRange::new(100, 200, tb()), 0.9).unwrap();
    let w3 = Word::try_new("three", TimeRange::new(200, 300, tb()), 0.9).unwrap();
    let s = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 0, TimeRange::new(0, 300, tb()))
      .unwrap()
      .try_with_words(std::vec![w1, w2, w3])
      .unwrap();
    let (row, mut words): (SqliteAudioSegmentRow, Vec<SqliteAudioSegmentWordRow>) = (&s).into();
    words.reverse();
    let s2 = audio_segment_from_rows(row, words, tb()).unwrap();
    assert_eq!(s2.words_slice()[0].text(), "one");
    assert_eq!(s2.words_slice()[2].text(), "three");
  }

  #[test]
  fn audio_facet_ref_roundtrip() {
    let a = Audio::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_total_segments(7)
      .with_track_progress(IndexProgress::try_new(3, 2, 1).unwrap());
    let row: SqliteAudioRow = (&a).into();
    let a2: Audio<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(a.id_ref(), a2.id_ref());
    assert_eq!(a.media_id_ref(), a2.media_id_ref());
    assert_eq!(a2.total_segments(), 7);
  }

  #[test]
  fn audio_track_ref_roundtrip() {
    let en = Language::from_bcp47("en").unwrap();
    let tags = Tags::new().with_title("Song").with_language(en);
    let t = AudioTrack::try_new(Uuid7::new(), Uuid7::new())
      .unwrap()
      .with_codec(AudioCodec::Flac)
      .with_profile("LC")
      .try_with_sample_rate(48_000)
      .unwrap()
      .try_with_channels(2)
      .unwrap()
      .with_channel_layout(ChannelLayout::Stereo)
      .with_bit_rate(900_000)
      .with_lossless(true)
      .with_language(Some(en))
      .with_disposition(TrackDisposition::empty())
      .with_loudness(Some(Loudness::new(-14.0, 6.0, -1.0, -3.0)))
      .with_fingerprint(Some(
        Fingerprint::try_new("chromaprint", std::vec![9u8, 8, 7]).unwrap(),
      ))
      .with_isrc("US-XXX-00-00000")
      .with_tags(Some(tags))
      .with_provenance(crate::domain::vo::Provenance::from_parts(
        "asry", "1.0", "p1", "idx-2",
      ))
      .try_with_index_status(AudioIndexStatus::EXTRACTED)
      .unwrap()
      .with_index_errors(std::vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad")]);
    let (row, errs, meta): (
      SqliteAudioTrackRow,
      Vec<SqliteAudioTrackIndexErrorRow>,
      Vec<SqliteAudioTrackMetadataRow>,
    ) = (&t).into();
    let err_refs: Vec<SqliteAudioTrackIndexErrorRowRef<'_>> = errs
      .iter()
      .map(SqliteAudioTrackIndexErrorRow::as_ref)
      .collect();
    let meta_refs: Vec<SqliteAudioTrackMetadataRowRef<'_>> = meta
      .iter()
      .map(SqliteAudioTrackMetadataRow::as_ref)
      .collect();
    let t2: AudioTrack<Uuid7> = (row.as_ref(), err_refs, meta_refs).try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn audio_segment_ref_roundtrip() {
    let es = Language::from_bcp47("es").unwrap();
    let w1 = Word::try_from_parts("hola", TimeRange::new(0, 400, tb()), 0.9, Some(es)).unwrap();
    let w2 = Word::try_new("mundo", TimeRange::new(400, 900, tb()), 0.8).unwrap();
    let s = AudioSegment::try_new(Uuid7::new(), Uuid7::new(), 4, TimeRange::new(0, 1000, tb()))
      .unwrap()
      .with_speaker_id(Some(Uuid7::new()))
      .with_text(crate::domain::vo::LocalizedText::from_src_translated(
        "hola mundo",
        "hello world",
      ))
      .with_language(Some(es))
      .try_with_no_speech_prob(Some(0.02))
      .unwrap()
      .with_avg_logprob(Some(-0.3))
      .try_with_words(std::vec![w1, w2])
      .unwrap();
    let (row, words): (SqliteAudioSegmentRow, Vec<SqliteAudioSegmentWordRow>) = (&s).into();
    let word_refs: Vec<SqliteAudioSegmentWordRowRef<'_>> = words
      .iter()
      .map(SqliteAudioSegmentWordRow::as_ref)
      .collect();
    let s2 = audio_segment_from_row_refs(row.as_ref(), word_refs, tb()).unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn sound_event_roundtrip_minimal() {
    let e = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 1500, tb()),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    )
    .unwrap();
    let row: SqliteSoundEventRow = (&e).into();
    assert!(row.code.is_none());
    let e2 = sound_event_from_row(row, tb()).unwrap();
    assert_eq!(e, e2);
  }

  #[test]
  fn sound_event_roundtrip_full() {
    let e = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      7,
      TimeRange::new(5_000, 10_000, tb()),
      "Siren",
      Some(316),
      0.87,
      CedDetector::Manual,
    )
    .unwrap();
    let row: SqliteSoundEventRow = (&e).into();
    assert_eq!(row.code, Some(316));
    assert_eq!(row.detector, "manual");
    let e2 = sound_event_from_row(row, tb()).unwrap();
    assert_eq!(e, e2);
  }

  #[test]
  fn sound_event_row_with_unknown_detector_rejected() {
    let e = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 100, tb()),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    )
    .unwrap();
    let mut row: SqliteSoundEventRow = (&e).into();
    row.detector = "bogus".to_owned();
    assert!(sound_event_from_row(row, tb())
      .unwrap_err()
      .is_unknown_discriminant());
  }

  #[test]
  fn sound_event_row_with_nil_uuid_rejected() {
    let e = SoundEvent::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      TimeRange::new(0, 100, tb()),
      "Speech",
      None,
      0.5,
      CedDetector::Ced,
    )
    .unwrap();
    let mut row: SqliteSoundEventRow = (&e).into();
    row.id = std::vec![0u8; 16];
    assert!(sound_event_from_row(row, tb())
      .unwrap_err()
      .is_invalid_uuid());
  }
}
