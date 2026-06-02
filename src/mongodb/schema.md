# `mediaschema::mongodb` — bson collection schemas

This document is the human-readable companion to the
`src/mongodb/*.rs` mapping code. For every locked domain aggregate it
records:

- The Mongo collection name.
- Each top-level bson field (name + type).
- The `IndexModel`s constructed by [`indexes::all_indexes`](crate::mongodb::indexes::all_indexes).

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
| `VoiceFingerprint<Uuid7>` | nested `{ vector_id: Binary(uuid), dimensions: Int32, extracted_at: DateTime, confidence: Double or Null, provenance: { … } }` |
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

### `media_files`

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
| `audio_track_id` | `Binary(uuid)` (`AudioTrack.id`) |
| `cluster_id` | `Int64` |
| `name` | `String` |
| `speech_duration` | `Timestamp` or `Null` |
| `voiceprint` | `VoiceFingerprint` sub-doc or `Null` |
| `person_id` | `Binary(uuid)` (`Person.id`) or `Null` |


`voiceprint` is the per-track aggregated centroid: `{ vector_id:
Binary(uuid), dimensions: Int32, extracted_at: DateTime, confidence:
Double or Null, provenance: { model_name, model_version,
prompt_version, indexer_version } }`. `person_id` is the FK back into
the `persons` collection (the cross-track identity anchor).

Indexes: `audio_track_id`, `person_id`.

### `persons`

The cross-track / cross-modality identity anchor. One `Person` ↔ many
`Speaker`s (one per track they appear in). Modality-neutral: a future
`FaceDetection.person` link hangs off this aggregate without
reshaping it.

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `name` | `String` (`""` = unnamed) |
| `confidence` | `Int32` (0 = `AutoMatched`, 1 = `UserConfirmed`) |
| `voiceprint` | `VoiceFingerprint` sub-doc or `Null` |
| `created_at` | `DateTime` |
| `updated_at` | `DateTime` |

`voiceprint` is the aggregated canonical voiceprint (the centroid
across all linked `Speaker`s' per-track voiceprints) — same embedded
shape as on `speakers`: `{ vector_id: Binary(uuid), dimensions:
Int32, extracted_at: DateTime, confidence: Double or Null,
provenance: { model_name, model_version, prompt_version,
indexer_version } }`. Only meaningful when the contributing samples
share one `(model, version)` pair (see `VoiceFingerprint`'s
`provenance`).

Indexes: compound `(voiceprint.provenance.model_name,
voiceprint.provenance.model_version)` for "find Persons by embedding
model" queries.

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
| `scene_id` | `Binary(uuid)` (`Scene.id`) |
| `favorite` | `Boolean` |
| `user_tag_ids` | `[Binary(uuid)]` (FK array → `user_tags(_id)`) |
| `rating` | `Int32` (0–5) or `Null` |
| `note` | `String` |
| `updated_at` | `DateTime` |

Indexes: unique `scene_id`, plus `favorite`, `rating`.

### `audio_facets`

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `media_id` | `Binary(uuid)` (`Media.id`, unique — 1:1) |
| `track_progress` | `{ total, indexed, failed }` (`Int64` fields) |
| `total_segments` | `Int64` |

The `tracks` reverse-FK list is **not** stored — it is derived by
querying `audio_tracks` where `parent == audio._id` (mirrors the sqlx
convention).

Indexes: unique `media_id` (1:1 with `Media`).

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

Indexes: `audio_id`, `is_primary`, `content`, `language`.

### `audio_segments`

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `audio_track_id` | `Binary(uuid)` |
| `index` | `Int64` |
| `span` | `TimeRange` |
| `speaker_id` | `Binary(uuid)` or `Null` |
| `text` | `LocalizedText` |
| `language` | `String` or `Null` |
| `words` | `[{ text, span, score, language }]` |
| `no_speech_prob` | `Double` or `Null` |
| `avg_logprob` | `Double` or `Null` |
| `temperature` | `Double` or `Null` |
| `voice_fingerprint` | `VoiceFingerprint` sub-doc or `Null` |

`voice_fingerprint` is the per-segment voice embedding (same nested
shape as `speakers.voiceprint`): `{ vector_id, dimensions,
extracted_at, confidence, provenance }`.

Indexes: `audio_track_id`, unique `(audio_track_id, index)`, `speaker_id`.

### `video_facets`

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `media_id` | `Binary(uuid)` (`Media.id`, unique — 1:1) |
| `total_scenes` | `Int64` |
| `track_progress` | `{ total, indexed, failed }` (`Int64` fields) |

The `tracks` reverse-FK list is **not** stored — it is derived by
querying `video_tracks` where `parent == video._id` (mirrors the sqlx
convention).

Indexes: unique `media_id` (1:1 with `Media`).

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
querying the `scenes` collection (keyed by `video_track_id`).

Indexes: `video_id`, `is_primary`.

### `scenes`

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `video_track_id` | `Binary(uuid)` (`VideoTrack.id`) |
| `index` | `Int64` |
| `span` | `TimeRange` |
| `detector` | `Int32` |
| `description` | `String` |

The `keyframes` reverse-FK list is **not** stored — it is derived by
querying the `keyframes` collection (keyed by `scene_id`).

Indexes: `video_track_id`, unique `(video_track_id, index)`.

### `keyframes`

The widest schema — the full apple-vision + colorthief + VLM bundle.
See `video.rs`'s detection-VO helpers (`detection_to_bson`,
`bbox_to_bson`, `human_to_bson`, …) for the per-sub-VO layouts.
`humans` is a nested document with nine arrays (`subjects`, `faces`,
`body_poses`, `hand_poses`, `body_poses_3d`, `instance_masks`,
`face_rectangles`, `face_landmarks`, `segmentation_masks`). All
detection arrays are embedded sub-documents — no reverse-FK lists.

