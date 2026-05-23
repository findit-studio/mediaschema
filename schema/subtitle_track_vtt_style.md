# `VttStyleBlock<Id>` — per-track WebVTT STYLE block  *(rev 1)*

## Domain meaning

One `STYLE` block from a WebVTT track header. Body is opaque CSS text;
multiple blocks per track are allowed and rendered in `ordinal` order.

## Fields

| field | domain type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | canonical identity |
| `subtitle_track_id` | `Id` | FK → `SubtitleTrack.id` |
| `ordinal` | `u32` | 0-based ordering across the track's style blocks |
| `css_text` | `SmolStr` | raw CSS body |

## Invariants

- Non-nil `id` and `subtitle_track_id`.
- `(subtitle_track_id, ordinal)` unique within the track.

## Projection notes

- **sqlx**: `subtitle_track_vtt_style` table; `(subtitle_track_id, ordinal)`
  unique index.
- **mongodb**: `_id` = UUIDv7; sort key `ordinal`.
- **wire**: a `VttStyleBlock` message.

**Status: rev 1 — implemented in PR #34.**
