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

use crate::domain::{ErrorInfo, Location, ScanStatus, Uuid7};

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

impl<Id: Default> Default for WatchedLocation<Id> {
    fn default() -> Self {
        Self {
            id: Id::default(),
            root: Location::default(),
            recursive: false,
            enabled: false,
            is_ejectable: false,
            added_at: Timestamp::default(),
            last_reconciled_at: None,
            last_reconcile_status: None,
            last_error: None,
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ErrorCode;

    #[test]
    fn default_is_paused_empty_root() {
        let w: WatchedLocation = WatchedLocation::default();
        assert!(!w.enabled);
        assert!(!w.recursive);
        assert!(!w.is_ejectable);
        assert!(w.last_reconciled_at.is_none());
        assert!(w.last_reconcile_status.is_none());
        assert!(w.last_error.is_none());
        match w.root {
            Location::Local { volume, components } => {
                assert!(volume.is_nil());
                assert!(components.is_empty());
            }
        }
    }

    #[test]
    fn construct_a_removable_drive_watch() {
        let vol = Uuid7::new();
        let w: WatchedLocation = WatchedLocation {
            id: Uuid7::new(),
            root: Location::local(vol, ["Movies"]),
            recursive: true,
            enabled: true,
            is_ejectable: true,
            added_at: Timestamp::default(),
            last_reconciled_at: None,
            last_reconcile_status: None,
            last_error: None,
        };
        assert!(w.is_ejectable);
        assert!(w.recursive);
        assert!(w.enabled);
        match &w.root {
            Location::Local { volume, components } => {
                assert_eq!(*volume, vol);
                assert_eq!(components.as_slice(), &["Movies"]);
            }
        }
    }

    #[test]
    fn non_ejectable_records_volume_unavailable_error() {
        let w: WatchedLocation = WatchedLocation {
            is_ejectable: false,
            last_error: Some(ErrorInfo::new(ErrorCode::VolumeNotAvailable, "drive offline")),
            ..WatchedLocation::default()
        };
        assert_eq!(
            w.last_error.as_ref().map(|e| e.code),
            Some(ErrorCode::VolumeNotAvailable)
        );
    }
}
