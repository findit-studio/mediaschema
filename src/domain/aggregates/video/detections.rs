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
// Validated scalar value-objects — Confidence / NormCoord
// ---------------------------------------------------------------------------

/// Error returned when a detection value-object cannot uphold a
/// numeric-range invariant. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum DetectionError {
  /// A confidence value was non-finite (`NaN`/`±inf`) or outside the
  /// calibrated `0.0..=1.0` range.
  #[error("confidence must be finite and within 0.0..=1.0")]
  ConfidenceOutOfRange,
  /// A normalized coordinate was non-finite (`NaN`/`±inf`) or outside
  /// the `0.0..=1.0` range (apple-vision convention).
  #[error("normalized coordinate must be finite and within 0.0..=1.0")]
  CoordOutOfRange,
  /// A bounding box extends past the normalized image edge — `x +
  /// width > 1.0` or `y + height > 1.0`.
  #[error("bounding box must not extend past the normalized image edge")]
  BoxOutOfBounds,
  /// A bounding box had zero width or height — a degenerate box is not
  /// a detection.
  #[error("bounding box width and height must be greater than zero")]
  BoxDegenerate,
  /// A percentage value was non-finite (`NaN`/`±inf`) or outside the
  /// `0.0..=100.0` image-share range.
  #[error("percentage must be finite and within 0.0..=100.0")]
  PercentageOutOfRange,
  /// A document quadrilateral had two coincident corners — a collapsed
  /// quad is not a detection.
  #[error("document quad must not have coincident corners")]
  QuadCollapsedCorners,
  /// A document quadrilateral enclosed zero area (shoelace formula) —
  /// a degenerate / colinear quad is not a detection.
  #[error("document quad must enclose a non-zero area")]
  QuadZeroArea,
  /// A document quadrilateral was non-simple (self-intersecting) — a
  /// pair of non-adjacent edges crossed, or the corner sequence did not
  /// follow a consistent winding order (a "bow-tie" quad).
  #[error("document quad must be simple (non-self-intersecting, consistent winding)")]
  QuadSelfIntersecting,
}

/// A calibrated detection confidence — a `f32` proven **finite** and
/// within the closed range `0.0..=1.0`.
///
/// Construct via [`Confidence::try_new`]; the invariant then holds for
/// the lifetime of the value (the inner `f32` has no public mutator).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Confidence(f32);

impl Confidence {
  /// Validating constructor — rejects `NaN`, `±inf`, and any value
  /// outside `0.0..=1.0`.
  #[inline]
  pub const fn try_new(value: f32) -> Result<Self, DetectionError> {
    if value.is_finite() && value >= 0.0 && value <= 1.0 {
      Ok(Self(value))
    } else {
      Err(DetectionError::ConfidenceOutOfRange)
    }
  }

  /// The validated confidence as a raw `f32` (always finite, in
  /// `0.0..=1.0`).
  #[inline]
  pub const fn get(self) -> f32 {
    self.0
  }
}

/// A normalized image coordinate / extent — a `f32` proven **finite**
/// and within the closed range `0.0..=1.0` (apple-vision convention,
/// origin top-left).
///
/// Construct via [`NormCoord::try_new`]; the invariant then holds for
/// the lifetime of the value (the inner `f32` has no public mutator).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormCoord(f32);

impl NormCoord {
  /// Validating constructor — rejects `NaN`, `±inf`, and any value
  /// outside `0.0..=1.0`.
  #[inline]
  pub const fn try_new(value: f32) -> Result<Self, DetectionError> {
    if value.is_finite() && value >= 0.0 && value <= 1.0 {
      Ok(Self(value))
    } else {
      Err(DetectionError::CoordOutOfRange)
    }
  }

  /// The validated coordinate as a raw `f32` (always finite, in
  /// `0.0..=1.0`).
  #[inline]
  pub const fn get(self) -> f32 {
    self.0
  }
}

// ---------------------------------------------------------------------------
// Detection — `{ label, confidence }`
// ---------------------------------------------------------------------------

/// Apple-vision image-classification detection. Label + calibrated
/// confidence.
///
/// `confidence` is a validated [`Confidence`] (finite, `0.0..=1.0`) —
/// the invariant holds through construction *and* every mutator.
#[derive(Debug, Clone, PartialEq)]
pub struct Detection {
  label: SmolStr,
  confidence: Confidence,
}

impl Detection {
  /// Validating constructor — rejects a non-finite / out-of-range
  /// `confidence` with [`DetectionError::ConfidenceOutOfRange`].
  #[inline]
  pub fn try_new(label: impl Into<SmolStr>, confidence: f32) -> Result<Self, DetectionError> {
    Ok(Self {
      label: label.into(),
      confidence: Confidence::try_new(confidence)?,
    })
  }

  /// The detected label.
  #[inline]
  pub fn label(&self) -> &str {
    self.label.as_str()
  }