Indexes: `scene_id`.

### `subtitle_facets`

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `media_id` | `Binary(uuid)` (`Media.id`, unique — 1:1) |
| `track_progress` | `{ total, indexed, failed }` (`Int64` fields) |

The `tracks` reverse-FK list is **not** stored — it is derived by
querying the `subtitle_tracks` collection (keyed by `subtitle_id`).

Indexes: unique `media_id` (1:1 with `Media`).

### `subtitle_tracks`

Per `schema/subtitle_track.md` r3; see `subtitle.rs` for the full
top-to-bottom shape.

The `cues` reverse-FK list is **not** stored — it is derived by
querying the `subtitle_cues` collection (keyed by `subtitle_track_id`).

Indexes: `subtitle_id`, `is_primary`, `language`.

### `subtitle_cues`

Polymorphic cue document. The base shape is shared across all
subtitle formats; per-format detail fields ride on the same document
and are dispatched by the `kind` discriminator.

**Base fields** (always present):

| field | type | notes |
| --- | --- | --- |
| `_id` | `Binary(uuid)` | `SubtitleCue.id` (`Uuid7`) |
| `subtitle_track_id` | `Binary(uuid)` | FK → `subtitle_tracks(_id)` |
| `ordinal` | `Int64` | per-track 0-based position |
| `span` | `TimeRange` | cue interval (start/end PTS) |
| `text` | `LocalizedText` | plain text (and translation, if any) |
| `kind` | `Int32` | `SubtitleCueKind` discriminator (slug table below) |

**Discriminator slug table** (`SubtitleCueKind` → `kind` value):

