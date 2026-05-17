# SP1 (`common`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use `- [ ]`. Executed autonomously/continuously per user directive — no per-task human check-in.

**Goal:** Migrate the full `findit-proto/src/common/` type set into `mediaschema`'s `media.v1` package as clean buffa-generated proto3, with round-trip/JSON/quickcheck tests.

**Architecture:** Append messages/enums to `proto/media/v1/types.proto` in 6 dependency-ordered batches; regenerate checked-in code via `cargo run -p xtask -- gen`; extend `src/lib.rs` named re-exports; add tests to `tests/roundtrip.rs`. Reuse SP0 `Detection`/`BoundingBox`; `mediatime.v1.*` stays `extern_path`-mapped (already imported). No `findit-proto` dependency — fidelity is by faithful authoring from spec §5 (read-only reference).

**Tech Stack:** buffa/buffa-types/buffa-build `0.6` + mediatime `0.1.6` (crates.io), protoc 34.0, feature `json`/`arbitrary`/`quickcheck`.

**Spec:** `docs/superpowers/specs/2026-05-17-mediaschema-full-migration-design.md` §5 (authoritative field/enum mappings). Branch: `sp1-common` (off `sp0-foundation`).

---

## Conventions (all batches)

- Append proto into `proto/media/v1/types.proto` (package `media.v1`; `import "mediatime/v1/mediatime.proto";` already present; `Detection`/`BoundingBox` already defined by SP0 — reference, don't redefine).
- proto3: every `enum` has a `*_UNSPECIFIED = 0` first value. Singular nested messages → `buffa::MessageField<T>`. `optional` scalar → `Option`. `repeated` → `Vec`. mediatime types referenced as `mediatime.v1.TimeRange` etc.
- After each batch's proto edit: `cargo run -p xtask -- gen` (regenerates `src/generated/**`).
- Extend `src/lib.rs` re-export list with the batch's new public message/enum idents (named, not glob).
- Tests use the existing `rt` helper in `tests/roundtrip.rs`:

```rust
fn rt<M: buffa::Message + PartialEq + std::fmt::Debug>(m: &M) {
    let bytes = m.encode_to_vec();
    let back = M::decode_from_slice(&bytes).expect("decode");
    assert_eq!(*m, back, "wire round-trip mismatch");
}
```
Per new type add: a populated instance + `M::default()`, both through `rt`. Construct via the generated owned struct (public fields; nested singular → `buffa::MessageField::some(...)`; enum fields → the generated enum's variant wrapped as the generated code expects; `repeated` → `vec![...]`). **Before writing each batch's tests, run** `grep -n "pub struct <T>\|pub enum <T>\|pub .*:\|MessageField\|EnumValue" src/generated/media.v1*.rs` for the batch's types and match the exact generated field names/wrappers (buffa may raw-ident-escape or wrap enums in `EnumValue`); adjust constructions to the real API.
- Verification block (run after each batch):
```
cd /Users/user/Develop/findit-studio/mediaschema
cargo build && cargo build --no-default-features && cargo build --features json && cargo build --features arbitrary && cargo build --features quickcheck && cargo build --all-features
cargo test --features quickcheck,json 2>&1 | grep "test result:"
cargo run -p xtask -- gen && git diff --exit-code -- src/generated && echo GEN_CLEAN
```
All builds PASS; tests all pass; `GEN_CLEAN`. If a cargo cmd hits a registry/network error, retry with the Bash `dangerouslyDisableSandbox: true` (deps cached).
- Commit each batch on `sp1-common`:
```
git add proto/media/v1/types.proto src/generated src/lib.rs tests/roundtrip.rs
git commit -m "feat(sp1): <batch name> media.v1 types"
```
(append `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`)

---

## Task 1: Batch 1 — primitives

**Files:** modify `proto/media/v1/types.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate `src/generated/**`.

- [ ] **Step 1: Append proto**

```protobuf
message Point2D { float x = 1; float y = 2; }

message Dimensions { uint32 width = 1; uint32 height = 2; }

message Aesthetics { float overall_score = 1; bool is_utility = 2; }

message HorizonInfo { float angle = 1; float confidence = 2; }

message CodecId { int64 value = 1; }

message FeaturePrint { bytes data = 1; uint32 element_type = 2; }

enum VideoFormat {
  VIDEO_FORMAT_UNSPECIFIED = 0;
  VIDEO_FORMAT_AVI = 1; VIDEO_FORMAT_FLV = 2; VIDEO_FORMAT_MP4 = 3;
  VIDEO_FORMAT_M4V = 4; VIDEO_FORMAT_MKV = 5; VIDEO_FORMAT_MOV = 6;
  VIDEO_FORMAT_MXF = 7; VIDEO_FORMAT_MTS = 8; VIDEO_FORMAT_TS = 9;
  VIDEO_FORMAT_WMV = 10; VIDEO_FORMAT_WEBM = 11;
}

enum AudioFormat {
  AUDIO_FORMAT_UNSPECIFIED = 0;
  AUDIO_FORMAT_MP3 = 1; AUDIO_FORMAT_AAC = 2; AUDIO_FORMAT_FLAC = 3;
  AUDIO_FORMAT_WAV = 4; AUDIO_FORMAT_OGG = 5; AUDIO_FORMAT_WMA = 6;
  AUDIO_FORMAT_M4A = 7; AUDIO_FORMAT_OPUS = 8; AUDIO_FORMAT_AIFF = 9;
}

message MediaKind {
  oneof kind { VideoFormat video = 1; AudioFormat audio = 2; }
}

message DocumentSegment {
  Point2D top_left = 1; Point2D top_right = 2;
  Point2D bottom_left = 3; Point2D bottom_right = 4;
  float confidence = 5;
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen` (expect `generated -> …`, new types in `src/generated/media.v1*.rs`, no `mediatime.v1`/`__view` files).

- [ ] **Step 3: Extend `src/lib.rs` re-exports** — add to the `pub use generated::media::v1::{…};` list: `Point2D, Dimensions, Aesthetics, HorizonInfo, CodecId, FeaturePrint, VideoFormat, AudioFormat, MediaKind, DocumentSegment`.

- [ ] **Step 4: Add round-trip tests** to `tests/roundtrip.rs` (confirm generated field names first per Conventions). Example shape:

```rust
#[test]
fn batch1_roundtrip() {
    rt(&Point2D { x: 0.25, y: -0.5, ..Default::default() });
    rt(&Point2D::default());
    rt(&Dimensions { width: 1920, height: 1080, ..Default::default() });
    rt(&Aesthetics { overall_score: 0.8, is_utility: true, ..Default::default() });
    rt(&HorizonInfo { angle: 12.5, confidence: 0.9, ..Default::default() });
    rt(&CodecId { value: 27, ..Default::default() });
    rt(&FeaturePrint { data: b"\x01\x02\x03".to_vec().into(), element_type: 4, ..Default::default() });
    rt(&DocumentSegment {
        top_left: buffa::MessageField::some(Point2D { x: 0.0, y: 0.0, ..Default::default() }),
        bottom_right: buffa::MessageField::some(Point2D { x: 1.0, y: 1.0, ..Default::default() }),
        confidence: 0.95, ..Default::default()
    });
    rt(&DocumentSegment::default());
    // MediaKind oneof: exercise both arms (use the generated oneof API confirmed from src/generated).
}
```
Add a `#[cfg(feature = "json")]` variant doing `serde_json` round-trip for one populated `DocumentSegment` (proves the gated serde over nested messages). The `quickcheck` feature exercises the injected `QuickcheckArbitrary` automatically — add one `#[cfg(feature="quickcheck")]` `quickcheck::quickcheck` prop over `Dimensions` (scalar-only) using `TestResult::discard()` for any non-finite (mirror SP0's `detection_quickcheck_roundtrip`).

- [ ] **Step 5: Verify** (Conventions verification block). Expected: all builds PASS, tests PASS, `GEN_CLEAN`.

- [ ] **Step 6: Commit** — `feat(sp1): batch1 primitives (Point2D/Dimensions/Aesthetics/HorizonInfo/CodecId/FeaturePrint/VideoFormat/AudioFormat/MediaKind/DocumentSegment)`.

---

## Task 2: Batch 2 — identity / infra

**Files:** same set.

- [ ] **Step 1: Append proto**

```protobuf
message Id { bytes value = 1; }            // 16-byte UUIDv7

message FileChecksum { bytes value = 1; }  // 32-byte blake3-256

message Local {
  Id volume = 1;
  repeated string components = 2;
}

message Location {
  oneof kind { Local local = 1; }
}

message LocationTarget {
  oneof kind { string local = 1; }
}

message AppPathBuf {
  FileChecksum checksum = 1;
  Location location = 2;
}

message Tag { string name = 1; uint32 color = 2; }

message ErrorInfo { uint32 code = 1; string message = 2; }
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.
- [ ] **Step 3: Re-exports** — add `Id, FileChecksum, Local, Location, LocationTarget, AppPathBuf, Tag, ErrorInfo`.
- [ ] **Step 4: Tests** — `batch2_roundtrip`: populated + default for each. For `Location`/`LocationTarget` exercise the `oneof` (the generated oneof enum — confirm name from `src/generated`); `Local` with `components: vec!["a".into(), "b".into()]` and a nested `Id`; `AppPathBuf` nesting `FileChecksum`+`Location`. `Id`/`FileChecksum` with non-empty `value` bytes (16/32 bytes) and default (empty). Add the `#[cfg(feature="json")]` JSON round-trip for one `AppPathBuf`.
- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp1): batch2 identity/infra (Id/FileChecksum/Local/Location/LocationTarget/AppPathBuf/Tag/ErrorInfo)`.

---

## Task 3: Batch 3 — location aggregates

**Files:** same set.

- [ ] **Step 1: Append proto**

```protobuf
message WatchedLocation {
  Id id = 1;
  Location location = 2;
  string name = 3;
  uint32 status = 4;            // LocationStatus bitflags (ACTIVE=1, TOMBSTONED=2)
  int64 created_at = 5;
  optional int64 deleted_at = 6;
  uint64 total_files = 7;
  uint64 indexed_files = 8;
  uint64 total_videos = 9;
  uint64 indexed_videos = 10;
  uint64 total_scenes = 11;
  uint64 total_audios = 12;
  uint64 indexed_audios = 13;
  uint64 total_failed_files = 14;
  uint64 failed_videos = 15;
  uint64 failed_audios = 16;
}

message VolumeMeta {
  Id id = 1;
  Location location = 2;
  string name = 3;
  uint64 total_size = 4;
  uint64 used_size = 5;
  uint32 status = 6;            // VolumeStatus bitflags (EJECTABLE=1, ACTIVE=2)
}
```

- [ ] **Step 2: Regenerate.**
- [ ] **Step 3: Re-exports** — add `WatchedLocation, VolumeMeta`.
- [ ] **Step 4: Tests** — `batch3_roundtrip`: a fully-populated `WatchedLocation` (nested `Id`/`Location`, `deleted_at: Some(...)` and a second case `None`, all counters non-zero, `status: 3`) + default; `VolumeMeta` populated + default. `#[cfg(feature="json")]` JSON round-trip for the populated `WatchedLocation`.
- [ ] **Step 5: Verify.**
- [ ] **Step 6: Commit** — `feat(sp1): batch3 location aggregates (WatchedLocation/VolumeMeta)`.

---

## Task 4: Batch 4 — detection envelopes + simple detectors

**Files:** same set.

- [ ] **Step 1: Append proto** (`Detection`/`BoundingBox` already exist from SP0 — reference only)

```protobuf
message ClassificationDetection { Detection detection = 1; }
message ActionDetection { Detection detection = 1; }
message EmotionDetection { Detection detection = 1; }
message MoodDetection { Detection detection = 1; }
message LightingDetection { Detection detection = 1; }
message ColorDetection { Detection detection = 1; }

message ObjectDetection {
  Detection detection = 1;
  optional BoundingBox bbox = 2;
}
message SubjectDetection {
  Detection detection = 1;
  BoundingBox bbox = 2;
}
message TextDetection {
  string text = 1;
  float confidence = 2;
  BoundingBox bbox = 3;
}
message BarcodeDetection {
  string payload = 1;
  string symbology = 2;
  float confidence = 3;
  BoundingBox bbox = 4;
}
message FaceDetection {
  BoundingBox bbox = 1;
  float confidence = 2;
  float capture_quality = 3;
  float roll = 4;
  float yaw = 5;
  float pitch = 6;
}
message SaliencyRegion {
  BoundingBox bbox = 1;
  float confidence = 2;
}
```

- [ ] **Step 2: Regenerate.**
- [ ] **Step 3: Re-exports** — add `ClassificationDetection, ActionDetection, EmotionDetection, MoodDetection, LightingDetection, ColorDetection, ObjectDetection, SubjectDetection, TextDetection, BarcodeDetection, FaceDetection, SaliencyRegion`.
- [ ] **Step 4: Tests** — `batch4_roundtrip`: each envelope with a populated `Detection { label, confidence }`; `ObjectDetection` with `bbox: Some` and a `None` case; `SubjectDetection`/`TextDetection`/`BarcodeDetection` nesting `BoundingBox`; `FaceDetection` all 6 floats; `SaliencyRegion`. Populated + default each. `#[cfg(feature="json")]` JSON round-trip for one `BarcodeDetection`.
- [ ] **Step 5: Verify.**
- [ ] **Step 6: Commit** — `feat(sp1): batch4 detection envelopes + simple detectors`.

---

## Task 5: Batch 5 — landmarks / pose / masks

**Files:** same set.

- [ ] **Step 1: Append proto**

```protobuf
message FaceLandmarkPoint { float x = 1; float y = 2; }
message FaceLandmarkRegion {
  string name = 1;
  repeated FaceLandmarkPoint points = 2;
}
message FaceLandmarksDetection {
  BoundingBox bbox = 1;
  float confidence = 2;
  repeated FaceLandmarkRegion regions = 3;
}
message BodyPoseJoint {
  string name = 1; float x = 2; float y = 3; float confidence = 4;
}
message BodyPoseDetection {
  BoundingBox bbox = 1;
  float confidence = 2;
  repeated BodyPoseJoint joints = 3;
}
message BodyPose3DJoint {
  string name = 1; float x = 2; float y = 3; float z = 4; float confidence = 5;
}
enum BodyPose3DHeightEstimation {
  BODY_POSE_3D_HEIGHT_ESTIMATION_UNSPECIFIED = 0;
  BODY_POSE_3D_HEIGHT_ESTIMATION_REFERENCE = 1;
  BODY_POSE_3D_HEIGHT_ESTIMATION_MEASURED = 2;
}
message BodyPose3DDetection {
  float confidence = 1;
  float body_height = 2;
  BodyPose3DHeightEstimation height_estimation = 3;
  repeated BodyPose3DJoint joints = 4;
}
enum HandChirality {
  HAND_CHIRALITY_UNSPECIFIED = 0;
  HAND_CHIRALITY_LEFT = 1;
  HAND_CHIRALITY_RIGHT = 2;
}
message HandPoseDetection {
  BoundingBox bbox = 1;
  float confidence = 2;
  HandChirality chirality = 3;
  repeated BodyPoseJoint joints = 4;
}
message PersonSegmentationMask {
  BoundingBox bbox = 1;
  float confidence = 2;
  Dimensions dimensions = 3;
  bytes data = 4;
}
message PersonInstanceMaskDetection {
  BoundingBox bbox = 1;
  float confidence = 2;
  uint32 instance_index = 3;
  Dimensions dimensions = 4;
  bytes data = 5;
}
```

- [ ] **Step 2: Regenerate.**
- [ ] **Step 3: Re-exports** — add `FaceLandmarkPoint, FaceLandmarkRegion, FaceLandmarksDetection, BodyPoseJoint, BodyPoseDetection, BodyPose3DJoint, BodyPose3DHeightEstimation, BodyPose3DDetection, HandChirality, HandPoseDetection, PersonSegmentationMask, PersonInstanceMaskDetection`.
- [ ] **Step 4: Tests** — `batch5_roundtrip`: `FaceLandmarkRegion` with `points: vec![FaceLandmarkPoint{..}, ..]`; `FaceLandmarksDetection`/`BodyPoseDetection`/`HandPoseDetection` with repeated joints/regions + nested `BoundingBox`; `BodyPose3DDetection` with the enum set to `REFERENCE`/`MEASURED` and `UNSPECIFIED`; `HandPoseDetection` chirality `LEFT`; `PersonSegmentationMask`/`PersonInstanceMaskDetection` with nested `Dimensions` + `data` bytes. Populated + default each. Enum fields: use the generated representation (likely `buffa::EnumValue<HandChirality>` — confirm from `src/generated` and construct accordingly). `#[cfg(feature="json")]` JSON round-trip for one `BodyPose3DDetection` (covers enum + repeated under serde).
- [ ] **Step 5: Verify.**
- [ ] **Step 6: Commit** — `feat(sp1): batch5 landmarks/pose/masks`.

---

## Task 6: Batch 6 — analysis aggregates + time (mediatime extern)

**Files:** same set.

- [ ] **Step 1: Append proto**

```protobuf
enum TrackTimeSource {
  TRACK_TIME_SOURCE_UNSPECIFIED = 0;
  TRACK_TIME_SOURCE_DECLARED = 1;
  TRACK_TIME_SOURCE_PACKET_OBSERVED = 2;
  TRACK_TIME_SOURCE_DECODED_OBSERVED = 3;
}

message TrackTime {
  optional mediatime.v1.TimeRange declared = 1;
  optional mediatime.v1.TimeRange packet_observed = 2;
  optional mediatime.v1.TimeRange decoded_observed = 3;
}

message AnimalAnalysis {
  repeated SubjectDetection subjects = 1;
  repeated BodyPoseDetection body_poses = 2;
}

message HumanAnalysis {
  repeated SubjectDetection subjects = 1;
  repeated FaceDetection faces = 2;
  repeated BodyPoseDetection body_poses = 3;
  repeated HandPoseDetection hand_poses = 4;
  repeated BodyPose3DDetection body_poses_3d = 5;
  repeated PersonInstanceMaskDetection instance_masks = 6;
  repeated FaceDetection face_rectangles = 7;
  repeated FaceLandmarksDetection face_landmarks = 8;
  repeated PersonSegmentationMask segmentation_masks = 9;
}
```

- [ ] **Step 2: Regenerate.**
- [ ] **Step 3: Re-exports** — add `TrackTimeSource, TrackTime, AnimalAnalysis, HumanAnalysis`.
- [ ] **Step 4: Tests** — `batch6_roundtrip`: `TrackTime` with `declared: Some(mediatime::TimeRange::new(10, 20, mediatime::Timebase::new(30000, NonZeroU32::new(1001).unwrap())))` and the other two `None`, plus a default — this is the SP1 **mediatime-extern** round-trip (proves `optional mediatime.v1.TimeRange` extern fields). `AnimalAnalysis`/`HumanAnalysis` with several repeated nested detections populated + default. `#[cfg(feature="json")]` JSON round-trip for the populated `TrackTime` and a populated `HumanAnalysis`.
- [ ] **Step 5: Verify** (full Conventions block) — and additionally run the SP0 tests still green (`cargo test` shows the SP0 `Detection`/`BoundingBox`/`TimedDetection` tests + all batch tests passing).
- [ ] **Step 6: Commit** — `feat(sp1): batch6 analysis aggregates + TrackTime (mediatime extern)`.

---

## Task 7: SP1 finalize — CI matrix note + DoD

**Files:** none (verification + branch finish).

- [ ] **Step 1: Full SP1 Definition-of-Done check** (run, all must pass):
```
cd /Users/user/Develop/findit-studio/mediaschema
cargo build --no-default-features && cargo build --all-features
cargo test --features quickcheck,json 2>&1 | grep "test result:"
cargo run -p xtask -- gen && git diff --exit-code -- src/generated && echo GEN_CLEAN
```
- [ ] **Step 2:** Confirm `.github/workflows/codegen.yml` already covers SP1 (it runs `xtask gen` drift-gate + the `--all-features` test matrix — no change needed; SP1 added only proto+generated+tests). If the test-matrix job does not exercise `--features quickcheck,json` and SP1 added quickcheck/json-only tests, add `- run: cargo test --features quickcheck,json` to the `test-matrix` job.
- [ ] **Step 3: Open stacked PR** `sp1-common` → `sp0-foundation`:
```
git push -u origin sp1-common
gh pr create --repo Findit-AI/mediaschema --base sp0-foundation --head sp1-common \
  --title "SP1: migrate findit-proto common/ → media.v1 (stacked on #1)" \
  --body "<summary of the 6 batches; DoD evidence; stacked on #1/sp0-foundation>"
```

---

## Self-Review

**1. Spec coverage:** Every spec §5 message/enum maps to a batch task (Batches 1–6 enumerate exactly the §5 catalog; `Detection`/`BoundingBox` reused from SP0; `mediatime.v1.*` extern, not regenerated; excluded helpers absent). ✓
**2. Placeholder scan:** Proto blocks are concrete and complete (the authoritative artifact). Test steps give a complete reusable `rt` recipe + the explicit per-batch type list & construction notes + a mandatory "confirm generated field names from `src/generated`" step (concrete command) — matching SP0's plan style; not placeholders. No "TBD"/"handle edge cases". ✓
**3. Type consistency:** Message/enum names identical between proto blocks, re-export lists, and test descriptions across tasks; `rt` helper defined once in Conventions and reused; mediatime types referenced as `mediatime.v1.TimeRange`/`::mediatime::*` consistently with SP0. ✓
