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
- **Content-addressed, not a traditional filesystem (user, foundational).**
  Dedup key = the content hash (`FileChecksum`, unique index): **same hash ⇒
  same `Media`** (one row, regardless of how many copies/paths exist). A
  `Location`/path is **discovery provenance, not identity** — the same content
  can live at many locations under many roots. Deliberate consequence: the
  schema models **no explicit `Media`↔`WatchedLocation` link** (no
  `Media.source`, no sighting aggregate — user decision); "what a scan found"
  is the scanner's concern, surfaced only as `WatchedLocation.media_count`.
  Object-storage `Location` (`Object`/`StorageProvider`) is **deferred** —
  `Location` is `Local`-only for now (the `RemoteUrl` stub stays a stub).
- **Time — two different concepts, do not conflate:**
  - `mediatime::{Timestamp,TimeRange,Timebase}` are **media-timeline** types
    (`Timestamp = { pts: i64, timebase }` — a presentation timestamp in
    timebase ticks). Used **only** for genuine media-time fields (e.g.
    `TrackTime`). Stay extern (`::mediatime`), reused, never re-modeled.
  - Wall-clock instants (`created_at`, `capture_date`) use **`jiff::Timestamp`**
    (Unix **milliseconds** — matches findit-proto's
    `jiff::Timestamp::now().as_millisecond()`). **Never** `mediatime::Timestamp`.
- **`mediaframe` extern (parallel to `mediatime`) — renamed from `videoframe`,
  charter BROADENED (user decision):** `mediaframe` is the generic
  **media-stream descriptor vocabulary** crate for **all three kinds**
  (video + audio + subtitle), not pixel/colour/frame only. It owns: pixel/
  colour/frame (`PixelFormat`, `ColorInfo`+colour enums, `DcpTargetGamut`,
  `Dimensions`, `Rect`, `Rotation`, `SampleAspectRatio`, HDR static, `Rational`,
  `FrameRate`, `FieldOrder`, `StereoMode`, `DolbyVisionConfig`) **and** (added
  incrementally as the review surfaces them) audio stream vocab
  (`ChannelLayout`, sample format, EBU-R128 `Loudness`, …) **and** subtitle
  stream vocab (`SubtitleCodec`/format, …). `#![no_std]`, depends on
  `mediatime`. mediaschema **does not define/regenerate** these — extern-map
  `.mediaframe.v1` → `::mediaframe` exactly like `.mediatime.v1` →
  `::mediatime`, and **dedup mediaschema's own migrated `Dimensions`**.
  **Revised boundary test:** *what a media stream/frame **is** (any kind —
  codec/profile/level, format, frame_rate, rotation/SAR, channel layout,
  loudness, subtitle codec) → `mediaframe`*; *how findit **indexes/relates**
  media (the `Media` graph, facets, per-kind indexing state, scenes, errors,
  identity) → mediaschema*. **Ripple:** this moves several "stream descriptor"
  types previously tagged **MS** (e.g. `VideoCodec`/`AudioCodec`/
  `SubtitleCodec`/profile/level/`ChannelLayout`/`Loudness`) toward
  **mediaframe** — re-tagged **per-doc during the ongoing one-by-one review**
  (not retroactively bulk-rewritten; already-locked docs get only the
  mechanical `::mediaframe` reference rename until explicitly re-reviewed).
- **Embeddings live in LanceDB, not the domain (user decision).** All
  similarity vectors — embeddings **and** Apple `feature_print`
  (`Scene`/`Keyframe`/`AudioSegment`/…) — are stored in **LanceDB**, keyed by
  the aggregate's UUIDv7 `id`. Domain aggregates carry **no** vector field;
  sqlx/mongodb projections have **no** vector column; graphql exposes
  similarity as a LanceDB-backed endpoint keyed by `id` (never a raw vector).
  Cancels the earlier shared-`Embedding`-VO idea.
