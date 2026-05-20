# Domain bitflags  *(rev 4 — LOCKED, user-approved)*

The wire keeps bare `u32` (proto3); the domain gets `bitflags!` companions.
Rules: `index_status` is **distinct per-kind** mediaschema companions (do
**not** unify; bit = that pipeline stage *succeeded*). **No per-track
`error_status`** — error-state is **derived** from `index_errors:
Vec<ErrorInfo>` (each carries a stage-coded `ErrorInfo.code`) + `index_status`;
the redundant per-kind error-category bitflags is dropped (user decision —
the error code already tells which stage). `disposition` is pure FFmpeg
stream-descriptor vocab → **one shared `TrackDisposition`, owned by
`::mediaframe`** (re-scoped, user-approved). `MediaErrorFlags` (root rollup) is
kept.

## `MediaErrorFlags` — `bitflags! u16` (on `Media`, rollup)

`VIDEO_ERROR` `AUDIO_ERROR` `SUBTITLE_ERROR` + reserved bits (u16 chosen for
future kinds — locked). Bit set ⇔ that kind's `track_progress.failed > 0` ⇒
drill `Media → kind facet → Track.index_errors`. (`Media.probe_error` is the
separate non-track file-unprobeable case.)

## Per-kind pipeline-status — `bitflags! u32` (on each `*Track`)

Source of truth for indexing; the coarse `*IndexStage` ([enums.md](enums.md)) is
**derived** from these + `index_errors`. **Bits VERIFIED against
`findit-proto::database` (the real pipeline)** — exact values preserved (the
status bit asserts the stage ran & its output landed, incl. vectors pushed to
LanceDB; the *vector itself* is not a domain field).

**`VideoIndexStatus`** (`findit-proto::database::video`, authoritative):
`PROBED 0x01` · `SCENE_DETECTED 0x02` · `KEYFRAME_EXTRACTED 0x04` · *(0x08
reserved)* · `VLM_ANALYZED 0x10` · `APPLE_VISION_ANALYZED 0x20` ·
`TEXT_EMBEDDING_FINISHED 0x40` (EmbeddingGemma) · `SCENE_EMBEDDING_FINISHED
0x80` (SigLIP2). VLM vs Apple-Vision are **distinct** stages (matches locked
`keyframe.md` producer-distinct model); two distinct embedding stages.

**`SubtitleIndexStatus`** (`findit-proto::database::subtitle`, authoritative):
`TRACKS_DISCOVERED 0x01` · `CUES_EXTRACTED 0x02` · `OCR_DONE 0x04`
(image-based only) · `SEARCH_INDEXED 0x08`.

**`AudioIndexStatus`** — the real audio pipeline is `ProcessingStage`
(`findit-proto::database::audio`, 11 bits): `EXTRACTED 0x01` · `CLASSIFIED
0x02` · `VAD_DONE 0x04` · `STT_DONE 0x08` · `SPEAKER_DONE 0x10` · `LLM_DONE
0x20` · `TEXT_EMBED 0x40` · `CED_DONE 0x80` · `CLAP_DONE 0x100` ·
`EBUR128_DONE 0x200` · `FPRINT_DONE 0x400`. **Pending the audio-cluster
review** (recorded here so the A-loc/audio_track pass uses the real bits, not a
guess).

## Error-state — *no bitflags* (derived; user decision)

There is **no `*ErrorStatus` type** (was `VideoErrorStatus`/`AudioErrorStatus`/
`SubtitleErrorStatus`). It carried no information not already in
`index_errors: Vec<ErrorInfo>` — each `ErrorInfo.code` is a stable
**stage-coded** id (the findit-indexer `"scene_detect_init"`/`"audio_whisper_send"`
op tags). Error-state is therefore **derived**:
- *failed?* — `index_errors` non-empty.
- *which stage(s)?* — the stages named by the `ErrorInfo.code`s.
- *coarse `*IndexStage`* — furthest contiguous `index_status` success bit,
  overridden by `Failed` when `index_errors` is non-empty.

Removed from all locked track docs (`video_track.md` r6, `subtitle_track.md`
r3) + the `audio_track.md` draft. `MediaErrorFlags` (root rollup) **kept** — a
deliberate cross-aggregate denormalization for media-list "has-errors"
filtering (same family as the locked `track_progress`/`total_scenes` rollups),
not redundant per-track state.

## `TrackDisposition` — **`::mediaframe` extern** (user-approved re-scope)

Mirrors FFmpeg `AV_DISPOSITION_*` — **authoritative set verified vs
`findit-proto::database::subtitle`** (20 bits): `DEFAULT 0x1` `DUB 0x2`
`ORIGINAL 0x4` `COMMENT 0x8` `LYRICS 0x10` `KARAOKE 0x20` `FORCED 0x40`
`HEARING_IMPAIRED 0x80` `VISUAL_IMPAIRED 0x100` `CLEAN_EFFECTS 0x200`
`ATTACHED_PIC 0x400` `TIMED_THUMBNAILS 0x800` `NON_DIEGETIC 0x1000`
`CAPTIONS 0x10000` `DESCRIPTIONS 0x20000` `METADATA 0x40000`
`DEPENDENT 0x80000` `STILL_IMAGE 0x100000` `MULTILAYER 0x200000`. One type,
used by all three tracks. **No longer a mediaschema
bitflags** — it is pure FFmpeg media-stream descriptor vocabulary → lives in
**`::mediaframe`** (tracked in [mediaframe-candidates.md](mediaframe-candidates.md);
batched into a post-`0.1.0` `mediaframe` minor). Locked docs (`video_track`/
`subtitle_track`/audio) get the mechanical `::mediaframe::TrackDisposition`
path rename only. The per-kind `*IndexStatus` bitflags stay mediaschema
(findit pipeline state, not stream vocab); there is no `*ErrorStatus`.

## Resolved

- **BF-stage-vocab:** VERIFIED vs `findit-proto::database` — video & subtitle
  bits are authoritative (above); audio = the real 11-bit `ProcessingStage`
  (recorded, pending the audio-cluster review).
- **BF-probed:** `PROBED` (video) / `TRACKS_DISCOVERED` (subtitle) = a
  **track bit** (your call); facet `IndexProgress` aggregates the rollup.
- **BF-error-cats:** **RESOLVED — no `*ErrorStatus` type** (user decision;
  derived from stage-coded `index_errors` + `index_status`).
- **BF-derive:** specified above — coarse `*IndexStage` = furthest contiguous
  `index_status` bit; `Failed` iff `index_errors` non-empty (precedence).

## Open questions

- *(none — all resolved; ready for your lock.)*

**Status: LOCKED (rev 4) — user-approved.** Per-track `error_status` REMOVED
(error-state derived from stage-coded `index_errors` + `index_status`); bits
VERIFIED vs `findit-proto::database`; `TrackDisposition`→`::mediaframe`
(20-bit); `MediaErrorFlags` kept; BF-probed=track bit. Audio `*IndexStatus`
bits ride with the deferred audio-cluster review.
