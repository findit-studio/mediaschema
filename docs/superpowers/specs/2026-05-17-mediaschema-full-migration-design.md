# mediaschema — Full findit-proto Replacement — Program Design (SP1 + SP2 + SP3)

- **Date:** 2026-05-17
- **Status:** Active — supersedes the product-agnostic scope of the SP0 spec
- **Supersedes:** `2026-05-17-mediaschema-buffa-sp0-foundation-design.md` §2 (decisions 3/9), §3 (decomposition), §7 (database/network out of scope). SP0 doc remains the accurate record of the SP0 foundation.
- **Execution mode:** Autonomous per explicit user directive ("continue … without stopping until you finish all remaining tasks"). Brainstorm/spec are self-authored with documented decisions (no per-gate user approval); quality is enforced by per-task two-stage subagent review. Progress reported at sub-project boundaries only.

## 1. Scope reversal

SP0 scoped mediaschema as *product-agnostic* (only `common` media primitives; findit `database`/`network` excluded). The user **reversed** this (option **a**): mediaschema is now the **full replacement for `findit-proto`** — every proto message type, across all three domains:

- **SP1 — `common`** (~47 files / ~50 types): detection/analysis/media primitives.
- **SP2 — `database`**: scene / subtitle / keyframe / media / video / video_track + the `database/audio/` subsystem.
- **SP3 — `network`**: the daemon↔desktop protocol (folder/browse/event/search/framing/daemon/progress/heartbeat/…, incl. the large `shared.rs`).

The **"product-agnostic" invariant is dropped.** Identity/infra types previously earmarked for exclusion (`Id`, `FileChecksum`, `path_buf`, `watched_location`, `pipeline`, …) are **in scope**.

## 2. Locked decisions (apply to SP1/SP2/SP3)

Carried from SP0 (minus product-agnostic):

- **Source of truth = findit-proto's hand-written Rust** (structs/enums + manual `Encode`/`Decode` for field numbers/wire types). **No byte-for-byte wire compatibility** — clean idiomatic proto3 redesign; correctness = round-trip + semantic fidelity.
- **Clean-redesign latitude** (because no wire-compat): split packed scalar fields into separate proto fields (e.g. `Dimensions` packed `u16|u16` → `uint32 width=1; uint32 height=2;`); model `#[repr]`/newtype-over-int "enums" (`VideoFormat`, `AudioFormat`, `TrackTimeSource`, …) as proto `enum`s; bitflags → `uint32`/`uint64`.
- **One proto message per logical *owned* type.** Ignore the `*Chunk` / zero-copy-decode mirrors and `*Ref` borrow types — same schema, different ownership; buffa generates owned + view itself.
- Deps: `buffa`/`buffa-types` `"0.6"`, `buffa-build` `"0.6"` (xtask), **`mediatime` `"0.1.6"`** — all crates.io. `mediatime::{Timebase,TimeRange,Timestamp}` are **already `extern_path`-mapped** (SP0); `common/timebase.rs` & `common/time_range.rs` are those extern types — **do not regenerate them**; `common/track_time.rs` wraps `TimeRange`.
- **SP0 already shipped `media.v1::{Detection, BoundingBox}`** (+ the `TimedDetection` fixture). SP1 **reuses** these — do not duplicate; other `common` types compose them.
- Codegen: checked-in via `xtask` (`buffa_build::Config`), `gate_impls_on_crate_features(true)`, `generate_json(true)`, `generate_arbitrary(true)`, `generate_views(false)`, `extern_path(".mediatime.v1","::mediatime")`, quickcheck `type_attribute`. Features literally `json`/`arbitrary`/`quickcheck` (quickcheck ⇒ arbitrary, via `mediaschema-derive`). Named (not glob) re-exports. CI `protoc` pinned to authoring version (34.0).
- **Out of scope** (unchanged): redb `Key`/`Value` storage impls, uniffi/FFI, and the ~13 findit consumer-crate cutover — these are findit-side, *not* mediaschema. Only the protobuf **message types** are migrated.

## 3. Package & branch layout

- **Packages:** keep **`media.v1`** for SP0 + SP1 `common` (extend `proto/media/v1/types.proto`, or split into per-area files within `media.v1`). Add **`findit.db.v1`** (SP2) and **`findit.net.v1`** (SP3) as new packages/files; buffa resolves cross-package refs via `super::` and the `include_file` stitcher. The SP0 `media.v1` package name is retained (already merged) — product-agnostic naming is no longer required but churning the merged package is pointless.
- **Stacked branches** (per user directive): `sp1-common` off `sp0-foundation`; `sp2-database` off `sp1-common`; `sp3-network` off `sp2-database`. One stacked PR per sub-project (base = the branch it stacks on).

## 4. Decomposition & per-sub-project flow

