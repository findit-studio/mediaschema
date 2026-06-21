//! `Thumbnail` — a first-class, storage-tagged thumbnail image referenced
//! by [`Keyframe`](super::keyframe::Keyframe) via FK.
//!
//! The video pipeline needs configurable thumbnail storage (filesystem /
//! database / remote) recorded **per thumbnail** so retrieval is
//! data-driven and backends can be mixed or migrated. The pure domain
//! aggregate stays storage-agnostic — it just carries the
//! [`ThumbnailKind`] discriminator plus the two mutually-exclusive
//! payload slots (`data` for `Database`, `location` for
//! `FileSystem`/`Remote`); the SQLite persistence layer maps the `kind`
//! to a real column.
//!
//! A `Keyframe` no longer inlines its image bytes — it holds a
//! `thumbnail_id` FK into this aggregate.

use bytes::Bytes;
use derive_more::IsVariant;
use smol_str::SmolStr;

use crate::domain::{ThumbnailKind, Uuid7};

/// A thumbnail image plus its storage descriptor.
///
/// Generic over `Id` (default [`Uuid7`]). Fields are private per the
/// encapsulation rule; access via the getter / `with_*` / `set_*`
/// accessors.
///
/// The `kind`/`data`/`location` trio carries a cross-field invariant
/// (see [`Thumbnail::try_new`]): a [`ThumbnailKind::Database`] thumbnail
/// inlines its bytes in `data` (and leaves `location` empty), while a
/// [`ThumbnailKind::FileSystem`] / [`ThumbnailKind::Remote`] thumbnail
/// records a path/URL in `location` (and leaves `data` empty). The trio
/// is therefore only mutated together through the fallible
/// [`Thumbnail::try_with_storage`] / [`Thumbnail::try_set_storage`]
/// re-validating mutators; the invariant-free `mime` / `width` /
/// `height` fields keep plain `with_*` / `set_*` accessors.
///
/// **No `Default`** — defaulting to a nil `id` would mint an orphan
/// thumbnail with no stable key. Construct via [`Thumbnail::try_new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbnail<Id = Uuid7> {
  id: Id,
  kind: ThumbnailKind,
  /// Inline image bytes (`bytes::Bytes`) — populated for
  /// [`ThumbnailKind::Database`], empty otherwise.
  data: Bytes,
  /// Path or URL — populated for [`ThumbnailKind::FileSystem`] /
  /// [`ThumbnailKind::Remote`], empty for [`ThumbnailKind::Database`].
  location: SmolStr,
  mime: SmolStr,
  width: u32,
  height: u32,
}

impl Thumbnail<Uuid7> {
  /// Validating constructor.
  ///
  /// Rejects:
  /// - nil `id` (orphan thumbnail with no stable key / FK target),
  /// - a [`ThumbnailKind::Database`] thumbnail with empty `data`
  ///   (nothing to serve),
  /// - a [`ThumbnailKind::FileSystem`] / [`ThumbnailKind::Remote`]
  ///   thumbnail with empty `location` (no path/URL to resolve).
  ///
  /// `width` / `height` are accepted as-is (a `0` extent is not rejected
  /// here — the keyframe owns the locked non-zero-dimensions invariant;
  /// a thumbnail may legitimately record an as-yet-unknown extent).
  pub fn try_new(
    id: Uuid7,
    kind: ThumbnailKind,
    data: impl Into<Bytes>,
    location: impl Into<SmolStr>,
    mime: impl Into<SmolStr>,
    width: u32,
    height: u32,
  ) -> Result<Self, ThumbnailError> {
    if id.is_nil() {
      return Err(ThumbnailError::NilId);
    }
    let data = data.into();
    let location = location.into();
    Self::check_storage(kind, &data, &location)?;
    Ok(Self {
      id,
      kind,
      data,
      location,
      mime: mime.into(),
      width,
      height,
    })
  }

  /// Re-validate the `kind`/`data`/`location` cross-field invariant.
  fn check_storage(
    kind: ThumbnailKind,
    data: &Bytes,
    location: &SmolStr,
  ) -> Result<(), ThumbnailError> {
    match kind {
      ThumbnailKind::Database if data.is_empty() => Err(ThumbnailError::EmptyData),
      ThumbnailKind::FileSystem | ThumbnailKind::Remote if location.is_empty() => {
        Err(ThumbnailError::EmptyLocation)
      }
      _ => Ok(()),
    }
  }

