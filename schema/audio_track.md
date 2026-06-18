# `AudioTrack<Id>` — an audio stream  *(rev 7 — LOCKED, user-approved; +`sound_events` refs (CED — `AudioSegment` parity))*

## Domain meaning

One audio stream of an `Audio` facet (``audio_id` → Audio.id`). A multi-track audio
file holds **N** distinct recordings, so **per-recording music metadata (tags +
cover art) lives here, not on a file/facet**. Holds the codec/stream
descriptor, per-track **signal** analysis (loudness/fingerprint), per-track
indexing state + provenance, and (**A-loc = per-track**, resolved) the
per-track diarization/transcript **`segments`** refs → [audio_segments.md](audio_segments.md) and CED **`sound_events`** refs → [sound_events.md](sound_events.md).
Total redesign parallel to `Video`/`VideoTrack` — the stale
`AudioAnalysis`/`AudioSummary`/`TrackRecord` sprawl is discarded.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Wall-clock = `jiff` ms; media-time = `::mediatime`
(`mediatime::Timestamp`). **`SmolStr` `""`=absent, never `Option`**;
`language` = **`::mediaframe::Language`**; descriptor enums
(`AudioCodec`/`ChannelLayout`/`TrackDisposition`/`BitRateMode`) =
**`::mediaframe`** externs. Genuine nested VOs nested (`AudioTags`,
`AudioCoverArt`, `Loudness`, `AudioFingerprint`). **No `error_status`** —
error-state derived from stage-coded `index_errors` + `index_status`.
**Embeddings → LanceDB** (no field; the `*_EMBED` status bits track that they
ran). `Provenance` = shared cross-cutting VO ([README.md](README.md)), per
track. Conversions deferred.

## Fields