Each sub-project is its own brainstorm(autonomous)→spec→writing-plans→subagent-driven-execution→finish cycle. Within each, the type set is **batched by category** (a batch = related types → one proto file/section + `xtask gen` + round-trip/JSON/semantic tests, executed as one implementer task with the two-stage spec+code-quality review and fix loops). Definition of done per sub-project mirrors SP0: full feature-matrix builds, `xtask gen` diff-clean, round-trip + JSON + extern + quickcheck tests pass, CI gen-gate green.

- **SP1 (`common`)** — concrete proto design (the ~50 types, their fields/enums/composition, and the batch breakdown) is authored from the source survey and recorded in §5 below before SP1 planning.
- **SP2 (`database`)** / **SP3 (`network`)** — each gets its own design section appended here (or its own spec doc) when its turn begins, surveyed from `findit-proto/src/database` and `…/network`.

## 5. SP1 (`common`) concrete design

Authored from a full survey of `findit-proto/src/common/`. All in package **`media.v1`** (extends the SP0 file/package). **Reuse SP0** `Detection`, `BoundingBox`. **Extern (do not generate):** `mediatime.v1.{Timebase,TimeRange,Timestamp}` (= `common/timebase.rs`,`time_range.rs`). **Excluded** (no wire format / ownership mirrors / pure-Rust helpers): all `*Ref`/`*Chunk`, `MediaKind` helper is modeled as a oneof, `LocalLocationComponent`/`LocationComponent`, `ErrorDomain`, `Identified<D>`, `PipelineError` (so `pipeline.rs` yields no message). Clean-redesign decisions: split `Dimensions` packed `u16|u16` → two `uint32`; `Id` → `bytes value` (16-byte UUIDv7); `FileChecksum` → `bytes value` (32-byte blake3-256); `TagColor`/bitflags (`LocationStatus`,`VolumeStatus`) → `uint32`; `#[repr]` int "enums" → proto3 `enum` with `*_UNSPECIFIED=0`.

**Enums:** `VideoFormat` (UNSPECIFIED=0, AVI=1,FLV=2,MP4=3,M4V=4,MKV=5,MOV=6,MXF=7,MTS=8,TS=9,WMV=10,WEBM=11); `AudioFormat` (UNSPECIFIED=0, MP3=1,AAC=2,FLAC=3,WAV=4,OGG=5,WMA=6,M4A=7,OPUS=8,AIFF=9); `BodyPose3DHeightEstimation` (UNSPECIFIED=0,REFERENCE=1,MEASURED=2); `HandChirality` (UNSPECIFIED=0,LEFT=1,RIGHT=2); `TrackTimeSource` (UNSPECIFIED=0,DECLARED=1,PACKET_OBSERVED=2,DECODED_OBSERVED=3).

**Messages** (`field: proto_type = N`; `[D]`=`media.v1.Detection`, `[BB]`=`media.v1.BoundingBox`, both from SP0):

| Message | Fields |
|---|---|
| `Point2D` | `float x=1; float y=2` |
| `Dimensions` | `uint32 width=1; uint32 height=2` |
| `Aesthetics` | `float overall_score=1; bool is_utility=2` |
| `HorizonInfo` | `float angle=1; float confidence=2` |
| `CodecId` | `int64 value=1` |
| `FeaturePrint` | `bytes data=1; uint32 element_type=2` |
| `MediaKind` | `oneof kind { VideoFormat video=1; AudioFormat audio=2 }` |
| `DocumentSegment` | `Point2D top_left=1; top_right=2; bottom_left=3; bottom_right=4; float confidence=5` |
| `Id` | `bytes value=1` (16B UUIDv7) |
| `FileChecksum` | `bytes value=1` (32B blake3) |
| `Local` | `Id volume=1; repeated string components=2` |
| `Location` | `oneof kind { Local local=1 }` |
| `LocationTarget` | `oneof kind { string local=1 }` |
| `AppPathBuf` | `FileChecksum checksum=1; Location location=2` |
| `Tag` | `string name=1; fixed32 color=2` |
| `ErrorInfo` | `uint32 code=1; string message=2` |
| `WatchedLocation` | `Id id=1; Location location=2; string name=3; uint32 status=4; int64 created_at=5; optional int64 deleted_at=6; uint64 total_files=7; indexed_files=8; total_videos=9; indexed_videos=10; total_scenes=11; total_audios=12; indexed_audios=13; total_failed_files=14; failed_videos=15; failed_audios=16` |
| `VolumeMeta` | `Id id=1; Location location=2; string name=3; uint64 total_size=4; uint64 used_size=5; uint32 status=6` |
| `ClassificationDetection`/`ActionDetection`/`EmotionDetection`/`MoodDetection`/`LightingDetection`/`ColorDetection` | `[D] detection=1` (each its own message) |
| `ObjectDetection` | `[D] detection=1; optional [BB] bbox=2` |
| `SubjectDetection` | `[D] detection=1; [BB] bbox=2` |
| `TextDetection` | `string text=1; float confidence=2; [BB] bbox=3` |
| `BarcodeDetection` | `string payload=1; string symbology=2; float confidence=3; [BB] bbox=4` |
| `FaceDetection` | `[BB] bbox=1; float confidence=2; capture_quality=3; roll=4; yaw=5; pitch=6` |
| `FaceLandmarkPoint` | `float x=1; float y=2` |
| `FaceLandmarkRegion` | `string name=1; repeated FaceLandmarkPoint points=2` |
| `FaceLandmarksDetection` | `[BB] bbox=1; float confidence=2; repeated FaceLandmarkRegion regions=3` |
| `BodyPoseJoint` | `string name=1; float x=2; float y=3; float confidence=4` |
| `BodyPoseDetection` | `[BB] bbox=1; float confidence=2; repeated BodyPoseJoint joints=3` |
| `BodyPose3DJoint` | `string name=1; float x=2; y=3; z=4; confidence=5` |
| `BodyPose3DDetection` | `float confidence=1; float body_height=2; BodyPose3DHeightEstimation height_estimation=3; repeated BodyPose3DJoint joints=4` |
| `HandPoseDetection` | `[BB] bbox=1; float confidence=2; HandChirality chirality=3; repeated BodyPoseJoint joints=4` |
| `SaliencyRegion` | `[BB] bbox=1; float confidence=2` |
| `PersonSegmentationMask` | `[BB] bbox=1; float confidence=2; Dimensions dimensions=3; bytes data=4` |
| `PersonInstanceMaskDetection` | `[BB] bbox=1; float confidence=2; uint32 instance_index=3; Dimensions dimensions=4; bytes data=5` |
| `AnimalAnalysis` | `repeated SubjectDetection subjects=1; repeated BodyPoseDetection body_poses=2` |
| `HumanAnalysis` | `repeated SubjectDetection subjects=1; repeated FaceDetection faces=2; repeated BodyPoseDetection body_poses=3; repeated HandPoseDetection hand_poses=4; repeated BodyPose3DDetection body_poses_3d=5; repeated PersonInstanceMaskDetection instance_masks=6; repeated FaceDetection face_rectangles=7; repeated FaceLandmarksDetection face_landmarks=8; repeated PersonSegmentationMask segmentation_masks=9` |
| `TrackTime` | `optional mediatime.v1.TimeRange declared=1; optional …TimeRange packet_observed=2; optional …TimeRange decoded_observed=3` (+`TrackTimeSource` enum) |

