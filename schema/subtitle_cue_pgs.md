# `PgsData` — Blu-ray PGS per-cue payload  *(rev 1)*

## Domain meaning

The `D` payload for `SubtitleCue<Id, PgsData>` (= [`PgsCue`]).
Blu-ray PGS (Presentation Graphics Stream) is a **bitmap** subtitle
format. Unlike VobSub, PGS embeds its palette **per cue** (one set of
YCrCbA quadruples per Presentation Composition Segment), so there is
no per-track palette aggregate. The bitmap is the RLE-encoded pixel
data; `composition_state` carries the PGS state byte (`0x00` = Normal
Case, `0x40` = Acquisition Point, `0x80` = Epoch Start).

The base `SubtitleCue.text` stays `""` until an OCR pipeline writes
plain text into it.

## Cross-cutting (locked)

`bitmap` and `palette_bytes` ride as `bytes::Bytes` (refcounted, cheap
clone). The palette bytes are opaque YCrCbA quadruples; the schema
layer doesn't decode them.

## Fields

| field | domain type | notes |
|---|---|---|
| `bitmap` | `Bytes` | RLE-encoded pixel data |
| `width` | `u32` | rendered width in pixels |
| `height` | `u32` | rendered height in pixels |
| `pos_x` | `i32` | x offset |
| `pos_y` | `i32` | y offset |
| `palette_bytes` | `Bytes` | per-cue YCrCbA quadruples |
| `composition_state` | `u8` | PGS composition state byte (`0x00` / `0x40` / `0x80`) |

## Invariants

None beyond the base cue invariants.

## Projection notes

- **sqlx**: `subtitle_cue_pgs` detail table. Bitmap + palette ride as
  `BYTEA` (postgres) / `MEDIUMBLOB`+`BLOB` (mysql) / `BLOB` (sqlite).
- **mongodb**: detail fields embed; bitmap + palette ride as `Binary`.
- **wire**: a `PgsData` message inside the `SubtitleCue.data` oneof.

**Status: rev 1 — implemented in PR #86 (closes #56).**
