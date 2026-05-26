//! Per-collection `IndexModel` constructors and the `all_indexes`
//! aggregate.
//!
//! Each free function returns the canonical index set for one
//! collection. [`all_indexes`] folds them into a single
//! `Vec<(CollectionName, Vec<IndexModel>)>` so a deployer can iterate
//! and `create_indexes` against a live cluster.

use ::bson::{doc, Document};
use ::mongodb::{options::IndexOptions, IndexModel};
use derive_more::Display;

/// Canonical collection names. These match `schema.md`.
///
/// Medium-specific variants (`AudioFacets`/`VideoFacets`/…) are
/// feature-gated on the corresponding `audio` / `video` / `subtitle`
/// feature; consumers building with a subset of media features see only
/// the variants they activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
#[display("{}", self.as_str())]
pub enum CollectionName {
  Media,
  MediaFiles,
  WatchedLocations,
  Speakers,
  Persons,
  UserTags,
  SceneAnnotations,
  #[cfg(feature = "audio")]
  AudioFacets,
  #[cfg(feature = "audio")]
  AudioTracks,
  #[cfg(feature = "audio")]
  AudioSegments,
  #[cfg(feature = "video")]
  VideoFacets,
  #[cfg(feature = "video")]
  VideoTracks,
  #[cfg(feature = "video")]
  Scenes,
  #[cfg(feature = "video")]
  Keyframes,
  #[cfg(feature = "subtitle")]
  SubtitleFacets,
  #[cfg(feature = "subtitle")]
  SubtitleTracks,
  #[cfg(feature = "subtitle")]
  SubtitleCues,
  #[cfg(feature = "subtitle")]
  SubtitleTrackVttRegions,
  #[cfg(feature = "subtitle")]
  SubtitleTrackVttStyles,
  #[cfg(feature = "subtitle")]
  SubtitleTrackAssStyles,
  #[cfg(feature = "subtitle")]
  SubtitleTrackLrcMetadata,
  #[cfg(feature = "subtitle")]
  SubtitleCueLrcWords,
  #[cfg(feature = "subtitle")]
  SubtitleTrackTtmlRegions,
  #[cfg(feature = "subtitle")]
  SubtitleTrackTtmlStyles,
  #[cfg(feature = "subtitle")]
  SubtitleTrackSamiStyles,
  #[cfg(feature = "subtitle")]
  SubtitleTrackVobSubPalettes,
}

impl CollectionName {
  /// Default Mongo collection slug.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Media => "media",
      Self::MediaFiles => "media_files",
      Self::WatchedLocations => "watched_locations",
      Self::Speakers => "speakers",
      Self::Persons => "persons",
      Self::UserTags => "user_tags",
      Self::SceneAnnotations => "scene_annotations",
      #[cfg(feature = "audio")]
      Self::AudioFacets => "audio_facets",
      #[cfg(feature = "audio")]
      Self::AudioTracks => "audio_tracks",
      #[cfg(feature = "audio")]
      Self::AudioSegments => "audio_segments",
      #[cfg(feature = "video")]
      Self::VideoFacets => "video_facets",
      #[cfg(feature = "video")]
      Self::VideoTracks => "video_tracks",
      #[cfg(feature = "video")]
      Self::Scenes => "scenes",
      #[cfg(feature = "video")]
      Self::Keyframes => "keyframes",
      #[cfg(feature = "subtitle")]
      Self::SubtitleFacets => "subtitle_facets",
      #[cfg(feature = "subtitle")]
      Self::SubtitleTracks => "subtitle_tracks",
      #[cfg(feature = "subtitle")]
      Self::SubtitleCues => "subtitle_cues",
      #[cfg(feature = "subtitle")]
      Self::SubtitleTrackVttRegions => "subtitle_track_vtt_regions",
      #[cfg(feature = "subtitle")]
      Self::SubtitleTrackVttStyles => "subtitle_track_vtt_styles",
      #[cfg(feature = "subtitle")]
      Self::SubtitleTrackAssStyles => "subtitle_track_ass_styles",
      #[cfg(feature = "subtitle")]
      Self::SubtitleTrackLrcMetadata => "subtitle_track_lrc_metadata",
      #[cfg(feature = "subtitle")]
      Self::SubtitleCueLrcWords => "subtitle_cue_lrc_words",
      #[cfg(feature = "subtitle")]
      Self::SubtitleTrackTtmlRegions => "subtitle_track_ttml_regions",
      #[cfg(feature = "subtitle")]
      Self::SubtitleTrackTtmlStyles => "subtitle_track_ttml_styles",
      #[cfg(feature = "subtitle")]
      Self::SubtitleTrackSamiStyles => "subtitle_track_sami_styles",
      #[cfg(feature = "subtitle")]
      Self::SubtitleTrackVobSubPalettes => "subtitle_track_vob_sub_palettes",
    }
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn unique_on(keys: Document, name: &str) -> IndexModel {
  IndexModel::builder()
    .keys(keys)
    .options(
      IndexOptions::builder()
        .unique(true)
        .name(name.to_owned())
        .build(),
    )
    .build()
}

