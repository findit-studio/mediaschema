//! `BlobRef` — a reserved externalization handle for out-of-band bytes.
//!
//! v1 stores **no attachment bytes**: the [`AttachmentTrack`]'s `blob` slot
//! is always `None`, and this value object exists only so the reserved
//! column / proto field / graph field have a concrete type to point at.
//! When attachment-blob (and the keyframe `thumbnail_id`/`BlobRef`)
//! externalization lands (Phase B), a `BlobRef` will name where the bytes
//! live (`uri`) plus their declared `byte_size` and `content_type`.
//!
//! [`AttachmentTrack`]: super::track::AttachmentTrack

use derive_more::IsVariant;
use smol_str::SmolStr;

/// An opaque handle to externally-stored bytes (filesystem path / URL /
/// object-store key in `uri`), plus the declared size and MIME type.
///
/// Generic over nothing — `BlobRef` is identity-free; it is a pointer, not
/// an aggregate. Fields are private per the encapsulation rule.
///
/// **No `Default`** — a `BlobRef` with an empty `uri` points nowhere.
/// Construct via [`BlobRef::try_new`] (rejects an empty `uri`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlobRef {
  uri: SmolStr,
  byte_size: u64,
  content_type: SmolStr,
}

impl BlobRef {
  /// Validating constructor. Rejects an empty `uri` (a handle that points
  /// nowhere). `byte_size` (`0` = unknown) and `content_type` (`""` =
  /// absent) are accepted as-is.
  pub fn try_new(
    uri: impl Into<SmolStr>,
    byte_size: u64,
    content_type: impl Into<SmolStr>,
  ) -> Result<Self, BlobRefError> {
    let uri = uri.into();
    if uri.is_empty() {
      return Err(BlobRefError::EmptyUri);
    }
    Ok(Self {
      uri,
      byte_size,
      content_type: content_type.into(),
    })
  }

  /// Where the bytes live (filesystem path / URL / object-store key).
  #[inline(always)]
  pub fn uri(&self) -> &str {
    self.uri.as_str()
  }

  /// Declared size in bytes (`0` = unknown).
  #[inline(always)]
  pub const fn byte_size(&self) -> u64 {
    self.byte_size
  }

  /// Declared MIME type (`""` = absent).
  #[inline(always)]
  pub fn content_type(&self) -> &str {
    self.content_type.as_str()
  }
}

/// Error returned by [`BlobRef::try_new`]. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum BlobRefError {
  /// Supplied `uri` was empty — a handle must point somewhere.
  #[error("BlobRef `uri` must not be empty")]
  EmptyUri,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  #[test]
  fn try_new_happy_path() {
    let b = BlobRef::try_new("file:///fonts/a.ttf", 4_096, "font/ttf").expect("valid");
    assert_eq!(b.uri(), "file:///fonts/a.ttf");
    assert_eq!(b.byte_size(), 4_096);
    assert_eq!(b.content_type(), "font/ttf");
  }

  #[test]
  fn try_new_rejects_empty_uri() {
    let r = BlobRef::try_new("", 0, "");
    assert_eq!(r.err(), Some(BlobRefError::EmptyUri));
    assert!(BlobRefError::EmptyUri.is_empty_uri());
  }
}
