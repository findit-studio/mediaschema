//! MySQL row mapping. `Uuid7` rides as a `BINARY(16)`, `FileChecksum` as
//! `BINARY(32)`. Nested VOs ride as MySQL `JSON` columns; the row struct
//! reads them as `String` (sqlx queries should cast — MySQL `JSON` auto-
//! coerces to text on retrieval, so no explicit cast is needed). Wall-
//! clock columns are `BIGINT` milliseconds-since-epoch.
//!
//! See the module-level [`super`] doc for the cross-backend mapping
//! conventions and current coverage scope.

#[cfg(feature = "audio")]
#[cfg_attr(docsrs, doc(cfg(feature = "audio")))]
pub mod audio;
// `Attachment` is container-level (like `chapter` / `media`) — ungated.
pub mod attachment;
pub mod chapter;
// `Data` is container-level (like `chapter` / `media`) — ungated.
pub mod data;
pub mod leaves;
pub mod media;
pub mod media_file;
pub mod person;
#[cfg(feature = "subtitle")]
#[cfg_attr(docsrs, doc(cfg(feature = "subtitle")))]
pub mod subtitle;
#[cfg(feature = "video")]
#[cfg_attr(docsrs, doc(cfg(feature = "video")))]
pub mod video;

pub use attachment::{
  attachment_track_from_rows, MySqlAttachmentRow, MySqlAttachmentTrackIndexErrorRow,
  MySqlAttachmentTrackMetadataRow, MySqlAttachmentTrackRow,
};
#[cfg(feature = "audio")]
pub use audio::{
  audio_track_from_rows, sound_event_from_row, MySqlAudioRow, MySqlAudioSegmentRow,
  MySqlAudioSegmentWordRow, MySqlAudioTrackIndexErrorRow, MySqlAudioTrackMetadataRow,
  MySqlAudioTrackRow, MySqlSoundEventRow,
};
pub use chapter::{chapter_from_rows, MySqlChapterMetadataRow, MySqlChapterRow};
pub use data::{
  data_track_from_rows, MySqlDataRow, MySqlDataTrackIndexErrorRow, MySqlDataTrackMetadataRow,
  MySqlDataTrackRow,
};
pub use leaves::{
  MySqlSceneAnnotationRow, MySqlSpeakerRow, MySqlUserTagRow, MySqlWatchedLocationRow,
};
pub use media::MySqlMediaRow;
pub use media_file::MySqlMediaFileRow;
pub use person::MySqlPersonRow;
#[cfg(feature = "subtitle")]
pub use subtitle::{
  subtitle_track_from_rows, MySqlSubtitleCueAssRow, MySqlSubtitleCueBaseRow,
  MySqlSubtitleCueLrcRow, MySqlSubtitleCueLrcWordRow, MySqlSubtitleCueVttRow, MySqlSubtitleRow,
  MySqlSubtitleTrackAssStyleRow, MySqlSubtitleTrackIndexErrorRow, MySqlSubtitleTrackLrcMetadataRow,
  MySqlSubtitleTrackMetadataRow, MySqlSubtitleTrackRow, MySqlSubtitleTrackVttRegionRow,
  MySqlSubtitleTrackVttStyleRow,
};
#[cfg(feature = "video")]
pub use video::{
  cover_keyframe_from_rows, cover_keyframe_rows_from, video_track_from_rows, MySqlCoverKeyframeRow,
  MySqlCoverKeyframeRows, MySqlKeyframeActionRow, MySqlKeyframeBarcodeRow,
  MySqlKeyframeBodyPose3DJointRow, MySqlKeyframeBodyPose3DRow, MySqlKeyframeBodyPoseJointRow,
  MySqlKeyframeBodyPoseRow, MySqlKeyframeClassificationRow, MySqlKeyframeColorRow,
  MySqlKeyframeDocumentSegmentRow, MySqlKeyframeFaceLandmarkPointRow,
  MySqlKeyframeFaceLandmarkRegionRow, MySqlKeyframeFaceLandmarksRow, MySqlKeyframeFaceRow,
  MySqlKeyframeHandPoseRow, MySqlKeyframeMaskRow, MySqlKeyframeObjectRow, MySqlKeyframeRow,
  MySqlKeyframeRows, MySqlKeyframeSaliencyRow, MySqlKeyframeSubjectRow,
  MySqlKeyframeTextDetectionRow, MySqlKeyframeVlmLabelRow, MySqlSceneRow, MySqlVideoRow,
  MySqlVideoTrackIndexErrorRow, MySqlVideoTrackMetadataRow, MySqlVideoTrackRow,
};

