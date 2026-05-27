# Changelog

All notable changes to mediaschema are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning policy: see the [Versioning section in `README.md`](README.md#versioning)
and the resolution comment on [#59](https://github.com/Findit-AI/mediaschema/issues/59).

## Unreleased

Additive changes accumulated after 0.1.0's publish accumulate here;
once enough land for a patch release the header is renamed to
`## 0.1.1 — YYYY-MM-DD`.

## 0.1.0 — Unreleased

Initial public release. Replace `Unreleased` in this header with the
actual publish date (e.g. `2026-06-15`) at `cargo publish` time.

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
