# `Subtitle<Id>` — subtitle facet (thin aggregate)  *(rev 3 — LOCKED, user-approved)*

## Domain meaning

The subtitle facet of a `Media` (via `Media.subtitle`). A **thin aggregate**:
just the grouping of this media's subtitle tracks + indexing roll-up. **No
scalar metadata of its own** — file-level data is on `Media`; per-track detail
(format/language/role/origin/codec) and analysis (cues/OCR/search) are
per-`SubtitleTrack`.

## Cross-cutting

Generic over `Id` (UUIDv7 single key). Conversions deferred.

## Fields

| field | domain type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | facet id (referenced by `Media.subtitle`; tracks back-ref it) |
| `tracks` | `Vec<Id>` | refs to child `SubtitleTrack`s |
| `track_progress` | `IndexProgress` | rollup; truth = each `SubtitleTrack.index_stage` |

That's all. An external `.srt`/`.vtt` is one track; embedded subtitles are N
tracks — all per-track detail lives on `SubtitleTrack`.

## Projection notes

- **sqlx**: `subtitle` table = `id` PK (uuid); `tracks` via
  `subtitle_track.subtitle_id` FK; `track_progress.*`.
- **mongodb**: `_id`=UUIDv7; `tracks` UUID ref array; progress embedded.

**Status: LOCKED (rev 3) — user-approved.**
