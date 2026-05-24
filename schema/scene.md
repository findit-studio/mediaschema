# `Scene<Id>` — a described video segment  *(rev 7 — LOCKED, user-approved)*

## Domain meaning

One **described time segment** of a video stream — e.g. a ~5–10 s span
captioned *"Jane is eating"*. Referenced by `VideoTrack.scenes` (detection per
video stream → `video_track_id` is the **track**; facet keeps the `Video.total_scenes`
rollup). Keyframes **are** the thumbnails (a scene has multiple). Immutable
detected+analysed record — **user curation (tags/favorite/…) is NOT here**; it
lives in the separate mutable layer ([smart_folder.md](smart_folder.md)).

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Media-time = `mediatime` (extern). No progress
lifecycle (id list + count rollup). Strings = `SmolStr` (`""`=absent).
**Embeddings live in LanceDB** (keyed by this `id`) — no embedding field.
`SceneDetector` is **mediaschema-owned** (scenesdetect is an engine crate →
flatten/own, not extern). Conversions deferred.

## Fields

| field | domain type | wire origin | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | `*.id: bytes` | canonical identity (LanceDB vector key) |
| `video_track_id` | `Id` | scene→track | FK → `VideoTrack.id` |
| `index` | `u32` | ordinal | 0-based scene order within the track |
| `span` | `mediatime::TimeRange` | start/end pts | the segment (e.g. 5–10 s); media-time |
| `detector` | `SceneDetector` (enum) | — | which detector raised this scene (you asked) |
| `keyframes` | `Vec<Id>` | — | refs → [keyframe.md](keyframe.md) — these **are** the thumbnails |
| `description` | `SmolStr` | VLM text | e.g. `"Jane is eating"`; `""` = none |

**`Provenance` lives on `VideoTrack`, not here** — one model bundle pins
all scene-detector + VLM runs for a track (per-track-per-run lock at the
indexer). Storing `Provenance` per-`Scene` would duplicate the same
`{model_name, model_version, prompt_version, indexer_version}` tuple
across tens-to-hundreds of records per track. See
`video_track.md`'s `provenance` field.

**`SceneDetector`** (mediaschema-owned enum; the 5 scenesdetect detection
modules + manual): `Histogram` · `Phash` · `Threshold` · `Content` ·
`Adaptive` · `Manual` (user-created / imported boundary). `#[non_exhaustive]`.

## Invariants

`id` non-empty; `span.start <= span.end`; `index` unique within `video_track_id`.

## Resolved (your calls)

- **`labels` dropped** — per-keyframe analysis (apple-vision
  `classifications`/`objects` + VLM `categories`/`tags`/`vlm_subjects` +
  colorthief `colors`) already covers frame-level search; *user* tags live in
  the scene-level smart-folder layer ([smart_folder.md](smart_folder.md)).
- **Smart folders are scene-level** (not keyframe-level): `SceneAnnotation`
  targets `Scene.id`; `Scene` carries **no** curation field.
- **rev 7 (user-authorised reopen):** `provenance` field **removed** —
  hoisted up to `VideoTrack.provenance`. The indexer pins one model
  bundle per track-per-run, so every `Scene` inside a `VideoTrack`
  shares the same provenance tuple; storing it per-`Scene` duplicated
  the same value across tens-to-hundreds of records per track. Audio +
  Subtitle already lived at the track level — this brings video into
  line. Same change applied to `keyframe.md`.

## Projection notes

- **sqlx**: `scene` table; `id` PK; `video_track_id` FK; `span`→`start/end_pts`;
  `detector` enum col (indexed — filterable in smart folders);
  `(video_track_id, index)` unique. No embedding column (LanceDB).
- **mongodb**: `_id`=UUIDv7; `keyframes` UUID ref array.
- **graphql**: `description`/`span`/`keyframes`/`detector`;
  similarity = LanceDB endpoint keyed by `id`.

**Status: LOCKED (rev 7) — user-approved.** Thin described-segment:
`id`/`video_track_id`(→VideoTrack)/`index`/`span`(mediatime)/`detector`(SceneDetector)/
`keyframes`(=thumbnails)/`description`. `Provenance` lives on
`VideoTrack` (per-track-per-run, not per-Scene); `labels` dropped;
embeddings→LanceDB; curation = scene-level smart-folder layer.
