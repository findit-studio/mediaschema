//! Detection / analysis value-objects for [`Keyframe`](super::keyframe::Keyframe).
//!
//! All types here are **mediaschema-owned** (apple-vision / VLM / colorthief
//! are engine/service crates — flatten/own, not extern, per the locked
//! `extern vs flatten` rule). Encapsulation: every field is private; access
//! via getters + `with_*` / `set_*`. Where a field is a plain `SmolStr`,
//! `""` means absent; where it is open-vocab VLM natural-language output,
//! the type is `LocalizedText` (locked rev 14/15 rule).

use bytes::Bytes;
use derive_more::IsVariant;
use mediaframe::frame::Dimensions;
use smol_str::SmolStr;

use crate::domain::{vo::LocalizedText, Rgba};

// ---------------------------------------------------------------------------
// Detection — `{ label, confidence }`
// ---------------------------------------------------------------------------

/// Apple-vision image-classification detection. Label + calibrated
/// confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct Detection {
  label: SmolStr,
  confidence: f32,
}

impl Detection {
  /// Construct from label + confidence.
  #[inline]
  pub fn new(label: impl Into<SmolStr>, confidence: f32) -> Self {
    Self {
      label: label.into(),
      confidence,
    }
  }

  /// The detected label.
  #[inline]
  pub fn label(&self) -> &str {
    self.label.as_str()
  }

  /// Calibrated apple-vision confidence (0.0 – 1.0).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }

  /// Builder: replace label.
  #[inline]
  pub fn with_label(mut self, label: impl Into<SmolStr>) -> Self {
    self.label = label.into();
    self
  }

  /// Builder: replace confidence.
  #[inline]
  pub const fn with_confidence(mut self, confidence: f32) -> Self {
    self.confidence = confidence;
    self
  }

  /// In-place mutator for label.
  #[inline]
  pub fn set_label(&mut self, label: impl Into<SmolStr>) {
    self.label = label.into();
  }

  /// In-place mutator for confidence.
  #[inline]
  pub const fn set_confidence(&mut self, confidence: f32) {
    self.confidence = confidence;
  }
}

// ---------------------------------------------------------------------------
// BoundingBox — `{ x, y, width, height }`
// ---------------------------------------------------------------------------

/// Normalised 2-D bounding box (apple-vision convention: floats in
/// `[0.0, 1.0]`, origin top-left).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
  x: f32,
  y: f32,
  width: f32,
  height: f32,
}

impl BoundingBox {
  /// Construct from `(x, y, width, height)`.
  #[inline]
  pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
    Self {
      x,
      y,
      width,
      height,
    }
  }

  /// `x` (left edge).
  #[inline]
  pub const fn x(&self) -> f32 {
    self.x
  }
  /// `y` (top edge).
  #[inline]
  pub const fn y(&self) -> f32 {
    self.y
  }
  /// Width.
  #[inline]
  pub const fn width(&self) -> f32 {
    self.width
  }
  /// Height.
  #[inline]
  pub const fn height(&self) -> f32 {
    self.height
  }
}

// ---------------------------------------------------------------------------
// Object / action / text / barcode / saliency / horizon / document
// ---------------------------------------------------------------------------

/// Apple-vision object detection: a [`Detection`] plus an optional bbox.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectDetection {
  detection: Detection,
  bbox: Option<BoundingBox>,
}

impl ObjectDetection {
  /// Construct.
  #[inline]
  pub fn new(detection: Detection, bbox: Option<BoundingBox>) -> Self {
    Self { detection, bbox }
  }

  /// Inner `{label, confidence}`.
  #[inline]
  pub const fn detection(&self) -> &Detection {
    &self.detection
  }

  /// Optional bounding box.
  #[inline]
  pub const fn bbox(&self) -> Option<&BoundingBox> {
    self.bbox.as_ref()
  }

  /// Builder: replace bbox.
  #[inline]
  pub const fn with_bbox(mut self, bbox: Option<BoundingBox>) -> Self {
    self.bbox = bbox;
    self
  }

  /// In-place mutator for bbox.
  #[inline]
  pub const fn set_bbox(&mut self, bbox: Option<BoundingBox>) {
    self.bbox = bbox;
  }
}

