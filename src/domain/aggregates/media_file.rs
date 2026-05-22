//! `MediaFile<Id>` — one physical copy of a piece of content
//! (new `schema/media_file.md`).
//!
//! The content/copy split: a [`Media`](crate::domain::Media) row is the
//! **content** (one per content hash), and a `MediaFile` is one **physical
//! copy** of that content on disk. Many copies of the same bytes (the same
//! file duplicated across folders/volumes) collapse to **one** `Media` but
//! keep **N** distinct `MediaFile`s — each carries its own filesystem
//! identity (name, path, creation time, discovering watch).
//!
//! ## Cross-cutting (locked conventions)
//!
//! - Generic over `Id` (default [`Uuid7`]); `media_id` /
//!   `watched_location_id` FKs are the same UUIDv7.
//! - The copy's file name is **derived** from `location` — it is the last
//!   path component, not a stored field, so name and path can never
//!   desync. Renaming/moving a file is replacing `location`.
//! - `created_at` is the **filesystem** creation time
//!   ([`jiff::Timestamp`], ms-resolution) — copy-specific, distinct from
//!   the content's intrinsic metadata. **Optional**: many filesystems
//!   lack a birth time. The domain stores the supplied `Option`
//!   faithfully — `Some(epoch)` is preserved distinctly from `None`;
//!   translating the legacy wire `0` (Unix epoch, ms) sentinel to `None`
//!   is the wire-decode adapter's responsibility, not the domain's.
//! - `location` is the structured [`Location`] (`Local { volume,
//!   components }`) where this copy lives — volume-aware, not a path
//!   string.
//! - `watched_location_id` is **non-optional**: every copy enters the
//!   index via a [`WatchedLocation`](crate::domain::WatchedLocation) scan,
//!   and WL deletion cascades to its files, so the FK is never dangling.
//! - **Volume consistency**: a `MediaFile`'s `location` must sit on the
//!   *same volume* as the [`WatchedLocation`](crate::domain::WatchedLocation)
//!   that discovered it (the watch is volume-scoped). `try_new` takes the
//!   `WatchedLocation` itself (not a bare id) so it can verify
//!   `WatchedLocation::volume_ref() == location.volume_ref()`. The watch volume is
//!   stored alongside `watched_location_id` so the location setters can
//!   re-check the invariant — hence `set_location` / `with_location` are
//!   **fallible** (`try_set_location` / `try_with_location`).
//!
//! ## Encapsulation
//!
//! No public fields. Access via getters (`const fn` where possible);
//! mutation via `with_*` builders and `set_*` setters. `try_new` is the
//! validating constructor; nil `id`, nil `media_id`, nil
//! `watched_location_id`, and a cross-volume `location`/watch mismatch are
//! rejected.

use derive_more::IsVariant;
use jiff::Timestamp as JiffTimestamp;

use crate::domain::{Location, Uuid7, WatchedLocation};

/// One physical copy of a piece of content (N copies ↔ 1 `Media`).
///
/// **No `Default` impl** — defaulting to `{ id: nil, media_id: nil,
/// watched_location_id: nil, … }` would represent an orphan copy with no
/// real identity and no content/discoverer to attach to. Construct via
/// [`MediaFile::try_new`] (validating: rejects all three nil ids).
///
/// **Fields are private**; access via getters and `with_*` / `set_*`
/// mutators per the encapsulation rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MediaFile<Id = Uuid7> {
  id: Id,
  /// FK → the shared [`Media`](crate::domain::Media) **content** row
  /// (one per content hash; many copies share it).
  media_id: Id,
  /// Filesystem creation time. **Optional**: many filesystems lack a
  /// birth time. Stored faithfully — `Some(epoch)` is preserved; the
  /// legacy wire `0` sentinel is collapsed to `None` by the wire-decode
  /// adapter, not here.
  created_at: Option<JiffTimestamp>,
  /// Structured `Local { volume, components }` — where this copy lives.
  /// The file name is derived from this (the last path component).
  location: Location<Id>,
  /// FK → the [`WatchedLocation`](crate::domain::WatchedLocation) that
  /// discovered this copy. **Non-optional**: every file enters the index
  /// via a WL scan, and WL deletion cascades to its files, so the FK is
  /// never dangling.
  watched_location_id: Id,
  /// Cached volume identity of the discovering `WatchedLocation` (its
  /// volume-scoped `volume`). Not a separate FK — it duplicates the
  /// watch's `volume` purely so the location setters can re-check the
  /// volume-consistency invariant (`location.volume_ref() == watch.volume_ref()`)
  /// without holding a reference to the watch. Set once at construction
  /// from the `WatchedLocation` passed to `try_new`.
  watch_volume: Id,
}

