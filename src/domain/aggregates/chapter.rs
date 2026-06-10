//! `Chapter<Id>` — container-level chapter / cue point
//! (locked in `schema/chapter.md` rev 1).
//!
//! One row per container chapter (`AVFormatContext.chapters[i]`); the parent
//! [`Media`](crate::domain::Media) carries the verbatim probe count
//! `nb_chapters` and the reverse-lookup `chapters: Vec<Id>` materialised by
//! joining on `Chapter.media_id`.
//!
//! ## Cross-cutting (locked conventions)
//!
//! - Generic over `Id` (default [`Uuid7`]); `media_id` FK is the same UUIDv7.
//! - Media-time interval = `mediatime::TimeRange` (carries its own
//!   `Timebase`). The chapter's `time_range` is *the* source of truth for
//!   start/end; no separate `start_pts`/`end_pts`/`timebase` columns.
//! - `title` is hoisted from `metadata["title" | "TITLE" | "Title" | ...]`
//!   (first case-insensitive match wins; value stored verbatim) for indexed
//!   SQL lookup via `LOWER(title)`. The matching entry is *removed* from
//!   `metadata` so persistence is lossless against semantic metadata —
//!   round-tripping to AVChapter writes `{"title": title, ...metadata}`.
//! - `metadata` is an [`indexmap::IndexMap<SmolStr, SmolStr>`] — preserves
//!   AVDictionary insertion order and supports `O(1)` keyed lookup; cheap
//!   to clone via `SmolStr`'s `Arc<str>` for long values.
//!
//! ## Intrinsic invariants
//!
//! `try_new` rejects nil `id` and nil `media_id`. Title is bounded by
//! [`MAX_CHAPTER_TITLE_BYTES`] (defensive cap mirroring mediaschema's
//! other string-field guards). The `chapters[i].index == i` collection-
//! composition invariant is application-layer, not enforced here.
//!
//! ## Encapsulation
//!
//! No public fields. Access via getters (`const fn` where possible);
//! mutation via `with_*` builders and `set_*` setters.

use indexmap::IndexMap;
use smol_str::SmolStr;
use thiserror::Error;

use crate::domain::Uuid7;
use mediatime::TimeRange;

/// Defensive cap on `Chapter.title` byte length. Mirrors mediaschema's
/// other string-field guards (`Provenance.model_name`, `ErrorInfo.message`,
/// `Tag.name`, …): no real container chapter title approaches this size, so
/// the cap exists to bound worst-case allocation on a hostile probe.
pub const MAX_CHAPTER_TITLE_BYTES: usize = 4 * 1024;

/// Defensive cap on `Chapter.metadata` entry count. AVDictionary itself is
/// unbounded; this is the worst-case backstop, well above real-world usage.
pub const MAX_CHAPTER_METADATA_ENTRIES: usize = 4096;

/// One container-level chapter / cue point.
///
/// **No `Default` impl** — defaulting to `{ id: nil, media_id: nil,
/// time_range: 0..0, … }` would represent an orphan chapter with no real
/// identity and no media to attach to. Construct via [`Chapter::try_new`]
/// (validating: rejects nil `id` and nil `media_id`).
///
/// **Fields are private**; access via getters and `with_*` / `set_*`
/// mutators per the encapsulation rule.
#[derive(Debug, Clone, PartialEq)]
pub struct Chapter<Id = Uuid7> {
  id: Id,
  /// FK → parent [`Media`](crate::domain::Media). Cascades on delete in
  /// every sqlx dialect; mongodb keeps the document as the truth.
  media_id: Id,
  /// Ordinal within the parent media, `0..media.nb_chapters`. Not
  /// validated here (cross-aggregate concern); kept verbatim from probe.
  index: u32,
  /// `AVChapter.id` verbatim. Container's own id; mediaschema does not
  /// rely on it for identity (UUIDv7 `id` is the only key).
  source_id: i64,
  /// Start..end on the chapter's own timebase (mediatime carries it).
  time_range: TimeRange,
  /// Conventional `metadata["title"]` value, hoisted for SQL lookup.
  /// **Empty = absent** (never `Option<SmolStr>`, per the codebase
  /// convention).
  title: SmolStr,
  /// AVDictionary entries with the title key (any case) removed.
  /// Insertion-ordered.
  metadata: IndexMap<SmolStr, SmolStr>,
}

/// Construction / mutation errors for [`Chapter`].
#[derive(Debug, Error, derive_more::IsVariant, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChapterError {
  /// `id` was nil — every chapter row must have a real identity.
  #[error("Chapter.id must be non-nil")]
  NilId,
  /// `media_id` was nil — every chapter must reference a real `Media`.
  #[error("Chapter.media_id must be non-nil")]
  NilMediaId,
  /// `title.len() > MAX_CHAPTER_TITLE_BYTES`.
  #[error("Chapter.title exceeds {MAX_CHAPTER_TITLE_BYTES} bytes: got {got}")]
  TitleTooLong {
    /// The offending byte count.
    got: usize,
  },
  /// `metadata.len() > MAX_CHAPTER_METADATA_ENTRIES`.
  #[error("Chapter.metadata exceeds {MAX_CHAPTER_METADATA_ENTRIES} entries: got {got}")]
  MetadataTooLarge {
    /// The offending entry count.
    got: usize,
  },
}

