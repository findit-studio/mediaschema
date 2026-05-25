# `SubtitleCue<Id, D>` — a polymorphic subtitle cue  *(rev 6 — all 13 formats)*

## Domain meaning

One parsed cue of a `SubtitleTrack` (`subtitle_track_id → SubtitleTrack.id`).
**rev 5** generalises rev 4's flat shape into a polymorphic
`SubtitleCue<Id, D>` whose per-format payload `D` carries the format-specific
detail. Every supported subtitle format gets a stable discriminant
([`SubtitleCueKind`]) reserved on day one, and the format-agnostic base
(`subtitle_cue` table) is the same across all of them. Per-track aggregates
(WebVTT regions / styles, ASS V4+ Style rows, LRC header metadata) live in
sibling tables keyed by `subtitle_track_id`.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7) **and** per-format `D`. Media-time = `mediatime`
(extern); per-stream timebase lives on the parent `SubtitleTrack`. Strings =
`SmolStr` (`""`=absent); free-text = **`LocalizedText`** shared VO.
**Embeddings → LanceDB** keyed by this `id` — no embedding field. Parse/OCR
**`Provenance` is per-track** (on `SubtitleTrack`, not per cue). The rev-3
text-non-empty invariant is **lifted**: polymorphic cues' content lives in
`data` (e.g. `AssData.styled_text`), so `text` empty is legal.

## Base fields (`SubtitleCue<Id, D>`)

| field | domain type | wire origin | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | — | canonical identity (LanceDB key) |
| `subtitle_track_id` | `Id` | cue→track | FK → `SubtitleTrack.id` |
| `ordinal` | `u32` | ordinal | 0-based cue order within the track |
| `span` | `mediatime::TimeRange` | start/end | on-screen interval (media-time) |
| `text` | `LocalizedText` | text subs | plain (style-stripped) text; `""` legal (un-OCR'd bitmap / ASS cue) |
| `data` | `D` | per-format payload | see [`SubtitleCueKind`] table below |

## Format implementation status

The closed `SubtitleCueKind` enum reserves a stable numeric discriminant
for **every** format on day one. **All 13 formats are now implemented**
(rev 6, closes #56).

| discriminant | variant | format | status | detail doc |
|---|---|---|---|---|
| 0 | `Srt` | SubRip | ✓ rev 5 | (no detail table — `SrtData` is a unit) |
| 1 | `Vtt` | WebVTT | ✓ rev 5 | [subtitle_cue_vtt.md](subtitle_cue_vtt.md) |
| 2 | `Ass` | ASS / SSA | ✓ rev 5 | [subtitle_cue_ass.md](subtitle_cue_ass.md) |
| 3 | `MicroDvd` | MicroDVD | ✓ rev 6 | [subtitle_cue_micro_dvd.md](subtitle_cue_micro_dvd.md) |
| 4 | `SubViewer` | SubViewer | ✓ rev 6 | [subtitle_cue_sub_viewer.md](subtitle_cue_sub_viewer.md) |
| 5 | `Sbv` | YouTube SBV | ✓ rev 6 | [subtitle_cue_sbv.md](subtitle_cue_sbv.md) |
| 6 | `Lrc` | LRC / Enhanced LRC | ✓ rev 5 | [subtitle_cue_lrc.md](subtitle_cue_lrc.md) |
| 7 | `Ttml` | TTML | ✓ rev 6 | [subtitle_cue_ttml.md](subtitle_cue_ttml.md) |
| 8 | `Sami` | SAMI | ✓ rev 6 | [subtitle_cue_sami.md](subtitle_cue_sami.md) |
| 9 | `VobSub` | DVD VobSub bitmap | ✓ rev 6 | [subtitle_cue_vob_sub.md](subtitle_cue_vob_sub.md) |
| 10 | `Pgs` | Blu-ray PGS bitmap | ✓ rev 6 | [subtitle_cue_pgs.md](subtitle_cue_pgs.md) |
| 11 | `Cea608` | CEA-608 captions | ✓ rev 6 | [subtitle_cue_cea_608.md](subtitle_cue_cea_608.md) |
| 12 | `EbuStl` | EBU STL teletext | ✓ rev 6 | [subtitle_cue_ebu_stl.md](subtitle_cue_ebu_stl.md) |

Bitmap formats (`VobSub`, `Pgs`) own their inline image bytes on the `D`
type (`bytes::Bytes`); the base `SubtitleCue.text` stays `""` until an
OCR pipeline writes plain text into it (the OCR pipeline integration
is tracked separately in #57).

## Per-track aggregates

A `SubtitleTrack` may carry per-format aggregate rows that cues reference:

| aggregate | format | doc |
|---|---|---|
| `VttRegion<Id>` | WebVTT | [subtitle_track_vtt_region.md](subtitle_track_vtt_region.md) |
| `VttStyleBlock<Id>` | WebVTT | [subtitle_track_vtt_style.md](subtitle_track_vtt_style.md) |
| `AssStyle<Id>` | ASS/SSA | [subtitle_track_ass_style.md](subtitle_track_ass_style.md) |
| `LrcMetadata<Id>` | LRC | [subtitle_track_lrc_metadata.md](subtitle_track_lrc_metadata.md) |
| `TtmlRegion<Id>` | TTML | [subtitle_track_ttml_region.md](subtitle_track_ttml_region.md) |
| `TtmlStyle<Id>` | TTML | [subtitle_track_ttml_style.md](subtitle_track_ttml_style.md) |
| `SamiStyle<Id>` | SAMI | [subtitle_track_sami_style.md](subtitle_track_sami_style.md) |
| `VobSubPalette<Id>` | VobSub | [subtitle_track_vob_sub_palette.md](subtitle_track_vob_sub_palette.md) |

## Invariants

- `id` non-empty; `subtitle_track_id` non-empty; `(subtitle_track_id, ordinal)`
  unique within the track.
- `span.start ≤ span.end` — enforced by construction (`mediatime::TimeRange`).
- `kind` matches the discriminant of the cue's `D` payload type (enforced
  by the row mapper).

## Projection notes

- **sqlx**: `subtitle_cue` base table (`id` PK, `subtitle_track_id` FK,
  `kind` SMALLINT, `ordinal`, `span_*`, `text_*`). Per-format detail tables
  (`subtitle_cue_vtt`, `subtitle_cue_ass`, `subtitle_cue_lrc`) share the
  `id` PK with the base via a 1:1 JOIN. SubRip has no detail table —
  `kind = 0` rows are complete on their own.
- **mongodb**: one document per cue; the per-format detail fields ride
  alongside the base fields on the same document (mongo favours embedded
  shape over JOINs). Aggregates (regions, styles, …) are their own
  collections keyed by `subtitle_track_id`.
- **buffa wire**: the base `SubtitleCue` message carries a `oneof data { …
  }` of per-format payloads; new formats extend the oneof additively. The
  current revision lands the Rust polymorphic surface only; the wire-layer
  bridges follow in a dedicated PR.

## Reserved → implementable later

All 13 day-1 discriminants are now implemented. The closed
`SubtitleCueKind` enum still reserves the discriminator space —
future additions (e.g. CEA-708, WebVTT-NG) would extend the enum
additively without breaking the existing wire / storage contract.

**Status: rev 6 — all 13 formats implemented (closes #56).** Bitmap
OCR pipeline tracked separately in #57.
