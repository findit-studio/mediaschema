# `AudioTrack<Id>` — an audio stream  *(rev 1 — drafted for review, NOT self-locked; the big one)*

## Domain meaning

One audio stream of an `Audio` facet (`parent → Audio.id`). A multi-track audio
file holds **N** distinct recordings, so **per-recording music metadata (tags +
cover art) lives here, not on a file/facet** (matches findit-proto's per-track
`AudioStreamMeta`). Holds the codec/stream descriptor, per-track **signal**
analysis (loudness/fingerprint), per-track indexing state, and (open A-loc) the
per-track diarization/transcript segment refs. The audio cluster is a **total
redesign parallel to `Video`** — the stale `AudioAnalysis`/`AudioSummary`/
`TrackRecord` sprawl is discarded, not migrated.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Wall-clock = `jiff` ms; media-time = `mediatime`
(`TrackTime`/`Timebase`). Strings = `SmolStr`; language = `LanguageCode`.
Genuine nested VOs nested (`AudioTags`, `AudioCoverArt`, `Loudness`); no
`*Meta` wrapper. Error details per-track. Conversions deferred.

## Fields

| field | domain type | wire origin | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | `*.id: bytes` | canonical identity |
| `parent` | `Id` | `audio_id` | FK → `Audio.id` |
| `stream_index` | `Option<u32>` | stream idx | source-locator; not identity |
| `container_track_id` | `Option<u64>` | — | keep only if pipeline uses it |
| `codec` | `AudioCodec` (enum) | codec | `Aac`/`Mp3`/`Flac`/`Opus`/`Vorbis`/`Ac3`/`Eac3`/`Dts`/`TrueHd`/`PcmS16Le`/…/`Other(SmolStr)` ([enums.md](enums.md)) |
| `profile` | `Option<SmolStr>` | profile | e.g. `LC`/`HE-AACv2`; small set, keep raw for now |
| `sample_rate` | `u32` | sample_rate | Hz |
| `channels` | `u16` | channels | channel count |
| `channel_layout` | `ChannelLayout` (enum) | channel_layout | `Mono`/`Stereo`/`5_1`/`7_1`/`Other(SmolStr)` |
| `bit_rate` | `u64` | bit_rate | bits/s (0 → unknown) |
| `bits_per_sample` | `Option<u16>` | — | PCM/lossless depth |
| `duration` | `Option<TrackTime>` | time | per-track duration (mediatime extern) |
| `language` | `Option<LanguageCode>` | language | |
| `disposition` | `TrackDisposition` (bitflags!) | disposition `u32` | shared flag set (`DEFAULT`/`COMMENTARY`/`ORIGINAL`/`DUB`/…) |
| `is_primary` · `auto_selected` | `bool` | selection | selection signals |
| `loudness` | `Option<Loudness>` (nested VO) | analysis | EBU R128: `{ integrated_lufs, true_peak_dbtp, loudness_range_lu }` |
| `fingerprint` | `Option<AudioFingerprint>` (nested VO) | chromaprint | `{ algo, value: SmolStr|bytes, duration_s }` (acoustic id / dedup) |
| `tags` | `Option<AudioTags>` (nested VO) | flat tag fields | per-recording music tags (grouped — see VO) |
| `cover_art` | `Option<AudioCoverArt>` (nested VO) | cover_art | per-recording embedded art |
| `segments` | `Vec<Id>` *(open A-loc)* | — | per-track diarization/transcript refs → [audio_segments.md](audio_segments.md) |
| `index_status` | `AudioIndexStatus` (bitflags!) | status `u32` | per-kind stages: analyze/transcribe/diarize/embed |
| `index_errors` | `Vec<ErrorInfo>` | index errors | per-track error truth (rolls up to `Media.error_flags`) |
| `error_status` | `AudioErrorStatus` (bitflags!) | — | error categories |

`ordinal` dropped (derive from `Audio.tracks` order).

## Nested value-objects

