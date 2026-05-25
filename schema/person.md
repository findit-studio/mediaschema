# `Person<Id>` — cross-track / cross-modality identity anchor  *(rev 1 — drafted, in-review)*

## Domain meaning

A `Person` is the canonical "this is the same human" anchor that sits **above**
the per-track [`Speaker`](speaker.md) aggregate. One `Person` ↔ many
`Speaker`s: the *same physical person* appearing in several `AudioTrack`s
collapses to **one** `Person` row, with each per-track diarized voice staying
as its own `Speaker` (the per-track scope of `dia` clustering is preserved —
see [speaker.md](speaker.md)). The reverse FK
(`Speaker.person: Option<Id>`) lives on `Speaker`, not on `Person`;
`Person → Vec<Speaker>` is **derived** by a back-reference query, never
stored as a forward list (stored child-id lists desync; the FK on the child is
the source of truth).

`Person` is designed **modality-neutral** on purpose: the voiceprint is the
*current* identity signal because the audio pipeline lands first, but a future
`FaceDetection.person: Option<Id>` (or any other cross-modality
back-reference) is meant to hang off the *same* `Person` aggregate without
restructuring it. The voiceprint field is one optional identity signal, not
the only one — adding face/visual identity later is an additive change on the
referring side, not a redesign of `Person`.

