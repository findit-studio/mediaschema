# `mediaschema::mongodb` — bson collection schemas

This document is the human-readable companion to the
`src/mongodb/*.rs` mapping code. For every locked domain aggregate it
records:

- The Mongo collection name.
- Each top-level bson field (name + type).
- The `IndexModel`s constructed by [`indexes::all_indexes`].

Round-trip identity is verified at unit-test level (see each module's
`#[cfg(test)] mod tests`); the test pattern is `domain → Document →
domain`, then `assert_eq!`.

## Type cheatsheet

| Domain type | bson representation |
| --- | --- |
| `Uuid7` | `Binary` subtype 4 (UUID), 16 bytes |
| `FileChecksum` | `Binary` subtype 0 (generic), 32 bytes |
| `SmolStr` | `String` (empty preserved as `""`) |
| `jiff::Timestamp` | `DateTime` (ms-since-epoch — sub-ms precision is dropped) |
| `mediatime::Timestamp` | nested `{ pts: i64, timebase: { num: i64, den: i64 } }` |
| `mediatime::TimeRange` | nested `{ start: i64, end: i64, timebase: … }` |
| `Rgba` | nested `{ r: i32, g: i32, b: i32, a: i32 }` |
| `ErrorInfo` | nested `{ code: i64, message: String }` |
| `Provenance` | nested `{ model_name: String, model_version: String, prompt_version: String, indexer_version: String }` |
| `LocalizedText` | nested `{ src: String, translated: String }` |
| `Location<Uuid7>` | nested `{ kind: "local", volume: Binary, components: [String] }` |
| domain enums | `Int32` (e.g. `MediaKind::Video → 0`) |
| `*IndexStatus` bitflags | `Int64` (raw `.bits()` value) |
| `MediaErrorFlags` | `Int64` (raw `.bits()` value) |

Optional values are stored as `Null` (never omitted) so the document
shape is constant.

## Collections

### `media`

| field | type | notes |
| --- | --- | --- |
| `_id` | `Binary(uuid)` | `Media.id` (`Uuid7`) |
| `checksum` | `Binary(generic, 32)` | unique index |
| `format` | `String` | container slug |
| `size` | `Int64` | |
| `duration` | `Timestamp` or `Null` | `mediatime::Timestamp` |
| `kind` | `Int32` | `0=Video / 1=Audio` |
| `video` | `Binary(uuid)` or `Null` | facet FK |
| `audio` | `Binary(uuid)` or `Null` | facet FK |
| `subtitle` | `Binary(uuid)` or `Null` | facet FK |
| `error_flags` | `Int64` | `MediaErrorFlags.bits()` |
| `probe_error` | `ErrorInfo` or `Null` | |
| `capture_date` | `DateTime` or `Null` | EXIF |
| `device` | `{ make: String, model: String }` or `Null` | |
| `gps` | `{ lat: Double, lon: Double, altitude: Double or Null }` or `Null` | |

Indexes: unique `checksum`, plus `kind`, `error_flags`, `capture_date`.

### `media_file`

One **physical copy** of a piece of content (N copies ↔ 1 `Media`).
`location` is kept as a natural embedded sub-document (Mongo indexes /
queries embedded docs first-class — no flattening). `name` is **derived**
from `location`'s last path component, never stored.

| field | type | notes |
| --- | --- | --- |
| `_id` | `Binary(uuid)` | `MediaFile.id` (`Uuid7`) |
| `media_id` | `Binary(uuid)` | FK → `media(_id)` (the shared content row) |
| `created_at` | `DateTime` or `Null` | filesystem creation time (`Null` = no birth time) |
| `location` | `{ kind: "local", volume: Binary, components: [String] }` | structured copy location |
| `watched_location_id` | `Binary(uuid)` | FK → `watched_locations(_id)` (discovering watch) |
| `watch_volume` | `Binary(uuid)` | cached `WatchedLocation.volume` (volume-consistency) |

Indexes: `media_id`, `watched_location_id`.

### `watched_locations`

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `volume` | `Binary(uuid)` (stable volume UUID) |
| `recursive` | `Boolean` |
| `enabled` | `Boolean` |
| `is_ejectable` | `Boolean` |
| `added_at` | `DateTime` |
| `last_reconciled_at` | `DateTime` or `Null` |
| `last_reconcile_status` | `Int32` (`0=Ok / 1=Partial / 2=Failed`) or `Null` |
| `last_error` | `ErrorInfo` or `Null` |

Indexes: `volume`, `enabled`.

### `speakers`

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `parent` | `Binary(uuid)` (`AudioTrack.id`) |
| `cluster_id` | `Int64` |
| `name` | `String` |
| `speech_duration` | `Timestamp` or `Null` |

Indexes: `parent`.

### `user_tags`

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `name` | `String` |
| `color` | `Rgba` or `Null` |
| `created_at` | `DateTime` |

Indexes: `name`.

### `scene_annotations`

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `scene` | `Binary(uuid)` (`Scene.id`) |
| `favorite` | `Boolean` |
| `user_tags` | `[Binary(uuid)]` |
| `rating` | `Int32` (0–5) or `Null` |
| `note` | `String` |
| `updated_at` | `DateTime` |

