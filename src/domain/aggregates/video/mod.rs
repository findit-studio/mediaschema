//! Video media-kind aggregate cluster — `Video` facet + `VideoTrack` +
//! per-stream `Scene` + `Keyframe`, plus the apple-vision / VLM /
//! colorthief detection VOs that hang off `Keyframe`.
//!
//! Locked specs: `schema/video.md` r8, `schema/video_track.md` r7,
//! `schema/scene.md` r6+r7-reopen, `schema/keyframe.md` r15+r16-reopen.
//!
//! The frame/pixel/colour vocabulary the locked specs route to
//! `::mediaframe` (codec / pixel-format / colour / frame / disposition
//! descriptor types) is consumed directly from the published
//! `mediaframe` crate; this cluster no longer carries placeholder types.

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
pub use track::{VideoTrack, VideoTrackError};
