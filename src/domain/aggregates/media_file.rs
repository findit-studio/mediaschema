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
//!   lack a birth time, and the wire encodes `0` (Unix epoch, ms) as
//!   absent, so a 0-ms timestamp is normalised to `None`.
//! - `location` is the structured [`Location`] (`Local { volume,
//!   components }`) where this copy lives — volume-aware, not a path
//!   string.
//! - `watched_location_id` is **non-optional**: every copy enters the
//!   index via a [`WatchedLocation`](crate::domain::WatchedLocation) scan,
//!   and WL deletion cascades to its files, so the FK is never dangling.
//!
//! ## Encapsulation
//!
//! No public fields. Access via getters (`const fn` where possible);
//! mutation via `with_*` builders and `set_*` setters. `try_new` is the
//! validating constructor; nil `id`, nil `media_id`, and nil
//! `watched_location_id` are rejected.

use derive_more::IsVariant;
use jiff::Timestamp as JiffTimestamp;

use crate::domain::{Location, Uuid7};

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
  /// birth time, and the wire encodes `0` (Unix epoch, ms) as absent.
  created_at: Option<JiffTimestamp>,
  /// Structured `Local { volume, components }` — where this copy lives.
  /// The file name is derived from this (the last path component).
  location: Location<Id>,
  /// FK → the [`WatchedLocation`](crate::domain::WatchedLocation) that
  /// discovered this copy. **Non-optional**: every file enters the index
  /// via a WL scan, and WL deletion cascades to its files, so the FK is
  /// never dangling.
  watched_location_id: Id,
}

impl MediaFile<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (every copy must have a real identity), nil
  /// `media_id` (a copy with no content row is an orphan), and nil
  /// `watched_location_id` (a copy is always discovered through a watch).
  pub fn try_new(
    id: Uuid7,
    media_id: Uuid7,
    created_at: Option<JiffTimestamp>,
    location: Location<Uuid7>,
    watched_location_id: Uuid7,
  ) -> Result<Self, MediaFileError> {
    if id.is_nil() {
      return Err(MediaFileError::NilId);
    }
    if media_id.is_nil() {
      return Err(MediaFileError::NilMediaId);
    }
    if watched_location_id.is_nil() {
      return Err(MediaFileError::NilWatchedLocationId);
    }
    Ok(Self {
      id,
      media_id,
      created_at: normalize_created_at(created_at),
      location,
      watched_location_id,
    })
  }
}

impl<Id> MediaFile<Id> {
  /// Canonical identity (the copy's key).
  #[inline]
  pub const fn id(&self) -> &Id {
    &self.id
  }

