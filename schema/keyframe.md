# `Keyframe<Id>` — a scene thumbnail + structured image analysis  *(rev 15 — LOCKED, user-approved)*

## Domain meaning

A representative thumbnail of a `Scene` (`keyframes` **are** the thumbnails;
`Keyframe.parent → Scene.id`). Stores the **inline image bytes** + analysis
from **two producers**:
- **apple-vision** → structured detections (calibrated `confidence` +
  `BoundingBox`), mirrored from `findit-proto::database::Keyframe`. Includes
  the full `HumanAnalysis` (9 fields) + `AnimalAnalysis` pose depth (per-joint
  `name`/`x`/`y`(`/z`)/`confidence`, `chirality`, `body_height`, masks).
- **VLM (`llmtask`)** → flat label/text fields (no confidence, `""`=absent),
  kept distinct from the apple-vision ones (your B). **`mood`/`emotion`/
  `lighting` are VLM** (your call), not apple-vision detections.
- **colorthief** → the colour palette (`colors`).

All mediaschema-owned (engine/service crates → flatten/own, not extern).

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Media-time `mediatime`; `Dimensions` =
`mediaframe` extern. Image is **inline `data: Bytes`** — **no `location`
field** (your E). **`feature_print` + embeddings → LanceDB**, keyed by `id`;
**`phash` dropped** (your C). Detections structured **with**
`confidence`/`bbox` (full depth — your D). Strings `SmolStr` (`""`=absent).
Conversions deferred.

## Fields

| field | domain type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | identity (LanceDB key) |
| `parent` | `Id` | FK → `Scene.id` |
| `pts` | `mediatime::Timestamp` | source position |
| `data` | `Bytes` | the thumbnail image bytes (inline; no `location`) |
| `mime` · `size` | `SmolStr` · `u32` | artifact meta |
| `dimensions` | `mediaframe::Dimensions` | thumbnail W×H (extern) |
| `extractor` | `KeyframeExtractor` (enum) | which extractor produced it |
| `provenance` | `Provenance` (nested VO) | analysis-run reproducibility (model/prompt/indexer) |
| — *apple-vision structured detections (full depth)* — |
| `classifications` | `Vec<Detection>` | image classification |
| `objects` | `Vec<ObjectDetection>` | `Detection` + `Option<BoundingBox>` (apple-vision) |
| `humans` | `HumanAnalysis` | **9 fields** — `subjects`/`faces`/`body_poses`/`hand_poses`/`body_poses_3d`/`instance_masks`/`face_rectangles`/`face_landmarks`/`segmentation_masks` |
| `animals` | `AnimalAnalysis` | `{subjects:Vec<SubjectDetection>, body_poses:Vec<BodyPoseDetection>}` |
| `actions` | `Vec<ActionDetection>` | apple-vision — `Detection` from `VNDetectHumanBodyPoseRequest` |
| `text_detections` | `Vec<TextDetection>` | OCR `{text,confidence,bbox}` |
| `barcodes` | `Vec<BarcodeDetection>` | `{payload,symbology,confidence,bbox}` |
| `attention_saliency` `objectness_saliency` | `Vec<SaliencyRegion>` | `{bbox,confidence}` |
| `horizon` | `HorizonInfo` | `{angle,confidence}` |
| `document_segments` | `Vec<DocumentSegment>` | quad corners + confidence |
| `aesthetics` | `Aesthetics` | `{overall_score,is_utility}` |
| `colors` | `Vec<DominantColor>` | **colorthief** — full `{rgb:Rgba,name,percentage,population}` (your G) |
| — *VLM output (grouped — your call)* — |
| `vlm` | `VlmAnalysis` (nested VO) | all VLM fields grouped (see VO); distinct from apple-vision/colorthief |

## Nested VOs (mediaschema-owned)

`Detection {label:SmolStr,confidence:f32}` · `BoundingBox {x,y,width,height:f32}`
· `ObjectDetection {Detection,bbox:Option<BoundingBox>}` ·
`ActionDetection {Detection}` (VNDetectHumanBodyPose-derived) ·
`SubjectDetection {Detection,bbox:BoundingBox}` ·
`FaceDetection {bbox,confidence,capture_quality:f32,roll:f32,yaw:f32,pitch:f32}`.

**`HumanAnalysis` (9 fields):**
`subjects:Vec<SubjectDetection>` · `faces:Vec<FaceDetection>` ·
`body_poses:Vec<BodyPoseDetection>` · `hand_poses:Vec<HandPoseDetection>` ·
`body_poses_3d:Vec<BodyPose3DDetection>` ·
`instance_masks:Vec<PersonInstanceMaskDetection>` ·
`face_rectangles:Vec<FaceDetection>` ·
`face_landmarks:Vec<FaceLandmarksDetection>` ·
`segmentation_masks:Vec<PersonSegmentationMask>`.
`AnimalAnalysis {subjects:Vec<SubjectDetection>, body_poses:Vec<BodyPoseDetection>}`.

**Pose VOs (full joint depth):**
`BodyPoseJoint {name:SmolStr, x:f32, y:f32, confidence:f32}` ·
`BodyPose3DJoint {name:SmolStr, x:f32, y:f32, z:f32, confidence:f32}` ·
`BodyPoseDetection {bbox:BoundingBox, confidence:f32, joints:Vec<BodyPoseJoint>}` ·
`HandPoseDetection {bbox, confidence, chirality:HandChirality
(Unknown·Left·Right), joints:Vec<BodyPoseJoint>}` ·
`BodyPose3DDetection {confidence:f32, body_height:f32,
height_estimation:BodyPose3DHeightEstimation (Unknown·Reference·Measured),
joints:Vec<BodyPose3DJoint>}`.