impl MediaFile<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (every copy must have a real identity), nil
  /// `media_id` (a copy with no content row is an orphan), and nil
  /// `watched_location_id` (a copy is always discovered through a watch).
  ///
  /// Takes the discovering [`WatchedLocation`] **by reference** rather than
  /// a bare id so the volume-consistency invariant can be enforced: the
  /// `location`'s volume must equal the watch's (volume-scoped) `volume`.
  /// A cross-volume pairing is rejected with
  /// [`MediaFileError::VolumeMismatch`]. Only the watch's `id` and `volume`
  /// are retained — no reference to the `WatchedLocation` is kept.
  pub fn try_new(
    id: Uuid7,
    media_id: Uuid7,
    created_at: Option<JiffTimestamp>,
    location: Location<Uuid7>,
    watched_location: &WatchedLocation<Uuid7>,
  ) -> Result<Self, MediaFileError> {
    if id.is_nil() {
      return Err(MediaFileError::NilId);
    }
    if media_id.is_nil() {
      return Err(MediaFileError::NilMediaId);
    }
    let watched_location_id = *watched_location.id_ref();
    if watched_location_id.is_nil() {
      return Err(MediaFileError::NilWatchedLocationId);
    }
    let watch_volume = *watched_location.volume_ref();
    if location_volume(&location) != &watch_volume {
      return Err(MediaFileError::VolumeMismatch);
    }
    Ok(Self {
      id,
      media_id,
      created_at,
      location,
      watched_location_id,
      watch_volume,
    })
  }
}

impl<Id> MediaFile<Id> {
  /// Raw constructor for **storage / wire reconstruction** — assembles a
  /// `MediaFile` directly from its persisted fields, bypassing the
  /// `WatchedLocation` indirection and the cross-volume re-validation that
  /// [`Self::try_new`] performs. Intended ONLY for backends rebuilding a
  /// `MediaFile` from a trusted persisted row/document (the data was
  /// validated by `try_new` when first written). Application code building
  /// a fresh `MediaFile` must use [`Self::try_new`].
  #[inline(always)]
  #[must_use]
  pub const fn from_parts(
    id: Id,
    media_id: Id,
    created_at: Option<JiffTimestamp>,
    location: Location<Id>,
    watched_location_id: Id,
    watch_volume: Id,
  ) -> Self {
    Self {
      id,
      media_id,
      created_at,
      location,
      watched_location_id,
      watch_volume,
    }
  }

  /// Canonical identity (the copy's key).
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// FK → the shared `Media` content row.
  #[inline(always)]
  pub const fn media_id_ref(&self) -> &Id {
    &self.media_id
  }

  /// File name — **derived** from `location` (its last path component),
  /// never a stored field, so it can never desync from the path.
  #[inline]
  pub fn name(&self) -> &str {
    // `Location` is `#[non_exhaustive]` but defined in this crate, so the
    // match stays exhaustive; future variants (remote URLs / object
    // storage) must add a name-derivation arm here when introduced.
    match &self.location {
      Location::Local(local) => local.file_name(),
    }
  }

  /// Filesystem creation time (wall-clock, ms-resolution). `None` when the
  /// filesystem has no birth time.
  #[inline(always)]
  pub const fn created_at_ref(&self) -> Option<&JiffTimestamp> {
    self.created_at.as_ref()
  }

  /// Structured location where this copy lives.
  #[inline(always)]
  pub const fn location_ref(&self) -> &Location<Id> {
    &self.location
  }

