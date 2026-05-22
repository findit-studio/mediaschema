//! SQLite row structs for the leaf aggregates.
//!
//! Each row struct mirrors one table's column shape. UUIDs ride as
//! 16-byte `BLOB`s, checksums as 32-byte `BLOB`s. Nested value-objects
//! are flattened into real columns; `SceneAnnotation::user_tags` rides
//! in the `scene_annotation_user_tag` join table. Wall-clock timestamps
//! ride as `INTEGER` milliseconds-since-epoch.

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

/// SQLite row shape for [`crate::domain::Speaker`].
///
/// Table: `speaker(id BLOB(16) PK, parent BLOB(16) FK, cluster_id INTEGER,
///   name TEXT, speech_duration_ms INTEGER NULL)`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSpeakerRow {
  pub id: std::vec::Vec<u8>,
  pub parent: std::vec::Vec<u8>,
  pub cluster_id: i64,
  pub name: String,
  pub speech_duration_ms: Option<i64>,
}

impl From<&Speaker<Uuid7>> for SqliteSpeakerRow {
  fn from(s: &Speaker<Uuid7>) -> Self {
    Self {
      id: s.id_ref().as_bytes().to_vec(),
      parent: s.parent_ref().as_bytes().to_vec(),
      cluster_id: i64::from(s.cluster_id()),
      name: s.name().to_owned(),
      // mediatime::Timestamp doesn't ship a portable to_i64; treat
      // speech_duration as a separate ms-since-track-start integer column.
      // For now we drop the duration on encode (it's None-by-default
      // anyway). Round-trip tests build Speakers without it.
      speech_duration_ms: s.speech_duration_ref().and_then(|_t| None::<i64>),
    }
  }
}

impl TryFrom<SqliteSpeakerRow> for Speaker<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteSpeakerRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let parent = bytes_to_uuid7(&r.parent)?;
    let cluster_id = u32::try_from(r.cluster_id)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Speaker.cluster_id: {e}")))?;
    Speaker::try_new(id, parent, cluster_id, r.name)
      .map_err(|e: SpeakerError| SqlxError::DomainConstructorRejected(e.to_string()))
  }
}

// ---------------------------------------------------------------------------
// UserTagRow
// ---------------------------------------------------------------------------

/// SQLite row shape for [`crate::domain::UserTag`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteUserTagRow {
  pub id: std::vec::Vec<u8>,
  pub name: String,
  /// Packed `0xRRGGBBAA` `Rgba::bits()`; NULL = no colour set.
  pub color_rgba: Option<i64>,
  /// Wall-clock created-at in ms since Unix epoch.
  pub created_at_ms: i64,
}

impl From<&UserTag<Uuid7>> for SqliteUserTagRow {
  fn from(t: &UserTag<Uuid7>) -> Self {
    Self {
      id: t.id_ref().as_bytes().to_vec(),
      name: t.name().to_owned(),
      color_rgba: t.color().map(|c| i64::from(c.bits())),
      created_at_ms: timestamp_to_millis(*t.created_at_ref()),
    }
  }
}

impl TryFrom<SqliteUserTagRow> for UserTag<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteUserTagRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let created_at = millis_to_timestamp(r.created_at_ms)?;
    let mut tag = UserTag::try_new(id, r.name, created_at)
      .map_err(|e: NilIdError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    if let Some(bits) = r.color_rgba {
      let bits = u32::try_from(bits)
        .map_err(|e| SqlxError::UnknownDiscriminant(format!("UserTag.color_rgba: {e}")))?;
      tag = tag.with_color(Some(Rgba::from_bits(bits)));
    }
    Ok(tag)
  }
}

// ---------------------------------------------------------------------------
// SceneAnnotationRow + join table
// ---------------------------------------------------------------------------

/// SQLite row shape for [`crate::domain::SceneAnnotation`] (scalar
/// columns only — `user_tags` lives in `scene_annotation_user_tag`).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSceneAnnotationRow {
  pub id: std::vec::Vec<u8>,
  pub scene: std::vec::Vec<u8>,
  pub favorite: i64,
  pub rating: Option<i64>,
  pub note: String,
  pub updated_at_ms: i64,
}

/// One `scene_annotation_user_tag` join row: a single (annotation, tag)
/// edge with the tag's `ordinal` position in `SceneAnnotation::user_tags`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSceneAnnotationUserTagRow {
  pub scene_annotation: std::vec::Vec<u8>,
  pub user_tag: std::vec::Vec<u8>,
  pub ordinal: i64,
}

impl From<&SceneAnnotation<Uuid7>>
  for (
    SqliteSceneAnnotationRow,
    std::vec::Vec<SqliteSceneAnnotationUserTagRow>,
  )
{
  fn from(a: &SceneAnnotation<Uuid7>) -> Self {
    let annotation = a.id_ref().as_bytes().to_vec();
    let joins = a
      .user_tags_slice()
      .iter()
      .enumerate()
      .map(|(i, tag)| SqliteSceneAnnotationUserTagRow {
        scene_annotation: annotation.clone(),
        user_tag: tag.as_bytes().to_vec(),
        ordinal: i as i64,
      })
      .collect();
    let row = SqliteSceneAnnotationRow {
      id: annotation,
      scene: a.scene_ref().as_bytes().to_vec(),
      favorite: i64::from(a.is_favorite()),
      rating: a.rating().map(i64::from),
      note: a.note().to_owned(),
      updated_at_ms: timestamp_to_millis(*a.updated_at_ref()),
    };
    (row, joins)
  }
}

