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
#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub mod vo;

// Always available (pure no-std no-alloc):
pub use primitives::{ErrorCode, FileChecksum, Rgba, Uuid7};

// `feature = "alloc"`-gated: types that reach `Vec` or `SmolStr`.
#[cfg(any(feature = "std", feature = "alloc"))]
pub use primitives::{ErrorInfo, Location};
#[cfg(any(feature = "std", feature = "alloc"))]
pub use vo::{LocalizedText, Provenance};