fn index_on(keys: Document, name: &str) -> IndexModel {
  IndexModel::builder()
    .keys(keys)
    .options(IndexOptions::builder().name(name.to_owned()).build())
    .build()
}

// ---------------------------------------------------------------------------
// Per-collection index sets
// ---------------------------------------------------------------------------

/// Indexes for the `media` collection. `_id` is implicit; we add a
/// unique index on `checksum` (the locked unique-across-`Media`
/// constraint) plus query-helper indexes on `kind`, `error_flags`, and
/// `capture_date`.
pub fn media_indexes() -> Vec<IndexModel> {
  vec![
    unique_on(doc! { "checksum": 1 }, "media_checksum_unique"),
    index_on(doc! { "kind": 1 }, "media_kind"),
    index_on(doc! { "error_flags": 1 }, "media_error_flags"),
    index_on(doc! { "capture_date": 1 }, "media_capture_date"),
  ]
}

/// `media_file` — one physical copy per document. FK indexes on
/// `media_id` (drives the `Media.files` reverse lookup) and
/// `watched_location_id` (the discovering watch / WL-deletion cascade).
pub fn media_file_indexes() -> Vec<IndexModel> {
  vec![
    index_on(doc! { "media_id": 1 }, "media_file_media_id"),
    index_on(
      doc! { "watched_location_id": 1 },
      "media_file_watched_location_id",
    ),
  ]
}

/// `watched_locations` — primarily a config table. Index on the
/// volume UUID for "find watch by volume" queries, and on `enabled`
/// for the monitor-startup sweep.
pub fn watched_location_indexes() -> Vec<IndexModel> {
  vec![
    index_on(doc! { "volume": 1 }, "watched_volume"),
    index_on(doc! { "enabled": 1 }, "watched_enabled"),
  ]
}

/// `speakers` — FK index on `audio_track_id` (`AudioTrack.id`) and on the
/// optional `person_id` FK (the `persons` collection back-reference).
pub fn speaker_indexes() -> Vec<IndexModel> {
  vec![
    index_on(doc! { "audio_track_id": 1 }, "speakers_audio_track_id"),
    index_on(doc! { "person_id": 1 }, "speakers_person_id"),
  ]
}

/// `persons` — cross-track / cross-modality identity anchor. The `_id`
/// implicit index drives FK lookups from `speakers.person_id`; we add a
/// compound `(voiceprint.provenance.model_name,
/// voiceprint.provenance.model_version)` index so "find all Persons
/// whose canonical voiceprint came from this model" stays a key-scan
/// (the aggregated centroid is only meaningful when contributing
/// `Speaker` voiceprints share one `(model, version)` pair).
pub fn person_indexes() -> Vec<IndexModel> {
  vec![index_on(
    doc! {
      "voiceprint.provenance.model_name": 1,
      "voiceprint.provenance.model_version": 1,
    },
    "persons_voiceprint_model",
  )]
}