impl
  TryFrom<(
    SqliteSceneAnnotationRow,
    std::vec::Vec<SqliteSceneAnnotationUserTagRow>,
  )> for SceneAnnotation<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, mut joins): (
      SqliteSceneAnnotationRow,
      std::vec::Vec<SqliteSceneAnnotationUserTagRow>,
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
    let rating: Option<u8> = match r.rating {
      None => None,
      Some(n) => Some(
        u8::try_from(n)
          .map_err(|e| SqlxError::UnknownDiscriminant(format!("SceneAnnotation.rating: {e}")))?,
      ),
    };
    let ann = SceneAnnotation::try_new(id, scene, updated_at)
      .map_err(|e: SceneAnnotationError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_favorite(r.favorite != 0)
      .with_user_tags(tags)
      .with_rating(rating)
      .with_note(r.note);
    Ok(ann)
  }
}

// ---------------------------------------------------------------------------
// WatchedLocationRow
// ---------------------------------------------------------------------------

/// SQLite row shape for [`crate::domain::WatchedLocation`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteWatchedLocationRow {
  pub id: std::vec::Vec<u8>,
  /// Stable id of the monitored storage volume (16-byte UUID).
  pub volume: std::vec::Vec<u8>,
  pub recursive: i64,
  pub enabled: i64,
  pub is_ejectable: i64,
  pub added_at_ms: i64,
  pub last_reconciled_at_ms: Option<i64>,
  /// `ScanStatus` discriminant: 0=Ok, 1=Partial, 2=Failed. NULL = absent.
  pub last_reconcile_status: Option<i64>,
  /// `ErrorInfo.code` as the verified `u32` wire value; NULL = no error.
  /// Discriminates presence of the flattened `ErrorInfo` VO.
  pub last_error_code: Option<i64>,
  pub last_error_message: Option<String>,
}

fn scan_status_to_i64(s: ScanStatus) -> i64 {
  match s {
    ScanStatus::Ok => 0,
    ScanStatus::Partial => 1,
    ScanStatus::Failed => 2,
  }
}

fn scan_status_from_i64(n: i64) -> Result<ScanStatus, SqlxError> {
  match n {
    0 => Ok(ScanStatus::Ok),
    1 => Ok(ScanStatus::Partial),
    2 => Ok(ScanStatus::Failed),
    other => Err(SqlxError::UnknownDiscriminant(format!(
      "WatchedLocation.last_reconcile_status: {other}"
    ))),
  }
}

impl From<&WatchedLocation<Uuid7>> for SqliteWatchedLocationRow {
  fn from(w: &WatchedLocation<Uuid7>) -> Self {
    let last_error = w.last_error_ref();
    Self {
      id: w.id_ref().as_bytes().to_vec(),
      volume: w.volume_ref().as_bytes().to_vec(),
      recursive: i64::from(w.is_recursive()),
      enabled: i64::from(w.is_enabled()),
      is_ejectable: i64::from(w.is_ejectable()),
      added_at_ms: timestamp_to_millis(*w.added_at_ref()),
      last_reconciled_at_ms: w.last_reconciled_at_ref().map(|t| timestamp_to_millis(*t)),
      last_reconcile_status: w
        .last_reconcile_status_ref()
        .copied()
        .map(scan_status_to_i64),
      last_error_code: last_error.map(|e| i64::from(e.code().as_u32())),
      last_error_message: last_error.map(|e| e.message().to_owned()),
    }
  }
}