/// Apple-vision body-pose-derived action detection.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionDetection {
  detection: Detection,
}

impl ActionDetection {
  /// Construct.
  #[inline]
  pub const fn new(detection: Detection) -> Self {
    Self { detection }
  }

  /// Inner `{label, confidence}`.
  #[inline]
  pub const fn detection(&self) -> &Detection {
    &self.detection
  }
}

/// Apple-vision OCR result.
#[derive(Debug, Clone, PartialEq)]
pub struct TextDetection {
  text: SmolStr,
  confidence: f32,
  bbox: BoundingBox,
}

impl TextDetection {
  /// Construct.
  #[inline]
  pub fn new(text: impl Into<SmolStr>, confidence: f32, bbox: BoundingBox) -> Self {
    Self {
      text: text.into(),
      confidence,
      bbox,
    }
  }

  /// Detected text.
  #[inline]
  pub fn text(&self) -> &str {
    self.text.as_str()
  }
  /// Confidence.
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }
  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
}

/// Apple-vision barcode detection.
#[derive(Debug, Clone, PartialEq)]
pub struct BarcodeDetection {
  payload: SmolStr,
  symbology: SmolStr,
  confidence: f32,
  bbox: BoundingBox,
}

impl BarcodeDetection {
  /// Construct.
  #[inline]
  pub fn new(
    payload: impl Into<SmolStr>,
    symbology: impl Into<SmolStr>,
    confidence: f32,
    bbox: BoundingBox,
  ) -> Self {
    Self {
      payload: payload.into(),
      symbology: symbology.into(),
      confidence,
      bbox,
    }
  }

  /// Decoded payload.
  #[inline]
  pub fn payload(&self) -> &str {
    self.payload.as_str()
  }
  /// Symbology name (`"qr"`, `"ean13"`, …).
  #[inline]
  pub fn symbology(&self) -> &str {
    self.symbology.as_str()
  }
  /// Confidence.
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }
  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
}

/// Apple-vision attention / objectness saliency region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SaliencyRegion {
  bbox: BoundingBox,
  confidence: f32,
}

impl SaliencyRegion {
  /// Construct.
  #[inline]
  pub const fn new(bbox: BoundingBox, confidence: f32) -> Self {
    Self { bbox, confidence }
  }

  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
  /// Confidence.
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }
}

/// Apple-vision horizon detection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HorizonInfo {
  angle: f32,
  confidence: f32,
}

impl HorizonInfo {
  /// Construct from `(angle, confidence)`.
  #[inline]
  pub const fn new(angle: f32, confidence: f32) -> Self {
    Self { angle, confidence }
  }

  /// Horizon angle (radians, per apple-vision).
  #[inline]
  pub const fn angle(&self) -> f32 {
    self.angle
  }

  /// Confidence.
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }
}

/// Apple-vision document quad-corner segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DocumentSegment {
  top_left: (f32, f32),
  top_right: (f32, f32),
  bottom_right: (f32, f32),
  bottom_left: (f32, f32),
  confidence: f32,
}

impl DocumentSegment {
  /// Construct from the four corners + confidence.
  #[inline]
  pub const fn new(
    top_left: (f32, f32),
    top_right: (f32, f32),
    bottom_right: (f32, f32),
    bottom_left: (f32, f32),
    confidence: f32,
  ) -> Self {
    Self {
      top_left,
      top_right,
      bottom_right,
      bottom_left,
      confidence,
    }
  }

  /// Top-left corner.
  #[inline]
  pub const fn top_left(&self) -> (f32, f32) {
    self.top_left
  }
  /// Top-right corner.
  #[inline]
  pub const fn top_right(&self) -> (f32, f32) {
    self.top_right
  }
  /// Bottom-right corner.
  #[inline]
  pub const fn bottom_right(&self) -> (f32, f32) {
    self.bottom_right
  }
  /// Bottom-left corner.
  #[inline]
  pub const fn bottom_left(&self) -> (f32, f32) {
    self.bottom_left
  }
  /// Confidence.
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }
}

// ---------------------------------------------------------------------------
// Pose joints + per-shape pose detections
// ---------------------------------------------------------------------------

/// One 2-D body / hand pose joint.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyPoseJoint {
  name: SmolStr,
  x: f32,
  y: f32,
  confidence: f32,
}