/// `user_tags` — case-insensitive lookup is a projection concern;
/// index on `name` for typeahead.
pub fn user_tag_indexes() -> Vec<IndexModel> {
  vec![index_on(doc! { "name": 1 }, "user_tags_name")]
}

/// `scene_annotations` — FK index on `scene_id`, plus `favorite` /
/// `rating` for filter queries.
pub fn scene_annotation_indexes() -> Vec<IndexModel> {
  vec![
    unique_on(doc! { "scene_id": 1 }, "scene_annotations_scene_id_unique"),
    index_on(doc! { "favorite": 1 }, "scene_annotations_favorite"),
    index_on(doc! { "rating": 1 }, "scene_annotations_rating"),
  ]
}

/// `audio_facets` — 1:1 with `Media`. The `media_id` FK is unique
/// (locked schema: one Audio facet per Media).
#[cfg(feature = "audio")]
pub fn audio_facet_indexes() -> Vec<IndexModel> {
  vec![unique_on(
    doc! { "media_id": 1 },
    "audio_facets_media_id_unique",
  )]
}

/// `audio_tracks` — FK on `audio_id`, plus `is_primary` / `content` for
/// track-selection queries.
#[cfg(feature = "audio")]
pub fn audio_track_indexes() -> Vec<IndexModel> {
  vec![
    index_on(doc! { "audio_id": 1 }, "audio_tracks_audio_id"),
    index_on(doc! { "is_primary": 1 }, "audio_tracks_primary"),
    index_on(doc! { "content": 1 }, "audio_tracks_content"),
    index_on(doc! { "language": 1 }, "audio_tracks_language"),
  ]
}

/// `audio_segments` — FK on `audio_track_id` + composite
/// `(audio_track_id, index)` for ordered enumeration.
#[cfg(feature = "audio")]
pub fn audio_segment_indexes() -> Vec<IndexModel> {
  vec![
    index_on(doc! { "audio_track_id": 1 }, "audio_segments_audio_track_id"),
    unique_on(
      doc! { "audio_track_id": 1, "index": 1 },
      "audio_segments_audio_track_id_index_unique",
    ),
    index_on(doc! { "speaker_id": 1 }, "audio_segments_speaker_id"),
  ]
}

/// `video_facets` — 1:1 with `Media`. The `media_id` FK is unique
/// (locked schema: one Video facet per Media).
#[cfg(feature = "video")]
pub fn video_facet_indexes() -> Vec<IndexModel> {
  vec![unique_on(
    doc! { "media_id": 1 },
    "video_facets_media_id_unique",
  )]
}

/// `video_tracks` — FK on `video_id` + selection signals.
#[cfg(feature = "video")]
pub fn video_track_indexes() -> Vec<IndexModel> {
  vec![
    index_on(doc! { "video_id": 1 }, "video_tracks_video_id"),
    index_on(doc! { "is_primary": 1 }, "video_tracks_primary"),
  ]
}

/// `scenes` — FK on `video_track_id` (`VideoTrack.id`); composite
/// `(video_track_id, index)` for ordered enumeration.
#[cfg(feature = "video")]
pub fn scene_indexes() -> Vec<IndexModel> {
  vec![
    index_on(doc! { "video_track_id": 1 }, "scenes_video_track_id"),
    unique_on(
      doc! { "video_track_id": 1, "index": 1 },
      "scenes_video_track_id_index_unique",
    ),
  ]
}

/// `keyframes` — FK on `scene_id` (`Scene.id`).
#[cfg(feature = "video")]
pub fn keyframe_indexes() -> Vec<IndexModel> {
  vec![index_on(doc! { "scene_id": 1 }, "keyframes_scene_id")]
}

