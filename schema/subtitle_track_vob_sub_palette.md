# `VobSubPalette<Id>` — per-track DVD VobSub palette  *(rev 1)*

## Domain meaning

One DVD VobSub palette — a 16-entry RGB lookup table referenced by
[`VobSubData::palette_id`](subtitle_cue_vob_sub.md). DVD subtitles
encode their colour information by index into this palette; the
per-cue 4-byte `color_indices` and `contrast_indices` arrays each
pick one of the 16 entries.

Per-track aggregate (one palette per VobSub stream, but a track may
carry more than one across DVD chapters — modelled as N rows per
track).

## Fields

| field | domain type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | canonical identity |
| `subtitle_track_id` | `Id` | FK → `SubtitleTrack.id` |
| `entries` | `[u32; 16]` | 16 fixed-position RGB entries; each is `0x00RRGGBB` (alpha conveyed via the cue's contrast indices) |

## Invariants

- Non-nil `id` and `subtitle_track_id` (enforced).
- Exactly 16 entries (the wire / sqlx / mongodb bridges all reject
  rows whose `entries.len() != 16`).

## Projection notes

- **sqlx (postgres)**: `subtitle_track_vob_sub_palette` table with
  `entries` as `BIGINT[]` (each `BIGINT` holds one `0x00RRGGBB` u32).
- **sqlx (mysql / sqlite)**: no native array — entries flatten to 16
  `entry00 … entry15` columns.
- **mongodb**: `_id` = UUIDv7; `entries` rides as a 16-element `Int64`
  array.
- **wire**: a `VobSubPalette` message with `repeated uint32 entries`;
  the bridge validates the length on decode.

**Status: rev 1 — implemented in PR #86 (closes #56).**
