# `TtmlData<Id>` — TTML per-cue payload  *(rev 1)*

## Domain meaning

The `D` payload for `SubtitleCue<Id, TtmlData<Id>>` (= [`TtmlCue`]).
TTML (Timed Text Markup Language, W3C) carries cue content as an XML
fragment with `<span>` runs, `<br/>`, etc. Per-track `<region>` and
`<style>` elements live in sibling aggregates; cues reference them via
optional FKs. `xml_id` is the cue's `xml:id` attribute (the parser's
stable cue handle).

## Cross-cutting (locked)

`region_id` / `style_id` are **nullable FKs** — TTML cues may carry
neither, either, or both. `xml_id` and `styled_text` use `""` =
absent.

## Fields

| field | domain type | notes |
|---|---|---|
| `region_id` | `Option<Id>` | FK → [`TtmlRegion`](subtitle_track_ttml_region.md) |
| `style_id` | `Option<Id>` | FK → [`TtmlStyle`](subtitle_track_ttml_style.md) |
| `xml_id` | `SmolStr` | the cue's `xml:id` attribute; `""` = absent |
| `styled_text` | `SmolStr` | inline XML fragment; `""` = absent |

## Invariants

None beyond the base cue invariants.

## Projection notes

- **sqlx**: `subtitle_cue_ttml` detail table; `region_id` / `style_id`
  indexed.
- **mongodb**: detail fields embed on the same cue document.
- **wire**: a `TtmlData` message inside the `SubtitleCue.data` oneof.

**Status: rev 1 — implemented in PR #86 (closes #56).**
