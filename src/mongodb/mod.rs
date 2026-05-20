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
//! | `Vec<u8>` (inline bytes) | [`bson::Binary`] with `subtype = 0` |
//! | Domain enums | `Int32` (cast from their `u8`/`u32` backing) |
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
