# `SubViewerData` — SubViewer per-cue payload  *(rev 1)*

## Domain meaning

The `D` payload for `SubtitleCue<Id, SubViewerData>` (= [`SubViewerCue`]).
SubViewer carries inline `[br]` / `[b]` / `[i]` / `[u]` / `{y:i}`
colour-code tags in the cue body; `styled_text` preserves them
verbatim. The plain text rides on the base `SubtitleCue.text` field.

## Cross-cutting (locked)

No per-track aggregate. `styled_text` uses `""` = absent.

## Fields

| field | domain type | notes |
|---|---|---|
| `styled_text` | `SmolStr` | raw SubViewer inline tags / codes; `""` = absent |

## Invariants

None beyond the base cue invariants.

## Projection notes

- **sqlx**: `subtitle_cue_sub_viewer` detail table, 1:1 with
  `subtitle_cue` via the shared `id` PK.
- **mongodb**: detail fields embed on the same cue document.
- **wire**: a `SubViewerData` message inside the `SubtitleCue.data` oneof.

**Status: rev 1 — implemented in PR #86 (closes #56).**
