//! Wire ⇄ graph conversions for the tree root: `media.v2::Media` ⇄
//! [`graph::Media`], plus its directly-owned children
//! `media.v2::MediaFile` ⇄ [`graph::MediaFile`] and `media.v2::Chapter`
//! ⇄ [`graph::Chapter`].
//!
//! ## Field correspondence — `Media`
//!
//! | wire field                       | graph field                  | notes                                          |
//! | -------------------------------- | ---------------------------- | ---------------------------------------------- |
//! | `id` (bytes, 16)                 | `id` (Uuid7)                 | validating                                     |
//! | `checksum` (bytes, 32)           | `checksum`                   | validating; all-zero rejected by `Media::try_new` |
//! | `format: ContainerFormat`        | `format`                     | mediaframe extern; unset ⇒ `Format::default()` (the `Other("")` absent sentinel) |
//! | `size: uint64`                   | `size`                       |                                                |
//! | `duration: Timestamp`            | `duration: Option<_>`        | mediatime extern; presence = `Some`            |
//! | `kind: DbMediaKind`              | `kind`                       | closed enum; `UNSPECIFIED`/unknown rejected    |
//! | `nb_streams` / `nb_chapters`     | same                         |                                                |
//! | `files: repeated MediaFile`      | `files: Vec<MediaFile>`      | children embedded                              |
//! | `chapters: repeated Chapter`     | `chapters: Vec<Chapter>`     | children embedded                              |
//! | `video` / `audio` / `subtitle`   | `Option<Video/Audio/Subtitle>` | facet subtrees; flat facet links re-derived on decode |
//! | `error_flags: uint32`            | `error_flags: MediaErrorFlags` | raw bits (`u16` widened); overflow ⇒ `Unsupported` |
//! | `probe_error: ErrorInfo`         | `probe_error: Option<_>`     | presence = `Some`                              |
//! | `capture_date_ms: optional int64`| `capture_date: Option<jiff::Timestamp>` | epoch millis                        |
//! | `device: Device`                 | `device: Option<_>`          | mediaframe extern; presence = `Some`           |
//! | `gps: GeoLocation`               | `gps: Option<_>`             | mediaframe extern; presence = `Some`           |
//!
//! ## Field correspondence — `MediaFile`
//!
//! | wire field                        | graph field             | notes                                  |
//! | --------------------------------- | ----------------------- | -------------------------------------- |
//! | `id` (bytes, 16)                  | `id`                    | validating                             |
//! | `created_at_ms: optional int64`   | `created_at: Option<_>` | epoch millis                           |
//! | `location: media.v1.Location`     | `location`              | required; `Local` arm only             |
//! | `watched_location_id` (bytes, 16) | `watched_location_id`   | cross-tree association, kept           |
//! | `watch_volume` (bytes, 16)        | `watch_volume`          | validating                             |
//!
//! Flat reconstruction uses [`domain::MediaFile::from_parts`] — the
//! sanctioned wire/storage door (`try_new` wants the discovering
//! `&WatchedLocation` aggregate, which the wire doesn't carry) — exactly
//! like the v1 [`media_file`](crate::buffa::media_file) bridge.
//!
//! ## Field correspondence — `Chapter`
//!
//! | wire field                  | graph field          | notes                                 |
//! | --------------------------- | -------------------- | ------------------------------------- |
//! | `id` (bytes, 16)            | `id`                 | validating                            |
//! | `index: uint32`             | `index`              |                                       |
//! | `source_id: int64`          | `source_id`          |                                       |
//! | `time_range: TimeRange`     | `time_range`         | mediatime extern; required            |
//! | `title: string`             | `title`              | `""` = absent                         |
//! | `metadata: repeated KeyValue` | `metadata: IndexMap` | insertion order preserved           |

use buffa::{EnumValue, MessageField};
use smol_str::SmolStr;

use super::{
  checksum_from_wire, checksum_to_wire, graph_err, id_from_wire, id_to_wire, metadata_from_wire,
  metadata_to_wire, ms_to_jiff, narrow_u16, opt_msg, rejected,
};
use crate::{
  buffa::error::BuffaError,
  domain::{self, ErrorInfo, Location, MediaErrorFlags, MediaKind, Uuid7},
  generated::media::{v1 as wire1, v2 as wire},
  graph,
};

