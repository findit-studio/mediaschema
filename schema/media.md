# `Media<Id>` — root / content entity  *(rev 10 — LOCKED, user-approved; content/copy split)*

## Domain meaning

The indexed media **content** — **one row per content hash**. After the
content/copy split (rev 9), `Media` is the *content*, not a file: many
physical copies of the same bytes (the same file duplicated across
folders/volumes) collapse to one `Media`. The **copy-specific** metadata
(file name, path, filesystem creation time, discovering watch) moved to the
per-copy [`MediaFile`](media_file.md) aggregate; `files: Vec<Id>` is the
reverse lookup to those copies. *All* **content-intrinsic** scalar metadata
lives here and nowhere else; the kind facets are thin aggregates and
stream/codec data is per-track.

## Cross-cutting (locked rules)

Generic over `Id` (single **UUIDv7** key — Postgres `uuid` / Mongo `_id`; FKs
are the UUID). `FileChecksum` distinct 32-byte newtype. Wall-clock =
`jiff::Timestamp` (ms); media-time = `::mediatime` (`TrackTime`). **EXIF/capture
types — `GeoLocation` and `Device` — are `::mediaframe` externs** (rev 8: the
EXIF/capture-metadata charter; *no longer mediaschema-owned VOs*). Entity's own
meta flattened in. Conversions deferred.

## Fields (flat — the content level)

| field | domain type | wire origin | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | `MediaMeta.id: bytes` | the single key |
| `checksum` | `FileChecksum` | `MediaMeta.checksum: bytes` | content identity; **unique** (one `Media` per hash) |
| `format` | `ContainerFormat` (enum) | `*Meta.format`/`container_format` | **container** format (MP4/MKV/MKA…); **codec is per-track** |
| `size` | `u64` | `MediaMeta.size` | content size in bytes |
| `duration` | `Option<TrackTime>` | `MediaMeta.time` | **overall** media length (per-track duration is on the track) |
| `kind` | `MediaKind` (enum) | `Media.kind: DbMediaKind` | closed; may gain kinds |
| `files` | `Vec<Id>` | — (reverse lookup) | reverse lookup → [`MediaFile`](media_file.md) copies (reverse side of `MediaFile.media_id`); starts empty |
| `video_id` | `Option<Id>` | `Media.video_id` | ref → `Video` facet |
| `audio_id` | `Option<Id>` | `Media.audio_id` | ref → `Audio` facet |
| `subtitle_id` | `Option<Id>` | `Media.subtitle_id` | ref → `Subtitle` facet |
| `error_flags` | `MediaErrorFlags` (bitflags! **u16**) | — (rollup) | `VIDEO_ERROR`/`AUDIO_ERROR`/`SUBTITLE_ERROR` + reserved bits |
| `probe_error` | `Option<ErrorInfo>` | `Media.index_error` (collapsed) | file-level probe failure only |
| `capture_date` | `Option<jiff::Timestamp>` | `Media.capture_date: i64` | EXIF; ms; `0→None` (stays `jiff` — wall-clock standard, not a mediaframe type) |
| `device` | `Option<mediaframe::Device>` (**extern**) | `Media.device_make`+`_model` | `{ make, model }` — `::mediaframe` (EXIF/capture charter) |
| `gps` | `Option<mediaframe::GeoLocation>` (**extern**) | `Media.gps_location` | `{lat,lon,altitude}` decimal; ISO-6709 parse/format **in mediaframe** (the `locat` owned-vs-borrowed reasoning now lives there) |

**Not here:** `codec`, `dimensions`, `frame_rate`, `bit_rate`, per-track
`duration`, index/error *details* → all per-track. No `meta:` wrapper, no
`Vec<ErrorInfo>`. **Copy-specific metadata** (`name`, filesystem
`created_at`, `location`, discovering watch) is **not** here either — it
moved to the per-copy [`MediaFile`](media_file.md) aggregate (rev 9
content/copy split). `Media` is the content; a `MediaFile` is one physical
copy of it.

## Error model

Details per-track (`*Track.index_errors`). `error_flags` (`u16` bitflags) is a
maintained rollup of `kind.track_progress.failed > 0` — bit set ⇒ drill down.
`probe_error` is the one non-track case (file unprobeable ⇒ no tracks).

## Projection notes

- **sqlx**: flat `media` table; `id` PK (uuid); `checksum` UNIQUE;
  `kind`/`format` indexed; `error_flags` 2-byte `INTEGER` +
  generated per-bit booleans; `device`/`gps` flattened (extern types still
  flatten to columns: `device_make`/`device_model`, `gps_lat`/`lon`/`alt`);
  facet FKs (UUIDv7). No `name`/`created_at` columns — those are
  copy-specific and live on `media_file`. `files` is **not** a stored
  column: it is the reverse side of the `media_file.media_id` FK,
  materialised by a join (`SELECT id FROM media_file WHERE media_id = ?`).
- **mongodb**: `_id`=UUIDv7; `device`/`gps` embedded (mediaframe externs).
- **graphql**: expose `error_flags`+`probe_error`; opaque external id.

**Status: LOCKED (rev 9) — user-approved.** *(rev 9: content/copy split
(codex review finding on PR #13). `Media` is now the **content** row (one
per content hash); the copy-specific `name` and `created_at` fields are
**removed** and moved to the new per-copy [`MediaFile`](media_file.md)
aggregate, along with the file's `location` and discovering watch. A new
`files: Vec<Id>` field is the reverse lookup to those copies. All
content-intrinsic fields — `checksum` (UNIQUE), `size`, `format`,
`duration`, `kind`, `capture_date`/`device`/`gps`, `error_flags`,
`probe_error`, facet FKs — are unchanged. rev 8 carried over: `device` /
`gps` are `::mediaframe` externs (EXIF/capture charter); ISO-6709 parse in
mediaframe; `capture_date` stays `jiff` — and the domain stores the
supplied `Option<jiff::Timestamp>` **faithfully**: `Some(epoch)` (0 ms) is
a real timestamp and is preserved distinctly from `None`. The nullable
domain/sqlx column can represent both, so no normalization happens in the
constructor/builders/setters. Translating the legacy wire `0` (Unix epoch,
ms) sentinel — where the int64 genuinely cannot distinguish epoch from
missing — to `None` is the responsibility of the wire-decode adapter (the
deferred #17-20 wire-conversion wave), not the domain. Mechanical
`::mediaframe::` path applies when
mediaschema externs the post-0.1.0 mediaframe minor — tracked in
[mediaframe-candidates.md](mediaframe-candidates.md).)*
