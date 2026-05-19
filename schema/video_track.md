# `VideoTrack<Id>` — a video stream  *(rev 6 — LOCKED, user-approved; `error_status` removed)*

## Domain meaning

One video stream of a `Video` facet (`parent → Video.id`). Holds stream/codec
descriptors, the frame/pixel/colour vocabulary (`::mediaframe` extern), the
per-stream `Scene` refs, and per-track indexing state. Its own schema.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). `mediatime` media-time; **`mediaframe` extern** for
pixel/colour/frame vocabulary. Source-locator = ffmpeg `stream_index` /
WebCodecs only. Conversions deferred.

## Fields (owner: **VF** = in `::mediaframe` 0.3.0 · **VF\*** = mediaframe-bound, pending the batched [mediaframe-candidates.md](mediaframe-candidates.md) PR · **MS** = mediaschema)

| field | domain type | owner | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | MS | canonical identity |
| `parent` | `Id` | MS | FK → `Video.id` |
| `stream_index` | `Option<u32>` | MS | source-locator (ffmpeg/WebCodecs); not identity |
| `container_track_id` | `Option<u64>` | MS | keep only if pipeline uses it |
| `start_pts` | `Option<mediatime::Timestamp>` | MS *(mediatime)* | stream start offset / first PTS — **mediatime-represented** (a pts @ timebase), not a raw `i64` |
| `duration` | `Option<TrackTime>` | MS *(mediatime)* | per-track duration |
| `codec` | `VideoCodec` (enum) | MS | `VideoCodec` + `Other(SmolStr)` escape ([enums.md](enums.md)) |
| `profile` · `level` | `Option<SmolStr>` · `Option<u16>` | MS | **separate** from codec (your call) |
| `bit_rate` | `u64` | MS | per-track bitrate (0 → unknown) |
| `nb_frames` | `Option<u64>` | MS | frame count (exact-duration / progress / VFR) |
| `has_b_frames` | `bool` | MS | bitstream has B-frames (seek/cut behaviour) |
| `closed_gop` | `Option<bool>` | MS | closed-GOP (seek/cut behaviour) |
| `bits_per_raw_sample` | `Option<u8>` | MS | coded sample depth (may differ from pixfmt) |
| `dimensions` | `mediaframe::Dimensions` | **VF** | coded W×H (dedup mediaschema's own) |
| `visible_rect` | `Option<mediaframe::Rect>` | **VF** | clean-aperture / crop |
| `sample_aspect_ratio` | `mediaframe::SampleAspectRatio` | **VF** | display aspect / anamorphic |
| `pixel_format` | `mediaframe::PixelFormat` | **VF** | FFmpeg pixfmt (bit-depth encoded here) |
| `color` | `mediaframe::ColorInfo` | **VF** | primaries/transfer/matrix/range/chroma |
| `hdr_static` | `Option<mediaframe::HdrStaticMetadata>` | **VF** | MaxCLL/MaxFALL + mastering display (real ffmpeg side-data) |
| `rotation` | `mediaframe::Rotation` | **VF** | display rotation |
| `frame_rate` | `mediaframe::FrameRate` (num/den + `is_vfr`) | **VF\*** | exact ratio (replaces `f64`); VFR-aware. **NOT `mediatime::Timebase`** — see mediatime note |
| `field_order` | `mediaframe::FieldOrder` (enum) | **VF\*** | progressive / tff / bff (interlace) |
| `stereo_mode` | `Option<mediaframe::StereoMode>` | **VF\*** | 3D/stereoscopic packing |
| `dovi` | `Option<mediaframe::DolbyVisionConfig>` | **VF\*** | Dolby Vision (profile/level/rpu/el/bl-compat); **≠ HDR10 static** |
| `has_embedded_captions` | `bool` | MS | CEA-608/708 present (bitstream-detected; not a disposition bit) |
| `disposition` | `TrackDisposition` (bitflags!) | MS | shared flag set ([bitflags.md](bitflags.md)) |
| `is_primary` · `auto_selected` | `bool` | MS | selection signals |
| `scenes` | `Vec<Id>` | MS | refs → [scene.md](scene.md) (per-stream detection) |
| `index_status` | `VideoIndexStatus` (bitflags!) | MS | per-kind pipeline stages (bit = stage succeeded) |
| `index_errors` | `Vec<ErrorInfo>` | MS | per-track error truth (stage-coded `ErrorInfo.code`); → `Media.error_flags` rollup. **Error-state is derived from this + `index_status`** — no separate `error_status` field |

`ordinal` / `selection_reason` dropped. **`is_attached_pic` is NOT a stored
field** — it is the `disposition` `ATTACHED_PIC` bit; expose it as a derived
helper (`disposition.is_attached_pic()`), don't duplicate state. The indexer
uses it to skip album-art pseudo-streams.

## mediatime mapping (your question — "any of them can be represented by mediatime's types?")

- **`start_pts` → `mediatime::Timestamp`** — yes; it is a presentation
  timestamp at the track timebase. Modelled as `mediatime::Timestamp`, not a
  raw `i64` (reuses the locked `mediatime` extern).
- **`duration` → `TrackTime`** — yes; already a `mediatime` alias.
- **`frame_rate` is a rational but NOT `mediatime::Timebase`.** `mediatime`'s
  own docs state frame-rate and PTS-timebase are *conceptually different* (a
  30 fps stream typically has PTS timebase `1/30000` and frame rate `30/1`).
  Reusing `Timebase` here would conflate two distinct media-time concepts —
  the same discipline as the Codex-F2 review. It is a separate
  `mediaframe::FrameRate` (generic video vocab → mediaframe; see candidates).
- Everything else (`nb_frames`, `has_b_frames`, codecs, dimensions, dovi,
  flags, index state, …) is **not** media-time — no `mediatime` type applies.

## Resolved (rev 4 → 5)

- **All "forgotten" fields adopted** (`start_pts`, `nb_frames`, `has_b_frames`,
  `closed_gop`, `bits_per_raw_sample`, `stereo_mode`, `field_order`); plus
  `dovi`. `is_attached_pic` folded into `disposition` (derived, not stored).
- **VT-codec:** `VideoCodec` enum + `Other(SmolStr)`; `profile`/`level`
  separate fields. **Locked by you.**
- **VT-scenes:** `Video.total_scenes` = Σ over its `VideoTrack`s of
  `scenes.len()`. **Confirmed by you.**
- **Dolby Vision:** `dovi: Option<mediaframe::DolbyVisionConfig>` added.
- **mediaframe candidates (VF\*):** `FrameRate`, `FieldOrder`, `StereoMode`,
  `DolbyVisionConfig` are generic frame/colour vocab → belong in `mediaframe`
  per the boundary rule; tracked in [mediaframe-candidates.md](mediaframe-candidates.md)
  for the batched follow-up mediaframe PR (not piecemeal). Until it publishes,
  these rows are mediaframe-bound-pending.

## Open questions

None remaining for the field set — pending your rev-5 approval. (The only
external dependency is the **VF\*** batched mediaframe PR, sequenced after the
schema review.)

## Projection notes

- **sqlx**: `video_track` table; `id` PK; `parent` FK; VF/VF\* types flattened
  (`dimensions`→`width`/`height`, `color`→5 enum cols, `pixel_format`→`u32`,
  `frame_rate`→`num`/`den`/`is_vfr`, `dovi`→its scalar cols, `hdr_static`→its
  cols); `start_pts`/`duration`→pts(+timebase); `index_*` `INTEGER` + gen bool.
- **mongodb**: `_id`=UUIDv7; embedded VF/VF\* subdocs.
- **graphql**: pixel/colour/dims/hdr/dovi exposed; derived stage;
  `index_errors` exposed (error-state/which-stage derived from it +
  `index_status`); `is_attached_pic` as a derived bool.

**Status: LOCKED (rev 6) — user-approved.** *(rev 6: per-track `error_status`
removed — error-state derived from `index_errors` (stage-coded) + `index_status`;
cross-cutting cleanup, user-authorized reopen of locked r5. `TrackDisposition`
is `::mediaframe` per the descriptor re-scope — mechanical path-rename pending.)*