impl BodyPoseJoint {
  /// Construct.
  #[inline]
  pub fn new(name: impl Into<SmolStr>, x: f32, y: f32, confidence: f32) -> Self {
    Self {
      name: name.into(),
      x,
      y,
      confidence,
    }
  }

  /// Joint name (apple-vision string id).
  #[inline]
  pub fn name(&self) -> &str {
    self.name.as_str()
  }
  /// `x` coordinate (normalised).
  #[inline]
  pub const fn x(&self) -> f32 {
    self.x
  }
  /// `y` coordinate (normalised).
  #[inline]
  pub const fn y(&self) -> f32 {
    self.y
  }
  /// Confidence.
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }
}

/// One 3-D body-pose joint (apple-vision body-3D request).
#[derive(Debug, Clone, PartialEq)]
pub struct BodyPose3DJoint {
  name: SmolStr,
  x: f32,
  y: f32,
  z: f32,
  confidence: f32,
}

impl BodyPose3DJoint {
  /// Construct.
  #[inline]
  pub fn new(name: impl Into<SmolStr>, x: f32, y: f32, z: f32, confidence: f32) -> Self {
    Self {
      name: name.into(),
      x,
      y,
      z,
      confidence,
    }
  }

  /// Joint name.
  #[inline]
  pub fn name(&self) -> &str {
    self.name.as_str()
  }
  /// `x` coordinate.
  #[inline]
  pub const fn x(&self) -> f32 {
    self.x
  }
  /// `y` coordinate.
  #[inline]
  pub const fn y(&self) -> f32 {
    self.y
  }
  /// `z` coordinate.
  #[inline]
  pub const fn z(&self) -> f32 {
    self.z
  }
  /// Confidence.
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }
}

/// Hand chirality (apple-vision hand-pose request).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, IsVariant)]
#[non_exhaustive]
pub enum HandChirality {
  #[default]
  Unknown,
  Left,
  Right,
}

/// 3-D body-pose height-estimation source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, IsVariant)]
#[non_exhaustive]
pub enum BodyPose3DHeightEstimation {
  #[default]
  Unknown,
  Reference,
  Measured,
}

/// 2-D body-pose detection.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyPoseDetection {
  bbox: BoundingBox,
  confidence: f32,
  joints: std::vec::Vec<BodyPoseJoint>,
}

impl BodyPoseDetection {
  /// Construct.
  #[inline]
  pub fn new(
    bbox: BoundingBox,
    confidence: f32,
    joints: impl Into<std::vec::Vec<BodyPoseJoint>>,
  ) -> Self {
    Self {
      bbox,
      confidence,
      joints: joints.into(),
    }
  }

  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
  /// Confidence.
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }
  /// Pose joints.
  #[inline]
  pub fn joints(&self) -> &[BodyPoseJoint] {
    &self.joints
  }
}

/// 2-D hand-pose detection.
#[derive(Debug, Clone, PartialEq)]
pub struct HandPoseDetection {
  bbox: BoundingBox,
  confidence: f32,
  chirality: HandChirality,
  joints: std::vec::Vec<BodyPoseJoint>,
}

impl HandPoseDetection {
  /// Construct.
  #[inline]
  pub fn new(
    bbox: BoundingBox,
    confidence: f32,
    chirality: HandChirality,
    joints: impl Into<std::vec::Vec<BodyPoseJoint>>,
  ) -> Self {
    Self {
      bbox,
      confidence,
      chirality,
      joints: joints.into(),
    }
  }

  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
  /// Confidence.
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }
  /// Hand chirality.
  #[inline]
  pub const fn chirality(&self) -> HandChirality {
    self.chirality
  }
  /// Hand pose joints.
  #[inline]
  pub fn joints(&self) -> &[BodyPoseJoint] {
    &self.joints
  }
}

/// 3-D body-pose detection.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyPose3DDetection {
  confidence: f32,
  body_height: f32,
  height_estimation: BodyPose3DHeightEstimation,
  joints: std::vec::Vec<BodyPose3DJoint>,
}

