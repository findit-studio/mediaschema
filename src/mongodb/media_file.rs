//! `MediaFile` ↔ bson `Document` mapping.
//!
//! A [`MediaFile`] is one **physical copy** of a piece of content (N
//! copies ↔ 1 [`Media`](crate::domain::Media)). See `schema/media_file.md`
//! for the locked spec.
//!
//! MongoDB is a document database, so the structured `location` is kept
//! as a natural embedded sub-document (queryable / indexable) rather than
//! flattened — unlike the SQL backend, which flattens it because columns
//! demand it. The discovering-watch FK (`watched_location_id`) and the
//! cached `watch_volume` are stored as Binary UUIDs.
//!
//! `name` is **derived** from `location`'s last component, not stored.

use ::bson::{Bson, Document};

use crate::domain::{aggregates::media_file::MediaFile, Uuid7};

use super::{error::MongoError, util::*};

// ---------------------------------------------------------------------------
// MediaFile
// ---------------------------------------------------------------------------

impl From<&MediaFile<Uuid7>> for Document {
  fn from(f: &MediaFile<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*f.id_ref()));
    d.insert("media_id", uuid7_to_bson(*f.media_id_ref()));
    d.insert(
      "created_at",
      f.created_at_ref()
        .map(|t| jiff_to_bson(*t))
        .unwrap_or(Bson::Null),
    );
    d.insert("location", location_to_bson(f.location_ref()));
    d.insert(
      "watched_location_id",
      uuid7_to_bson(*f.watched_location_id_ref()),
    );
    d.insert("watch_volume", uuid7_to_bson(*f.watch_volume_ref()));
    d
  }
}

impl TryFrom<Document> for MediaFile<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let media_id = uuid7_from_bson(take(&mut d, "media_id")?, "media_id")?;
    let created_at = opt(take_opt(&mut d, "created_at"), |b| {
      jiff_from_bson(b, "created_at")
    })?;
    let location = location_from_bson(take(&mut d, "location")?, "location")?;
    let watched_location_id =
      uuid7_from_bson(take(&mut d, "watched_location_id")?, "watched_location_id")?;
    let watch_volume = uuid7_from_bson(take(&mut d, "watch_volume")?, "watch_volume")?;
    // `from_parts` is the raw storage-reconstruction constructor: the
    // document was validated by `try_new` when first written, so the
    // `WatchedLocation`-indirection / volume re-check is not repeated here.
    Ok(MediaFile::from_parts(
      id,
      media_id,
      created_at,
      location,
      watched_location_id,
      watch_volume,
    ))
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use crate::domain::{Location, WatchedLocation};
  use jiff::Timestamp as JiffTimestamp;

  fn loc(volume: Uuid7) -> Location<Uuid7> {
    Location::try_local_uuid7(volume, ["Movies", "clip.mp4"]).expect("valid location")
  }

  fn watch(volume: Uuid7) -> WatchedLocation<Uuid7> {
    WatchedLocation::try_new(Uuid7::new(), volume, JiffTimestamp::default()).expect("valid watch")
  }

  #[test]
  fn media_file_minimal_roundtrip() {
    let vol = Uuid7::new();
    let wl = watch(vol);
    let f = MediaFile::try_new(Uuid7::new(), Uuid7::new(), None, loc(vol), &wl).unwrap();
    let doc: Document = (&f).into();
    let f2: MediaFile<Uuid7> = doc.try_into().unwrap();
    assert_eq!(f, f2);
  }

  #[test]
  fn media_file_full_roundtrip() {
    let vol = Uuid7::new();
    let wl = watch(vol);
    let created = JiffTimestamp::from_millisecond(1_700_000_000_000).unwrap();
    let f = MediaFile::try_new(Uuid7::new(), Uuid7::new(), Some(created), loc(vol), &wl).unwrap();
    let doc: Document = (&f).into();
    let f2: MediaFile<Uuid7> = doc.try_into().unwrap();
    assert_eq!(f, f2);
    assert_eq!(f2.name(), "clip.mp4");
  }

  #[test]
  fn media_file_missing_id_errors() {
    let mut d = Document::new();
    d.insert("media_id", uuid7_to_bson(Uuid7::new()));
    let err = MediaFile::<Uuid7>::try_from(d).unwrap_err();
    assert!(err.is_missing_field());
  }

  #[test]
  fn media_file_missing_location_errors() {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(Uuid7::new()));
    d.insert("media_id", uuid7_to_bson(Uuid7::new()));
    let err = MediaFile::<Uuid7>::try_from(d).unwrap_err();
    assert!(err.is_missing_field());
  }
}
