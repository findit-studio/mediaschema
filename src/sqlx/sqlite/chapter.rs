//! SQLite row shapes for `Chapter` + its `chapter_metadata` child table.
//!
//! Mirrors the postgres / mysql shapes; SQLite uses `BLOB`-as-`Vec<u8>`
//! for identity / FK columns and only has `INTEGER` for numeric types,
//! so timebase + ordinal ride as `i64`.

use core::num::NonZeroU32;

use indexmap::IndexMap;
use mediatime::{TimeRange, Timebase};
use smol_str::SmolStr;

use crate::{
  domain::{
    aggregates::chapter::{Chapter, ChapterError},
    Uuid7,
  },
  sqlx::{dto::bytes_to_uuid7, SqlxError},
};

/// SQLite row for the `chapter` table.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteChapterRow {
  pub id: std::vec::Vec<u8>,
  pub media_id: std::vec::Vec<u8>,
  pub chapter_index: i64,
  pub source_id: i64,
  pub start_pts: i64,
  pub end_pts: i64,
  pub timebase_num: i64,
  pub timebase_den: i64,
  pub title: String,
}

/// SQLite row for `chapter_metadata`. Position in the per-`chapter_id`
/// `ordinal` sequence IS the [`IndexMap`] insertion order.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteChapterMetadataRow {
  pub chapter_id: std::vec::Vec<u8>,
  pub ordinal: i64,
  pub key: String,
  pub value: String,
}

impl From<&Chapter<Uuid7>> for (SqliteChapterRow, std::vec::Vec<SqliteChapterMetadataRow>) {
  fn from(c: &Chapter<Uuid7>) -> Self {
    let id_bytes = c.id_ref().as_bytes().to_vec();
    let row = SqliteChapterRow {
      id: id_bytes.clone(),
      media_id: c.media_id_ref().as_bytes().to_vec(),
      chapter_index: i64::from(c.index()),
      source_id: c.source_id(),
      start_pts: c.time_range_ref().start_pts(),
      end_pts: c.time_range_ref().end_pts(),
      timebase_num: i64::from(c.time_range_ref().timebase().num()),
      timebase_den: i64::from(c.time_range_ref().timebase().den().get()),
      title: c.title().to_owned(),
    };
    let metadata = c
      .metadata_ref()
      .iter()
      .enumerate()
      .map(|(i, (k, v))| SqliteChapterMetadataRow {
        chapter_id: id_bytes.clone(),
        ordinal: i64::try_from(i).unwrap_or(i64::MAX),
        key: k.as_str().to_owned(),
        value: v.as_str().to_owned(),
      })
      .collect();
    (row, metadata)
  }
}

/// Reconstruct a domain [`Chapter`] from the row + its metadata side-table
/// rows. The supplied `metadata` may be in any order — sorted by
/// `ordinal` before insertion so the original [`IndexMap`] order is
/// recovered.
pub fn chapter_from_rows(
  row: SqliteChapterRow,
  mut metadata: std::vec::Vec<SqliteChapterMetadataRow>,
) -> Result<Chapter<Uuid7>, SqlxError> {
  let id = bytes_to_uuid7(&row.id)?;
  let media_id = bytes_to_uuid7(&row.media_id)?;
  let index = u32::try_from(row.chapter_index)
    .map_err(|e| SqlxError::UnknownDiscriminant(format!("Chapter.index: {e}")))?;
  let num = u32::try_from(row.timebase_num)
    .map_err(|e| SqlxError::UnknownDiscriminant(format!("Chapter.timebase_num: {e}")))?;
  let den_u32 = u32::try_from(row.timebase_den)
    .map_err(|e| SqlxError::UnknownDiscriminant(format!("Chapter.timebase_den: {e}")))?;
  let den = NonZeroU32::new(den_u32).ok_or_else(|| {
    SqlxError::DomainConstructorRejected("Chapter.timebase_den must be non-zero".to_owned())
  })?;
  let timebase = Timebase::new(num, den);
  let time_range = TimeRange::new(row.start_pts, row.end_pts, timebase);

  let chapter = Chapter::try_new(id, media_id, index, row.source_id, time_range)
    .map_err(chapter_err_to_sqlx)?;
  let chapter = chapter
    .try_with_title(row.title)
    .map_err(chapter_err_to_sqlx)?;

  metadata.sort_by_key(|m| m.ordinal);
  let mut bag = IndexMap::with_capacity(metadata.len());
  for entry in metadata {
    if entry.chapter_id != row.id {
      return Err(SqlxError::DomainConstructorRejected(
        "chapter_metadata.chapter_id does not match parent chapter.id".to_owned(),
      ));
    }
    bag.insert(SmolStr::from(entry.key), SmolStr::from(entry.value));
  }
  chapter.try_with_metadata(bag).map_err(chapter_err_to_sqlx)
}

fn chapter_err_to_sqlx(e: ChapterError) -> SqlxError {
  SqlxError::DomainConstructorRejected(e.to_string())
}