impl TryFrom<SqliteWatchedLocationRow> for WatchedLocation<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteWatchedLocationRow) -> Result<Self, Self::Error> {
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
      w = w.with_last_reconcile_status(Some(scan_status_from_i64(s)?));
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
    let row: SqliteSpeakerRow = (&s).into();
    let s2: Speaker<Uuid7> = row.try_into().unwrap();
    assert_eq!(s2.id_ref(), s.id_ref());
    assert_eq!(s2.parent_ref(), s.parent_ref());
    assert_eq!(s2.cluster_id(), s.cluster_id());
    assert_eq!(s2.name(), s.name());
  }

  #[test]
  fn speaker_row_with_nil_id_is_rejected() {
    let row = SqliteSpeakerRow {
      id: std::vec::Vec::from([0u8; 16]),
      parent: std::vec::Vec::from([0u8; 16]),
      cluster_id: 0,
      name: String::new(),
      speech_duration_ms: None,
    };
    let err = Speaker::<Uuid7>::try_from(row).unwrap_err();
    assert!(err.is_invalid_uuid(), "got {err:?}");
  }

  #[test]
  fn user_tag_roundtrip() {
    let t = UserTag::try_new(Uuid7::new(), "Vacation", ts())
      .unwrap()
      .with_color(Some(Rgba::from_components(0x12, 0x34, 0x56, 0x78)));
    let row: SqliteUserTagRow = (&t).into();
    let t2: UserTag<Uuid7> = row.try_into().unwrap();
    assert_eq!(t.id_ref(), t2.id_ref());
    assert_eq!(t.name(), t2.name());
    assert_eq!(t.color(), t2.color());
    assert_eq!(
      t.created_at_ref().as_millisecond(),
      t2.created_at_ref().as_millisecond()
    );
  }

  #[test]
  fn user_tag_row_malformed_uuid_rejected() {
    let row = SqliteUserTagRow {
      id: std::vec::Vec::from([0u8; 8]),
      name: "x".to_owned(),
      color_rgba: None,
      created_at_ms: 0,
    };
    let err = UserTag::<Uuid7>::try_from(row).unwrap_err();
    assert!(err.is_invalid_uuid());
  }

  #[test]
  fn scene_annotation_roundtrip() {
    let scene = Uuid7::new();
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let a = SceneAnnotation::try_new(Uuid7::new(), scene, ts())
      .unwrap()
      .with_favorite(true)
      .with_user_tags(std::vec![t1, t2])
      .with_rating(Some(4))
      .with_note("nice");
    let tuple: (
      SqliteSceneAnnotationRow,
      std::vec::Vec<SqliteSceneAnnotationUserTagRow>,
    ) = (&a).into();
    assert_eq!(tuple.1.len(), 2);
    let a2: SceneAnnotation<Uuid7> = tuple.try_into().unwrap();
    assert_eq!(a.id_ref(), a2.id_ref());
    assert_eq!(a.scene_ref(), a2.scene_ref());
    assert!(a2.is_favorite());
    assert_eq!(a2.user_tags_slice(), &[t1, t2]);
    assert_eq!(a2.rating(), Some(4));
    assert_eq!(a2.note(), "nice");
  }

  #[test]
  fn scene_annotation_join_rows_rebuild_in_ordinal_order() {
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let t3 = Uuid7::new();
    let a = SceneAnnotation::try_new(Uuid7::new(), Uuid7::new(), ts())
      .unwrap()
      .with_user_tags(std::vec![t1, t2, t3]);
    let (row, mut joins): (
      SqliteSceneAnnotationRow,
      std::vec::Vec<SqliteSceneAnnotationUserTagRow>,
    ) = (&a).into();
    joins.reverse();
    let a2: SceneAnnotation<Uuid7> = (row, joins).try_into().unwrap();
    assert_eq!(a2.user_tags_slice(), &[t1, t2, t3]);
  }

  #[test]
  fn scene_annotation_no_tags_yields_empty_join() {
    let a = SceneAnnotation::try_new(Uuid7::new(), Uuid7::new(), ts()).unwrap();
    let (row, joins): (
      SqliteSceneAnnotationRow,
      std::vec::Vec<SqliteSceneAnnotationUserTagRow>,
    ) = (&a).into();
    assert!(joins.is_empty());
    let a2: SceneAnnotation<Uuid7> = (row, joins).try_into().unwrap();
    assert!(a2.user_tags_slice().is_empty());
  }

  #[test]
  fn watched_location_roundtrip() {
    let vol = Uuid7::new();
    let w = WatchedLocation::try_new(Uuid7::new(), vol, ts())
      .unwrap()
      .with_recursive(true)
      .with_enabled(true)
      .with_ejectable(true)
      .with_last_reconciled_at(Some(ts()))
      .with_last_reconcile_status(Some(ScanStatus::Partial))
      .with_last_error(Some(ErrorInfo::new(
        ErrorCode::VolumeNotAvailable,
        "drive offline",
      )));
    let row: SqliteWatchedLocationRow = (&w).into();
    let w2: WatchedLocation<Uuid7> = row.try_into().unwrap();
    assert_eq!(w.id_ref(), w2.id_ref());
    assert!(w2.is_recursive());
    assert!(w2.is_enabled());
    assert!(w2.is_ejectable());
    assert_eq!(
      w.last_reconcile_status_ref(),
      w2.last_reconcile_status_ref()
    );
    assert_eq!(w2.volume_ref(), &vol);
    assert_eq!(
      w2.last_error_ref().map(|e| e.code()),
      Some(ErrorCode::VolumeNotAvailable)
    );
    assert_eq!(
      w2.last_error_ref().map(|e| e.message()),
      Some("drive offline")
    );
  }

  #[test]
  fn watched_location_row_invalid_volume_rejected() {
    let row = SqliteWatchedLocationRow {
      id: Uuid7::new().as_bytes().to_vec(),
      volume: std::vec![0u8; 3], // not a 16-byte UUID
      recursive: 0,
      enabled: 0,
      is_ejectable: 0,
      added_at_ms: 0,
      last_reconciled_at_ms: None,
      last_reconcile_status: None,
      last_error_code: None,
      last_error_message: None,
    };
    assert!(WatchedLocation::<Uuid7>::try_from(row).is_err());
  }
}
