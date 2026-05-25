# `SamiData` — SAMI per-cue payload  *(rev 1)*

## Domain meaning

The `D` payload for `SubtitleCue<Id, SamiData>` (= [`SamiCue`]). SAMI
(Synchronized Accessible Media Interchange, Microsoft) is an
HTML-like captioning format with a `<SYNC Start=…>` per cue + an inner
`<P Class="…">…</P>` body. `class_name` selects which per-track
[`SamiStyle`](subtitle_track_sami_style.md) applies (e.g. `ENCC` for
English captions); `styled_text` carries the inline HTML-like body.
Plain text rides on the base `SubtitleCue.text`.

## Cross-cutting (locked)

`class_name` is a **string** key into the per-track `SamiStyle`
aggregate (SAMI doesn't have a numeric id concept); referential
integrity is an application concern (per
[validation-responsibility-boundary]).

## Fields

| field | domain type | notes |
|---|---|---|
| `class_name` | `SmolStr` | SAMI selector key (e.g. `ENCC`); `""` = absent |
| `styled_text` | `SmolStr` | inline HTML-like body; `""` = absent |

## Invariants

None beyond the base cue invariants.

## Projection notes

- **sqlx**: `subtitle_cue_sami` detail table.
- **mongodb**: detail fields embed on the same cue document.
- **wire**: a `SamiData` message inside the `SubtitleCue.data` oneof.

**Status: rev 1 — implemented in PR #86 (closes #56).**
