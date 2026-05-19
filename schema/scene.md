# `Scene<Id>` — a described video segment  *(rev 6 — LOCKED, user-approved)*

## Domain meaning

One **described time segment** of a video stream — e.g. a ~5–10 s span
captioned *"Jane is eating"*. Referenced by `VideoTrack.scenes` (detection per
video stream → parent is the **track**; facet keeps the `Video.total_scenes`
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
| `parent` | `Id` | scene→track | FK → `VideoTrack.id` |
| `index` | `u32` | ordinal | 0-based scene order within the track |
| `span` | `mediatime::TimeRange` | start/end pts | the segment (e.g. 5–10 s); media-time |
| `detector` | `SceneDetector` (enum) | — | which detector raised this scene (you asked) |
| `keyframes` | `Vec<Id>` | — | refs → [keyframe.md](keyframe.md) — these **are** the thumbnails |
| `description` | `SmolStr` | VLM text | e.g. `"Jane is eating"`; `""` = none |
| `provenance` | `Provenance` (shared VO) | — | analysis-run reproducibility; shared cross-cutting VO ([README.md](README.md)) |

**`SceneDetector`** (mediaschema-owned enum; the 5 scenesdetect detection
modules + manual): `Histogram` · `Phash` · `Threshold` · `Content` ·
`Adaptive` · `Manual` (user-created / imported boundary). `#[non_exhaustive]`.

## Invariants

`id` non-empty; `span.start <= span.end`; `index` unique within `parent`.

## Resolved (your calls)

- **`labels` dropped** — per-keyframe analysis (apple-vision
  `classifications`/`objects` + VLM `categories`/`tags`/`vlm_subjects` +
  colorthief `colors`) already covers frame-level search; *user* tags live in
  the scene-level smart-folder layer ([smart_folder.md](smart_folder.md)).
- **Smart folders are scene-level** (not keyframe-level): `SceneAnnotation`
  targets `Scene.id`; `Scene` carries **no** curation field.

## Projection notes

- **sqlx**: `scene` table; `id` PK; `parent` FK; `span`→`start/end_pts`;
  `detector` enum col (indexed — filterable in smart folders);
  `(parent, index)` unique. No embedding column (LanceDB).
- **mongodb**: `_id`=UUIDv7; `keyframes` UUID ref array.
- **graphql**: `description`/`span`/`keyframes`/`detector`;
  similarity = LanceDB endpoint keyed by `id`.

**Status: LOCKED (rev 6) — user-approved.** Thin described-segment:
`id`/`parent`(→VideoTrack)/`index`/`span`(mediatime)/`detector`(SceneDetector)/
`keyframes`(=thumbnails)/`description`/`provenance`(shared VO). `labels`
dropped; embeddings→LanceDB; curation = scene-level smart-folder layer.