  /// FK → the `WatchedLocation` that discovered this copy.
  #[inline(always)]
  pub const fn watched_location_id_ref(&self) -> &Id {
    &self.watched_location_id
  }

  /// Volume identity of the discovering `WatchedLocation` — the volume
  /// every valid `location` of this copy must sit on.
  #[inline(always)]
  pub const fn watch_volume_ref(&self) -> &Id {
    &self.watch_volume
  }

  // --- builders -----------------------------------------------------------

  /// Builder: replace `created_at`. Stores the supplied `Option`
  /// faithfully — see [`MediaFile::set_created_at`].
  #[inline(always)]
  #[must_use]
  pub const fn with_created_at(mut self, t: Option<JiffTimestamp>) -> Self {
    self.created_at = t;
    self
  }

  /// Builder: replace `location`, re-checking volume consistency. This is
  /// also the single atomic rename/move API — the derived
  /// [`name`](MediaFile::name) follows.
  ///
  /// **Fallible**: the new `location` must stay on the same volume as the
  /// discovering watch ([`watch_volume_ref`](MediaFile::watch_volume_ref)); a
  /// cross-volume move is rejected with [`MediaFileError::VolumeMismatch`].
  /// Moving a copy to a different volume is a *new copy under a different
  /// watch*, not a mutation of this record.
  #[inline]
  pub fn try_with_location(mut self, location: Location<Id>) -> Result<Self, MediaFileError>
  where
    Id: PartialEq,
  {
    self.try_set_location(location)?;
    Ok(self)
  }

  /// Builder: replace the `Media` content FK.
  #[inline(always)]
  #[must_use]
  pub fn with_media_id(mut self, media_id: Id) -> Self {
    self.media_id = media_id;
    self
  }

  /// Builder: re-point this copy at a different discovering
  /// `WatchedLocation`.
  ///
  /// Takes the `WatchedLocation` (not a bare id) and is **fallible**: the
  /// new watch must be on the same volume as this copy's current
  /// `location` — re-pointing a copy at a watch for a different volume is
  /// rejected with [`MediaFileError::VolumeMismatch`]. Updates both
  /// `watched_location_id` and the cached `watch_volume`.
  #[inline]
  pub fn try_with_watched_location(
    mut self,
    watched_location: &WatchedLocation<Id>,
  ) -> Result<Self, MediaFileError>
  where
    Id: Clone + PartialEq,
  {
    self.try_set_watched_location(watched_location)?;
    Ok(self)
  }

  // --- in-place setters ---------------------------------------------------

  /// In-place mutator for `created_at`.
  ///
  /// The supplied `Option` is stored **faithfully** — `Some(epoch)` is a
  /// real timestamp and is preserved distinctly from `None`. Translating
  /// the legacy wire `0` (Unix epoch, ms) sentinel to `None` is the
  /// responsibility of the wire-decode adapter, not the domain.
  #[inline(always)]
  pub const fn set_created_at(&mut self, t: Option<JiffTimestamp>) -> &mut Self {
    self.created_at = t;
    self
  }

  /// In-place mutator for `location` — also the single atomic rename/move
  /// API; the derived [`name`](MediaFile::name) follows.
  ///
  /// **Fallible**: the new `location` must stay on the same volume as the
  /// discovering watch ([`watch_volume_ref`](MediaFile::watch_volume_ref)); a
  /// cross-volume move is rejected with [`MediaFileError::VolumeMismatch`]
  /// and `self` is left unchanged.
  #[inline]
  pub fn try_set_location(&mut self, location: Location<Id>) -> Result<&mut Self, MediaFileError>
  where
    Id: PartialEq,
  {
    if location_volume(&location) != &self.watch_volume {
      return Err(MediaFileError::VolumeMismatch);
    }
    self.location = location;
    Ok(self)
  }

  /// In-place mutator for the `Media` content FK.
  #[inline(always)]
  pub fn set_media_id(&mut self, media_id: Id) -> &mut Self {
    self.media_id = media_id;
    self
  }

