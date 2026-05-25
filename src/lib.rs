//! Product-agnostic media-primitive schema.
//!
//! The architectural hub is the hand-written [`domain`] layer; every
//! backend — protobuf wire (under [`buffa`]), `sqlx` row mappers
//! (under [`sqlx`]), MongoDB (under [`mongodb`]) — is a thin lossless
//! conversion to/from the domain. Locked schema docs live under
//! `schema/*.md`; they are the specification the implementation tracks.
//!
//! Encode-side bridges (`From<&Domain> for Backend`) are infallible —
//! the domain is the validated side. Decode-side bridges
//! (`TryFrom<Backend> for Domain`) route through the same `try_new` +
//! `with_*` builders application code uses, so every invariant is
//! re-enforced at the wire/storage edge.
//!
//! ## Aggregate clusters
//!
//! - [`domain::Media`] / [`domain::MediaFile`] / [`domain::WatchedLocation`]
//!   — content-addressed media identity + per-copy locator + FS-event
//!   monitor.
//! - [`domain::Audio`] / [`domain::AudioTrack`] / [`domain::AudioSegment`]
//!   (+ `Word` child) — audio cluster.
//! - [`domain::Video`] / [`domain::VideoTrack`] / [`domain::Scene`] /
//!   [`domain::Keyframe`] — video cluster.
//! - [`domain::Subtitle`] / [`domain::SubtitleTrack`] /
//!   [`domain::SubtitleCue`]`<Id, D>` — subtitle cluster; cues are
//!   polymorphic over the per-format payload `D` (`SrtData`, `VttData`,
//!   `AssData`, `LrcData`).
//! - [`domain::Person`] / [`domain::Speaker`] — identity (per-track
//!   diarized voice with embedding-keyed voiceprint).
//!
//! ## Feature flags
//!
//! Three independent **capability tiers** (`alloc` / `std`, both
//! additive) plus optional **backend** features. Default is `std`.
//!
//! | flag | role | depends on |
//! |---|---|---|
//! | _none_ | no-std + no-alloc; stack-only types only | — |
//! | `alloc` | no-std + alloc; adds heap-using domain types | — |
//! | `std` (default) | adds `jiff`-using aggregates + `Uuid::now_v7` | — |
//! | `buffa` | proto3 wire layer (under [`buffa`]) | `std` or `alloc` |
//! | `json` | wire JSON via serde | `std + buffa` |
//! | `arbitrary` | `Arbitrary` derives for property tests | `std + buffa` |
//! | `mongodb` | bson backend (under [`mongodb`]) | `std + json` |
//! | `sqlx-postgres` / `sqlx-mysql` / `sqlx-sqlite` | sql backends (under [`sqlx`]) | `std` |
//!
//! ## Regenerating wire code
//!
//! The buffa-generated wire layer (`src/generated/`) is produced from
//! the `.proto` files in `proto/`. Regenerate with
//! `cargo run -p xtask -- gen`. Do **not** hand-edit the generated
//! files.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]

// Alias `alloc as std` on no-std + alloc builds so domain code can use
// `std::vec::Vec` / `std::string::String` uniformly across feature
// combos. When `feature = "std"` is on, the real `std` is in scope via
// the prelude (its `extern crate` is implicit).
#[cfg(all(not(feature = "std"), feature = "alloc"))]
#[allow(unused_extern_crates)]
extern crate alloc as std;

// Wire layer (buffa-generated) is `std`-only. The codegen uses `Vec` /
// `String` (need alloc) AND the buffa runtime utilities pull in std
// path types — gating the whole module on `feature = "std"` is the
// simplest correct shape.
#[cfg(feature = "buffa")]
#[allow(warnings, clippy::all)]
mod generated {
  include!("generated/mod.rs");
}

/// Hand-written domain layer — the architectural hub. App logic programs
/// against `mediaschema::domain::*`; the buffa-generated wire types at the
/// crate root (re-exported from `generated::media::v1::*`, available with
/// `feature = "std"`) are the serialization edge. Domain ⇄ wire conversions
/// are added incrementally as each aggregate lands (with `domain → wire →
/// domain` round-trip property tests). See `schema/*.md` for the locked
/// specifications.
pub mod domain;