/// Canonical MySQL DDL for the mediaschema tables this revision maps.
pub const SCHEMA_SQL: &str = include_str!("schema.sql");

/// Initial migration mirror of [`SCHEMA_SQL`].
pub const MIGRATION_0001_INIT: &str = include_str!("migrations/0001_init.sql");

#[cfg(all(test, feature = "video"))]
mod schema_tests {
  use super::{MIGRATION_0001_INIT, SCHEMA_SQL};

  #[test]
  fn schema_has_thumbnail_table_and_keyframe_fk() {
    // The thumbnail FK target exists with its storage discriminator.
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS thumbnail"));
    assert!(SCHEMA_SQL.contains("kind      VARCHAR(32)  NOT NULL"));
    // Keyframe references the thumbnail by FK.
    assert!(SCHEMA_SQL.contains("thumbnail_id               BINARY(16)   NOT NULL"));
    assert!(SCHEMA_SQL.contains("idx_keyframe_thumbnail_id"));
    // The thumbnail table is declared BEFORE keyframe (FK target first).
    let thumb = SCHEMA_SQL
      .find("CREATE TABLE IF NOT EXISTS thumbnail")
      .expect("thumbnail table present");
    let keyframe = SCHEMA_SQL
      .find("CREATE TABLE IF NOT EXISTS keyframe (")
      .expect("keyframe table present");
    assert!(thumb < keyframe, "thumbnail must precede keyframe");
  }

  #[test]
  fn schema_has_cover_keyframe_and_role_and_nullable_scene_id() {
    // The cover keyframe is a distinct table parented by video_id.
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS cover_keyframe ("));
    assert!(SCHEMA_SQL.contains("idx_cover_keyframe_video_id"));
    assert!(SCHEMA_SQL.contains("idx_cover_keyframe_thumbnail_id"));
    // The video facet gains the cover FK.
    assert!(SCHEMA_SQL.contains("cover_keyframe_id      BINARY(16)"));
    // The scene keyframe gains a role column and its scene_id is now
    // nullable (the cover decision-b override).
    assert!(SCHEMA_SQL.contains("role                       VARCHAR(64)  NOT NULL DEFAULT 'scene'"));
    assert!(!SCHEMA_SQL.contains("scene_id                     BINARY(16)   NOT NULL"));
  }

  #[test]
  fn migration_mirror_matches_schema() {
    assert_eq!(SCHEMA_SQL, MIGRATION_0001_INIT);
  }
}

#[cfg(test)]
mod data_schema_tests {
  use super::{MIGRATION_0001_INIT, SCHEMA_SQL};

  #[test]
  fn schema_has_data_cluster_tables() {
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS data ("));
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS data_track ("));
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS data_track_metadata ("));
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS data_track_index_error ("));
    assert!(SCHEMA_SQL.contains("idx_data_track_data_id"));
  }

  #[test]
  fn schema_has_attachment_cluster_tables() {
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS attachment ("));
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS attachment_track ("));
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS attachment_track_metadata ("));
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS attachment_track_index_error ("));
    assert!(SCHEMA_SQL.contains("idx_attachment_track_attachment_id"));
    assert!(SCHEMA_SQL.contains("blob_uri"));
  }

  #[test]
  fn media_wires_data_and_attachment_facet_fks() {
    assert!(SCHEMA_SQL.contains("idx_media_data"));
    assert!(SCHEMA_SQL.contains("idx_media_attachment"));
  }

  #[test]
  fn data_migration_mirror_matches_schema() {
    assert_eq!(SCHEMA_SQL, MIGRATION_0001_INIT);
  }
}
