# Domain bitflags  *(rev 1 — drafted for review, NOT self-locked)*

The wire keeps bare `u32` (proto3); the domain gets `bitflags!` companions.
Locked rule: `index_status` is **6 distinct vocabularies → per-kind** companions
(do **not** unify); `disposition` is identical across the 3 track types → **one
shared** `TrackDisposition`. Error *details* are per-track (`index_errors:
Vec<ErrorInfo>`); these flag sets are the compact, queryable signal.

## `MediaErrorFlags` — `bitflags! u16` (on `Media`, rollup)

`VIDEO_ERROR` `AUDIO_ERROR` `SUBTITLE_ERROR` + reserved bits (u16 chosen for
future kinds — locked). Bit set ⇔ that kind's `track_progress.failed > 0` ⇒
drill `Media → kind facet → Track.index_errors`. (`Media.probe_error` is the
separate non-track file-unprobeable case.)

## Per-kind pipeline-status — `bitflags! u32` (on each `*Track`)

Source of truth for indexing; the coarse `*IndexStage` ([enums.md](enums.md)) is
**derived** from these + `index_errors`.

| companion | bits (proposed — confirm vs real pipeline) |
|---|---|
| `VideoIndexStatus` | `PROBED` `SCENES_DETECTED` `KEYFRAMES_EXTRACTED` `VLM_CAPTIONED` `EMBEDDED` |
| `AudioIndexStatus` | `PROBED` `ANALYZED` (loudness/fingerprint) `TRANSCRIBED` `DIARIZED` `EMBEDDED` |
| `SubtitleIndexStatus` | `PROBED` `CUES_PARSED` `OCR_DONE` (image-based only) `SEARCH_INDEXED` |

## Per-kind error categories — `bitflags! u32` (on each `*Track`)

`VideoErrorStatus` / `AudioErrorStatus` / `SubtitleErrorStatus` — coarse
categories over the per-track `index_errors`: e.g. `DECODE` `IO`
`UNSUPPORTED_CODEC` `CORRUPT` `TIMEOUT` `OOM` `MODEL_FAILURE` (per-kind subset).
Lets the API say *why* without shipping every `ErrorInfo`.

## `TrackDisposition` — shared `bitflags! u32` (Video/Audio/Subtitle)

Mirrors FFmpeg `AV_DISPOSITION_*`: `DEFAULT` `DUB` `ORIGINAL` `COMMENT`
`LYRICS` `KARAOKE` `FORCED` `HEARING_IMPAIRED` `VISUAL_IMPAIRED`
`CLEAN_EFFECTS` `ATTACHED_PIC` `CAPTIONS` `DESCRIPTIONS` `DEPENDENT`
`STILL_IMAGE`. One type, used by all three tracks (locked: disposition is
identical across kinds).

## Open questions

- **BF-stage-vocab:** the exact per-kind status bits must be confirmed against
  the real indexing pipeline (the table above is the proposed vocabulary). This
  is the main thing to nail with you per kind.
- **BF-probed:** is the one-shot `PROBED` event a *track* bit (as above) or a
  *container* (`Media`/facet) signal? (Global README open — *lean: track bit;
  facet `IndexProgress` aggregates.*)
- **BF-error-cats:** confirm the error category set per kind (vs a single shared
  `ErrorCategory` — but errors, like stages, differ per kind ⇒ per-kind).
- **BF-derive:** `*IndexStage` derivation rule (which bit-set ⇒ which stage,
  precedence with `Failed`) — specify once the bits are locked.

**Status: in review (rev 1) — drafted for your one-by-one review. NOT self-locked.**
