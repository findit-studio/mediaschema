//! Hand-written domain layer — the architectural hub for mediaschema.
//!
//! App logic programs against these types; the buffa-generated wire types at
//! the crate root are the serialization edge. Domain types are governed by
//! `domain → wire → domain` round-trip property tests (added as the aggregate
//! types come online in later stacked PRs).
//!
//! Locked-schema implementation tracks `schema/*.md`. This module starts with
//! the primitives + shared cross-cutting VOs; subsequent stacked PRs add the
//! enums + bitflags, then the leaf aggregates, then the big container/track
//! aggregates.

pub mod primitives;
pub mod vo;

pub use primitives::{ErrorCode, ErrorInfo, FileChecksum, Location, Rgba, Uuid7};
pub use vo::{LocalizedText, Provenance};
