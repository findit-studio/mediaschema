# `EbuStlData` — EBU STL teletext per-cue payload  *(rev 1)*

## Domain meaning

The `D` payload for `SubtitleCue<Id, EbuStlData>` (= [`EbuStlCue`]).
EBU STL (Tech 3264) is the European broadcast subtitle interchange
format. Each cue mirrors a TTI (Text and Timing Information) block:
`subtitle_number` is the SN field, `cumulative` is the
Cumulative-Subtitle flag (multi-row stacked subtitles),
`vertical_pos` is the VP field (line on screen), `justification` is
the JC field (`1` = left, `2` = centre, `3` = right). `styled_text`
carries the decoded line with any inline STL control codes preserved.

## Cross-cutting (locked)

EBU STL has no per-track aggregate; styling rides per-cue. The on-disk
file extension is `stl`.

## Fields

| field | domain type | notes |
|---|---|---|
| `subtitle_number` | `u32` | SN field (sequence within the file) |
| `cumulative` | `bool` | Cumulative-Subtitle flag |
| `vertical_pos` | `i32` | VP field (line on screen) |
| `justification` | `u8` | JC field (`1` = left, `2` = centre, `3` = right); validated |
| `styled_text` | `SmolStr` | decoded line text with inline STL control codes |

## Invariants

- `justification` in `1..=3` (rejected at construction —
  `SubtitleCueError::EbuStlJustificationOutOfRange`).

## Projection notes

- **sqlx**: `subtitle_cue_ebu_stl` detail table.
- **mongodb**: detail fields embed.
- **wire**: an `EbuStlData` message inside the `SubtitleCue.data` oneof.
  The bridge validates `justification` on read.

**Status: rev 1 — implemented in PR #86 (closes #56).**