- **Extern vs flatten — vocabulary crates only.** Extern (`::mediatime`,
  `::mediaframe`) is reserved for **shared *vocabulary* crates**. (Language is
  `mediaframe::Language` — BCP-47, wraps `icu_locid` subtags; the earlier
  separate-`medialang`-crate plan is **superseded** — folded into `mediaframe`
  as media-stream descriptor vocab, your call.) Output types
  of **engine/task/service crates** (`llmtask::ImageAnalysis`,
  `scenesdetect::FrameMetrics`, `findit-apple-vision`) are **flattened /
  mediaschema-owned**, *not* externed — never take a dependency on an engine
  crate for a data shape. Keyframe analysis is **producer-distinct, not
  producer-agnostic**: apple-vision = structured detections (calibrated
  `confidence`+`bbox`); VLM = a grouped `VlmAnalysis` VO of flat label/text
  fields (incl. `mood`/`emotion`/`lighting`), no confidence; colorthief =
  `colors`. (`llmtask`'s `""`=absent / no-confidence conventions adopted.)
- **Shared `Provenance` VO (user decision).** Analysis-run reproducibility is
  **one cross-cutting VO**, not redefined per doc:
  `Provenance { model_name, model_version, prompt_version, indexer_version }`
  (all `SmolStr`, `""`=absent). **Lives at the track level only** — one
  `provenance` field per `VideoTrack` / `AudioTrack` / `SubtitleTrack`. The
  indexer pins one model bundle per track-per-run, so every leaf analysis
  record inside a track (`Keyframe`, `Scene`, `AudioSegment`, `SubtitleCue`)
  inherits the track's `Provenance` rather than carrying its own. This
  saves ~10 GB at library scale (100M keyframes × ~100 B) and is the
  natural "one run = one provenance" granularity. Re-running just one
  track creates a new track-level run, others unaffected.
  Supersedes the ad-hoc `asr_model`/`model_version` recommendations in
  the audio docs.
- **Shared `LocalizedText` VO (user decision).** Free-text with an optional
  translation is **one cross-cutting VO**:
  `LocalizedText { src: SmolStr, translated: SmolStr }` (both `""`=absent;
  single translation target = English by convention; **no** `src_lang` —
  deferred, language stays the parent field where it exists). Rule: **all VLM
  natural-language output is `LocalizedText`** — including open-vocab label
  vectors (`Vec<LocalizedText>` for `categories`/`objects`/`subjects`/`mood`/
  `emotion`/`lighting`), because the VLM emits them in its response language.
  Only **controlled** labels/enums stay plain (`shot_type`). *Adopted in
  `Keyframe.VlmAnalysis` (all 7 text fields incl. `tags`)*; `Scene.description`
  / `AudioSegment.text` (folds in `translated_text`) deferred (video-first).
- **User-curation / smart-folder layer is separate & mutable
  ([smart_folder.md](smart_folder.md)).** `UserTag`/`SceneAnnotation`
  (favorite/rating/note/user-tags)/`SmartFolder` (saved dynamic filter) are a
  user-owned mutable layer over the **immutable** detected+analysed aggregates
  — never mixed into `Scene`/`Keyframe`. User tags ≠ VLM `tags`/`labels`.
  Engine-derived enums (`SceneDetector` = scenesdetect's 5 detectors,
  `KeyframeExtractor`) are **mediaschema-owned** (extern-vs-flatten rule).
- **EXIF / capture metadata → `::mediaframe` (user decision 2026-05-19,
  broadens the charter).** mediaframe now owns **media-stream descriptor
  *and* EXIF/capture-metadata** vocabulary. `GeoLocation { lat, lon,
  altitude }` (owned, decimal degrees; ISO-6709 parse/format) plus
  device/camera make·model, capture-time, lens/exposure, orientation
  (`Rotation` already there) are **mediaframe** types; mediaschema externs
  them. *Supersedes the earlier "mediaschema owns `GeoLocation`" bullet*
  (`locat::Coordinate` borrowed-vs-owned still motivates an owned type — it
  just lives in mediaframe now). findit-proto has these only as loose
  `MediaMeta` `SmolStr` fields (ISO-6709 location string + `device_*`); the
  structured types are a mediaschema redesign that lands in mediaframe.
  Tracked in [mediaframe-candidates.md](mediaframe-candidates.md); batched
  post-`0.1.0`. Locked docs get the mechanical `::mediaframe::` rename — but
  where a locked doc currently models capture data as a *string* (e.g.
  `media.md` ISO-6709 `SmolStr`), switching to the structured type is a
  content change requiring an explicit re-review, not a silent rename.
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
  distinct vocabularies, kept as `bitflags!` companions), the derived
  `IndexStage` lifecycle, and `index_errors` live on the **Track** schemas
  (`VideoTrack`/`AudioTrack`/`SubtitleTrack`), **not** the kind containers.
  **No per-track `error_status`** — error-state is derived from stage-coded
  `index_errors` + `index_status` (user decision); `MediaErrorFlags` root
  rollup kept. A
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
- **Indexing errors/status are API-facing, not internal:** `index_errors`
  (stage-coded) + derived error-state and indexing progress are **exposed by
  the graphql projection** on every aggregate (clients display why/whether
  indexing succeeded). Only truly internal plumbing is dropped from the API.
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
- **Content / copy split (resolved):** `Media` is **not** the file — it is
  the **content row**, one per content hash. The per-copy filesystem identity
  lives on the separate [`MediaFile`](media_file.md) aggregate (N copies ↔ 1
  `Media`). **`Media`** = the content row (`id`, `checksum` UNIQUE, container
  `format`, `size`, overall `duration`, `kind`, EXIF
  `capture_date`/`device`/`gps`, `error_flags`, `probe_error`, 3 facet refs,
  plus `files: Vec<Id>` — the reverse lookup to its copies). It carries **no**
  `name` and **no** `created_at` — those are copy-specific.
  **`MediaFile`** = one physical copy (`id`, `media_id` FK → `Media`,
  `created_at` filesystem creation time, `location` structured volume-aware
  path, `watched_location_id` FK → discovering `WatchedLocation`); `name` is
  **derived** from `location`'s last path component, never stored. The
  `media_file` table/collection requires: PK `id`, FK `media_id` → `media`,
  FK `watched_location_id` → `watched_location`.