// Wire ⇄ domain conversion bridge. Requires `feature = "buffa"` (for
// the wire types themselves) AND a heap tier (`std` or `alloc`) because
// every domain type the bridge touches that has a wire counterpart
// (`Location`, `ErrorInfo`, `WatchedLocation`, `Media`, …) is itself
// `any(std, alloc)`-gated.
#[cfg(all(feature = "buffa", any(feature = "std", feature = "alloc")))]
#[cfg_attr(
  docsrs,
  doc(cfg(all(feature = "buffa", any(feature = "std", feature = "alloc"))))
)]
pub mod buffa;

/// `sqlx` row-mapping backend — Postgres / MySQL / SQLite. Off by default;
/// enable one (or more) of `sqlx-postgres` / `sqlx-mysql` / `sqlx-sqlite`.
#[cfg(any(
  feature = "sqlx-postgres",
  feature = "sqlx-mysql",
  feature = "sqlx-sqlite"
))]
#[cfg_attr(
  docsrs,
  doc(cfg(any(
    feature = "sqlx-postgres",
    feature = "sqlx-mysql",
    feature = "sqlx-sqlite"
  )))
)]
pub mod sqlx;

/// Optional MongoDB backend — `bson::Document` ↔ domain aggregates plus
/// per-collection [`::mongodb::IndexModel`](mongodb::IndexModel)
/// constructors. Off by default; enable with `--features mongodb`.
/// Inside the module the external `::mongodb` crate is referenced via
/// its absolute path so the `crate::mongodb` module name does not
/// shadow it.
#[cfg(feature = "mongodb")]
#[cfg_attr(docsrs, doc(cfg(feature = "mongodb")))]
pub mod mongodb;

