# `SoundEvent<Id>` — a detected sound event  *(rev 2 — drafted)*

## Domain meaning

One **detected, time-ranged sound classification** on an **audio track** —
the audio analog of [`Scene`](scene.md). Where `Scene`
records a detected video segment, `SoundEvent` records a sound the **CED**
(sound-event detector) raised over a window of an `AudioTrack`: a `label`
(class name, e.g. *"Speech"* / *"Music"* / *"Doorbell"*), an optional stable
soundevents `code`, and a `[0,1]` `score`. `audio_track_id → AudioTrack.id`
(**A-loc = per-track** — mirrors locked `AudioSegment.audio_track_id` and
`VideoTrack.scenes`; multi-track files keep which-track attribution).

Produced by the audio `CED_DONE` stage (see `AudioIndexStatus::CED_DONE` /
the `Ced*` `ErrorCode`s). This is the first-class home for the
sound-event vocabulary that `audio_segments.md` deferred ("`SegmentKind`
… would come from the separate CED/CLAP stage").

No progress lifecycle: a `SoundEvent` exists iff its row exists. It is wired
into `AudioTrack` exactly like its sibling `AudioSegment`:
`AudioTrack.sound_events: Vec<Id>` holds the per-track refs and the `Audio`
facet keeps a `total_sound_events` rollup
(`Σ AudioTrack.sound_events.len()`, mirroring `total_segments`). In the
object-graph shape ([graph.md](graph.md)) `graph::SoundEvent` is embedded in
`graph::AudioTrack` (`sound_events: Vec<SoundEvent>`, lifted via
`try_from_flat`), again mirroring `segments`.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Media-time = `::mediatime` (`TimeRange`).
Strings = `SmolStr` (`""` = absent, **no `Option`**). **Embeddings → an
external vendor-neutral vector store** keyed by this `id`, if ever embedded
— no embedding field here. **`Provenance` is per-track** (on `AudioTrack`,
one per run) — not per sound event, exactly as for `Scene` /
`AudioSegment`. sqlx (postgres/mysql/sqlite), mongodb, and buffa (media.v1
flat + media.v2 graph) bridges are all implemented (see Projection notes).

## Fields

| field | domain type | source | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | — | canonical identity |
| `audio_track_id` | `Id` | event→track | FK → `AudioTrack.id` (**A-loc per-track**) |
| `index` | `u32` | ordinal | 0-based sound-event order within the track |
| `span` | `mediatime::TimeRange` | CED | detected window (media-time) |
| `label` | `SmolStr` | CED | CED class name, e.g. `"Speech"`; `""` = absent |
| `code` | `Option<u64>` | CED | stable soundevents dataset code (the src `CedTag` u64); `None` = unmapped class |
| `score` | `f32` ∈ `[0,1]` | CED | detection confidence; finite, validated `[0,1]` |

`score` is a validated raw `f32` (finite, `[0,1]`), mirroring this
cluster's `Word.score`. It is **not** the video-cluster `Confidence`
value-object: `Confidence` lives behind the `video` feature, and
`SoundEvent` is an `audio`-feature aggregate that must compile without
`video` — the invariant is identical either way.

The CED model that produced this track's sound events is recorded
per-track on [`AudioTrack.ced_provenance`](audio_track.md), not per event.

## Invariants

`id` non-empty; `audio_track_id` non-empty; `span.start <= span.end`
(checked on semantic, timebase-correct time); `score` finite and within
`[0,1]`; `(audio_track_id, index)` unique within the track.

## Resolved (your calls)

- **A-loc:** per-track — `audio_track_id → AudioTrack.id`, with
  `AudioTrack.sound_events: Vec<Id>` refs + the `Audio.total_sound_events`
  rollup + `graph::AudioTrack` embedding, mirroring `AudioSegment` (full
  parity — the earlier "high fan-out, derive by reverse-FK" deferral was
  reversed for symmetry with `segments`).
- **`score`:** validated raw `f32` (`[0,1]`), not `Confidence` — keeps the
  aggregate inside the `audio` feature without a `video` dependency.
- **`code`:** `Option<u64>` (the soundevents stable code; `None` =
  unmapped). Matches the wire `CedDetection.tag: fixed64`.
- **Backends:** sqlx (postgres/mysql/sqlite), mongodb, and buffa (media.v1
  flat + media.v2 graph) bridges are all implemented.

## Projection notes

- **sqlx**: `sound_events` table; `id` PK; `audio_track_id` FK →
  `audio_track`; `span` → `start_pts` / `end_pts`; `label` text; `code`
  nullable `BIGINT`; `score` `REAL`; `(audio_track_id, index)` unique. No
  vector column.
- **mongodb**: `sound_events` collection; `_id` = UUIDv7; flat scalars;
  FK + `(audio_track_id, index)` indexes.
- **buffa**: media.v1 flat `SoundEvent` (carries `audio_track_id`) +
  media.v2 graph `SoundEvent` (drops the FK; nested `repeated SoundEvent
  sound_events = 38` on `AudioTrack`).

**Status: drafted (rev 2).** Thin detected sound event:
`id` / `audio_track_id`(→AudioTrack) / `index` / `span`(mediatime) /
`label` / `code` / `score`. `Provenance` lives
on `AudioTrack` (per-track-per-run, not per-event); embeddings → external
vector store. Hooked into `AudioTrack` at full `AudioSegment` parity
(`sound_events` refs + `Audio.total_sound_events` rollup + graph
embedding); sqlx / mongodb / buffa bridges implemented.
