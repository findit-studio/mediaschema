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
  video_track_from_rows, MySqlKeyframeActionRow, MySqlKeyframeBarcodeRow,
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
  fn data_migration_mirror_matches_schema() {
    assert_eq!(SCHEMA_SQL, MIGRATION_0001_INIT);
  }
}
