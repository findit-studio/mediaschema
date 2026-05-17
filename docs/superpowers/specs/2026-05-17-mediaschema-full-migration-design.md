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

- **Packages:** **single package `media.v1`** in a single file `proto/media/v1/types.proto` for SP0 + SP1 + SP2 + SP3. The previously-anticipated `findit.db.v1` (SP2) and `findit.net.v1` (SP3) separate packages/files were **consolidated into the one same-package file per explicit user directive** — no cross-package refs anywhere; all former `media.v1.*` / `findit.db.v1.*` / `findit.net.v1.*` references are bare same-package references. The 2 name collisions that arose from flattening are resolved by renaming the proto definitions: `findit.db.v1.MediaKind`→`DbMediaKind` (SP1's `MediaKind` oneof message keeps its name) and `findit.net.v1.FailedFile`→`NetFailedFile` (SP2's `FailedFile` DB record keeps its name). `mediatime.v1` remains the sole `extern_path` (`extern_path(".mediatime.v1","::mediatime")`), unchanged.
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

## 6. SP2 — `database` (findit.db.v1)

Authored from a full survey of `findit-proto/src/database/` (incl. `database/audio/` and `database/audio/track/`). Same correctness model as §5 (no `findit-proto` dependency): correctness = wire round-trip + JSON round-trip (under `json`) + quickcheck property + `mediatime`-extern round-trip, plus faithful authoring verified against the surveyed source as read-only reference. All locked decisions of §2 apply unchanged.

### 6.1 Scope & package

