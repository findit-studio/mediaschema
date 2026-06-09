# Changelog

All notable changes to mediaschema are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning policy: see the [Versioning section in `README.md`](README.md#versioning)
and the resolution comment on [#59](https://github.com/Findit-AI/mediaschema/issues/59).

## Unreleased

## 0.2.0 — Unreleased

### Added

- **`Chapter<Id>` aggregate** (`schema/chapter.md` rev 1) — one row per
  container chapter (`AVFormatContext.chapters[i]`), with `media_id` FK,
  `index` ordinal, `source_id` verbatim AVChapter id, `time_range`
  (`mediatime::TimeRange`), `title` hoisted from
  `metadata["title"]` (any case; first-match wins; SQL-indexed via
  `LOWER(title)`), and `metadata: IndexMap<SmolStr, SmolStr>` for the
  remaining AVDictionary entries preserving insertion order. Full
  backend coverage: domain (`src/domain/aggregates/chapter.rs`), buffa
  wire bridge (`src/buffa/chapter.rs`), sqlx postgres / mysql / sqlite
  row mappers + DDL (`chapter` + `chapter_metadata` companion tables
  preserving `IndexMap` insertion order via `ordinal`), and mongodb
  document bridge with `Chapters` collection + index registration
  (`chapter_indexes()` — case-insensitive `title` collation).
- `Media.nb_streams: u32` — verbatim `AVFormatContext.nb_streams`, the
  total stream count from the container probe (including data /
  attachment streams the schema may not model per-track).
- `Media.nb_chapters: u32` — verbatim `AVFormatContext.nb_chapters`,
  kept symmetric with `nb_streams` so probe-without-chapter-fetch
  stays meaningful.
- `Media.chapters: Vec<Id>` — reverse lookup → `Chapter` rows
  (materialised by joining on `Chapter.media_id`; mirrors `files`).
- New optional dep `indexmap = "2"`, forwarded under the `std` feature
  (the default hasher generic is `std::hash::RandomState`) and through
  `indexmap?/serde` when `json` is enabled.
- New `BuffaError::DomainConstructorRejected(SmolStr)` and
  `MongoError::Chapter(...)` / `DomainConstructorRejected(String)`
  variants for the new aggregate's decode-edge failure modes.

### Per-track AVDictionary metadata + extras (`VideoTrack` rev 9, `AudioTrack` rev 6, `SubtitleTrack` rev 6)

- `AudioTrack.replay_gain: Option<mediaframe::audio::ReplayGain>` — the
  container's `REPLAYGAIN_*` tag bundle (track gain/peak required, album
  gain/peak independently optional). Distinct from `loudness`
  (`Loudness` = the EBU R128 / ITU-R BS.1770 measurement; `ReplayGain` =
  the tagger's normalization recommendation delta). Requires mediaframe
  `0.1.4`. SQL projection: `has_replay_gain bool` discriminator +
  4 nullable `real`/`FLOAT`/`REAL` columns
  (`replay_gain_{track_gain_db,track_peak,album_gain_db,album_peak}`).
  Proto wire: new `ReplayGain` message + `AudioTrack.replay_gain`
  (field 11). mongodb encodes as nested `replay_gain` document.

- `VideoTrack.avg_frame_rate: mediaframe::FrameRate` —
  `AVStream.avg_frame_rate` empirical / declared average. Equals
  `frame_rate` (= `AVStream.r_frame_rate`, the timebase reciprocal) for
  CFR content; diverges for VFR.
- `AudioTrack.sample_format: mediaframe::audio::SampleFormat` — the
  `AVCodecParameters.format` field for audio (`AV_SAMPLE_FMT_*`-coded;
  default `Unknown(u32::MAX)` = `AV_SAMPLE_FMT_NONE`). Parallel to
  `VideoTrack.pixel_format`. SQL projection: integer column carrying
  `SampleFormat::to_u32()`; `SampleFormat::Other(SmolStr)` collapses
  through the integer codec to `Unknown(u32::MAX)`.
- `VideoTrack.metadata` / `AudioTrack.metadata` /
  `SubtitleTrack.metadata`: `IndexMap<SmolStr, SmolStr>` — container
  `AVDictionary` entries, with conventionally-hoisted keys
  (`title`/`language`/audio `Tags`) already consumed into dedicated
  columns. Same insertion-order-preserving join-table shape as
  `chapter_metadata`: `(*_track_id, ordinal, key, value)` PK
  `(*_track_id, ordinal)`. mongodb stores `metadata` as a nested BSON
  document (insertion-ordered natively).
- Medium-aggregate features (`audio` / `video` / `subtitle`) now imply
  `std` (rather than `any(std, alloc)`) because the new
  `track.metadata: IndexMap` reaches the std-only default hasher
  `RandomState`. Same constraint as `Chapter`.

### Changed (BREAKING — 0.2.0 scope)

- `schema/media.md` rev 10 → **rev 11** (added 4 fields above).
- `schema/video_track.md` rev 8 → **rev 9** (added `avg_frame_rate` +
  `metadata`).
- `schema/audio_track.md` rev 4 → **rev 6** (added `sample_format`,
  `metadata`, and `replay_gain`).
- `schema/subtitle_track.md` rev 5 → **rev 6** (added `metadata`).
- Rust API: `Media` gains `nb_streams()` / `nb_chapters()` /
  `chapters_slice()` + matching `with_*` / `set_*` mutators;
  `VideoTrack` gains `avg_frame_rate()` / `metadata_ref()` +
  builders / setters; `AudioTrack` gains `sample_format_ref()`
  (returns `&SampleFormat` — non-`Copy` due to the `Other(SmolStr)`
  variant) / `metadata_ref()` + builders / setters; `SubtitleTrack`
  gains `metadata_ref()` + builders / setters. No existing accessor
  was removed or renamed.
- Proto wire: `Media` gains fields `13 nb_streams` / `14 nb_chapters`;
  new `Chapter` message + ordered-pair `KeyValue` message;
  `VideoStreamMeta` gains `avg_frame_rate` (field 7); `VideoTrack`
  gains `repeated KeyValue metadata` (field 9); `AudioTrack` gains
  `repeated KeyValue metadata` (field 10); `SubtitleTrack` gains
  `repeated KeyValue metadata` (field 14).
- sqlx schemas (all 3 dialects) gain `media.nb_streams` /
  `media.nb_chapters` integer columns; new `chapter` +
  `chapter_metadata` tables; new `video_track_metadata` /
  `audio_track_metadata` / `subtitle_track_metadata` companion tables;
  `video_track.avg_fr_num` + `avg_fr_den` columns;
  `audio_track.sample_format` integer column carrying
  `SampleFormat::to_u32()`. mongodb adds the `chapters` collection;
  the existing track documents gain the corresponding nested fields.
- Medium-aggregate domain gates (`audio` / `video` / `subtitle`)
  tighten from `(std OR alloc) AND <medium>` to `std AND <medium>`
  (the new `IndexMap` fields use the std-only default hasher).

Any breaking change on any of the four surfaces (Rust API, proto wire,
sqlx DDL, mongodb document shape) under the pre-1.0 contract bumps `x`
in `0.x.y`. Version `0.1.0` → **`0.2.0`**.

## 0.1.0 — 2026-05-27

Initial public release.

Pre-1.0 contract: any breaking change on any surface (Rust API, proto wire,
sqlx DDL, mongodb document shape) bumps the `x` in `0.x.y`; purely-additive
changes across all surfaces are patches.

### Added

- Single Cargo SemVer covering all four surfaces (see README §Versioning).
- Per-aggregate type-by-type schema review locked across every domain
  cluster (`media r8`, `media_file r8`, `audio r8` + `audio_track r3` +
  `audio_segments r3`, `video r8` + `video_track r6` + `scene r6` +
  `keyframe r15`, `subtitle r3` + `subtitle_track r3` + `subtitle_cues r6`,
  `speaker r1`, `person r3`, `watched_location r5`, `enums r4`, `bitflags r4`,
  `primitives r5`, wire-only r1).
- 13 subtitle cue formats end-to-end (SRT / WebVTT / ASS / LRC + 9 long-tail
  formats: MicroDVD, SubViewer, SBV, TTML, SAMI, VobSub, PGS, CEA-608,
  EBU STL) across domain, proto wire, buffa, sqlx (3 dialects), and mongodb.
- Polymorphic `SubtitleCue<Id, D>` domain type + `SubtitleCueDetails<Id>`
  runtime-tagged union + per-format buffa wire bridges.
- Medium-aggregate feature gates (`video` / `audio` / `subtitle`) — all
  enabled in `default` for backward compatibility; consumers can opt out
  for narrow surfaces (e.g. analysis engines that only emit video
  detections).
- `Identified<Id, D>` transport envelope at the crate root —
  `(id, data)` pair for engine/service crates emitting detection output
  before persistence.
- sqlx backends across 3 dialects (postgres, mysql, sqlite) with owned
  `*Row` + borrowed `*RowRef<'r>` siblings.
- mongodb backend with bson `Document` ⇄ domain bridges + per-collection
  `IndexModel` constructors.
- buffa wire layer with `media.v1` proto messages auto-generated from
  `proto/media/v1/*.proto`.
- Crate-level documentation, per-module `//!` doc comments, README with
  architectural model + feature flag reference, and an end-to-end
  `examples/end_to_end_mongodb.rs` round-trip.
- CI feature-matrix workflow (`cargo hack --each-feature` +
  `--feature-powerset --depth 2`) exercising every meaningful flag combo.
- Validation test coverage sweep — every `try_new` invariant has a
  rejecting test asserting the right error variant.
- Dependency baseline: `sha2 = "0.11"`, `bson = "3"` (mongodb 3.7 via
  the `bson-3` compat feature), `sqlx = "0.9"`.

### Schema decisions locked

- **Content-addressed media**: same hash = same `Media`; no
  Media ↔ WatchedLocation link.
- **WatchedLocation = FS-event monitor** with `is_ejectable`.
- **A-loc = per-track** (audio mirrors the locked video model).
- **Voiceprints + embeddings → LanceDB** keyed by aggregate `id`;
  `phash` dropped from `Keyframe`.
- **Per-track error_status REMOVED** — derived from stage-coded
  `index_errors` + `index_status`; the `MediaErrorFlags` bitflag stays.
- **Descriptor enums + `TrackDisposition` live in `::mediaframe`**
  (extern), not duplicated here.
- **FK column / field naming**: every FK is `<target_type>_id` (no
  `parent`, no bare type names).
- **Validation responsibility boundary**: domain types validate
  intrinsic single-value invariants only; collection composition /
  referential integrity / cross-aggregate coordination = application
  layer.

### Deferred (post-0.1.0 follow-ups)

- **Query / repository layer design**
  ([#58](https://github.com/Findit-AI/mediaschema/issues/58)) —
  current consumers write their own sqlx queries against the `*Row`
  types; an opinionated repository layer can land in 0.2 without
  breaking that.
- **Schema-versioning policy details**
  ([#59](https://github.com/Findit-AI/mediaschema/issues/59)) —
  three explicit decisions outstanding before policy lands in
  `VERSIONING.md`: proto-reservation cutover timing, mongodb
  removed-key grace period, schema-doc-rev formality.
- **Cross-media `Person` / identity layer** — speaker similarity /
  face identity / voice-fingerprint correlation are app-layer
  concerns; the storage shape is already in place via the locked
  `Person` aggregate.