Correctness model (per SP0's final approach — **no `findit-proto` dependency, not even dev**): correctness = wire round-trip + JSON round-trip (under `json`) + quickcheck property + mediatime-extern round-trip, plus **faithful authoring** (the `.proto` is authored to mirror the surveyed source field/enum mappings; fidelity is verified at authoring/review time against the survey + findit-proto source as read-only reference, not asserted via a code dependency).

**Implementation batches** (dependency-ordered; one implementer task + two-stage review each; each batch = author proto into `proto/media/v1/types.proto` + `xtask gen` + extend `src/lib.rs` named re-exports + round-trip/JSON/quickcheck tests):
1. **Primitives:** `Point2D, Dimensions, Aesthetics, HorizonInfo, CodecId, FeaturePrint, VideoFormat, AudioFormat, MediaKind, DocumentSegment`
2. **Identity/infra:** `Id, FileChecksum, Local, Location, LocationTarget, AppPathBuf, Tag, ErrorInfo`
3. **Location aggregates:** `WatchedLocation, VolumeMeta`
4. **Detection envelopes + simple detectors:** the 6 Detection-envelopes, `ObjectDetection, SubjectDetection, TextDetection, BarcodeDetection, FaceDetection, SaliencyRegion`
5. **Landmarks/pose/masks:** `FaceLandmarkPoint, FaceLandmarkRegion, FaceLandmarksDetection, BodyPoseJoint, BodyPoseDetection, BodyPose3DJoint, BodyPose3DHeightEstimation, BodyPose3DDetection, HandChirality, HandPoseDetection, PersonSegmentationMask, PersonInstanceMaskDetection`
6. **Aggregates + time:** `AnimalAnalysis, HumanAnalysis, TrackTime, TrackTimeSource`

Done-when (SP1): all 6 batches landed; full feature-matrix builds (default/no-default/json/arbitrary/quickcheck/all-features); `xtask gen` diff-clean; round-trip + JSON + quickcheck + mediatime-extern (`TrackTime`) tests green for every SP1 type; stacked PR `sp1-common` → `sp0-foundation`.

## 6. Risks

- **Scale.** ~120 types / ~47k LOC across three domains; executed continuously via the batched subagent pipeline — long-running by nature.
- **Composition depth.** Detection-family types nest base `Detection`/`BoundingBox` and each other; batch ordering must respect dependencies (primitives first).
- **Extern reuse.** `timebase`/`time_range` must map to the existing `::mediatime` extern (not regenerated); `track_time` composes `mediatime.v1.TimeRange`.
- **Enum/bitflags fidelity.** `#[repr]` newtype "enums" and bitflags must round-trip via proto `enum`/integer without losing unknown values (buffa `EnumValue` preserves unknowns).
