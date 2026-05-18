# `Audio<Id>` — audio facet (thin aggregate)  *(rev 7 — LOCKED, user-approved)*

> Approved with `segments` at the **facet** level (A-loc = facet, parallel to
> `Video.scenes`). The segment aggregate's internal schema + A-agg/A-name are
> designed in the analysis phase (`audio_segments.md`).

## Domain meaning

The audio facet of a `Media` (via `Media.audio`). A **thin aggregate**: groups
this media's audio tracks, a dedicated **analysis-segment** aggregate
(pyannote speaker diarization + whisper transcript), and the indexing
roll-up — directly parallel to how `Video` references `Scene` via
`Video.scenes`.

## Principle refinement (reconciles the two earlier statements)

- **Heavy *segmented ML outputs*** get their **own aggregate**, referenced at
  the **facet** level: video → `Scene` (`Video.scenes`); audio → a
  diarization/transcript segment aggregate (`Audio.segments`).
- **Per-track *signal* analysis** (codec/stream, loudness/EBU R128,
  chromaprint, …) + per-track index state stay on **`AudioTrack`**.
- So "analysis is per-track" = the per-track *signal* analysis; the segmented
  ML output (transcript/diarization, like scenes) is a facet-referenced
  aggregate. No contradiction.

## Cross-cutting

Generic over `Id` (UUIDv7 single key). Conversions deferred.

## Fields

| field | domain type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | facet id (referenced by `Media.audio`; tracks back-ref it) |
| `segments` | `Vec<Id>` | refs to the audio-analysis aggregate (pyannote diarization + whisper transcript) — parallel to `Video.scenes`. *(name tentative)* |
| `tracks` | `Vec<Id>` | refs to child `AudioTrack`s |
| `track_progress` | `IndexProgress` | rollup; truth = each `AudioTrack.index_stage` |

(`AudioFileRecord` dissolved; per-recording tags + cover-art are per-`AudioTrack`.)

## Open questions (the crux for `Audio`)

- **A-loc — facet vs per-track (you stated *both* principles; this resolves
  which applies to diarization/transcript):** is the segment aggregate
  referenced at the **`Audio` facet** (`Audio.segments` — parallel to
  `Video.scenes`; one logical transcript/diarization for the audio) **or**
  per-**`AudioTrack`** (each audio track diarized/transcribed independently —
  consistent with "each track runs the full pipeline" + multi-track files)?
  *My lean: facet-level for consistency with locked `Video.scenes` + the
  parallel you drew — but multi-track caveat: facet-level loses which track a
  transcript came from. Your call.*
- **A-agg:** one unified segment aggregate (a timeline span carrying speaker +
  transcript text) vs separate `Diarization` / `Transcript` aggregates. (Its
  internal schema is designed in the analysis phase — naming/shape only now.)
- **A-name:** `segments` vs `analyses` vs `transcript`.

## Projection notes

- **sqlx**: `audio` table = `id` PK (uuid); `segments`→join; `tracks` via
  `audio_track.audio_id` FK; `track_progress.*`.
- **mongodb**: `_id`=UUIDv7; `segments`/`tracks` UUID ref arrays.
- **graphql**: `track_progress` + transcript/diarization via the segment
  aggregate; tags via `AudioTrack`.

**Status: LOCKED (rev 7) — user-approved.**
