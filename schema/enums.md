# Domain enums  *(rev 1 — drafted for review, NOT self-locked)*

Reference for the **18 enums** (17 wire enums + the new per-kind index-stage
model). Policy: small fixed vocabularies = **closed** enums; open/extensible
vocabularies (codecs, containers) = `#[non_exhaustive]` + an `Other(SmolStr)`
arm so an unrecognised wire value is preserved, not lost (the proto3 zero arm
becomes a domain sentinel). Colour/pixfmt enums are **`::videoframe` extern** —
*not* redefined here (FFmpeg-n8.1 numbered + `Unknown(u32)` + `DOMAIN_EXT_BASE`,
per videoframe PR #2); listed only for completeness.

| domain enum | kind | variants (key) | wire / notes |
|---|---|---|---|
| `MediaKind` | closed | `Video`, `Audio` | wire `UNSPECIFIED` = pre-probe sentinel → `Option`/explicit `Unknown`; **kept, not derived** (drives which facets are created) |
| `ContainerFormat` | non_exhaustive | `Mp4`,`Mkv`,`Mov`,`Webm`,`Avi`,`Ts`,`Mka`,`Other(SmolStr)` | file/container (codec is per-track) |
| `AudioFormat` | non_exhaustive | `Mp3`,`Flac`,`Wav`,`Aac`,`Ogg`,`M4a`,`Other` | standalone-audio file form |
| `AudioContainerFormat` | non_exhaustive | container of an audio file | parallels `ContainerFormat` |
| `VideoCodec` | non_exhaustive | `H264`,`Hevc`,`Av1`,`Vp9`,`Vp8`,`Mpeg2`,`ProRes`,`Other(SmolStr)` | stream descriptor (VT-codec) |
| `AudioCodec` | non_exhaustive | `Aac`,`Mp3`,`Flac`,`Opus`,`Vorbis`,`Ac3`,`Eac3`,`Dts`,`TrueHd`,`PcmS16Le`,`Other` | |
| `SubtitleCodec` | non_exhaustive | `Srt`,`Ass`,`WebVtt`,`MovText`,`DvbSub`,`Pgs`,`DvdSub`,`Other` | `is_image_based()` derived |
| `SubtitleTrackOrigin` | closed | `External`,`Embedded`,`Generated` | cheap-unambiguous redesign (locked) |
| `ChannelLayout` | non_exhaustive | `Mono`,`Stereo`,`5_1`,`7_1`,`Other(SmolStr)` | audio |
| `KeyframeKind` | closed | `Poster`,`SceneRepresentative`,`Interval`,`IFrame` | |
| `SegmentKind` | closed | `Speech`,`Music`,`Silence`,`Noise`,`Overlap` | audio-segment (proposed enrichment) |
| `BitRateMode` | closed | `Cbr`,`Vbr`,`Abr` | audio (proposed enrichment) |
| `AudioContentKind` | closed | `Speech`,`Music`,`Mixed`,`Silence` | audio track (proposed enrichment) |
| `ScanStatus` | closed | `Ok`,`Partial`,`Failed` | `WatchedLocation` |
| `VideoIndexStage` | closed | `Pending→Probed→Scenes→Keyframes→Vlm→Embedded→Done`(+`Failed`) | **per-kind**, distinct lifecycle; derived from `VideoIndexStatus` bits |
| `AudioIndexStage` | closed | `Pending→Probed→Analyzed→Transcribed→Diarized→Embedded→Done`(+`Failed`) | **per-kind** |
| `SubtitleIndexStage` | closed | `Pending→Probed→CuesParsed→Ocr→SearchIndexed→Done`(+`Failed`) | **per-kind** |
| *(videoframe extern)* `ColorPrimaries`/`ColorTransfer`/`ColorMatrix`/`ColorRange`/`ChromaLocation`/`DcpTargetGamut`/`PixelFormat` | — | — | `::videoframe`; not a mediaschema enum |

The three `*IndexStage` enums are **separate types** (do not unify — pipelines
genuinely differ). Coarse stage is **derived** from the per-kind
`*IndexStatus` bitflags ([bitflags.md](bitflags.md)) + `index_errors`; it is not
an independently stored field of record.

## Open questions

- **E-codec granularity:** how fine should `VideoCodec`/`AudioCodec` go (e.g.
  `H264` vs profiles)? *Lean: codec family only; profile/level separate fields.*
- **E-closed:** confirm `#[non_exhaustive]`+`Other(SmolStr)` for codec/container
  vs strictly closed (closed loses unknown real-world values). *Lean: as tabled.*
- **E-stage:** the exact stage ordering per kind needs confirmation against the
  real pipeline (depends on [bitflags.md](bitflags.md) stage bits).
- **MediaKind pre-probe:** model the wire `UNSPECIFIED` as `Option<MediaKind>`
  on `Media` vs a `MediaKind::Unknown` variant. *Lean: `kind` stays required
  (`Media` exists post-probe); pre-probe is a different lifecycle, not this enum.*

**Status: in review (rev 1) — drafted for your one-by-one review. NOT self-locked.**