Indexes: unique `scene`, plus `favorite`, `rating`.

### `audio_facets`

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `track_progress` | `{ total, indexed, failed }` (`Int64` fields) |
| `total_segments` | `Int64` |

The `tracks` reverse-FK list is **not** stored — it is derived by
querying `audio_tracks` where `parent == audio._id` (mirrors the sqlx
convention).

Indexes: `_id` only.

### `audio_tracks`

Full per-recording shape from `schema/audio_track.md` r3 — see
`audio.rs`'s `From`/`TryFrom` impl for the field list (`codec`,
`profile`, `sample_rate`, `channels`, `channel_layout`, `bit_rate`,
`bit_rate_mode`, `bits_per_sample`, `is_lossless`, `duration`,
`start_pts`, `language`, `detected_language`, `language_mismatch`,
`disposition`, `is_primary`, `auto_selected`, `content`, `speech_ratio`,
`is_silent`, `loudness`, `fingerprint`, `isrc`, `acoustid`,
`musicbrainz_recording_id`, `tags`, `cover_art`, `provenance`,
`index_status`, `index_errors`).

The `speakers` and `segments` reverse-FK lists are **not** stored —
they are derived by querying `speakers` and `audio_segments` (both
keyed by `parent`).

Indexes: `parent`, `is_primary`, `content`, `language`.

### `audio_segments`

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `parent` | `Binary(uuid)` |
| `index` | `Int64` |
| `span` | `TimeRange` |
| `speaker` | `Binary(uuid)` or `Null` |
| `text` | `LocalizedText` |
| `language` | `String` or `Null` |
| `words` | `[{ text, span, score, language }]` |
| `no_speech_prob` | `Double` or `Null` |
| `avg_logprob` | `Double` or `Null` |
| `temperature` | `Double` or `Null` |

Indexes: `parent`, unique `(parent, index)`, `speaker`.

### `video_facets`

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `total_scenes` | `Int64` |
| `track_progress` | `{ total, indexed, failed }` (`Int64` fields) |

The `tracks` reverse-FK list is **not** stored — it is derived by
querying `video_tracks` where `parent == video._id` (mirrors the sqlx
convention).

Indexes: `_id` only.

### `video_tracks`

Full per-stream descriptor from `schema/video_track.md` r8 (see
`video.rs`'s `From`/`TryFrom`). The `mediaframe` descriptor types map per
the table in `mongodb/mod.rs`: `codec` → `String` slug; `pixel_format` /
`rotation` / `field_order` / `stereo_mode` → `Int32` codes; `disposition`
→ `Int64` bits; `dimensions` → `{ w, h }`; `sample_aspect_ratio` /
`frame_rate` → `{ num, den[, is_vfr] }`; `visible_rect` →
`{ x, y, width, height }`; `color` → 5 `Int32` enum codes; `hdr_static` →
`{ mastering?, content_light? }`; `dovi` →
`{ profile, level, rpu_present, el_present, bl_signal_compat_id }`.

The `scenes` reverse-FK list is **not** stored — it is derived by
querying the `scenes` collection (keyed by `parent`).

Indexes: `parent`, `is_primary`.

### `scenes`

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `parent` | `Binary(uuid)` (`VideoTrack.id`) |
| `index` | `Int64` |
| `span` | `TimeRange` |
| `detector` | `Int32` |
| `description` | `String` |

The `keyframes` reverse-FK list is **not** stored — it is derived by
querying the `keyframes` collection (keyed by `parent`).

Indexes: `parent`, unique `(parent, index)`.

### `keyframes`

The widest schema — the full apple-vision + colorthief + VLM bundle.
See `video.rs`'s detection-VO helpers (`detection_to_bson`,
`bbox_to_bson`, `human_to_bson`, …) for the per-sub-VO layouts.
`humans` is a nested document with nine arrays (`subjects`, `faces`,
`body_poses`, `hand_poses`, `body_poses_3d`, `instance_masks`,
`face_rectangles`, `face_landmarks`, `segmentation_masks`). All
detection arrays are embedded sub-documents — no reverse-FK lists.

Indexes: `parent`.

### `subtitle_facets`

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `parent` | `Binary(uuid)` (`Media.id`) |
| `tracks` | `[Binary(uuid)]` |
| `track_progress` | `{ total, indexed, failed }` |

Indexes: `parent`.

### `subtitle_tracks`

Per `schema/subtitle_track.md` r3; see `subtitle.rs` for the full
top-to-bottom shape.

Indexes: `parent`, `is_primary`, `language`.

### `subtitle_cues`

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `parent` | `Binary(uuid)` (`SubtitleTrack.id`) |
| `index` | `Int64` |
| `span` | `TimeRange` |
| `text` | `LocalizedText` |
| `styled_text` | `String` |
| `image` | `Binary(generic)` (empty = absent) |
| `ocr_text` | `LocalizedText` |

Indexes: `parent`, unique `(parent, index)`.
