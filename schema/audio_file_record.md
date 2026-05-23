# `AudioFileRecord` — SUPERSEDED (dissolved into the audio cluster)

Per the locked **three-level data placement** + **multi-track ⇒ per-track
identity metadata** rules, `AudioFileRecord` is **not** a separate aggregate:

- File-level scalars (`id`, `checksum`, `name`, `format`/`container_format`,
  `size`, overall `duration`, `created_at`) → **`Media`** (the file) +
  **`Audio`** (thin facet). See [media.md](media.md), [audio.md](audio.md).
- Per-recording music metadata (`AudioTags` — the 16 tag fields grouped — and
  `AudioCoverArt`), per-stream codec/rate/channels, loudness/fingerprint →
  **`AudioTrack`** (a multi-track audio file = N distinct recordings, so tags
  are per-track, matching findit-proto's per-track `AudioStreamMeta`). See
  [audio_track.md](audio_track.md).
- Diarization/transcript segmented ML → [audio_segments.md](audio_segments.md).

The stale pre-track audio sprawl (`AudioAnalysis`/`AudioSummary`/`TrackRecord`)
is **discarded, not migrated** (locked: audio is a total redesign parallel to
`Video`). The audit's "fold into one" is moot — there is no `AudioFileRecord`
aggregate to fold.

## Surviving open question (product — was AFR1)

**Standalone music file ↔ `Media`/`Audio` linkage:** a library `.flac`/`.mp3`
with no video — is it `Media(kind=Audio)` + `Audio` facet + one `AudioTrack`
(uniform model; recommended), or a distinct standalone entity with its own
`id`/`checksum` and no `Media`/`Audio` linkage (findit-proto's original shape)?
This is a **product decision**: it changes only FKs/entry-points, not any
aggregate's shape. *Lean: uniform `Media(kind=Audio)` — one model, music
library is just `kind=Audio` queries; avoids a parallel identity world.* Your
call (carried into [audio.md](audio.md)/[media.md](media.md) review).

**Status: SUPERSEDED. The AFR1 product question is the only open item — your call.**
