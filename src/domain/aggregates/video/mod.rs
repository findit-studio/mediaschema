//! Video media-kind aggregate cluster — `Video` facet + `VideoTrack` +
//! per-stream `Scene` + `Keyframe`, plus the apple-vision / VLM /
//! colorthief detection VOs that hang off `Keyframe`.
//!
//! Locked specs: `schema/video.md` r8, `schema/video_track.md` r7,
//! `schema/scene.md` r6+r7-reopen, `schema/keyframe.md` r15+r16-reopen.
//!
//! **Mediaframe placeholders.** The frame/pixel/colour vocabulary that
//! the locked specs route to `::mediaframe` is not yet an external dep
//! of this crate; every such field is tagged `TODO(mediaframe)` and
//! carries a placeholder type whose shape mirrors the wire (or, where
//! the wire is a bare numeric id, a `u32`/`u64`). Once mediaframe ships
//! the batched post-`0.1.0` minor, those placeholders flip in one
//! mechanical sweep.

pub mod detections;
pub mod facet;
pub mod keyframe;
pub mod scene;
pub mod track;

pub use detections::{
  ActionDetection, Aesthetics, AnimalAnalysis, BarcodeDetection, BodyPose3DDetection,
  BodyPose3DHeightEstimation, BodyPose3DJoint, BodyPoseDetection, BodyPoseJoint, BoundingBox,
  Detection, DocumentSegment, DominantColor, FaceDetection, FaceLandmarkRegion,
  FaceLandmarksDetection, HandChirality, HandPoseDetection, HorizonInfo, HumanAnalysis,
  ObjectDetection, PersonInstanceMaskDetection, PersonSegmentationMask, SaliencyRegion,
  SubjectDetection, TextDetection, VlmAnalysis,
};
pub use facet::{IndexProgress, IndexProgressError, Video, VideoError};
pub use keyframe::{Keyframe, KeyframeError};
pub use scene::{Scene, SceneError};
pub use track::{
  ColorInfoPlaceholder, DolbyVisionConfigPlaceholder, HdrStaticMetadataPlaceholder,
  RectPlaceholder, VideoCodec, VideoTrack, VideoTrackError,
};
