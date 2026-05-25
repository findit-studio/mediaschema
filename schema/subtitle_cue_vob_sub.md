# `VobSubData<Id>` — DVD VobSub per-cue payload  *(rev 1)*

## Domain meaning

The `D` payload for `SubtitleCue<Id, VobSubData<Id>>` (= [`VobSubCue`]).
DVD VobSub is a **bitmap** subtitle format — each cue ships its
rendered pixel data as a run-length-encoded blob and references a
per-track palette by FK. `palette_id` resolves to a
[`VobSubPalette`](subtitle_track_vob_sub_palette.md) row that carries
the 16-entry RGB lookup table; the per-cue
`color_indices` / `contrast_indices` (4 entries each) index into it.

Because the cue's plain text isn't directly available, the base
`SubtitleCue.text` stays `""` until an OCR pipeline writes plain text
into it.

## Cross-cutting (locked)

`palette_id` is a **strict FK** (every VobSub cue belongs to one
palette). `bitmap` rides as `bytes::Bytes` (refcounted, cheap clone).
The 4-byte colour/contrast index arrays ride packed into a single u32
on the wire + SQL boundary to keep the row fixed-arity.

## Fields

| field | domain type | notes |
|---|---|---|
| `palette_id` | `Id` | FK → `subtitle_track_vob_sub_palette.id` |
| `bitmap` | `Bytes` | RLE-encoded pixel data |
| `width` | `u32` | rendered width in pixels |
| `height` | `u32` | rendered height in pixels |
| `pos_x` | `i32` | x offset (origin top-left) |
| `pos_y` | `i32` | y offset |
| `color_indices` | `[u8; 4]` | 4-entry palette index array |
| `contrast_indices` | `[u8; 4]` | 4-entry alpha / contrast index array |

## Invariants

None beyond the base cue invariants. The `palette_id` referential
integrity is the application's responsibility (per
[validation-responsibility-boundary] — single-instance intrinsic
invariants only).

## Projection notes

- **sqlx**: `subtitle_cue_vob_sub` detail table; `palette_id` indexed.
  Bitmap rides as `BYTEA` (postgres) / `MEDIUMBLOB` (mysql) / `BLOB`
  (sqlite). Index arrays ride as packed `BIGINT` LE u32.
- **mongodb**: detail fields embed; bitmap rides as `Binary` (generic
  subtype). Index arrays ride as packed `Int64`.
- **wire**: a `VobSubData` message inside the `SubtitleCue.data` oneof.

**Status: rev 1 — implemented in PR #86 (closes #56).**