**Mask / landmark VOs:**
`PersonInstanceMaskDetection {bbox, confidence, instance_index:u32,
dimensions:mediaframe::Dimensions, data:Bytes}` ·
`PersonSegmentationMask {bbox, confidence, dimensions:mediaframe::Dimensions,
data:Bytes}` ·
`FaceLandmarksDetection {bbox, confidence, regions:Vec<FaceLandmarkRegion>}`
(`FaceLandmarkRegion` = named Apple Vision landmark group, e.g. `leftEye` /
`outerLips`, with normalised points).

`TextDetection {text,confidence,bbox}` · `BarcodeDetection {payload,symbology,
confidence,bbox}` · `SaliencyRegion {bbox,confidence}` ·
`HorizonInfo {angle,confidence}` · `DocumentSegment {4 corners,confidence}` ·
`Aesthetics {overall_score:f32,is_utility:bool}` ·
`DominantColor {rgb:Rgba,name:SmolStr,percentage:f32,population:u32}`.

**`VlmAnalysis`** (grouped `llmtask` output — no confidence, `""`=absent):
`{ categories:Vec<LocalizedText> (was scene_category),
description:LocalizedText, tags:Vec<LocalizedText>,
shot_type:SmolStr, objects:Vec<LocalizedText>, subjects:Vec<LocalizedText>,
mood:Vec<LocalizedText>, emotion:Vec<LocalizedText>,
lighting:Vec<LocalizedText> }`. (Struct namespace replaces the `vlm_` prefix —
`vlm.objects`/`vlm.subjects` vs apple-vision `objects`/`humans`.) All VLM
**natural-language output** is **`LocalizedText`** (shared cross-cutting VO,
[README.md](README.md) — open-vocab phrases the VLM emits in its response
language). Only **controlled** labels stay plain: `shot_type` (small set,
future enum).

**`Provenance`** = the **shared cross-cutting VO** (defined once in
[README.md](README.md) — `{model_name, model_version, prompt_version,
indexer_version}`, all `SmolStr`/`""`=absent); not redefined here. Reused by
`Scene`/`AudioSegment`/… too.

## Resolved (your calls)

- **A** structured apple-vision model adopted (supersedes rev-5 flat lists).
- **B** VLM-only kept: `categories`(was `scene_category`)/`description`/`tags`/
  `shot_type`/`vlm_objects`/`vlm_subjects` — distinct from apple-vision's.
- **C** `feature_print`+embeddings → LanceDB; `phash` dropped.
- **D** full pose/joint depth retained — **rev 10:** `HumanAnalysis` corrected
  to all **9** fields (added `face_landmarks`, `segmentation_masks`); pose VOs
  fully modelled (`BodyPoseJoint`/`BodyPose3DJoint`, `chirality`,
  `body_height`+`height_estimation`, mask `dimensions`/`data`).
- **rev 10:** `mood`/`emotion`/`lighting` moved apple-vision → **VLM**
  (`Vec<SmolStr>`, no confidence); `actions` stays apple-vision
  (`ActionDetection`).
- **rev 11:** added `provenance: Provenance {model_name, model_version,
  prompt_version, indexer_version}` (analysis-run reproducibility).
- **rev 12:** all VLM fields grouped into a **`VlmAnalysis`** VO (resolves the
  `vlm_*` nit — struct namespace replaces the prefix); `Provenance` promoted to
  a **shared cross-cutting VO** (defined in README, reused by scene/audio).
- **rev 13:** `VlmAnalysis.description` → **`LocalizedText {src,translated}`**
  (shared cross-cutting VO; `src_lang` deferred; scene/audio deferred —
  video-first).
- **rev 14:** rule corrected — **all VLM open-vocab output is localized**:
  `categories`/`objects`/`subjects`/`mood`/`emotion`/`lighting` →
  `Vec<LocalizedText>`. `shot_type` stays `SmolStr` (controlled).
- **rev 15:** `tags` → `Vec<LocalizedText>` (your call); `shot_type` confirmed
  plain. **All resolved → user-LOCKED.**
- **E** `location` removed; image is inline `data: Bytes`.
- **F** `KeyframeExtractor` (`#[non_exhaustive]`) = **all `SceneDetector`
  variants** `Histogram·Phash·Threshold·Content·Adaptive` (a scene-detector
  boundary frame is a keyframe) **+** `CompositeQuality·Interval·IFrame·
  SceneRepresentative·Manual`.
- **G** full `DominantColor` (incl. `population`).

## Open

- *(none — all calls resolved; **user-LOCKED**.)*

## Projection notes

- **sqlx**: `keyframe` + detection child tables (`bbox`/`confidence` cols);
  `data` → `BYTEA` or object-store offload keyed by `id`; `text_detections.
  text` full-text; colours in `keyframe_color`. No vector column (LanceDB).
- **mongodb**: `_id`=UUIDv7; detections embedded; `data` GridFS if large.
- **graphql**: image via signed-URL endpoint (never raw `data` in lists) +
  detections/OCR/colours for search; similarity = LanceDB by `id`.

**Status: LOCKED (rev 15) — user-approved.** Full apple-vision body-pose depth
(9-field `HumanAnalysis` + joint/mask VOs); `mood`/`emotion`/`lighting`→VLM;
VLM grouped (`VlmAnalysis`) with all open-vocab output `Vec<LocalizedText>`;
shared `Provenance`/`LocalizedText`; `feature_print`+embeddings→LanceDB.