| slug | int | format |
| --- | --- | --- |
| `Srt` | 0 | SubRip |
| `Vtt` | 1 | WebVTT |
| `Ass` | 2 | Advanced SubStation Alpha |
| `Lrc` | 3 | LRC / Enhanced LRC |
| _(reserved)_ | 4–… | `MicroDvd`, `SubViewer`, `Sbv`, `Ttml`, `Sami`, `VobSub`, `Pgs`, `Cea608`, `EbuStl` — discriminants reserved (no detail / aggregate collections yet, deferred to #56) |

**Per-format detail fields** (present iff `kind == …`):

- `kind = Srt` — no extra fields (base only).
- `kind = Vtt` — `cue_identifier: String`, `vertical: Int32?`,
  `line_value: String`, `line_align: Int32?`, `position_value: String`,
  `position_align: Int32?`, `size_value: Double?`,
  `text_align: Int32?`, `region_id: Binary(uuid)?` (FK →
  `subtitle_track_vtt_regions(_id)`), `voice: String`,
  `styled_text: String`.
- `kind = Ass` — `layer: Int32`, `style_id: Binary(uuid)?` (FK →
  `subtitle_track_ass_styles(_id)`), `name: String`,
  `margin_l: Int32`, `margin_r: Int32`, `margin_v: Int32`,
  `effect: String`, `styled_text: String`.
- `kind = Lrc` — `has_word_timing: Boolean`. When set, per-word rows
  live in the `subtitle_cue_lrc_words` child collection (`subtitle_cue_id`
  FK).

Indexes: `subtitle_track_id`, unique `(subtitle_track_id, ordinal)`.

### `subtitle_track_vtt_regions`

Per-track WebVTT `REGION` block (one row per `REGION`). Referenced
from `subtitle_cues.region_id` (FK) when `kind = Vtt`.

| field | type | notes |
| --- | --- | --- |
| `_id` | `Binary(uuid)` | `VttRegion.id` |
| `subtitle_track_id` | `Binary(uuid)` | FK → `subtitle_tracks(_id)` |
| `name` | `String` | REGION identifier (unique within track) |
| `width` | `Double` | viewport-percentage |
| `lines` | `Int64` | line count |
| `region_anchor_x` / `_y` | `Double` | anchor (percentages) |
| `viewport_anchor_x` / `_y` | `Double` | viewport-anchor (percentages) |
| `scroll_up` | `Boolean` | scroll direction |

Indexes: `subtitle_track_id`, unique `(subtitle_track_id, name)`.

### `subtitle_track_vtt_styles`

Per-track WebVTT `STYLE` block (ordered CSS chunks).

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `subtitle_track_id` | `Binary(uuid)` |
| `ordinal` | `Int64` |
| `css_text` | `String` |

Indexes: `subtitle_track_id`, unique `(subtitle_track_id, ordinal)`.

### `subtitle_track_ass_styles`

Per-track ASS `[V4+ Styles]` row. Referenced from
`subtitle_cues.style_id` (FK) when `kind = Ass`.

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` |
| `subtitle_track_id` | `Binary(uuid)` |
| `name` | `String` (unique within track) |
| `fontname`, `fontsize` | `String`, `Double` |
| `primary_colour`, `secondary_colour`, `outline_colour`, `back_colour` | `Int64` each (RGBA packed) |
| `bold`, `italic`, `underline`, `strikeout` | `Boolean` each |
| `scale_x`, `scale_y`, `spacing` | `Int32` each |
| `angle` | `Double` |
| `border_style`, `alignment` | `Int32` each (small-enum codes) |
| `outline`, `shadow` | `Double` each |
| `margin_l`, `margin_r`, `margin_v`, `encoding` | `Int32` each |

Indexes: `subtitle_track_id`, unique `(subtitle_track_id, name)`.

### `subtitle_track_lrc_metadata`

Per-track LRC header block (the `[ti]`, `[ar]`, `[al]`, … tags). The
metadata _is_ the collection of metadata fields for that track (1:1
with `subtitle_tracks`), so the document's `_id` IS the
`subtitle_track_id`.

| field | type |
| --- | --- |
| `_id` | `Binary(uuid)` (= `SubtitleTrack.id`) |
| `title`, `artist`, `album`, `author`, `creator`, `length` | `String` each |
| `offset_ms` | `Int32` |

Indexes: `_id` only (1:1 with `subtitle_tracks`).

### `subtitle_cue_lrc_words`

Per-cue word-timing row, written only when Enhanced LRC carries
word-level timestamps (`kind = Lrc` AND `has_word_timing = true`).

| field | type |
| --- | --- |
| `subtitle_cue_id` | `Binary(uuid)` (FK → `subtitle_cues(_id)`) |
| `ordinal` | `Int64` |
| `text` | `String` |
| `start_pts` | `Int64` |

Indexes: `subtitle_cue_id`, unique `(subtitle_cue_id, ordinal)`.