/// `subtitle_facets` — 1:1 with `Media`. The `media_id` FK is unique
/// (locked schema: one Subtitle facet per Media).
#[cfg(feature = "subtitle")]
pub fn subtitle_facet_indexes() -> Vec<IndexModel> {
  vec![unique_on(
    doc! { "media_id": 1 },
    "subtitle_facets_media_id_unique",
  )]
}

/// `subtitle_tracks` — FK on `subtitle_id` + selection signals.
#[cfg(feature = "subtitle")]
pub fn subtitle_track_indexes() -> Vec<IndexModel> {
  vec![
    index_on(doc! { "subtitle_id": 1 }, "subtitle_tracks_subtitle_id"),
    index_on(doc! { "is_primary": 1 }, "subtitle_tracks_primary"),
    index_on(doc! { "language": 1 }, "subtitle_tracks_language"),
  ]
}

/// `subtitle_cues` — FK on `subtitle_track_id` + composite UNIQUE on
/// `(subtitle_track_id, ordinal)` (the bson writer in `subtitle.rs`
/// emits `ordinal`, not `index`).
#[cfg(feature = "subtitle")]
pub fn subtitle_cue_indexes() -> Vec<IndexModel> {
  vec![
    index_on(doc! { "subtitle_track_id": 1 }, "subtitle_cues_subtitle_track_id"),
    unique_on(
      doc! { "subtitle_track_id": 1, "ordinal": 1 },
      "subtitle_cues_subtitle_track_id_ordinal_unique",
    ),
  ]
}

/// `subtitle_track_vtt_regions` — per-track WebVTT `REGION` blocks.
/// FK on `subtitle_track_id` + UNIQUE composite on
/// `(subtitle_track_id, name)` (region names are unique within a track).
#[cfg(feature = "subtitle")]
pub fn subtitle_track_vtt_region_indexes() -> Vec<IndexModel> {
  vec![
    index_on(
      doc! { "subtitle_track_id": 1 },
      "subtitle_track_vtt_regions_subtitle_track_id",
    ),
    unique_on(
      doc! { "subtitle_track_id": 1, "name": 1 },
      "subtitle_track_vtt_regions_subtitle_track_id_name_unique",
    ),
  ]
}

/// `subtitle_track_vtt_styles` — per-track WebVTT `STYLE` blocks
/// (ordered CSS chunks). FK on `subtitle_track_id` + UNIQUE composite
/// on `(subtitle_track_id, ordinal)` (one block per ordinal slot).
#[cfg(feature = "subtitle")]
pub fn subtitle_track_vtt_style_indexes() -> Vec<IndexModel> {
  vec![
    index_on(
      doc! { "subtitle_track_id": 1 },
      "subtitle_track_vtt_styles_subtitle_track_id",
    ),
    unique_on(
      doc! { "subtitle_track_id": 1, "ordinal": 1 },
      "subtitle_track_vtt_styles_subtitle_track_id_ordinal_unique",
    ),
  ]
}

/// `subtitle_track_ass_styles` — per-track ASS `[V4+ Styles]` rows. FK
/// on `subtitle_track_id` + UNIQUE composite on
/// `(subtitle_track_id, name)` (style names are unique within a track).
#[cfg(feature = "subtitle")]
pub fn subtitle_track_ass_style_indexes() -> Vec<IndexModel> {
  vec![
    index_on(
      doc! { "subtitle_track_id": 1 },
      "subtitle_track_ass_styles_subtitle_track_id",
    ),
    unique_on(
      doc! { "subtitle_track_id": 1, "name": 1 },
      "subtitle_track_ass_styles_subtitle_track_id_name_unique",
    ),
  ]
}