  /// Calibrated apple-vision confidence (finite, `0.0..=1.0`).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence.get()
  }

  /// Builder: replace label.
  #[inline]
  pub fn with_label(mut self, label: impl Into<SmolStr>) -> Self {
    self.label = label.into();
    self
  }

  /// Fallible builder: replace `confidence`, re-validating the
  /// finite-and-`0.0..=1.0` invariant.
  #[inline]
  pub fn try_with_confidence(mut self, confidence: f32) -> Result<Self, DetectionError> {
    self.confidence = Confidence::try_new(confidence)?;
    Ok(self)
  }

  /// In-place mutator for label.
  #[inline]
  pub fn set_label(&mut self, label: impl Into<SmolStr>) {
    self.label = label.into();
  }

  /// Fallible in-place mutator for `confidence`, re-validating the
  /// invariant. On success returns `&mut Self`; on a bad value returns
  /// [`DetectionError::ConfidenceOutOfRange`] and leaves `self`
  /// unchanged.
  #[inline]
  pub fn try_set_confidence(&mut self, confidence: f32) -> Result<&mut Self, DetectionError> {
    self.confidence = Confidence::try_new(confidence)?;
    Ok(self)
  }
}

// ---------------------------------------------------------------------------
// BoundingBox — `{ x, y, width, height }`
// ---------------------------------------------------------------------------

/// Normalised 2-D bounding box (apple-vision convention: floats in
/// `[0.0, 1.0]`, origin top-left).
///
/// Each coordinate / extent is a validated [`NormCoord`] (finite,
/// `0.0..=1.0`) — `BoundingBox` is immutable (no public mutator), so
/// the only entry point is the validating [`BoundingBox::try_new`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
  x: NormCoord,
  y: NormCoord,
  width: NormCoord,
  height: NormCoord,
}

impl BoundingBox {
  /// Validating constructor from `(x, y, width, height)`.
  ///
  /// Rejects:
  /// - any non-finite / out-of-`[0.0, 1.0]` component
  ///   ([`DetectionError::CoordOutOfRange`]);
  /// - a zero-extent box (`width == 0.0` or `height == 0.0` — a
  ///   degenerate box is not a detection,
  ///   [`DetectionError::BoxDegenerate`]);
  /// - a box that extends past the normalized image edge (`x + width >
  ///   1.0` or `y + height > 1.0`,
  ///   [`DetectionError::BoxOutOfBounds`]).
  #[inline]
  pub const fn try_new(x: f32, y: f32, width: f32, height: f32) -> Result<Self, DetectionError> {
    // `?` is not allowed in `const fn`; match each component.
    let x = match NormCoord::try_new(x) {
      Ok(v) => v,
      Err(e) => return Err(e),
    };
    let y = match NormCoord::try_new(y) {
      Ok(v) => v,
      Err(e) => return Err(e),
    };
    let width = match NormCoord::try_new(width) {
      Ok(v) => v,
      Err(e) => return Err(e),
    };
    let height = match NormCoord::try_new(height) {
      Ok(v) => v,
      Err(e) => return Err(e),
    };
    // Composite geometry: extents must be non-degenerate and the box
    // must stay inside the normalized image. Each component is already
    // finite and in `[0.0, 1.0]`, so the sums below cannot be NaN.
    if width.get() <= 0.0 || height.get() <= 0.0 {
      return Err(DetectionError::BoxDegenerate);
    }
    if x.get() + width.get() > 1.0 || y.get() + height.get() > 1.0 {
      return Err(DetectionError::BoxOutOfBounds);
    }
    Ok(Self {
      x,
      y,
      width,
      height,
    })
  }

  /// `x` (left edge).
  #[inline]
  pub const fn x(&self) -> f32 {
    self.x.get()
  }
  /// `y` (top edge).
  #[inline]
  pub const fn y(&self) -> f32 {
    self.y.get()
  }
  /// Width.
  #[inline]
  pub const fn width(&self) -> f32 {
    self.width.get()
  }
  /// Height.
  #[inline]
  pub const fn height(&self) -> f32 {
    self.height.get()
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
  confidence: Confidence,
  bbox: BoundingBox,
}

impl TextDetection {
  /// Validating constructor — rejects a non-finite / out-of-range
  /// `confidence` with [`DetectionError::ConfidenceOutOfRange`].
  #[inline]
  pub fn try_new(
    text: impl Into<SmolStr>,
    confidence: f32,
    bbox: BoundingBox,
  ) -> Result<Self, DetectionError> {
    Ok(Self {
      text: text.into(),
      confidence: Confidence::try_new(confidence)?,
      bbox,
    })
  }

  /// Detected text.
  #[inline]
  pub fn text(&self) -> &str {
    self.text.as_str()
  }
  /// Confidence (finite, `0.0..=1.0`).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence.get()
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
  confidence: Confidence,
  bbox: BoundingBox,
}

impl BarcodeDetection {
  /// Validating constructor — rejects a non-finite / out-of-range
  /// `confidence` with [`DetectionError::ConfidenceOutOfRange`].
  #[inline]
  pub fn try_new(
    payload: impl Into<SmolStr>,
    symbology: impl Into<SmolStr>,
    confidence: f32,
    bbox: BoundingBox,
  ) -> Result<Self, DetectionError> {
    Ok(Self {
      payload: payload.into(),
      symbology: symbology.into(),
      confidence: Confidence::try_new(confidence)?,
      bbox,
    })
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
  /// Confidence (finite, `0.0..=1.0`).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence.get()
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
  confidence: Confidence,
}

impl SaliencyRegion {
  /// Validating constructor — rejects a non-finite / out-of-range
  /// `confidence` with [`DetectionError::ConfidenceOutOfRange`].
  #[inline]
  pub const fn try_new(bbox: BoundingBox, confidence: f32) -> Result<Self, DetectionError> {
    match Confidence::try_new(confidence) {
      Ok(confidence) => Ok(Self { bbox, confidence }),
      Err(e) => Err(e),
    }
  }

  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
  /// Confidence (finite, `0.0..=1.0`).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence.get()
  }
}

/// Apple-vision horizon detection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HorizonInfo {
  angle: f32,
  confidence: Confidence,
}

