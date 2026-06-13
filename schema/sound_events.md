# `SoundEvent<Id>` — a detected sound event  *(rev 1 — drafted)*

## Domain meaning

One **detected, time-ranged sound classification** on an **audio track** —
the audio analog of [`Scene`](scene.md)'s detector field. Where `Scene`
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

No progress lifecycle: a `SoundEvent` exists iff its row exists. There is
**no `AudioTrack.sound_events` rollup** field — sound events are
high-fan-out and attach purely via their `audio_track_id` FK; the database
derives a track's events by reverse-FK (the same way `VideoTrack.scenes`
is not embedded in the graph and a track's children are derived). In the
object-graph shape ([graph.md](graph.md)) `graph::SoundEvent` stands alone
(the consumer builds it per detected event); it is **not** embedded in
`graph::AudioTrack` in this pass.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Media-time = `::mediatime` (`TimeRange`).
Strings = `SmolStr` (`""` = absent, **no `Option`**). **Embeddings → an
external vendor-neutral vector store** keyed by this `id`, if ever embedded
— no embedding field here. **`Provenance` is per-track** (on `AudioTrack`,
one per run) — not per sound event, exactly as for `Scene` /
`AudioSegment`. Conversions (sqlx / mongodb / buffa) deferred (the crate
explicitly supports deferring backends).

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
| `detector` | `CedDetector` (enum) | — | which detector raised this event |

`score` is a validated raw `f32` (finite, `[0,1]`), mirroring this
cluster's `Word.score`. It is **not** the video-cluster `Confidence`
value-object: `Confidence` lives behind the `video` feature, and
`SoundEvent` is an `audio`-feature aggregate that must compile without
`video` — the invariant is identical either way.

**`CedDetector`** (mediaschema-owned enum; the audio analog of
`SceneDetector`): `Ced` (the soundevents CED model) · `Manual`
(user-created / imported event). `#[non_exhaustive]` — the audio pipeline
may add detectors (e.g. a future CLAP-based tagger).

## Invariants

`id` non-empty; `audio_track_id` non-empty; `span.start <= span.end`
(checked on semantic, timebase-correct time); `score` finite and within
`[0,1]`; `(audio_track_id, index)` unique within the track.

## Resolved (your calls)

- **A-loc:** per-track — `audio_track_id → AudioTrack.id`; **no**
  `AudioTrack.sound_events` rollup (high fan-out; derived by reverse-FK,
  same as `VideoTrack.scenes` is not stored).
- **`score`:** validated raw `f32` (`[0,1]`), not `Confidence` — keeps the
  aggregate inside the `audio` feature without a `video` dependency.
- **`code`:** `Option<u64>` (the soundevents stable code; `None` =
  unmapped). Matches the wire `CedDetection.tag: fixed64`.
- **Backends deferred:** no sqlx / mongodb / buffa bridge for `SoundEvent`
  in this pass (the crate defers backends; e.g. media.v2 graph has no
  `Scene` yet either).

## Projection notes (deferred)

- **sqlx**: a future `sound_event` table; `id` PK; `audio_track_id` FK →
  `audio_track`; `span` → `start_pts` / `end_pts`; `label` text; `code`
  nullable `BIGINT`; `score` `REAL`; `detector` enum col (indexed —
  filterable); `(audio_track_id, index)` unique. No vector column.
- **mongodb**: `_id` = UUIDv7; flat scalars.

**Status: drafted (rev 1).** Thin detected sound event:
`id` / `audio_track_id`(→AudioTrack) / `index` / `span`(mediatime) /
`label` / `code` / `score` / `detector`(`CedDetector`). `Provenance` lives
on `AudioTrack` (per-track-per-run, not per-event); embeddings → external
vector store; no rollup field (reverse-FK derived); backend bridges
deferred.