/// `subtitle_track_lrc_metadata` — per-track LRC `[ti]/[ar]/…` header
/// block. The metadata _is_ the collection-of-metadata-fields for that
/// track (1:1 with `SubtitleTrack`), so the bson writer stores
/// `subtitle_track_id` as `_id`; an explicit UNIQUE on `_id` is
/// implicit. No extra indexes needed.
#[cfg(feature = "subtitle")]
pub fn subtitle_track_lrc_metadata_indexes() -> Vec<IndexModel> {
  vec![]
}

/// `subtitle_cue_lrc_words` — per-cue word-timing rows for Enhanced
/// LRC. FK on `subtitle_cue_id` + UNIQUE composite on
/// `(subtitle_cue_id, ordinal)` (one word per ordinal slot).
#[cfg(feature = "subtitle")]
pub fn subtitle_cue_lrc_word_indexes() -> Vec<IndexModel> {
  vec![
    index_on(
      doc! { "subtitle_cue_id": 1 },
      "subtitle_cue_lrc_words_subtitle_cue_id",
    ),
    unique_on(
      doc! { "subtitle_cue_id": 1, "ordinal": 1 },
      "subtitle_cue_lrc_words_subtitle_cue_id_ordinal_unique",
    ),
  ]
}

/// `subtitle_track_ttml_regions` — per-track TTML `<region>` blocks.
/// FK on `subtitle_track_id` + UNIQUE composite on `(subtitle_track_id,
/// xml_id)` (TTML xml-ids are unique within a track).
#[cfg(feature = "subtitle")]
pub fn subtitle_track_ttml_region_indexes() -> Vec<IndexModel> {
  vec![
    index_on(
      doc! { "subtitle_track_id": 1 },
      "subtitle_track_ttml_regions_subtitle_track_id",
    ),
    unique_on(
      doc! { "subtitle_track_id": 1, "xml_id": 1 },
      "subtitle_track_ttml_regions_subtitle_track_id_xml_id_unique",
    ),
  ]
}

/// `subtitle_track_ttml_styles` — per-track TTML `<style>` blocks.
/// Same shape as TTML regions.
#[cfg(feature = "subtitle")]
pub fn subtitle_track_ttml_style_indexes() -> Vec<IndexModel> {
  vec![
    index_on(
      doc! { "subtitle_track_id": 1 },
      "subtitle_track_ttml_styles_subtitle_track_id",
    ),
    unique_on(
      doc! { "subtitle_track_id": 1, "xml_id": 1 },
      "subtitle_track_ttml_styles_subtitle_track_id_xml_id_unique",
    ),
  ]
}

/// `subtitle_track_sami_styles` — per-track SAMI `<STYLE>` classes.
/// FK on `subtitle_track_id` + UNIQUE on `(subtitle_track_id,
/// class_name)`.
#[cfg(feature = "subtitle")]
pub fn subtitle_track_sami_style_indexes() -> Vec<IndexModel> {
  vec![
    index_on(
      doc! { "subtitle_track_id": 1 },
      "subtitle_track_sami_styles_subtitle_track_id",
    ),
    unique_on(
      doc! { "subtitle_track_id": 1, "class_name": 1 },
      "subtitle_track_sami_styles_subtitle_track_id_class_name_unique",
    ),
  ]
}

/// `subtitle_track_vob_sub_palettes` — per-track DVD VobSub palette
/// rows. FK on `subtitle_track_id`.
#[cfg(feature = "subtitle")]
pub fn subtitle_track_vob_sub_palette_indexes() -> Vec<IndexModel> {
  vec![index_on(
    doc! { "subtitle_track_id": 1 },
    "subtitle_track_vob_sub_palettes_subtitle_track_id",
  )]
}

