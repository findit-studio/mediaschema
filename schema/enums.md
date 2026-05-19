# Domain enums  *(rev 4 — LOCKED, user-approved; + `ErrorCode`)*

Policy: small fixed findit vocabularies = **closed** enums; open/extensible =
`#[non_exhaustive]` + `Other(SmolStr)` (unrecognised wire value preserved, not
lost). **Boundary ([extern-vs-flatten]):** media-stream *descriptor*
vocabulary (codec/container/format/layout/disposition — FFmpeg-derived,
reusable beyond findit) lives in **`::mediaframe`**, not here. mediaschema owns
only the findit **pipeline/analysis** enums.

## mediaschema-owned (findit domain / pipeline — flatten/own)

| domain enum | kind | variants (key) | notes |
|---|---|---|---|
| `MediaKind` | closed | `Video`, `Audio` | **kept, not derived** (drives which facets are created); `kind` required post-probe (pre-probe = different lifecycle, not an `Unknown` arm) |
| `SceneDetector` | non_exhaustive | `Histogram`,`Phash`,`Threshold`,`Content`,`Adaptive`,`Manual` | **scenesdetect** engine; locked in `scene.md` r6 — **added (resolved)** |
| `KeyframeExtractor` | non_exhaustive | all `SceneDetector` variants + `CompositeQuality`,`Interval`,`IFrame`,`SceneRepresentative`,`Manual` | findit keyframe pipeline; locked in `keyframe.md` r15 — **replaces the dropped closed `KeyframeKind` (resolved)** |
| `SubtitleKind` | closed | `FullDialogue`,`ForcedNarrative`,`CommentaryText` | **mediaschema-owned** (subtitle *role* — a findit selection/search facet, not a raw stream property); locked `subtitle_track.md` r2 adopted it — **added (resolved)** |
| `SegmentKind` | closed | `Speech`,`Music`,`Silence`,`Noise`,`Overlap` | audio-segment analysis (pyannote) — *pending audio-cluster review* |
| `AudioContentKind` | closed | `Speech`,`Music`,`Mixed`,`Silence` | audio-track analysis — *pending audio-cluster review* |
| `ScanStatus` | closed | `Ok`,`Partial`,`Failed` | `WatchedLocation` product concept |
| `ErrorCode` | non_exhaustive `#[repr(u32)]` | HTTP-style `BadRequest 400`/`PermissionDenied 403`/`NotFound 404`/`AlreadyExists 409`/`UnprocessableEntity 422`/`InternalError 500`/`ServiceUnavailable 503`/`Timeout 504`; domain: `Probe* 1000–1003`, `SceneDetection* 2000–2001`, `Transcription* 3000–3001`, `Vlm* 4000–4001`, `AppleVision* 5000–5001`, `Embedding* 6000–6005`, `Path/Volume* 7000–7007`, `Remote* 8000–8005`, `Cancelled 9000`/`OutOfMemory 9001`, `Ced* 10000–10002` | the `code` of `ErrorInfo` ([primitives.md](primitives.md)); verified vs `findit-proto::common::error_info`; **carries which-stage/why** (the derived error model — no `error_status`) |
| `VideoIndexStage` | closed | `Pending→Probed→SceneDetected→KeyframeExtracted→Analyzed→Embedded→Done`(+`Failed`) | **per-kind**; derived (VERIFIED bits): `Analyzed` = `VLM_ANALYZED`+`APPLE_VISION_ANALYZED`; `Embedded` = `TEXT_EMBEDDING_FINISHED`+`SCENE_EMBEDDING_FINISHED` |
| `AudioIndexStage` | closed | derived from the real 11-bit `ProcessingStage` — *pending audio-cluster review* | **per-kind** |
| `SubtitleIndexStage` | closed | `Pending→TracksDiscovered→CuesExtracted→Ocr→SearchIndexed→Done`(+`Failed`) | **per-kind**; `Ocr` image-based only (VERIFIED bits) |

The three `*IndexStage` enums are **separate types** (pipelines genuinely
differ); coarse stage is **derived** from the per-kind `*IndexStatus` bitflags
([bitflags.md](bitflags.md)) + `index_errors`, not a stored field.

## `::mediaframe` extern — NOT redefined here (user-approved re-scope)

Media-stream descriptor vocabulary. Tracked in
[mediaframe-candidates.md](mediaframe-candidates.md); batched into a
`mediaframe` minor **after `0.1.0`**; mediaschema externs them
(`.mediaframe.v1 → ::mediaframe`). Locked docs get only the **mechanical
`::mediaframe::` path rename** (not a re-open — per the tracker rule).

| `::mediaframe` enum | kind | notes |
|---|---|---|
| `VideoCodec`/`AudioCodec`/`SubtitleCodec` (+ profile/level) | non_exhaustive `Other(SmolStr)` | codec family only; profile/level separate fields |
| `ContainerFormat`/`AudioFormat`/`AudioContainerFormat` | non_exhaustive `Other` | file/container form |
| `SubtitleFormat` | non_exhaustive | text vs bitmap form (locked `subtitle_track.md` r2 uses it) |
| `ChannelLayout` | non_exhaustive `Other(SmolStr)` | `Mono`/`Stereo`/`5_1`/`7_1`/… |
| `SubtitleTrackOrigin` | closed | `External`/`Embedded`/`Generated` |
| `BitRateMode` | closed | `Cbr`/`Vbr`/`Abr` (with audio batch) |
| `ColorPrimaries`/`ColorTransfer`/`ColorMatrix`/`ColorRange`/`ChromaLocation`/`DcpTargetGamut`/`PixelFormat` | — | FFmpeg-n8.1 numbered + `Unknown(u32)` + `DOMAIN_EXT_BASE` (already in `mediaframe`, ex-`videoframe` PR #2 → now PR #3 `0.1.0`) |
| `TrackDisposition` (bitflags) | — | FFmpeg `AV_DISPOSITION_*`; see [bitflags.md](bitflags.md) |

## Resolved (your calls)

- **Gap fixes:** `SceneDetector` added; old closed `KeyframeKind` **dropped**,
  replaced by locked `KeyframeExtractor`; `SubtitleKind` added
  **mediaschema-owned** (subtitle role = findit facet, your agreement).
- **E-stage:** **VERIFIED** vs `findit-proto::database` — `VideoIndexStage`/
  `SubtitleIndexStage` ordering corrected to the real bits ([bitflags.md](bitflags.md));
  `AudioIndexStage` derived from the real 11-bit `ProcessingStage`, pending the
  audio-cluster review.
- Descriptor enums re-scoped `::mediaframe` (user-approved).

## Open questions

- *(none for the mediaschema-owned set — audio enums + `AudioIndexStage` final
  shape ride with the deferred audio-cluster review.)*

**Status: LOCKED (rev 4) — user-approved.** Gap-fixes done (`SceneDetector`+,
`KeyframeKind`→`KeyframeExtractor`, `SubtitleKind` mediaschema); stage vocab
VERIFIED vs `findit-proto`; descriptor enums `::mediaframe`. *(rev 4:
user-authorized reopen of locked r3 to add **`ErrorCode`** — mediaschema-owned,
verified vs `findit-proto::common::error_info`; underpins the derived error
model.)* Audio enums + `AudioIndexStage` ride with the deferred audio-cluster
review (not a reopen of this lock).
