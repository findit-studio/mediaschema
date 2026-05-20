# `AudioSegment<Id>` — diarization + transcript segment  *(rev 3 — LOCKED, user-approved; `speaker`→`Speaker`)*

## Domain meaning

One analysis segment of an **audio track** — the heavy segmented-ML aggregate,
the audio analog of `Scene`. It is the **reconciled join** of two engine
timelines: `dia` speaker **diarization** (who) ⋈ `asry` word-level **ASR**
(what), as one timeline span. `parent → AudioTrack.id` (**A-loc = per-track**,
your call — mirrors locked `VideoTrack.scenes`; multi-track files keep
which-track attribution). No progress lifecycle (id list + `Audio.total_segments`
rollup, like scenes).

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Media-time = `::mediatime` (`TimeRange`). Strings
= `SmolStr` (`""`=absent, **no `Option`**); free-text = **`LocalizedText`**
shared VO; `language` = **`mediaframe::Language`** (extern — BCP-47 tag;
renamed from `LanguageCode` → `Language`, moved to `mediaframe`, your call). **Embeddings → LanceDB** keyed by this `id` — no embedding
field. **`Provenance` is per-track** (on `AudioTrack`, one per run) — not per
segment. Conversions deferred.

## Fields

| field | domain type | source | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | — | canonical identity |
| `parent` | `Id` | seg→track | FK → `AudioTrack.id` (**A-loc per-track**) |
| `index` | `u32` | ordinal | 0-based segment order within the track |
| `span` | `mediatime::TimeRange` | dia/asry | segment time span (media-time, extern) |
| `speaker` | `Option<Id>` | `dia` | FK → `Speaker` ([speaker.md](speaker.md)); `None` = not diarized. (The raw `dia` `u32` is `Speaker.cluster_id`; voiceprint → LanceDB keyed by `Speaker.id`) |
| `text` | `LocalizedText` | `asry` | `{src, translated}`; `src`=`asry` transcript, `translated`=whisper-translate (**planned `asry` extension** — `""` until emitted) |
| `language` | `Option<mediaframe::Language>` | `asry` | chunk language (`asry::Transcript.language`) |
| `words` | `Vec<Word>` | `asry` | word-level timing+score; **may be empty** (= no word timing for this span; no `Option`) |
| `no_speech_prob` | `Option<f32>` | `asry` | whisper silence prob (replaces the generic per-segment `confidence`) |
| `avg_logprob` | `Option<f32>` | `asry` | whisper mean token logprob |
| `temperature` | `Option<f32>` | `asry` | final decode temperature (retry/quality signal) |

## Nested value-objects

- *(`SpeakerId(u32)` removed — `speaker` is now `Option<Id> → Speaker`; the
  `dia` cluster id lives as `Speaker.cluster_id`. Display `"SPEAKER_NN"` is
  derived from `cluster_id`, not stored.)*
- **`Word`** = `{ text: SmolStr, span: mediatime::TimeRange, score: f32,
  language: Option<mediaframe::Language> }`. `score` ∈ `[0,1]`, **always present**
  (NaN-free; producer-agnostic — whisperX-port alignment now, native-timing
  models later). Per-word `language` carries code-switch/multilingual.

## Invariants

`id` non-empty; `span.start <= span.end`; `(parent, index)` unique; every
`words[i].span` ⊆ `span`; `speaker = None` ⇒ segment not diarized (text-only).

## Resolved (your calls)

- **A-loc:** per-track — `parent → AudioTrack.id`; `AudioTrack.segments` +
  `Audio.total_segments` rollup. **Reopens locked `audio.md` r7 → r8** (handled
  in step 3 of the audio order).
- **A-agg:** **unified** `AudioSegment` (speaker + text + words per span) —
  the reconciled `dia`⋈`asry` join (the pipeline does the join; the domain
  models the result).
- **A-name:** `segments` (`AudioTrack.segments` / `AudioSegment` /
  `Audio.total_segments`).
- **A-spk (rev 3 — superseded: `Speaker` promoted future→now, your call):**
  `speaker: Option<Id>` → **`Speaker`** ([speaker.md](speaker.md)), a per-track
  aggregate; `dia` `u32` = `Speaker.cluster_id`; 256-d voice embedding →
  LanceDB keyed by `Speaker.id`. (Was: inline `SpeakerId(u32)` + future
  registry.)
- **Words:** keep — first-class, roadmapped, producer-agnostic.
- **`text` = `LocalizedText`** (kept; `asry` `translated_text` is a planned
  cross-crate follow-up).
- **Quality:** `no_speech_prob`/`avg_logprob`/`temperature` (the real `asry`
  trio — **not** `compression_ratio`, which `asry` doesn't emit). Generic
  per-segment `confidence` dropped.
- **`SegmentKind`** (Speech/Music/Silence/Noise/Overlap) **deferred** — not in
  `dia`/`asry` output; would come from the separate CED/CLAP stage. Add only
  if/when that stage's real vocabulary is confirmed.

## Projection notes

- **sqlx**: `audio_segment` table; `id` PK; `parent` FK → `audio_track`;
  `span`→`start_pts`/`end_pts`; `speaker_id` INTEGER; `text_src`/
  `text_translated` (derived `display_text` full-text indexed); `words` → side
  table or JSON; quality columns. No vector column (LanceDB).
- **mongodb**: `_id`=UUIDv7; `words` embedded array; text index on display text.
- **graphql**: transcript (`text`/`words`/`speaker`/`span`/`language`) exposed
  for the player; similarity = LanceDB endpoint keyed by `id` (never a field).

**Status: LOCKED (rev 3) — user-approved.** A-loc=per-track
(`parent→AudioTrack.id`); A-agg unified (reconciled `dia`⋈`asry`); A-name
`segments`; **A-spk rev 3: `speaker: Option<Id>` → `Speaker`** ([speaker.md](speaker.md),
promoted future→now — user-authorized reopen of r2); Words first-class; `text`
`LocalizedText` (asry-translate follow-up); quality =
`no_speech_prob`/`avg_logprob`/`temperature`; `language` =
`mediaframe::Language`. *(Order: this✓ → `audio_track.md` → `speaker.md` →
`audio.md` r7→r8.)*
