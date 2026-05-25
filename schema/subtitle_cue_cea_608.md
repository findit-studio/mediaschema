# `Cea608Data` — CEA-608 line-21 caption per-cue payload  *(rev 1)*

## Domain meaning

The `D` payload for `SubtitleCue<Id, Cea608Data>` (= [`Cea608Cue`]).
CEA-608 is the analogue / line-21 broadcast caption standard — four
caption channels (CC1, CC2, CC3, CC4) ride encoded into video line 21.
`channel` selects which channel the cue came from; `pac_byte_pair`
holds the raw Preamble Address Code byte pair (row / colour /
underline encoded per CEA-608). `styled_text` carries the decoded
line text with any inline style codes preserved.

## Cross-cutting (locked)

CEA-608 has no per-track aggregate (the style codes ride inline in the
PAC byte-pair and the text body). No persistent disk format (broadcast
streams only); the file extension is `""`.

## Fields

| field | domain type | notes |
|---|---|---|
| `channel` | `u8` | CC channel (`1..=4`); validated by `Cea608Data::try_new` |
| `pac_byte_pair` | `u32` | raw PAC byte pair |
| `styled_text` | `SmolStr` | decoded line text with inline CEA-608 style codes |

## Invariants

- `channel` in `1..=4` (rejected at construction —
  `SubtitleCueError::Cea608ChannelOutOfRange`).
- The 6-channel XDS / CEA-708 extensions are out of scope for the
  CEA-608 payload; they are tracked separately if added later.

## Projection notes

- **sqlx**: `subtitle_cue_cea_608` detail table.
- **mongodb**: detail fields embed.
- **wire**: a `Cea608Data` message inside the `SubtitleCue.data` oneof.
  The bridge validates `channel` on read.

**Status: rev 1 — implemented in PR #86 (closes #56).**