The voiceprint on `Person` is a **cross-track aggregated centroid** — the
centroid across all linked `Speaker`s' per-track voiceprints. It is **not**
the per-segment voice embedding (that's
[`AudioSegment.voice_fingerprint`](audio_segments.md)) and **not** the
per-track aggregated voiceprint (that's [`Speaker.voiceprint`](speaker.md));
those three levels are intentional, with the same [`VoiceFingerprint`](#voicefingerprint-vo)
VO reused at each level. Person-level aggregation is meaningful only when the
contributing samples share one `(model, version)` pair — see the VO's
`provenance`.

## Cross-cutting (locked)

Generic over `Id` (single **UUIDv7** key — Postgres `uuid` / Mongo `_id`; FKs
are the UUID). Wall-clock = `jiff::Timestamp` (ms). Strings = `SmolStr`
(`""` = absent — no `Option` for strings). **Voiceprint vectors live in an
external vector store** keyed by `VoiceFingerprint.vector_id`: vendor-neutral
— the application picks **LanceDB / Qdrant / Milvus / pgvector / …** —
mediaschema does not interpret `vector_id`. The fingerprint model is
**decoupled from `dia`** segmentation/clustering: upgrading `dia`'s clusterer
does not invalidate fingerprints, because the embedding model recorded in
`VoiceFingerprint.provenance` is a separate, stable choice. Conversions
deferred.

## Fields

| field | domain type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | canonical identity; `Speaker.person` FKs this id |
| `name` | `SmolStr` | human name; `""` = unnamed (the locked string-empty-is-absent convention) |
| `voiceprint` | `Option<VoiceFingerprint<Id>>` | aggregated canonical voiceprint — the centroid across all linked `Speaker`s' per-track voiceprints. `None` until aggregation runs. Meaningful only when contributing samples share one `(model, version)` — see [VoiceFingerprint VO](#voicefingerprint-vo) |
| `confidence` | `PersonConfidence` | whether the identity was user-confirmed or auto-matched (default `AutoMatched`) |
| `created_at` | `jiff::Timestamp` | record creation time (Unix ms) |
| `updated_at` | `jiff::Timestamp` | record last-updated time (Unix ms) |

`Speaker → Person` is the **reverse** FK (lives on `Speaker.person`); the
forward `Person → Vec<Speaker>` view is a derived query, not a stored field.

### `PersonConfidence`

| variant | meaning |
|---|---|
| `AutoMatched` *(default)* | auto-clustered by voiceprint similarity, not yet user-reviewed |
| `UserConfirmed` | user-confirmed, manually created, or manually edited |

A freshly auto-matched identity starts at `AutoMatched`; the user-curation
layer ([smart_folder.md](smart_folder.md) territory) may promote it to
`UserConfirmed` once reviewed.

## VoiceFingerprint VO

Shared cross-cutting VO — **reused** at three levels:

- [`AudioSegment.voice_fingerprint`](audio_segments.md) — the *per-segment*
  embedding extracted from one speak range (the raw extraction);
- [`Speaker.voiceprint`](speaker.md) — the *per-track* aggregated centroid
  across that speaker's segments;
- `Person.voiceprint` — the *cross-track* aggregated centroid across all
  linked `Speaker`s.

| field | domain type | notes |
|---|---|---|
| `vector_id` | `Id` (UUIDv7) | opaque key into the external vector store; non-nil. mediaschema does not interpret it — the application keeps it consistent with whichever backend (LanceDB / Qdrant / Milvus / pgvector / …) is in use |
| `dimensions` | `u32` | length of the embedding vector that lives in the external store; validated `> 0` |
| `extracted_at` | `jiff::Timestamp` | wall-clock time the embedding was extracted (when the model produced the vector) |
| `confidence` | `Option<f32>` | model-reported confidence in `[0.0, 1.0]`; finite-validated; `None` when the model does not expose one (vs. a real `0.0`) |
| `provenance` | `Provenance` | shared cross-cutting VO — `{ model_name, model_version, prompt_version, indexer_version }` (all `SmolStr`, `""`=absent). The embedding-model pin |

**Derives:** `PartialEq` but **not `Eq` / `Hash`** — `Option<f32>` confidence
carries a float (NaN ≠ NaN), so the locked rule from `rust-type-conventions`
§1 ("a floating-point field precludes `Eq` / `Hash`") applies. `Person` itself
also drops `Eq` / `Hash` for the same reason (it embeds `VoiceFingerprint`);
aggregates are keyed by `id` anyway, so callers should hash / equate by id.

**No `Default`.** A "default" fingerprint would carry a nil `vector_id`
(orphan linkage) and zero `dimensions` (nonsensical shape); construct via the
validating `try_new` (on the canonical `Uuid7` specialization) or
`from_parts` (storage / wire reconstruction).

## Invariants

`id` non-nil (rejected by `try_new`); a present `voiceprint` is itself
constructor-validated (non-nil `vector_id`, non-zero `dimensions`, finite
in-range `confidence`); `created_at` / `updated_at` carried faithfully by the
domain (Unix-epoch sentinel collapsing is a wire-decode concern, not a domain
invariant). **No `Default`** — defaulting to `{ id: nil, … }` would be an
identity-less anchor, the same reasoning as `Media`'s "No Default".

## Identity-matching flow

The identity layer is **application-orchestrated**, not a domain
auto-derivation:

1. **Per-segment extraction** — the indexer's voice-embedding worker, sitting
   *downstream* of `dia`, extracts one `VoiceFingerprint` per
   `AudioSegment.span` from the chosen embedding model, writes the vector to
   the external store under a fresh `vector_id`, and persists the VO on
   `AudioSegment.voice_fingerprint`.
2. **Per-`Speaker` aggregation** — once `dia` clustering finishes for the
   track, the worker averages the per-segment vectors per cluster, writes the
   centroid under a new `vector_id`, and persists it on `Speaker.voiceprint`.
3. **Person matching** — a sequential post-indexing step performs a KNN query
   against the vector store, **filtered by `(provenance.model_name,
   provenance.model_version)`** (cross-model centroids are not comparable),
   and decides one of:
   - **auto-link** — high similarity ⇒ set `Speaker.person` to the existing
     `Person.id`, then recompute `Person.voiceprint` as the new centroid
     across the (now larger) linked-`Speaker` set;
   - **new `Person`** — no neighbour above threshold ⇒ insert a fresh
     `Person` (`confidence = AutoMatched`) and link;
   - **queue-for-review** — borderline similarity ⇒ leave `Speaker.person =
     None` and surface the candidate to the user-curation layer for manual
     confirmation (which then writes a link with
     `confidence = UserConfirmed`).
4. **Cross-modality (future)** — a face-detection / face-recognition step
   would link the same `Person.id` from video-side `FaceDetection`
   aggregates. `Person` is modality-neutral: adding that linkage is an
   additive change on the referring side, no re-shape here.

Threshold tuning, re-extraction triggered by a model upgrade, and merge/split
of mis-clustered `Person`s are all **application-layer** concerns; the domain
exposes the FK + VO surface, not the policy.

## Projection notes

- **sqlx**: `person` table; `id` PK (`uuid`); `name` `text`; `confidence`
  `smallint` (`0`=`AutoMatched`, `1`=`UserConfirmed`);
  `created_at_ms`/`updated_at_ms` `bigint`. The optional
  `VoiceFingerprint` is **flattened** into the row:
  `voiceprint_vector_id` (`uuid`, **discriminator** — `NOT NULL` ⇒ the rest of
  the `voiceprint_*` columns are present), `voiceprint_dimensions`,
  `voiceprint_extracted_at_ms`, `voiceprint_confidence`, and the four flattened
  `voiceprint_provenance_*` columns. Compound index
  `idx_person_voiceprint_model ON person(voiceprint_provenance_model_name,
  voiceprint_provenance_model_version)` supports "find Persons by embedding
  model" during re-extraction. `Speaker` adds matching `voiceprint_*` columns
  plus a nullable `person` FK column with `idx_speaker_person ON
  speaker(person)` for the FK lookup. `AudioSegment` adds matching
  `voice_fingerprint_*` columns.
- **mongodb**: `persons` collection; `_id` = UUIDv7 (`BinData`); `voiceprint`
  embedded as an **optional sub-document** (not flattened — Mongo indexes can
  reach into sub-documents, so flattening is unnecessary). Compound index
  `persons_voiceprint_model` on
  `(voiceprint.provenance.model_name,
  voiceprint.provenance.model_version)` matches the sqlx re-extraction query.
  `speakers` adds an embedded `voiceprint` sub-doc plus a `person` field with
  single-field index `speakers_person` for FK lookup. `audio_segments` adds
  an embedded `voice_fingerprint` sub-doc.
- **wire**: proto3 `Person` / `VoiceFingerprint` messages + `PersonConfidence`
  enum landed in PR #44, plus the shared `Provenance` message. The
  per-`Speaker` / per-`AudioSegment` fields are **additive** on the wire side
  too (`optional VoiceFingerprint`); the `Speaker` and `AudioSegment` wire
  bridges follow the existing wire-only boundary
  ([wire-only.md](wire-only.md)). `vector_id` rides as `bytes` (the 16-byte
  UUIDv7); `extracted_at` / `created_at` / `updated_at` ride as `int64` ms.
- **graphql**: `Person` exposed with `name` / `confidence` /
  `voiceprint?.{dimensions, extracted_at, confidence, provenance}` (never a
  raw vector — `vector_id` stays opaque, and the *vector* is a vector-store
  endpoint, not a field); the inverse `speakers: [Speaker!]!` resolver is a
  back-reference query on `Speaker.person`.

**Status: drafted (rev 1) — pending user review.**