  /// Fallible builder: replace the `kind` + payload trio together,
  /// re-validating the per-kind invariant (see [`Thumbnail::try_new`]).
  /// On rejection returns the error and leaves `self` untouched.
  #[inline]
  pub fn try_with_storage(
    mut self,
    kind: ThumbnailKind,
    data: impl Into<Bytes>,
    location: impl Into<SmolStr>,
  ) -> Result<Self, ThumbnailError> {
    let data = data.into();
    let location = location.into();
    Self::check_storage(kind, &data, &location)?;
    self.kind = kind;
    self.data = data;
    self.location = location;
    Ok(self)
  }

  /// Fallible in-place mutator for the `kind` + payload trio — see
  /// [`Thumbnail::try_with_storage`]. On success returns `&mut Self` so
  /// it chains; on rejection returns the error and leaves `self`
  /// unchanged.
  #[inline]
  pub fn try_set_storage(
    &mut self,
    kind: ThumbnailKind,
    data: impl Into<Bytes>,
    location: impl Into<SmolStr>,
  ) -> Result<&mut Self, ThumbnailError> {
    let data = data.into();
    let location = location.into();
    Self::check_storage(kind, &data, &location)?;
    self.kind = kind;
    self.data = data;
    self.location = location;
    Ok(self)
  }
}

impl<Id> Thumbnail<Id> {
  // --- identity / storage ---
  /// Canonical identity — the FK target of `Keyframe.thumbnail_id`.
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }
  /// Which storage backend holds the image bytes.
  #[inline(always)]
  pub const fn kind(&self) -> ThumbnailKind {
    self.kind
  }
  /// Inline image bytes (`""`/empty = stored out-of-band; see
  /// [`Thumbnail::location`]).
  #[inline(always)]
  pub fn data(&self) -> &[u8] {
    &self.data
  }
  /// Owned handle to the image bytes — O(1) refcount clone, no copy.
  #[inline(always)]
  pub fn data_bytes(&self) -> Bytes {
    self.data.clone()
  }
  /// Path or URL of the image (`""` = inlined in `data`).
  #[inline(always)]
  pub fn location(&self) -> &str {
    self.location.as_str()
  }
  /// MIME type (`""` = absent).
  #[inline(always)]
  pub fn mime(&self) -> &str {
    self.mime.as_str()
  }
  /// Pixel width of the stored image.
  #[inline(always)]
  pub const fn width(&self) -> u32 {
    self.width
  }
  /// Pixel height of the stored image.
  #[inline(always)]
  pub const fn height(&self) -> u32 {
    self.height
  }
}

// Builders + setters for the invariant-free fields. The invariant-bearing
// `kind`/`data`/`location` trio is mutated only through the fallible
// `try_with_storage` / `try_set_storage` above.
impl<Id> Thumbnail<Id> {
  #[must_use]
  #[inline(always)]
  pub fn with_mime(mut self, v: impl Into<SmolStr>) -> Self {
    self.mime = v.into();
    self
  }
  #[inline(always)]
  pub fn set_mime(&mut self, v: impl Into<SmolStr>) -> &mut Self {
    self.mime = v.into();
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_width(mut self, v: u32) -> Self {
    self.width = v;
    self
  }
  #[inline(always)]
  pub const fn set_width(&mut self, v: u32) -> &mut Self {
    self.width = v;
    self
  }
  #[must_use]
  #[inline(always)]
  pub const fn with_height(mut self, v: u32) -> Self {
    self.height = v;
    self
  }
  #[inline(always)]
  pub const fn set_height(&mut self, v: u32) -> &mut Self {
    self.height = v;
    self
  }
}

/// Error returned when a [`Thumbnail`] constructor or fallible mutator
/// cannot uphold an invariant. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum ThumbnailError {
  /// Supplied `id` was the nil sentinel.
  #[error("Thumbnail id must not be the nil UUID")]
  NilId,
  /// A [`ThumbnailKind::Database`] thumbnail was given empty `data` —
  /// there are no inlined bytes to serve.
  #[error("Thumbnail with `kind = database` must have non-empty `data`")]
  EmptyData,
  /// A [`ThumbnailKind::FileSystem`] / [`ThumbnailKind::Remote`]
  /// thumbnail was given an empty `location` — there is no path/URL to
  /// resolve.
  #[error("Thumbnail with `kind = filesystem`/`remote` must have non-empty `location`")]
  EmptyLocation,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  #[test]
  fn try_new_database_happy_path() {
    let id = Uuid7::new();
    let t = Thumbnail::try_new(
      id,
      ThumbnailKind::Database,
      std::vec![0xff_u8, 0xd8, 0xff],
      "",
      "image/jpeg",
      320,
      180,
    )
    .unwrap();
    assert_eq!(t.id_ref(), &id);
    assert!(t.kind().is_database());
    assert_eq!(t.data(), &[0xff, 0xd8, 0xff]);
    assert!(t.location().is_empty());
    assert_eq!(t.mime(), "image/jpeg");
    assert_eq!(t.width(), 320);
    assert_eq!(t.height(), 180);
  }