  /// FK → the shared `Media` content row.
  #[inline]
  pub const fn media_id(&self) -> &Id {
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
  /// filesystem has no birth time (or the wire `0`-ms sentinel was given).
  #[inline]
  pub const fn created_at(&self) -> Option<&JiffTimestamp> {
    self.created_at.as_ref()
  }

  /// Structured location where this copy lives.
  #[inline]
  pub const fn location(&self) -> &Location<Id> {
    &self.location
  }

  /// FK → the `WatchedLocation` that discovered this copy.
  #[inline]
  pub const fn watched_location_id(&self) -> &Id {
    &self.watched_location_id
  }

  // --- builders -----------------------------------------------------------

  /// Builder: replace `created_at`, collapsing the 0-ms wire sentinel to
  /// `None` (see [`MediaFile::set_created_at`]).
  #[inline]
  pub fn with_created_at(mut self, t: Option<JiffTimestamp>) -> Self {
    self.created_at = normalize_created_at(t);
    self
  }

  /// Builder: replace `location`. This is also the single atomic
  /// rename/move API — the derived [`name`](MediaFile::name) follows.
  #[inline]
  pub fn with_location(mut self, location: Location<Id>) -> Self {
    self.location = location;
    self
  }

  /// Builder: replace the `Media` content FK.
  #[inline]
  pub fn with_media_id(mut self, media_id: Id) -> Self {
    self.media_id = media_id;
    self
  }

  /// Builder: replace the discovering `WatchedLocation` FK.
  #[inline]
  pub fn with_watched_location_id(mut self, watched_location_id: Id) -> Self {
    self.watched_location_id = watched_location_id;
    self
  }

  // --- in-place setters ---------------------------------------------------

  /// In-place mutator for `created_at`, collapsing the 0-ms wire sentinel
  /// to `None`.
  ///
  /// The wire encodes `created_at == 0` (Unix epoch, ms) as absent, and
  /// many filesystems lack a birth time; storing `Some(<0-ms timestamp>)`
  /// would round-trip back to `None` and lose data. A `Some(t)` whose
  /// `t.as_millisecond() == 0` is therefore collapsed to `None`.
  #[inline]
  pub fn set_created_at(&mut self, t: Option<JiffTimestamp>) {
    self.created_at = normalize_created_at(t);
  }

  /// In-place mutator for `location` — also the single atomic rename/move
  /// API; the derived [`name`](MediaFile::name) follows.
  #[inline]
  pub fn set_location(&mut self, location: Location<Id>) {
    self.location = location;
  }

  /// In-place mutator for the `Media` content FK.
  #[inline]
  pub fn set_media_id(&mut self, media_id: Id) {
    self.media_id = media_id;
  }

  /// In-place mutator for the discovering `WatchedLocation` FK.
  #[inline]
  pub fn set_watched_location_id(&mut self, watched_location_id: Id) {
    self.watched_location_id = watched_location_id;
  }
}

/// Collapse the 0-ms `created_at` wire sentinel to `None`.
///
/// The wire codec encodes `0` (Unix epoch, ms) as "absent", and many
/// filesystems have no birth time, so a `Some(<0-ms>)` would be
/// indistinguishable from `None` after a round-trip. Normalising on the
/// way in keeps the domain incapable of holding the sentinel as a genuine
/// creation time. (Mirrors `normalize_capture_date` on `Media`.)
#[inline]
fn normalize_created_at(t: Option<JiffTimestamp>) -> Option<JiffTimestamp> {
  match t {
    Some(ts) if ts.as_millisecond() == 0 => None,
    other => other,
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
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  /// A real (non-epoch) creation timestamp. `JiffTimestamp::default()` is
  /// 0 ms (Unix epoch), which `try_new` collapses to `None`, so
  /// `created_at` tests that expect a value must use a non-zero instant.
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

  #[test]
  fn try_new_happy_path() {
    let id = Uuid7::new();
    let media_id = Uuid7::new();
    let wl_id = Uuid7::new();
    let vol = Uuid7::new();
    let f = MediaFile::try_new(id, media_id, Some(real_ts()), loc(vol), wl_id)
      .expect("valid construction must succeed");
    assert_eq!(f.id(), &id);
    assert_eq!(f.media_id(), &media_id);
    // name is derived from the location's last component.
    assert_eq!(f.name(), "clip.mp4");
    assert_eq!(f.created_at(), Some(&real_ts()));
    assert_eq!(f.watched_location_id(), &wl_id);
    // location() projects &Location; use the IsVariant + Unwrap derives.
    assert!(f.location().is_local());
    let local = f.location().unwrap_local_ref();
    assert_eq!(local.volume(), &vol);
    assert_eq!(local.components(), &["Movies", "clip.mp4"]);
  }

  #[test]
  fn name_is_derived_from_location() {
    let vol = Uuid7::new();
    let f = MediaFile::try_new(
      Uuid7::new(),
      Uuid7::new(),
      None,
      named_loc(vol, "holiday.mkv"),
      Uuid7::new(),
    )
    .unwrap();
    assert_eq!(f.name(), "holiday.mkv");
    // Replacing the location atomically renames/moves the file.
    let f = f.with_location(named_loc(vol, "renamed.mp4"));
    assert_eq!(f.name(), "renamed.mp4");
  }

  #[test]
  fn created_at_collapses_zero_ms_sentinel() {
    let f = MediaFile::try_new(
      Uuid7::new(),
      Uuid7::new(),
      // Epoch (0 ms) is the wire "absent" sentinel — must collapse to None.
      Some(JiffTimestamp::default()),
      loc(Uuid7::new()),
      Uuid7::new(),
    )
    .unwrap();
    assert!(
      f.created_at().is_none(),
      "0-ms created_at must collapse to None"
    );

    // An explicit `None` stays `None`.
    let f = f.with_created_at(None);
    assert!(f.created_at().is_none());

    // A real instant survives.
    let f = f.with_created_at(Some(real_ts()));
    assert_eq!(f.created_at(), Some(&real_ts()));

    // The in-place setter collapses identically.
    let mut f = f;
    f.set_created_at(Some(JiffTimestamp::default()));
    assert!(f.created_at().is_none());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = MediaFile::try_new(
      Uuid7::nil(),
      Uuid7::new(),
      Some(real_ts()),
      loc(Uuid7::new()),
      Uuid7::new(),
    );
    assert_eq!(r.err(), Some(MediaFileError::NilId));
    assert!(MediaFileError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_media_id() {
    let r = MediaFile::try_new(
      Uuid7::new(),
      Uuid7::nil(),
      Some(real_ts()),
      loc(Uuid7::new()),
      Uuid7::new(),
    );
    assert_eq!(r.err(), Some(MediaFileError::NilMediaId));
    assert!(MediaFileError::NilMediaId.is_nil_media_id());
  }

  #[test]
  fn try_new_rejects_nil_watched_location_id() {
    let r = MediaFile::try_new(
      Uuid7::new(),
      Uuid7::new(),
      Some(real_ts()),
      loc(Uuid7::new()),
      Uuid7::nil(),
    );
    assert_eq!(r.err(), Some(MediaFileError::NilWatchedLocationId));
    assert!(MediaFileError::NilWatchedLocationId.is_nil_watched_location_id());
  }

  #[test]
  fn builders_chain() {
    let media_id = Uuid7::new();
    let wl_id = Uuid7::new();
    let vol = Uuid7::new();
    let f = MediaFile::try_new(
      Uuid7::new(),
      Uuid7::new(),
      None,
      named_loc(Uuid7::new(), "old.mp4"),
      Uuid7::new(),
    )
    .unwrap()
    .with_media_id(media_id)
    .with_location(named_loc(vol, "new.mkv"))
    .with_created_at(Some(real_ts()))
    .with_watched_location_id(wl_id);
    assert_eq!(f.name(), "new.mkv");
    assert_eq!(f.media_id(), &media_id);
    assert_eq!(f.created_at(), Some(&real_ts()));
    assert_eq!(f.watched_location_id(), &wl_id);
    assert_eq!(f.location().unwrap_local_ref().volume(), &vol);
  }

  #[test]
  fn setters_mutate_in_place() {
    let mut f = MediaFile::try_new(
      Uuid7::new(),
      Uuid7::new(),
      Some(real_ts()),
      named_loc(Uuid7::new(), "clip.mp4"),
      Uuid7::new(),
    )
    .unwrap();
    let media_id = Uuid7::new();
    let wl_id = Uuid7::new();
    let vol = Uuid7::new();
    f.set_media_id(media_id);
    f.set_watched_location_id(wl_id);
    f.set_location(named_loc(vol, "renamed.mp4"));
    f.set_created_at(None);
    assert_eq!(f.name(), "renamed.mp4");
    assert_eq!(f.media_id(), &media_id);
    assert!(f.created_at().is_none());
    assert_eq!(f.watched_location_id(), &wl_id);
    assert_eq!(f.location().unwrap_local_ref().volume(), &vol);
  }
}
