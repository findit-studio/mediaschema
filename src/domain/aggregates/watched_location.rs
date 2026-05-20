//! `WatchedLocation` — a filesystem root the monitor watches for FS events.
//!
//! Locked `schema/watched_location.md` r5. **An event monitor, not a
//! scanner**: on a create/modify/delete/move event the monitor triggers a
//! (re)index of the affected file/subfolder. Content-addressed model ⇒ no
//! `Media` link, no media count, no scan rollup — "what exists" lives in
//! `Media` (by hash), not here. `is_ejectable` (from `whichdisk`) drives
//! whether a missing root is expected (transient pause) or a real
//! `VolumeNotAvailable` error.

use derive_more::{IsVariant, TryUnwrap, Unwrap};
use jiff::Timestamp;

use crate::domain::{primitives::LocationError, ErrorInfo, Location, ScanStatus, Uuid7};

/// A monitored source root.
///
/// Generic over `Id` (default [`Uuid7`]); `Location::Local.volume` flows the
/// same `Id` type. See `schema/watched_location.md` r5 for the full design.
///
/// **No `Default`** — defaulting to `{ id: nil, root: Local(nil_volume, []) }`
/// would be indistinguishable from a real missing-volume monitor entry.
/// Construct via [`WatchedLocation::try_new`] (the `Uuid7` validating
/// builder). Fields are private; access via the `with_*` / `set_*`
/// builders + getters listed below.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WatchedLocation<Id = Uuid7> {
  id: Id,
  root: Location<Id>,
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
  /// Rejects nil `id` (every aggregate row needs a real identity) and
  /// any `LocationError` from the path validation (empty path / nil
  /// volume). All other fields take sensible non-enabled defaults
  /// (`recursive=false`, `enabled=false`, `is_ejectable=false`) — the
  /// caller flips them via `with_*` / `set_*` after construction.
  pub fn try_new<I, S>(
    id: Uuid7,
    volume: Uuid7,
    path: I,
    added_at: Timestamp,
  ) -> Result<Self, WatchedLocationError>
  where
    I: IntoIterator<Item = S>,
    S: Into<smol_str::SmolStr>,
  {
    if id.is_nil() {
      return Err(WatchedLocationError::NilId);
    }
    let root = Location::try_local_uuid7(volume, path).map_err(WatchedLocationError::Root)?;
    Ok(Self {
      id,
      root,
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
  #[inline]
  pub const fn id(&self) -> &Id {
    &self.id
  }

  /// Watched root (`Local`-only for now; structured object-storage
  /// support is deferred).
  #[inline]
  pub const fn root(&self) -> &Location<Id> {
    &self.root
  }

  /// Descend subdirectories.
  #[inline]
  pub const fn is_recursive(&self) -> bool {
    self.recursive
  }

  /// Actively monitored (vs paused).
  #[inline]
  pub const fn is_enabled(&self) -> bool {
    self.enabled
  }

  /// Volume is removable / ejectable (from `whichdisk::is_ejectable`).
  /// Ejectable + absent ⇒ expected/transient (monitor pauses; reconcile
  /// on remount). Non-ejectable + absent ⇒ `last_error` =
  /// `VolumeNotAvailable` / `FolderNotAvailable`.
  #[inline]
  pub const fn is_ejectable(&self) -> bool {
    self.is_ejectable
  }

  /// When this watch was configured.
  #[inline]
  pub const fn added_at(&self) -> &Timestamp {
    &self.added_at
  }

  /// Last full reconcile sweep (bootstrap / after-downtime /
  /// volume-remount catch-up — events the monitor missed while offline).
  #[inline]
  pub const fn last_reconciled_at(&self) -> Option<&Timestamp> {
    self.last_reconciled_at.as_ref()
  }

  /// Status of that sweep.
  #[inline]
  pub const fn last_reconcile_status(&self) -> Option<&ScanStatus> {
    self.last_reconcile_status.as_ref()
  }

  /// Monitor-health failure (e.g. `VolumeNotAvailable`,
  /// `FolderNotAvailable`, `LocalPermissionDenied`). The non-track error
  /// case — `WatchedLocation` is config + monitor health, not media.
  #[inline]
  pub const fn last_error(&self) -> Option<&ErrorInfo> {
    self.last_error.as_ref()
  }

  /// Builder: replace `recursive` flag.
  #[inline]
  pub const fn with_recursive(mut self, recursive: bool) -> Self {
    self.recursive = recursive;
    self
  }

  /// Builder: replace `enabled` flag.
  #[inline]
  pub const fn with_enabled(mut self, enabled: bool) -> Self {
    self.enabled = enabled;
    self
  }

  /// Builder: replace `is_ejectable` flag.
  #[inline]
  pub const fn with_ejectable(mut self, is_ejectable: bool) -> Self {
    self.is_ejectable = is_ejectable;
    self
  }

  /// Builder: replace `last_reconciled_at`.
  #[inline]
  pub fn with_last_reconciled_at(mut self, t: Option<Timestamp>) -> Self {
    self.last_reconciled_at = t;
    self
  }

  /// Builder: replace `last_reconcile_status`.
  #[inline]
  pub fn with_last_reconcile_status(mut self, s: Option<ScanStatus>) -> Self {
    self.last_reconcile_status = s;
    self
  }

  /// Builder: replace `last_error`.
  #[inline]
  pub fn with_last_error(mut self, e: Option<ErrorInfo>) -> Self {
    self.last_error = e;
    self
  }

  /// In-place mutator for `recursive`.
  #[inline]
  pub const fn set_recursive(&mut self, recursive: bool) {
    self.recursive = recursive;
  }

  /// In-place mutator for `enabled`.
  #[inline]
  pub const fn set_enabled(&mut self, enabled: bool) {
    self.enabled = enabled;
  }

  /// In-place mutator for `is_ejectable`.
  #[inline]
  pub const fn set_ejectable(&mut self, is_ejectable: bool) {
    self.is_ejectable = is_ejectable;
  }

  /// In-place mutator for `last_reconciled_at`.
  #[inline]
  pub fn set_last_reconciled_at(&mut self, t: Option<Timestamp>) {
    self.last_reconciled_at = t;
  }

  /// In-place mutator for `last_reconcile_status`.
  #[inline]
  pub fn set_last_reconcile_status(&mut self, s: Option<ScanStatus>) {
    self.last_reconcile_status = s;
  }

  /// In-place mutator for `last_error`.
  #[inline]
  pub fn set_last_error(&mut self, e: Option<ErrorInfo>) {
    self.last_error = e;
  }
}

/// Error returned when [`WatchedLocation::try_new`] cannot uphold the
/// non-nil-id / valid-root invariants. Newtype variant wrapping
/// [`LocationError`] for root validation; derives `IsVariant` plus
/// `Unwrap`/`TryUnwrap` with shared-ref + mut-ref accessor flavours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, Unwrap, TryUnwrap)]
#[unwrap(ref, ref_mut)]
#[try_unwrap(ref, ref_mut)]
#[non_exhaustive]
pub enum WatchedLocationError {
  /// The supplied `id` was the nil sentinel — not a real identity.
  NilId,
  /// Root construction failed; see the inner [`LocationError`].
  Root(LocationError),
}

impl core::fmt::Display for WatchedLocationError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::NilId => f.write_str("WatchedLocation id must not be the nil UUID"),
      Self::Root(e) => write!(f, "WatchedLocation root invalid: {e}"),
    }
  }
}

