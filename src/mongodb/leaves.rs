//! Leaf-aggregate mappings: [`WatchedLocation`], [`Speaker`], [`UserTag`],
//! [`SceneAnnotation`]. Each one is small enough to live in this single
//! file.

use ::bson::{Bson, Document};

use crate::domain::{
  aggregates::{
    curation::{SceneAnnotation, UserTag},
    speaker::Speaker,
    watched_location::WatchedLocation,
  },
  enums::ScanStatus,
  Uuid7,
};

use super::{error::MongoError, util::*};

// ---------------------------------------------------------------------------
// `ScanStatus` ↔ Int32 (0/1/2)
// ---------------------------------------------------------------------------

fn scan_status_to_i32(s: ScanStatus) -> i32 {
  match s {
    ScanStatus::Ok => 0,
    ScanStatus::Partial => 1,
    ScanStatus::Failed => 2,
  }
}

fn scan_status_from_i64(v: i64, field: &'static str) -> Result<ScanStatus, MongoError> {
  match v {
    0 => Ok(ScanStatus::Ok),
    1 => Ok(ScanStatus::Partial),
    2 => Ok(ScanStatus::Failed),
    _ => Err(MongoError::IntOutOfRange {
      field: smol_str::SmolStr::from(field),
      value: v,
    }),
  }
}

// ---------------------------------------------------------------------------
// WatchedLocation
// ---------------------------------------------------------------------------

impl From<&WatchedLocation<Uuid7>> for Document {
  fn from(w: &WatchedLocation<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*w.id()));
    d.insert("root", location_to_bson(w.root()));
    d.insert("recursive", Bson::Boolean(w.is_recursive()));
    d.insert("enabled", Bson::Boolean(w.is_enabled()));
    d.insert("is_ejectable", Bson::Boolean(w.is_ejectable()));
    d.insert("added_at", jiff_to_bson(*w.added_at()));
    d.insert(
      "last_reconciled_at",
      w.last_reconciled_at()
        .map(|t| jiff_to_bson(*t))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "last_reconcile_status",
      w.last_reconcile_status()
        .map(|s| Bson::Int32(scan_status_to_i32(*s)))
        .unwrap_or(Bson::Null),
    );
    d.insert(
      "last_error",
      w.last_error().map(error_info_to_bson).unwrap_or(Bson::Null),
    );
    d
  }
}

impl TryFrom<Document> for WatchedLocation<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    // Decode the root into volume + components and re-run through the
    // validating ctor so all invariants are re-enforced.
    let mut root_doc = as_doc(take(&mut d, "root")?, "root")?;
    let _kind = as_str(take(&mut root_doc, "kind")?, "kind")?;
    let volume = uuid7_from_bson(take(&mut root_doc, "volume")?, "volume")?;
    let components = smolstr_vec_from_bson(take(&mut root_doc, "components")?, "components")?;
    let added_at = jiff_from_bson(take(&mut d, "added_at")?, "added_at")?;
    let mut w = WatchedLocation::try_new(id, volume, components, added_at)?;
    if let Some(b) = take_opt(&mut d, "recursive") {
      w.set_recursive(as_bool(b, "recursive")?);
    }
    if let Some(b) = take_opt(&mut d, "enabled") {
      w.set_enabled(as_bool(b, "enabled")?);
    }
    if let Some(b) = take_opt(&mut d, "is_ejectable") {
      w.set_ejectable(as_bool(b, "is_ejectable")?);
    }
    if let Some(b) = take_opt(&mut d, "last_reconciled_at") {
      w.set_last_reconciled_at(Some(jiff_from_bson(b, "last_reconciled_at")?));
    }
    if let Some(b) = take_opt(&mut d, "last_reconcile_status") {
      let v = as_i64(b, "last_reconcile_status")?;
      w.set_last_reconcile_status(Some(scan_status_from_i64(v, "last_reconcile_status")?));
    }
    if let Some(b) = take_opt(&mut d, "last_error") {
      w.set_last_error(Some(error_info_from_bson(b, "last_error")?));
    }
    Ok(w)
  }
}

// ---------------------------------------------------------------------------
// Speaker
// ---------------------------------------------------------------------------

impl From<&Speaker<Uuid7>> for Document {
  fn from(s: &Speaker<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*s.id()));
    d.insert("parent", uuid7_to_bson(*s.parent()));
    d.insert("cluster_id", Bson::Int64(s.cluster_id() as i64));
    d.insert("name", Bson::String(s.name().to_owned()));
    d.insert(
      "speech_duration",
      s.speech_duration()
        .map(|t| media_ts_to_bson(*t))
        .unwrap_or(Bson::Null),
    );
    d
  }
}

impl TryFrom<Document> for Speaker<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let parent = uuid7_from_bson(take(&mut d, "parent")?, "parent")?;
    let cluster_id = as_u32(take(&mut d, "cluster_id")?, "cluster_id")?;
    let name = as_smol(take(&mut d, "name")?, "name")?;
    let mut s = Speaker::try_new(id, parent, cluster_id, name)?;
    if let Some(b) = take_opt(&mut d, "speech_duration") {
      s.set_speech_duration(Some(media_ts_from_bson(b, "speech_duration")?));
    }
    Ok(s)
  }
}

// ---------------------------------------------------------------------------
// UserTag
// ---------------------------------------------------------------------------

impl From<&UserTag<Uuid7>> for Document {
  fn from(t: &UserTag<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*t.id()));
    d.insert("name", Bson::String(t.name().to_owned()));
    d.insert("color", t.color().map(rgba_to_bson).unwrap_or(Bson::Null));
    d.insert("created_at", jiff_to_bson(*t.created_at()));
    d
  }
}