// ---------------------------------------------------------------------------
// graph::Media ⇄ wire::Media
// ---------------------------------------------------------------------------

/// Encode the whole record. Fallible only through the video subtree
/// (phase-A scenes guard); every other field is infallible.
impl TryFrom<&graph::Media<Uuid7>> for wire::Media {
  type Error = BuffaError;

  fn try_from(g: &graph::Media<Uuid7>) -> Result<Self, Self::Error> {
    let video = match g.video_ref() {
      Some(v) => MessageField::some(wire::Video::try_from(v)?),
      None => MessageField::none(),
    };
    Ok(wire::Media {
      id: id_to_wire(g.id_ref()),
      checksum: checksum_to_wire(g.checksum_ref()),
      format: MessageField::some(g.format_ref().clone()),
      size: g.size(),
      duration: opt_msg(g.duration_ref().copied()),
      kind: EnumValue::from(g.kind()),
      nb_streams: g.nb_streams(),
      nb_chapters: g.nb_chapters(),
      files: g.files_slice().iter().map(wire::MediaFile::from).collect(),
      chapters: g.chapters_slice().iter().map(wire::Chapter::from).collect(),
      video,
      audio: opt_msg(g.audio_ref().map(wire::Audio::from)),
      subtitle: opt_msg(g.subtitle_ref().map(wire::Subtitle::from)),
      error_flags: u32::from(g.error_flags().bits()),
      probe_error: opt_msg(g.probe_error_ref().map(wire1::ErrorInfo::from)),
      capture_date_ms: g.capture_date_ref().map(|t| t.as_millisecond()),
      device: opt_msg(g.device_ref().cloned()),
      gps: opt_msg(g.gps_ref().copied()),
      __buffa_unknown_fields: Default::default(),
    })
  }
}

/// Decode the whole record: facets first (their own `TryFrom`s), then
/// the flat root with the facet links re-derived from the embedded
/// facets, then the lift (which re-validates the whole tree's id
/// coherence).
impl TryFrom<&wire::Media> for graph::Media<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::Media) -> Result<Self, Self::Error> {
    let id = id_from_wire(&w.id, "Media.id")?;
    let checksum = checksum_from_wire(&w.checksum)?;
    let format = w.format.as_option().cloned().unwrap_or_default();
    let kind = MediaKind::try_from(&w.kind)?;
    let error_flags =
      MediaErrorFlags::from_bits_retain(narrow_u16(w.error_flags, "Media.error_flags: u16")?);
    let capture_date = w.capture_date_ms.map(ms_to_jiff).transpose()?;

    let video = w
      .video
      .as_option()
      .map(graph::Video::try_from)
      .transpose()?;
    let audio = w
      .audio
      .as_option()
      .map(graph::Audio::try_from)
      .transpose()?;
    let subtitle = w
      .subtitle
      .as_option()
      .map(graph::Subtitle::try_from)
      .transpose()?;
    let files = w
      .files
      .iter()
      .map(|f| flat_media_file(f, id))
      .collect::<Result<Vec<_>, _>>()?;
    let chapters = w
      .chapters
      .iter()
      .map(|c| flat_chapter(c, id))
      .collect::<Result<Vec<_>, _>>()?;

    let flat = domain::Media::try_new(id, checksum, format, w.size, kind)
      .map_err(rejected)?
      .try_with_duration(w.duration.as_option().copied())
      .map_err(rejected)?
      .with_nb_streams(w.nb_streams)
      .with_nb_chapters(w.nb_chapters)
      .with_video_id(video.as_ref().map(|v| *v.id_ref()))
      .with_audio_id(audio.as_ref().map(|a| *a.id_ref()))
      .with_subtitle_id(subtitle.as_ref().map(|s| *s.id_ref()))
      .with_error_flags(error_flags)
      .with_probe_error(w.probe_error.as_option().map(ErrorInfo::from))
      .with_capture_date(capture_date)
      .with_device(w.device.as_option().cloned())
      .with_gps(w.gps.as_option().copied());

    graph::Media::try_from_flat(flat, files, chapters, video, audio, subtitle).map_err(graph_err)
  }
}

