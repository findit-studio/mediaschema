# `VttData<Id>` — WebVTT per-cue payload  *(rev 1)*

## Domain meaning

The `D` payload for `SubtitleCue<Id, VttData<Id>>` (= [`VttCue`]).
Carries WebVTT's per-cue settings (vertical / line / position / size /
align), the optional region link, the voice tag, and the raw inline
markup (`<v.italic>Bob</v>` etc.) preserved verbatim for render fidelity.

## Cross-cutting (locked)

WebVTT cue-setting axes are closed vocabularies (ratified by W3C). Each
rides as a `Option<…>` enum with a stable u8 discriminant; `None` = the
setting wasn't present on the cue line. `line_value` / `position_value`
ride as raw `SmolStr` because the spec allows both percentages (`"50%"`)
and line numbers (`"-2"`). `cue_identifier`, `voice`, `styled_text` use
`""` = absent.

## Fields

| field | domain type | notes |
|---|---|---|
| `cue_identifier` | `SmolStr` | the identifier line preceding `-->`; `""` = absent |
| `vertical` | `Option<VttVertical>` | `vertical:lr` (`Lr`) / `vertical:rl` (`Rl`); `None` = horizontal |
| `line_value` | `SmolStr` | raw `line` value (`"50%"`, `"-2"`); `""` = absent |
| `line_align` | `Option<VttLineAlign>` | `start` / `center` / `end` |
| `position_value` | `SmolStr` | raw `position` value; `""` = absent |
| `position_align` | `Option<VttPositionAlign>` | `start` / `center` / `end` / `line-left` / `line-right` |
| `size_value` | `Option<f32>` | `size` percentage; `None` = absent |
| `text_align` | `Option<VttTextAlign>` | `start` / `center` / `end` / `left` / `right` |
| `region_id` | `Option<Id>` | FK → [`VttRegion`](subtitle_track_vtt_region.md) on the parent track |
| `voice` | `SmolStr` | speaker tag (`<v Bob>…</v>` → `Bob`); `""` = absent |
| `styled_text` | `SmolStr` | raw inline markup (`<i>`, `<b>`, `<c.…>`, …); `""` = absent |

## Invariants

None beyond the base cue invariants — every WebVTT cue setting is
optional and the spec is open about ordering / partial settings.

## Projection notes

- **sqlx**: `subtitle_cue_vtt` detail table, 1:1 with `subtitle_cue` via
  the shared `id` PK. Enum columns ride as `SMALLINT` discriminants;
  `region_id` is a nullable FK to `subtitle_track_vtt_region.id`.
- **mongodb**: detail fields embed on the same document as the base
  fields (no JOIN needed).
- **wire**: a `VttData` message inside the `SubtitleCue.data` oneof.

**Status: rev 1 — implemented in PR #34.**