impl BodyPose3DDetection {
  /// Construct.
  #[inline]
  pub fn new(
    confidence: f32,
    body_height: f32,
    height_estimation: BodyPose3DHeightEstimation,
    joints: impl Into<std::vec::Vec<BodyPose3DJoint>>,
  ) -> Self {
    Self {
      confidence,
      body_height,
      height_estimation,
      joints: joints.into(),
    }
  }

  /// Confidence.
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }
  /// Body height (apple-vision estimate, metres).
  #[inline]
  pub const fn body_height(&self) -> f32 {
    self.body_height
  }
  /// Source of the height estimate.
  #[inline]
  pub const fn height_estimation(&self) -> BodyPose3DHeightEstimation {
    self.height_estimation
  }
  /// 3-D joints.
  #[inline]
  pub fn joints(&self) -> &[BodyPose3DJoint] {
    &self.joints
  }
}

// ---------------------------------------------------------------------------
// Subject / face / mask / landmark
// ---------------------------------------------------------------------------

/// Apple-vision subject detection (humans/animals share this shape).
#[derive(Debug, Clone, PartialEq)]
pub struct SubjectDetection {
  detection: Detection,
  bbox: BoundingBox,
}

impl SubjectDetection {
  /// Construct.
  #[inline]
  pub const fn new(detection: Detection, bbox: BoundingBox) -> Self {
    Self { detection, bbox }
  }

  /// Inner `{label, confidence}`.
  #[inline]
  pub const fn detection(&self) -> &Detection {
    &self.detection
  }

  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
}

/// Apple-vision face detection (incl. quality + 3-D Euler angles).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceDetection {
  bbox: BoundingBox,
  confidence: f32,
  capture_quality: f32,
  roll: f32,
  yaw: f32,
  pitch: f32,
}

impl FaceDetection {
  /// Construct.
  #[inline]
  pub const fn new(
    bbox: BoundingBox,
    confidence: f32,
    capture_quality: f32,
    roll: f32,
    yaw: f32,
    pitch: f32,
  ) -> Self {
    Self {
      bbox,
      confidence,
      capture_quality,
      roll,
      yaw,
      pitch,
    }
  }

  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
  /// Confidence.
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }
  /// Face capture quality (apple-vision).
  #[inline]
  pub const fn capture_quality(&self) -> f32 {
    self.capture_quality
  }
  /// Roll angle (radians).
  #[inline]
  pub const fn roll(&self) -> f32 {
    self.roll
  }
  /// Yaw angle (radians).
  #[inline]
  pub const fn yaw(&self) -> f32 {
    self.yaw
  }
  /// Pitch angle (radians).
  #[inline]
  pub const fn pitch(&self) -> f32 {
    self.pitch
  }
}

/// Apple-vision face landmark region (e.g. `leftEye`, `outerLips`) with
/// its normalised points.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceLandmarkRegion {
  name: SmolStr,
  points: std::vec::Vec<(f32, f32)>,
}

impl FaceLandmarkRegion {
  /// Construct.
  #[inline]
  pub fn new(name: impl Into<SmolStr>, points: impl Into<std::vec::Vec<(f32, f32)>>) -> Self {
    Self {
      name: name.into(),
      points: points.into(),
    }
  }

  /// Region name.
  #[inline]
  pub fn name(&self) -> &str {
    self.name.as_str()
  }

  /// Normalised landmark points.
  #[inline]
  pub fn points(&self) -> &[(f32, f32)] {
    &self.points
  }
}

/// Apple-vision face-landmark detection — a bbox + a confidence + a set
/// of named landmark regions.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceLandmarksDetection {
  bbox: BoundingBox,
  confidence: f32,
  regions: std::vec::Vec<FaceLandmarkRegion>,
}

impl FaceLandmarksDetection {
  /// Construct.
  #[inline]
  pub fn new(
    bbox: BoundingBox,
    confidence: f32,
    regions: impl Into<std::vec::Vec<FaceLandmarkRegion>>,
  ) -> Self {
    Self {
      bbox,
      confidence,
      regions: regions.into(),
    }
  }

  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
  /// Confidence.
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }
  /// Landmark regions.
  #[inline]
  pub fn regions(&self) -> &[FaceLandmarkRegion] {
    &self.regions
  }
}

