//! `WatchedLocation` — a **volume** the monitor watches for FS events.
//!
//! Locked `schema/watched_location.md` r8. **An event monitor, not a
//! scanner**: on a create/modify/delete/move event the monitor triggers a
//! (re)index of the affected file/subfolder. Content-addressed model ⇒ no
//! `Media` link, no media count, no scan rollup — "what exists" lives in
//! `Media` (by hash), not here. `is_ejectable` (from `whichdisk`) drives
//! whether a missing volume is expected (transient pause) or a real
//! `VolumeNotAvailable` error.
//!
//! **Volume-scoped, not folder-scoped.** A `WatchedLocation` monitors a
//! whole *volume* (identified by its stable volume UUID — the same identity
//! `LocalLocation.volume` carries), not a particular folder on it. *Which*
//! folder(s) on the volume the monitor actually walks is **application-layer
//! configuration**, deliberately outside this schema: keeping the watch
//! volume-level guarantees a single volume → single `WatchedLocation` →
//! single-FK cascade and makes "two overlapping folder watches on one
//! volume" unrepresentable.

use derive_more::{IsVariant, TryUnwrap, Unwrap};
use jiff::Timestamp;

use crate::domain::{ErrorInfo, ScanStatus, Uuid7};

/// A monitored source volume.
///
/// Generic over `Id` (default [`Uuid7`]); `volume` flows the same `Id` type
/// that `LocalLocation::volume` carries. See `schema/watched_location.md`
/// r8 for the full design.
///
/// **Identity / uniqueness is the volume**: there is exactly one
/// `WatchedLocation` per monitored volume. The `volume` field is not a
/// folder path — it has no path components — so `Movies` and `Movies/2024`
/// watches on a single volume are not representable.
///
/// **No `Default`** — defaulting to `{ id: nil, volume: nil }` would be
/// indistinguishable from a real missing-volume monitor entry. Construct via
/// [`WatchedLocation::try_new`] (the `Uuid7` validating builder). Fields are
/// private; access via the `with_*` / `set_*` builders + getters listed
/// below.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WatchedLocation<Id = Uuid7> {
  id: Id,
  /// Stable identity of the monitored volume — the same UUID
  /// `LocalLocation::volume` carries (written once to
  /// `<mount>/.findit_index/.id`). **Not** a folder path: the watch is
  /// volume-scoped; which folders are walked is application-layer config.
  volume: Id,
  recursive: bool,
  enabled: bool,
  is_ejectable: bool,
  added_at: Timestamp,
  last_reconciled_at: Option<Timestamp>,
  last_reconcile_status: Option<ScanStatus>,
  last_error: Option<ErrorInfo>,
}

impl WatchedLocation<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (every aggregate row needs a real identity) and nil
  /// `volume` (a watch with no volume identity cannot be monitored). All
  /// other fields take sensible non-enabled defaults (`recursive=false`,
  /// `enabled=false`, `is_ejectable=false`) — the caller flips them via
  /// `with_*` / `set_*` after construction.
  pub fn try_new(
    id: Uuid7,
    volume: Uuid7,
    added_at: Timestamp,
  ) -> Result<Self, WatchedLocationError> {
    if id.is_nil() {
      return Err(WatchedLocationError::NilId);
    }
    if volume.is_nil() {
      return Err(WatchedLocationError::NilVolume);
    }
    Ok(Self {
      id,
      volume,
      recursive: false,
      enabled: false,
      is_ejectable: false,
      added_at,
      last_reconciled_at: None,
      last_reconcile_status: None,
      last_error: None,
    })
  }
}

impl<Id> WatchedLocation<Id> {
  /// Canonical identity.
  #[inline(always)]
  pub const fn id(&self) -> &Id {
    &self.id
  }

  /// Stable identity of the monitored volume — the same UUID
  /// `LocalLocation::volume` carries. The watch is volume-scoped; this is
  /// **not** a folder path.
  #[inline(always)]
  pub const fn volume(&self) -> &Id {
    &self.volume
  }

  /// Descend subdirectories.
  #[inline(always)]
  pub const fn is_recursive(&self) -> bool {
    self.recursive
  }

  /// Actively monitored (vs paused).
  #[inline(always)]
  pub const fn is_enabled(&self) -> bool {
    self.enabled
  }

  /// Volume is removable / ejectable (from `whichdisk::is_ejectable`).
  /// Ejectable + absent ⇒ expected/transient (monitor pauses; reconcile
  /// on remount). Non-ejectable + absent ⇒ `last_error` =
  /// `VolumeNotAvailable`.
  #[inline(always)]
  pub const fn is_ejectable(&self) -> bool {
    self.is_ejectable
  }

  /// When this watch was configured.
  #[inline(always)]
  pub const fn added_at(&self) -> &Timestamp {
    &self.added_at
  }

  /// Last full reconcile sweep (bootstrap / after-downtime /
  /// volume-remount catch-up — events the monitor missed while offline).
  #[inline(always)]
  pub const fn last_reconciled_at(&self) -> Option<&Timestamp> {
    self.last_reconciled_at.as_ref()
  }

  /// Status of that sweep.
  #[inline(always)]
  pub const fn last_reconcile_status(&self) -> Option<&ScanStatus> {
    self.last_reconcile_status.as_ref()
  }

  /// Monitor-health failure (e.g. `VolumeNotAvailable`,
  /// `LocalPermissionDenied`). The non-track error case —
  /// `WatchedLocation` is config + monitor health, not media.
  #[inline(always)]
  pub const fn last_error(&self) -> Option<&ErrorInfo> {
    self.last_error.as_ref()
  }