| field | domain type | wire/source | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | `*.id: bytes` | canonical identity |
| `audio_id` | `Id` | `audio_id` | FK → `Audio.id` |
| `stream_index` | `Option<u32>` | stream idx | source-locator; not identity |
| `container_track_id` | `Option<u64>` | — | keep iff the pipeline uses it |
| `codec` | `mediaframe::AudioCodec` | codec | extern; `Aac`/`Mp3`/`Flac`/`Opus`/…/`Other(SmolStr)` |
| `profile` | `SmolStr` | profile | e.g. `LC`/`HE-AACv2`; `""`=absent (no `Option`) |
| `sample_rate` | `u32` | sample_rate | Hz |
| `channels` | `u16` | channels | channel count |
| `channel_layout` | `mediaframe::ChannelLayout` | channel_layout | extern; `Mono`/`Stereo`/`5_1`/`7_1`/`Other` |
| `sample_format` | `mediaframe::audio::SampleFormat` | sample_format | extern; FFmpeg `AV_SAMPLE_FMT_*`-coded enum (`U8`/`S16`/`S32`/`Flt`/`Dbl` + planar siblings + `Unknown(u32)` / `Other(SmolStr)` escapes). Default = `Unknown(u32::MAX)` (the `AV_SAMPLE_FMT_NONE` sentinel). SQL projection: integer column carrying `SampleFormat::to_u32()` |
| `bit_rate` | `u64` | bit_rate | bits/s (0 → unknown) |
| `bit_rate_mode` | `Option<mediaframe::BitRateMode>` | analyze | `Cbr`/`Vbr`/`Abr` (extern; adopted) |
| `bits_per_sample` | `Option<u16>` | — | PCM/lossless depth |
| `is_lossless` | `bool` | derived/codec | transcode/quality + search facet |
| `duration` | `Option<mediatime::Timestamp>` | time | per-track duration (extern) |
| `start_pts` | `Option<mediatime::Timestamp>` | — | audio rarely starts at 0 — A/V sync/seek (adopted) |
| `language` | `Option<mediaframe::Language>` | language | declared BCP-47 tag (extern) |
| `detected_language` | `Option<mediaframe::Language>` | whisper LID | track-scope detected (vs declared) |
| `language_mismatch` | `bool` | derived | `detected ≠ declared` |
| `disposition` | `mediaframe::TrackDisposition` (bitflags) | disposition | extern shared flag set |
| `is_primary` · `auto_selected` | `bool` | selection | selection signals |
| `content` | `Option<AudioContentKind>` | analyze | `Speech`/`Music`/`Mixed`/`Silence` (mediaschema enum) — drives transcribe/diarize |
| `speech_ratio` | `Option<f32>` | analyze | fraction speech (drives the pipeline) |
| `is_silent` | `bool` | analyze | cheap defect signal |
| `loudness` | `Option<Loudness>` (VO) | EBU R128 | `{ integrated_lufs, true_peak_dbtp, loudness_range_lu }` |
| `replay_gain` | `Option<ReplayGain>` (VO) | tags | `{ track_gain_db, track_peak, album_gain_db?, album_peak? }` — container's normalization recommendation; distinct from EBU R128 `loudness` (the measurement) |
| `fingerprint` | `Option<AudioFingerprint>` (VO) | chromaprint | acoustic id / dedup |
| `isrc` | `SmolStr` | tags | recording code; `""`=absent |
| `acoustid` · `musicbrainz_recording_id` | `SmolStr` · `SmolStr` | resolve | canonical dedup ids; `""`=absent |
| `speakers` | `Vec<Id>` | `dia` | → `Speaker` ([speaker.md](speaker.md)) — the track's diarized speaker set; voiceprint → LanceDB. Distinct-count = `speakers.len()` (no separate `speaker_count`) |
| `tags` | `Option<AudioTags>` (VO) | flat tag fields | per-recording music tags (grouped) |
| `cover_art` | `Option<AudioCoverArt>` (VO) | cover_art | per-recording embedded art (inline) |
| `segments` | `Vec<Id>` | — | → `AudioSegment` (**A-loc per-track**); `Audio.total_segments` rolls these up |
| `sound_events` | `Vec<Id>` | — | → `SoundEvent` (**A-loc per-track**, CED); `Audio.total_sound_events` rolls these up |
| `metadata` | `IndexMap<SmolStr, SmolStr>` | — | container `AVDictionary` entries from this stream, with the keys hoisted into `Tags` and `language` already consumed. Insertion-ordered. SQL projection: `audio_track_metadata` join table with `(audio_track_id, ordinal, key, value)` |
| `provenance` | `Provenance` (shared VO) | — | analysis-run reproducibility |
| `vad_provenance` | `Provenance` (shared VO) | — | provenance of the VAD (voice-activity) model that produced this track's `SpeechSegment`s; distinct from the general analysis `provenance` |
| `index_status` | `AudioIndexStatus` (bitflags `u32`) | status | the **verified 11-bit `ProcessingStage`** (below) |
| `index_errors` | `Vec<ErrorInfo>` | index errors | per-track truth (stage-coded `ErrorInfo.code`); → `Media.error_flags` rollup; error-state **derived** (no `error_status`) |

`ordinal` dropped (derive from `Audio.tracks` order).

## `AudioIndexStatus` — verified vs `findit-proto::database::audio` `ProcessingStage`

`EXTRACTED 0x01` · `CLASSIFIED 0x02` · `VAD_DONE 0x04` · `STT_DONE 0x08`
(asry transcript) · `SPEAKER_DONE 0x10` (dia diarization) · `LLM_DONE 0x20` ·
`TEXT_EMBED 0x40` · `CED_DONE 0x80` · `CLAP_DONE 0x100` · `EBUR128_DONE 0x200`
(loudness) · `FPRINT_DONE 0x400` (chromaprint). `AudioIndexStage`
([enums.md](enums.md)) is **derived** from these + `index_errors`
(`Failed` iff `index_errors` non-empty).

## Nested value-objects

- **`AudioTags`** (16 flat wire tag fields grouped — DRY): `title`, `artist`,
  `album_artist`, `album`, `genre`, `composer`, `performer`, `date` (SmolStr);
  `track_number`, `total_tracks`, `disc_number`, `total_discs` (u32);
  `comment`, `lyrics` (SmolStr); `tag_types: Vec<SmolStr>`.
- **`AudioCoverArt`** = `{ data: Bytes (inline — mirrors locked
  `Keyframe.data`; no `Location`), mime: SmolStr (`""`=absent),
  dimensions: Option<mediaframe::Dimensions> }`. `size` dropped (= `data.len()`).
- **`Loudness`** = `{ integrated_lufs: f32, true_peak_dbtp: f32,
  loudness_range_lu: f32 }` (EBU R128).