  #[test]
  fn try_new_filesystem_and_remote_happy_path() {
    let fs = Thumbnail::try_new(
      Uuid7::new(),
      ThumbnailKind::FileSystem,
      Bytes::new(),
      "/var/thumbs/a.jpg",
      "image/jpeg",
      64,
      64,
    )
    .unwrap();
    assert!(fs.kind().is_file_system());
    assert!(fs.data().is_empty());
    assert_eq!(fs.location(), "/var/thumbs/a.jpg");

    let remote = Thumbnail::try_new(
      Uuid7::new(),
      ThumbnailKind::Remote,
      Bytes::new(),
      "https://cdn.example/a.webp",
      "image/webp",
      128,
      72,
    )
    .unwrap();
    assert!(remote.kind().is_remote());
    assert_eq!(remote.location(), "https://cdn.example/a.webp");
  }

  #[test]
  fn try_new_rejects_nil_id() {
    assert_eq!(
      Thumbnail::try_new(
        Uuid7::nil(),
        ThumbnailKind::Database,
        std::vec![1u8],
        "",
        "image/png",
        1,
        1
      )
      .err(),
      Some(ThumbnailError::NilId)
    );
    assert!(ThumbnailError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_enforces_per_kind_payload_invariants() {
    // Database without data → EmptyData.
    assert_eq!(
      Thumbnail::try_new(
        Uuid7::new(),
        ThumbnailKind::Database,
        Bytes::new(),
        "",
        "image/png",
        1,
        1
      )
      .err(),
      Some(ThumbnailError::EmptyData)
    );
    // FileSystem / Remote without location → EmptyLocation.
    for kind in [ThumbnailKind::FileSystem, ThumbnailKind::Remote] {
      assert_eq!(
        Thumbnail::try_new(Uuid7::new(), kind, Bytes::new(), "", "image/png", 1, 1).err(),
        Some(ThumbnailError::EmptyLocation),
        "{kind:?} with empty location should be rejected"
      );
    }
    assert!(ThumbnailError::EmptyData.is_empty_data());
    assert!(ThumbnailError::EmptyLocation.is_empty_location());
  }

  #[test]
  fn try_with_storage_revalidates_and_switches_backend() {
    let t = Thumbnail::try_new(
      Uuid7::new(),
      ThumbnailKind::Database,
      std::vec![1u8, 2, 3],
      "",
      "image/png",
      8,
      8,
    )
    .unwrap();

    // Switch DB → FileSystem (clears data, sets location).
    let t = t
      .try_with_storage(ThumbnailKind::FileSystem, Bytes::new(), "/x.png")
      .unwrap();
    assert!(t.kind().is_file_system());
    assert!(t.data().is_empty());
    assert_eq!(t.location(), "/x.png");

    // A switch that violates the invariant is rejected and leaves self
    // unchanged.
    let mut t = t;
    assert_eq!(
      t.try_set_storage(ThumbnailKind::Database, Bytes::new(), "")
        .err(),
      Some(ThumbnailError::EmptyData)
    );
    assert!(t.kind().is_file_system());
    assert_eq!(t.location(), "/x.png");

    // A valid in-place switch back to Database is accepted.
    t.try_set_storage(ThumbnailKind::Database, std::vec![9u8], "")
      .unwrap();
    assert!(t.kind().is_database());
    assert_eq!(t.data(), &[9]);
    assert!(t.location().is_empty());
  }

  #[test]
  fn scalar_builders_and_setters_chain() {
    let mut t = Thumbnail::try_new(
      Uuid7::new(),
      ThumbnailKind::Database,
      std::vec![1u8],
      "",
      "image/png",
      1,
      1,
    )
    .unwrap()
    .with_mime("image/webp")
    .with_width(640)
    .with_height(360);
    assert_eq!(t.mime(), "image/webp");
    assert_eq!(t.width(), 640);
    assert_eq!(t.height(), 360);

    t.set_mime("").set_width(0).set_height(0);
    assert!(t.mime().is_empty());
    assert_eq!(t.width(), 0);
    assert_eq!(t.height(), 0);
  }
}