/// Every collection + its canonical index set, in one call. Iterate
/// this list in the deployer to create the full schema.
///
/// The result only contains entries for media features the consumer
/// has enabled — a build with `--no-default-features --features
/// mongodb,video` excludes the audio / subtitle collections.
pub fn all_indexes() -> Vec<(CollectionName, Vec<IndexModel>)> {
  // `mut` only when at least one medium feature contributes an
  // `extend` call below; suppress otherwise so a no-medium build stays
  // warning-clean.
  #[allow(unused_mut)]
  let mut v: Vec<(CollectionName, Vec<IndexModel>)> = vec![
    (CollectionName::Media, media_indexes()),
    (CollectionName::MediaFiles, media_file_indexes()),
    (CollectionName::WatchedLocations, watched_location_indexes()),
    (CollectionName::Speakers, speaker_indexes()),
    (CollectionName::Persons, person_indexes()),
    (CollectionName::UserTags, user_tag_indexes()),
    (CollectionName::SceneAnnotations, scene_annotation_indexes()),
  ];
  #[cfg(feature = "audio")]
  v.extend([
    (CollectionName::AudioFacets, audio_facet_indexes()),
    (CollectionName::AudioTracks, audio_track_indexes()),
    (CollectionName::AudioSegments, audio_segment_indexes()),
  ]);
  #[cfg(feature = "video")]
  v.extend([
    (CollectionName::VideoFacets, video_facet_indexes()),
    (CollectionName::VideoTracks, video_track_indexes()),
    (CollectionName::Scenes, scene_indexes()),
    (CollectionName::Keyframes, keyframe_indexes()),
  ]);
  #[cfg(feature = "subtitle")]
  v.extend([
    (CollectionName::SubtitleFacets, subtitle_facet_indexes()),
    (CollectionName::SubtitleTracks, subtitle_track_indexes()),
    (CollectionName::SubtitleCues, subtitle_cue_indexes()),
    (
      CollectionName::SubtitleTrackVttRegions,
      subtitle_track_vtt_region_indexes(),
    ),
    (
      CollectionName::SubtitleTrackVttStyles,
      subtitle_track_vtt_style_indexes(),
    ),
    (
      CollectionName::SubtitleTrackAssStyles,
      subtitle_track_ass_style_indexes(),
    ),
    (
      CollectionName::SubtitleTrackLrcMetadata,
      subtitle_track_lrc_metadata_indexes(),
    ),
    (
      CollectionName::SubtitleCueLrcWords,
      subtitle_cue_lrc_word_indexes(),
    ),
    (
      CollectionName::SubtitleTrackTtmlRegions,
      subtitle_track_ttml_region_indexes(),
    ),
    (
      CollectionName::SubtitleTrackTtmlStyles,
      subtitle_track_ttml_style_indexes(),
    ),
    (
      CollectionName::SubtitleTrackSamiStyles,
      subtitle_track_sami_style_indexes(),
    ),
    (
      CollectionName::SubtitleTrackVobSubPalettes,
      subtitle_track_vob_sub_palette_indexes(),
    ),
  ]);
  v
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn all_indexes_covers_every_collection() {
    let v = all_indexes();
    // 7 always-on (Media + MediaFiles + WatchedLocations + Speakers
    // + Persons + UserTags + SceneAnnotations), plus 3 audio,
    // 4 video, 12 subtitle when those features are on.
    let mut expected = 7usize;
    if cfg!(feature = "audio") {
      expected += 3;
    }
    if cfg!(feature = "video") {
      expected += 4;
    }
    if cfg!(feature = "subtitle") {
      expected += 12;
    }
    assert_eq!(v.len(), expected);
    // No collection appears twice.
    let mut names: Vec<_> = v.iter().map(|(c, _)| c.as_str()).collect();
    names.sort();
    let mut dedup = names.clone();
    dedup.dedup();
    assert_eq!(names, dedup);
  }

  #[test]
  fn media_unique_checksum_present() {
    let idx = media_indexes();
    let names: Vec<_> = idx
      .iter()
      .map(|m| {
        m.options
          .as_ref()
          .and_then(|o| o.name.clone())
          .unwrap_or_default()
      })
      .collect();
    assert!(names.iter().any(|n| n == "media_checksum_unique"));
  }
}
