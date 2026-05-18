# `Scene<Id>` — a detected video scene/shot  *(rev 1 — drafted for review, NOT self-locked)*

## Domain meaning

One detected scene/shot segment of a video stream — the **heavy segmented-ML
aggregate** referenced by `VideoTrack.scenes` (detection runs per video stream,
so the parent is the **track**, not the facet; the facet keeps only the
`Video.total_scenes` rollup). Carries the scene's time span, representative
keyframe refs, the VLM/caption output, and the embedding for similarity search.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Media-time = `mediatime` (extern). Scenes have **no
progress lifecycle** — an id list + count rollup only (not an index stage).
Strings = `SmolStr`. Conversions deferred.

## Fields

| field | domain type | wire origin | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | `*.id: bytes` | canonical identity |
| `parent` | `Id` | scene→track | FK → `VideoTrack.id` (per-stream detection) |
| `index` | `u32` | ordinal | 0-based scene order within the track |
| `span` | `mediatime::TimeRange` | start/end pts | shot boundary span (media-time, extern; not wall-clock) |
| `keyframes` | `Vec<Id>` | — | refs → [keyframe.md](keyframe.md) (representative frame(s)) |
| `caption` | `Option<SmolStr>` | VLM text | VLM/caption description of the scene |
| `labels` | `Vec<SmolStr>` | tags | detected labels/tags |
| `embedding` | `Option<Embedding>` (nested VO) | vector | similarity-search vector (see open SC-embed) |
| `confidence` | `Option<f32>` | score | detector confidence |

## Nested value-objects

- **`Embedding`**: `{ model: SmolStr, dim: u32, vector: Vec<f32> }` — the
  scene/VLM embedding. Storage is projection-sensitive (see open).

## Invariants

`id` non-empty; `span.start <= span.end`; `index` unique within `parent`;
`embedding.vector.len() == embedding.dim` when present.

## Open questions

- **SC-time:** `mediatime::TimeRange` (recommended — true media-time, extern,
  consistent) vs frame-index pair (`first_frame`/`last_frame: u64`). *Lean:
  `TimeRange`, optionally also frame indices if the pipeline needs them.*
- **SC-embed (projection-critical):** store the vector **inline**
  (`Vec<f32>` → mongodb array / sqlx `vector`/`BLOB` / pgvector) vs a **ref to
  an external vector store** (`embedding_ref: SmolStr`) vs **both**. Affects all
  three projections + graphql (likely *not* exposed raw). *Lean: inline VO in
  the domain; projections decide physical (pgvector / Mongo Atlas vector / FAISS
  ref) — flag for the persistence sub-projects.*
- **SC-keyframes:** `Scene.keyframes → Keyframe` (recommended — keyframes are
  scene-scoped) vs `VideoTrack.keyframes` (track-scoped, scene references by
  pts). *Lean: Scene-scoped.*
- **Scene detector metadata** (algo/threshold) — per-scene, or once on the
  track's index state? *Lean: track-level (not per scene).*

## Projection notes

- **sqlx**: `scene` table; `id` PK; `parent` FK (`video_track_id`); `span`→
  `start_pts`/`end_pts`(+timebase via track); `labels`→ side table or `text[]`;
  `embedding`→ `vector`/pgvector or side table; `(parent, index)` unique.
- **mongodb**: `_id`=UUIDv7; `keyframes` UUID ref array; `embedding` array
  (Atlas vector index).
- **graphql**: caption/labels/span/keyframes exposed; embedding **not** exposed
  raw (search endpoint instead).

**Status: in review (rev 1) — drafted for your one-by-one review. NOT self-locked.**
