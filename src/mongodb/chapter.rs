//! `Chapter` ↔ bson `Document` mapping.
//!
//! BSON documents are insertion-ordered, so the [`IndexMap`] metadata
//! bag round-trips natively through a nested `metadata` sub-document
//! (`{ key: value, … }`) without an explicit ordinal field.

use ::bson::{Bson, Document};

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::domain::{aggregates::chapter::Chapter, Uuid7};

use super::{error::MongoError, util::*};

impl From<&Chapter<Uuid7>> for Document {
  fn from(c: &Chapter<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*c.id_ref()));
    d.insert("media_id", uuid7_to_bson(*c.media_id_ref()));
    d.insert("index", Bson::Int64(i64::from(c.index())));
    d.insert("source_id", Bson::Int64(c.source_id()));
    d.insert("time_range", time_range_to_bson(c.time_range_ref()));
    d.insert("title", Bson::String(c.title().to_owned()));

    // BSON documents preserve insertion order — IndexMap round-trips
    // natively. Iterate the source IndexMap in order.
    let mut bag = Document::new();
    for (k, v) in c.metadata_ref() {
      bag.insert(k.as_str(), Bson::String(v.as_str().to_owned()));
    }
    d.insert("metadata", Bson::Document(bag));
    d
  }
}

impl TryFrom<Document> for Chapter<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let media_id = uuid7_from_bson(take(&mut d, "media_id")?, "media_id")?;
    let index = as_u32(take(&mut d, "index")?, "index")?;
    let source_id = as_i64(take(&mut d, "source_id")?, "source_id")?;
    let time_range = time_range_from_bson(take(&mut d, "time_range")?, "time_range")?;

    let chapter = Chapter::try_new(id, media_id, index, source_id, time_range)
      .map_err(|e| MongoError::DomainConstructorRejected(e.to_string()))?;

    let title = if let Some(b) = take_opt(&mut d, "title") {
      as_str(b, "title")?
    } else {
      String::new()
    };
    let chapter = chapter
      .try_with_title(title)
      .map_err(|e| MongoError::DomainConstructorRejected(e.to_string()))?;

    let metadata = if let Some(b) = take_opt(&mut d, "metadata") {
      let inner = as_doc(b, "metadata")?;
      // BSON documents preserve iteration order — fold into IndexMap.
      let mut bag = IndexMap::with_capacity(inner.len());
      for (k, v) in inner {
        let value = as_str(v, "metadata.value")?;
        bag.insert(SmolStr::from(k), SmolStr::from(value));
      }
      bag
    } else {
      IndexMap::new()
    };
    chapter
      .try_with_metadata(metadata)
      .map_err(|e| MongoError::DomainConstructorRejected(e.to_string()))
  }
}