impl HorizonInfo {
  /// Validating constructor from `(angle, confidence)` — rejects a
  /// non-finite / out-of-range `confidence` with
  /// [`DetectionError::ConfidenceOutOfRange`]. `angle` is a radian
  /// measure (not a `0.0..=1.0` quantity) and is not range-validated.
  #[inline]
  pub const fn try_new(angle: f32, confidence: f32) -> Result<Self, DetectionError> {
    match Confidence::try_new(confidence) {
      Ok(confidence) => Ok(Self { angle, confidence }),
      Err(e) => Err(e),
    }
  }

  /// Horizon angle (radians, per apple-vision).
  #[inline]
  pub const fn angle(&self) -> f32 {
    self.angle
  }

  /// Confidence (finite, `0.0..=1.0`).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence.get()
  }
}

/// Apple-vision document quad-corner segment.
///
/// Each corner is a pair of validated normalized [`NormCoord`]s; the
/// segment is immutable, so the only entry point is the validating
/// [`DocumentSegment::try_new`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DocumentSegment {
  top_left: (NormCoord, NormCoord),
  top_right: (NormCoord, NormCoord),
  bottom_right: (NormCoord, NormCoord),
  bottom_left: (NormCoord, NormCoord),
  confidence: Confidence,
}

impl DocumentSegment {
  /// Validating constructor from the four corners + confidence.
  ///
  /// Rejects:
  /// - any non-finite / out-of-`[0.0, 1.0]` corner component
  ///   ([`DetectionError::CoordOutOfRange`]) or `confidence`
  ///   ([`DetectionError::ConfidenceOutOfRange`]);
  /// - a collapsed quad — any two of the four corners coincide
  ///   ([`DetectionError::QuadCollapsedCorners`]);
  /// - a zero-area quad — the four corners are colinear / degenerate,
  ///   detected via the shoelace formula
  ///   ([`DetectionError::QuadZeroArea`]);
  /// - a non-simple (self-intersecting) quad — a "bow-tie" where a pair
  ///   of non-adjacent edges crosses, or the `TL → TR → BR → BL`
  ///   corner sequence does not follow a consistent winding order
  ///   ([`DetectionError::QuadSelfIntersecting`]).
  #[inline]
  pub const fn try_new(
    top_left: (f32, f32),
    top_right: (f32, f32),
    bottom_right: (f32, f32),
    bottom_left: (f32, f32),
    confidence: f32,
  ) -> Result<Self, DetectionError> {
    let top_left = match norm_pair(top_left) {
      Ok(v) => v,
      Err(e) => return Err(e),
    };
    let top_right = match norm_pair(top_right) {
      Ok(v) => v,
      Err(e) => return Err(e),
    };
    let bottom_right = match norm_pair(bottom_right) {
      Ok(v) => v,
      Err(e) => return Err(e),
    };
    let bottom_left = match norm_pair(bottom_left) {
      Ok(v) => v,
      Err(e) => return Err(e),
    };
    let confidence = match Confidence::try_new(confidence) {
      Ok(v) => v,
      Err(e) => return Err(e),
    };
    // Composite geometry: every component is already finite and in
    // `[0.0, 1.0]`, so the comparisons / sums below cannot be NaN.
    // Corners are walked in order TL → TR → BR → BL.
    let corners = [
      (top_left.0.get(), top_left.1.get()),
      (top_right.0.get(), top_right.1.get()),
      (bottom_right.0.get(), bottom_right.1.get()),
      (bottom_left.0.get(), bottom_left.1.get()),
    ];
    // Reject any two coincident corners — a collapsed quad.
    let mut i = 0;
    while i < 4 {
      let mut j = i + 1;
      while j < 4 {
        if corners[i].0 == corners[j].0 && corners[i].1 == corners[j].1 {
          return Err(DetectionError::QuadCollapsedCorners);
        }
        j += 1;
      }
      i += 1;
    }
    // Shoelace formula — twice the signed polygon area. Zero ⇒ the
    // four corners are colinear / the quad encloses no area.
    let mut twice_area = 0.0f32;
    let mut k = 0;
    while k < 4 {
      let (x0, y0) = corners[k];
      let (x1, y1) = corners[(k + 1) % 4];
      twice_area += x0 * y1 - x1 * y0;
      k += 1;
    }
    if twice_area == 0.0 {
      return Err(DetectionError::QuadZeroArea);
    }
    // Simple-quad check. A bow-tie has non-zero shoelace area and four
    // distinct corners yet is self-intersecting. Two guards:
    //  1. consistent winding — the cross-products of every pair of
    //     consecutive edges share one sign (a bow-tie flips sign);
    //  2. non-adjacent edges do not cross — edge TL→TR vs BR→BL, and
    //     edge TR→BR vs BL→TL.
    if !consistent_winding(&corners) {
      return Err(DetectionError::QuadSelfIntersecting);
    }
    if segments_intersect(corners[0], corners[1], corners[2], corners[3])
      || segments_intersect(corners[1], corners[2], corners[3], corners[0])
    {
      return Err(DetectionError::QuadSelfIntersecting);
    }
    Ok(Self {
      top_left,
      top_right,
      bottom_right,
      bottom_left,
      confidence,
    })
  }

