# `SubtitleCue<Id>` — a parsed subtitle cue  *(rev 3 — LOCKED, user-approved)*

## Domain meaning

One parsed cue of a `SubtitleTrack` (`parent → SubtitleTrack.id`) — the heavy
per-track segmented aggregate (parallel to locked `Scene` / proposed
`AudioSegment`); the track keeps only the `cue_count` rollup. Text formats fill
`text`; bitmap formats (PGS/DVBSUB) carry the inline `image` and an `ocr_text`
produced by the OCR stage. `text` and `ocr_text` are **kept distinct** (source
of truth differs — parsed vs OCR). Created now that **ST-cues = referenced
aggregate** is accepted.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Media-time = `mediatime` (extern). No progress
lifecycle (id list + count). **`SmolStr`/`Bytes` use `""`/empty = absent —
never `Option`** (`Option` only for structured/enum/numeric absence). Free-text
= **`LocalizedText`** shared cross-cutting VO ([README.md](README.md)).
**Embeddings → LanceDB** keyed by this `id` — **no embedding field** (same as
`Scene`/`Keyframe`/`AudioSegment`). Parse/OCR **`Provenance` is per-track**
(one value per run) → lives on `SubtitleTrack`, **not** per cue. Conversions
deferred.

## Fields

| field | domain type | wire origin | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | `*.id: bytes` | canonical identity |
| `parent` | `Id` | cue→track | FK → `SubtitleTrack.id` |
| `index` | `u32` | ordinal | 0-based cue order |
| `span` | `mediatime::TimeRange` | start/end | on-screen interval (media-time) |
| `text` | `LocalizedText` | text subs | parsed plain text (styling stripped); `src`+`translated`, each `""`=absent |
| `styled_text` | `SmolStr` | ASS/SSA | original markup retained for render fidelity; `""` = none (no `Option`) |
| `image` | `Bytes` | bitmap subs | **inline** rendered cue bitmap (PGS/DVBSUB); empty = none (mirrors locked `Keyframe.data` — no `Option`, no `Location`) |
| `ocr_text` | `LocalizedText` | OCR stage | text extracted from `image`; kept distinct from `text`; `src`+`translated`, each `""`=absent |

## Invariants

`id` non-empty; `span.start <= span.end`; `(parent, index)` unique; cue is
non-empty (at least one of `text` / `ocr_text` / `image` non-empty);
`ocr_text` non-empty ⇒ `image` non-empty (OCR came from the bitmap).

## Resolved

- **`text`/`ocr_text` kept separate** (your call), both `LocalizedText`.
- **`styled_text` kept** (your call) — ASS/SSA markup; default render =
  `text.src`.
- **`image` inline `Bytes`** (your call: "if it is image then inline bytes").
- **`display_text` derived** for search = (`text` ∨ `ocr_text`) then
  `.translated` ∨ `.src` per consumer locale — not stored.

## Projection notes

- **sqlx**: `subtitle_cue` table; `id` PK; `parent` FK; `text_src`/
  `text_translated`/`ocr_*` columns, derived `display_text` full-text indexed;
  `image` → `BYTEA`/object-store offload keyed by `id`; `(parent,index)`
  unique. No vector column (LanceDB).
- **mongodb**: `_id`=UUIDv7; text index on the derived `display_text`;
  `image` GridFS if large.

**Status: LOCKED (rev 3) — user-approved.** `text`/`ocr_text` separate, both
`LocalizedText`; `styled_text` kept (`SmolStr`); `image` inline `Bytes`;
embedding removed (→LanceDB); no-`Option`/empty=absent throughout;
per-track `Provenance` lives on `SubtitleTrack`. `display_text` = derived.
