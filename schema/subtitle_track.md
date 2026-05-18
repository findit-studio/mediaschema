# `SubtitleTrack<Id>` — a subtitle stream  *(rev 1 — drafted for review, NOT self-locked)*

## Domain meaning

One subtitle stream of a `Subtitle` facet (`parent → Subtitle.id`). An external
`.srt`/`.vtt` is **one** `SubtitleTrack`; embedded subtitles are **N**. Holds
the per-track stream/codec descriptor, language/role/origin, the parsed-cue
aggregate refs, and per-track indexing state. Its own schema (no shared track
core — subtitle pipeline = parse-cues / OCR / search-index, distinct per kind).

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Wall-clock = `jiff` ms; media-time = `mediatime`.
Strings = `SmolStr`; language = `LanguageCode`. Error *details* per-track only.
Conversions deferred.

## Fields

| field | domain type | wire origin | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | `*.id: bytes` | canonical identity |
| `parent` | `Id` | `subtitle_id` | FK → `Subtitle.id` |
| `stream_index` | `Option<u32>` | stream idx | source-locator (ffmpeg/WebCodecs); not identity; `None` for external file |
| `container_track_id` | `Option<u64>` | — | keep only if pipeline uses it |
| `codec` | `SubtitleCodec` (enum) | codec | `Srt`/`Ass`/`WebVtt`/`MovText`/`DvbSub`/`Pgs`/`DvdSub`/`Other(SmolStr)` (see [enums.md](enums.md)) |
| `format` | `SubtitleFormat` (enum) | format | text vs bitmap container form; may fold into `codec` (open ST-codec) |
| `origin` | `SubtitleTrackOrigin` (enum) | origin | `External` / `Embedded` / `Generated` (cheap-unambiguous redesign, locked) |
| `language` | `Option<LanguageCode>` | language | BCP-47/ISO-639 newtype |
| `title` | `Option<SmolStr>` | title | track title/label |
| `is_image_based` | `bool` | derived from `codec` | PGS/DVBSUB/DVDSUB ⇒ OCR stage required |
| `disposition` | `TrackDisposition` (bitflags!) | disposition `u32` | shared flag set — `FORCED`/`HEARING_IMPAIRED`/`DEFAULT`/… (see [bitflags.md](bitflags.md)) |
| `is_primary` · `auto_selected` | `bool` | selection | selection signals |
| `duration` | `Option<TrackTime>` | time | per-track duration (mediatime extern) |
| `cue_count` | `u32` | — (rollup) | maintained Σ of the cue aggregate's len (no progress lifecycle, like scenes) |
| `cues` | `Vec<Id>` | — | refs to the per-track `SubtitleCue` segment aggregate (see open ST-cues) |
| `index_status` | `SubtitleIndexStatus` (bitflags!) | status `u32` | per-kind pipeline stages |
| `index_errors` | `Vec<ErrorInfo>` | index errors | per-track error truth (rolls up to `Media.error_flags`) |
| `error_status` | `SubtitleErrorStatus` (bitflags!) | — | error categories |

`ordinal` dropped (derive from `Subtitle.tracks` order). Per-kind index model:
`SubtitleIndexStatus` + `index_errors` are truth; coarse stage derived per-kind.

## Nested value-objects

None of its own beyond the shared `ErrorInfo`. Cues are a referenced
aggregate (heavy, segmented — same pattern as `VideoTrack.scenes → Scene`),
**not** an inline `Vec<Cue>`.

## Invariants

`id` non-empty; `codec`/`origin` closed-ish enums (`Other(SmolStr)` escape);
`is_image_based` is a pure function of `codec` (store derived or compute — open).

## Open questions

- **ST-cues:** model parsed cues as a referenced per-track aggregate
  `SubtitleCue {index, start, end, text|image_ref, ocr_text}` + `cue_count`
  rollup (recommended — parallels `Scene`, keeps the track row light, OCR/search
  attach per cue) **vs** inline `Vec<Cue>` (simpler, but unbounded on the row).
  *Lean: referenced aggregate.* If yes it gets its own `subtitle_cues.md`.