/// Apple-vision person-instance segmentation mask.
///
/// `data` is the raw mask bytes (alpha 8 by convention, but
/// apple-vision exposes it via `CVPixelBuffer`).
#[derive(Debug, Clone, PartialEq)]
pub struct PersonInstanceMaskDetection {
  bbox: BoundingBox,
  confidence: f32,
  instance_index: u32,
  dimensions: Dimensions,
  data: Bytes,
}

impl PersonInstanceMaskDetection {
  /// Construct.
  #[inline]
  pub fn new(
    bbox: BoundingBox,
    confidence: f32,
    instance_index: u32,
    dimensions: Dimensions,
    data: impl Into<Bytes>,
  ) -> Self {
    Self {
      bbox,
      confidence,
      instance_index,
      dimensions,
      data: data.into(),
    }
  }

  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
  /// Confidence.
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }
  /// Per-instance index (apple-vision).
  #[inline]
  pub const fn instance_index(&self) -> u32 {
    self.instance_index
  }
  /// Mask dimensions (`mediaframe::frame::Dimensions`).
  #[inline]
  pub const fn dimensions(&self) -> Dimensions {
    self.dimensions
  }
  /// Raw mask bytes (`bytes::Bytes`).
  #[inline]
  pub fn data(&self) -> &[u8] {
    &self.data
  }
}

/// Apple-vision person whole-frame segmentation mask.
#[derive(Debug, Clone, PartialEq)]
pub struct PersonSegmentationMask {
  bbox: BoundingBox,
  confidence: f32,
  dimensions: Dimensions,
  data: Bytes,
}

impl PersonSegmentationMask {
  /// Construct.
  #[inline]
  pub fn new(
    bbox: BoundingBox,
    confidence: f32,
    dimensions: Dimensions,
    data: impl Into<Bytes>,
  ) -> Self {
    Self {
      bbox,
      confidence,
      dimensions,
      data: data.into(),
    }
  }

  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
  /// Confidence.
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence
  }
  /// Mask dimensions (`mediaframe::frame::Dimensions`).
  #[inline]
  pub const fn dimensions(&self) -> Dimensions {
    self.dimensions
  }
  /// Raw mask bytes (`bytes::Bytes`).
  #[inline]
  pub fn data(&self) -> &[u8] {
    &self.data
  }
}

// ---------------------------------------------------------------------------
// HumanAnalysis (9 fields) / AnimalAnalysis
// ---------------------------------------------------------------------------

/// Apple-vision full human analysis block (9 fields — `subjects`,
/// `faces`, `body_poses`, `hand_poses`, `body_poses_3d`,
/// `instance_masks`, `face_rectangles`, `face_landmarks`,
/// `segmentation_masks`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HumanAnalysis {
  subjects: std::vec::Vec<SubjectDetection>,
  faces: std::vec::Vec<FaceDetection>,
  body_poses: std::vec::Vec<BodyPoseDetection>,
  hand_poses: std::vec::Vec<HandPoseDetection>,
  body_poses_3d: std::vec::Vec<BodyPose3DDetection>,
  instance_masks: std::vec::Vec<PersonInstanceMaskDetection>,
  face_rectangles: std::vec::Vec<FaceDetection>,
  face_landmarks: std::vec::Vec<FaceLandmarksDetection>,
  segmentation_masks: std::vec::Vec<PersonSegmentationMask>,
}

impl HumanAnalysis {
  /// Empty block.
  #[inline]
  pub fn new() -> Self {
    Self::default()
  }

  /// Subjects.
  #[inline]
  pub fn subjects(&self) -> &[SubjectDetection] {
    &self.subjects
  }
  /// Faces.
  #[inline]
  pub fn faces(&self) -> &[FaceDetection] {
    &self.faces
  }
  /// 2-D body poses.
  #[inline]
  pub fn body_poses(&self) -> &[BodyPoseDetection] {
    &self.body_poses
  }
  /// 2-D hand poses.
  #[inline]
  pub fn hand_poses(&self) -> &[HandPoseDetection] {
    &self.hand_poses
  }
  /// 3-D body poses.
  #[inline]
  pub fn body_poses_3d(&self) -> &[BodyPose3DDetection] {
    &self.body_poses_3d
  }
  /// Per-person instance masks.
  #[inline]
  pub fn instance_masks(&self) -> &[PersonInstanceMaskDetection] {
    &self.instance_masks
  }
  /// Face rectangles (apple-vision face-detect request).
  #[inline]
  pub fn face_rectangles(&self) -> &[FaceDetection] {
    &self.face_rectangles
  }
  /// Face-landmark detections.
  #[inline]
  pub fn face_landmarks(&self) -> &[FaceLandmarksDetection] {
    &self.face_landmarks
  }
  /// Whole-frame person segmentation masks.
  #[inline]
  pub fn segmentation_masks(&self) -> &[PersonSegmentationMask] {
    &self.segmentation_masks
  }

