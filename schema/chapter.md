# `Chapter<Id>` — container-level chapter / cue point  *(rev 1 — LOCKED, user-approved)*

## Domain meaning

A container-level chapter marker — `AVFormatContext.chapters[i]` in FFmpeg.
Chapters live next to streams in the container header (MP4 `chpl`, MKV
`ChapterAtom`, Matroska `EditionEntry`, M4B chapter-tracks, …), **not** on
any particular stream. They apply equally to video, audio-only podcasts,
audiobooks, and screencasts.

One `Chapter` row per container chapter. `nb_chapters` on the parent
[`Media`](media.md) is the verbatim probe count (kept symmetric with
`nb_streams`); `chapters: Vec<Id>` on `Media` is the reverse lookup to
these rows.

## Cross-cutting (locked rules)

Generic over `Id` (single **UUIDv7** key). FK column = `media_id` (per the
FK naming convention; never `parent`, never a bare type name). Media-time
intervals = `::mediatime` (`TimeRange`, which carries its own `Timebase`).
Container metadata (`AVDictionary`) is per-chapter — `title` (the only
queryable key) is hoisted into its own column with case-insensitive
lookup; **every other entry lives in a separate
[`chapter_metadata`](chapter.md#child-metadata-table) join table** so the
chapter row stays scalar.

## Fields (flat — the chapter level)

| field | domain type | wire origin | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | `Chapter.id: bytes` | mediaschema-native key |
| `media_id` | `Id` | `Chapter.media_id: bytes` | FK → [`Media`](media.md); per FK convention |
| `index` | `u32` | `Chapter.index` | ordinal within the media, `0..media.nb_chapters` |
| `source_id` | `i64` | `Chapter.source_id` | `AVChapter.id` verbatim (container's own id — containers reuse) |
| `time_range` | `mediatime::TimeRange` | `Chapter.time_range` | start..end on the chapter's own timebase (mediatime carries the `Timebase`) |
| `title` | `SmolStr` | `Chapter.title` | the conventional `metadata["title"]` value, hoisted; **empty = absent** (never `Option<SmolStr>`) |
| `metadata` | `IndexMap<SmolStr, SmolStr>` | `Chapter.metadata` (`repeated KeyValue`) | every AVDictionary entry **except** the `title` key (any case); insertion-ordered |

**Casing rules:**

- `title` is populated from `metadata["title" | "TITLE" | "Title" | ...]` —
  the **first** case-insensitive match wins; the value is stored verbatim.
- The matching entry is **removed** from `metadata` before persistence
  (lossless against semantic metadata; round-tripping to AVChapter writes
  `{"title": title, ...metadata}` and reconstructs the AVDictionary).
- SQL lookup is case-insensitive via `LOWER(title)` indexes (postgres /
  mysql / sqlite all support functional / expression indexes; postgres
  can alternatively use `CITEXT` — that decision is per-deployment, not
  schema-locked).

## Intrinsic invariants

- `title.len() <= 4 KiB` — defensive cap matching mediaschema's other
  string-field guards.
- `index <= u32::MAX` — natural type.
- `metadata` may carry up to a defensive cap of `MAX_CHAPTER_METADATA_ENTRIES`
  (documented constant; one extreme-container backstop, not a wire field).
- The collection-composition invariant *`chapters[i].index == i` after sort
  by `index`* is **application-layer**, not validated here (per the
  validation responsibility boundary).

## Error model

Chapters are probed alongside streams; failures surface on the parent
`Media.probe_error`, not per-chapter.

## Projection notes

- **sqlx**: flat `chapter` table; `id` PK (uuid); `(media_id, index)`
  unique; `media_id` indexed; `LOWER(title)` expression index for
  case-insensitive lookup. Companion `chapter_metadata`
  `(chapter_id, ordinal, key, value)` table preserves `IndexMap`
  insertion order via `ORDER BY ordinal`; FK `chapter_id` cascades on
  delete.
- **mongodb**: `_id`=UUIDv7; `time_range` embedded; `metadata` as a
  nested BSON document (BSON documents are insertion-ordered, so
  `IndexMap` round-trips natively). Index on `media_id`,
  `(media_id, index)` unique, and `title` (collation strength 2 — diacritic
  + case insensitive) for the lookup case-insensitivity.
- **buffa wire**: `Chapter` message with `repeated KeyValue metadata`
  (proto3 has no map ordering; we use a repeated message to preserve
  order on the wire — see also how subtitle-track style maps are
  represented).

## Child metadata table

```
chapter_metadata
  chapter_id : Id   FK → chapter.id  (cascade)
  ordinal    : u32  insertion order (0..)
  key        : SmolStr
  value      : SmolStr
  PRIMARY KEY (chapter_id, ordinal)
```

Reconstruction:

```sql
SELECT key, value
FROM chapter_metadata
WHERE chapter_id = ?
ORDER BY ordinal
```

This yields the original `IndexMap` insertion order. The pair
`(chapter_id, key)` is **not** unique — AVDictionary allows duplicate
keys; `IndexMap` does not, so the bridge enforces the dedup at decode
time (last-write-wins, ordered by ordinal). The schema permits duplicates
to faithfully round-trip a container that has them; the domain type
collapses them.

## Status

LOCKED (rev 1) — initial introduction. Added in mediaschema 0.2.0 alongside
`Media.nb_streams` / `Media.nb_chapters` / `Media.chapters` (parent rev 11).
