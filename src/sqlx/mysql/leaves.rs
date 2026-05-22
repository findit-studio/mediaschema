//! MySQL row structs for the leaf aggregates.
//!
//! UUIDs are `BINARY(16)` (`Vec<u8>` in sqlx), checksums `BINARY(32)`.
//! Nested value-objects are flattened into real columns;
//! `SceneAnnotation::user_tags` rides in the `scene_annotation_user_tag`
//! join table. Wall-clock timestamps are `BIGINT` ms-since-epoch (`i64`).

use crate::{
  domain::{
    aggregates::{
      curation::{NilIdError, SceneAnnotationError},
      speaker::SpeakerError,
      watched_location::WatchedLocationError,
    },
    ErrorCode, ErrorInfo, Rgba, ScanStatus, SceneAnnotation, Speaker, UserTag, Uuid7,
    WatchedLocation,
  },
  sqlx::{
    dto::{bytes_to_uuid7, millis_to_timestamp, timestamp_to_millis},
    SqlxError,
  },
};

// ---------------------------------------------------------------------------
// SpeakerRow
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSpeakerRow {
  pub id: std::vec::Vec<u8>,
  pub parent: std::vec::Vec<u8>,
  pub cluster_id: u32,
  pub name: String,
  pub speech_duration_ms: Option<i64>,
}

impl From<&Speaker<Uuid7>> for MySqlSpeakerRow {
  fn from(s: &Speaker<Uuid7>) -> Self {
    Self {
      id: s.id_ref().as_bytes().to_vec(),
      parent: s.parent_ref().as_bytes().to_vec(),
      cluster_id: s.cluster_id(),
      name: s.name().to_owned(),
      speech_duration_ms: s.speech_duration_ref().and_then(|_| None::<i64>),
    }
  }
}

impl TryFrom<MySqlSpeakerRow> for Speaker<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: MySqlSpeakerRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let parent = bytes_to_uuid7(&r.parent)?;
    Speaker::try_new(id, parent, r.cluster_id, r.name)
      .map_err(|e: SpeakerError| SqlxError::DomainConstructorRejected(e.to_string()))
  }
}

// ---------------------------------------------------------------------------
// UserTagRow
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlUserTagRow {
  pub id: std::vec::Vec<u8>,
  pub name: String,
  pub color_rgba: Option<u32>,
  pub created_at_ms: i64,
}

impl From<&UserTag<Uuid7>> for MySqlUserTagRow {
  fn from(t: &UserTag<Uuid7>) -> Self {
    Self {
      id: t.id_ref().as_bytes().to_vec(),
      name: t.name().to_owned(),
      color_rgba: t.color().map(|c| c.bits()),
      created_at_ms: timestamp_to_millis(*t.created_at_ref()),
    }
  }
}

impl TryFrom<MySqlUserTagRow> for UserTag<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: MySqlUserTagRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let created_at = millis_to_timestamp(r.created_at_ms)?;
    let mut tag = UserTag::try_new(id, r.name, created_at)
      .map_err(|e: NilIdError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    if let Some(bits) = r.color_rgba {
      tag = tag.with_color(Some(Rgba::from_bits(bits)));
    }
    Ok(tag)
  }
}

// ---------------------------------------------------------------------------
// SceneAnnotationRow + join table
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSceneAnnotationRow {
  pub id: std::vec::Vec<u8>,
  pub scene: std::vec::Vec<u8>,
  pub favorite: i8,
  pub rating: Option<u8>,
  pub note: String,
  pub updated_at_ms: i64,
}

/// One `scene_annotation_user_tag` join row: a single (annotation, tag)
/// edge with the tag's `ordinal` position in `SceneAnnotation::user_tags`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSceneAnnotationUserTagRow {
  pub scene_annotation: std::vec::Vec<u8>,
  pub user_tag: std::vec::Vec<u8>,
  pub ordinal: i32,
}

impl From<&SceneAnnotation<Uuid7>>
  for (
    MySqlSceneAnnotationRow,
    std::vec::Vec<MySqlSceneAnnotationUserTagRow>,
  )
{
  fn from(a: &SceneAnnotation<Uuid7>) -> Self {
    let annotation = a.id_ref().as_bytes().to_vec();
    let joins = a
      .user_tags_slice()
      .iter()
      .enumerate()
      .map(|(i, tag)| MySqlSceneAnnotationUserTagRow {
        scene_annotation: annotation.clone(),
        user_tag: tag.as_bytes().to_vec(),
        ordinal: i as i32,
      })
      .collect();
    let row = MySqlSceneAnnotationRow {
      id: annotation,
      scene: a.scene_ref().as_bytes().to_vec(),
      favorite: i8::from(a.is_favorite()),
      rating: a.rating(),
      note: a.note().to_owned(),
      updated_at_ms: timestamp_to_millis(*a.updated_at_ref()),
    };
    (row, joins)
  }
}