- **`AudioTags`** (16 flat wire tag fields grouped — DRY, mirrors "don't flatten
  in domain"): `title`, `artist`, `album_artist`, `album`, `genre`, `composer`,
  `performer`, `date` (SmolStr); `track_number`, `total_tracks`, `disc_number`,
  `total_discs` (u32); `comment`, `lyrics` (SmolStr); `tag_types: Vec<SmolStr>`.
- **`AudioCoverArt`**: `{ path: Option<Location>, mime: SmolStr,
  dimensions: Option<videoframe::Dimensions>, size: u32 }`. (`Dimensions` = VF
  extern, parallel to `VideoTrack`.)
- **`Loudness`**: `{ integrated_lufs: f32, true_peak_dbtp: f32,
  loudness_range_lu: f32 }` (EBU R128). **`AudioFingerprint`**: chromaprint.

## Invariants

`id` non-empty; `sample_rate`/`channels` > 0 when probed; `codec`/
`channel_layout` closed-ish (`Other` escape); `tags`/`cover_art` `None` ⇒ absent.

## Open questions

- **A-loc (the crux — cascade from locked `audio.md`):** the diarization/
  transcript segment aggregate is currently `Audio.segments` (facet, locked).
  Video moved its segmented-ML refs **per-track** (`VideoTrack.scenes`). Cascade
  audio the same way → `AudioTrack.segments` + `Audio.total_segments` rollup
  (consistent with "each track runs the full pipeline" + multi-track files,
  preserves *which track* a transcript came from) **vs** keep facet-level.
  *Lean: per-track (consistency with `Video`; multi-track correctness). This
  reopens locked `audio.md` — your call.*
- **A-agg / A-name:** unified `AudioSegment` (span + speaker + text) vs split
  `Diarization`/`Transcript`; field name `segments` vs `analyses`/`transcript`
  — decided in [audio_segments.md](audio_segments.md).
- **AudioTags grouping:** one `AudioTags` VO (recommended; DB re-flattens for
  query) vs flat fields.
- **`codec`/`channel_layout` enum granularity** vs raw `SmolStr`. *Lean: enum +
  `Other(SmolStr)`.* **`fingerprint` value** `SmolStr` vs `bytes`.
- **PROBED**: is the one-shot probe event per-track here or a container-level
  `Audio` signal? (Global README open.)

## Projection notes

- **sqlx**: `audio_track` table; `id` PK; `parent` FK; `AudioTags`/`Loudness`
  flattened to queryable columns (artist/album/genre/lufs); `cover_art` →
  side-table/blob; `segments` via FK (A-loc target); `index_*` INTEGER + bool.
- **mongodb**: `_id`=UUIDv7; `tags`/`cover_art`/`loudness` embedded; `segments`
  UUID ref array.
- **graphql**: full tag surface (music browsing), loudness, language,
  disposition; transcript/diarization via the segment aggregate;
  `index_errors`/`error_status` exposed.

## Useful information you may have forgotten (proposed — accept/reject individually)

Things a media-indexing/search + music-library domain usually needs that are
*not* in findit-proto / weren't mentioned. Each has a one-line rationale; none
is added to the field table until you say so.

**A/V sync & gapless (often bites later):**
- `start_pts: Option<i64>` (mediatime ticks) — audio rarely starts at 0; needed
  for correct A/V alignment and seeking.
- `encoder_delay` / `priming_samples: Option<u32>` + `trailing_padding` —
  AAC/MP3 priming; required for gapless playback and exact-duration math.

**Codec/quality health (defect detection during indexing):**
- `bit_rate_mode: Option<BitRateMode>` (`Cbr`/`Vbr`/`Abr`) and
  `is_lossless: bool` — drives transcode/quality decisions and search facets.
- `is_silent: bool`, `clipping_detected: bool`, `dc_offset: Option<f32>` —
  cheap health signals computed during the analyze pass; surface bad tracks.

**Loudness/normalization (extend the `Loudness` VO):**
- `sample_peak_dbfs`, `dialnorm: Option<i8>` (AC-3 dialogue normalization),
  `replaygain_track_gain` / `replaygain_album_gain` / `*_peak` — music libraries
  and consistent-loudness playback depend on these.

**Music identity (high value for an audio library):**
- `isrc: Option<SmolStr>` (International Standard Recording Code, from tags),
  `acoustid: Option<SmolStr>` + `musicbrainz_recording_id: Option<SmolStr>` —
  the fingerprint resolves to these; enables canonical dedup & metadata lookup.
- `bpm: Option<f32>`, `musical_key: Option<SmolStr>` — music browsing/search.

**Speech vs music & analysis rollups (drive the pipeline + UX):**
- `content: Option<AudioContentKind>` (`Speech`/`Music`/`Mixed`/`Silence`) and
  `speech_ratio: Option<f32>` — decide whether to transcribe/diarize at all.
- `detected_language: Option<LanguageCode>` (whisper LID at track scope, vs the
  *declared* `language`) + `language_mismatch: bool`.
- `speaker_count: Option<u32>` — distinct-speaker rollup over the diarization
  segments (cheap, very useful as a list/search facet).
- `embedding: Option<Embedding>` — track-level audio embedding for
  similarity/recommendation (same VO as `scene.md`; storage = projection call).

**Provenance (reproducibility — put on index-state, not per segment):**
- `asr_model` / `diarization_model` / `model_version: Option<SmolStr>` — which
  models produced the transcript/diarization; needed to re-run on upgrade.

*Recommended to adopt now:* `start_pts`, `bit_rate_mode`, `is_lossless`,
`is_silent`, `content`+`speech_ratio`, `detected_language`, `speaker_count`,
`isrc`/`acoustid`/`musicbrainz_recording_id`, model-provenance.
*Defer (YAGNI until a consumer needs it):* `bpm`/`musical_key`, `dc_offset`,
`dialnorm`, per-track `embedding`.

**Status: in review (rev 1) — drafted for your one-by-one review. NOT self-locked.**
