# `AssStyle<Id>` — per-track ASS V4+ Style row  *(rev 1)*

## Domain meaning

One row of the `[V4+ Styles]` section of an ASS / SSA script — the full
set of fields a `Dialogue:` line resolves via its Style name. Parsers
build N `AssStyle` rows up front and store the parsed `style_id` on
each [`AssCue`](subtitle_cue_ass.md).

## Fields

Typography group:

| field | domain type | notes |
|---|---|---|
| `name` | `SmolStr` | Style name (the parser's FK key); non-empty |
| `fontname` | `SmolStr` | default `"Arial"` |
| `fontsize` | `f32` | points; default `20.0` |
| `primary_colour` | `u32` | `&HAABBGGRR` (alpha-byte high); default white |
| `secondary_colour` | `u32` | karaoke pre-fill |
| `outline_colour` | `u32` | outline |
| `back_colour` | `u32` | shadow / outline background |

Style flags (booleans — ASS stores `-1`/`0`):

| `bold` `italic` `underline` `strikeout` | `bool` | default `false` |

Geometry / motion:

| field | domain type | notes |
|---|---|---|
| `scale_x` · `scale_y` | `i32` · `i32` | percentages; default `(100, 100)` |
| `spacing` | `i32` | inter-character spacing (px); default `0` |
| `angle` | `f32` | rotation in degrees; default `0.0` |
| `border_style` | `i16` | `1` = outline+drop-shadow (default), `3` = opaque box |
| `outline` · `shadow` | `f32` · `f32` | outline / shadow width (px) |
| `alignment` | `i16` | ASS-numpad alignment (`1`…`9`); default `2` (bottom-centre) |
| `margin_l` · `margin_r` · `margin_v` | `i32` × 3 | default margins (px); default `10` each |
| `encoding` | `i32` | Windows codepage; default `1` |

## Invariants

- Non-nil `id` and `subtitle_track_id`.
- `name` non-empty (parsers key the per-cue `AssData::style_id` on this).
- No range checks on numeric fields — ASS allows wide-open values and
  parsers are the source of truth.

## Projection notes

- **sqlx**: `subtitle_track_ass_style` table; `subtitle_track_id` indexed.
  Colour columns ride as `BIGINT` (the ASS u32 packs alpha as the high
  byte; signed-vs-unsigned conversions stay lossless via i64).
- **mongodb**: `_id` = UUIDv7; colour fields ride as `Int64`.
- **wire**: an `AssStyle` message.

**Status: rev 1 — implemented in PR #34.**