// Flatten the product-neutral `media.v1` package to the crate root so
// consumers write `mediaschema::Detection`. Named (not glob) so buffa
// internals (`__buffa`, `__*_JSON_ANY`) stay out of the public surface.
// SP2 database-domain types (same media.v1 package since mono-consolidation).
// SP3 network-domain types (same media.v1 package since mono-consolidation).
/// Oneof variant for [`Event`]: the `kind` discriminant arm.
#[cfg(feature = "buffa")]
pub use generated::media::v1::event::Kind as EventKind;
/// Oneof variant for [`Location`]: `Kind::Local(…)`.
#[cfg(feature = "buffa")]
pub use generated::media::v1::location::Kind as LocationKind;
/// Oneof variant for [`LocationTarget`]: `Kind::Local(…)`.
#[cfg(feature = "buffa")]
pub use generated::media::v1::location_target::Kind as LocationTargetKind;
/// Oneof variant for [`MediaKind`]: `Kind::Video(…)` or `Kind::Audio(…)`.
#[cfg(feature = "buffa")]
pub use generated::media::v1::media_kind::Kind as MediaKindKind;
/// Oneof variant for [`Request`]: the `kind` discriminant arm.
#[cfg(feature = "buffa")]
pub use generated::media::v1::request::Kind as RequestKind;
/// Oneof variant for [`Response`]: the `kind` discriminant arm.
#[cfg(feature = "buffa")]
pub use generated::media::v1::response::Kind as ResponseKind;
/// Oneof variant for [`SubtitleTrackOrigin`]: `Source::SourceAudioTrackId(…)` or `Source::SourceSubtitleTrackId(…)`.
#[cfg(feature = "buffa")]
pub use generated::media::v1::subtitle_track_origin::Source as SubtitleTrackOriginSource;
/// Oneof variant for [`SubtitleCue`]: the per-format `data` arm
/// (`Data::Srt` / `Data::Vtt` / `Data::Ass` / `Data::Lrc`).
#[cfg(feature = "buffa")]
pub use generated::media::v1::subtitle_cue::Data as SubtitleCueData;
#[cfg(feature = "buffa")]
pub use generated::media::v1::{
  ActionDetection, Aesthetics, AnimalAnalysis, AppPathBuf, Audio, AudioAnalysis,
  AudioChannelLayout, AudioChannelOrderKind, AudioChannelSpec, AudioClipKind, AudioCodec,
  AudioContainerFormat, AudioCoverArt, AudioEvent, AudioFileRecord, AudioFormat, AudioMeta,
  AudioPrefilterClass, AudioSampleFormat, AudioSegment, AudioStreamMeta, AudioSummary, AudioTrack,
  AudioTrackMeta, AudioTrackRole, AudioTranscriptSegment, BarcodeDetection, BodyPose3DDetection,
  BodyPose3DHeightEstimation, BodyPose3DJoint, BodyPoseDetection, BodyPoseJoint, BoundingBox,
  BrowseItem, BrowseRequest, BrowseResponse, Ced, CedDetection, ChannelLayoutKind, Chromaprint,
  Clap, ClassificationDetection, CodecId, ColorDetection, DbMediaKind, Detection, Dimensions,
  DocumentSegment, Ebur128, EjectVolumeRequest, EjectVolumeResponse, EmotionDetection, ErrorInfo,
  Event, FaceDetection, FaceLandmarkPoint, FaceLandmarkRegion, FaceLandmarksDetection, FailedFile,
  FailedFilesResponse, FeaturePrint, FileChecksum, FolderUpdatedEvent, GetDaemonInfoRequest,
  GetDaemonInfoResponse, GetFileIndexingStatsRequest, GetFileIndexingStatsResponse,
  GetIndexedFileRequest, GetIndexedFileResponse, GetLocationStatsRequest, GetLocationStatsResponse,
  GetModelStatusRequest, GetModelStatusResponse, HandChirality, HandPoseDetection,
  HeartbeatRequest, HeartbeatResponse, HorizonInfo, HumanAnalysis, Id, IndexLocationRequest,
  IndexLocationResponse, IndexingFile, IndexingProgressResponse, Keyframe, Language,
  LightingDetection, ListLocationsRequest, ListLocationsResponse, Local, LocalizedText, Location,
  LocationTarget, Media, MediaFile, MediaKind, MediaMeta, ModelDownloadProgress,
  ModelDownloadProgressEvent, ModelDownloadProgressResponse, ModelInfo, MoodDetection,
  NetFailedFile, ObjectDetection, Pagination, Person, PersonConfidence,
  PersonInstanceMaskDetection, PersonSegmentationMask, Point2D, Provenance, RemoveLocationRequest,
  RemoveLocationResponse, Request, RequestEnvelope, Response, ResponseEnvelope, RetryFailedRequest,
  RetryFailedResponse, SaliencyRegion, Scene, SceneMeta, SceneVlmResult, SearchFilter, SearchHit,
  SearchRequest, SearchResponse, SoundSource, Sp2CodegenSmoke, Sp3CodegenSmoke, Speaker,
  SpeakerSegment, SrtData, SubjectDetection, Subtitle, SubtitleCue, SubtitleCueKind,
  SubtitleMeta, SubtitleTrack, SubtitleTrackFormat, SubtitleTrackMeta, SubtitleTrackOrigin,
  SubtitleTrackRole, Tag,
  TagConfidence, TextDetection, Timecode, TimedDetection, TrackClassificationType, TrackRecord,
  TrackTag, TrackTime, TrackTimeSource, UpdateAnnotationRequest, UpdateAnnotationResponse, Video,
  VideoFormat, VideoMeta, VideoStreamMeta, VideoTrack, VideoTrackMeta, VoiceFingerprint, Volume,
  VolumeMeta, VolumeStateChangedEvent, WatchedLocation, Word,
};
/// Per-format subtitle-cue payload messages — the oneof arms of
/// [`SubtitleCue.data`](SubtitleCue).
#[cfg(feature = "buffa")]
pub use generated::media::v1::{AssData, AssStyle, LrcData, LrcMetadata, LrcWord, VttData};
/// WebVTT cue-setting enums (used by [`VttData`]).
#[cfg(feature = "buffa")]
pub use generated::media::v1::{VttLineAlign, VttPositionAlign, VttTextAlign, VttVertical};
/// Per-track subtitle aggregate messages — WebVTT regions / style
/// blocks (siblings of [`VttData`]).
#[cfg(feature = "buffa")]
pub use generated::media::v1::{VttRegion, VttStyleBlock};