  /// Top-left corner.
  #[inline]
  pub const fn top_left(&self) -> (f32, f32) {
    (self.top_left.0.get(), self.top_left.1.get())
  }
  /// Top-right corner.
  #[inline]
  pub const fn top_right(&self) -> (f32, f32) {
    (self.top_right.0.get(), self.top_right.1.get())
  }
  /// Bottom-right corner.
  #[inline]
  pub const fn bottom_right(&self) -> (f32, f32) {
    (self.bottom_right.0.get(), self.bottom_right.1.get())
  }
  /// Bottom-left corner.
  #[inline]
  pub const fn bottom_left(&self) -> (f32, f32) {
    (self.bottom_left.0.get(), self.bottom_left.1.get())
  }
  /// Confidence (finite, `0.0..=1.0`).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence.get()
  }
}

/// Validate a `(f32, f32)` coordinate pair into a `(NormCoord, NormCoord)`.
const fn norm_pair(p: (f32, f32)) -> Result<(NormCoord, NormCoord), DetectionError> {
  let x = match NormCoord::try_new(p.0) {
    Ok(v) => v,
    Err(e) => return Err(e),
  };
  let y = match NormCoord::try_new(p.1) {
    Ok(v) => v,
    Err(e) => return Err(e),
  };
  Ok((x, y))
}