// ---------------------------------------------------------------------------
// graph::MediaFile ⇄ wire::MediaFile
// ---------------------------------------------------------------------------

impl From<&graph::MediaFile<Uuid7>> for wire::MediaFile {
  fn from(g: &graph::MediaFile<Uuid7>) -> Self {
    wire::MediaFile {
      id: id_to_wire(g.id_ref()),
      created_at_ms: g.created_at_ref().map(|t| t.as_millisecond()),
      location: MessageField::some(wire1::Location::from(g.location_ref())),
      watched_location_id: id_to_wire(g.watched_location_id_ref()),
      watch_volume: id_to_wire(g.watch_volume_ref()),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Reconstruct the flat copy record under the given `media_id`.
fn flat_media_file(
  w: &wire::MediaFile,
  media_id: Uuid7,
) -> Result<domain::MediaFile<Uuid7>, BuffaError> {
  let id = id_from_wire(&w.id, "MediaFile.id")?;
  let watched_location_id = id_from_wire(&w.watched_location_id, "MediaFile.watched_location_id")?;
  let watch_volume = id_from_wire(&w.watch_volume, "MediaFile.watch_volume")?;
  let location = match w.location.as_option() {
    Some(l) => Option::<Location<Uuid7>>::try_from(l)?
      .ok_or(BuffaError::MissingRequiredField("MediaFile.location"))?,
    None => return Err(BuffaError::MissingRequiredField("MediaFile.location")),
  };
  let created_at = w.created_at_ms.map(ms_to_jiff).transpose()?;
  Ok(domain::MediaFile::from_parts(
    id,
    media_id,
    created_at,
    location,
    watched_location_id,
    watch_volume,
  ))
}

/// Standalone decode — the parent FK is synthesized from the file's own
/// id and consumed by the lift (see the module doc of
/// [`graph`](crate::buffa::graph)).
impl TryFrom<&wire::MediaFile> for graph::MediaFile<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::MediaFile) -> Result<Self, Self::Error> {
    let synthetic_parent = id_from_wire(&w.id, "MediaFile.id")?;
    let flat = flat_media_file(w, synthetic_parent)?;
    graph::MediaFile::try_from_flat(&synthetic_parent, flat).map_err(graph_err)
  }
}

// ---------------------------------------------------------------------------
// graph::Chapter ⇄ wire::Chapter
// ---------------------------------------------------------------------------

impl From<&graph::Chapter<Uuid7>> for wire::Chapter {
  fn from(g: &graph::Chapter<Uuid7>) -> Self {
    wire::Chapter {
      id: id_to_wire(g.id_ref()),
      index: g.index(),
      source_id: g.source_id(),
      time_range: MessageField::some(*g.time_range_ref()),
      title: SmolStr::from(g.title()),
      metadata: metadata_to_wire(g.metadata_ref()),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Reconstruct the flat chapter under the given `media_id`.
fn flat_chapter(w: &wire::Chapter, media_id: Uuid7) -> Result<domain::Chapter<Uuid7>, BuffaError> {
  let id = id_from_wire(&w.id, "Chapter.id")?;
  let time_range = *w
    .time_range
    .as_option()
    .ok_or(BuffaError::MissingRequiredField("Chapter.time_range"))?;
  domain::Chapter::try_new(id, media_id, w.index, w.source_id, time_range)
    .map_err(rejected)?
    .try_with_title(w.title.as_str())
    .map_err(rejected)?
    .try_with_metadata(metadata_from_wire(&w.metadata))
    .map_err(rejected)
}

/// Standalone decode — the parent FK is synthesized from the chapter's
/// own id and consumed by the lift.
impl TryFrom<&wire::Chapter> for graph::Chapter<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::Chapter) -> Result<Self, Self::Error> {
    let synthetic_parent = id_from_wire(&w.id, "Chapter.id")?;
    let flat = flat_chapter(w, synthetic_parent)?;
    graph::Chapter::try_from_flat(&synthetic_parent, flat).map_err(graph_err)
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use core::num::NonZeroU32;

  use jiff::Timestamp as JiffTimestamp;
  use mediaframe::{
    capture::{Device, GeoLocation},
    container::Format,
    lang::Language,
  };
  use mediatime::{TimeRange, Timebase, Timestamp};

  use super::*;
  use crate::domain::{
    aggregates::subtitle::{SrtData, SubtitleCueDetails},
    AudioContentKind, AudioIndexStatus, ErrorCode, FileChecksum, IndexProgress, LocalizedText,
    Provenance, SubtitleKind, VideoIndexStatus, VoiceFingerprint, WatchedLocation, Word,
  };

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).expect("nonzero"))
  }