  /// In-place mutator: re-point this copy at a different discovering
  /// `WatchedLocation`.
  ///
  /// Takes the `WatchedLocation` (not a bare id) and is **fallible**: the
  /// new watch must be on the same volume as this copy's current
  /// `location` — re-pointing at a watch for a different volume is
  /// rejected with [`MediaFileError::VolumeMismatch`] and `self` is left
  /// unchanged. Updates both `watched_location_id` and the cached
  /// `watch_volume`.
  #[inline]
  pub fn try_set_watched_location(
    &mut self,
    watched_location: &WatchedLocation<Id>,
  ) -> Result<&mut Self, MediaFileError>
  where
    Id: Clone + PartialEq,
  {
    if location_volume(&self.location) != watched_location.volume_ref() {
      return Err(MediaFileError::VolumeMismatch);
    }
    self.watched_location_id = watched_location.id_ref().clone();
    self.watch_volume = watched_location.volume_ref().clone();
    Ok(self)
  }
}

/// Project the volume identity out of a [`Location`].
///
/// `Location` is `#[non_exhaustive]` but defined in this crate, so the
/// match stays exhaustive; future variants (remote URLs / object storage)
/// must add a volume-projection arm here when introduced.
#[inline]
fn location_volume<Id>(location: &Location<Id>) -> &Id {
  match location {
    Location::Local(local) => local.volume_ref(),
  }
}

