# `VideoTrack<Id>` — a video stream  *(rev 4 — open Qs resolved per videoframe PR #2 + your decisions; in review, NOT self-locked)*

## Domain meaning

One video stream of a `Video` facet (`parent → Video.id`). Holds stream/codec
descriptors, the frame/pixel/colour vocabulary (`::videoframe` extern), the
per-stream `Scene` refs, and per-track indexing state. Its own schema.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). `mediatime` media-time; **`videoframe` extern** for
pixel/colour/frame vocabulary (parallel to mediatime). Source-locator = ffmpeg
`stream_index` / WebCodecs only. Conversions deferred.

## Fields (owner: **VF** = `::videoframe` extern · **MS** = mediaschema)

| field | domain type | owner | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | MS | canonical identity |
| `parent` | `Id` | MS | FK → `Video.id` |
| `stream_index` | `Option<u32>` | MS | source-locator (ffmpeg/WebCodecs); not identity |
| `container_track_id` | `Option<u64>` | MS | keep only if pipeline uses it |
| `duration` | `Option<TrackTime>` | MS | per-track duration (mediatime extern) |
| `codec` | `VideoCodec` (enum) | MS | stream descriptor; `Other(SmolStr)` escape ([enums.md](enums.md)) |
| `profile` · `level` | `Option<SmolStr>` · `Option<u16>` | MS | codec profile/level |
| `bit_rate` | `u64` | MS | **per-track** bitrate (0 → unknown) — was missing |
| `dimensions` | `videoframe::Dimensions` | **VF** | coded W×H (dedup mediaschema's own) |
| `visible_rect` | `Option<videoframe::Rect>` | **VF** | clean-aperture / crop |
| `sample_aspect_ratio` | `videoframe::SampleAspectRatio` | **VF** | resolved → VF (added in PR #2) |
| `pixel_format` | `videoframe::PixelFormat` | **VF** | FFmpeg pixfmt (bit-depth encoded here) |
| `color` | `videoframe::ColorInfo` | **VF** | primaries/transfer/matrix/range/chroma → HDR *detection* |
| `hdr_static` | `Option<videoframe::HdrStaticMetadata>` | **VF** | **resolved**: MaxCLL/MaxFALL + mastering display, from real ffmpeg side-data (added in PR #2) |
| `frame_rate` | `Rational` + `is_vfr: bool` | MS | exact ratio (replaces `f64`); VFR-aware |
| `rotation` | `videoframe::Rotation` | **VF** | resolved → VF (added in PR #2) |
| `field_order` | `FieldOrder` (enum) | MS | progressive / tff / bff — interlace (forgotten; high value) |
| `has_embedded_captions` | `bool` | MS | CEA-608/708 present |
| `is_attached_pic` | `bool` | MS | album-art pseudo-stream — **not** a real video track (forgotten; important) |
| `disposition` | `TrackDisposition` (bitflags!) | MS | shared flag set ([bitflags.md](bitflags.md)) |
| `is_primary` · `auto_selected` | `bool` | MS | selection signals |
| `scenes` | `Vec<Id>` | MS | refs → [scene.md](scene.md) (per-stream detection) |
| `index_status` | `VideoIndexStatus` (bitflags!) | MS | per-kind pipeline stages |
| `index_errors` | `Vec<ErrorInfo>` | MS | per-track error truth (→ `Media.error_flags`) |
| `error_status` | `VideoErrorStatus` (bitflags!) | MS | error categories |

`ordinal` / `selection_reason` dropped. Per-kind index model: `VideoIndexStatus`
+ `index_errors` are truth; coarse stage derived.

## Resolved (rev 3 → 4)

- **Boundary:** confirmed — `::videoframe` ships buffa + `Dimensions`/`Rect`/
  `PixelFormat`/`ColorInfo`/`HdrStaticMetadata`/`Rotation`/`SampleAspectRatio`
  (videoframe **PR #2**, open). mediaschema extern wiring is **blocked on PR #2
  merge + videoframe publish** (task #69), then `.extern_path(".videoframe.v1",
  "::videoframe")` + dedup the migrated `Dimensions`.
- **HDR-static:** `Option<videoframe::HdrStaticMetadata>` — real decoder
  side-data (`AV_FRAME_DATA_CONTENT_LIGHT_LEVEL` + `MASTERING_DISPLAY`); PQ/HLG
  transfer is the *fallback* HDR signal, not a replacement.
- **VT-disp:** shared `TrackDisposition` (cross-cutting locked).
- **rotation / SAR:** moved MS → **VF** (now first-class videoframe types).

## Open questions (recommendations in place — confirm)

- **VT-codec:** `VideoCodec` enum + `Other(SmolStr)` (recommended) vs opaque
  `CodecId` VO. *Lean: enum (queryable facet); profile/level separate.*
- **VT-scenes:** `Video.total_scenes` = Σ over tracks of `scenes.len()` —
  confirm the rollup rule.
- **Dolby Vision:** add `dovi: Option<DolbyVisionConfig>` (profile/level/
  rpu-present)? DoVi ≠ HDR10 static. *Lean: yes if DoVi is in scope (forgotten).*

## Useful information you may have forgotten (proposed — accept/reject)

- `start_pts: Option<i64>` — non-zero stream start; A/V sync & seeking.
- `nb_frames: Option<u64>` — frame count (exact-duration / progress / VFR).
- `has_b_frames: bool`, `closed_gop: Option<bool>` — seek/cut behaviour.
- `bits_per_raw_sample: Option<u8>` — true bit depth beyond pixfmt.
- `stereo_mode: Option<StereoMode>` — 3D/stereoscopic packing (rare but lossy
  if dropped).
- `is_attached_pic` (in table) — **strongly recommend**: prevents album-art
  "video" streams in audio files from being indexed as real video.
- `field_order` (in table) — **strongly recommend**: interlaced sources need
  deinterlace decisions; commonly forgotten.

## Projection notes

- **sqlx**: `video_track` table; `id` PK; `parent` FK; VF types flattened
  (`dimensions`→`width`/`height`, `color`→5 enum cols, `pixel_format`→`u32`,
  `hdr_static`→its scalar cols); `index_*` `INTEGER` + generated bool cols.
- **mongodb**: `_id`=UUIDv7; embedded VF subdocs.
- **graphql**: pixel/colour/dims/hdr exposed; derived stage; `index_errors`/
  `error_status` exposed.

**Status: in review (rev 4) — recommendations in place; confirm open Qs. NOT self-locked.**
