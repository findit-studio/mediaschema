//! `WatchedLocation` — a filesystem root the monitor watches for FS events.
//!
//! Locked `schema/watched_location.md` r5. **An event monitor, not a
//! scanner**: on a create/modify/delete/move event the monitor triggers a
//! (re)index of the affected file/subfolder. Content-addressed model ⇒ no
//! `Media` link, no media count, no scan rollup — "what exists" lives in
//! `Media` (by hash), not here. `is_ejectable` (from `whichdisk`) drives
//! whether a missing root is expected (transient pause) or a real
//! `VolumeNotAvailable` error.

use jiff::Timestamp;

use crate::domain::{primitives::LocationError, ErrorInfo, Location, ScanStatus, Uuid7};

/// A monitored source root.
///
/// Generic over `Id` (default [`Uuid7`]); `Location::Local.volume` flows the
/// same `Id` type. See `schema/watched_location.md` r5 for the full design.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WatchedLocation<Id = Uuid7> {
  /// Canonical identity.
  pub id: Id,
  /// The watched root (`Local`-only for now; structured object-storage
  /// support is deferred).
  pub root: Location<Id>,
  /// Descend subdirectories.
  pub recursive: bool,
  /// Actively monitored (vs paused).
  pub enabled: bool,
  /// Volume is removable / ejectable (from `whichdisk::is_ejectable`).
  /// Ejectable + absent ⇒ expected/transient (monitor pauses; reconcile
  /// on remount). Non-ejectable + absent ⇒ `last_error` =
  /// `VolumeNotAvailable` / `FolderNotAvailable`.
  pub is_ejectable: bool,
  /// When this watch was configured.
  pub added_at: Timestamp,
  /// Last full reconcile sweep (bootstrap / after-downtime / volume-remount
  /// catch-up — events the monitor missed while offline).
  pub last_reconciled_at: Option<Timestamp>,
  /// Status of that sweep.
  pub last_reconcile_status: Option<ScanStatus>,
  /// Monitor-health failure (e.g. `VolumeNotAvailable`,
  /// `FolderNotAvailable`, `LocalPermissionDenied`). The non-track error
  /// case — `WatchedLocation` is config + monitor health, not media.
  pub last_error: Option<ErrorInfo>,
}

// Note: no `Default for WatchedLocation`. Codex PR #11 round-1 finding
// #2: defaulting to `{ id: nil, root: Local { volume: nil, components:
// [] }, enabled: true(-ish) }` looked indistinguishable from a real
// missing-volume monitor entry — projection callers could persist that
// invalid root. Construct explicitly via `WatchedLocation::try_new` (or
// the struct literal for tests with non-nil ids), and let
// `Location::try_local_uuid7` enforce the non-nil/non-empty path
// invariant at the root.

impl WatchedLocation<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (every aggregate row needs a real identity) and
  /// any `LocationError` from the path validation (empty path / nil
  /// volume). All other fields take sensible non-enabled defaults
  /// (`recursive=false`, `enabled=false`, `is_ejectable=false`) — the
  /// caller flips them via field assignment after construction.
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

/// Error returned when [`WatchedLocation::try_new`] cannot uphold the
/// non-nil-id / valid-root invariants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[cfg(feature = "std")]
impl std::error::Error for WatchedLocationError {}

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
    assert_eq!(w.id, id);
    assert!(!w.enabled, "monitor starts paused");
    assert!(!w.recursive);
    assert!(!w.is_ejectable);
    match &w.root {
      Location::Local { volume, components } => {
        assert_eq!(*volume, vol);
        assert_eq!(components.as_slice(), &["Movies"]);
      }
    }
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = WatchedLocation::try_new(Uuid7::nil(), Uuid7::new(), ["Movies"], Timestamp::default());
    assert_eq!(r.err(), Some(WatchedLocationError::NilId));
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
    match r {
      Err(WatchedLocationError::Root(LocationError::EmptyPath)) => {}
      other => panic!("expected Root(EmptyPath), got {other:?}"),
    }
  }

  #[test]
  fn enabling_a_removable_drive_watch() {
    let vol = Uuid7::new();
    let mut w =
      WatchedLocation::try_new(Uuid7::new(), vol, ["Movies"], Timestamp::default()).unwrap();
    w.recursive = true;
    w.enabled = true;
    w.is_ejectable = true;
    assert!(w.is_ejectable && w.recursive && w.enabled);
  }

  #[test]
  fn non_ejectable_records_volume_unavailable_error() {
    let mut w =
      WatchedLocation::try_new(Uuid7::new(), Uuid7::new(), ["Movies"], Timestamp::default())
        .unwrap();
    w.last_error = Some(ErrorInfo::new(
      ErrorCode::VolumeNotAvailable,
      "drive offline",
    ));
    assert_eq!(
      w.last_error.as_ref().map(|e| e.code),
      Some(ErrorCode::VolumeNotAvailable)
    );
  }
}