/// Error returned when [`MediaFile::try_new`] cannot uphold the
/// non-nil-id invariants. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum MediaFileError {
  /// Supplied `id` was the [`Uuid7`] nil sentinel — not a real identity.
  #[error("MediaFile id must not be the nil UUID")]
  NilId,
  /// Supplied `media_id` was the nil sentinel — a copy with no `Media`
  /// content row is an orphan.
  #[error("MediaFile media_id (Media) must not be the nil UUID")]
  NilMediaId,
  /// Supplied `watched_location_id` was the nil sentinel — every copy is
  /// discovered through a `WatchedLocation` scan.
  #[error("MediaFile watched_location_id must not be the nil UUID")]
  NilWatchedLocationId,
  /// The `location`'s volume did not match the discovering
  /// `WatchedLocation`'s (volume-scoped) volume. A copy is always on the
  /// volume its watch monitors; a cross-volume pairing is a different
  /// copy under a different watch.
  #[error("MediaFile location volume must match its WatchedLocation volume")]
  VolumeMismatch,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  /// A representative non-epoch creation timestamp.
  fn real_ts() -> JiffTimestamp {
    JiffTimestamp::from_millisecond(1_700_000_000_000).expect("valid timestamp")
  }

  fn loc(volume: Uuid7) -> Location<Uuid7> {
    Location::try_local_uuid7(volume, ["Movies", "clip.mp4"]).expect("valid location")
  }

  /// Build a location whose last component is `name`.
  fn named_loc(volume: Uuid7, name: &str) -> Location<Uuid7> {
    Location::try_local_uuid7(volume, ["Movies", name]).expect("valid location")
  }

  /// A `WatchedLocation` monitoring `volume`.
  fn watch(volume: Uuid7) -> WatchedLocation<Uuid7> {
    WatchedLocation::try_new(Uuid7::new(), volume, JiffTimestamp::default()).expect("valid watch")
  }

  #[test]
  fn try_new_happy_path() {
    let id = Uuid7::new();
    let media_id = Uuid7::new();
    let vol = Uuid7::new();
    let wl = watch(vol);
    let f = MediaFile::try_new(id, media_id, Some(real_ts()), loc(vol), &wl)
      .expect("valid construction must succeed");
    assert_eq!(f.id_ref(), &id);
    assert_eq!(f.media_id_ref(), &media_id);
    // name is derived from the location's last component.
    assert_eq!(f.name(), "clip.mp4");
    assert_eq!(f.created_at_ref(), Some(&real_ts()));
    assert_eq!(f.watched_location_id_ref(), wl.id_ref());
    assert_eq!(f.watch_volume_ref(), &vol);
    // location() projects &Location; use the IsVariant + Unwrap derives.
    assert!(f.location_ref().is_local());
    let local = f.location_ref().unwrap_local_ref();
    assert_eq!(local.volume_ref(), &vol);
    assert_eq!(local.components_slice(), &["Movies", "clip.mp4"]);
  }

  #[test]
  fn from_parts_round_trips_a_validated_instance() {
    // `from_parts` is the storage-reconstruction constructor: rebuilding a
    // `MediaFile` from a validated instance's raw fields must yield an
    // identical aggregate.
    let vol = Uuid7::new();
    let wl = watch(vol);
    let original = MediaFile::try_new(Uuid7::new(), Uuid7::new(), Some(real_ts()), loc(vol), &wl)
      .expect("valid construction must succeed");
    let rebuilt = MediaFile::from_parts(
      *original.id_ref(),
      *original.media_id_ref(),
      original.created_at_ref().copied(),
      original.location_ref().clone(),
      *original.watched_location_id_ref(),
      *original.watch_volume_ref(),
    );
    assert_eq!(rebuilt, original);
  }

  #[test]
  fn name_is_derived_from_location() {
    let vol = Uuid7::new();
    let wl = watch(vol);
    let f = MediaFile::try_new(
      Uuid7::new(),
      Uuid7::new(),
      None,
      named_loc(vol, "holiday.mkv"),
      &wl,
    )
    .unwrap();
    assert_eq!(f.name(), "holiday.mkv");
    // Replacing the location (same volume) atomically renames/moves the file.
    let f = f
      .try_with_location(named_loc(vol, "renamed.mp4"))
      .expect("same-volume rename must succeed");
    assert_eq!(f.name(), "renamed.mp4");
  }

  #[test]
  fn created_at_stored_faithfully() {
    // The domain preserves `Some(epoch)` distinctly from `None`. Collapsing
    // the legacy wire `0` sentinel is the wire-decode adapter's job, not the
    // domain's — see `MediaFile::with_created_at`.
    let epoch = JiffTimestamp::default();
    let vol = Uuid7::new();
    let wl = watch(vol);
    let f = MediaFile::try_new(Uuid7::new(), Uuid7::new(), Some(epoch), loc(vol), &wl).unwrap();
    assert_eq!(
      f.created_at_ref(),
      Some(&epoch),
      "Some(epoch) must be preserved faithfully"
    );

    // An explicit `None` stays `None`.
    let f = f.with_created_at(None);
    assert!(f.created_at_ref().is_none());

    // A real instant is preserved too.
    let f = f.with_created_at(Some(real_ts()));
    assert_eq!(f.created_at_ref(), Some(&real_ts()));

    // The in-place setter stores faithfully and identically.
    let mut f = f;
    f.set_created_at(Some(epoch));
    assert_eq!(f.created_at_ref(), Some(&epoch));
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let vol = Uuid7::new();
    let r = MediaFile::try_new(
      Uuid7::nil(),
      Uuid7::new(),
      Some(real_ts()),
      loc(vol),
      &watch(vol),
    );
    assert_eq!(r.err(), Some(MediaFileError::NilId));
    assert!(MediaFileError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_media_id() {
    let vol = Uuid7::new();
    let r = MediaFile::try_new(
      Uuid7::new(),
      Uuid7::nil(),
      Some(real_ts()),
      loc(vol),
      &watch(vol),
    );
    assert_eq!(r.err(), Some(MediaFileError::NilMediaId));
    assert!(MediaFileError::NilMediaId.is_nil_media_id());
  }

  #[test]
  fn try_new_rejects_cross_volume_watch() {
    // The file lives on `file_vol`; the watch monitors a *different*
    // volume — `try_new` must reject the pairing.
    let file_vol = Uuid7::new();
    let watch_vol = Uuid7::new();
    let r = MediaFile::try_new(
      Uuid7::new(),
      Uuid7::new(),
      Some(real_ts()),
      loc(file_vol),
      &watch(watch_vol),
    );
    assert_eq!(r.err(), Some(MediaFileError::VolumeMismatch));
    assert!(MediaFileError::VolumeMismatch.is_volume_mismatch());
  }

  #[test]
  fn try_set_location_rejects_cross_volume_move() {
    let vol = Uuid7::new();
    let wl = watch(vol);
    let mut f = MediaFile::try_new(
      Uuid7::new(),
      Uuid7::new(),
      None,
      named_loc(vol, "clip.mp4"),
      &wl,
    )
    .unwrap();
    // Moving to a different volume is rejected and leaves `self` unchanged.
    let other_vol = Uuid7::new();
    let r = f.try_set_location(named_loc(other_vol, "moved.mp4"));
    assert_eq!(r, Err(MediaFileError::VolumeMismatch));
    assert_eq!(f.name(), "clip.mp4", "rejected move must not mutate self");
    assert_eq!(f.location_ref().unwrap_local_ref().volume_ref(), &vol);
    // A same-volume move succeeds.
    f.try_set_location(named_loc(vol, "renamed.mp4"))
      .expect("same-volume move must succeed");
    assert_eq!(f.name(), "renamed.mp4");

    // The builder form rejects the same cross-volume move.
    let r = f.clone().try_with_location(named_loc(other_vol, "x.mp4"));
    assert_eq!(r.err(), Some(MediaFileError::VolumeMismatch));
  }

  #[test]
  fn try_set_watched_location_rejects_cross_volume_watch() {
    let vol = Uuid7::new();
    let wl = watch(vol);
    let mut f = MediaFile::try_new(
      Uuid7::new(),
      Uuid7::new(),
      None,
      named_loc(vol, "clip.mp4"),
      &wl,
    )
    .unwrap();
    // Re-pointing at a watch on a different volume is rejected.
    let other = watch(Uuid7::new());
    let r = f.try_set_watched_location(&other);
    assert_eq!(r, Err(MediaFileError::VolumeMismatch));
    assert_eq!(
      f.watched_location_id_ref(),
      wl.id_ref(),
      "rejected re-point must not mutate"
    );
    // Re-pointing at another watch on the *same* volume succeeds.
    let same_vol_watch = watch(vol);
    f.try_set_watched_location(&same_vol_watch)
      .expect("same-volume re-point must succeed");
    assert_eq!(f.watched_location_id_ref(), same_vol_watch.id_ref());
    assert_eq!(f.watch_volume_ref(), &vol);
  }

  #[test]
  fn builders_chain() {
    let media_id = Uuid7::new();
    let vol = Uuid7::new();
    let wl = watch(vol);
    let f = MediaFile::try_new(
      Uuid7::new(),
      Uuid7::new(),
      None,
      named_loc(vol, "old.mp4"),
      &wl,
    )
    .unwrap()
    .with_media_id(media_id)
    .try_with_location(named_loc(vol, "new.mkv"))
    .unwrap()
    .with_created_at(Some(real_ts()));
    assert_eq!(f.name(), "new.mkv");
    assert_eq!(f.media_id_ref(), &media_id);
    assert_eq!(f.created_at_ref(), Some(&real_ts()));
    assert_eq!(f.watched_location_id_ref(), wl.id_ref());
    assert_eq!(f.location_ref().unwrap_local_ref().volume_ref(), &vol);
  }

  #[test]
  fn setters_mutate_in_place() {
    let vol = Uuid7::new();
    let wl = watch(vol);
    let mut f = MediaFile::try_new(
      Uuid7::new(),
      Uuid7::new(),
      Some(real_ts()),
      named_loc(vol, "clip.mp4"),
      &wl,
    )
    .unwrap();
    let media_id = Uuid7::new();
    f.set_media_id(media_id);
    f.try_set_location(named_loc(vol, "renamed.mp4")).unwrap();
    f.set_created_at(None);
    assert_eq!(f.name(), "renamed.mp4");
    assert_eq!(f.media_id_ref(), &media_id);
    assert!(f.created_at_ref().is_none());
    assert_eq!(f.location_ref().unwrap_local_ref().volume_ref(), &vol);
  }
}
