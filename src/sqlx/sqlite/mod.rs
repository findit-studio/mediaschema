//! SQLite row mapping. `Uuid7` rides as a 16-byte `BLOB`, `FileChecksum`
//! as a 32-byte `BLOB`. Nested VOs (`Provenance`, capture `Device`, etc.)
//! are stored as `TEXT` containing JSON.
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
pub mod thumbnail;
#[cfg(feature = "video")]
#[cfg_attr(docsrs, doc(cfg(feature = "video")))]
pub mod video;

pub use attachment::{
  attachment_track_from_rows, SqliteAttachmentRow, SqliteAttachmentTrackIndexErrorRow,
  SqliteAttachmentTrackMetadataRow, SqliteAttachmentTrackRow,
};
#[cfg(feature = "audio")]
pub use audio::{
  audio_track_from_rows, sound_event_from_row, SqliteAudioRow, SqliteAudioSegmentRow,
  SqliteAudioSegmentWordRow, SqliteAudioTrackIndexErrorRow, SqliteAudioTrackMetadataRow,
  SqliteAudioTrackRow, SqliteSoundEventRow,
};
pub use chapter::{chapter_from_rows, SqliteChapterMetadataRow, SqliteChapterRow};
pub use data::{
  data_track_from_rows, SqliteDataRow, SqliteDataTrackIndexErrorRow, SqliteDataTrackMetadataRow,
  SqliteDataTrackRow,
};
pub use leaves::{
  SqliteSceneAnnotationRow, SqliteSpeakerRow, SqliteUserTagRow, SqliteWatchedLocationRow,
};
pub use media::SqliteMediaRow;
pub use media_file::SqliteMediaFileRow;
pub use person::SqlitePersonRow;
#[cfg(feature = "subtitle")]
pub use subtitle::{
  subtitle_track_from_rows, SqliteSubtitleCueAssRow, SqliteSubtitleCueBaseRow,
  SqliteSubtitleCueLrcRow, SqliteSubtitleCueLrcWordRow, SqliteSubtitleCueVttRow, SqliteSubtitleRow,
  SqliteSubtitleTrackAssStyleRow, SqliteSubtitleTrackIndexErrorRow,
  SqliteSubtitleTrackLrcMetadataRow, SqliteSubtitleTrackMetadataRow, SqliteSubtitleTrackRow,
  SqliteSubtitleTrackVttRegionRow, SqliteSubtitleTrackVttStyleRow,
};
#[cfg(feature = "video")]
pub use thumbnail::SqliteThumbnailRow;
#[cfg(feature = "video")]
pub use video::{
  cover_keyframe_from_rows, cover_keyframe_rows_from, video_track_from_rows,
  SqliteCoverKeyframeRow, SqliteCoverKeyframeRows, SqliteKeyframeActionRow,
  SqliteKeyframeBarcodeRow, SqliteKeyframeBodyPose3DJointRow, SqliteKeyframeBodyPose3DRow,
  SqliteKeyframeBodyPoseJointRow, SqliteKeyframeBodyPoseRow, SqliteKeyframeClassificationRow,
  SqliteKeyframeColorRow, SqliteKeyframeDocumentSegmentRow, SqliteKeyframeFaceLandmarkPointRow,
  SqliteKeyframeFaceLandmarkRegionRow, SqliteKeyframeFaceLandmarksRow, SqliteKeyframeFaceRow,
  SqliteKeyframeHandPoseRow, SqliteKeyframeMaskRow, SqliteKeyframeObjectRow, SqliteKeyframeRow,
  SqliteKeyframeRows, SqliteKeyframeSaliencyRow, SqliteKeyframeSubjectRow,
  SqliteKeyframeTextDetectionRow, SqliteKeyframeVlmLabelRow, SqliteSceneRow, SqliteVideoRow,
  SqliteVideoTrackIndexErrorRow, SqliteVideoTrackMetadataRow, SqliteVideoTrackRow,
};

/// Canonical SQLite DDL for the mediaschema tables this revision maps.
///
/// Sourced from [`schema.sql`](./schema.sql) so the DDL is text-grep-able
/// alongside the row structs.
pub const SCHEMA_SQL: &str = include_str!("schema.sql");

/// Initial migration mirror of [`SCHEMA_SQL`], also embedded as a static
/// string so consumers can wire it into their migration runner.
pub const MIGRATION_0001_INIT: &str = include_str!("migrations/0001_init.sql");

#[cfg(all(test, feature = "video"))]
mod schema_tests {
  use super::{MIGRATION_0001_INIT, SCHEMA_SQL};

  #[test]
  fn schema_has_thumbnail_table_and_keyframe_fk() {
    // The thumbnail FK target exists with its storage discriminator.
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS thumbnail"));
    assert!(SCHEMA_SQL.contains("kind      TEXT    NOT NULL"));
    // Keyframe references the thumbnail by FK and no longer inlines the
    // image bytes / mime.
    assert!(SCHEMA_SQL.contains("thumbnail_id               BLOB    NOT NULL"));
    assert!(SCHEMA_SQL.contains("idx_keyframe_thumbnail_id"));
    assert!(!SCHEMA_SQL.contains("data                       BLOB    NOT NULL"));
    assert!(!SCHEMA_SQL.contains("mime                       TEXT    NOT NULL"));
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
    assert!(SCHEMA_SQL.contains("cover_keyframe_id      BLOB"));
    // The scene keyframe gains a role column and its scene_id is now
    // nullable (the cover decision-b override).
    assert!(SCHEMA_SQL.contains("role                       TEXT    NOT NULL DEFAULT 'scene'"));
    assert!(!SCHEMA_SQL.contains("scene_id                     BLOB    NOT NULL"));
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
    // Reserved blob columns are declared (always NULL in v1).
    assert!(SCHEMA_SQL.contains("blob_uri"));
  }

  #[test]
  fn data_migration_mirror_matches_schema() {
    assert_eq!(SCHEMA_SQL, MIGRATION_0001_INIT);
  }
}
