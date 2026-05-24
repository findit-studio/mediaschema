# `VttRegion<Id>` — per-track WebVTT region  *(rev 1)*

## Domain meaning

One `REGION` block from a WebVTT track header. Cues opt in to a region
via [`VttData::region_id`](subtitle_cue_vtt.md). Per-track aggregate
(N regions per track).

## Fields

| field | domain type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | canonical identity |
| `subtitle_track_id` | `Id` | FK → `SubtitleTrack.id` |
| `name` | `SmolStr` | WebVTT region id (`id:foo`) |
| `width` | `f32` | percentage; default `100.0` |
| `lines` | `u32` | number of lines visible; default `3` |
| `region_anchor_x` · `region_anchor_y` | `f32` · `f32` | anchor inside the region, percentages; default `(0, 100)` |
| `viewport_anchor_x` · `viewport_anchor_y` | `f32` · `f32` | anchor inside the viewport, percentages; default `(0, 100)` |
| `scroll_up` | `bool` | `scroll:up`; default `false` |

## Invariants

- Non-nil `id` and `subtitle_track_id` (enforced).
- Geometry-range checks (percentages in `[0, 100]`) — relaxed for now;
  parser-side validation owns this.

## Projection notes

- **sqlx**: `subtitle_track_vtt_region` table; `subtitle_track_id` indexed.
- **mongodb**: `_id` = UUIDv7; `subtitle_track_id` indexed.
- **wire**: a `VttRegion` message.

**Status: rev 1 — implemented in PR #34.**