  /// Builder: replace `recursive` flag.
  #[inline(always)]
  #[must_use]
  pub const fn with_recursive(mut self, recursive: bool) -> Self {
    self.recursive = recursive;
    self
  }

  /// Builder: replace `enabled` flag.
  #[inline(always)]
  #[must_use]
  pub const fn with_enabled(mut self, enabled: bool) -> Self {
    self.enabled = enabled;
    self
  }

  /// Builder: replace `is_ejectable` flag.
  #[inline(always)]
  #[must_use]
  pub const fn with_ejectable(mut self, is_ejectable: bool) -> Self {
    self.is_ejectable = is_ejectable;
    self
  }

  /// Builder: replace `last_reconciled_at`.
  #[inline(always)]
  #[must_use]
  pub fn with_last_reconciled_at(mut self, t: Option<Timestamp>) -> Self {
    self.last_reconciled_at = t;
    self
  }

  /// Builder: replace `last_reconcile_status`.
  #[inline(always)]
  #[must_use]
  pub fn with_last_reconcile_status(mut self, s: Option<ScanStatus>) -> Self {
    self.last_reconcile_status = s;
    self
  }

  /// Builder: replace `last_error`.
  #[inline(always)]
  #[must_use]
  pub fn with_last_error(mut self, e: Option<ErrorInfo>) -> Self {
    self.last_error = e;
    self
  }

  /// In-place mutator for `recursive`.
  #[inline(always)]
  pub const fn set_recursive(&mut self, recursive: bool) -> &mut Self {
    self.recursive = recursive;
    self
  }

  /// In-place mutator for `enabled`.
  #[inline(always)]
  pub const fn set_enabled(&mut self, enabled: bool) -> &mut Self {
    self.enabled = enabled;
    self
  }

  /// In-place mutator for `is_ejectable`.
  #[inline(always)]
  pub const fn set_ejectable(&mut self, is_ejectable: bool) -> &mut Self {
    self.is_ejectable = is_ejectable;
    self
  }

  /// In-place mutator for `last_reconciled_at`.
  #[inline(always)]
  pub fn set_last_reconciled_at(&mut self, t: Option<Timestamp>) -> &mut Self {
    self.last_reconciled_at = t;
    self
  }

  /// In-place mutator for `last_reconcile_status`.
  #[inline(always)]
  pub fn set_last_reconcile_status(&mut self, s: Option<ScanStatus>) -> &mut Self {
    self.last_reconcile_status = s;
    self
  }

  /// In-place mutator for `last_error`.
  #[inline(always)]
  pub fn set_last_error(&mut self, e: Option<ErrorInfo>) -> &mut Self {
    self.last_error = e;
    self
  }
}

/// Error returned when [`WatchedLocation::try_new`] cannot uphold the
/// non-nil-id / non-nil-volume invariants. Unit-only enum; derives
/// `IsVariant` plus `Unwrap`/`TryUnwrap` with shared-ref + mut-ref accessor
/// flavours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, Unwrap, TryUnwrap, thiserror::Error)]
#[unwrap(ref, ref_mut)]
#[try_unwrap(ref, ref_mut)]
#[non_exhaustive]
pub enum WatchedLocationError {
  /// The supplied `id` was the nil sentinel — not a real identity.
  #[error("WatchedLocation id must not be the nil UUID")]
  NilId,
  /// The supplied `volume` was the nil sentinel — a watch with no volume
  /// identity cannot be monitored.
  #[error("WatchedLocation volume must not be the nil UUID")]
  NilVolume,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use crate::domain::ErrorCode;

  #[test]
  fn try_new_happy_path() {
    let id = Uuid7::new();
    let vol = Uuid7::new();
    let w = WatchedLocation::try_new(id, vol, Timestamp::default())
      .expect("valid construction must succeed");
    assert_eq!(w.id(), &id);
    assert_eq!(w.volume(), &vol);
    assert!(!w.is_enabled(), "monitor starts paused");
    assert!(!w.is_recursive());
    assert!(!w.is_ejectable());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = WatchedLocation::try_new(Uuid7::nil(), Uuid7::new(), Timestamp::default());
    assert_eq!(r.err(), Some(WatchedLocationError::NilId));
    assert!(WatchedLocationError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_volume() {
    let r = WatchedLocation::try_new(Uuid7::new(), Uuid7::nil(), Timestamp::default());
    assert_eq!(r.err(), Some(WatchedLocationError::NilVolume));
    assert!(WatchedLocationError::NilVolume.is_nil_volume());
  }

  #[test]
  fn enabling_a_removable_drive_watch() {
    let vol = Uuid7::new();
    let w = WatchedLocation::try_new(Uuid7::new(), vol, Timestamp::default())
      .unwrap()
      .with_recursive(true)
      .with_enabled(true)
      .with_ejectable(true);
    assert!(w.is_ejectable() && w.is_recursive() && w.is_enabled());
  }

  #[test]
  fn non_ejectable_records_volume_unavailable_error() {
    let w = WatchedLocation::try_new(Uuid7::new(), Uuid7::new(), Timestamp::default())
      .unwrap()
      .with_last_error(Some(ErrorInfo::new(
        ErrorCode::VolumeNotAvailable,
        "drive offline",
      )));
    assert_eq!(
      w.last_error().map(|e| e.code()),
      Some(ErrorCode::VolumeNotAvailable)
    );
  }

  #[test]
  fn setters_mutate_in_place() {
    let mut w = WatchedLocation::try_new(Uuid7::new(), Uuid7::new(), Timestamp::default()).unwrap();
    w.set_enabled(true);
    w.set_recursive(true);
    w.set_ejectable(true);
    assert!(w.is_enabled() && w.is_recursive() && w.is_ejectable());
  }
}