impl
  TryFrom<(
    MySqlSceneAnnotationRow,
    std::vec::Vec<MySqlSceneAnnotationUserTagRow>,
  )> for SceneAnnotation<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, mut joins): (
      MySqlSceneAnnotationRow,
      std::vec::Vec<MySqlSceneAnnotationUserTagRow>,
    ),
  ) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let scene = bytes_to_uuid7(&r.scene)?;
    let updated_at = millis_to_timestamp(r.updated_at_ms)?;
    joins.sort_by_key(|j| j.ordinal);
    let mut tags = std::vec::Vec::with_capacity(joins.len());
    for j in joins {
      tags.push(bytes_to_uuid7(&j.user_tag)?);
    }
    let ann = SceneAnnotation::try_new(id, scene, updated_at)
      .map_err(|e: SceneAnnotationError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_favorite(r.favorite != 0)
      .with_user_tags(tags)
      .with_rating(r.rating)
      .with_note(r.note);
    Ok(ann)
  }
}

// ---------------------------------------------------------------------------
// WatchedLocationRow
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlWatchedLocationRow {
  pub id: std::vec::Vec<u8>,
  pub volume: std::vec::Vec<u8>,
  pub recursive: i8,
  pub enabled: i8,
  pub is_ejectable: i8,
  pub added_at_ms: i64,
  pub last_reconciled_at_ms: Option<i64>,
  pub last_reconcile_status: Option<i16>,
  /// `ErrorInfo.code` as the verified `u32` wire value; NULL = no error.
  /// Discriminates presence of the flattened `ErrorInfo` VO.
  pub last_error_code: Option<i32>,
  pub last_error_message: Option<String>,
}

fn scan_status_to_i16(s: ScanStatus) -> i16 {
  match s {
    ScanStatus::Ok => 0,
    ScanStatus::Partial => 1,
    ScanStatus::Failed => 2,
  }
}

fn scan_status_from_i16(n: i16) -> Result<ScanStatus, SqlxError> {
  match n {
    0 => Ok(ScanStatus::Ok),
    1 => Ok(ScanStatus::Partial),
    2 => Ok(ScanStatus::Failed),
    other => Err(SqlxError::UnknownDiscriminant(format!(
      "WatchedLocation.last_reconcile_status: {other}"
    ))),
  }
}

impl From<&WatchedLocation<Uuid7>> for MySqlWatchedLocationRow {
  fn from(w: &WatchedLocation<Uuid7>) -> Self {
    let last_error = w.last_error_ref();
    Self {
      id: w.id_ref().as_bytes().to_vec(),
      volume: w.volume_ref().as_bytes().to_vec(),
      recursive: i8::from(w.is_recursive()),
      enabled: i8::from(w.is_enabled()),
      is_ejectable: i8::from(w.is_ejectable()),
      added_at_ms: timestamp_to_millis(*w.added_at_ref()),
      last_reconciled_at_ms: w.last_reconciled_at_ref().map(|t| timestamp_to_millis(*t)),
      last_reconcile_status: w
        .last_reconcile_status_ref()
        .copied()
        .map(scan_status_to_i16),
      last_error_code: last_error.map(|e| e.code().as_u32() as i32),
      last_error_message: last_error.map(|e| e.message().to_owned()),
    }
  }
}