  // --- builders ---
  #[inline]
  pub fn with_subjects(mut self, v: impl Into<std::vec::Vec<SubjectDetection>>) -> Self {
    self.subjects = v.into();
    self
  }
  #[inline]
  pub fn with_faces(mut self, v: impl Into<std::vec::Vec<FaceDetection>>) -> Self {
    self.faces = v.into();
    self
  }
  #[inline]
  pub fn with_body_poses(mut self, v: impl Into<std::vec::Vec<BodyPoseDetection>>) -> Self {
    self.body_poses = v.into();
    self
  }
  #[inline]
  pub fn with_hand_poses(mut self, v: impl Into<std::vec::Vec<HandPoseDetection>>) -> Self {
    self.hand_poses = v.into();
    self
  }
  #[inline]
  pub fn with_body_poses_3d(mut self, v: impl Into<std::vec::Vec<BodyPose3DDetection>>) -> Self {
    self.body_poses_3d = v.into();
    self
  }
  #[inline]
  pub fn with_instance_masks(
    mut self,
    v: impl Into<std::vec::Vec<PersonInstanceMaskDetection>>,
  ) -> Self {
    self.instance_masks = v.into();
    self
  }
  #[inline]
  pub fn with_face_rectangles(mut self, v: impl Into<std::vec::Vec<FaceDetection>>) -> Self {
    self.face_rectangles = v.into();
    self
  }
  #[inline]
  pub fn with_face_landmarks(
    mut self,
    v: impl Into<std::vec::Vec<FaceLandmarksDetection>>,
  ) -> Self {
    self.face_landmarks = v.into();
    self
  }
  #[inline]
  pub fn with_segmentation_masks(
    mut self,
    v: impl Into<std::vec::Vec<PersonSegmentationMask>>,
  ) -> Self {
    self.segmentation_masks = v.into();
    self
  }

  // --- setters ---
  #[inline]
  pub fn set_subjects(&mut self, v: impl Into<std::vec::Vec<SubjectDetection>>) {
    self.subjects = v.into();
  }
  #[inline]
  pub fn set_faces(&mut self, v: impl Into<std::vec::Vec<FaceDetection>>) {
    self.faces = v.into();
  }
  #[inline]
  pub fn set_body_poses(&mut self, v: impl Into<std::vec::Vec<BodyPoseDetection>>) {
    self.body_poses = v.into();
  }
  #[inline]
  pub fn set_hand_poses(&mut self, v: impl Into<std::vec::Vec<HandPoseDetection>>) {
    self.hand_poses = v.into();
  }
  #[inline]
  pub fn set_body_poses_3d(&mut self, v: impl Into<std::vec::Vec<BodyPose3DDetection>>) {
    self.body_poses_3d = v.into();
  }
  #[inline]
  pub fn set_instance_masks(&mut self, v: impl Into<std::vec::Vec<PersonInstanceMaskDetection>>) {
    self.instance_masks = v.into();
  }
  #[inline]
  pub fn set_face_rectangles(&mut self, v: impl Into<std::vec::Vec<FaceDetection>>) {
    self.face_rectangles = v.into();
  }
  #[inline]
  pub fn set_face_landmarks(&mut self, v: impl Into<std::vec::Vec<FaceLandmarksDetection>>) {
    self.face_landmarks = v.into();
  }
  #[inline]
  pub fn set_segmentation_masks(&mut self, v: impl Into<std::vec::Vec<PersonSegmentationMask>>) {
    self.segmentation_masks = v.into();
  }
}