/// 2-D cross product of `b - a` and `c - a` — the signed area term used
/// for orientation tests. All inputs are finite (`NormCoord`-validated),
/// so the result is finite.
#[inline]
const fn cross(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
  (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// True iff the four consecutive-edge turns of the `TL → TR → BR → BL`
/// corner ring all share one sign — i.e. the quad is convex and wound
/// consistently. A bow-tie (self-intersecting) quad flips orientation
/// at the crossing and fails this test. Zero turns (a colinear vertex)
/// are tolerated here; the zero-area guard already rejects a fully
/// degenerate quad.
#[inline]
const fn consistent_winding(c: &[(f32, f32); 4]) -> bool {
  let mut pos = false;
  let mut neg = false;
  let mut i = 0;
  while i < 4 {
    // Turn at vertex `i+1`: edge (i → i+1) followed by (i+1 → i+2).
    let t = cross(c[i], c[(i + 1) % 4], c[(i + 2) % 4]);
    if t > 0.0 {
      pos = true;
    } else if t < 0.0 {
      neg = true;
    }
    i += 1;
  }
  !(pos && neg)
}

/// True iff open segment `p1→p2` and open segment `p3→p4` properly
/// cross. Uses the standard orientation-sign test; collinear / shared-
/// endpoint touching is **not** treated as a crossing (adjacent edges
/// of a quad legitimately share a corner — only non-adjacent edges are
/// passed here).
#[inline]
const fn segments_intersect(
  p1: (f32, f32),
  p2: (f32, f32),
  p3: (f32, f32),
  p4: (f32, f32),
) -> bool {
  let d1 = cross(p3, p4, p1);
  let d2 = cross(p3, p4, p2);
  let d3 = cross(p1, p2, p3);
  let d4 = cross(p1, p2, p4);
  // Proper crossing: each segment's endpoints straddle the other line.
  ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
    && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

// ---------------------------------------------------------------------------
// Pose joints + per-shape pose detections
// ---------------------------------------------------------------------------

/// One 2-D body / hand pose joint.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyPoseJoint {
  name: SmolStr,
  x: NormCoord,
  y: NormCoord,
  confidence: Confidence,
}

impl BodyPoseJoint {
  /// Validating constructor — rejects a non-finite / out-of-`[0.0, 1.0]`
  /// normalized `x`/`y` ([`DetectionError::CoordOutOfRange`]) or
  /// `confidence` ([`DetectionError::ConfidenceOutOfRange`]).
  #[inline]
  pub fn try_new(
    name: impl Into<SmolStr>,
    x: f32,
    y: f32,
    confidence: f32,
  ) -> Result<Self, DetectionError> {
    Ok(Self {
      name: name.into(),
      x: NormCoord::try_new(x)?,
      y: NormCoord::try_new(y)?,
      confidence: Confidence::try_new(confidence)?,
    })
  }

  /// Joint name (apple-vision string id).
  #[inline]
  pub fn name(&self) -> &str {
    self.name.as_str()
  }
  /// `x` coordinate (normalised, `0.0..=1.0`).
  #[inline]
  pub const fn x(&self) -> f32 {
    self.x.get()
  }
  /// `y` coordinate (normalised, `0.0..=1.0`).
  #[inline]
  pub const fn y(&self) -> f32 {
    self.y.get()
  }
  /// Confidence (finite, `0.0..=1.0`).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence.get()
  }
}

/// One 3-D body-pose joint (apple-vision body-3D request).
#[derive(Debug, Clone, PartialEq)]
pub struct BodyPose3DJoint {
  name: SmolStr,
  x: f32,
  y: f32,
  z: f32,
  confidence: Confidence,
}

impl BodyPose3DJoint {
  /// Validating constructor — rejects a non-finite / out-of-range
  /// `confidence` with [`DetectionError::ConfidenceOutOfRange`]. The
  /// `x`/`y`/`z` coordinates are model-space metres (not a `0.0..=1.0`
  /// quantity) and are not range-validated.
  #[inline]
  pub fn try_new(
    name: impl Into<SmolStr>,
    x: f32,
    y: f32,
    z: f32,
    confidence: f32,
  ) -> Result<Self, DetectionError> {
    Ok(Self {
      name: name.into(),
      x,
      y,
      z,
      confidence: Confidence::try_new(confidence)?,
    })
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
  /// Confidence (finite, `0.0..=1.0`).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence.get()
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
  confidence: Confidence,
  joints: std::vec::Vec<BodyPoseJoint>,
}

impl BodyPoseDetection {
  /// Validating constructor — rejects a non-finite / out-of-range
  /// `confidence` with [`DetectionError::ConfidenceOutOfRange`].
  #[inline]
  pub fn try_new(
    bbox: BoundingBox,
    confidence: f32,
    joints: impl Into<std::vec::Vec<BodyPoseJoint>>,
  ) -> Result<Self, DetectionError> {
    Ok(Self {
      bbox,
      confidence: Confidence::try_new(confidence)?,
      joints: joints.into(),
    })
  }

  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
  /// Confidence (finite, `0.0..=1.0`).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence.get()
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
  confidence: Confidence,
  chirality: HandChirality,
  joints: std::vec::Vec<BodyPoseJoint>,
}

impl HandPoseDetection {
  /// Validating constructor — rejects a non-finite / out-of-range
  /// `confidence` with [`DetectionError::ConfidenceOutOfRange`].
  #[inline]
  pub fn try_new(
    bbox: BoundingBox,
    confidence: f32,
    chirality: HandChirality,
    joints: impl Into<std::vec::Vec<BodyPoseJoint>>,
  ) -> Result<Self, DetectionError> {
    Ok(Self {
      bbox,
      confidence: Confidence::try_new(confidence)?,
      chirality,
      joints: joints.into(),
    })
  }

  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
  /// Confidence (finite, `0.0..=1.0`).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence.get()
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
  confidence: Confidence,
  body_height: f32,
  height_estimation: BodyPose3DHeightEstimation,
  joints: std::vec::Vec<BodyPose3DJoint>,
}

impl BodyPose3DDetection {
  /// Validating constructor — rejects a non-finite / out-of-range
  /// `confidence` with [`DetectionError::ConfidenceOutOfRange`].
  /// `body_height` is a metre estimate (not a `0.0..=1.0` quantity)
  /// and is not range-validated.
  #[inline]
  pub fn try_new(
    confidence: f32,
    body_height: f32,
    height_estimation: BodyPose3DHeightEstimation,
    joints: impl Into<std::vec::Vec<BodyPose3DJoint>>,
  ) -> Result<Self, DetectionError> {
    Ok(Self {
      confidence: Confidence::try_new(confidence)?,
      body_height,
      height_estimation,
      joints: joints.into(),
    })
  }

  /// Confidence (finite, `0.0..=1.0`).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence.get()
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
  confidence: Confidence,
  capture_quality: f32,
  roll: f32,
  yaw: f32,
  pitch: f32,
}

impl FaceDetection {
  /// Validating constructor — rejects a non-finite / out-of-range
  /// `confidence` with [`DetectionError::ConfidenceOutOfRange`].
  /// `capture_quality` and the `roll`/`yaw`/`pitch` Euler angles are
  /// apple-vision-defined measures, not `0.0..=1.0` quantities, and are
  /// not range-validated.
  #[inline]
  pub const fn try_new(
    bbox: BoundingBox,
    confidence: f32,
    capture_quality: f32,
    roll: f32,
    yaw: f32,
    pitch: f32,
  ) -> Result<Self, DetectionError> {
    match Confidence::try_new(confidence) {
      Ok(confidence) => Ok(Self {
        bbox,
        confidence,
        capture_quality,
        roll,
        yaw,
        pitch,
      }),
      Err(e) => Err(e),
    }
  }

  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
  /// Confidence (finite, `0.0..=1.0`).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence.get()
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
///
/// Each point is a pair of validated normalized [`NormCoord`]s
/// (finite, `0.0..=1.0`); the region is immutable, so the only entry
/// point is the validating [`FaceLandmarkRegion::try_new`].
#[derive(Debug, Clone, PartialEq)]
pub struct FaceLandmarkRegion {
  name: SmolStr,
  points: std::vec::Vec<(NormCoord, NormCoord)>,
}

impl FaceLandmarkRegion {
  /// Validating constructor — rejects any non-finite / out-of-`[0.0,
  /// 1.0]` point component with [`DetectionError::CoordOutOfRange`].
  #[inline]
  pub fn try_new(
    name: impl Into<SmolStr>,
    points: impl IntoIterator<Item = (f32, f32)>,
  ) -> Result<Self, DetectionError> {
    let points = points
      .into_iter()
      .map(norm_pair)
      .collect::<Result<std::vec::Vec<_>, _>>()?;
    Ok(Self {
      name: name.into(),
      points,
    })
  }

  /// Region name.
  #[inline]
  pub fn name(&self) -> &str {
    self.name.as_str()
  }

  /// Normalised landmark points (each component finite, `0.0..=1.0`).
  #[inline]
  pub fn points(&self) -> std::vec::Vec<(f32, f32)> {
    self.points.iter().map(|p| (p.0.get(), p.1.get())).collect()
  }
}

/// Apple-vision face-landmark detection — a bbox + a confidence + a set
/// of named landmark regions.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceLandmarksDetection {
  bbox: BoundingBox,
  confidence: Confidence,
  regions: std::vec::Vec<FaceLandmarkRegion>,
}

impl FaceLandmarksDetection {
  /// Validating constructor — rejects a non-finite / out-of-range
  /// `confidence` with [`DetectionError::ConfidenceOutOfRange`].
  #[inline]
  pub fn try_new(
    bbox: BoundingBox,
    confidence: f32,
    regions: impl Into<std::vec::Vec<FaceLandmarkRegion>>,
  ) -> Result<Self, DetectionError> {
    Ok(Self {
      bbox,
      confidence: Confidence::try_new(confidence)?,
      regions: regions.into(),
    })
  }

  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
  /// Confidence (finite, `0.0..=1.0`).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence.get()
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
  confidence: Confidence,
  instance_index: u32,
  dimensions: Dimensions,
  data: Bytes,
}

impl PersonInstanceMaskDetection {
  /// Validating constructor — rejects a non-finite / out-of-range
  /// `confidence` with [`DetectionError::ConfidenceOutOfRange`].
  #[inline]
  pub fn try_new(
    bbox: BoundingBox,
    confidence: f32,
    instance_index: u32,
    dimensions: Dimensions,
    data: impl Into<Bytes>,
  ) -> Result<Self, DetectionError> {
    Ok(Self {
      bbox,
      confidence: Confidence::try_new(confidence)?,
      instance_index,
      dimensions,
      data: data.into(),
    })
  }

  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
  /// Confidence (finite, `0.0..=1.0`).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence.get()
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
  confidence: Confidence,
  dimensions: Dimensions,
  data: Bytes,
}

impl PersonSegmentationMask {
  /// Validating constructor — rejects a non-finite / out-of-range
  /// `confidence` with [`DetectionError::ConfidenceOutOfRange`].
  #[inline]
  pub fn try_new(
    bbox: BoundingBox,
    confidence: f32,
    dimensions: Dimensions,
    data: impl Into<Bytes>,
  ) -> Result<Self, DetectionError> {
    Ok(Self {
      bbox,
      confidence: Confidence::try_new(confidence)?,
      dimensions,
      data: data.into(),
    })
  }

  /// Bounding box.
  #[inline]
  pub const fn bbox(&self) -> &BoundingBox {
    &self.bbox
  }
  /// Confidence (finite, `0.0..=1.0`).
  #[inline]
  pub const fn confidence(&self) -> f32 {
    self.confidence.get()
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
  /// Validating constructor — rejects a non-finite (`NaN`/`±inf`) or
  /// out-of-`[0.0, 100.0]` `percentage` with
  /// [`DetectionError::PercentageOutOfRange`].
  #[inline]
  pub fn try_new(
    rgb: Rgba,
    name: impl Into<SmolStr>,
    percentage: f32,
    population: u32,
  ) -> Result<Self, DetectionError> {
    if !(percentage.is_finite() && (0.0..=100.0).contains(&percentage)) {
      return Err(DetectionError::PercentageOutOfRange);
    }
    Ok(Self {
      rgb,
      name: name.into(),
      percentage,
      population,
    })
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

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  #[test]
  fn confidence_rejects_nan_inf_and_out_of_range() {
    for bad in [
      f32::NAN,
      f32::INFINITY,
      f32::NEG_INFINITY,
      -0.001,
      1.001,
      2.0,
    ] {
      assert_eq!(
        Confidence::try_new(bad).err(),
        Some(DetectionError::ConfidenceOutOfRange),
        "{bad} should be rejected"
      );
    }
    // Bounds and an interior value are accepted.
    for ok in [0.0, 0.5, 1.0] {
      assert_eq!(Confidence::try_new(ok).unwrap().get(), ok);
    }
  }

  #[test]
  fn norm_coord_rejects_nan_inf_and_out_of_range() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.5, 1.5] {
      assert_eq!(
        NormCoord::try_new(bad).err(),
        Some(DetectionError::CoordOutOfRange),
        "{bad} should be rejected"
      );
    }
    for ok in [0.0, 0.25, 1.0] {
      assert_eq!(NormCoord::try_new(ok).unwrap().get(), ok);
    }
  }

  #[test]
  fn detection_ctor_and_mutators_validate_confidence() {
    assert_eq!(
      Detection::try_new("dog", f32::NAN).err(),
      Some(DetectionError::ConfidenceOutOfRange)
    );
    assert_eq!(
      Detection::try_new("dog", 1.5).err(),
      Some(DetectionError::ConfidenceOutOfRange)
    );

    let d = Detection::try_new("dog", 0.9).unwrap();
    assert_eq!(d.confidence(), 0.9);

    // The consuming builder rejects a bad value.
    assert_eq!(
      d.clone().try_with_confidence(f32::INFINITY).err(),
      Some(DetectionError::ConfidenceOutOfRange)
    );

    // The in-place setter rejects a bad value and leaves `self` intact.
    let mut d = d;
    assert_eq!(
      d.try_set_confidence(-0.1).err(),
      Some(DetectionError::ConfidenceOutOfRange)
    );
    assert_eq!(d.confidence(), 0.9);

    // A valid replacement is accepted.
    d.try_set_confidence(0.2).unwrap();
    assert_eq!(d.confidence(), 0.2);
    assert!(DetectionError::ConfidenceOutOfRange.is_confidence_out_of_range());
  }

  #[test]
  fn bounding_box_ctor_validates_every_component() {
    let ok = BoundingBox::try_new(0.1, 0.2, 0.3, 0.4).unwrap();
    assert_eq!(
      (ok.x(), ok.y(), ok.width(), ok.height()),
      (0.1, 0.2, 0.3, 0.4)
    );

    // Each component is checked.
    assert_eq!(
      BoundingBox::try_new(f32::NAN, 0.0, 0.0, 0.0).err(),
      Some(DetectionError::CoordOutOfRange)
    );
    assert_eq!(
      BoundingBox::try_new(0.0, 1.5, 0.0, 0.0).err(),
      Some(DetectionError::CoordOutOfRange)
    );
    assert_eq!(
      BoundingBox::try_new(0.0, 0.0, -0.1, 0.0).err(),
      Some(DetectionError::CoordOutOfRange)
    );
    assert_eq!(
      BoundingBox::try_new(0.0, 0.0, 0.0, f32::INFINITY).err(),
      Some(DetectionError::CoordOutOfRange)
    );
  }

  #[test]
  fn bounding_box_rejects_edge_overflow() {
    // rev-2 finding: per-component validation accepts `x=0.9,width=0.2`
    // even though the box runs past the normalized image edge.
    assert_eq!(
      BoundingBox::try_new(0.9, 0.0, 0.2, 0.1).err(),
      Some(DetectionError::BoxOutOfBounds)
    );
    assert_eq!(
      BoundingBox::try_new(0.0, 0.9, 0.1, 0.2).err(),
      Some(DetectionError::BoxOutOfBounds)
    );
    assert!(DetectionError::BoxOutOfBounds.is_box_out_of_bounds());

    // A box flush against the edge (`x + width == 1.0`) is allowed.
    assert!(BoundingBox::try_new(0.8, 0.7, 0.2, 0.3).is_ok());
    // The full-frame box is allowed.
    assert!(BoundingBox::try_new(0.0, 0.0, 1.0, 1.0).is_ok());
  }

  #[test]
  fn bounding_box_rejects_zero_extent() {
    // rev-2 finding: a degenerate (zero-area) box is not a detection.
    assert_eq!(
      BoundingBox::try_new(0.1, 0.2, 0.0, 0.4).err(),
      Some(DetectionError::BoxDegenerate)
    );
    assert_eq!(
      BoundingBox::try_new(0.1, 0.2, 0.3, 0.0).err(),
      Some(DetectionError::BoxDegenerate)
    );
    assert!(DetectionError::BoxDegenerate.is_box_degenerate());

    // Any positive extent that stays in-bounds is fine.
    assert!(BoundingBox::try_new(0.1, 0.2, 0.001, 0.001).is_ok());
  }

  #[test]
  fn dominant_color_validates_percentage() {
    let rgb = Rgba::default();

    // rev-2 finding: NaN / inf / negative / > 100 are all rejected.
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.001, 100.001] {
      assert_eq!(
        DominantColor::try_new(rgb, "red", bad, 1).err(),
        Some(DetectionError::PercentageOutOfRange),
        "{bad} should be rejected"
      );
    }
    assert!(DetectionError::PercentageOutOfRange.is_percentage_out_of_range());

    // Bounds and an interior value are accepted.
    for ok in [0.0, 42.5, 100.0] {
      assert_eq!(
        DominantColor::try_new(rgb, "red", ok, 1)
          .unwrap()
          .percentage(),
        ok
      );
    }
  }

  #[test]
  fn detection_vos_reject_bad_confidence() {
    let bb = BoundingBox::try_new(0.0, 0.0, 1.0, 1.0).unwrap();
    assert!(TextDetection::try_new("hi", f32::NAN, bb).is_err());
    assert!(BarcodeDetection::try_new("p", "qr", 2.0, bb).is_err());
    assert!(SaliencyRegion::try_new(bb, -1.0).is_err());
    assert!(HorizonInfo::try_new(0.0, f32::INFINITY).is_err());
    assert!(BodyPoseJoint::try_new("j", 0.5, 0.5, 1.5).is_err());
    assert!(BodyPoseJoint::try_new("j", 1.5, 0.5, 0.5).is_err());
    assert!(BodyPose3DJoint::try_new("j", 0.0, 0.0, 0.0, f32::NAN).is_err());
    assert!(BodyPoseDetection::try_new(bb, 9.0, std::vec::Vec::new()).is_err());
    assert!(
      HandPoseDetection::try_new(bb, f32::NAN, HandChirality::Left, std::vec::Vec::new()).is_err()
    );
    assert!(BodyPose3DDetection::try_new(
      -0.5,
      1.7,
      BodyPose3DHeightEstimation::Measured,
      std::vec::Vec::new()
    )
    .is_err());
    assert!(FaceDetection::try_new(bb, 1.2, 0.0, 0.0, 0.0, 0.0).is_err());
    assert!(FaceLandmarksDetection::try_new(bb, f32::NAN, std::vec::Vec::new()).is_err());

    // Happy paths all succeed.
    assert!(TextDetection::try_new("hi", 0.8, bb).is_ok());
    assert!(BodyPoseJoint::try_new("j", 0.5, 0.5, 0.9).is_ok());
    assert!(FaceDetection::try_new(bb, 0.9, 0.5, 0.1, 0.2, 0.3).is_ok());
  }

  #[test]
  fn document_segment_and_landmark_region_validate_coords() {
    // A proper non-degenerate unit-square quad (TL, TR, BR, BL).
    let tl = (0.1, 0.1);
    let tr = (0.9, 0.1);
    let br = (0.9, 0.9);
    let bl = (0.1, 0.9);
    assert!(DocumentSegment::try_new(tl, tr, br, bl, 0.9).is_ok());
    // A bad corner component is rejected.
    assert_eq!(
      DocumentSegment::try_new((1.5, 0.5), tr, br, bl, 0.9).err(),
      Some(DetectionError::CoordOutOfRange)
    );
    // A bad confidence is rejected.
    assert_eq!(
      DocumentSegment::try_new(tl, tr, br, bl, 2.0).err(),
      Some(DetectionError::ConfidenceOutOfRange)
    );

    assert!(FaceLandmarkRegion::try_new("leftEye", [(0.1, 0.2), (0.3, 0.4)]).is_ok());
    assert_eq!(
      FaceLandmarkRegion::try_new("leftEye", [(0.1, 0.2), (f32::NAN, 0.4)]).err(),
      Some(DetectionError::CoordOutOfRange)
    );
  }

  #[test]
  fn document_segment_rejects_degenerate_quad() {
    // rev-3 finding 2: per-corner validation accepts a collapsed /
    // zero-area quad even though it is not a real document detection.
    let p = (0.5, 0.5);
    // All four corners coincident — fully collapsed.
    assert_eq!(
      DocumentSegment::try_new(p, p, p, p, 0.9).err(),
      Some(DetectionError::QuadCollapsedCorners)
    );
    // Just two coincident corners (TR == BR) — still collapsed.
    assert_eq!(
      DocumentSegment::try_new((0.1, 0.1), (0.9, 0.5), (0.9, 0.5), (0.1, 0.9), 0.9).err(),
      Some(DetectionError::QuadCollapsedCorners)
    );
    assert!(DetectionError::QuadCollapsedCorners.is_quad_collapsed_corners());

    // Four distinct but colinear corners — zero enclosed area.
    assert_eq!(
      DocumentSegment::try_new((0.1, 0.1), (0.2, 0.2), (0.3, 0.3), (0.4, 0.4), 0.9).err(),
      Some(DetectionError::QuadZeroArea)
    );
    assert!(DetectionError::QuadZeroArea.is_quad_zero_area());

    // A proper non-degenerate quad still passes.
    assert!(DocumentSegment::try_new((0.1, 0.1), (0.9, 0.1), (0.9, 0.9), (0.1, 0.9), 0.9).is_ok());
  }

  #[test]
  fn document_segment_rejects_self_intersecting_quad() {
    // rev-4 finding 3: a bow-tie quad has four distinct in-range
    // corners and a non-zero shoelace area, yet is self-intersecting.
    // This one crosses the TL→TR vs BR→BL non-adjacent edge pair.
    let bowtie = DocumentSegment::try_new(
      (0.1, 0.1), // TL
      (0.2, 0.2), // TR
      (0.1, 0.2), // BR
      (0.9, 0.1), // BL
      0.9,
    );
    assert_eq!(bowtie.err(), Some(DetectionError::QuadSelfIntersecting));
    assert!(DetectionError::QuadSelfIntersecting.is_quad_self_intersecting());

    // A second bow-tie with non-zero shoelace area, crossing the other
    // non-adjacent edge pair (TR→BR vs BL→TL).
    let bowtie2 = DocumentSegment::try_new(
      (0.1, 0.1), // TL
      (0.1, 0.2), // TR
      (0.2, 0.1), // BR
      (0.2, 0.8), // BL
      0.9,
    );
    assert_eq!(bowtie2.err(), Some(DetectionError::QuadSelfIntersecting));

    // A normal convex document quad (slightly skewed) still passes.
    assert!(
      DocumentSegment::try_new((0.12, 0.10), (0.88, 0.14), (0.90, 0.86), (0.10, 0.90), 0.9).is_ok()
    );
  }
}
