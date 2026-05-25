# `MicroDvdData` — MicroDVD per-cue payload  *(rev 1)*

## Domain meaning

The `D` payload for `SubtitleCue<Id, MicroDvdData>` (= [`MicroDvdCue`]).
MicroDVD is a frame-based subtitle format that lacks a separate styling
header; per-cue inline `{y:i}` codes (italics), `{c:$00FF00}` (colour)
etc. ride inline in the cue body. `styled_text` preserves them verbatim
so a writer can losslessly round-trip the original file; the plain
text rides on the base `SubtitleCue.text` field.

## Cross-cutting (locked)

No per-track aggregate. `styled_text` uses `""` = absent (matching the
empty-string-is-absent convention used across the schema).

## Fields

| field | domain type | notes |
|---|---|---|
| `styled_text` | `SmolStr` | raw MicroDVD inline codes (`{y:i}…`); `""` = absent |

## Invariants

None beyond the base cue invariants.

## Projection notes

- **sqlx**: `subtitle_cue_micro_dvd` detail table, 1:1 with
  `subtitle_cue` via the shared `id` PK.
- **mongodb**: detail fields embed on the same cue document.
- **wire**: a `MicroDvdData` message inside the `SubtitleCue.data` oneof.

**Status: rev 1 — implemented in PR #86 (closes #56).**