impl core::error::Error for WatchedLocationError {
  fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
    match self {
      Self::Root(e) => Some(e),
      _ => None,
    }
  }
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
    let w = WatchedLocation::try_new(id, vol, ["Movies"], Timestamp::default())
      .expect("valid construction must succeed");
    assert_eq!(w.id(), &id);
    assert!(!w.is_enabled(), "monitor starts paused");
    assert!(!w.is_recursive());
    assert!(!w.is_ejectable());
    // root() returns &Location; use the IsVariant + Unwrap derives.
    assert!(w.root().is_local());
    let local = w.root().unwrap_local_ref();
    assert_eq!(local.volume(), &vol);
    assert_eq!(local.components(), &["Movies"]);
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = WatchedLocation::try_new(Uuid7::nil(), Uuid7::new(), ["Movies"], Timestamp::default());
    assert_eq!(r.err(), Some(WatchedLocationError::NilId));
    assert!(WatchedLocationError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_invalid_root() {
    // Empty path → LocationError::EmptyPath flows through Root().
    let r = WatchedLocation::try_new::<core::iter::Empty<&str>, &str>(
      Uuid7::new(),
      Uuid7::new(),
      core::iter::empty(),
      Timestamp::default(),
    );
    let err = r.unwrap_err();
    assert!(err.is_root());
    assert_eq!(err.try_unwrap_root_ref(), Ok(&LocationError::EmptyPath));
  }

  #[test]
  fn enabling_a_removable_drive_watch() {
    let vol = Uuid7::new();
    let w = WatchedLocation::try_new(Uuid7::new(), vol, ["Movies"], Timestamp::default())
      .unwrap()
      .with_recursive(true)
      .with_enabled(true)
      .with_ejectable(true);
    assert!(w.is_ejectable() && w.is_recursive() && w.is_enabled());
  }

  #[test]
  fn non_ejectable_records_volume_unavailable_error() {
    let w = WatchedLocation::try_new(Uuid7::new(), Uuid7::new(), ["Movies"], Timestamp::default())
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
    let mut w =
      WatchedLocation::try_new(Uuid7::new(), Uuid7::new(), ["Movies"], Timestamp::default())
        .unwrap();
    w.set_enabled(true);
    w.set_recursive(true);
    w.set_ejectable(true);
    assert!(w.is_enabled() && w.is_recursive() && w.is_ejectable());
  }
}