impl Chapter<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` / `media_id`. `title` and `metadata` default to
  /// empty / empty-map; populate via builders.
  pub fn try_new(
    id: Uuid7,
    media_id: Uuid7,
    index: u32,
    source_id: i64,
    time_range: TimeRange,
  ) -> Result<Self, ChapterError> {
    if id.is_nil() {
      return Err(ChapterError::NilId);
    }
    if media_id.is_nil() {
      return Err(ChapterError::NilMediaId);
    }
    Ok(Self {
      id,
      media_id,
      index,
      source_id,
      time_range,
      title: SmolStr::default(),
      metadata: IndexMap::new(),
    })
  }
}

impl<Id> Chapter<Id> {
  /// Canonical identity.
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// FK → parent `Media`.
  #[inline(always)]
  pub const fn media_id_ref(&self) -> &Id {
    &self.media_id
  }

  /// Ordinal within the parent media.
  #[inline(always)]
  pub const fn index(&self) -> u32 {
    self.index
  }

  /// `AVChapter.id` verbatim.
  #[inline(always)]
  pub const fn source_id(&self) -> i64 {
    self.source_id
  }

  /// Start..end on the chapter's own timebase.
  #[inline(always)]
  pub const fn time_range_ref(&self) -> &TimeRange {
    &self.time_range
  }

  /// Hoisted title value (empty = absent).
  #[inline(always)]
  pub fn title(&self) -> &str {
    self.title.as_str()
  }

  /// Borrow the title `SmolStr` directly (cheap `Arc<str>` clone-source
  /// for long values).
  #[inline(always)]
  pub const fn title_ref(&self) -> &SmolStr {
    &self.title
  }

  /// AVDictionary entries (title-key removed), insertion-ordered.
  #[inline(always)]
  pub const fn metadata_ref(&self) -> &IndexMap<SmolStr, SmolStr> {
    &self.metadata
  }

  // --- builders -----------------------------------------------------------

  /// Builder: replace `index`.
  #[inline(always)]
  #[must_use]
  pub const fn with_index(mut self, index: u32) -> Self {
    self.index = index;
    self
  }

  /// Builder: replace `source_id`.
  #[inline(always)]
  #[must_use]
  pub const fn with_source_id(mut self, source_id: i64) -> Self {
    self.source_id = source_id;
    self
  }

  /// Builder: replace `time_range`.
  #[inline(always)]
  #[must_use]
  pub const fn with_time_range(mut self, time_range: TimeRange) -> Self {
    self.time_range = time_range;
    self
  }

  /// Validating builder: replace `title`.
  ///
  /// Rejects titles longer than [`MAX_CHAPTER_TITLE_BYTES`] with
  /// [`ChapterError::TitleTooLong`]; on rejection `self` is returned
  /// unchanged inside the `Err`.
  #[inline]
  pub fn try_with_title(mut self, title: impl Into<SmolStr>) -> Result<Self, ChapterError> {
    let title = title.into();
    if title.len() > MAX_CHAPTER_TITLE_BYTES {
      return Err(ChapterError::TitleTooLong { got: title.len() });
    }
    self.title = title;
    Ok(self)
  }

  /// Validating builder: replace `metadata`.
  ///
  /// Rejects bags larger than [`MAX_CHAPTER_METADATA_ENTRIES`] with
  /// [`ChapterError::MetadataTooLarge`].
  #[inline]
  pub fn try_with_metadata(
    mut self,
    metadata: IndexMap<SmolStr, SmolStr>,
  ) -> Result<Self, ChapterError> {
    if metadata.len() > MAX_CHAPTER_METADATA_ENTRIES {
      return Err(ChapterError::MetadataTooLarge {
        got: metadata.len(),
      });
    }
    self.metadata = metadata;
    Ok(self)
  }

  // --- in-place setters ---------------------------------------------------

  /// In-place mutator: replace `index`.
  #[inline(always)]
  pub const fn set_index(&mut self, index: u32) -> &mut Self {
    self.index = index;
    self
  }

  /// In-place mutator: replace `source_id`.
  #[inline(always)]
  pub const fn set_source_id(&mut self, source_id: i64) -> &mut Self {
    self.source_id = source_id;
    self
  }

  /// In-place mutator: replace `time_range`.
  #[inline(always)]
  pub const fn set_time_range(&mut self, time_range: TimeRange) -> &mut Self {
    self.time_range = time_range;
    self
  }

  /// Validating in-place mutator: replace `title`.
  ///
  /// Same cap as [`Self::try_with_title`]; on rejection the prior value
  /// is left unchanged.
  #[inline]
  pub fn try_set_title(&mut self, title: impl Into<SmolStr>) -> Result<&mut Self, ChapterError> {
    let title = title.into();
    if title.len() > MAX_CHAPTER_TITLE_BYTES {
      return Err(ChapterError::TitleTooLong { got: title.len() });
    }
    self.title = title;
    Ok(self)
  }

  /// Validating in-place mutator: replace `metadata`.
  #[inline]
  pub fn try_set_metadata(
    &mut self,
    metadata: IndexMap<SmolStr, SmolStr>,
  ) -> Result<&mut Self, ChapterError> {
    if metadata.len() > MAX_CHAPTER_METADATA_ENTRIES {
      return Err(ChapterError::MetadataTooLarge {
        got: metadata.len(),
      });
    }
    self.metadata = metadata;
    Ok(self)
  }
}

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use core::num::NonZeroU32;
  use mediatime::Timebase;

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).unwrap())
  }

  #[test]
  fn try_new_accepts_well_formed() {
    let c = Chapter::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      0,
      TimeRange::new(0, 1000, tb()),
    )
    .expect("well-formed");
    assert_eq!(c.index(), 0);
    assert_eq!(c.source_id(), 0);
    assert!(c.title().is_empty());
    assert!(c.metadata_ref().is_empty());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let err = Chapter::try_new(
      Uuid7::nil(),
      Uuid7::new(),
      0,
      0,
      TimeRange::new(0, 1000, tb()),
    )
    .unwrap_err();
    assert!(err.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_media_id() {
    let err = Chapter::try_new(
      Uuid7::new(),
      Uuid7::nil(),
      0,
      0,
      TimeRange::new(0, 1000, tb()),
    )
    .unwrap_err();
    assert!(err.is_nil_media_id());
  }

  #[test]
  fn try_with_title_rejects_oversize() {
    let c = Chapter::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      0,
      TimeRange::new(0, 1000, tb()),
    )
    .expect("well-formed");
    let huge =
      std::string::String::from_utf8(std::vec![b'a'; MAX_CHAPTER_TITLE_BYTES + 1]).unwrap();
    let err = c.try_with_title(huge).unwrap_err();
    assert!(
      matches!(err, ChapterError::TitleTooLong { got } if got == MAX_CHAPTER_TITLE_BYTES + 1)
    );
  }

  #[test]
  fn try_with_metadata_rejects_oversize() {
    let c = Chapter::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      0,
      TimeRange::new(0, 1000, tb()),
    )
    .expect("well-formed");
    let mut huge = IndexMap::new();
    for i in 0..(MAX_CHAPTER_METADATA_ENTRIES + 1) {
      huge.insert(SmolStr::from(std::format!("k{i}")), SmolStr::from("v"));
    }
    let err = c.try_with_metadata(huge).unwrap_err();
    assert!(
      matches!(err, ChapterError::MetadataTooLarge { got } if got == MAX_CHAPTER_METADATA_ENTRIES + 1)
    );
  }

  #[test]
  fn metadata_preserves_insertion_order() {
    let c = Chapter::try_new(
      Uuid7::new(),
      Uuid7::new(),
      0,
      0,
      TimeRange::new(0, 1000, tb()),
    )
    .expect("well-formed");
    let mut bag = IndexMap::new();
    bag.insert(SmolStr::from("artist"), SmolStr::from("Beethoven"));
    bag.insert(SmolStr::from("genre"), SmolStr::from("classical"));
    bag.insert(SmolStr::from("year"), SmolStr::from("1808"));
    let c = c.try_with_metadata(bag).expect("ok");
    let keys: std::vec::Vec<&str> = c.metadata_ref().keys().map(|k| k.as_str()).collect();
    assert_eq!(keys, std::vec!["artist", "genre", "year"]);
  }
}

/// Exhaustive by-value decomposition of [`Chapter`] — every stored
/// field.
///
/// Public-field data-transfer struct (the conversion-boundary exception
/// to the encapsulation rule): cross-suite conversions (`crate::graph`)
/// destructure it exhaustively, so adding a field breaks them at compile
/// time instead of silently dropping data.
#[derive(Debug, Clone, PartialEq)]
pub struct ChapterParts<Id = Uuid7> {
  pub id: Id,
  pub media_id: Id,
  pub index: u32,
  pub source_id: i64,
  pub time_range: TimeRange,
  pub title: SmolStr,
  pub metadata: IndexMap<SmolStr, SmolStr>,
}

impl<Id> Chapter<Id> {
  /// Decompose into [`ChapterParts`] — exhaustive, by value.
  #[inline(always)]
  pub fn into_parts(self) -> ChapterParts<Id> {
    let Self {
      id,
      media_id,
      index,
      source_id,
      time_range,
      title,
      metadata,
    } = self;
    ChapterParts {
      id,
      media_id,
      index,
      source_id,
      time_range,
      title,
      metadata,
    }
  }
}