impl TryFrom<MySqlWatchedLocationRow> for WatchedLocation<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: MySqlWatchedLocationRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let volume = bytes_to_uuid7(&r.volume)?;
    let added_at = millis_to_timestamp(r.added_at_ms)?;
    let mut w = WatchedLocation::try_new(id, volume, added_at)
      .map_err(|e: WatchedLocationError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_recursive(r.recursive != 0)
      .with_enabled(r.enabled != 0)
      .with_ejectable(r.is_ejectable != 0);
    if let Some(ms) = r.last_reconciled_at_ms {
      w = w.with_last_reconciled_at(Some(millis_to_timestamp(ms)?));
    }
    if let Some(s) = r.last_reconcile_status {
      w = w.with_last_reconcile_status(Some(scan_status_from_i16(s)?));
    }
    if let Some(code) = r.last_error_code {
      let code = u32::try_from(code).map_err(|e| {
        SqlxError::UnknownDiscriminant(format!("WatchedLocation.last_error_code: {e}"))
      })?;
      w = w.with_last_error(Some(ErrorInfo::new(
        ErrorCode::from_u32(code),
        r.last_error_message.unwrap_or_default(),
      )));
    }
    Ok(w)
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;
  use jiff::Timestamp as JiffTimestamp;

  fn ts() -> JiffTimestamp {
    JiffTimestamp::from_millisecond(1_700_000_000_000).unwrap()
  }

  #[test]
  fn speaker_roundtrip() {
    let parent = Uuid7::new();
    let s = Speaker::try_new(Uuid7::new(), parent, 3, "Jane").unwrap();
    let row: MySqlSpeakerRow = (&s).into();
    let s2: Speaker<Uuid7> = row.try_into().unwrap();
    assert_eq!(s2.id_ref(), s.id_ref());
    assert_eq!(s2.parent_ref(), s.parent_ref());
    assert_eq!(s2.cluster_id(), s.cluster_id());
    assert_eq!(s2.name(), s.name());
  }

  #[test]
  fn user_tag_roundtrip() {
    let t = UserTag::try_new(Uuid7::new(), "v", ts())
      .unwrap()
      .with_color(Some(Rgba::from_bits(0xdeadbeef)));
    let row: MySqlUserTagRow = (&t).into();
    let t2: UserTag<Uuid7> = row.try_into().unwrap();
    assert_eq!(t.id_ref(), t2.id_ref());
    assert_eq!(t.color(), t2.color());
  }

  #[test]
  fn scene_annotation_roundtrip() {
    let t1 = Uuid7::new();
    let a = SceneAnnotation::try_new(Uuid7::new(), Uuid7::new(), ts())
      .unwrap()
      .with_favorite(true)
      .with_user_tags(std::vec![t1])
      .with_rating(Some(5));
    let tuple: (
      MySqlSceneAnnotationRow,
      std::vec::Vec<MySqlSceneAnnotationUserTagRow>,
    ) = (&a).into();
    assert_eq!(tuple.1.len(), 1);
    let a2: SceneAnnotation<Uuid7> = tuple.try_into().unwrap();
    assert_eq!(a2.user_tags_slice(), &[t1]);
    assert_eq!(a2.rating(), Some(5));
    assert!(a2.is_favorite());
  }

  #[test]
  fn scene_annotation_join_rows_rebuild_in_ordinal_order() {
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let a = SceneAnnotation::try_new(Uuid7::new(), Uuid7::new(), ts())
      .unwrap()
      .with_user_tags(std::vec![t1, t2]);
    let (row, mut joins): (
      MySqlSceneAnnotationRow,
      std::vec::Vec<MySqlSceneAnnotationUserTagRow>,
    ) = (&a).into();
    joins.reverse();
    let a2: SceneAnnotation<Uuid7> = (row, joins).try_into().unwrap();
    assert_eq!(a2.user_tags_slice(), &[t1, t2]);
  }

  #[test]
  fn watched_location_roundtrip() {
    let vol = Uuid7::new();
    let w = WatchedLocation::try_new(Uuid7::new(), vol, ts())
      .unwrap()
      .with_enabled(true)
      .with_last_error(Some(ErrorInfo::new(ErrorCode::PathNotFound, "gone")));
    let row: MySqlWatchedLocationRow = (&w).into();
    let w2: WatchedLocation<Uuid7> = row.try_into().unwrap();
    assert!(w2.is_enabled());
    assert_eq!(
      w2.last_error_ref().map(|e| e.code()),
      Some(ErrorCode::PathNotFound)
    );
    assert_eq!(w2.last_error_ref().map(|e| e.message()), Some("gone"));
  }

  #[test]
  fn speaker_row_with_nil_uuid_rejected() {
    let row = MySqlSpeakerRow {
      id: std::vec::Vec::from([0u8; 16]),
      parent: Uuid7::new().as_bytes().to_vec(),
      cluster_id: 0,
      name: String::new(),
      speech_duration_ms: None,
    };
    assert!(Speaker::<Uuid7>::try_from(row)
      .unwrap_err()
      .is_invalid_uuid());
  }
}
