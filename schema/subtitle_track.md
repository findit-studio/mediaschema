# `SubtitleTrack<Id>` — a subtitle stream  *(rev 3 — LOCKED, user-approved; `error_status` removed)*

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
| `format` | `SubtitleFormat` (enum) | format | text vs bitmap container form — **kept** (your call: do not fold into `codec`) |
| `origin` | `SubtitleTrackOrigin` (enum) | origin | `External` / `Embedded` / `Generated` (cheap-unambiguous redesign, locked) |
| `language` | `Option<LanguageCode>` | language | BCP-47/ISO-639 newtype |
| `title` | `SmolStr` | title | track title/label; `""`=absent (no `Option` — string rule) |
| `image_based` | `Option<bool>` | derived from `codec`/`format` | lossless tri-state classifier: `Some(true)` known bitmap (PGS/DVBSUB/DVDSUB ⇒ OCR required), `Some(false)` known text, `None` unclassifiable — **derived, not stored**. `requires_ocr()` is the conservative `bool` projection (`None` ⇒ `true`) used by completion gating |
| `disposition` | `TrackDisposition` (bitflags!) | disposition `u32` | shared flag set — `FORCED`/`HEARING_IMPAIRED`/`DEFAULT`/… (see [bitflags.md](bitflags.md)) |
| `is_primary` · `auto_selected` | `bool` | selection | selection signals |
| `duration` | `Option<TrackTime>` | time | per-track duration (mediatime extern) |
| `cue_count` | `u32` | — (rollup) | maintained Σ of the cue aggregate's len (no progress lifecycle, like scenes) |
| `cues` | `Vec<Id>` | — | refs to the per-track `SubtitleCue` segment aggregate |
| `provenance` | `Provenance` (shared VO) | — | parse/OCR reproducibility; shared cross-cutting VO ([README.md](README.md)) |
| — *adopted rev 2 (all obtainable via ffmpeg-probe / parse / ingest — your "if we can obtain them" gate met)* — |
| `source_path` | `Option<Location>` | ingest | external `.srt`/`.vtt` *is its own file*; `None` for embedded (structured ⇒ `Option` OK) |
| `source_checksum` | `Option<FileChecksum>` | ingest | change-detection / re-index; `None` for embedded |
| `character_encoding` | `SmolStr` | parse | charset (Win-1252/GBK/…); `""`=absent (ffmpeg `sub_charenc` / detector) |
| `bom_present` | `bool` | parse | BOM sniffed at parse |
| `is_sdh` | `bool` | ffmpeg disp/title | SDH (deaf/HoH) — finer than `disposition.HEARING_IMPAIRED` (best-effort) |
| `is_closed_caption` | `bool` | ffmpeg | CEA-608/708 lifted to a track (codec/disposition) |
| `is_translation` | `bool` | computed | sub `language` ≠ audio `language` (from ffmpeg stream metadata) |
| `kind` | `SubtitleKind` (enum) | ffmpeg disp | `FullDialogue`/`ForcedNarrative`(=`FORCED`)/`CommentaryText`(=`COMMENT`) (best-effort) |
| `coverage_ratio` | `Option<f32>` | cue-parse | subtitled duration ÷ track duration (partial/truncated detection) |
| `is_empty` | `bool` | cue-parse | parsed but zero cues (a defect to surface) |
| `first_cue` · `last_cue` | `Option<TrackTime>` | cue-parse | first/last cue start (mediatime extern) |
| `index_status` | `SubtitleIndexStatus` (bitflags!) | status `u32` | per-kind pipeline stages (bit = stage succeeded) |
| `index_errors` | `Vec<ErrorInfo>` | index errors | per-track error truth (stage-coded `ErrorInfo.code`); → `Media.error_flags` rollup. **Error-state derived from this + `index_status`** — no separate `error_status` field |

`ordinal` dropped (derive from `Subtitle.tracks` order). Per-kind index model:
`SubtitleIndexStatus` + `index_errors` are truth; coarse stage derived per-kind.

## Nested value-objects

None of its own beyond the shared `ErrorInfo`. Cues are a referenced
aggregate (heavy, segmented — same pattern as `VideoTrack.scenes → Scene`),
**not** an inline `Vec<Cue>`.

## Invariants

