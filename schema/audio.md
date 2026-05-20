# `Audio<Id>` — audio facet (thin aggregate)  *(rev 8 — LOCKED, user-approved; A-loc=per-track)*

> Rev 8 = user-authorized reopen of r7 (the **A-loc cascade**): `segments`
> moved from facet → **per-track** (`AudioTrack.segments`, mirrors locked
> `VideoTrack.scenes`); the facet keeps only `total_segments` rollup.

## Domain meaning

The audio facet of a `Media` (via `Media.audio`). A **thin aggregate**: groups
this media's audio tracks + the indexing roll-up. The diarization/transcript
**segment** aggregate ([audio_segments.md](audio_segments.md)) is referenced
**per-`AudioTrack`** (not the facet), directly parallel to locked
`VideoTrack.scenes`; the facet keeps a `total_segments` rollup for cheap
"how many segments under this media" queries.

## Principle (rev 8 — A-loc resolved per-track)

- **Heavy *segmented ML outputs*** get their **own aggregate**, referenced
  **per-track**: video → `Scene` on `VideoTrack.scenes`; audio →
  `AudioSegment` on `AudioTrack.segments`. The facet keeps `total_*` rollup
  only.
- **Per-track *signal* analysis** (codec/stream, loudness/EBU R128,
  chromaprint, …) + per-track index state + `speakers` stay on
  **`AudioTrack`**.
- Per-track is required for multi-track audio files (N recordings): the
  schema preserves *which track* a transcript/diarization came from.

## Cross-cutting

Generic over `Id` (UUIDv7 single key). Conversions deferred.

## Fields

| field | domain type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | facet id (referenced by `Media.audio`; tracks back-ref it) |
| `tracks` | `Vec<Id>` | refs to child `AudioTrack`s |
| `total_segments` | `u32` | **rollup** (rev 8): Σ `AudioTrack.segments.len()` across this facet's tracks — cheap "how many segments under this media" facet (mirrors locked `Video.total_scenes`). Truth = the per-track `AudioTrack.segments`. *(Replaces the old facet-level `segments: Vec<Id>`.)* |
| `track_progress` | `IndexProgress` | rollup; truth = each `AudioTrack.index_stage` |

(`AudioFileRecord` dissolved; per-recording tags + cover-art are per-`AudioTrack`.)

## Resolved (rev 8 cascade)

- **A-loc = per-track** ([audio_track.md](audio_track.md), [audio_segments.md](audio_segments.md));
  facet `Audio.segments` removed; `total_segments` rollup added.
- **A-agg = unified** `AudioSegment` (reconciled `dia`⋈`asry`).
- **A-name = `segments`** (`AudioTrack.segments` / `AudioSegment` /
  `Audio.total_segments`).
- *Speakers* per-track on `AudioTrack.speakers` → [speaker.md](speaker.md).

## Projection notes

- **sqlx**: `audio` table = `id` PK (uuid); `tracks` via `audio_track.audio_id`
  FK; `total_segments` `u32` column (rollup of `audio_segment` rows joined
  through `audio_track`); `track_progress.*`.
- **mongodb**: `_id`=UUIDv7; `tracks` UUID ref array; `total_segments` int.
- **graphql**: `track_progress` + `total_segments`; transcript/diarization
  resolves via `AudioTrack.segments` → `AudioSegment`; tags via `AudioTrack`.

**Status: LOCKED (rev 8) — user-approved.** *(rev 8: A-loc cascade complete —
`segments: Vec<Id>` at facet level **removed**, replaced by `total_segments:
u32` rollup; segments now live per-track on `AudioTrack.segments`. Mirrors
locked `Video`/`VideoTrack.scenes` model.)*
