# `LrcData` — LRC line + Enhanced word-level payload  *(rev 1)*

## Domain meaning

The `D` payload for `SubtitleCue<Id, LrcData>` (= [`LrcCue`]). LRC is a
line-timed lyric format; **Enhanced LRC** extends it with word-level
timing on the same line. `LrcData` is a tiny tag (`has_word_timing`)
discriminating which form a row carries; the word rows themselves live
in a child aggregate ([`LrcWord`]) keyed by `subtitle_cue_id`.

## Cross-cutting (locked)

Line-level lyrics fill `SubtitleCue::text` (`LocalizedText`) on the
base; word-level cues set `has_word_timing = true` and append child
[`LrcWord`] rows. Per-track header metadata (`[ti:]`, `[ar:]`, …) lives
on a separate [`LrcMetadata`](subtitle_track_lrc_metadata.md) row.

## Fields — `LrcData`

| field | domain type | notes |
|---|---|---|
| `has_word_timing` | `bool` | `true` ⇒ child `LrcWord` rows exist; `false` ⇒ line-level only |

## Fields — `LrcWord<Id>`

| field | domain type | notes |
|---|---|---|
| `subtitle_cue_id` | `Id` | FK → `SubtitleCue.id` |
| `ordinal` | `u32` | 0-based word order within the cue |
| `text` | `SmolStr` | the word text |
| `start_pts` | `i64` | media-time PTS tick in the parent track's timebase |

The per-word end time is implicit: the next word's `start_pts`, or the
cue's `span.end` for the last word.

## Invariants

- `LrcCue` base: standard non-nil id / parent invariants.
- `LrcWord`: non-nil `subtitle_cue_id`; `(subtitle_cue_id, ordinal)`
  unique within the cue (enforced by the PK).
- `has_word_timing = false` is consistent with **zero** child rows, but
  the cross-row referential check is the application's responsibility
  (per [[validation-responsibility-boundary]]).

## Projection notes

- **sqlx**: `subtitle_cue_lrc` detail table (1:1 with `subtitle_cue` via
  shared `id` PK) + `subtitle_cue_lrc_word` child table
  (`(subtitle_cue_id, ordinal)` PK).
- **mongodb**: the `has_word_timing` bool embeds on the cue document;
  the word rows are a child collection keyed by `subtitle_cue_id`.
- **wire**: a `LrcData` message inside the `SubtitleCue.data` oneof,
  plus a sibling `LrcWord` message.

**Status: rev 1 — implemented in PR #34.**
