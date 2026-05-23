//! MySQL row mapping. `Uuid7` rides as a `BINARY(16)`, `FileChecksum` as
//! `BINARY(32)`. Nested VOs ride as MySQL `JSON` columns; the row struct
//! reads them as `String` (sqlx queries should cast — MySQL `JSON` auto-
//! coerces to text on retrieval, so no explicit cast is needed). Wall-
//! clock columns are `BIGINT` milliseconds-since-epoch.
//!
//! See the module-level [`super`] doc for the cross-backend mapping
//! conventions and current coverage scope.

pub mod audio;
pub mod leaves;
pub mod media;
pub mod media_file;
pub mod person;
pub mod subtitle;
pub mod video;

pub use audio::{
  MySqlAudioRow, MySqlAudioSegmentRow, MySqlAudioSegmentWordRow, MySqlAudioTrackIndexErrorRow,
  MySqlAudioTrackRow,
};
pub use leaves::{
  MySqlSceneAnnotationRow, MySqlSpeakerRow, MySqlUserTagRow, MySqlWatchedLocationRow,
};
pub use media::MySqlMediaRow;
pub use media_file::MySqlMediaFileRow;
pub use person::MySqlPersonRow;
pub use subtitle::{
  MySqlSubtitleCueRow, MySqlSubtitleRow, MySqlSubtitleTrackIndexErrorRow, MySqlSubtitleTrackRow,
};
pub use video::{
  MySqlKeyframeActionRow, MySqlKeyframeBarcodeRow, MySqlKeyframeBodyPose3DJointRow,
  MySqlKeyframeBodyPose3DRow, MySqlKeyframeBodyPoseJointRow, MySqlKeyframeBodyPoseRow,
  MySqlKeyframeClassificationRow, MySqlKeyframeColorRow, MySqlKeyframeDocumentSegmentRow,
  MySqlKeyframeFaceLandmarkPointRow, MySqlKeyframeFaceLandmarkRegionRow,
  MySqlKeyframeFaceLandmarksRow, MySqlKeyframeFaceRow, MySqlKeyframeHandPoseRow,
  MySqlKeyframeMaskRow, MySqlKeyframeObjectRow, MySqlKeyframeRow, MySqlKeyframeRows,
  MySqlKeyframeSaliencyRow, MySqlKeyframeSubjectRow, MySqlKeyframeTextDetectionRow,
  MySqlKeyframeVlmLabelRow, MySqlSceneRow, MySqlVideoRow, MySqlVideoTrackIndexErrorRow,
  MySqlVideoTrackRow,
};

/// Canonical MySQL DDL for the mediaschema tables this revision maps.
pub const SCHEMA_SQL: &str = include_str!("schema.sql");

/// Initial migration mirror of [`SCHEMA_SQL`].
pub const MIGRATION_0001_INIT: &str = include_str!("migrations/0001_init.sql");
