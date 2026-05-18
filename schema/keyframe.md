# `Keyframe<Id>` — an extracted representative frame  *(rev 1 — drafted for review, NOT self-locked)*

## Domain meaning

A representative frame extracted from a video stream — scene thumbnail / poster
/ interval sample. Referenced by `Scene.keyframes` (recommended parent: the
scene the frame represents). Carries the stored image artifact, its
visual embedding (CLIP-style) for image search, and a perceptual hash for
near-duplicate detection.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Media-time = `mediatime` (extern). `videoframe`
extern for `Dimensions`. No progress lifecycle. Strings = `SmolStr`.
Conversions deferred.

## Fields

| field | domain type | wire origin | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | `*.id: bytes` | canonical identity |
| `parent` | `Id` | kf→scene | FK → `Scene.id` (open KF-parent) |
| `pts` | `mediatime::Timestamp` | frame pts | exact source position (media-time) |
| `kind` | `KeyframeKind` (enum) | — | `Poster`/`SceneRepresentative`/`Interval`/`IFrame` |
| `location` | `Location` | path | stored artifact (object store / fs path) |
| `mime` | `SmolStr` | — | `image/jpeg`/`image/webp` |
| `dimensions` | `videoframe::Dimensions` | — (VF) | thumbnail W×H (extern) |
| `size` | `u32` | — | artifact bytes |
| `phash` | `Option<u64>` | — | perceptual hash — near-dup / dedup / "find similar frame" |
| `embedding` | `Option<Embedding>` | vector | CLIP-style image embedding (see `scene.md` SC-embed) |

## Invariants

`id` non-empty; `location` non-empty; `dimensions` positive; `embedding.vector
.len() == embedding.dim` when present.

## Open questions

- **KF-parent:** `Scene` (recommended — keyframes represent a scene) vs
  `VideoTrack` (track-scoped, scene by pts). *Lean: Scene; a poster frame for
  the whole track can be a `Scene`-less special case or `kind=Poster` on the
  first scene — confirm.*
- **Artifact storage:** `location` (object-store/fs path — recommended) vs blob
  in DB vs both. Projection-level (object store for sqlx/mongo; never inline in
  graphql).
- **`phash`/`embedding` adopt now?** *Lean: `phash` yes (cheap, enables dedup &
  reverse-image); `embedding` storage deferred to the persistence sub-projects.*

## Projection notes

- **sqlx**: `keyframe` table; `id` PK; `parent` FK; `phash` BIGINT (indexed for
  Hamming/near-dup); `embedding` → pgvector/side table; `location` = path.
- **mongodb**: `_id`=UUIDv7; `embedding` array (Atlas vector); `phash` indexed.
- **graphql**: `location` (signed URL)/`dimensions`/`pts` exposed; embedding not
  raw (similarity endpoint).

**Status: in review (rev 1) — drafted for your one-by-one review. NOT self-locked.**