`id` non-empty; `codec`/`origin` closed-ish enums (`Other(SmolStr)` escape);
`image_based` is a pure function of `codec`/`format` (derived, not stored).

Completion / stage are derived **on the aggregate**: `SubtitleTrack::is_fully_indexed()`
and `SubtitleTrack::index_stage()` call `self.requires_ocr()` internally, so OCR
gating cannot be bypassed by a caller passing a wrong `bool`. These two aggregate
methods are the **only public** completion/stage path. The
`requires_ocr`-parameterised `SubtitleIndexStatus::is_fully_indexed` /
`SubtitleIndexStage::from_status` are crate-private (`pub(crate)`) lower-level
helpers — they are deliberately **not** exported, because an unbound
caller-supplied `requires_ocr` bool would let external code mark an unknown /
image subtitle complete without `OCR_DONE`.

## Resolved (your calls)

- **ST-cues** = referenced per-track aggregate ([subtitle_cues.md](subtitle_cues.md)) + `cue_count` rollup.
- **ST-codec/format** = **keep both** `codec` + `format` (do not fold).
- **`image_based`** = **derived** tri-state from `codec`/`format`, not stored;
  `requires_ocr()` is its conservative `bool` projection for completion gating.
- **`provenance`** = shared `Provenance` VO added.
- Recommended-field set **all adopted** (obtainable via ffmpeg-probe / parse /
  ingest); `Option<SmolStr>` `title` → `SmolStr` (`""`=absent).

## Open questions

- **OCR stage:** image-based subs (PGS/DVBSUB) need an OCR pipeline stage →
  bit in `SubtitleIndexStatus`; confirm the stage vocabulary in [bitflags.md](bitflags.md).
- *Deferred (YAGNI, your split):* `sync_offset_ms`, `frame_rate`
  (frame-based MicroDVD/MPL2 only — if adopted later → `mediaframe::FrameRate`),
  karaoke/positioning flags, `max_chars_per_sec`, ASS styling detail.

## Projection notes

- **sqlx**: `subtitle_track` table; `id` PK; `parent` FK; `cues` via
  `subtitle_cue.subtitle_track_id` FK; `index_*` `INTEGER` + generated bool cols;
  `language`/`codec`/`origin` indexed.
- **mongodb**: `_id`=UUIDv7; `cues` UUID ref array; flags as ints.
- **graphql**: codec/language/origin/disposition exposed; cues + OCR text
  searchable via the cue aggregate; `index_errors` exposed (error-state /
  which-stage derived from it + `index_status`).

## Forgotten-info pass — resolved

**Adopted (rev 2, in the Fields table):** `source_path`+`source_checksum`
(external-file identity — embedded ⇒ `None`), `character_encoding`+
`bom_present` (charset / silent-corruption guard), `is_sdh`,
`is_closed_caption` (CEA-608/708 lifted; relates to
`VideoTrack.has_embedded_captions`), `is_translation`, `kind: SubtitleKind`,
`coverage_ratio`, `is_empty`, `first_cue`/`last_cue`. All obtainable via
ffmpeg-probe / parse / ingest (your gate). OCR model-provenance folded into the
shared `Provenance` VO; `ocr_avg_confidence` stays a per-track quality signal.

**Deferred (YAGNI):** `sync_offset_ms`, `frame_rate` (frame-based
MicroDVD/MPL2 only — `mediaframe::FrameRate` if ever adopted), `is_styled`/
`has_karaoke`/`has_positioning`, `max_chars_per_sec`, `total_chars`/
`total_words`.

**Status: LOCKED (rev 3) — user-approved.** `format` kept (codec+format both);
`is_image_based` derived; shared `provenance: Provenance`; all recommended
fields adopted (ffmpeg-probe/parse/ingest-obtainable); `title: SmolStr`
(`""`=absent). Per-track `cues` aggregate ([subtitle_cues.md](subtitle_cues.md)).
The OCR-stage bit in `SubtitleIndexStatus` is settled in
[bitflags.md](bitflags.md) (a bit value, not a `SubtitleTrack`-shape change).
*(rev 3: per-track `error_status` removed — error-state derived from
`index_errors` (stage-coded) + `index_status`; cross-cutting cleanup,
user-authorized reopen of locked r2.)*
