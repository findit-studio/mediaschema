# `TtmlRegion<Id>` — per-track TTML `<region>` element  *(rev 1)*

## Domain meaning

One `<region>` element from a TTML track header. Cues opt in to a
region via [`TtmlData::region_id`](subtitle_cue_ttml.md). Per-track
aggregate (N regions per track).

The TTML styling vocabulary is large enough that pinning each
attribute as a typed column would invent a parser; instead we store
the raw XML attribute fragment verbatim in `xml_attrs` for a lossless
round trip.

## Fields

| field | domain type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | canonical identity |
| `subtitle_track_id` | `Id` | FK → `SubtitleTrack.id` |
| `xml_id` | `SmolStr` | the region's `xml:id` attribute (parser key) |
| `xml_attrs` | `SmolStr` | raw XML attribute list (`tts:origin=…`, `tts:extent=…`); `""` = absent |

## Invariants

- Non-nil `id` and `subtitle_track_id` (enforced).
- `(subtitle_track_id, xml_id)` unique within a track (enforced by the
  unique index on the mongodb / sqlx projections).

## Projection notes

- **sqlx**: `subtitle_track_ttml_region` table; `subtitle_track_id`
  indexed.
- **mongodb**: `_id` = UUIDv7; UNIQUE on `(subtitle_track_id, xml_id)`.
- **wire**: a `TtmlRegion` message.

**Status: rev 1 — implemented in PR #86 (closes #56).**
