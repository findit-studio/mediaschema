# `SubtitleCue<Id, D>` — a polymorphic subtitle cue  *(rev 5 — polymorphic foundation)*

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
for **every** format on day one. Variants flagged 🕒 are **reserved**;
their `D` types and detail tables land in a follow-up tracked by issue #56.

| discriminant | variant | format | status | detail doc |
|---|---|---|---|---|
| 0 | `Srt` | SubRip | ✓ rev 5 | (no detail table — `SrtData` is a unit) |
| 1 | `Vtt` | WebVTT | ✓ rev 5 | [subtitle_cue_vtt.md](subtitle_cue_vtt.md) |
| 2 | `Ass` | ASS / SSA | ✓ rev 5 | [subtitle_cue_ass.md](subtitle_cue_ass.md) |
| 3 | `MicroDvd` | MicroDVD | 🕒 #56 | — |
| 4 | `SubViewer` | SubViewer | 🕒 #56 | — |
| 5 | `Sbv` | YouTube SBV | 🕒 #56 | — |
| 6 | `Lrc` | LRC / Enhanced LRC | ✓ rev 5 | [subtitle_cue_lrc.md](subtitle_cue_lrc.md) |
| 7 | `Ttml` | TTML | 🕒 #56 | — |
| 8 | `Sami` | SAMI | 🕒 #56 | — |
| 9 | `VobSub` | DVD VobSub bitmap | 🕒 #56 | — |
| 10 | `Pgs` | Blu-ray PGS bitmap | 🕒 #56 | — |
| 11 | `Cea608` | CEA-608 captions | 🕒 #56 | — |
| 12 | `EbuStl` | EBU STL teletext | 🕒 #56 | — |

The rev-4 `image: Bytes` + `ocr_text: LocalizedText` fields are gone from
the base; bitmap formats will own their inline image bytes on their `D`
type when they land per #56 (the OCR pipeline integration is also tracked
separately in #57).

## Per-track aggregates

A `SubtitleTrack` may carry per-format aggregate rows that cues reference:

| aggregate | format | doc |
|---|---|---|
| `VttRegion<Id>` | WebVTT | [subtitle_track_vtt_region.md](subtitle_track_vtt_region.md) |
| `VttStyleBlock<Id>` | WebVTT | [subtitle_track_vtt_style.md](subtitle_track_vtt_style.md) |
| `AssStyle<Id>` | ASS/SSA | [subtitle_track_ass_style.md](subtitle_track_ass_style.md) |
| `LrcMetadata<Id>` | LRC | [subtitle_track_lrc_metadata.md](subtitle_track_lrc_metadata.md) |

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

The 9 deferred variants exist on `SubtitleCueKind` from rev 5 onward so
the discriminator is stable across releases. Adding any of them is a
purely additive change: ship the `D` type + detail table + row-mapper
helpers, no wire-format break.

**Status: rev 5 — polymorphic foundation + SRT / WebVTT / ASS / LRC
implemented.** Long-tail formats deferred to GitHub issue #56; bitmap OCR
pipeline tracked separately in #57.
