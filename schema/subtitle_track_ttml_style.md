# `TtmlStyle<Id>` — per-track TTML `<style>` element  *(rev 1)*

## Domain meaning

One `<style>` element from a TTML track header. Cues opt in via
[`TtmlData::style_id`](subtitle_cue_ttml.md). Per-track aggregate
(N styles per track).

Like [`TtmlRegion`](subtitle_track_ttml_region.md), the styling
vocabulary is stored verbatim in `xml_attrs` for a lossless round
trip.

## Fields

| field | domain type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | canonical identity |
| `subtitle_track_id` | `Id` | FK → `SubtitleTrack.id` |
| `xml_id` | `SmolStr` | the style's `xml:id` attribute (parser key) |
| `xml_attrs` | `SmolStr` | raw XML attribute list (`tts:color=…`, `tts:fontSize=…`); `""` = absent |

## Invariants

- Non-nil `id` and `subtitle_track_id` (enforced).
- `(subtitle_track_id, xml_id)` unique within a track.

## Projection notes

- **sqlx**: `subtitle_track_ttml_style` table; `subtitle_track_id`
  indexed.
- **mongodb**: `_id` = UUIDv7; UNIQUE on `(subtitle_track_id, xml_id)`.
- **wire**: a `TtmlStyle` message.

**Status: rev 1 — implemented in PR #86 (closes #56).**