- **Three-level data placement (resolved):** scalar data lives at the
  *content*, *copy*, or *track* level, never the facet. **`Video`/`Audio`/`Subtitle`** =
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
- [x] [media.md](media.md) — `Media` (content row, one per content hash) — **LOCKED (rev 9)** — content/copy split: no `name`/`created_at`, carries `files: Vec<Id>`; `Device`/`GeoLocation` → `::mediaframe` (EXIF/capture)
- [x] [media_file.md](media_file.md) — `MediaFile` (per-copy row, N ↔ 1 `Media`) — codex PR #13: derived `name`, `created_at`, structured `location`, volume-scoped `watched_location_id`
- [x] [subtitle.md](subtitle.md) — `Subtitle` facet — **LOCKED (rev 3)**
- [x] [audio.md](audio.md) — `Audio` facet — **LOCKED (rev 8)** — A-loc cascade applied: facet `segments` removed, `total_segments` rollup; per-track segments
- [x] [audio_track.md](audio_track.md) — `AudioTrack` — **LOCKED (rev 3)** — A-loc=per-track; cascades + adopt/defer; `speakers→Speaker`
- [x] [speaker.md](speaker.md) — `Speaker` (per-track diarized voice) — **LOCKED (rev 1)** — voiceprint→LanceDB *(rev 2 drafted — adds per-track `voiceprint: Option<VoiceFingerprint>` + `person: Option<Id>` FK; in-review)*
- [ ] [person.md](person.md) — `Person` (cross-track / cross-modality identity anchor) — **drafted (rev 1)** — `voiceprint: Option<VoiceFingerprint>` aggregated centroid; `Speaker.person` reverse FK; modality-neutral for future face/visual identity
- [x] [video.md](video.md) — `Video` facet — **LOCKED (rev 8)** — user-approved
- [x] [video_track.md](video_track.md) — `VideoTrack` — **LOCKED (rev 6)** — user-approved (`error_status` removed)
- [x] [scene.md](scene.md) — `Scene` — **LOCKED (rev 6)** — user-approved
- [x] [keyframe.md](keyframe.md) — `Keyframe` — **LOCKED (rev 15)** — user-approved
- [x] [subtitle_track.md](subtitle_track.md) — `SubtitleTrack` — **LOCKED (rev 3)** — user-approved (`error_status` removed)
- [x] [subtitle_cues.md](subtitle_cues.md) — `SubtitleCue` — **LOCKED (rev 3)** — user-approved
- [x] [enums.md](enums.md) — domain enums — **LOCKED (rev 4)** — user-approved (+ `ErrorCode`; descriptor enums → `::mediaframe`; audio enums pending audio review)
- [x] [bitflags.md](bitflags.md) — domain bitflags — **LOCKED (rev 4)** — user-approved (no `error_status`; `TrackDisposition`→`::mediaframe`)
- [x] [primitives.md](primitives.md) — domain newtypes — **LOCKED (rev 5)** — user-approved (+`ErrorInfo`/`ErrorCode`, structured `Location`; `Rational`/`GeoLocation`/**`Language`**(ex-`LanguageCode`)→`::mediaframe`; `medialang`-crate plan superseded)
- [x] [audio_segments.md](audio_segments.md) — `AudioSegment` — **LOCKED (rev 3)** — user-approved (A-loc=per-track; `speaker`→`Speaker`; `dia`/`asry`-grounded; `mediaframe::Language`) *(rev 4 drafted — adds per-segment `voice_fingerprint: Option<VoiceFingerprint>`; in-review)*
- [x] [watched_location.md](watched_location.md) — `WatchedLocation` — **LOCKED (rev 5)** — user-approved (FS-event monitor; `is_ejectable`; no link/count/globs; `Local`-only)
- [x] [wire-only.md](wire-only.md) — WIRE-only boundary — **LOCKED (rev 1)** — user-approved (W-verify passed; query-layer disposition deferred to post-storage+indexer)

*In review — for your one-by-one pass (suggested order):*
- [tracking] [mediaframe-candidates.md](mediaframe-candidates.md) — `videoframe`→**`mediaframe`** rename + charter broadened (all media-stream vocab); **PR #3** = `mediaframe 0.1.0` (rename + Rational/FrameRate/FieldOrder/StereoMode/DolbyVisionConfig + SAR-via-Rational); audio/subtitle stream vocab appends here as the review surfaces it

*Superseded (stubs, one product Q survives in AFR):*
- [~] [track_core.md](track_core.md) — no shared track type
- [~] [audio_file_record.md](audio_file_record.md) — dissolved → `AudioTrack`; **AFR1 product Q open**

**Cross-doc decisions awaiting you (highest-leverage first):**
1. **A-loc cascade** — move audio diarization/transcript refs per-`AudioTrack`
   (+ `Audio.total_segments`), reopening locked `audio.md`, mirroring the
   `Video.scenes → VideoTrack` move? (`audio_track.md`/`audio_segments.md`)
2. **video.md rev 8 / video_track.md rev 4** re-approval (mediaframe boundary
   now real via PR #2).
3. **bitflags.md** per-kind index/error **stage vocabulary** — needs your real
   pipeline definition (everything stage-derived depends on it).
4. **AFR1** — standalone music file = `Media(kind=Audio)` vs standalone entity.
5. ST-cues / SC-embed / KF-parent / WL-scope — modeling leans noted, your call.
