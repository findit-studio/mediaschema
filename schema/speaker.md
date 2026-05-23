# `Speaker<Id>` — a per-track diarized speaker  *(rev 1 — LOCKED, user-approved)*

## Domain meaning

One distinct voice in an `AudioTrack`, as clustered by `dia` speaker
diarization. The track's speaker set; each `AudioSegment` references the
`Speaker` who spoke it. The **voice fingerprint** = the `dia` 256-d voice
embedding, stored in **LanceDB keyed by `Speaker.id`** (cross-media voice
similarity / identity resolution) — *not* the chromaprint
`AudioTrack.fingerprint` (that's whole-recording acoustic dedup, a different
thing). Per-track scope: the same physical person in another track/file is a
*separate* `Speaker` row; linking them is a future LanceDB voice-similarity
concern, not a stored FK.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Media-time = `::mediatime`. Strings = `SmolStr`
(`""`=absent, no `Option`). **Voice embedding → LanceDB keyed by this `id` —
no inline vector field** (the locked embeddings rule; the voiceprint is a
similarity vector, like scene/keyframe/segment embeddings). No progress
lifecycle. Conversions deferred.

## Fields

| field | domain type | source | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | — | canonical identity; **LanceDB voiceprint key** |
| `parent` | `Id` | speaker→track | FK → `AudioTrack.id` (per-track, A-loc) |
| `cluster_id` | `u32` | `dia` | the `dia` cluster label within this track (`DiarizedSpan.speaker_id`); stable within the diarization run |
| `name` | `SmolStr` | user/future | human-assigned identity label; `""` = unassigned (diarization is anonymous — `SPEAKER_00`-style display is derived from `cluster_id`, not stored) |
| `speech_duration` | `Option<mediatime::TrackTime>` | rollup | total time this speaker spoke (Σ of their `AudioSegment.span`s) — list/search facet; maintained rollup, truth = the segments |

## Invariants

`id` non-empty; `(parent, cluster_id)` unique within the track;
`speech_duration` is a denormalized rollup (truth = this speaker's
`AudioSegment`s). Voiceprint presence is a LanceDB concern, not a domain
invariant.

## Relationships

`AudioTrack.speakers: Vec<Id> → Speaker` (the track's speaker set).
`AudioSegment.speaker: Option<Id> → Speaker` (was `Option<SpeakerId(u32)>`;
the raw `dia` id is now `Speaker.cluster_id`). Voiceprint =
`LanceDB(Speaker.id)`; cross-media identity = LanceDB voice-embedding
similarity (future — a canonical `Person`/identity layer is a later
enhancement, not modelled now).

## Open questions

- **SP-identity (future, flagged not modelled):** a cross-media canonical
  `Person` layer linking `Speaker`s across files by voiceprint similarity +
  human naming. *Lean: future — for now `name` is a per-track free label;
  cross-media resolution is a LanceDB-similarity query, not a stored FK.*

## Projection notes

- **sqlx**: `speaker` table; `id` PK; `parent` FK → `audio_track`;
  `cluster_id`/`name`/`speech_duration` columns. **No vector column** (voiceprint
  in LanceDB). `(parent, cluster_id)` unique.
- **mongodb**: `_id`=UUIDv7; single collection; voiceprint not embedded.

**Status: LOCKED (rev 1) — user-approved.** Per-track diarized speaker
(`dia`); voiceprint → LanceDB keyed by `Speaker.id` (locked rule; chromaprint
stays inline `Bytes` because it's an exact-dedup hash, not an ANN vector).
`AudioTrack.speakers: Vec<Id>→Speaker`; `AudioSegment.speaker: Option<Id>→
Speaker`. Cross-media identity (`Person` canonical layer) = future, not
modelled now.
