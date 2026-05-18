# `SubtitleCue<Id>` — a parsed subtitle cue  *(rev 1 — drafted for review, NOT self-locked)*

## Domain meaning

One parsed cue of a `SubtitleTrack` (`parent → SubtitleTrack.id`) — the heavy
per-track segmented aggregate (parallel to `Scene` / `AudioSegment`); the track
keeps only the `cue_count` rollup. Unifies text and image-based subtitles: text
formats fill `text`; bitmap formats (PGS/DVBSUB) carry the image region and an
`ocr_text` produced by the OCR stage. Created only if **ST-cues = referenced
aggregate** is approved (the recommended option).

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Media-time = `mediatime` (extern). No progress
lifecycle (id list + count). Strings = `SmolStr`. Conversions deferred.

## Fields

| field | domain type | wire origin | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | `*.id: bytes` | canonical identity |
| `parent` | `Id` | cue→track | FK → `SubtitleTrack.id` |
| `index` | `u32` | ordinal | 0-based cue order |
| `span` | `mediatime::TimeRange` | start/end | on-screen interval (media-time) |
| `text` | `Option<SmolStr>` | text subs | plain text (styling stripped); `None` for bitmap |
| `styled_text` | `Option<SmolStr>` | ASS/SSA | original markup retained for fidelity (open) |
| `image_ref` | `Option<Location>` | bitmap subs | rendered cue bitmap (PGS/DVBSUB) |
| `ocr_text` | `Option<SmolStr>` | OCR stage | text extracted from `image_ref` |
| `embedding` | `Option<Embedding>` | vector | optional semantic-search vector (see `scene.md` SC-embed) |

## Invariants

`id` non-empty; `span.start <= span.end`; `(parent, index)` unique; at least one
of `text` / `ocr_text` / `image_ref` present; `ocr_text` ⇒ `image_ref` present.

## Open questions

- **Searchable text:** expose one derived `display_text` (= `text` ∨ `ocr_text`)
  for full-text/graphql, keeping `text`/`ocr_text` distinct as source of truth.
  *Lean: yes (derived, not stored).*
- **`styled_text`:** retain ASS/SSA markup (render fidelity) vs plain-only.
  *Lean: keep optional; default render uses `text`.*
- **Embedding** for semantic subtitle search — adopt now vs later. *Lean: later
  (YAGNI) unless subtitle semantic search is in scope.*

## Projection notes

- **sqlx**: `subtitle_cue` table; `id` PK; `parent` FK; `display_text`
  full-text indexed; `image_ref` → object-store path; `(parent,index)` unique.
- **mongodb**: `_id`=UUIDv7; text index on `display_text`.
- **graphql**: `span` + `display_text` exposed (player/search); raw markup &
  embedding not exposed.

**Status: in review (rev 1) — conditional on ST-cues=aggregate. NOT self-locked.**
