# `TrackCore` — SUPERSEDED (no shared track type)

Per user direction: the three tracks are **genuinely different**, and the
**index/stage model is per-kind** (each kind's pipeline differs:
video = scene/keyframe/VLM/embed · audio = analyze/transcribe/diarize/embed ·
subtitle = cues/OCR/search). There is **no shared `TrackCore` type and no single
`IndexStage` enum**. Each track is its own schema:

- [video_track.md](video_track.md)
- [audio_track.md](audio_track.md)
- [subtitle_track.md](subtitle_track.md)

For reference only (recur **by convention**, NOT a shared type): `id`(UUIDv7) ·
`parent`(→ owning facet) · `ordinal` · `stream_index` · `container_track_id` ·
`is_primary` · `auto_selected` · `selection_reason` · `index_errors:
Vec<ErrorInfo>` (per-track error truth `Media.error_flags` rolls up).

Everything else — stream/codec, role, the **per-kind index/stage model** (the
kind's `*IndexStatus` bitflags + derived per-kind status), and the
signal-analysis payload — is designed independently in each track doc.
