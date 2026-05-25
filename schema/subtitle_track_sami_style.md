# `SamiStyle<Id>` — per-track SAMI `<STYLE>` class  *(rev 1)*

## Domain meaning

One CSS class block from a SAMI track's `<STYLE>` header. Cues opt in
via [`SamiData::class_name`](subtitle_cue_sami.md) (SAMI uses string
selectors, not numeric ids — the FK key is the `class_name`).

`css_text` carries the raw CSS body verbatim for a lossless round
trip.

## Fields

| field | domain type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | canonical identity |
| `subtitle_track_id` | `Id` | FK → `SubtitleTrack.id` |
| `class_name` | `SmolStr` | SAMI class selector (e.g. `ENCC`) |
| `css_text` | `SmolStr` | CSS body (`{color: yellow; …}`); `""` = absent |

## Invariants

- Non-nil `id` and `subtitle_track_id` (enforced).
- `(subtitle_track_id, class_name)` unique within a track.

## Projection notes

- **sqlx**: `subtitle_track_sami_style` table; `subtitle_track_id`
  indexed.
- **mongodb**: `_id` = UUIDv7; UNIQUE on `(subtitle_track_id, class_name)`.
- **wire**: a `SamiStyle` message.

**Status: rev 1 — implemented in PR #86 (closes #56).**