/// Apple-vision animal analysis block.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnimalAnalysis {
  subjects: std::vec::Vec<SubjectDetection>,
  body_poses: std::vec::Vec<BodyPoseDetection>,
}

impl AnimalAnalysis {
  /// Empty block.
  #[inline]
  pub fn new() -> Self {
    Self::default()
  }

  /// Subjects.
  #[inline]
  pub fn subjects(&self) -> &[SubjectDetection] {
    &self.subjects
  }
  /// Body poses.
  #[inline]
  pub fn body_poses(&self) -> &[BodyPoseDetection] {
    &self.body_poses
  }

  /// Builder: replace subjects.
  #[inline]
  pub fn with_subjects(mut self, v: impl Into<std::vec::Vec<SubjectDetection>>) -> Self {
    self.subjects = v.into();
    self
  }
  /// Builder: replace body poses.
  #[inline]
  pub fn with_body_poses(mut self, v: impl Into<std::vec::Vec<BodyPoseDetection>>) -> Self {
    self.body_poses = v.into();
    self
  }
  /// Setter: subjects.
  #[inline]
  pub fn set_subjects(&mut self, v: impl Into<std::vec::Vec<SubjectDetection>>) {
    self.subjects = v.into();
  }
  /// Setter: body poses.
  #[inline]
  pub fn set_body_poses(&mut self, v: impl Into<std::vec::Vec<BodyPoseDetection>>) {
    self.body_poses = v.into();
  }
}

// ---------------------------------------------------------------------------
// Aesthetics + DominantColor
// ---------------------------------------------------------------------------

/// Apple-vision aesthetics analysis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aesthetics {
  overall_score: f32,
  is_utility: bool,
}

impl Aesthetics {
  /// Construct.
  #[inline]
  pub const fn new(overall_score: f32, is_utility: bool) -> Self {
    Self {
      overall_score,
      is_utility,
    }
  }

  /// Overall aesthetic score (apple-vision).
  #[inline]
  pub const fn overall_score(&self) -> f32 {
    self.overall_score
  }
  /// "Utility" classifier — apple-vision flag.
  #[inline]
  pub const fn is_utility(&self) -> bool {
    self.is_utility
  }
}

/// Colorthief dominant colour.
#[derive(Debug, Clone, PartialEq)]
pub struct DominantColor {
  rgb: Rgba,
  name: SmolStr,
  percentage: f32,
  population: u32,
}

impl DominantColor {
  /// Construct.
  #[inline]
  pub fn new(rgb: Rgba, name: impl Into<SmolStr>, percentage: f32, population: u32) -> Self {
    Self {
      rgb,
      name: name.into(),
      percentage,
      population,
    }
  }

  /// Packed RGBA.
  #[inline]
  pub const fn rgb(&self) -> Rgba {
    self.rgb
  }
  /// Human / palette-name label (`""` = unnamed).
  #[inline]
  pub fn name(&self) -> &str {
    self.name.as_str()
  }
  /// Percentage share of the image (0.0–100.0).
  #[inline]
  pub const fn percentage(&self) -> f32 {
    self.percentage
  }
  /// Cluster population (pixel count or sample count).
  #[inline]
  pub const fn population(&self) -> u32 {
    self.population
  }
}

// ---------------------------------------------------------------------------
// VlmAnalysis — grouped llmtask output (all open-vocab → LocalizedText)
// ---------------------------------------------------------------------------

/// All VLM (`llmtask`) natural-language output, grouped (rev 12). Each
/// open-vocab field is `LocalizedText` / `Vec<LocalizedText>` per rev 14
/// — the VLM emits these in its response language. `shot_type` stays
/// plain `SmolStr` (controlled vocabulary, future enum).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VlmAnalysis {
  categories: std::vec::Vec<LocalizedText>,
  description: LocalizedText,
  tags: std::vec::Vec<LocalizedText>,
  shot_type: SmolStr,
  objects: std::vec::Vec<LocalizedText>,
  subjects: std::vec::Vec<LocalizedText>,
  mood: std::vec::Vec<LocalizedText>,
  emotion: std::vec::Vec<LocalizedText>,
  lighting: std::vec::Vec<LocalizedText>,
}

impl VlmAnalysis {
  /// Empty analysis (every field empty / `""`).
  #[inline]
  pub fn new() -> Self {
    Self::default()
  }