impl TryFrom<Document> for UserTag<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let name = as_smol(take(&mut d, "name")?, "name")?;
    let created_at = jiff_from_bson(take(&mut d, "created_at")?, "created_at")?;
    let mut t = UserTag::try_new(id, name, created_at)?;
    if let Some(b) = take_opt(&mut d, "color") {
      t.set_color(Some(rgba_from_bson(b, "color")?));
    }
    Ok(t)
  }
}

// ---------------------------------------------------------------------------
// SceneAnnotation
// ---------------------------------------------------------------------------

impl From<&SceneAnnotation<Uuid7>> for Document {
  fn from(a: &SceneAnnotation<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*a.id()));
    d.insert("scene", uuid7_to_bson(*a.scene()));
    d.insert("favorite", Bson::Boolean(a.is_favorite()));
    d.insert("user_tags", uuid7_vec_to_bson(a.user_tags()));
    d.insert(
      "rating",
      a.rating()
        .map(|r| Bson::Int32(r as i32))
        .unwrap_or(Bson::Null),
    );
    d.insert("note", Bson::String(a.note().to_owned()));
    d.insert("updated_at", jiff_to_bson(*a.updated_at()));
    d
  }
}

impl TryFrom<Document> for SceneAnnotation<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let scene = uuid7_from_bson(take(&mut d, "scene")?, "scene")?;
    let updated_at = jiff_from_bson(take(&mut d, "updated_at")?, "updated_at")?;
    let mut a = SceneAnnotation::try_new(id, scene, updated_at)?;
    if let Some(b) = take_opt(&mut d, "favorite") {
      a.set_favorite(as_bool(b, "favorite")?);
    }
    if let Some(b) = take_opt(&mut d, "user_tags") {
      a.set_user_tags(uuid7_vec_from_bson(b, "user_tags")?);
    }
    if let Some(b) = take_opt(&mut d, "rating") {
      a.set_rating(Some(as_u8(b, "rating")?));
    }
    if let Some(b) = take_opt(&mut d, "note") {
      a.set_note(as_smol(b, "note")?);
    }
    Ok(a)
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::{
    primitives::{ErrorCode, ErrorInfo},
    Rgba,
  };
  use core::num::NonZeroU32;
  use jiff::Timestamp as JiffTimestamp;
  use mediatime::{Timebase, Timestamp as MediaTimestamp};

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).unwrap())
  }

  #[test]
  fn watched_location_roundtrip() {
    let id = Uuid7::new();
    let vol = Uuid7::new();
    let w = WatchedLocation::try_new(id, vol, ["Movies", "2024"], JiffTimestamp::default())
      .unwrap()
      .with_enabled(true)
      .with_recursive(true)
      .with_ejectable(true)
      .with_last_reconciled_at(Some(JiffTimestamp::default()))
      .with_last_reconcile_status(Some(ScanStatus::Partial))
      .with_last_error(Some(ErrorInfo::new(
        ErrorCode::VolumeNotAvailable,
        "offline",
      )));
    let doc: Document = (&w).into();
    let w2: WatchedLocation<Uuid7> = doc.try_into().unwrap();
    assert_eq!(w, w2);
  }

  #[test]
  fn watched_location_missing_field_errors() {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(Uuid7::new()));
    let err = WatchedLocation::<Uuid7>::try_from(d).unwrap_err();
    assert!(err.is_missing_field());
  }

  #[test]
  fn watched_location_nil_id_rejected() {
    // Encode a fake nil-id document by hand (the From impl can't
    // produce one because `WatchedLocation::try_new` would have
    // rejected the source value).
    let mut d = Document::new();
    d.insert(
      "_id",
      Bson::Binary(::bson::Binary {
        subtype: ::bson::spec::BinarySubtype::Uuid,
        bytes: vec![0u8; 16],
      }),
    );
    let mut root = Document::new();
    root.insert("kind", "local");
    root.insert("volume", uuid7_to_bson(Uuid7::new()));
    root.insert(
      "components",
      Bson::Array(vec![Bson::String("Movies".into())]),
    );
    d.insert("root", root);
    d.insert("added_at", jiff_to_bson(JiffTimestamp::default()));
    // Decode rejects nil at Uuid7 layer (which wraps validate_v7).
    let err = WatchedLocation::<Uuid7>::try_from(d).unwrap_err();
    assert!(err.is_uuid_7());
  }

  #[test]
  fn speaker_roundtrip() {
    let s = Speaker::try_new(Uuid7::new(), Uuid7::new(), 3, "Jane")
      .unwrap()
      .with_speech_duration(Some(MediaTimestamp::new(12000, tb())));
    let doc: Document = (&s).into();
    let s2: Speaker<Uuid7> = doc.try_into().unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn user_tag_roundtrip() {
    let t = UserTag::try_new(Uuid7::new(), "Vacation", JiffTimestamp::default())
      .unwrap()
      .with_color(Some(Rgba::from_components(0xff, 0x88, 0x00, 0xff)));
    let doc: Document = (&t).into();
    let t2: UserTag<Uuid7> = doc.try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn scene_annotation_roundtrip() {
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let a = SceneAnnotation::try_new(Uuid7::new(), Uuid7::new(), JiffTimestamp::default())
      .unwrap()
      .with_favorite(true)
      .with_user_tags(vec![t1, t2])
      .with_rating(Some(4))
      .with_note("great driving scene");
    let doc: Document = (&a).into();
    let a2: SceneAnnotation<Uuid7> = doc.try_into().unwrap();
    assert_eq!(a, a2);
  }
}