- **SP2 `database` types live in the single `media.v1` package / `proto/media/v1/types.proto`** (no separate `findit.db.v1` package or `database.proto` file — consolidated per explicit user directive, §3). All former `media.v1.*` references by SP2 types are bare same-package references post-consolidation; no `import` or cross-package prefix is needed.
- **Reuse `media.v1.*` — same-package, never redefine.** Every SP1/SP0 type the database layer composes is a bare same-package reference. Confirmed present in the final SP1 proto `proto/media/v1/types.proto` and used by SP2:
  - `media.v1.VideoFormat` (SP1 enum) — `VideoMeta.format`.
  - `media.v1.AudioFormat` (SP1 enum) — `AudioFileRecord.format`.
  - `media.v1.CodecId` (SP1 msg) — `VideoStreamMeta.codec_id`, `SubtitleTrack.codec_id`, `AudioStreamMeta.codec_id`.
  - `media.v1.Dimensions` (SP1 msg) — `VideoMeta`, `VideoStreamMeta`, `Keyframe`, `AudioCoverArt`.
  - `media.v1.ErrorInfo` (SP1 msg) — every `index_error` slot.
  - `media.v1.Location` (SP1 msg) — `AudioCoverArt.path`.
  - `media.v1.Aesthetics`, `media.v1.HorizonInfo`, `media.v1.FeaturePrint`, `media.v1.DocumentSegment` (SP1 msgs) — `Keyframe`.
  - `media.v1.HumanAnalysis`, `media.v1.AnimalAnalysis` (SP1 aggregates) — `Keyframe.humans`/`animals`.
  - `media.v1.Detection` (SP0 msg) — `TrackTag.detections`, and (collapse, below) every `Clap` detection slot.
  - `media.v1.SubjectDetection`, `media.v1.ObjectDetection`, `media.v1.ClassificationDetection`, `media.v1.ActionDetection`, `media.v1.MoodDetection`, `media.v1.EmotionDetection`, `media.v1.LightingDetection`, `media.v1.ColorDetection`, `media.v1.TextDetection`, `media.v1.BarcodeDetection`, `media.v1.SaliencyRegion` (SP1 detectors) — `Keyframe`, `SceneVlmResult`. All confirmed by exact name in the SP1 proto. (`media.v1.SaliencyRegion` confirmed present; `Keyframe` uses it for both `attention_saliency` and `objectness_saliency`.)
  - The `Id`/`FileChecksum` convention (a 16-byte / 32-byte `bytes` field, NOT a wrapper message — matching §5's `Id`→`bytes value`, `FileChecksum`→`bytes value` decision): every `*_id`/`checksum` field below is emitted as an inline `bytes` field, *not* a `media.v1.Id`/`media.v1.FileChecksum` message reference (those SP1 messages exist but are themselves the bytes-newtype convention; SP2 fields carry the bytes directly, consistent with §5).
- **Extern (reference, never regenerate):** `mediatime.v1.{Timebase,TimeRange,Timestamp}` (already `extern_path`-mapped to `::mediatime` per §2/§3). `TrackTime` is the **SP1** message `media.v1.TrackTime` (it composes `mediatime.v1.TimeRange`) — referenced, not redefined. Bare `Timebase` fields (`VideoStreamMeta.time_base`, `AudioFileRecord.time_base`, `TrackRecord.time_base`) reference `mediatime.v1.Timebase` directly.
- **Name disambiguation (call-out):** SP2 defines **`DbMediaKind`** — the database tri-state asset enum (`MEDIA_KIND_UNSPECIFIED=0 / VIDEO=1 / AUDIO=2`). This is a **DIFFERENT type** from **`MediaKind`** (the SP1 video/audio-format `oneof`, §5 line 58). After mono-consolidation both live in the same `media.v1` package, so the package no longer disambiguates: the SP2 enum is **renamed at the proto definition** to `DbMediaKind` (same-package rename; SP1's `MediaKind` oneof keeps its name) — user directive removed the package-based disambiguation, making the rename necessary.

### 6.2 Collapse / extern decisions (clean-redesign, explicit)

- **clap.rs's 5 newtype Detection-wrappers** (`AudioDetection`, `AudioSceneDetection`, `AudioMoodDetection`, `VoiceDetection`, `SoundEvent`) are transparent single-field wrappers around `Detection` with no own fields (confirmed: `detection_wrapper!` macro emits exactly `{ detection: Detection }` encoded at tag=1). They are **NOT emitted** as messages. `Clap` references `media.v1.Detection` directly at each slot: `optional media.v1.Detection audio_detection`, `optional … scene`, `optional … mood`, `optional … voice`, `repeated media.v1.Detection sound_events`.
- **`SubtitleTrackOrigin`** is a Rust enum-with-data (`Unknown | Embedded | Sidecar | GeneratedWhisper{source_audio_track_id} | GeneratedOcr{source_subtitle_track_id}`). It becomes a **proto3 `message` with a `oneof`** (not an enum) — the discriminant `kind` is preserved as a sibling field exactly as the hand-rolled wire format encodes it (tag=1 kind varint, tag=2/3 the data ids).
- **`Iso6392B`** (transparent `u32` newtype, packed 3×ASCII) and **`CedTag`** (transparent `u64` newtype over the soundevents dataset code, wire type fixed64) carry no own message — they become **inline scalar fields at use sites**: `Iso6392B` → `uint32` (`TrackRecord.language`), `CedTag` → `fixed64` (`CedDetection.tag`, source wire type is `SixtyFourBit`).
- **`VoiceRange`** (`voice_range.rs`) has **no `Encode`/`Decode`** (pure-Rust runtime helper) → excluded entirely.
- All `*Ref`/`*Chunk` mirrors (`VideoMetaRef`/`VideoMetaChunk`/`VideoRef`/`VideoChunk`/`SceneRef`/`SceneChunk`/`KeyframeRef`/`KeyframeChunk`/`AnimalAnalysisRef`/`HumanAnalysisRef`), the `define_code_type!`/`bitflags!`/quickcheck/test/wire-const helper modules carry no own schema → excluded (buffa generates owned+view itself).

### 6.3 Owned enums (SP2-owned; `*_UNSPECIFIED=0` inserted or renamed per §2)

Source `Unknown=0`/`Unspecified=0` ⇒ `*_UNSPECIFIED=0` is a **rename, not an insertion** (logical order preserved). `VideoFormat`/`AudioFormat` are **NOT** owned by SP2 (reuse `media.v1`).

| Enum | Values (discriminant from source `from_u32`/`to_u32`/`#[repr]`) |
|---|---|
| `MediaKind` | `MEDIA_KIND_UNSPECIFIED=0` (src `Unknown`), `MEDIA_KIND_VIDEO=1`, `MEDIA_KIND_AUDIO=2` |
| `SubtitleTrackFormat` | `…_UNSPECIFIED=0` (src `Unknown`), `TEXT=1, ASS=2, BITMAP=3, SRT=4, VTT=5, TTML=6, SAMI=7, LRC=8, WHISPER=9` |
| `SubtitleTrackRole` | `…_UNSPECIFIED=0` (src `Unknown`), `SUBTITLE=1, CAPTION=2, TRANSCRIPT=3, TRANSLATION=4, LYRICS=5, COMMENTARY=6` |
| `ChannelLayoutKind` | `…_UNSPECIFIED=0` (src `Unknown`), `MONO=1, STEREO=2, STEREO_DOWNMIX=3, SURROUND=4, QUAD=5, HEXAGONAL=6, OCTAGONAL=7, HEXADECAGONAL=8, CUBE=9, CH2_1=10, CH2_1_ALT=11, CH2_2=12, CH3_1=13, CH3_1_2=14, CH4_0=15, CH4_1=16, CH5_0=17, CH5_0_BACK=18, CH5_1=19, CH5_1_BACK=20, CH5_1_2_BACK=21, CH5_1_4_BACK=22, CH6_0=23, CH6_0_FRONT=24, CH6_1=25, CH6_1_BACK=26, CH6_1_FRONT=27, CH7_0=28, CH7_0_FRONT=29, CH7_1=30, CH7_1_WIDE=31, CH7_1_WIDE_BACK=32, CH7_1_TOP_BACK=33, CH7_1_2=34, CH7_1_4_BACK=35, CH7_2_3=36, CH9_1_4_BACK=37, CH22_2=38` |
| `AudioChannelOrderKind` | `…_UNSPECIFIED=0` (src `Unspecified`), `NATIVE=1, CUSTOM=2, AMBISONIC=3` |
| `AudioClipKind` | `…_UNSPECIFIED=0` (src `Unknown`), `WHOLE_TRACK_SUMMARY=1, VIDEO_SCENE_ALIGNED=2, FIXED_WINDOW=3, EVENT_SPAN=4` |
| `AudioPrefilterClass` | `…_UNSPECIFIED=0` (src `Unknown`), `CONTENT=1, SILENT=2, NOISE=3` |
| `AudioTrackRole` | `…_UNSPECIFIED=0` (src `Unknown`), `MAIN_PROGRAM=1, COMMENTARY=2, DUB=3, DESCRIPTIVE_AUDIO=4, KARAOKE=5, LYRICS=6` |
| `AudioContainerFormat` | `…_UNSPECIFIED=0` (inserted; src `define_code_type!` 0=None), `MP4=1, MKV=2, MOV=3, WAV=4, MP3=5, FLAC=6, OGG=7, MKA=8, WMA=9, AAC=10` |
| `AudioCodec` | `…_UNSPECIFIED=0` (inserted), `AAC=1, FLAC=2, OPUS=3, MP3=4, PCM_S16LE=5, AC3=6, VORBIS=7` |
| `AudioSampleFormat` | `…_UNSPECIFIED=0` (inserted), `FLTP=1, S16=2, S32=3, F32=4` |
| `TrackClassificationType` | `…_UNSPECIFIED=0` (src `Unknown`), `TIMECODE=1, SILENT=2, AMBIENCE=3, VOICE=4, MUSIC=5, SOUND_EFFECT=6, MIXED=7` |

### 6.4 Inline `uint32` bitflags (documented bit layout in proto comments — NOT standalone types)

Per §2, every `bitflags!` becomes a single inline `uint32` field at its use site, with the bit layout recorded in a comment. Bit values from source:

- `VideoIndexStatus` (video.rs): `PROBED=0x01, SCENE_DETECTED=0x02, KEYFRAME_EXTRACTED=0x04, VLM_ANALYZED=0x10, APPLE_VISION_ANALYZED=0x20, TEXT_EMBEDDING_FINISHED=0x40, SCENE_EMBEDDING_FINISHED=0x80` (note source gap at 0x08).
- `VideoTrackDisposition` / `SubtitleTrackDisposition` / `AudioTrackDisposition` (identical layout): `DEFAULT=0x1, DUB=0x2, ORIGINAL=0x4, COMMENT=0x8, LYRICS=0x10, KARAOKE=0x20, FORCED=0x40, HEARING_IMPAIRED=0x80, VISUAL_IMPAIRED=0x100, CLEAN_EFFECTS=0x200, ATTACHED_PIC=0x400, TIMED_THUMBNAILS=0x800, NON_DIEGETIC=0x1000, CAPTIONS=0x10000, DESCRIPTIONS=0x20000, METADATA=0x40000, DEPENDENT=0x80000, STILL_IMAGE=0x100000, MULTILAYER=0x200000`.
- `MediaIndexStatus` (media.rs): `PROBED=0x01, VIDEO_INDEXED=0x02, AUDIO_INDEXED=0x04, SUBTITLE_INDEXED=0x08`.
- `SubtitleIndexStatus` (subtitle.rs): `TRACKS_DISCOVERED=0x01, CUES_EXTRACTED=0x02, OCR_DONE=0x04, SEARCH_INDEXED=0x08`.
- `AudioIndexStatus` (kinds.rs): `PROBED=0x01, ANALYZED_LOCAL=0x02, TRANSCRIPTED=0x04, GEMINI_ENHANCED=0x08, AUDIO_EMBEDDING_FINISHED=0x10, DESCRIPTION_EMBEDDING_FINISHED=0x20`.
- `ProcessingStage` (code_types.rs): `EXTRACTED=0x01, CLASSIFIED=0x02, VAD_DONE=0x04, STT_DONE=0x08, SPEAKER_DONE=0x10, LLM_DONE=0x20, TEXT_EMBED=0x40, CED_DONE=0x80, CLAP_DONE=0x100, EBUR128_DONE=0x200, FPRINT_DONE=0x400`.
- `StopReason` (code_types.rs): `TIMECODE=0x01, SILENT=0x02, FLATNESS=0x04, NO_SPEECH=0x08, LLM_NO_NET=0x10, LLM_NO_CREDIT=0x20`.

### 6.5 Authoritative message mapping (`proto_type proto_name = tag;`)

Field tags/wire types are taken from each file's explicit `*_IDENT`/`FieldTag::new(n)`/`WireType` constants. Source wire-type notes: `ThirtyTwoBit` on packed `Dimensions` → reuse `media.v1.Dimensions` (split fields, per §5); `f32`/`f64` `ThirtyTwoBit`/`SixtyFourBit` → `float`/`double`; transparent-int `fixed64` (`CedTag`) → `fixed64`; varint int-newtypes → `uint32`. `[D]`=`media.v1.Detection`. Source tag gaps → `reserved`.

**Non-audio (`video.rs`, `video_track.rs`, `media.rs`, `failed_file.rs`, `scene.rs`, `scene_vlm.rs`, `keyframe.rs`, `subtitle.rs`):**

| Message | Fields |
|---|---|
| `VideoMeta` | `bytes id=1; reserved 2; string name=3; media.v1.VideoFormat format=4; media.v1.Dimensions dimensions=5; uint64 size=6; media.v1.TrackTime time=7; double frame_rate=8; uint64 bit_rate=9; int64 created_at=10` |
| `Video` | `VideoMeta meta=1; repeated bytes scenes=2; reserved 3 to 7; uint32 index_status=8; optional media.v1.ErrorInfo index_error=9; reserved 10; uint32 error_status=11` |
| `VideoTrackMeta` | `bytes id=1; uint32 ordinal=2; uint32 stream_index=3; optional uint64 container_track_id=4; media.v1.TrackTime time=5` |
| `VideoStreamMeta` | `media.v1.CodecId codec_id=1; media.v1.Dimensions dimensions=2; int64 total_pts=3; double frame_rate=4; uint64 bit_rate=5; mediatime.v1.Timebase time_base=6` |
| `VideoTrack` | `VideoTrackMeta meta=1; VideoStreamMeta stream=2; uint32 disposition=3; bool is_primary=4; bool auto_selected=5; string selection_reason=6; bytes video_id=7; optional media.v1.ErrorInfo index_error=8` |
| `MediaMeta` | `bytes id=1; bytes checksum=2; string name=3; uint64 size=4; media.v1.TrackTime time=5; int64 created_at=6` |
| `Media` | `MediaMeta meta=1; MediaKind kind=2; uint32 index_status=3; optional media.v1.ErrorInfo index_error=4; optional bytes video_id=5; optional bytes audio_id=6; optional bytes subtitle_id=7; uint32 error_status=8; int64 capture_date=9; string device_make=10; string device_model=11; string gps_location=12` |
| `FailedFile` | `bytes id=1; bytes media_id=2; bytes location_id=3; int64 failed_at=4` |
| `SceneMeta` | `bytes id=1; bytes video_id=2; mediatime.v1.TimeRange range=3; int64 created_at=4; bytes video_track_id=5` |
| `Scene` | `SceneMeta meta=1; repeated bytes keyframes=2; string description=3; string shot_type=4; string camera_motion=5; string tags=6; uint32 people_count=7; repeated bytes tag_ids=8; repeated string vision_provider=9; reserved 10; repeated string smart_folders=11; reserved 12` |
| `SceneVlmResult` | `optional string scene=1; optional string description=2; repeated media.v1.SubjectDetection subjects=3; repeated media.v1.ObjectDetection objects=4; repeated media.v1.ActionDetection actions=5; repeated media.v1.MoodDetection mood=6; optional string shot_type=7; repeated media.v1.LightingDetection lighting=8; repeated media.v1.ColorDetection colors=9; repeated string tags=10; repeated media.v1.ClassificationDetection classifications=11` (pure-Rust struct, no `Encode`; tags assigned in clean-redesign field order — see 6.8) |
| `Keyframe` | `bytes id=1; bytes scene_id=2; int64 pts=3; media.v1.Dimensions dimensions=4; bytes data=5; repeated media.v1.ClassificationDetection classifications=6; media.v1.HumanAnalysis humans=7; media.v1.AnimalAnalysis animals=8; repeated media.v1.ObjectDetection objects=9; repeated media.v1.ActionDetection actions=10; repeated media.v1.MoodDetection mood=11; repeated media.v1.EmotionDetection emotion=12; repeated media.v1.LightingDetection lighting=13; repeated media.v1.ColorDetection colors=14; repeated media.v1.TextDetection text_detections=15; repeated media.v1.BarcodeDetection barcodes=16; repeated media.v1.SaliencyRegion attention_saliency=17; repeated media.v1.SaliencyRegion objectness_saliency=18; media.v1.HorizonInfo horizon=19; repeated media.v1.DocumentSegment document_segments=20; media.v1.FeaturePrint feature_print=21; media.v1.Aesthetics aesthetics=22` |
| `SubtitleMeta` | `bytes id=1; int64 created_at=2` |
| `Subtitle` | `SubtitleMeta meta=1; reserved 2; uint32 index_status=3; optional media.v1.ErrorInfo index_error=4` |
| `SubtitleTrackMeta` | `bytes id=1; uint32 ordinal=2; optional uint32 stream_index=3; optional uint64 container_track_id=4; media.v1.TrackTime time=5` |
| `SubtitleTrackOrigin` | `uint32 kind=1; oneof source { bytes source_audio_track_id=2; bytes source_subtitle_track_id=3; }` (Rust enum-with-data → message+oneof; `kind` mirrors src discriminant: 0=Unspecified,1=Embedded,2=Sidecar,3=GeneratedWhisper,4=GeneratedOcr) |
| `SubtitleTrack` | `SubtitleTrackMeta meta=1; bytes subtitle_id=2; SubtitleTrackOrigin origin=3; SubtitleTrackFormat format=4; SubtitleTrackRole role=5; string language=6; string title=7; media.v1.CodecId codec_id=8; uint32 disposition=9; bool is_primary=10; bool auto_selected=11; string selection_reason=12; optional media.v1.ErrorInfo index_error=13` |
| `SubtitleCue` | `bytes id=1; bytes subtitle_track_id=2; mediatime.v1.TimeRange range=3; string text=4; string language=5; optional float confidence=6; string raw_payload=7` |

**Audio leaves & layout (`audio/mod.rs` shared idents, `channel_layout.rs`, `leaves.rs`, `ced.rs`, `chromaprint.rs`, `clap.rs`, `ebur128.rs`, `timecode.rs`, `track/tag.rs`):**

| Message | Fields |
|---|---|
| `TagConfidence` | `string label=1; float confidence=2` |
| `SoundSource` | `string name=1; string prominence=2; string description=3` |
| `AudioEvent` | `string event_type=1; uint32 start_ms=2; uint32 end_ms=3; float avg_confidence=4` |
| `SpeakerSegment` | `uint32 start_ms=1; uint32 end_ms=2; uint32 speaker_id=3` |
| `AudioTranscriptSegment` | `uint32 start_ms=1; uint32 end_ms=2; string text=3; string language=4; float confidence=5` |
| `AudioChannelSpec` | `uint32 index=1; uint32 raw_id=2; string label=3` |
| `AudioChannelLayout` | `AudioChannelOrderKind order=1; uint32 channels=2; ChannelLayoutKind known_kind=3; optional uint64 native_mask=4; repeated AudioChannelSpec custom_channels=5; string description=6` |
| `CedDetection` | `fixed64 tag=1; float confidence=2` (src `CedTag` newtype inlined as `fixed64`, wire `SixtyFourBit`) |
| `Ced` | `repeated CedDetection tags=1` |
| `Chromaprint` | `bytes fingerprint=1; double fingerprint_duration=2` |
| `Clap` | `optional media.v1.Detection audio_detection=1; optional media.v1.Detection scene=2; optional media.v1.Detection mood=3; optional media.v1.Detection voice=4; repeated media.v1.Detection sound_events=5` (5 newtype wrappers collapsed → `media.v1.Detection`) |
| `Ebur128` | `float loudness_lufs=1; float loudness_range_lu=2; float true_peak_dbtp=3` |
| `Timecode` | `string start=1; string end=2; float fps=3; bool drop_frame=4` |
| `TrackTag` | `string category=1; repeated media.v1.Detection detections=2; string source=3` |

**Audio aggregates (`models.rs`, `summary.rs`, `analysis.rs`, `root.rs`, `track/mod.rs`, `track/record.rs`, `file_record.rs`):**

| Message | Fields |
|---|---|
| `AudioMeta` | `bytes id=1; reserved 2; string name=3; string container=4; uint64 size=5; media.v1.TrackTime time=6; int64 created_at=7` |
| `AudioStreamMeta` | `media.v1.CodecId codec_id=1; uint32 sample_rate=2; AudioChannelLayout layout=3; uint64 bit_rate=4; string language=5; string stream_title=6; string album=7; string artist=8; string title=9; string genre=10; uint32 track_number=11; string sample_format=12; uint32 bits_per_sample=13` |
| `AudioTrackMeta` | `bytes id=1; uint32 ordinal=2; uint32 stream_index=3; optional uint64 container_track_id=4; media.v1.TrackTime time=5` |
| `AudioSummary` | `AudioPrefilterClass prefilter_class=1; optional TagConfidence audio_type=2; optional TagConfidence scene=3; optional TagConfidence mood=4; optional TagConfidence voice=5; bool has_speech=6; bool has_music=7; string dominant_language=8; float speech_ratio=9; uint32 speaker_count=10; float loudness_lufs=11; float rms_db=12; string transcript_preview=13; uint32 clip_count=14; repeated uint32 fingerprint=15; bool gemini_enhanced=16` |
| `Audio` | `AudioMeta meta=1; repeated bytes analyses=2; reserved 3, 4; AudioSummary summary=5; uint32 index_status=6; optional media.v1.ErrorInfo index_error=7` |
| `AudioTrack` | `AudioTrackMeta meta=1; reserved 2; AudioStreamMeta stream=3; uint32 disposition=4; AudioTrackRole role=5; bool is_primary=6; bool auto_selected=7; string selection_reason=8; bytes audio_id=9; optional media.v1.ErrorInfo index_error=10` |
| `AudioAnalysis` | `bytes id=1; bytes audio_id=2; bytes track_id=7; optional bytes scene_id=3; AudioClipKind kind=4; uint32 start_ms=5; uint32 end_ms=6; repeated CedDetection ced_tags=10; optional TagConfidence zs_audio_type=20; optional TagConfidence zs_scene=21; optional TagConfidence zs_mood=22; repeated TagConfidence zs_sound_events=23; optional TagConfidence zs_voice=24; string description_en=30; string description_zh=31; string gemini_scene=32; string gemini_mood=33; repeated SoundSource gemini_sound_sources=34; string gemini_foreground=35; string gemini_background=36; bool gemini_enhanced=37; repeated AudioEvent event_timeline=40; string foreground_layer=41; string background_layer=42; float speech_ratio=50; uint32 speaker_count=51; repeated SpeakerSegment speaker_segments=52; optional TagConfidence voice_gender=60; optional TagConfidence voice_emotion=61; string transcript=70; repeated AudioTranscriptSegment transcript_segments=71; string language=72; optional TagConfidence music_genre=80; optional float music_bpm=81; repeated TagConfidence music_instruments=82; float loudness_lufs=90; float rms_db=91; optional float snr_db=92; bool has_sudden_onset=93; string energy_profile=94; float spectral_flatness=95; repeated uint32 fingerprint=100; AudioPrefilterClass prefilter_class=101; bool has_speech=110; bool has_music=111` (source has wide tag gaps 8–9,11–19,25–29,38–39,43–49,53–59,62–69,73–79,83–89,96–99,102–109; treated as semantic groupings — assign exactly as source, no `reserved` needed since proto3 permits sparse field numbers and there is no wire-compat constraint; this preserves the source's deliberate numbering bands) |
| `TrackTag` | (see leaves table above) |
| `TrackRecord` | `bytes id=1; bytes audio_id=2; uint32 track_index=3; AudioCodec codec=4; AudioSampleFormat sample_format=5; uint32 sample_rate=6; uint32 channels=7; ChannelLayoutKind channel_layout=8; uint64 bit_rate=9; uint32 bit_depth=10; int64 total_pts=11; mediatime.v1.Timebase time_base=12; uint32 language=13; optional Timecode timecode=14; TrackClassificationType classification=15; uint32 stop_reason=16; reserved 17; repeated TrackTag tags=18; optional Ebur128 ebur_128=1000; optional Clap clap=1001; optional Ced ced=1002; optional Chromaprint chromaprint=1003; uint32 index_status=2000; optional media.v1.ErrorInfo index_error=2001; uint32 error_status=2002` (`language`=`Iso6392B` newtype inlined as `uint32`; source banded tags 1000s/2000s preserved exactly; gap at 17 reserved) |
| `AudioCoverArt` | `optional media.v1.Location path=1; string mime=2; optional media.v1.Dimensions dimensions=3; uint32 size=4` |
| `AudioFileRecord` | `bytes id=1; optional bytes checksum=2; string name=3; media.v1.AudioFormat format=4; uint64 size=5; int64 total_pts=6; double frame_rate=7; uint64 bit_rate=8; mediatime.v1.Timebase time_base=9; AudioContainerFormat container_format=10; uint32 stream_count=11; string title=12; string artist=13; string album_artist=14; string album=15; string genre=16; string composer=17; string performer=18; string date=19; uint32 track_number=20; uint32 total_tracks=21; uint32 disc_number=22; uint32 total_discs=23; string comment=24; string lyrics=25; repeated string tag_types=26; optional AudioCoverArt cover_art=27; reserved 28 to 30; int64 created_at=31` |

### 6.6 Implementation batches (dependency-ordered; one implementer task + two-stage review each; each batch = author proto into `proto/media/v1/types.proto` (package `media.v1`, same-package — mono-consolidation) + `xtask gen` + extend `src/lib.rs` named re-exports + round-trip/JSON/quickcheck tests)

1. **DB enums + bitflag carriers (no message deps):** all 12 owned enums of 6.3 (`MediaKind, SubtitleTrackFormat, SubtitleTrackRole, ChannelLayoutKind, AudioChannelOrderKind, AudioClipKind, AudioPrefilterClass, AudioTrackRole, AudioContainerFormat, AudioCodec, AudioSampleFormat, TrackClassificationType`). (Bitflags are inline `uint32` — no standalone type; documented in 6.4, materialized at use sites in later batches.)
2. **Audio scalar leaves:** `TagConfidence, SoundSource, AudioEvent, SpeakerSegment, AudioTranscriptSegment, AudioChannelSpec, Chromaprint, Ebur128, Timecode, CedDetection, Ced`.
3. **Composite audio leaves + reuse-only wrappers:** `AudioChannelLayout` (needs `AudioChannelSpec`+enums), `Clap` (reuses `media.v1.Detection`), `TrackTag` (reuses `media.v1.Detection`).
4. **Non-audio meta blocks:** `VideoMeta, VideoTrackMeta, VideoStreamMeta, MediaMeta, SceneMeta, SubtitleMeta, SubtitleTrackMeta, SubtitleTrackOrigin, FailedFile`.
5. **Non-audio track/record wrappers:** `Video, VideoTrack, Media, Subtitle, SubtitleTrack, SubtitleCue`.
6. **Scene + VLM:** `Scene, SceneVlmResult`.
7. **Keyframe (own batch — large, reuse-heavy):** `Keyframe` (composes 16 `media.v1.*` detector/aggregate types + `media.v1.{Dimensions,HorizonInfo,FeaturePrint,Aesthetics}`).
8. **Audio meta + summary blocks:** `AudioMeta, AudioStreamMeta, AudioTrackMeta, AudioSummary`.
9. **Audio aggregates (own/small batch — `AudioAnalysis` and `TrackRecord` are large):** `AudioAnalysis` (≈40 fields); `TrackRecord` (banded tags, composes `Timecode/Ebur128/Clap/Ced/Chromaprint/TrackTag`); `Audio`, `AudioTrack`.
10. **Audio file record (own/small batch):** `AudioCoverArt` then `AudioFileRecord` (composes `AudioCoverArt`, reuses `media.v1.{AudioFormat,Location,Dimensions}` + `mediatime.v1.Timebase`).

Totals: **42 owned messages** (40 from the directive inventory + the 2 file-record discrepancies in 6.8), **12 owned enums**, **10 batches**.

### 6.7 Excluded types (with reasons)

- **clap.rs 5 wrappers** `AudioDetection`/`AudioSceneDetection`/`AudioMoodDetection`/`VoiceDetection`/`SoundEvent` — transparent single-`Detection` newtypes, no own fields → collapsed to `media.v1.Detection` at `Clap` slots.
- **`CedTag`** (`u64` transparent newtype, wire fixed64) → inline `fixed64` at `CedDetection.tag`.
- **`Iso6392B`** (`u32` transparent newtype) → inline `uint32` at `TrackRecord.language`.
- **`VoiceRange`** — no `Encode`/`Decode` (pure-Rust runtime sample-range helper) → excluded.
- **All `*Ref`/`*Chunk`** ownership/zero-copy mirrors (`VideoMetaRef/Chunk`, `VideoRef/Chunk`, `SceneRef/Chunk`, `KeyframeRef/Chunk`, plus the SP1 `AnimalAnalysisRef`/`HumanAnalysisRef` used only by `KeyframeRef`) — same schema, different ownership; buffa generates owned+view itself.
- **`MediaKind`/enum helper impls, `define_code_type!`/`bitflags!` macro bodies, `MergeLengthDelimitedItem`/`decode_repeated_item!`/`encode_str!` and all wire-const (`*_IDENT`) modules, `quickcheck`/`tests` modules** — pure-Rust helpers / encoding conventions, no own message.
- **SP1/SP0 types and `mediatime.v1.*`** — same-package (SP1/SP0) or extern (`mediatime.v1`), never redefined in the SP2 section of `proto/media/v1/types.proto` (see 6.1).

### 6.8 Ambiguities resolved (no TBDs)

1. **Inventory said ~40 messages; survey found 42.** `file_record.rs` contributes **`AudioFileRecord`** and **`AudioCoverArt`** — both have full hand-rolled `Encode`/`Decode` and own fields, so both are owned messages (the directive's batch guidance flagged "AudioFileRecord-if-present"; it is present). They are added to the inventory and to batch 10. **Resolution:** include both as owned messages; total = 42.
2. **`SceneVlmResult` has no `Encode`/`Decode`** (pure-Rust struct, `scene_vlm.rs`) — unlike `VoiceRange` it has *real owned fields* composed of migrated detection types and is in the directive's authoritative inventory, so it is a kept message. Its source has no `*_IDENT` tags; **resolution:** assign field tags 1..11 in struct-declaration order (a clean-redesign choice — §2 grants this latitude; no wire-compat to honor). Stated explicitly in 6.5.
3. **`Audio` reserved 3,4 / `Video` reserved 3–7,10 / `Scene` reserved 10,12 / `VideoMeta` reserved 2 / `AudioMeta` reserved 2 / `AudioTrack` reserved 2 / `Subtitle` reserved 2 / `TrackRecord` reserved 17 / `AudioFileRecord` reserved 28–30.** Source `Decode` explicitly `skip_field`s or simply omits these tags. **Resolution:** emit `reserved` for the *bounded interior* gaps so the historical numbering is documented and future fields cannot silently reuse them. `AudioAnalysis`'s *wide banded* gaps (8–9, 11–19, 25–29, …) are deliberate semantic bands, not accidental holes; per §2 (no wire-compat) proto3 sparse field numbers are emitted as-is **without** `reserved` (reserving dozens of unused numbers adds noise with no correctness value); the band structure is preserved by using the source numbers verbatim. Same rationale for `TrackRecord`'s 1000s/2000s bands.
4. **`Iso6392B` source wire type is varint `uint32`; `CedTag` is fixed64.** §2 says transparent int-newtypes become idiomatic proto scalars at use sites. **Resolution:** `Iso6392B`→`uint32` (varint, matches source `TR_LANGUAGE_IDENT` = `Varint`); `CedTag`→`fixed64` (matches source `CED_DETECTION_TAG_IDENT` = `SixtyFourBit`, and the dataset code is a dense 64-bit value where fixed64 is idiomatic).
5. **Bare `Timebase` vs `media.v1.TrackTime`.** `VideoStreamMeta`/`AudioFileRecord`/`TrackRecord` carry a raw `Timebase` (source encodes it via `Timebase::encode`/`decode`, an extern); other meta types carry `TrackTime`. **Resolution:** raw-`Timebase` fields → `mediatime.v1.Timebase` (extern, §2); `time` fields → `TrackTime` (the SP1 message that itself wraps `mediatime.v1.TimeRange` — bare same-package ref post-consolidation). Neither is redefined in the SP2 section.
6. **`media.v1.MediaKind` vs `findit.db.v1.MediaKind` collision (original two-package framing).** *Superseded by mono-consolidation (§6.8 new item below) — under the single `media.v1` package the SP2 enum is renamed to `DbMediaKind` at the proto definition; see that item for the current authoritative resolution.*
7. **`Dimensions` on the wire is packed `ThirtyTwoBit` (`from_u32`/`to_u32`) in `keyframe.rs`/`video*.rs`/`file_record.rs`'s `AudioCoverArt`.** §5 already migrated `media.v1.Dimensions` as split `uint32 width/height`. **Resolution:** reference `media.v1.Dimensions` everywhere (no SP2 redefinition); the packed↔split conversion is a clean-redesign per §2/§5 (correctness = round-trip, not byte-compat).
8. **Mono-consolidation (user directive) — supersedes the §6.1 / §3 two-package framing for SP2.** The 3-package split (`media.v1` SP0+SP1 / `findit.db.v1` SP2 / `findit.net.v1` SP3) was **consolidated into a single `media.v1` package in `proto/media/v1/types.proto`** with no cross-package refs, per explicit user directive. This is a structural refactor — no field/tag/enum/message content changed; only the package partition and the 2 collision names. Specifically for SP2: the `findit.db.v1` package and `database.proto` file are abolished; all SP2 types are authored directly into `proto/media/v1/types.proto` as `media.v1`-package members; all former cross-package `media.v1.*` refs by SP2 types become bare same-package refs. The `findit.db.v1.MediaKind`→`DbMediaKind` same-package rename is required (§6.1 name-disambiguation call-out above); SP1's `MediaKind` oneof keeps its name unchanged. The cross-package codegen mechanism (native `super::` resolution via `buffa_codegen::context::TypePath`) is no longer exercised by SP2 (all same-package); however, `mediatime.v1` remains the sole extern (`extern_path(".mediatime.v1","::mediatime")`) — unchanged. The xtask `.files()` single-entry change (SP2 added a second entry for `database.proto`) is reversed; the single `proto/media/v1/types.proto` entry covers all packages. The `Sp2CodegenSmoke` fixture is retained as a permanent regression guard but is now a plain same-package `media.v1` message (no cross-package refs in its fields — the `media.v1.ErrorInfo` and `media.v1.VideoFormat` refs become bare same-package refs; the `mediatime.v1.TimeRange` extern ref is unchanged).

## 7. SP3 — `network` (findit.net.v1)

Authored from a full survey of `findit-proto/src/network/` (all 17 files: `mod.rs`, `shared.rs`, `folder.rs`, `event.rs`, `file.rs`, `browse.rs`, `search.rs`, `model.rs`, `daemon.rs`, `progress.rs`, `annotation.rs`, `heartbeat.rs`, `volume_eject.rs`, `error.rs`, `framing.rs`, `async_framing.rs`). Same correctness model as §5/§6 (no `findit-proto` dependency): correctness = wire round-trip + JSON round-trip (under `json`) + quickcheck property + `mediatime`-extern round-trip, plus faithful authoring verified against the surveyed source as read-only reference. All locked decisions of §2 apply unchanged.

### 7.1 Scope & package

- **SP3 `network` types live in the single `media.v1` package / `proto/media/v1/types.proto`** (no separate `findit.net.v1` package or `network.proto` file — consolidated per explicit user directive, §3). No cross-package refs anywhere: all former `media.v1.*`, `findit.db.v1.*`, and `findit.net.v1.*` references within SP3 types are bare same-package references post-consolidation. The xtask `.files()` single entry (`proto/media/v1/types.proto`) covers everything; no additional entry for `network.proto`. No `import` statements needed.
- **`mediatime.v1.*` stays extern → `::mediatime`** (`.extern_path(".mediatime.v1", "::mediatime")`, unchanged; never regenerated; no new `mediatime.v1*` generated file).
- **Reuse `media.v1.*` and former `findit.db.v1.*` types — same-package, never redefine.** Every SP1/SP2 type the network layer composes is a bare same-package reference. Confirmed present by exact name in the final SP1 proto `proto/media/v1/types.proto` and used by SP3:
  - `media.v1.Location` (SP1 msg, line 96) — `SearchHit.location`, `FailedFile.location`, `FolderUpdatedEvent.folder_location`, `IndexingFile.location`, `GetLocationStatsRequest.location`, `RemoveLocationRequest.location`, `RetryFailedRequest.location`, `IndexingProgressResponse.location`.
  - `media.v1.LocationTarget` (SP1 msg, line 101) — `IndexLocationRequest.target`.
  - `media.v1.Dimensions` (SP1 msg, line 40) — `SearchHit.dimensions` (resolution 7).
  - `media.v1.ErrorInfo` (SP1 msg, line 115) — `FailedFile.error`, `GetFileIndexingStatsResponse.error`, and the `Response` oneof's error arm (resolution 4).
  - `media.v1.VolumeMeta` (SP1 msg, line 138) — `Volume.meta`, `VolumeStateChangedEvent.volume`.
  - `media.v1.WatchedLocation` (SP1 msg, line 118) — `Volume.folders` (repeated), `IndexLocationResponse.folder`.
  - `media.v1.Tag` (SP1 msg, line 112) — `UpdateAnnotationRequest.user_tags` (repeated).
  - `VideoMeta` (SP2 msg, formerly `findit.db.v1.VideoMeta`) — `BrowseItem.meta`.
  - `Video` (SP2 msg, formerly `findit.db.v1.Video`) — `GetIndexedFileResponse.video`.
  - `Scene` (SP2 msg, formerly `findit.db.v1.Scene`) — `GetIndexedFileResponse.scenes` (repeated).
  - `mediatime.v1.TimeRange` (extern → `::mediatime`) — `SearchHit.range` (source `TimeRange`, the extern that itself carries `Timebase`; SP1 maps bare `TimeRange` to this extern, §2/§5).
  - The `Id`/`FileChecksum` convention (a 16-byte / 32-byte `bytes` field, NOT a wrapper message — matching §5's `Id`→`bytes value`, `FileChecksum`→`bytes value` decision, and SP2 §6.1's inline-bytes rule): every `*_id`/`checksum`/`scene_ids` field below is emitted as an inline `bytes` (or `repeated bytes`) field, *not* an `Id`/`FileChecksum` message reference. Applies to `SearchHit.{scene_id,video_id}`, `SearchResponse.search_id`, `EjectVolumeRequest.volume_id`, `GetIndexedFileRequest.checksum`, `GetFileIndexingStatsRequest.video_id`, `GetFileIndexingStatsResponse.video_id`, `UpdateAnnotationRequest.scene_ids` (`repeated bytes`), `FailedFilesResponse.location_id`.
- **Name disambiguation (call-out):** SP3's `network` types introduce one name that collides with an SP2 name in the same package: `FailedFile` is **owned by both** SP2 (the DB record `{id, media_id, location_id, failed_at}`, §6.5) *and* SP3 (the frontend wire type `{kind, location, error, error_status, index_status}`, `shared.rs`). Under the single `media.v1` package, the SP3 one is **renamed at the proto definition** to `NetFailedFile` (same-package rename; SP2's `FailedFile` keeps its name) — user directive removed the package-based disambiguation, making the rename necessary. SP3 does **not** reference SP2's `FailedFile` in its own fields (each is independently named at its own definition).

### 7.2 Collapse / extern / transport-exclusion decisions (clean-redesign, explicit)

- **`error.rs::ErrorResponse` is NOT a new message.** It is a thin single-field wrapper `{ error: ErrorInfo }` encoded at tag=1 (confirmed: only `merge_field` arm is `(LD,1) => self.error`). Per the §6.2/§6.8 thin-wrapper-collapse precedent, the `Response` oneof's error arm references **`ErrorInfo` directly** (bare same-package ref post-consolidation) at tag **20000** (remapped from the source `ErrorResponse` discriminant 19999 — protobuf reserves field numbers 19000–19999; §7.8). No `NetErrorResponse` is emitted.
- **`SearchHit.dimensions`** is encoded on the wire as packed `ThirtyTwoBit` via `dimensions.to_u32()` (source `SH_DIMENSIONS_IDENTIFIER` = `ThirtyTwoBit`, tag=9). Per resolution 7 / §5 / SP2 §6.8 #7 (`AudioCoverArt.dimensions` precedent), it references **`Dimensions`** (the SP1 split `uint32 width=1; height=2` message — bare same-package ref); the packed↔split conversion is a clean-redesign (correctness = round-trip, not byte-compat). No SP3 redefinition.
- **`SearchHit.range`** is source `TimeRange` (the `mediatime` extern; `SH_RANGE_IDENTIFIER` = `LengthDelimited`, tag=7) → references `mediatime.v1.TimeRange` (extern → `::mediatime`), never redefined.
- **Transport plumbing — EXCLUDED entirely (resolution 3):**
  - **`Header`** (`#[repr(C)]` 24-byte binary frame: `magic[8] version reserved[3] msg_type flags payload_len pad[4]`) — a fixed-layout on-wire frame header, not a proto3 message; no `Encode`/`Decode` (only `from_bytes`/`to_array`). Excluded.
  - **`MessageType`** (`#[repr(u16)] #[non_exhaustive]` wire-dispatch enum, sparse values 0/1/2/500/501/1000…20002) — it is the frame-dispatch discriminant. It is **not** emitted as a proto `enum`; instead its integer values become the **`oneof` arm tag numbers** of `Request`/`Response`/`Event` (resolution 1). Excluded as a standalone type.
  - **`MessageFlags`** (`bitflags!` struct, **currently empty** — zero flags defined) — a header field with no carrier once `Header` is excluded, and no bits to document. Excluded (no inline `uint32` either — there is no message that carries it).
  - **`RequestId`** (`#[repr(transparent)] struct RequestId(u64)` newtype) — a transparent u64 newtype; per §2 (transparent newtype → inline scalar at use sites) it becomes an inline **`uint64`** on the two envelope messages (resolution 2), not a wrapper type.
  - **`framing.rs`** + **`async_framing.rs`** (length-prefixed batch stream codec: `encode`/`decode`/`stream_read`/`stream_write`/async `read`/`write`) — pure transport I/O over `Encode`/`Decode`, no message types. Excluded.
  - All `Encode`/`Decode`/`Arbitrary` impls, the `define_oneof!` macro body, all `*_IDENTIFIER`/`*_IDENT` wire-const modules, `#[cfg(test)] mod tests`, `#[cfg(any(test, feature="quickcheck"))]` blocks — pure-Rust encoding conventions / test code, no own schema (buffa regenerates encode/decode/arbitrary itself).
- **All `*Ref`/`*Chunk` mirrors** (`SearchFilterRef/Chunk`, `SearchHitRef/Chunk`, `BrowseItemRef/Chunk`, `ModelInfoRef/Chunk`, `ModelDownloadProgressRef/Chunk`, `IndexingFileRef/Chunk`, every `*RequestRef/Chunk`, `*ResponseRef/Chunk`, `*EventRef/Chunk`, `VolumeRef/Chunk`, `ErrorResponseRef/Chunk`, `RequestMessage`/`ResponseMessage` type-aliases) — same schema, different ownership / encode-only borrow / type alias; buffa generates owned+view itself. Excluded.

### 7.3 Owned enums (SP3-owned)

**None.** SP3 owns **zero** proto enums.

Rationale (stated per directive resolution-5 expectation, with the correction): the directive anticipated ≈5 SP3-owned enums (`ModelStatus`, `ModelDownloadStatus`, `FileKind`, `VolumeEvent`, `FolderEvent`) requiring `*_UNSPECIFIED=0` insertion + value shift. **Survey finding (discrepancy — see 7.8 #2): none of these exist as Rust `enum`s or `#[repr]`/newtype int-enums.** Every one is a **bare `u32` struct field** whose enumerated meaning is documented only in a `///` doc-comment table:
- `ModelInfo.status: u32` — doc: `0=not_downloaded, 1=downloading, 2=ready, 3=error` (`shared.rs`).
- `ModelDownloadProgress.status: u32` — doc: `0=pending, 1=downloading, 2=paused, 3=completed, 4=error` (`shared.rs`). *(Distinct semantics from `ModelInfo.status`; the survey's "may be same or distinct" question is resolved: distinct value sets, but both are plain `u32` — no enum either way, so the distinction is documentary only.)*
- `FailedFile.kind: u32` — doc: `0=video, 1=audio` (`shared.rs`).
- `VolumeStateChangedEvent.event: u32` — doc: `0=mounted, 1=unmounted, 2=ejected` (`event.rs`).
- `FolderUpdatedEvent.event: u32` — doc: `0=file_created, 1=file_modified, 2=file_removed` (`event.rs`).

§2's "model `#[repr]`/newtype int-'enums' as proto3 `enum`s" rule is scoped to types that are *Rust enums or int newtypes* (e.g. SP1 `VideoFormat`, SP2 `AudioCodec`). These five are neither — they are unconstrained `u32` accumulators. Promoting them to proto `enum`s would (a) be speculative typing the source deliberately avoided (YAGNI — mirrors resolution 8's "avoid speculative optionality"), and (b) **risk lossy round-trip**: a `u32` value outside the documented set must round-trip exactly (the migration's correctness criterion), which a closed proto `enum` does not guarantee at the JSON boundary. **Resolution: each maps to plain `uint32`, with the documented enum semantics reproduced verbatim in the proto field comment** (self-contained, SP2 bit-layout-comment convention). No `*_UNSPECIFIED`, no value shift, no SP3 enum. (`VideoIndexStatus` is likewise not an SP3 type — see 7.4.)

### 7.4 Inline `uint32` bitflags (documented bit layout in proto comments — NOT a standalone type)

Per §2, every `bitflags!` becomes a single inline `uint32` field at its use site, with the bit layout recorded in a self-contained comment (a full listing, not a cross-reference — SP2 §6.4 convention).

- **`VideoIndexStatus`** is **not owned by SP3** — it is the SP2 `database` bitflag (defined in `database/video.rs`, surveyed in §6.4) reused by the network layer. Source `network` files import `crate::database::VideoIndexStatus` and store it in `FailedFile.{error_status,index_status}`, `IndexingFile.completed_phases`, `GetFileIndexingStatsResponse.index_status` (all `ThirtyTwoBit` wire, via `from_bits_retain`/`bits()`). Since §2/§6.4 already render `VideoIndexStatus` as an **inline `uint32`** (NOT a message/enum/standalone type), SP3 simply emits an inline `uint32` field at each use site with the **same bit layout documented inline** (full listing, identical to SP2 §6.4): `PROBED=0x01, SCENE_DETECTED=0x02, KEYFRAME_EXTRACTED=0x04, VLM_ANALYZED=0x10, APPLE_VISION_ANALYZED=0x20, TEXT_EMBEDDING_FINISHED=0x40, SCENE_EMBEDDING_FINISHED=0x80` (source gap at 0x08). No cross-package reference is needed or possible (it is not a type in any package); the bit contract is duplicated by value, consistent with the §2 "self-contained comment, a full listing, not a cross-reference" rule.
- `MessageFlags` is **not** a bitflag carrier here — it is empty and its only container (`Header`) is excluded (7.2); no inline `uint32` is emitted for it.

### 7.5 Authoritative message mapping (`proto_type proto_name = tag;`)

Field tags/wire types are taken from each file's explicit `*_IDENTIFIER`/`*_IDENT` = `Identifier::new(WireType::_, FieldTag::new(n))` constants and `merge_field` arms. Source wire-type notes: `ThirtyTwoBit` f32 → `float`; `ThirtyTwoBit` packed `Dimensions` (`to_u32`) → `media.v1.Dimensions` (split, §5); varint `i64` → `int64`; varint `u64`/`u32`/`u16` → `uint64`/`uint32`/`uint32`; `Id`/`FileChecksum` `LengthDelimited` → inline `bytes`; bare-`u32` doc-enum fields → `uint32` (7.3); `VideoIndexStatus` `ThirtyTwoBit` → inline `uint32` (7.4). No source tag gaps occur in any owned message (all are contiguous from 1) → **no `reserved` needed** in SP3. `[Loc]`=`media.v1.Location`; reuse refs fully qualified.

**`shared.rs` (leaves):**

| Message | Fields |
|---|---|
| `Pagination` | `uint32 limit=1; uint32 offset=2` (client default `limit=50` when absent — documented client-side convention per resolution 8, NOT `optional`; `offset` default 0) |
| `SearchFilter` | `string key=1; string value=2; float weight=3` (client default `weight=1.0` when absent — resolution 8; plain `float`, not `optional`) |
| `SearchHit` | `bytes scene_id=1; bytes video_id=2; string video_name=3; media.v1.Location location=4; string description=5; float score=6; mediatime.v1.TimeRange range=7; bytes thumbnail=8; media.v1.Dimensions dimensions=9` (`scene_id`/`video_id`=inline `bytes`; `range`=extern; `dimensions`=SP1 split, resolution 7) |
| `BrowseItem` | `findit.db.v1.VideoMeta meta=1; media.v1.Location location=2; uint32 scene_count=3; bytes thumbnail=4` (`scene_count` src `u16` varint → `uint32`; `meta` = SP2 cross-package ref) |
| `ModelInfo` | `string name=1; uint32 status=2; uint64 size_bytes=3` (`status` doc-enum 0=not_downloaded/1=downloading/2=ready/3=error → plain `uint32`, 7.3) |
| `ModelDownloadProgress` | `string name=1; float progress=2; uint64 downloaded_bytes=3; uint64 total_bytes=4; uint32 status=5; string error_msg=6` (`status` doc-enum 0=pending/1=downloading/2=paused/3=completed/4=error → plain `uint32`, 7.3; `progress` 0.0–1.0) |
| `FailedFile` | `uint32 kind=1; media.v1.Location location=2; media.v1.ErrorInfo error=3; uint32 error_status=4; uint32 index_status=5` (`kind` doc-enum 0=video/1=audio → plain `uint32`, 7.3; `error_status`/`index_status` = inline `uint32` VideoIndexStatus, 7.4) |
| `IndexingFile` | `media.v1.Location location=1; string name=2; uint32 completed_phases=3` (`completed_phases` = inline `uint32` VideoIndexStatus, 7.4) |

**`folder.rs`:**

| Message | Fields |
|---|---|
| `ListLocationsRequest` | *(empty — zero fields)* |
| `Volume` | `media.v1.VolumeMeta meta=1; repeated media.v1.WatchedLocation folders=2` |
| `ListLocationsResponse` | `repeated Volume groups=1` |
| `GetLocationStatsRequest` | `media.v1.Location location=1` |
| `GetLocationStatsResponse` | `uint64 total_files=1; uint64 indexed_files=2; uint64 total_videos=3; uint64 total_scenes=4; uint64 total_audios=5; repeated FailedFile failed_files=6` |
| `IndexLocationRequest` | `media.v1.LocationTarget target=1` |
| `IndexLocationResponse` | `media.v1.WatchedLocation folder=1` |
| `RemoveLocationRequest` | `media.v1.Location location=1` |
| `RemoveLocationResponse` | *(empty — zero fields)* |
| `RetryFailedRequest` | `media.v1.Location location=1` |
| `RetryFailedResponse` | *(empty — zero fields)* |
| `FailedFilesResponse` | `bytes location_id=1; repeated FailedFile failed_files=2` (`location_id` = inline `bytes` Id) |

**`heartbeat.rs`, `volume_eject.rs`, `file.rs`, `daemon.rs`, `search.rs`, `browse.rs`, `model.rs`, `progress.rs`, `annotation.rs`:**

| Message | Fields |
|---|---|
| `HeartbeatRequest` | `int64 timestamp=1` (Unix ms) |
| `HeartbeatResponse` | `int64 timestamp=1` (Unix ms) |
| `EjectVolumeRequest` | `bytes volume_id=1` (inline `bytes` Id) |
| `EjectVolumeResponse` | *(empty — zero fields)* |
| `GetIndexedFileRequest` | `bytes checksum=1` (inline `bytes` FileChecksum) |
| `GetIndexedFileResponse` | `findit.db.v1.Video video=1; repeated findit.db.v1.Scene scenes=2` |
| `GetFileIndexingStatsRequest` | `bytes video_id=1` (inline `bytes` Id) |
| `GetFileIndexingStatsResponse` | `bytes video_id=1; uint32 index_status=2; optional media.v1.ErrorInfo error=3` (`video_id`=inline `bytes`; `index_status`=inline `uint32` VideoIndexStatus, 7.4; `error` source `Option<ErrorInfo>` → `optional media.v1.ErrorInfo`, presence per §2 — source normalizes empty→None) |
| `GetDaemonInfoRequest` | *(empty — zero fields)* |
| `GetDaemonInfoResponse` | `string version=1; int64 started_at=2; uint64 total_videos=3; uint64 total_scenes=4; uint32 active_tasks=5` |
| `SearchRequest` | `string query=1; Pagination pagination=2; repeated SearchFilter filters=3` |
| `SearchResponse` | `bytes search_id=1; uint32 total_count=2` (`search_id` = inline `bytes` Id) |
| `BrowseRequest` | `media.v1.Location location=1; Pagination pagination=2` |
| `BrowseResponse` | `repeated BrowseItem items=1; uint32 total_count=2; Pagination pagination=3` |
| `GetModelStatusRequest` | *(empty — zero fields)* |
| `GetModelStatusResponse` | `repeated ModelInfo models=1` |
| `ModelDownloadProgressResponse` | `ModelDownloadProgress model=1` (thin wrapper kept as a named message — distinct **response** role; references shared `ModelDownloadProgress`, see 7.8 #4) |
| `IndexingProgressResponse` | `media.v1.Location location=1; uint64 total_files=2; uint64 indexed_files=3; repeated IndexingFile active_files=4` |
| `UpdateAnnotationRequest` | `repeated bytes scene_ids=1; repeated media.v1.Tag user_tags=2` (`scene_ids` = `repeated bytes` Id) |
| `UpdateAnnotationResponse` | *(empty — zero fields)* |

**`event.rs`:**

| Message | Fields |
|---|---|
| `VolumeStateChangedEvent` | `media.v1.VolumeMeta volume=1; uint32 event=2` (`event` doc-enum 0=mounted/1=unmounted/2=ejected → plain `uint32`, 7.3) |
| `FolderUpdatedEvent` | `media.v1.Location folder_location=1; string path=2; uint32 event=3` (`event` doc-enum 0=file_created/1=file_modified/2=file_removed → plain `uint32`, 7.3) |
| `ModelDownloadProgressEvent` | `ModelDownloadProgress model=1` (thin wrapper kept as a named message — distinct **push/event** role; same wire shape as `ModelDownloadProgressResponse` but a separate type per its source role; references shared `ModelDownloadProgress`, see 7.8 #4) |

**`mod.rs` — the three oneof envelopes + the two correlation envelopes (resolutions 1 & 2):**

`Request`/`Response`/`Event` are Rust enums-with-data → each a proto3 `message` with a single `oneof kind` (owned; mirrors SP2 `SubtitleTrackOrigin`). The source `MessageType` discriminant integer of each variant is preserved verbatim as that arm's `oneof` field tag (sparse, semantic-band numbering up to 20002 — emitted as-is, **no `reserved`**, per §2 wide-band rule / SP2 §6.8 #3 `AudioAnalysis` precedent). Every arm enumerated from `mod.rs`'s `define_oneof!` blocks:

`message Request { oneof kind { … } }`:

| Variant (`mod.rs`) | Inner `media.v1` message (SP3 network type) | oneof tag |
|---|---|---|
| `HeartbeatRequest` | `HeartbeatRequest` | 1 |
| `SearchRequest` | `SearchRequest` | 500 |
| `BrowseRequest` | `BrowseRequest` | 501 |
| `GetLocationStatsRequest` | `GetLocationStatsRequest` | 1000 |
| `ListLocationsRequest` | `ListLocationsRequest` | 1001 |
| `GetIndexedFileRequest` | `GetIndexedFileRequest` | 1002 |
| `GetFileIndexingStatsRequest` | `GetFileIndexingStatsRequest` | 1003 |
| `GetModelStatusRequest` | `GetModelStatusRequest` | 1004 |
| `GetDaemonInfoRequest` | `GetDaemonInfoRequest` | 1007 |
| `IndexLocationRequest` | `IndexLocationRequest` | 2000 |
| `RemoveLocationRequest` | `RemoveLocationRequest` | 2001 |
| `UpdateAnnotationRequest` | `UpdateAnnotationRequest` | 2002 |
| `EjectVolumeRequest` | `EjectVolumeRequest` | 2003 |
| `RetryFailedRequest` | `RetryFailedRequest` | 2005 |

`message Response { oneof kind { … } }` (**15 arms** — see 7.8 #1; the `ErrorResponse` arm references `media.v1.ErrorInfo`, resolution 4):

| Variant (`mod.rs`) | Inner `media.v1` message (SP3 network type) | oneof tag |
|---|---|---|
| `HeartbeatResponse` | `HeartbeatResponse` | 10001 |
| `SearchResponse` | `SearchResponse` | 10500 |
| `BrowseResponse` | `BrowseResponse` | 10501 |
| `GetIndexedFileResponse` | `GetIndexedFileResponse` | 11002 |
| `ListLocationsResponse` | `ListLocationsResponse` | 11003 |
| `GetLocationStatsResponse` | `GetLocationStatsResponse` | 11004 |
| `GetFileIndexingStatsResponse` | `GetFileIndexingStatsResponse` | 11005 |
| `GetModelStatusResponse` | `GetModelStatusResponse` | 11006 |
| `GetDaemonInfoResponse` | `GetDaemonInfoResponse` | 11009 |
| `IndexLocationResponse` | `IndexLocationResponse` | 12000 |
| `RemoveLocationResponse` | `RemoveLocationResponse` | 12001 |
| `UpdateAnnotationResponse` | `UpdateAnnotationResponse` | 12003 |
| `EjectVolumeResponse` | `EjectVolumeResponse` | 12004 |
| `RetryFailedResponse` | `RetryFailedResponse` | 12006 |
| `ErrorResponse` | `media.v1.ErrorInfo` *(collapse — resolution 4, no SP3 type)* | 20000 *(remapped from source MessageType::ErrorResponse=19999 — protobuf reserves field numbers 19000–19999; §7.8)* |

`message Event { oneof kind { … } }` (no envelope — uncorrelated push, resolution 2):

| Variant (`mod.rs`) | Inner `media.v1` message (SP3 network type) | oneof tag |
|---|---|---|
| `IndexingProgressEvent` | `IndexingProgressResponse` | 13000 |
| `FailedFilesEvent` | `FailedFilesResponse` | 12007 |
| `VolumeStateChangedEvent` | `VolumeStateChangedEvent` | 20000 |
| `FolderUpdatedEvent` | `FolderUpdatedEvent` | 20001 |
| `ModelDownloadProgressEvent` | `ModelDownloadProgressEvent` | 20002 |

`Payload<T>` (generic correlation wrapper) → **two concrete messages** (resolution 2; proto3 has no generics):

| Message | Fields |
|---|---|
| `RequestEnvelope` | `uint64 request_id=1; Request request=2` (`request_id`=src `RequestId` newtype inlined as `uint64`; `request_id=0` ⇒ no-correlation default — matches source `RequestId::from_raw(0)` Default + `id.value()!=0` encode guard) |
| `ResponseEnvelope` | `uint64 request_id=1; Response response=2` (same; the source `RequestMessage=Payload<Request>` / `ResponseMessage=Payload<Response>` aliases are NOT separate types) |

`Event` has **no** envelope (source comment: "events are NOT wrapped in `Payload<>` — they have no `req_id`"; encoded directly as the frame payload).

### 7.6 Implementation batches (dependency-ordered; one implementer task + two-stage review each; each batch = author proto into `proto/media/v1/types.proto` (package `media.v1`, same-package — mono-consolidation) + `xtask gen` + extend `src/lib.rs` named re-exports + round-trip/JSON/quickcheck tests)

0. **Codegen scaffolding (smoke — MUST land first):** add `Sp3CodegenSmoke` into `proto/media/v1/types.proto` (package `media.v1`) — **one** smoke message proving the `mediatime` extern path in the same single-package run:
   ```proto
   message Sp3CodegenSmoke {
     bytes id = 1;                              // inline-bytes id convention
     ErrorInfo error = 2;                       // same-package ref (formerly cross-package media.v1 ref)
     VideoMeta video_meta = 3;                  // same-package ref (formerly cross-package findit.db.v1 ref)
     optional mediatime.v1.TimeRange range = 4; // mediatime extern (-> ::mediatime)
   }
   ```
   xtask `.files()` entry unchanged (single `proto/media/v1/types.proto`). Run `cargo run -p xtask -- gen`. Kept permanently as the SP3 regression guard (mirrors SP0's `TimedDetection` / SP2's `Sp2CodegenSmoke`). Round-trip + JSON test on `Sp3CodegenSmoke`.
1. **Scalar leaves (no other SP3 deps):** `Pagination`, `SearchFilter`, `HeartbeatRequest`, `HeartbeatResponse`. (Bare-`u32` doc-enums and `VideoIndexStatus` are inline `uint32` — no standalone type; materialized at use sites in later batches.)
2. **Reuse-only leaves (same-package refs, no SP3 deps):** `SearchHit` (`Location`, `Dimensions` + `mediatime.v1.TimeRange` + inline bytes), `BrowseItem` (`VideoMeta` + `Location`), `ModelInfo`, `ModelDownloadProgress`, `IndexingFile` (`Location` + inline `uint32` VideoIndexStatus), `NetFailedFile` (`Location`, `ErrorInfo` + inline `uint32` VideoIndexStatus).
3. **Empty + simple request/response leaves:** `ListLocationsRequest`, `RemoveLocationResponse`, `RetryFailedResponse`, `EjectVolumeResponse`, `GetModelStatusRequest`, `GetDaemonInfoRequest`, `UpdateAnnotationResponse` (all empty); `EjectVolumeRequest`, `GetIndexedFileRequest`, `GetFileIndexingStatsRequest` (inline bytes); `GetLocationStatsRequest`, `RemoveLocationRequest`, `RetryFailedRequest`, `BrowseRequest` (`Location`/`Pagination`); `IndexLocationRequest` (`LocationTarget`); `IndexLocationResponse` (`WatchedLocation`); `UpdateAnnotationRequest` (`repeated bytes` + `Tag`); `SearchResponse`, `GetDaemonInfoResponse`.
4. **Composite responses (depend on batch-2/3 leaves):** `Volume` (`VolumeMeta`, `WatchedLocation`), `ListLocationsResponse` (`Volume`), `GetLocationStatsResponse` (`NetFailedFile`), `FailedFilesResponse` (`NetFailedFile` + inline bytes), `SearchRequest` (`Pagination`+`SearchFilter`), `BrowseResponse` (`BrowseItem`+`Pagination`), `GetIndexedFileResponse` (`Video`, `Scene`), `GetFileIndexingStatsResponse` (`optional ErrorInfo` + inline `uint32`), `GetModelStatusResponse` (`ModelInfo`), `ModelDownloadProgressResponse` (`ModelDownloadProgress`), `IndexingProgressResponse` (`Location`+`IndexingFile`).
5. **Events:** `VolumeStateChangedEvent` (`VolumeMeta`), `FolderUpdatedEvent` (`Location`), `ModelDownloadProgressEvent` (`ModelDownloadProgress`).
6. **`Request` oneof envelope (own batch — large, depends on all 14 request messages of batches 1–4):** `message Request { oneof kind { … 14 arms, source `MessageType` tags … } }`.
7. **`Response` oneof envelope (own batch — large, depends on all response messages of batches 1–5 + `ErrorInfo`):** `message Response { oneof kind { … 15 arms; `ErrorResponse` arm → `ErrorInfo`, tag 20000 (remapped from reserved 19999, §7.8) … } }`.
8. **`Event` oneof envelope (own batch — depends on `IndexingProgressResponse`/`FailedFilesResponse` + the 3 event messages):** `message Event { oneof kind { … 5 arms … } }`.
9. **Correlation envelopes (LAST — depend on `Request`+`Response`):** `RequestEnvelope { uint64 request_id=1; Request request=2 }`, `ResponseEnvelope { uint64 request_id=1; Response response=2 }`.

Totals: **48 owned messages** (43 non-`mod.rs` messages from the §7.5 survey + `Request` + `Response` + `Event` + `RequestEnvelope` + `ResponseEnvelope`), **0 owned enums**, **0 inline-bitflag types of its own** (`VideoIndexStatus` reused by-value as inline `uint32`, 7.4), **10 batches** (Task 0 codegen scaffolding + 9 content batches).

### 7.7 Excluded types (with reasons)

- **`Header`** (`#[repr(C)]` 24-byte binary frame; no `Encode`/`Decode`, only `from_bytes`/`to_array`) — fixed-layout transport frame header, not a proto3 message. Excluded (resolution 3).
- **`MessageType`** (`#[repr(u16)] #[non_exhaustive]` wire-dispatch enum) — its sparse integer values become the `Request`/`Response`/`Event` `oneof` arm tags (7.5, resolution 1); not emitted as a proto `enum`.
- **`MessageFlags`** (`bitflags!`, **empty** — no bits) — header field with no carrier once `Header` is excluded; no inline `uint32` (no containing message). Excluded (resolution 3).
- **`RequestId`** (`#[repr(transparent)] struct RequestId(u64)`) — transparent u64 newtype → inline `uint64` on `RequestEnvelope`/`ResponseEnvelope` (resolution 2/3); not a wrapper type.
- **`Payload<Data>`** (generic correlation wrapper) — proto3 has no generics; monomorphized into the two concrete `RequestEnvelope`/`ResponseEnvelope` messages (resolution 2). `RequestMessage`/`ResponseMessage` (type aliases for `Payload<Request>`/`Payload<Response>`) — aliases, not types.
- **`error.rs::ErrorResponse`** (thin `{ error: ErrorInfo }` single-field wrapper, encoded tag=1) — collapsed; `Response`'s error arm references `ErrorInfo` directly (bare same-package ref, resolution 4). No `NetErrorResponse` emitted.
- **`framing.rs` + `async_framing.rs`** (length-prefixed batch stream codec + tokio async I/O) — pure transport plumbing over `Encode`/`Decode`, no message types. Excluded (resolution 3).
- **All `*Ref`/`*Chunk`** ownership/zero-copy mirrors + encode-only borrow structs (every `*Ref`, `*Chunk` listed in 7.2) — same schema, different ownership; buffa generates owned+view itself. Excluded.
- **The 5 bare-`u32` "enums-by-convention"** (`ModelInfo.status`, `ModelDownloadProgress.status`, `NetFailedFile.kind`, `VolumeStateChangedEvent.event`, `FolderUpdatedEvent.event`) — not Rust enums/newtypes; emitted as inline `uint32` with documented semantics, NOT promoted to proto `enum`s (7.3, 7.8 #2).
- **`VideoIndexStatus`** — SP2 `database` bitflag, already an inline `uint32` per §2/§6.4; SP3 reuses the bit contract by-value as an inline `uint32` (7.4), emits no SP3 type.
- **`define_oneof!` macro body, all `*_IDENTIFIER`/`*_IDENT` wire-const modules, `Encode`/`Decode`/`Arbitrary`/`Default`/`msg_type()` impls, `#[cfg(test)] mod tests`, `#[cfg(any(test, feature="quickcheck"))]` blocks** — pure-Rust encoding conventions / test code, no own schema (buffa regenerates these).
- **SP1/SP0/SP2 types and `mediatime.v1.*`** — same-package (SP1/SP0/SP2) or extern (`mediatime.v1`), never redefined in the SP3 section (see 7.1).

### 7.8 Ambiguities resolved (no TBDs)

These are SP3's resolutions; #1–#8 apply the directive's eight locked resolutions with rationale; #9–#10 are additional ambiguities surfaced by the survey and resolved autonomously.

1. **`Request`/`Response`/`Event` (Rust enums-with-data) → proto3 `message` each with a single `oneof kind`** (owned; mirrors SP2 `SubtitleTrackOrigin`, §6.2). The source `MessageType` discriminant integer of each variant is the `oneof` arm's tag number. These are sparse, deliberately-banded values (1, 2/500/501, 1000s, 2000s, 10000s, 12000s, 13000, 20000s) — emitted **verbatim, with NO `reserved`**, per §2's wide-deliberate-band rule and the SP2 §6.8 #3 (`AudioAnalysis`/`TrackRecord` band) precedent. **Single exception: the `Response.error` arm** (source `MessageType::ErrorResponse=19999`) is the only deviation — remapped to **20000** because protobuf reserves field numbers 19000–19999; per §2's NO-wire-compat invariant the source discriminant is documentation not an obligation, making the remap correct (see §7.8 new item below). All other arm tags remain verbatim. **Discrepancy vs. directive inventory:** the directive listed `Response` with 14 arms; `mod.rs` defines **15** (`ErrorResponse(error::ErrorResponse) = 19999` is the 15th). All 15 are enumerated in 7.5; the `ErrorResponse` arm resolves via #4. `Request`=14 arms, `Event`=5 arms — both match the directive.
2. **The ≈5 expected SP3 enums do not exist as Rust enums (significant discrepancy).** `ModelStatus`/`ModelDownloadStatus`/`FileKind`/`VolumeEvent`/`FolderEvent` are **bare `u32` struct fields** with doc-comment-only enumerations, not Rust `enum`s or `#[repr]`/newtype int-enums (verified against `shared.rs`/`model.rs`/`event.rs`). §2's enum rule targets Rust enums/int-newtypes only. **Resolution:** map each to plain `uint32` with the documented semantics reproduced verbatim in the proto field comment; **SP3 owns zero enums**, zero `*_UNSPECIFIED` insertions, zero value-shifts. Rationale: promoting unconstrained `u32` to a closed proto `enum` is speculative typing the source declined (YAGNI, mirroring resolution-8's anti-speculation stance) and risks lossy round-trip of out-of-set values (the correctness criterion). The `ModelInfo.status` vs `ModelDownloadProgress.status` "same-or-distinct" survey question is moot under this resolution (both plain `uint32`); for the record their documented value sets *differ* (3-value vs 5-value) — documentary only.
3. **`Payload<T>` → two concrete messages** `RequestEnvelope {uint64 request_id=1; Request request=2}` and `ResponseEnvelope {uint64 request_id=1; Response response=2}` (proto3 has no generics; `request_id=0` = no-correlation default, matching the source `RequestId::from_raw(0)` Default and the `id.value()!=0` encode guard). `Event` has **no** envelope (uncorrelated push, source-documented). The `RequestMessage`/`ResponseMessage` aliases are not separate types.
4. **`error.rs::ErrorResponse` is not a new message** — thin single-`ErrorInfo` wrapper (tag=1 only); the `Response` oneof's error arm references `ErrorInfo` directly (bare same-package ref, SP2 clap-wrapper-collapse precedent, §6.2). No `NetErrorResponse`.
5. **Transport plumbing excluded:** `Header` (24-byte `#[repr(C)]` frame), `MessageType` (`#[repr(u16)]` dispatch enum — values → oneof tags, not a proto enum), `MessageFlags` (empty `bitflags!`, no carrier), `RequestId` (u64 newtype → inline `uint64`), `framing.rs`+`async_framing.rs`, all `Encode`/`Decode`/`Arbitrary` impls. (7.2/7.7.)
6. **Enum 0-value conflicts (N/A under #2).** The directive anticipated `*_UNSPECIFIED=0` insertion + value-shift for `VolumeEvent`/`FolderEvent`/`FileKind`/`ModelStatus`/`DownloadStatus`. Since none is a Rust enum (#2), there is no enum to shift: the semantic 0-values (`mounted`/`file_created`/`video`/`not_downloaded`/`pending`) remain the literal `uint32` value `0`, documented in the field comment, round-tripping exactly. No `*_UNSPECIFIED` is introduced.
7. **Codegen scaffolding — Task 0.** Batch 0 (7.6) lands a single `Sp3CodegenSmoke` message as a plain same-package `media.v1` type, proving the `mediatime.v1.TimeRange` extern path in one buffa run **before** any content batch (formerly: three-way cross-package smoke — see §7.8 #14 for why cross-package is superseded). xtask `.files()` entry unchanged (single `proto/media/v1/types.proto`). **No** new generated package directories or files beyond the growing `media.v1.types.rs`; **no** new `mediatime.v1*`. Kept permanently (regression guard, mirrors SP0 `TimedDetection` / SP2 `Sp2CodegenSmoke`).
8. **`SearchHit.dimensions`** (source packed `u32` via `to_u32()`, `ThirtyTwoBit` tag=9) → references `media.v1.Dimensions` (SP1 split `uint32 width/height`); packed↔split is a clean-redesign per §2/§5 (SP2 §6.8 #7 `AudioCoverArt.dimensions` precedent). No SP3 redefinition.
9. **Non-zero Rust defaults** (`Pagination.limit=50`, `SearchFilter.weight=1.0`) → plain `uint32`/`float`, **not** `optional`. Rationale (directive resolution 8): the value round-trips exactly (= the migration's correctness criterion); "use 50 / 1.0 when the field is absent" is a documented **client-side** convention (the source `Encode` skips the field when it equals the default, and `Default` re-supplies it on decode — a Rust-side behavior, not a wire presence bit). Adding `optional` would model speculative presence the wire format does not carry (YAGNI / anti-speculative-optionality). The client default is recorded in each field's proto comment (7.5).
10. **`ModelDownloadProgressResponse` vs `ModelDownloadProgressEvent` — both kept as named messages (not collapsed).** Both are thin single-field wrappers `{ model: ModelDownloadProgress }` (tag=1) with identical wire shape (and `model.rs` even notes "Same wire format"). Per the survey question, they are **kept as two distinct named messages** (not collapsed into one, and not collapsed into `ModelDownloadProgress`): they serve distinct protocol roles — `ModelDownloadProgressResponse` (`model.rs`) is the standalone streamed **response** message returned to a model-status caller, while `ModelDownloadProgressEvent` (`event.rs`) is the uncorrelated server-push **`Event` arm** at discriminant tag 20002. They are separate, independently-named decode targets in the source (defined in different modules, used on different code paths); collapsing them would erase the response-vs-push role distinction the protocol relies on. Both reference the shared `ModelDownloadProgress` leaf (not a re-typed copy). This is consistent with the directive's explicit guidance ("keep both as named messages — they serve distinct response vs push roles — but reference the shared `ModelDownloadProgress`") and does not contradict the §6.2 thin-wrapper-collapse rule (that rule collapses a wrapper whose *only* role is to wrap; here the two wrappers carry distinct, load-bearing protocol identity, exactly like `ModelDownloadProgressResponse` is genuinely a separate decode target from `ModelDownloadProgressEvent` in the source dispatch).
11. **SP3 message-count reconciliation: §7.5 mapping tables are the authoritative anchor; final count is 48 messages / 0 enums.** The §7.6 totals line originally stated "35 owned messages (30 from survey + 5)" — a pre-survey estimate from the directive planning stage. The §7.5 full survey superseded that estimate: every request message and its paired response message is a distinct fielded message (e.g. `HeartbeatRequest`/`HeartbeatResponse` are two separate owned messages, not one), and dropping either would lose real wire types and break the `Request`/`Response` oneof arm tables in §7.5 that reference each request and each response as separate inner types. The §7.5 tables enumerate **43 non-`mod.rs` messages** (shared.rs=8, folder.rs=12, heartbeat/file/daemon/search/browse/model/progress/annotation=20, event.rs=3) **+ 5 `mod.rs` messages** (`Request`, `Response`, `Event`, `RequestEnvelope`, `ResponseEnvelope`) **= 48 owned SP3 `network` messages** (all in the single `media.v1` package post-consolidation), **0 owned enums**. The §7.6 totals line has been corrected to match. (Mirrors SP2 §6.8 #1's "~40 → 42" self-correction.)

12. **`Response.error` arm tag remap 19999→20000 (protobuf reserved-range constraint).** Protobuf reserves field numbers 19000–19999 for internal implementation use (FieldDescriptorProto range); protoc rejects any proto field or oneof arm assigned a number in this range. The source `MessageType::ErrorResponse=19999` therefore cannot be used as a proto field number verbatim. Per §2 (NO wire-compat — correctness is round-trip + semantic fidelity; the source `MessageType` discriminant is a documentation artifact, not a wire obligation), the `Response.error` arm is remapped to **20000** — the next clean non-reserved sentinel above the reserved band, unique within the `Response` oneof. Per-message oneof tag-spaces are independent, so this does not collide with the unrelated `Event` oneof arm `VolumeStateChangedEvent=20000` (a different message's tag-space). This is the **only** arm in all of SP3 affected: all other `Request`, `Response`, and `Event` arm tags (1, 500–501, 1000s, 2000s, 10000s, 11000s, 12000s, 13000, 20001, 20002) fall entirely outside the 19000–19999 reserved range and are emitted verbatim. The shipped `proto/media/v1/types.proto` reflects `= 20000` and the SP3 plan Task 7 reflects the same. No xtask/codegen workaround is used (an earlier attempt to patch the `FileDescriptorSet` to keep 19999 was rejected — it contradicted the locked minimal-checked-in-codegen / NO-wire-compat design).
13. **`findit.net.v1.FailedFile`→`NetFailedFile` same-package rename (mono-consolidation, user directive).** The original two-package framing had `findit.net.v1.FailedFile` (the frontend wire type `{kind, location, error, error_status, index_status}`, `shared.rs`) and `findit.db.v1.FailedFile` (the DB record `{id, media_id, location_id, failed_at}`, §6.5) disambiguated by their distinct packages — both names retained, as stated in §7.1 Name-disambiguation. After mono-consolidation (§3 + §7.8 #14 below), both live in the single `media.v1` package; the package disambiguation is gone. **Resolution:** the SP3 network-wire-type `FailedFile` is **renamed at the proto definition** to `NetFailedFile`; SP2's DB-record `FailedFile` keeps its name unchanged. This is a same-package rename required by user directive — no field/tag/enum content changed.

14. **Mono-consolidation (user directive) — supersedes the §7.1 / §3 three-package framing for SP3.** The 3-package split (`media.v1` SP0+SP1 / `findit.db.v1` SP2 / `findit.net.v1` SP3) was **consolidated into a single `media.v1` package in `proto/media/v1/types.proto`** with no cross-package refs, per explicit user directive. This is a structural refactor — no field/tag/enum/message content changed; only the package partition and the 2 collision names. Specifically for SP3: the `findit.net.v1` package and `network.proto` file are abolished; all SP3 types are authored directly into `proto/media/v1/types.proto` as `media.v1`-package members; all former cross-package refs (the previous three-way `media.v1.*` + `findit.db.v1.*` + `mediatime.v1.*` surface) become bare same-package refs (except `mediatime.v1` which remains the sole extern, unchanged). The `findit.net.v1.FailedFile`→`NetFailedFile` same-package rename is required (§7.8 #13 above). The xtask `.files()` three-entry list reverts to the single `proto/media/v1/types.proto` entry; the second and third entries added by SP2/SP3 for `database.proto`/`network.proto` are removed. The cross-package codegen mechanism (native `super::` resolution via `buffa_codegen::context::TypePath`, the SP2/SP3 risk front-loaded in Task 0 scaffolding) is **no longer exercised** by SP3 (all same-package); `mediatime.v1` remains the sole extern unchanged. The `Sp3CodegenSmoke` fixture is retained as a permanent regression guard but is now a plain same-package `media.v1` message (the `media.v1.ErrorInfo`/`findit.db.v1.VideoMeta` cross-package refs in its fields become bare same-package refs; the `mediatime.v1.TimeRange` extern ref is unchanged). SP3 plan Task 0 proto header and batch Conventions should be read with this understanding — field/tag/enum/message content is authoritative; only the package header, import lines, and FQN prefixes (`media.v1.` / `findit.db.v1.`) are superseded by the single-package same-package outcome.

### 7.9 Definition of done (SP3)

- All 10 batches landed (Task 0 codegen scaffolding first; 9 content batches dependency-ordered).
- Full feature-matrix builds (default / no-default / `json` / `arbitrary` / `quickcheck` / all-features).
- `cargo run -p xtask -- gen` is drift-clean (CI gen-gate green); xtask `.files()` has the single `proto/media/v1/types.proto` entry (mono-consolidation — no `network.proto` entry).
- Wire round-trip + JSON round-trip (under `json`) + quickcheck property + `mediatime`-extern round-trip green for **every** SP3 owned message (incl. `Sp3CodegenSmoke`, the 3 oneof envelopes, the 2 correlation envelopes).
- All SP3 types live in the single `media.v1` package; **no cross-package refs** anywhere (all same-package except `mediatime.v1` extern, unchanged); `NetFailedFile` is the proto definition name for the SP3 network-wire-type (SP2's `FailedFile` keeps its name).
- **No** new `mediatime.v1*` generated file; no new `findit.net.v1.*` generated package directory; SP3 content is in the growing `media.v1.types.rs`.
- No `NetErrorResponse` exists (collapsed to bare `ErrorInfo`); no proto `enum` exists in the SP3 section; `Header`/`MessageType`/`MessageFlags`/`RequestId`/framing/`*Ref`/`*Chunk` are absent.
- `src/lib.rs` named (not glob) re-exports extended for the SP3 surface.
- Stacked PR `sp3-network` → `sp2-database`.

## 8. Risks

- **Scale.** ~120 types / ~47k LOC across three domains; executed continuously via the batched subagent pipeline — long-running by nature.
- **Composition depth.** Detection-family types nest base `Detection`/`BoundingBox` and each other; batch ordering must respect dependencies (primitives first).
- **Extern reuse.** `timebase`/`time_range` must map to the existing `::mediatime` extern (not regenerated); `track_time` composes `mediatime.v1.TimeRange`.
- **Enum/bitflags fidelity.** `#[repr]` newtype "enums" and bitflags must round-trip via proto `enum`/integer without losing unknown values (buffa `EnumValue` preserves unknowns).
- **SP2 same-package name discipline.** Under mono-consolidation all SP2 types live in `media.v1`; the `DbMediaKind` rename must be applied at the proto definition; the 5 clap wrappers / `CedTag` / `Iso6392B` / `VoiceRange` collapses must not regress into emitted messages; no SP1/SP0 type may be redefined by the SP2 section.
- **SP3 same-package name discipline.** Under mono-consolidation all SP3 types live in `media.v1`; the `NetFailedFile` rename must be applied at the proto definition; the `Request`/`Response`/`Event` discriminant integers (sparse, up to 20002) must be emitted verbatim as `oneof` arm tags (no `reserved`, no renumber); the bare-`u32` "enum-by-convention" fields (`ModelInfo.status`, `ModelDownloadProgress.status`, `NetFailedFile.kind`, `VolumeStateChangedEvent.event`, `FolderUpdatedEvent.event`) must stay plain `uint32` (NOT promoted to proto `enum`s) so unknown discriminants round-trip; `Header`/`MessageType`/`MessageFlags`/`RequestId`/framing must not regress into emitted messages; `ErrorResponse` must collapse to bare `ErrorInfo` (no `NetErrorResponse`). The `mediatime.v1` extern is unchanged.
