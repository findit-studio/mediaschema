//! SQLite row mapping. `Uuid7` rides as a 16-byte `BLOB`, `FileChecksum`
//! as a 32-byte `BLOB`. Nested VOs (`Provenance`, capture `Device`, etc.)
//! are stored as `TEXT` containing JSON.
//!
//! See the module-level [`super`] doc for the cross-backend mapping
//! conventions and current coverage scope.

pub mod audio;
pub mod leaves;
pub mod media;
pub mod media_file;
pub mod subtitle;
pub mod video;

pub use audio::{
  SqliteAudioRow, SqliteAudioSegmentRow, SqliteAudioSegmentWordRow, SqliteAudioTrackIndexErrorRow,
  SqliteAudioTrackRow,
};
pub use leaves::{
  SqliteSceneAnnotationRow, SqliteSpeakerRow, SqliteUserTagRow, SqliteWatchedLocationRow,
};
pub use media::SqliteMediaRow;
pub use media_file::SqliteMediaFileRow;
pub use subtitle::{
  SqliteSubtitleCueRow, SqliteSubtitleRow, SqliteSubtitleTrackIndexErrorRow, SqliteSubtitleTrackRow,
};
pub use video::{
  SqliteKeyframeActionRow, SqliteKeyframeBarcodeRow, SqliteKeyframeBodyPose3DJointRow,
  SqliteKeyframeBodyPose3DRow, SqliteKeyframeBodyPoseJointRow, SqliteKeyframeBodyPoseRow,
  SqliteKeyframeClassificationRow, SqliteKeyframeColorRow, SqliteKeyframeDocumentSegmentRow,
  SqliteKeyframeFaceLandmarkPointRow, SqliteKeyframeFaceLandmarkRegionRow,
  SqliteKeyframeFaceLandmarksRow, SqliteKeyframeFaceRow, SqliteKeyframeHandPoseRow,
  SqliteKeyframeMaskRow, SqliteKeyframeObjectRow, SqliteKeyframeRow, SqliteKeyframeRows,
  SqliteKeyframeSaliencyRow, SqliteKeyframeSubjectRow, SqliteKeyframeTextDetectionRow,
  SqliteKeyframeVlmLabelRow, SqliteSceneRow, SqliteVideoRow, SqliteVideoTrackIndexErrorRow,
  SqliteVideoTrackRow,
};

/// Canonical SQLite DDL for the mediaschema tables this revision maps.
///
/// Sourced from [`schema.sql`](./schema.sql) so the DDL is text-grep-able
/// alongside the row structs.
pub const SCHEMA_SQL: &str = include_str!("schema.sql");

/// Initial migration mirror of [`SCHEMA_SQL`], also embedded as a static
/// string so consumers can wire it into their migration runner.
pub const MIGRATION_0001_INIT: &str = include_str!("migrations/0001_init.sql");