- **ST-codec/format:** one `SubtitleCodec` enum vs split `codec`+`format`
  (text/bitmap). *Lean: single `SubtitleCodec` with an `is_image_based()` helper;
  drop `format`.*
- **OCR stage:** image-based subs (PGS/DVBSUB) need an OCR pipeline stage →
  bit in `SubtitleIndexStatus`; confirm the stage vocabulary in [bitflags.md](bitflags.md).
- **`is_image_based`** stored vs derived (recommend derived; no column).

## Projection notes

- **sqlx**: `subtitle_track` table; `id` PK; `parent` FK; `cues` via
  `subtitle_cue.subtitle_track_id` FK; `index_*` `INTEGER` + generated bool cols;
  `language`/`codec`/`origin` indexed.
- **mongodb**: `_id`=UUIDv7; `cues` UUID ref array; flags as ints.
- **graphql**: codec/language/origin/disposition exposed; cues + OCR text
  searchable via the cue aggregate; `index_errors`/`error_status` exposed.

## Useful information you may have forgotten (proposed — accept/reject individually)

Subtitle-domain essentials not in findit-proto / unmentioned. Rationale each;
none added to the field table until you approve.

**External-file identity (the big gap):** an `origin = External` `.srt`/`.vtt`
**is its own file** — it needs `source_path: Option<Location>` and
`source_checksum: Option<FileChecksum>` (change-detection / re-index), the
subtitle analog of `Media`'s file identity. Embedded tracks leave these `None`.
*Strongly recommend adopting.*

**Text decoding (silent corruption if missed):**
- `character_encoding: Option<SmolStr>` — legacy `.srt` is frequently
  Windows-1252/Latin-1/GBK, not UTF-8; wrong charset ⇒ garbled cues & broken
  search. `bom_present: bool`. *Recommend adopting.*

**Timing correction (subtitles are routinely out of sync):**
- `sync_offset_ms: Option<i32>` — global delay correction applied/needed.
- `frame_rate: Option<Rational>` — required to convert frame-based formats
  (MicroDVD/MPL2) to time; mismatched fps is a classic desync cause.

**Accessibility / selection flags (distinct from generic disposition):**
- `is_sdh: bool` (Subtitles for Deaf/Hard-of-hearing — includes sound
  descriptions; a real selection/search facet, not the same as
  `disposition.HEARING_IMPAIRED`’s coarse bit).
- `is_closed_caption: bool` + note the relationship to
  `VideoTrack.has_embedded_captions` (CEA-608/708 lifted to a `SubtitleTrack`).
- `kind: SubtitleKind` (`FullDialogue`/`ForcedNarrative`/`CommentaryText`) and
  `is_translation: bool` vs transcription (subtitle in a *different* language
  than the audio) — drives default-track selection.

**Cue/quality stats (QC + accessibility + “is this complete?”):**
- `cue_count` (have), `total_chars`/`total_words: u32`,
  `coverage_ratio: Option<f32>` (subtitled duration ÷ track duration — detects
  partial/truncated subs), `first_cue`/`last_cue: Option<TrackTime>`,
  `max_chars_per_sec: Option<f32>` (reading-speed/accessibility QC),
  `is_empty: bool` (parsed but zero cues — a defect to surface).

**Styling/format detail (ASS/SSA):** `is_styled: bool`, `has_karaoke: bool`,
`has_positioning: bool` — affects render fidelity and plain-text extraction.

**OCR provenance (image-based subs):** `ocr_engine`/`ocr_language:
Option<SmolStr>`, `ocr_avg_confidence: Option<f32>` — quality + re-OCR on
upgrade. (Lives with the OCR pipeline stage.)

*Recommended to adopt now:* `source_path`+`source_checksum`,
`character_encoding`, `is_sdh`, `is_closed_caption`, `is_translation`,
`coverage_ratio`, `is_empty`, `first/last_cue`. *Defer:* `sync_offset_ms`,
karaoke/positioning flags, `max_chars_per_sec`.

**Status: in review (rev 1) — drafted for your one-by-one review. NOT self-locked.**
