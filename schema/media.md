# `Media<Id>` — root / file entity  *(rev 7 — LOCKED, user-approved)*

## Domain meaning

The indexed media **file** itself. After the refinement, *all* file-level
scalar metadata lives here and nowhere else; the kind facets are thin
aggregates and stream/codec data is per-track.

## Cross-cutting (locked rules)

Generic over `Id` (single **UUIDv7** key — Postgres `uuid` / Mongo `_id`; FKs
are the UUID). `FileChecksum` distinct 32-byte newtype. Wall-clock =
`jiff::Timestamp` (ms); media-time = `mediatime`/`TrackTime`. Genuine nested
VOs (`Device`, `GeoLocation`, `TrackTime`) nested; entity's own meta flattened
in. Conversions deferred.

## Fields (flat — the file level)

| field | domain type | wire origin | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | `MediaMeta.id: bytes` | the single key |
| `checksum` | `FileChecksum` | `MediaMeta.checksum: bytes` | content identity; unique |
| `name` | `SmolStr` | `MediaMeta.name` | **file** name |
| `format` | `ContainerFormat` (enum) | `*Meta.format`/`container_format` | **file/container** format (MP4/MKV/MKA…); **codec is per-track** |
| `size` | `u64` | `MediaMeta.size` | file bytes |
| `duration` | `Option<TrackTime>` | `MediaMeta.time` | **overall** media length (per-track duration is on the track) |
| `created_at` | `jiff::Timestamp` | `MediaMeta.created_at: i64` | Unix **ms** |
| `kind` | `MediaKind` (enum) | `Media.kind: DbMediaKind` | closed; may gain kinds |
| `video` | `Option<Id>` | `Media.video_id` | ref → `Video` facet |
| `audio` | `Option<Id>` | `Media.audio_id` | ref → `Audio` facet |
| `subtitle` | `Option<Id>` | `Media.subtitle_id` | ref → `Subtitle` facet |
| `error_flags` | `MediaErrorFlags` (bitflags! **u16**) | — (rollup) | `VIDEO_ERROR`/`AUDIO_ERROR`/`SUBTITLE_ERROR` + reserved bits |
| `probe_error` | `Option<ErrorInfo>` | `Media.index_error` (collapsed) | file-level probe failure only |
| `capture_date` | `Option<jiff::Timestamp>` | `Media.capture_date: i64` | EXIF; ms; `0→None` |
| `device` | `Option<Device>` (nested VO) | `Media.device_make`+`_model` | `{ make, model }` |
| `gps` | `Option<GeoLocation>` (nested VO) | `Media.gps_location` | owned decimal; parsed via `locat` |

**Not here:** `codec`, `dimensions`, `frame_rate`, `bit_rate`, per-track
`duration`, index/error *details* → all per-track. No `meta:` wrapper, no
`Vec<ErrorInfo>`.

## Error model

Details per-track (`*Track.index_errors`). `error_flags` (`u16` bitflags) is a
maintained rollup of `kind.track_progress.failed > 0` — bit set ⇒ drill down.
`probe_error` is the one non-track case (file unprobeable ⇒ no tracks).

## Projection notes

- **sqlx**: flat `media` table; `id` PK (uuid); `checksum` UNIQUE;
  `kind`/`format`/`created_at` indexed; `error_flags` 2-byte `INTEGER` +
  generated per-bit booleans; `device`/`gps` flattened; facet FKs (UUIDv7).
- **mongodb**: `_id`=UUIDv7; `device`/`gps` embedded.
- **graphql**: expose `error_flags`+`probe_error`; opaque external id.

**Status: LOCKED (rev 7) — user-approved.**