  /// Scene categories.
  #[inline]
  pub fn categories(&self) -> &[LocalizedText] {
    &self.categories
  }
  /// Free-form description.
  #[inline]
  pub const fn description(&self) -> &LocalizedText {
    &self.description
  }
  /// VLM-suggested tags.
  #[inline]
  pub fn tags(&self) -> &[LocalizedText] {
    &self.tags
  }
  /// Shot type (controlled — `""` = absent).
  #[inline]
  pub fn shot_type(&self) -> &str {
    self.shot_type.as_str()
  }
  /// VLM open-vocab objects (distinct from apple-vision `objects`).
  #[inline]
  pub fn objects(&self) -> &[LocalizedText] {
    &self.objects
  }
  /// VLM open-vocab subjects.
  #[inline]
  pub fn subjects(&self) -> &[LocalizedText] {
    &self.subjects
  }
  /// Mood labels.
  #[inline]
  pub fn mood(&self) -> &[LocalizedText] {
    &self.mood
  }
  /// Emotion labels.
  #[inline]
  pub fn emotion(&self) -> &[LocalizedText] {
    &self.emotion
  }
  /// Lighting labels.
  #[inline]
  pub fn lighting(&self) -> &[LocalizedText] {
    &self.lighting
  }

  // --- builders ---
  #[inline]
  pub fn with_categories(mut self, v: impl Into<std::vec::Vec<LocalizedText>>) -> Self {
    self.categories = v.into();
    self
  }
  #[inline]
  pub fn with_description(mut self, v: LocalizedText) -> Self {
    self.description = v;
    self
  }
  #[inline]
  pub fn with_tags(mut self, v: impl Into<std::vec::Vec<LocalizedText>>) -> Self {
    self.tags = v.into();
    self
  }
  #[inline]
  pub fn with_shot_type(mut self, v: impl Into<SmolStr>) -> Self {
    self.shot_type = v.into();
    self
  }
  #[inline]
  pub fn with_objects(mut self, v: impl Into<std::vec::Vec<LocalizedText>>) -> Self {
    self.objects = v.into();
    self
  }
  #[inline]
  pub fn with_subjects(mut self, v: impl Into<std::vec::Vec<LocalizedText>>) -> Self {
    self.subjects = v.into();
    self
  }
  #[inline]
  pub fn with_mood(mut self, v: impl Into<std::vec::Vec<LocalizedText>>) -> Self {
    self.mood = v.into();
    self
  }
  #[inline]
  pub fn with_emotion(mut self, v: impl Into<std::vec::Vec<LocalizedText>>) -> Self {
    self.emotion = v.into();
    self
  }
  #[inline]
  pub fn with_lighting(mut self, v: impl Into<std::vec::Vec<LocalizedText>>) -> Self {
    self.lighting = v.into();
    self
  }

  // --- setters ---
  #[inline]
  pub fn set_categories(&mut self, v: impl Into<std::vec::Vec<LocalizedText>>) {
    self.categories = v.into();
  }
  #[inline]
  pub fn set_description(&mut self, v: LocalizedText) {
    self.description = v;
  }
  #[inline]
  pub fn set_tags(&mut self, v: impl Into<std::vec::Vec<LocalizedText>>) {
    self.tags = v.into();
  }
  #[inline]
  pub fn set_shot_type(&mut self, v: impl Into<SmolStr>) {
    self.shot_type = v.into();
  }
  #[inline]
  pub fn set_objects(&mut self, v: impl Into<std::vec::Vec<LocalizedText>>) {
    self.objects = v.into();
  }
  #[inline]
  pub fn set_subjects(&mut self, v: impl Into<std::vec::Vec<LocalizedText>>) {
    self.subjects = v.into();
  }
  #[inline]
  pub fn set_mood(&mut self, v: impl Into<std::vec::Vec<LocalizedText>>) {
    self.mood = v.into();
  }
  #[inline]
  pub fn set_emotion(&mut self, v: impl Into<std::vec::Vec<LocalizedText>>) {
    self.emotion = v.into();
  }
  #[inline]
  pub fn set_lighting(&mut self, v: impl Into<std::vec::Vec<LocalizedText>>) {
    self.lighting = v.into();
  }
}
