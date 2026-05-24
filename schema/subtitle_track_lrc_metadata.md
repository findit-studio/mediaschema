# `LrcMetadata<Id>` — per-track LRC header metadata  *(rev 1)*

## Domain meaning

The set of `[ti:]` / `[ar:]` / `[al:]` / `[au:]` / `[by:]` / `[length:]`
/ `[offset:]` header tags from an LRC file, materialised as a 1:1 sibling
of the parent [`SubtitleTrack`](subtitle_track.md).

## Fields

| field | domain type | notes |
|---|---|---|
| `subtitle_track_id` | `Id` | PK + FK → `SubtitleTrack.id` (1:1) |
| `title` | `SmolStr` | `[ti:]`; `""` = absent |
| `artist` | `SmolStr` | `[ar:]` |
| `album` | `SmolStr` | `[al:]` |
| `author` | `SmolStr` | `[au:]` |
| `creator` | `SmolStr` | `[by:]` (LRC editor / generator credit) |
| `length` | `SmolStr` | `[length:]` (e.g. `3:25.50`); raw text |
| `offset_ms` | `i32` | `[offset:]` — global ms offset applied to every line; `0` default |

## Invariants

- Non-nil `subtitle_track_id`.
- All free-text fields follow the `""` = absent rule.
- `offset_ms` is signed (LRC allows negative offsets to nudge sync earlier).

## Projection notes

- **sqlx**: `subtitle_track_lrc_metadata` table; `subtitle_track_id` is
  the PK (enforces 1:1 with `subtitle_track`).
- **mongodb**: `_id` = the track id.
- **wire**: an `LrcMetadata` message.

**Status: rev 1 — implemented in PR #34.**
