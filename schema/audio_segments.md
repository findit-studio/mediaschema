# `AudioSegment<Id>` — diarization + transcript segment  *(rev 1 — drafted for review, NOT self-locked)*

## Domain meaning

One analysis segment of an audio stream — the **heavy segmented-ML aggregate**
that is the audio analog of `Scene`: pyannote **speaker diarization** + whisper
**transcript**, as a timeline span. Referenced by `Audio.segments` (facet —
locked in `audio.md`) **or** per-`AudioTrack.segments` (open **A-loc**, see
[audio_track.md](audio_track.md)). No progress lifecycle (id list + count, like
scenes).

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Media-time = `mediatime` (extern). Strings =
`SmolStr`; language = `LanguageCode`. Conversions deferred.

## Fields (proposed — unified segment; see A-agg)

| field | domain type | wire origin | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | `*.id: bytes` | canonical identity |
| `parent` | `Id` | seg→(audio\|audio_track) | FK target depends on **A-loc** |
| `index` | `u32` | ordinal | 0-based segment order |
| `span` | `mediatime::TimeRange` | start/end | segment time span (media-time, extern) |
| `speaker` | `Option<SpeakerLabel>` | diarization | `SmolStr` newtype (`SPEAKER_00`, …); registry open A-spk |
| `text` | `Option<SmolStr>` | transcript | whisper transcript for the span |
| `language` | `Option<LanguageCode>` | detected lang | whisper language id |
| `words` | `Vec<Word>` (nested VO) | word ts | word-level timestamps (optional, may be empty) |
| `confidence` | `Option<f32>` | score | ASR/diarization confidence |
| `embedding` | `Option<Embedding>` (nested VO) | vector | optional text/segment embedding (see `scene.md` SC-embed) |

## Nested value-objects

- **`Word`**: `{ text: SmolStr, span: mediatime::TimeRange,
  confidence: Option<f32> }`.
- **`Embedding`**: same shape as `scene.md` (`model`, `dim`, `vector`) — reuse.

## Invariants

`id` non-empty; `span.start <= span.end`; `index` unique within `parent`;
`words` (if any) lie within `span`.

## Open questions

- **A-loc** (the crux, cascades from locked `audio.md`): parent = `Audio` facet
  vs `AudioTrack`. *Lean: per-track (multi-track files; consistency with
  `Video.scenes` moving to `VideoTrack`).* Decides the FK only, not the shape.
- **A-agg:** **unified** `AudioSegment` carrying *both* speaker + text per span
  (recommended — diarization and ASR are reconciled into one timeline; simplest
  to query/display) **vs** separate `Diarization`/`Transcript` aggregates (truer
  to the two models, but needs span-join). *Lean: unified.*
- **A-name:** `segments` (recommended) vs `analyses` vs `transcript`.
- **A-spk:** inline `speaker: SmolStr` label vs a `Speaker` registry aggregate
  (per-media speaker identity, embeddings, names). *Lean: inline label now;
  `Speaker` aggregate is a later enhancement (YAGNI) — flag as future.*
- **Words:** keep word-level timestamps (recommended for search/karaoke) vs
  segment-only. Storage cost is a projection concern.

## Projection notes

- **sqlx**: `audio_segment` table; `id` PK; `parent` FK (A-loc target);
  `span`→ `start_pts`/`end_pts`; `text` full-text indexed; `words` → side table
  or JSON; `embedding` like `scene`.
- **mongodb**: `_id`=UUIDv7; `words` embedded array; `text` text index.
- **graphql**: transcript (`text`/`words`/`speaker`/`span`) exposed for the
  player; embedding not raw.

## Useful information you may have forgotten (proposed — accept/reject individually)

Diarization/ASR essentials not in findit-proto / unmentioned.

**Segment type (diarization labels more than speech):**
- `kind: SegmentKind` (`Speech`/`Music`/`Silence`/`Noise`/`Overlap`) — pyannote
  emits non-speech regions; without this you can't tell silence from untranscribed
  speech. *Recommend adopting.*
- `is_overlap: bool` — overlapping speakers (degrades ASR; flag for UX/QC).

**Whisper quality signals (hallucination filtering — high value, cheap):**
- `no_speech_prob: Option<f32>`, `avg_logprob: Option<f32>`,
  `compression_ratio: Option<f32>` — the standard whisper trio to drop
  hallucinated/low-confidence text from search and display. *Recommend adopting.*

**Transcribe vs translate:**
- `translated_text: Option<SmolStr>` — whisper's `translate` task output
  (usually English) alongside original-language `text`; enables cross-language
  search. Pairs with per-segment `language`.

**Normalization & provenance:**
- `is_punctuated: bool` (raw vs punctuated/cased text — affects display & search
  tokenization).
- Model provenance (`asr_model`/`diarization_model`/`model_version`) belongs on
  the **track index-state**, not per segment (one value for the run).

**Speaker identity (rollup + future):**
- `speaker_count` is a **parent** rollup (`AudioTrack`/`Audio`), not per segment.
- A `Speaker` registry aggregate (per-media speaker id, voice embedding,
  human-assigned name, gender) is a real future enhancement — keep `speaker:
  SmolStr` label inline now (YAGNI), flag `Speaker` as a later aggregate.

*Recommended to adopt now:* `kind`, `no_speech_prob`/`avg_logprob`/
`compression_ratio`, `translated_text`, parent `speaker_count`.
*Defer:* `is_overlap`, `is_punctuated`, the `Speaker` registry aggregate.

**Status: in review (rev 1) — drafted for your one-by-one review. NOT self-locked.**
