# `AssData<Id>` — ASS / SSA per-cue payload  *(rev 1)*

## Domain meaning

The `D` payload for `SubtitleCue<Id, AssData<Id>>` (= [`AssCue`]).
Mirrors one `Dialogue:` line of an `[Events]` section in an ASS / SSA
script: render layer, the Style FK, the actor/name field, the per-cue
margin overrides, an optional `Effect` (e.g. `Karaoke`), and the raw
inline override markup (`{\b1}…{\b0}` etc.).

## Cross-cutting (locked)

`style_id` is a **strict FK** to the parent track's
[`AssStyle`](subtitle_track_ass_style.md) aggregate; parsers resolve the
`Style` name to an id at parse time. `name` / `effect` / `styled_text`
use `""` = absent. Margin override values are `i32` with the ASS
sentinel `0` meaning "no override" (the renderer falls back to the Style
row's margins).

## Fields

| field | domain type | notes |
|---|---|---|
| `layer` | `i32` | render layer (higher = on top); ASS default `0` |
| `style_id` | `Id` | FK → [`AssStyle.id`](subtitle_track_ass_style.md) |
| `name` | `SmolStr` | ASS "Name" field (actor / annotation); `""` = absent |
| `margin_l` | `i32` | per-cue left margin override; `0` = no override |
| `margin_r` | `i32` | per-cue right margin override |
| `margin_v` | `i32` | per-cue vertical margin override |
| `effect` | `SmolStr` | `Effect` field (`Karaoke`, `Scroll up;…`); `""` = absent |
| `styled_text` | `SmolStr` | raw ASS markup (`{\b1}hi{\b0}`); `""` = absent |

## Invariants

None beyond the base cue invariants. The `style_id` referential integrity
is the application's responsibility (per
[[validation-responsibility-boundary]] — single-instance intrinsic
invariants only).

## Projection notes

- **sqlx**: `subtitle_cue_ass` detail table, 1:1 with `subtitle_cue` via
  the shared `id` PK; `style_id` indexed.
- **mongodb**: detail fields embed on the same cue document.
- **wire**: an `AssData` message inside the `SubtitleCue.data` oneof.

**Status: rev 1 — implemented in PR #34.**
