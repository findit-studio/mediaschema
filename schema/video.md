# `Video<Id>` — video facet (thin aggregate)  *(rev 10 — LOCKED, user-approved; `media_id` FK to Media)*

## Domain meaning

The video facet of a `Media` (via `Media.video`). A **thin aggregate**: groups
this media's video tracks + indexing roll-up. Scene refs now live on
**`VideoTrack`** (scene detection is per video stream); the facet keeps only a
`total_scenes` rollup count.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7 single key). Conversions deferred.

## Fields

| field | domain type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | facet id (referenced by `Media.video`; tracks back-ref it) |
| `media_id` | `Id` (UUIDv7) | **FK → `Media.id`** (rev 9): the `Media` this facet belongs to. Set at construction; identity-bearing (no setter). Mirrors locked
`Subtitle.media_id`; makes the three facets (Audio/Video/Subtitle) uniform in their back-reference to `Media`. |
| `total_scenes` | `u32` | rollup = Σ over its `VideoTrack`s of `scenes.len()` (maintained denormalized cache) |
| `tracks` | `Vec<Id>` | refs to child `VideoTrack`s |
| `track_progress` | `IndexProgress` | rollup; truth = each `VideoTrack` index state |

Change from rev 8: `parent: Id` FK to `Media` added (uniform with locked
`Subtitle.media_id`; foundation for sqlx/mongodb schema additions). Change
from rev 7: `scenes: Vec<Id>` **removed** → moved to `VideoTrack.scenes`
(per-track). No scalar metadata of its own; file-level → `Media`,
stream/scene-level → `VideoTrack`.

## Projection notes

- **sqlx**: `video` table = `id` PK (uuid); `media_id` uuid FK → `media.id`;
  `total_scenes` column; `tracks` via `video_track.video_id` FK;
  `track_progress.*`. (No scene join at the facet — scenes are
  `video_track`-scoped.)
- **mongodb**: `_id`=UUIDv7; `media_id` Binary(uuid); `tracks` UUID ref array;
  counts/progress embedded.
- **graphql**: `total_scenes` + `track_progress`; scene detail via `VideoTrack`.

**Status: LOCKED (rev 10) — user-approved.** *(rev 10: FK field renamed
`parent` → `media_id` (consistent `<target>_id` naming across all FK
columns/fields). rev 9: `media_id: Id` FK to `Media` added — mirrors
locked `Subtitle.media_id` so the three facets (Audio/Video/Subtitle)
share a uniform back-reference; additive on top of `Media.video_id`.)*