  fn span(start: i64, end: i64) -> TimeRange {
    TimeRange::new(start, end, tb())
  }

  fn ts() -> JiffTimestamp {
    JiffTimestamp::from_millisecond(1_700_000_000_000).expect("valid ts")
  }

  fn vfp() -> VoiceFingerprint<Uuid7> {
    VoiceFingerprint::try_new(
      Uuid7::new(),
      192,
      ts(),
      Some(0.83),
      Provenance::from_parts("ecapa-tdnn", "v1.0.0", "", "findit-indexer-0.1.0"),
    )
    .expect("valid voiceprint")
  }

  fn flat_file(media_id: Uuid7) -> domain::MediaFile<Uuid7> {
    let volume = Uuid7::new();
    let wl = WatchedLocation::try_new(Uuid7::new(), volume, JiffTimestamp::default())
      .expect("valid watch");
    let loc = Location::try_local_uuid7(volume, ["Movies", "clip.mp4"]).expect("valid location");
    domain::MediaFile::try_new(Uuid7::new(), media_id, Some(ts()), loc, &wl)
      .expect("valid media file")
  }

  fn flat_chapter_fixture(media_id: Uuid7) -> domain::Chapter<Uuid7> {
    let mut bag = indexmap::IndexMap::new();
    bag.insert(SmolStr::from("comment"), SmolStr::from("intro"));
    domain::Chapter::try_new(Uuid7::new(), media_id, 0, 7, span(0, 60_000))
      .expect("valid chapter")
      .try_with_title("Intro")
      .expect("title fits")
      .try_with_metadata(bag)
      .expect("metadata fits")
  }

  // ---- Chapter ---------------------------------------------------------------

  #[test]
  fn chapter_round_trips() {
    let media_id = Uuid7::new();
    let g =
      graph::Chapter::try_from_flat(&media_id, flat_chapter_fixture(media_id)).expect("coherent");
    let w = wire::Chapter::from(&g);
    let g2 = graph::Chapter::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }

  #[test]
  fn chapter_missing_time_range_errors() {
    let media_id = Uuid7::new();
    let g =
      graph::Chapter::try_from_flat(&media_id, flat_chapter_fixture(media_id)).expect("coherent");
    let mut w = wire::Chapter::from(&g);
    w.time_range = MessageField::none();
    let err = graph::Chapter::try_from(&w).unwrap_err();
    assert!(err.is_missing_required_field());
  }

  // ---- MediaFile -------------------------------------------------------------

  #[test]
  fn media_file_round_trips() {
    let media_id = Uuid7::new();
    let g = graph::MediaFile::try_from_flat(&media_id, flat_file(media_id)).expect("coherent");
    let w = wire::MediaFile::from(&g);
    let g2 = graph::MediaFile::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }

  #[test]
  fn media_file_missing_location_errors() {
    let media_id = Uuid7::new();
    let g = graph::MediaFile::try_from_flat(&media_id, flat_file(media_id)).expect("coherent");
    let mut w = wire::MediaFile::from(&g);
    w.location = MessageField::none();
    let err = graph::MediaFile::try_from(&w).unwrap_err();
    assert!(err.is_missing_required_field());
  }

  // ---- Media (minimal) -------------------------------------------------------

  fn flat_media(id: Uuid7) -> domain::Media<Uuid7> {
    domain::Media::try_new(
      id,
      FileChecksum::from_bytes([7u8; 32]),
      Format::Mp4,
      1024,
      MediaKind::Video,
    )
    .expect("valid media")
  }

  #[test]
  fn media_minimal_round_trips() {
    let id = Uuid7::new();
    let g = graph::Media::try_from_flat(flat_media(id), vec![], vec![], None, None, None)
      .expect("coherent");
    let w = wire::Media::try_from(&g).expect("encodes");
    assert!(w.video.is_unset());
    assert!(w.capture_date_ms.is_none());
    let g2 = graph::Media::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }

  #[test]
  fn media_error_flags_overflow_errors() {
    let id = Uuid7::new();
    let g = graph::Media::try_from_flat(flat_media(id), vec![], vec![], None, None, None)
      .expect("coherent");
    let mut w = wire::Media::try_from(&g).expect("encodes");
    w.error_flags = 0x1_0000;
    let err = graph::Media::try_from(&w).unwrap_err();
    assert!(err.is_unsupported());
  }

  // ---- Media (full tree) -----------------------------------------------------

  /// The headline fixture: one media row carrying a file, a chapter and
  /// all three facet subtrees (video track; audio track with one
  /// segment + one speaker; subtitle track with one SRT cue), pushed
  /// through `graph → wire → graph` and compared whole.
  #[test]
  fn media_full_tree_round_trips() {
    let media_id = Uuid7::new();

    // Video facet + one track (no scenes — phase A).
    let vfacet = domain::Video::try_new(Uuid7::new(), media_id)
      .expect("valid facet")
      .with_total_scenes(0)
      .with_track_progress(IndexProgress::try_new(1, 1, 0).expect("valid rollup"));
    let vfacet_id = *vfacet.id_ref();
    let vtrack = domain::VideoTrack::try_new(Uuid7::new(), vfacet_id)
      .expect("valid track")
      .with_stream_index(Some(0))
      .with_container_track_id(Some(1))
      .with_start_pts(Some(Timestamp::new(0, tb())))
      .try_with_duration(Some(Timestamp::new(90_000, tb())))
      .expect("valid duration")
      .with_codec("h264".parse().expect("total"))
      .with_profile(Some(SmolStr::from("High")))
      .with_level(Some(41))
      .with_bit_rate(4_500_000)
      .with_nb_frames(Some(2_700))
      .with_has_b_frames(true)
      .with_closed_gop(Some(true))
      .with_bits_per_raw_sample(Some(8))
      .try_with_dimensions(mediaframe::frame::Dimensions::new(1920, 1080))
      .expect("valid dims")
      .with_has_embedded_captions(true)
      .with_is_primary(true)
      .with_auto_selected(true)
      .with_metadata({
        let mut bag = indexmap::IndexMap::new();
        bag.insert(SmolStr::from("encoder"), SmolStr::from("x264"));
        bag
      })
      .with_index_status(VideoIndexStatus::PROBED | VideoIndexStatus::SCENE_DETECTED)
      .with_index_errors(vec![ErrorInfo::new(ErrorCode::ProbeCorrupt, "glitch")])
      .with_provenance(Provenance::from_parts("ffprobe", "7.0", "", "indexer-0.1"));
    let g_vtrack = graph::VideoTrack::try_from_flat(&vfacet_id, vtrack, vec![]).expect("coherent");
    let g_video = graph::Video::try_from_flat(&media_id, vfacet, vec![g_vtrack]).expect("coherent");

    // Audio facet + one track with a segment and a speaker.
    let afacet = domain::Audio::try_new(Uuid7::new(), media_id)
      .expect("valid facet")
      .with_total_segments(1)
      .with_track_progress(IndexProgress::try_new(1, 1, 0).expect("valid rollup"));
    let afacet_id = *afacet.id_ref();
    let atrack = domain::AudioTrack::try_new(Uuid7::new(), afacet_id)
      .expect("valid track")
      .with_stream_index(Some(1))
      .with_codec("aac".parse().expect("total"))
      .with_profile("LC")
      .try_with_sample_rate(48_000)
      .expect("valid rate")
      .try_with_channels(2)
      .expect("valid channels")
      .with_bit_rate(192_000)
      .with_language(Some(Language::from_bcp47("en").expect("valid tag")))
      .with_detected_language(Some(Language::from_bcp47("en").expect("valid tag")))
      .with_primary(true)
      .with_content(Some(AudioContentKind::Speech))
      .try_with_speech_ratio(Some(0.85))
      .expect("valid ratio")
      .with_isrc("USRC17607839")
      .with_provenance(Provenance::from_parts(
        "whisper",
        "v3",
        "p-1",
        "indexer-0.1",
      ))
      .try_with_index_status(AudioIndexStatus::EXTRACTED)
      .expect("valid status");
    let atrack_id = *atrack.id_ref();
    let speaker_id = Uuid7::new();
    let es = Language::from_bcp47("es").expect("valid tag");
    let segment = domain::AudioSegment::try_new(Uuid7::new(), atrack_id, 0, span(0, 1_500))
      .expect("valid segment")
      .with_speaker_id(Some(speaker_id))
      .with_text(LocalizedText::from_src_translated(
        "hola mundo",
        "hello world",
      ))
      .with_language(Some(es))
      .try_with_words(vec![Word::try_new("hola", span(0, 500), 0.95)
        .expect("valid word")
        .with_language(Some(es))])
      .expect("words fit")
      .try_with_no_speech_prob(Some(0.05))
      .expect("valid prob")
      .with_avg_logprob(Some(-0.4))
      .with_temperature(Some(0.0))
      .with_voice_fingerprint(Some(vfp()));
    let speaker = domain::Speaker::try_new(speaker_id, atrack_id, 0, "Jane")
      .expect("valid speaker")
      .try_with_speech_duration(Some(Timestamp::new(1_500, tb())))
      .expect("valid duration")
      .with_voiceprint(vfp())
      .with_person_id(Uuid7::new());
    let g_atrack =
      graph::AudioTrack::try_from_flat(&afacet_id, atrack, vec![segment], vec![speaker])
        .expect("coherent");
    let g_audio = graph::Audio::try_from_flat(&media_id, afacet, vec![g_atrack]).expect("coherent");

    // Subtitle facet + one track with one SRT cue.
    let sfacet = domain::Subtitle::try_new(Uuid7::new(), media_id)
      .expect("valid facet")
      .with_track_progress(IndexProgress::try_new(1, 0, 0).expect("valid rollup"));
    let sfacet_id = *sfacet.id_ref();
    let strack = domain::SubtitleTrack::try_new(Uuid7::new(), sfacet_id)
      .expect("valid track")
      .with_codec("subrip".parse().expect("total"))
      .with_language(Language::from_bcp47("en").expect("valid tag"))
      .with_title("English")
      .with_primary(true)
      .with_duration(Some(Timestamp::new(90_000, tb())))
      .with_cue_count(1)
      .with_character_encoding("utf-8")
      .with_kind(SubtitleKind::FullDialogue)
      .with_first_cue(Some(Timestamp::new(0, tb())))
      .with_last_cue(Some(Timestamp::new(1_000, tb())));
    let strack_id = *strack.id_ref();
    let cue = domain::SubtitleCue::try_new(
      Uuid7::new(),
      strack_id,
      0,
      span(0, 1_000),
      LocalizedText::new(),
      SubtitleCueDetails::Srt(SrtData::new()),
    )
    .expect("valid cue");
    let g_strack =
      graph::SubtitleTrack::try_from_flat(&sfacet_id, strack, vec![cue]).expect("coherent");
    let g_subtitle =
      graph::Subtitle::try_from_flat(&media_id, sfacet, vec![g_strack]).expect("coherent");

    // Root with every optional field populated + the facet links set.
    let mut root = flat_media(media_id)
      .try_with_duration(Some(Timestamp::new(90_000, tb())))
      .expect("valid duration")
      .with_nb_streams(3)
      .with_nb_chapters(1)
      .with_error_flags(MediaErrorFlags::VIDEO_ERROR)
      .with_probe_error(Some(ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad atom")))
      .with_capture_date(Some(ts()))
      .with_device(Some(Device::new().with_make("Sony").with_model("A7M3")))
      .with_gps(Some(
        GeoLocation::try_new(48.8566, 2.3522, Some(35.0)).expect("valid gps"),
      ));
    root.set_video_id(Some(vfacet_id));
    root.set_audio_id(Some(afacet_id));
    root.set_subtitle_id(Some(sfacet_id));

    let g = graph::Media::try_from_flat(
      root,
      vec![flat_file(media_id)],
      vec![flat_chapter_fixture(media_id)],
      Some(g_video),
      Some(g_audio),
      Some(g_subtitle),
    )
    .expect("coherent tree");

    let w = wire::Media::try_from(&g).expect("encodes");
    let g2 = graph::Media::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }
}