- **`ReplayGain`** = `{ track_gain_db: f32, track_peak: f32,
  album_gain_db: Option<f32>, album_peak: Option<f32> }`. Container-tagged
  normalization recommendation (`REPLAYGAIN_*` tags) — distinct from
  `Loudness` (the measurement). Track scalars required; album scalars
  independently optional (single tracks lack album context).
- **`AudioFingerprint`** = `{ algo: SmolStr, value: Bytes }` (chromaprint
  acoustic hash for recording dedup / AcoustID lookup; `value` = `Bytes`.
  `duration_s` **dropped** — AcoustID lookup uses `AudioTrack.duration`; a
  whole-track fingerprint makes a separate duration redundant).

## Invariants

`id` non-empty; `sample_rate`/`channels` > 0 when probed; `codec`/
`channel_layout` closed-ish (`mediaframe` `Other` escape); `tags`/`cover_art`
`None` ⇒ absent; `speakers` = the track's `Speaker` set (distinct count =
`speakers.len()`); each `AudioSegment.speaker` ∈ `speakers`.

## Resolved (your calls)

- **A-loc:** per-track — `segments: Vec<Id> → AudioSegment`; `Audio` keeps the
  `total_segments` rollup (step 3, reopens `audio.md` r7→r8).
- **Cascades applied:** descriptor enums + `Language` → `::mediaframe`;
  `AudioIndexStatus` = the verified 11-bit `ProcessingStage`; no `error_status`
  (derived); `provenance: Provenance`; `AudioCoverArt` inline `data: Bytes`;
  `profile: SmolStr` (no `Option`); embeddings → LanceDB.
- **Adopted** (ffmpeg/pipeline-obtainable): `start_pts`, `bit_rate_mode`,
  `is_lossless`, `is_silent`, `content`+`speech_ratio`, `detected_language`+
  `language_mismatch`, `isrc`/`acoustid`/`musicbrainz_recording_id`,
  `provenance`. **Deferred (YAGNI):** `bpm`/`musical_key`, `dc_offset`,
  `dialnorm`, replaygain, per-track `embedding` (→ LanceDB), `sample_peak_dbfs`.
- **Speakers (your call — `Speaker` promoted future→now):** `speakers: Vec<Id>
  → Speaker` ([speaker.md](speaker.md)); **`speaker_count` dropped** (=
  `speakers.len()`). Voiceprint = LanceDB keyed by `Speaker.id` (not inline;
  it's a similarity vector, unlike the chromaprint hash).
- `AudioTags` grouped VO (DB re-flattens for query); `AudioFingerprint =
  {algo, value:Bytes}` (`duration_s` dropped — redundant with
  `AudioTrack.duration`).

## Open questions

- *(none — cross-cutting + adopt/defer all resolved; ready for your lock.)*

## Projection notes

- **sqlx**: `audio_track` table; `id` PK; `audio_id` FK; `AudioTags`/`Loudness`
  flattened to queryable columns (artist/album/genre/lufs); `cover_art.data` →
  `BYTEA`/object-store; `segments` via `audio_segment.audio_track_id` FK;
  `index_status` INTEGER + generated per-bit bools; `Language` flattens to a
  BCP-47 text column. No vector column (LanceDB).
- **mongodb**: `_id`=UUIDv7; `tags`/`cover_art`/`loudness`/`fingerprint`
  embedded; `segments` UUID ref array.

**Status: LOCKED (rev 3) — user-approved.** A-loc=per-track
(`segments: Vec<Id>→AudioSegment`); cascades = descriptor enums + `Language`
→ `::mediaframe`; 11-bit `AudioIndexStatus` (verified `ProcessingStage`); no
`error_status` (derived); `provenance: Provenance`; `AudioCoverArt` inline;
no-`Option`; embeddings→LanceDB. `speakers: Vec<Id>→Speaker`
([speaker.md](speaker.md)). Adopted: start_pts/bit_rate_mode/is_lossless/
is_silent/content+speech_ratio/detected_language+language_mismatch/isrc/
acoustid/musicbrainz/provenance. `AudioFingerprint={algo,value}`.
*(Cascade: `audio.md` r7→r8 = final step.)*
