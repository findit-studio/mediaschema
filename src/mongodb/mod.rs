//! Optional MongoDB backend — `bson::Document` ↔ domain aggregates +
//! per-collection [`::mongodb::IndexModel`] constructors.
//!
//! # Mapping rules
//!
//! The mapping is **lossless** and **bidirectional** for every locked
//! aggregate. The two halves are:
//!
//! - `impl From<&Aggregate> for bson::Document` — infallible encode.
//! - `impl TryFrom<bson::Document> for Aggregate` — fallible decode
//!   routed through the aggregate's `try_new` + `with_*` accessors so
//!   every domain invariant (`nil id`, orphan FKs, zero checksum, …) is
//!   re-enforced at the bson edge.
//!
//! ## Type translations
//!
//! | Domain type | bson representation |
//! | --- | --- |
//! | [`crate::domain::Uuid7`] | [`bson::Binary`] with `subtype = 4` (UUID); raw 16-byte payload |
//! | [`crate::domain::FileChecksum`] | [`bson::Binary`] with `subtype = 0` (generic); raw 32-byte payload |
//! | [`smol_str::SmolStr`] | [`bson::Bson::String`] (empty string preserved, never omitted) |
//! | [`jiff::Timestamp`] | [`bson::DateTime`] (via nanoseconds-since-epoch round-trip; sub-microsecond precision lost — same trade as everywhere else in the ecosystem) |
//! | [`mediatime::Timestamp`] | nested document `{ pts: i64, timebase: { num: u32, den: u32 } }` |
//! | [`mediatime::TimeRange`] | nested document `{ start: i64, end: i64, timebase: …}` |
//! | [`crate::domain::Rgba`] | nested document `{ r: u8, g: u8, b: u8, a: u8 }` |
//! | [`crate::domain::Provenance`] | nested document with the 4 string fields |
//! | [`crate::domain::LocalizedText`] | nested document with `src` + `translated` |
//! | [`crate::domain::ErrorInfo`] | nested document `{ code: u32, message: String }` |
//! | [`crate::domain::ErrorCode`] | `Int64` (its `as_u32()` wire value — `Unknown(_)` preserved) |
//! | [`crate::domain::Location`] | nested document `{ kind: "local", volume: Binary, components: [String] }` |
//! | [`crate::domain::MediaErrorFlags`] etc. | `Int64` (raw `.bits()` value) |
//! | `bytes::Bytes` / `Vec<u8>` (inline bytes) | [`bson::Binary`] with `subtype = 0` |
//! | Domain enums | `Int32` (cast from their `u8`/`u32` backing) |
//!
//! ### `mediaframe` descriptor / VO types
//!
//! | `mediaframe` type | bson representation |
//! | --- | --- |
//! | `codec::{Video,Audio,Subtitle}Codec`, `audio::ChannelLayout`, `container::Format` | `String` (their `as_str()` slug; `FromStr` is total — unknown values round-trip via `Other`) |
//! | `audio::BitRateMode`, `subtitle::TrackOrigin`, `pixel_format::PixelFormat`, `frame::{Rotation,FieldOrder,StereoMode}`, `color::{Primaries,Transfer,Matrix,DynamicRange,ChromaLocation}` | `Int32` (their stable `to_u32`/`from_u32` code) |
//! | `disposition::TrackDisposition` | `Int64` (its `to_u32()` bits) |
//! | `lang::Language` | `String` (BCP-47 via `to_bcp47`/`from_bcp47`) |
//! | `frame::Dimensions` | nested `{ w: i64, h: i64 }` |
//! | `frame::Rect` | nested `{ x, y, width, height }` |
//! | `frame::SampleAspectRatio`, `frame::FrameRate` | nested `{ num, den }` (FrameRate adds `is_vfr`) |
//! | `color::Info` | nested `{ primaries, transfer, matrix, range, chroma_location }` (Int32 codes) |
//! | `color::HdrStaticMetadata` | nested `{ mastering?, content_light? }` (mastering = primaries `[{x,y};3]` + white point + max/min luminance; content_light = `{max_cll, max_fall}`) |
//! | `color::DolbyVisionConfig` | nested `{ profile, level, rpu_present, el_present, bl_signal_compat_id }` |
//! | `audio::Loudness` | nested `{ integrated_lufs, range_lu, true_peak_dbtp, sample_peak_dbfs }` |
//! | `audio::Fingerprint` | nested `{ algorithm: String, value: Binary }` |
//! | `audio::Tags` | nested doc (string fields `""`-means-absent; numeric / `language` fields `Null`-means-absent) |
//! | `audio::CoverArt` | nested `{ mime: String, data: Binary }` |
//! | `capture::Device` | nested `{ make, model }` |
//! | `capture::GeoLocation` | nested `{ lat: f64, lon: f64, altitude?: f64 }` |
//!
//! ## Collections + indexes
//!
//! [`indexes::all_indexes`] returns the full collection-name + `IndexModel`
//! list so a deployer can iterate and `create_indexes` on a live cluster.
//!
//! See `schema.md` (sibling file) for the per-collection bson shape and
//! index list in human-readable form.

#![allow(clippy::module_inception)]

// Use absolute paths for both external crates so the module name
// `crate::mongodb` does not shadow either.
#[allow(unused_imports)]
use ::bson;
#[allow(unused_imports)]
use ::mongodb as mongodb_crate;

pub mod audio;
pub mod error;
pub mod indexes;
pub mod leaves;
pub mod media;
pub mod subtitle;
pub mod util;
pub mod video;

pub use error::MongoError;
pub use indexes::{all_indexes, CollectionName};
