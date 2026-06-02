//! [`Identified<Id, D>`] — a payload paired with a stable identifier.
//!
//! Lightweight transport-level envelope used by engine / service crates
//! that emit detection or analysis output *before* it is persisted:
//! assign an `Id`, wrap as `Identified<Id, D>`, hand to the storage
//! layer. The persistence layer can then call
//! [`Identified::into_components`] to split the pair back into the row
//! id and the row body.
//!
//! Predates the per-aggregate typed-id design (`Speaker<Id>`,
//! `Audio<Id>`, …) and complements it: typed-id aggregates carry their
//! id *inside* the value; [`Identified`] keeps the id *next to* a value
//! that does not own one (a `FaceDetection`, an LLM analysis blob, an
//! engine-emitted detection that has not yet been folded into its
//! parent aggregate).
//!
//! ## Conventions
//!
//! Per `rust-type-conventions`: getters are named `_ref`-suffixed
//! (`id_ref`, `data_ref`), `new` is `const`, and there is no
//! `as_ref()` impl — this is a value-type wrapper, not a smart
//! pointer. There are no intrinsic invariants (`validation-responsibility-boundary`:
//! sentinel-id rejection is an aggregate / application concern, not a
//! transport one), so the constructor is infallible.
//!
//! `Identified` is `no-std + no-alloc` — pure-stack composition.

/// A payload paired with a stable identifier.
///
/// Used by engine / service crates that emit detection / analysis
/// output before it is persisted: assign an id, wrap as
/// `Identified<Id, D>`, hand to the storage layer.
///
/// `Clone` / `Copy` are derived: they apply iff `Id` and `D` both
/// implement the trait, courtesy of `derive`'s built-in per-generic
/// gating. No `as_ref()` smart-pointer impl — see the module doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Identified<Id, D> {
  id: Id,
  data: D,
}

impl<Id, D> Identified<Id, D> {
  /// Pair `id` with `data`.
  ///
  /// Const-fn so it composes inside `const` contexts; `#[must_use]`
  /// because the returned wrapper is the only useful product.
  #[inline]
  #[must_use]
  pub const fn new(id: Id, data: D) -> Self {
    Self { id, data }
  }

  /// Borrow the identifier.
  #[inline]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// Borrow the payload.
  #[inline]
  pub const fn data_ref(&self) -> &D {
    &self.data
  }

  /// Decompose into the `(id, data)` pair.
  #[inline]
  pub fn into_components(self) -> (Id, D) {
    (self.id, self.data)
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn new_and_accessors() {
    let v = Identified::new(7u32, "hello");
    assert_eq!(v.id_ref(), &7u32);
    assert_eq!(v.data_ref(), &"hello");
  }

  #[test]
  fn into_components_returns_pair() {
    let v = Identified::new(42u64, [1u8, 2, 3]);
    let (id, data) = v.into_components();
    assert_eq!(id, 42u64);
    assert_eq!(data, [1u8, 2, 3]);
  }

  #[test]
  fn clone_when_both_clone() {
    // String is Clone but not Copy — exercises Clone path.
    let v = Identified::new(1u8, 2u8);
    #[allow(clippy::clone_on_copy)]
    let w = v.clone();
    assert_eq!(v, w);
  }

  #[test]
  fn copy_when_both_copy() {
    let v = Identified::new(1u8, 2u16);
    // Pass `v` by value, then use again — only compiles if `Copy`.
    fn take<T: Copy>(_: T) {}
    take(v);
    assert_eq!(v.id_ref(), &1u8);
    assert_eq!(v.data_ref(), &2u16);
  }

  #[cfg(any(feature = "std", feature = "alloc"))]
  #[test]
  fn debug_format_smoke() {
    // Smoke-test: `Debug` derived, exact format is not load-bearing.
    // Heap-tier-only because `format!` needs `alloc`.
    use std::format;
    let s = format!("{:?}", Identified::new(1u8, 2u8));
    assert!(s.contains("id"));
    assert!(s.contains("data"));
  }
}
