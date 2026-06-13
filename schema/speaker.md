# `Speaker<Id>` — a per-track diarized speaker  *(rev 3 — LOCKED, user-approved; FK renamed to `audio_track_id`, `person_id`)*

## Domain meaning

One distinct voice in an `AudioTrack`, as clustered by `dia` speaker
diarization. The track's speaker set; each `AudioSegment` references the
`Speaker` who spoke it. The **voice fingerprint** is now modelled as a
first-class [`VoiceFingerprint`](person.md#voicefingerprint-vo) VO (the
*per-track aggregated centroid* across this speaker's
`AudioSegment.voice_fingerprint`s) — *not* the chromaprint
`AudioTrack.fingerprint` (that's whole-recording acoustic dedup, a different
thing). The actual vector lives in an external **vendor-neutral** vector store
(LanceDB / Qdrant / Milvus / pgvector / …) keyed by
`VoiceFingerprint.vector_id`. Per-track scope is preserved: the same physical
person in another track/file is a *separate* `Speaker` row; linking them is
the role of the new [`Person`](person.md) cross-track identity anchor, via
the optional `Speaker.person` FK.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Media-time = `::mediatime`. Strings = `SmolStr`
(`""`=absent, no `Option`). **Voice embedding vector → external vector store
keyed by `VoiceFingerprint.vector_id` — no inline vector field** (the locked
embeddings rule; the voiceprint is a similarity vector, like
scene/keyframe/segment embeddings). The VO recorded on `Speaker.voiceprint`
is the *linkage + provenance metadata* only. No progress lifecycle.
Conversions deferred.

## Fields

| field | domain type | source | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | — | canonical identity; **LanceDB voiceprint key** |
| `audio_track_id` | `Id` | speaker→track | FK → `AudioTrack.id` (per-track, A-loc) |
| `cluster_id` | `u32` | `dia` | the `dia` cluster label within this track (`DiarizedSpan.speaker_id`); stable within the diarization run |
| `name` | `SmolStr` | user/future | human-assigned identity label; `""` = unassigned (diarization is anonymous — `SPEAKER_00`-style display is derived from `cluster_id`, not stored) |
| `speech_duration` | `Option<mediatime::Timestamp>` | rollup | total time this speaker spoke (Σ of their `AudioSegment.span`s) — list/search facet; maintained rollup, truth = the segments |
| `voiceprint` | `Option<VoiceFingerprint<Id>>` | indexer | per-track aggregated centroid across this speaker's `AudioSegment.voice_fingerprint`s; `None` until aggregation runs. Shared VO with [`Person`](person.md) — see [VoiceFingerprint VO](person.md#voicefingerprint-vo) |
| `person` | `Option<Id>` | identity matcher | FK → [`Person.id`](person.md) — cross-track / cross-modality identity anchor; `None` until the identity-matching step links this speaker (auto-match or user-confirm). **The reverse FK** (`Person → Vec<Speaker>` is derived, not stored on `Person`) |

## Invariants

`id` non-empty; `(audio_track_id, cluster_id)` unique within the track;
`speech_duration` is a denormalized rollup (truth = this speaker's
`AudioSegment`s). A present `voiceprint` is itself constructor-validated
(non-nil `vector_id`, non-zero `dimensions`, finite in-range `confidence`).
`person` is a forward FK to an existing [`Person`](person.md); the reverse
view (`Person → Vec<Speaker>`) is a derived query, not a stored field on
`Person`.

## Relationships

`AudioTrack.speakers: Vec<Id> → Speaker` (the track's speaker set).
`AudioSegment.speaker: Option<Id> → Speaker` (the raw `dia` id is
`Speaker.cluster_id`). `Speaker.person: Option<Id> → Person` — the cross-track
identity anchor (see [person.md](person.md)). Per-track centroid =
`Speaker.voiceprint`; per-segment embedding =
`AudioSegment.voice_fingerprint`; cross-track centroid = `Person.voiceprint`
(same VO at three levels, see [VoiceFingerprint VO](person.md#voicefingerprint-vo)).

## Projection notes

- **sqlx**: `speaker` table; `id` PK; `audio_track_id` FK → `audio_track`;
  `cluster_id`/`name`/`speech_duration` columns. **No vector column** (the
  vector lives in the external store). The optional `VoiceFingerprint` is
  **flattened**: `voiceprint_vector_id` (`uuid`, **discriminator** — `NOT
  NULL` ⇒ the rest of the `voiceprint_*` columns are present),
  `voiceprint_dimensions`, `voiceprint_extracted_at_ms`,
  `voiceprint_confidence`, and the four flattened `voiceprint_provenance_*`
  columns. Nullable `person_id` FK column with `idx_speaker_person_id ON
speaker(person_id)` for FK lookup from a `Person`. `(audio_track_id, cluster_id)`
  unique.
- **mongodb**: `_id`=UUIDv7; single `speakers` collection; `voiceprint`
  embedded as an **optional sub-document** (Mongo indexes can reach into
  sub-docs, so flattening is unnecessary); `person_id` field with single-field
  index `speakers_person_id` for FK lookup from `persons`.
- **wire**: a `Speaker` proto3 message is not yet wire-bridged (per the wire
  audit at PR #44 — `Speaker` sits outside the existing wire boundary, see
  [wire-only.md](wire-only.md)); the `voiceprint` and `person` wire fields
  will be added together when `Speaker` is bridged.

**Status: drafted (rev 2) — pending user review.** *(rev 2: per-track
aggregated `voiceprint: Option<VoiceFingerprint>` first-class on the
aggregate (replaces the earlier "vector lives in LanceDB keyed by
`Speaker.id`, no inline metadata" sketch); added `person: Option<Id> →
Person` cross-track identity FK; `LanceDB` references generalised to a
vendor-neutral external vector store. Domain code landed across PRs #40-44;
this rev catches the doc up.)*

*(rev 1)* Per-track diarized speaker (`dia`); voiceprint → LanceDB keyed by
`Speaker.id` (locked rule; chromaprint stays inline `Bytes` because it's an
exact-dedup hash, not an ANN vector). `AudioTrack.speakers:
Vec<Id>→Speaker`; `AudioSegment.speaker: Option<Id>→Speaker`. Cross-media
identity (`Person` canonical layer) = future, not modelled now.
