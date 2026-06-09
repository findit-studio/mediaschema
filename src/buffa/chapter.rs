//! Wire ⇄ domain conversions for `Chapter` (`media.v1::Chapter`) and the
//! ordered key/value entries in `metadata` (`media.v1::KeyValue`).
//!
//! ## Field correspondence
//!
//! | wire field                  | domain field        | notes                                |
//! | --------------------------- | ------------------- | ------------------------------------ |
//! | `id` (Bytes, 16)            | `id` (Uuid7)        | validating                           |
//! | `media_id` (Bytes, 16)      | `media_id` (Uuid7)  | validating                           |
//! | `index: u32`                | `index: u32`        |                                      |
//! | `source_id: i64`            | `source_id: i64`    |                                      |
//! | `time_range: TimeRange`     | `time_range`        | mediatime extern; required           |
//! | `title: SmolStr`            | `title: SmolStr`    | empty = absent                       |
//! | `metadata: Vec<KeyValue>`   | `metadata: IndexMap` | repeated KV preserves insertion order |

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::{
  buffa::error::BuffaError,
  domain::{
    aggregates::chapter::{Chapter, ChapterError},
    Uuid7,
  },
  generated::media::v1 as wire,
};

fn wire_bytes_to_uuid7(
  b: &::buffa::bytes::Bytes,
  field: &'static str,
) -> Result<Uuid7, BuffaError> {
  let arr: [u8; 16] = b
    .as_ref()
    .try_into()
    .map_err(|_| BuffaError::IdWrongLength(b.len()))?;
  Uuid7::try_from_bytes(arr).map_err(|e| {
    // `field` is the source-side label; the typed error already carries
    // the specifics of *why* the layout failed.
    let _ = field;
    BuffaError::from(e)
  })
}

impl TryFrom<&wire::Chapter> for Chapter<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::Chapter) -> Result<Self, Self::Error> {
    let id = wire_bytes_to_uuid7(&w.id, "Chapter.id")?;
    let media_id = wire_bytes_to_uuid7(&w.media_id, "Chapter.media_id")?;
    let time_range = *w
      .time_range
      .as_option()
      .ok_or(BuffaError::MissingRequiredField("Chapter.time_range"))?;

    let chapter = Chapter::try_new(id, media_id, w.index, w.source_id, time_range)
      .map_err(chapter_err_to_buffa)?;
    let chapter = chapter
      .try_with_title(w.title.as_str())
      .map_err(chapter_err_to_buffa)?;

    let mut bag = IndexMap::with_capacity(w.metadata.len());
    for kv in &w.metadata {
      bag.insert(
        SmolStr::from(kv.key.as_str()),
        SmolStr::from(kv.value.as_str()),
      );
    }
    chapter.try_with_metadata(bag).map_err(chapter_err_to_buffa)
  }
}

impl From<&Chapter<Uuid7>> for wire::Chapter {
  fn from(c: &Chapter<Uuid7>) -> Self {
    let metadata = c
      .metadata_ref()
      .iter()
      .map(|(k, v)| wire::KeyValue {
        key: ::buffa::smol_str::SmolStr::from(k.as_str()),
        value: ::buffa::smol_str::SmolStr::from(v.as_str()),
        __buffa_unknown_fields: Default::default(),
      })
      .collect();

    wire::Chapter {
      id: ::buffa::bytes::Bytes::copy_from_slice(c.id_ref().as_bytes()),
      media_id: ::buffa::bytes::Bytes::copy_from_slice(c.media_id_ref().as_bytes()),
      index: c.index(),
      source_id: c.source_id(),
      time_range: ::buffa::MessageField::some(*c.time_range_ref()),
      title: ::buffa::smol_str::SmolStr::from(c.title()),
      metadata,
      __buffa_unknown_fields: Default::default(),
    }
  }
}

fn chapter_err_to_buffa(e: ChapterError) -> BuffaError {
  BuffaError::DomainConstructorRejected(SmolStr::from(e.to_string()))
}
