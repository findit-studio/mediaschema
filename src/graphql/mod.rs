//! Optional GraphQL backend — `async-graphql` `#[Object]` resolvers,
//! custom scalars, enum mirrors, and bitflags wrappers for the locked
//! domain aggregates (`schema/*.md`).
//!
//! # Why manual `#[Object] impl` blocks on `Gql*` wrappers
//!
//! Every domain type in `src/domain/` keeps its fields **private** and
//! exposes them via `pub const fn` getters per the encapsulation rule.
//! `async_graphql::SimpleObject` needs public fields, so it does not
//! apply. We use `#[async_graphql::Object] impl <T>` blocks — but the
//! macro extends `<T>`'s impl block, so the resolver method names would
//! collide with the domain accessors of the same name. The answer is a
//! thin `Gql<T>` newtype wrapper per domain type; the macro decorates
//! the wrapper's impl, the resolver bodies delegate to the wrapped
//! domain getter, and `From` conversions are infallible both ways.
//!
//! # Enum mirrors
//!
//! `async_graphql::Enum` would need to be derived on the source enum;
//! the domain layer is off-limits, so this module ships a *newtype
//! mirror* per domain enum: `GqlMediaKind`, `GqlAudioContentKind`, …
//! Each mirror is itself a unit-variant enum (no payload) with `From`
//! conversions both ways. See `enums.rs`.
//!
//! # Bitflags
//!
//! Each bitflags companion gets a wrapper Object that exposes `bits`
//! (raw flag word) plus a `flags: [String]` list. See `bitflags.rs`.
//!
//! # Location (Union)
//!
//! [`crate::domain::Location`] is a newtype-variant enum — wrapped in a
//! GraphQL Union mirror with `From` conversions both ways. See
//! `media.rs` (`GqlLocation`).
//!
//! # Custom scalars
//!
//! - [`crate::domain::Uuid7`] → hyphenated UUID string.
//! - [`crate::domain::FileChecksum`] → lower-case hex string.
//! - [`jiff::Timestamp`] (via [`GqlJiffTimestamp`]) → RFC 3339 string.
//! - [`mediatime::Timestamp`] / [`mediatime::TimeRange`] (via the two
//!   wrappers) → `<pts>@<num>/<den>` / `[<start>,<end>]@<num>/<den>`.
//! - [`crate::domain::Rgba`] is **not** a scalar — exposed as an Object
//!   with `{ r, g, b, a, bits }`.
//!
//! See `scalars.rs`.
//!
//! # Stub Query root
//!
//! The Query root in `query.rs` exists so the schema can be `build()`-ed
//! and an SDL emitted. The resolvers all return `None` / empty `Vec`s —
//! plugging a real data source is the caller's job. The SDL is
//! committed at `src/graphql/schema.graphql` for human inspection;
//! regenerable from the integration test in `tests/graphql_sdl.rs`.

#![cfg_attr(docsrs, doc(cfg(feature = "async-graphql")))]
#![allow(clippy::module_inception)]
// async-graphql's `#[Object]` macro emits non-elided lifetimes in its
// `register` impls, which fires `rust_2018_idioms` under
// `RUSTFLAGS=-Dwarnings`. We silence it at the module boundary instead
// of patching macro output.
#![allow(elided_lifetimes_in_paths)]

pub mod audio;
pub mod bitflags;
pub mod enums;
pub mod error;
pub mod leaves;
pub mod media;
pub mod query;
pub mod scalars;
pub mod subtitle;
pub mod video;

pub use audio::{
  GqlAudio, GqlAudioCoverArt, GqlAudioFingerprint, GqlAudioSegment, GqlAudioTags, GqlAudioTrack,
  GqlLoudness, GqlWord,
};
pub use bitflags::{
  GqlAudioIndexStatus, GqlMediaErrorFlags, GqlSubtitleIndexStatus, GqlVideoIndexStatus,
};
pub use enums::{
  GqlAudioContentKind, GqlAudioIndexStage, GqlKeyframeExtractor, GqlMediaKind, GqlScanStatus,
  GqlSceneDetector, GqlSubtitleIndexStage, GqlSubtitleKind, GqlVideoIndexStage,
};
pub use error::GqlError;
pub use leaves::{GqlMediaFile, GqlSceneAnnotation, GqlSpeaker, GqlUserTag, GqlWatchedLocation};
pub use media::{
  GqlErrorInfo, GqlLocalLocation, GqlLocalizedText, GqlLocation, GqlMedia, GqlMediaDevice,
  GqlMediaGeoLocation, GqlProvenance, GqlRgba,
};
pub use query::Query;
pub use scalars::{GqlJiffTimestamp, GqlMediaTimeRange, GqlMediaTimestamp};
pub use subtitle::{GqlSubtitle, GqlSubtitleCue, GqlSubtitleIndexProgress, GqlSubtitleTrack};
pub use video::{
  GqlActionDetection, GqlAesthetics, GqlAnimalAnalysis, GqlBarcodeDetection,
  GqlBodyPose3DDetection, GqlBodyPose3DJoint, GqlBodyPoseDetection, GqlBodyPoseJoint,
  GqlBoundingBox, GqlColorInfo, GqlDetection, GqlDimensions, GqlDocumentSegment,
  GqlDolbyVisionConfig, GqlDominantColor, GqlFaceDetection, GqlFaceLandmarkRegion,
  GqlFaceLandmarksDetection, GqlFrameRate, GqlHdrStaticMetadata, GqlHorizonInfo, GqlHumanAnalysis,
  GqlKeyframe, GqlObjectDetection, GqlPersonInstanceMaskDetection, GqlPersonSegmentationMask,
  GqlPoint2D, GqlRational, GqlRect, GqlSaliencyRegion, GqlScene, GqlSubjectDetection,
  GqlTextDetection, GqlVideo, GqlVideoCodec, GqlVideoIndexProgress, GqlVideoTrack, GqlVlmAnalysis,
};
