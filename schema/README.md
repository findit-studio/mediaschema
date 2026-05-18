# mediaschema — Domain Schema

Canonical documentation of the mediaschema **domain / programming-types layer**:
what each type means, the design intent ("the idea about the types"), and how it
relates to the buffa wire types and the downstream projections.

> Tracked folder (distinct from the local-only `docs/`). This is the design
> record we review **type-by-type**; it precedes the domain-layer implementation.

## Architecture

```
DOMAIN / programming-types   ← application logic programs against THIS
 (idiomatic Rust: bitflags!, real enums, SmolStr, Id<T> newtypes,
  non-optional invariants)
   ├── ⇄ wire (buffa)     serialization / RPC edge  (proto/media/v1/types.proto)
   ├── →  sqlx suite      persistence projection (Postgres/SQLite/MySQL)
   ├── →  mongodb suite   persistence projection
   └── →  async-graphql   API projection (curated)
```

- The buffa-generated types remain the wire/RPC contract. The domain layer is
  hand-written, governed by `domain → wire → domain` round-trip property tests.
- Scope is **curated** — aggregate + invariant-bearing types only, **not** a
  138× mirror. CV/meta value-objects become **nested domain sub-structs** of
  their aggregate (documented within that aggregate's doc), *not* separate
  aggregates and *not* flattened.

## Classification (from the type audit — `/tmp/type-audit.md`)

| Bucket | Count | Meaning |
|---|---|---|
| DOM-aggregate | ~14 | domain aggregate roots (one doc each) |
| DOM-value | ~51 | value objects — nested inside their aggregate |
| DOM-primitive | ~10 | `Id<T>`/`FileChecksum`/… newtypes |
| enums | 18 | 17 wire enums + new `IndexStage` |
| `bitflags!` companions | ~7 | typed flag sets over wire `u32` |
| WIRE-only | 63 | RPC/transport — no domain twin |
| REDESIGN | 13+ | proto model-correction before mirroring |
| DELETE | 0 | nothing removed; 3 codegen fixtures kept as guards, excluded from domain |

## Cross-cutting decisions

- **Genuine nested VOs stay nested; an entity's own `*Meta` is flattened IN.**
  Meaningful sub-value-objects (`Dimensions`, `TrackTime`, `Device`,
  `GeoLocation`, `AudioTags`, `AudioCoverArt`) stay nested in the domain. But a
  kind/root entity's own `*Meta` is **not** a sub-VO — it *is* the entity, so
  `Media`/`Video`/`Audio`/`Subtitle` are **flat** (no `meta:` wrapper). DB
  further flattens the remaining nested VOs (a persistence concern); the domain
  keeps only *meaningful* nesting. (Refines the original "preserve nesting" rule.)
- **Identity = structs generic over the id type** — `MediaMeta<Id>`,
  `Media<Id>`, … (the *struct* is generic; **not** a phantom `Id<Video>`). The
  aggregate graph shares one id type parameter; cross-entity references are that
  same id type. The id is a **UUIDv7** newtype (time-ordered). **Single key,
  no surrogate** — because UUIDv7 is monotonic it serves directly as the
  physical key in every projection; the generic `Id` param just carries its
  backend-native representation of the *same* value: Postgres `uuid` PK ·
  SQLite `BLOB(16)` PK · MySQL `BINARY(16)` PK · MongoDB `_id` = the UUIDv7
  (`BinData`, **not** `ObjectId` — that would re-introduce an id mapping).
  FKs are the UUIDv7 (no prog↔native resolution). `FileChecksum` stays a
  distinct 32-byte newtype (content hash ≠ identity). *Aside:* UUIDv7 leaks
  creation time — the **graphql** projection may expose an opaque id; this does
  not justify a DB surrogate.
- **Time — two different concepts, do not conflate:**
  - `mediatime::{Timestamp,TimeRange,Timebase}` are **media-timeline** types
    (`Timestamp = { pts: i64, timebase }` — a presentation timestamp in
    timebase ticks). Used **only** for genuine media-time fields (e.g.
    `TrackTime`). Stay extern (`::mediatime`), reused, never re-modeled.
  - Wall-clock instants (`created_at`, `capture_date`) use **`jiff::Timestamp`**
    (Unix **milliseconds** — matches findit-proto's
    `jiff::Timestamp::now().as_millisecond()`). **Never** `mediatime::Timestamp`.
- **`videoframe` extern (parallel to `mediatime`):** `videoframe` is the
  generic pixel/color/frame *data-vocabulary* crate (it already owns
  `PixelFormat`, `ColorInfo`+5 color enums+`DcpTargetGamut`, `Dimensions`,
  `Rect`; depends on `mediatime`). mediaschema **does not define/regenerate**
  these — extern-map `.videoframe.v1` → `::videoframe` exactly like
  `.mediatime.v1` → `::mediatime`, and **dedup mediaschema's own migrated
  `Dimensions`**. Requires adding a `buffa` feature + `videoframe.proto` to the
  `videoframe` crate — a separate sub-project (like mediatime#4), pending user
  go. Boundary test: *what a pixel/frame/color IS* → videoframe; *how findit
  indexes media* (incl. stream descriptors: codec/profile/level/frame_rate/
  rotation/SAR) → mediaschema. *(Borderline: rotation/SAR/HDR-static — user's call.)*
- **Geo = owned `GeoLocation`.** `locat::Coordinate<'a>` is zero-copy/borrowed,
  so the *domain* type is an owned `GeoLocation { lat, lon, alt }` (decimal
  degrees) parsed from the wire ISO 6709 string via `locat` (exact source
  form/CRS not retained in the programming type; DB stores decimal columns).
- **Error model — track-detailed, rolled up.** Error *details* live only on
  the **Track** (`*Track.index_errors: Vec<ErrorInfo>`). Higher levels carry a
  compact maintained roll-up, not detail: a kind entity's error signal is its
  `track_progress.failed > 0`; **`Media.error_flags`** is a `bitflags!` over
  **`u16`** (`VIDEO_ERROR`/`AUDIO_ERROR`/`SUBTITLE_ERROR` + bits reserved for
  future kinds) — bit set ⇒ drill down to the kind entity → track. The one
  non-track case: **`Media.probe_error: Option<ErrorInfo>`** (file couldn't be
  probed ⇒ no tracks to drill to). No `Vec<ErrorInfo>` on `Media`/kind entities.
- **Bitflags:** wire keeps bare `u32`; domain gets `bitflags!` companions.
  `index_status` is **6 distinct vocabularies** → **per-kind** companions
  (Video / Audio / Subtitle / `ProcessingStage` differ — do *not* unify).
  `disposition` is identical across the 3 track types → **one shared
  `TrackDisposition`**.
- **Indexing model-correction:** a reusable **`IndexProgress {total,indexed,
  failed}`** rollup of a container's child **tracks** (on `Video`/`Audio`/
  `Subtitle`) + a **per-kind** index/stage model — each kind's `*IndexStatus`
  bitflags + `index_errors` are the source of truth, the coarse status is
  **derived per-kind** (no single shared `IndexStage` enum; tracks differ per
  kind).
  Scenes (and likely audio analyses) have **no progress lifecycle** — only an
  id list + **total count** (`Video.scenes: Vec<Id>`). Containers also carry
  the forward **child id lists** (`tracks: Vec<Id>`, `scenes: Vec<Id>`) in
  addition to the child→parent back-ref. `Media`'s conflated rollup bitflag
  (`MediaIndexStatus`) is **removed** (per-kind "indexed?" derived from the
  containers' `IndexProgress`). **Indexing is track-based**: the per-kind
  **pipeline-stage** bitflags (`VideoIndexStatus`/`AudioIndexStatus`/… —
  distinct vocabularies, kept as `bitflags!` companions), the `IndexStage`
  lifecycle, `index_errors`, and `error_status` live on the **Track** schemas
  (`VideoTrack`/`AudioTrack`/`SubtitleTrack`), **not** the kind containers. A
  kind container holds only metadata + child id lists + the `IndexProgress`
  rollup (a denormalized cache of its tracks' state). Rollup ≠ pipeline.
  (`IndexProgress` was renamed from `TrackIndexProgress`.) *Open:* whether a
  one-shot container event like `PROBED` stays container-level.
- **Redesign staging (proposed, confirm per-type):** core Media/kind/track
  model-correction + `index_errors` collection + cheap-unambiguous
  (`SubtitleTrackOrigin`, `Tag→Rgba`, `language→LanguageCode`) first; the
  `Audio`/`AudioFileRecord`/`AudioMeta` reconciliation staged separately
  (highest modeling risk).
- **Conversion sequencing:** the domain types (generic over `Id`) are designed
  first; the `From`/`TryFrom` conversions (domain ↔ wire/rpc, → sqlx, → mongodb,
  → graphql) are **deferred until all four projections are defined**, then
  written once — avoids piecemeal rework as later projections shift the shape.
- **Pipeline errors:** `index_errors: Vec<ErrorInfo>`; `ErrorInfo.code`
  identifies the failing stage (no separate `stage` wrapper).
- **`kind` is kept, not derived:** it's set at probe and *drives* which
  kind-containers get created (an input to indexing, not a derivable output);
  domain `MediaKind { Video, Audio }` (wire `UNSPECIFIED` = pre-probe sentinel).
- **Indexing errors/status are API-facing, not internal:** `index_errors`,
  `error_status`, and indexing progress are **exposed by the graphql
  projection** on every aggregate (clients display why/whether indexing
  succeeded). Only truly internal plumbing is dropped from the API.
- **Audit is a hint, not authority:** the type-audit reasons from generated
  field-overlap; verify domain shape against findit-proto's Rust — *and* even
  that may be out of date (user said so), so genuine model/product decisions
  are the user's, not auto-derived. (The `Audio` vs `AudioFileRecord` split is
  a **pending proposal**, not a decided override.)
- **Analysis is per-track (agreed direction):** each track is processed by the
  full indexing pipeline, so *all* analysis output lives on the **track**
  schema (`VideoTrack`/`AudioTrack`/`SubtitleTrack`), not the kind container.
  The audio cluster is a **total redesign parallel to `Video`** (stale
  pre-track structures — `AudioAnalysis`/`AudioSummary`/`TrackRecord` sprawl —
  discarded, not migrated). Kind containers are trivial parallel shells; the
  **track schemas are the design focus**.
- **Multi-track ⇒ identity metadata is per-track:** an audio file can contain
  multiple tracks, each a distinct recording, so music tags + cover-art live on
  `AudioTrack`, not `AudioMeta` (matches findit-proto's original per-track
  `AudioStreamMeta` tags).
- **Three-level data placement (resolved):** scalar data lives at the *file*
  or *track* level, never the facet. **`Media`** = the file (`id`, `checksum`,
  `name`, container `format`, `size`, overall `duration`, `created_at`, EXIF,
  `error_flags`, `probe_error`, 3 facet refs). **`Video`/`Audio`/`Subtitle`** =
  thin **facet aggregates** = `{ id, <count rollups>, tracks: Vec<Id>,
  track_progress }` — no scalar metadata of their own. **`*Track`** = the
  *stream* (codec, dimensions, frame_rate, bit_rate, per-track duration, index
  state, **per-track segmented-analysis refs** + signal analysis; audio also
  tags + cover-art). Rule (revised): the heavy *segmented ML aggregate* is
  referenced **per-`*Track`** (`VideoTrack.scenes → Scene`; detection runs on
  the stream); the **facet keeps only a rollup count** (`Video.total_scenes`).
  *(Cascade to audio — `Audio.segments`→`AudioTrack` + `Audio.total_segments` —
  pending user call; `audio.md` locked at facet-level for now.)*

## Per-aggregate doc template

- **Domain meaning** — what it is, why it exists.
- **Wire counterpart** — buffa type(s); bucket; REDESIGN? (the exact change).
- **Fields** — table: domain field | domain type | wire origin | rationale/invariant.
- **Nested value-objects** — the VOs that live inside (kept nested, not flattened).
- **Invariants** — what the domain type guarantees that the wire type cannot.
- **Wire mapping** — `From`/`TryFrom` direction; lossy/fallible points.
- **Projection notes** — brief sqlx / mongodb / graphql remarks.
- **Open questions**.

## Index (one-by-one review status)

**All remaining docs drafted** (overnight) with recommended designs + explicit
open questions; **none self-locked**. Only **you** mark a doc resolved/locked —
I present, you approve.

*Locked (no action unless you reopen):*
- [x] [media.md](media.md) — `Media` (root/file) — **LOCKED (rev 7)**
- [x] [subtitle.md](subtitle.md) — `Subtitle` facet — **LOCKED (rev 3)**
- [x] [audio.md](audio.md) — `Audio` facet — **LOCKED (rev 7)** — ⚠ cascade Q below

*In review — for your one-by-one pass (suggested order):*
- [ ] [video.md](video.md) — `Video` facet — **rev 8, re-approval** (scenes → `VideoTrack`)
- [ ] [video_track.md](video_track.md) — **rev 4** — open Qs resolved (HDR/rotation/SAR → VF per videoframe PR #2); confirm VT-codec / VT-scenes / DoVi
- [ ] [subtitle_track.md](subtitle_track.md) — **rev 1** — + forgotten-info pass; ST-cues
- [ ] [audio_track.md](audio_track.md) — **rev 1, the big one** — + forgotten-info; **A-loc cascade** (reopens `audio.md`)
- [ ] [scene.md](scene.md) — **rev 1** — `VideoTrack.scenes →` ; SC-embed/SC-time
- [ ] [audio_segments.md](audio_segments.md) — **rev 1** — + forgotten-info; A-loc/A-agg/A-name
- [ ] [subtitle_cues.md](subtitle_cues.md) — **rev 1** — conditional on ST-cues=aggregate
- [ ] [keyframe.md](keyframe.md) — **rev 1** — KF-parent; phash/embedding
- [ ] [watched_location.md](watched_location.md) — **rev 1** — WL-scope/link (product)
- [ ] [primitives.md](primitives.md) — **rev 1** — Id/FileChecksum/LanguageCode/…
- [ ] [enums.md](enums.md) — **rev 1** — 18 enums; closed vs `#[non_exhaustive]`
- [ ] [bitflags.md](bitflags.md) — **rev 1** — per-kind status/error + shared disposition; **stage vocab needs your pipeline confirm**
- [ ] [wire-only.md](wire-only.md) — **rev 1** — 63 RPC types, no domain twin

*Superseded (stubs, one product Q survives in AFR):*
- [~] [track_core.md](track_core.md) — no shared track type
- [~] [audio_file_record.md](audio_file_record.md) — dissolved → `AudioTrack`; **AFR1 product Q open**

**Cross-doc decisions awaiting you (highest-leverage first):**
1. **A-loc cascade** — move audio diarization/transcript refs per-`AudioTrack`
   (+ `Audio.total_segments`), reopening locked `audio.md`, mirroring the
   `Video.scenes → VideoTrack` move? (`audio_track.md`/`audio_segments.md`)
2. **video.md rev 8 / video_track.md rev 4** re-approval (videoframe boundary
   now real via PR #2).
3. **bitflags.md** per-kind index/error **stage vocabulary** — needs your real
   pipeline definition (everything stage-derived depends on it).
4. **AFR1** — standalone music file = `Media(kind=Audio)` vs standalone entity.
5. ST-cues / SC-embed / KF-parent / WL-scope — modeling leans noted, your call.
