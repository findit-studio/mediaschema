//! PostgreSQL row structs for the leaf aggregates.
//!
//! UUIDs are native `uuid` (`uuid::Uuid`), checksums are `BYTEA`
//! (`Vec<u8>`). Nested value-objects are flattened into real columns;
//! `SceneAnnotation::user_tags` rides in the `scene_annotation_user_tag`
//! join table. Wall-clock timestamps are `BIGINT`
//! milliseconds-since-epoch.

use uuid::Uuid;

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
    dto::{millis_to_timestamp, timestamp_to_millis, uuid7_to_uuid, uuid_to_uuid7},
    SqlxError,
  },
};

// ---------------------------------------------------------------------------
// SpeakerRow
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSpeakerRow {
  pub id: Uuid,
  pub parent: Uuid,
  pub cluster_id: i32,
  pub name: String,
  pub speech_duration_ms: Option<i64>,
}

impl From<&Speaker<Uuid7>> for PgSpeakerRow {
  fn from(s: &Speaker<Uuid7>) -> Self {
    Self {
      id: uuid7_to_uuid(*s.id_ref()),
      parent: uuid7_to_uuid(*s.parent_ref()),
      cluster_id: s.cluster_id() as i32,
      name: s.name().to_owned(),
      speech_duration_ms: s.speech_duration_ref().and_then(|_| None::<i64>),
    }
  }
}

impl TryFrom<PgSpeakerRow> for Speaker<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgSpeakerRow) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let parent = uuid_to_uuid7(r.parent)?;
    let cluster_id = r.cluster_id as u32;
    Speaker::try_new(id, parent, cluster_id, r.name)
      .map_err(|e: SpeakerError| SqlxError::DomainConstructorRejected(e.to_string()))
  }
}

// ---------------------------------------------------------------------------
// UserTagRow
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgUserTagRow {
  pub id: Uuid,
  pub name: String,
  pub color_rgba: Option<i64>,
  pub created_at_ms: i64,
}

impl From<&UserTag<Uuid7>> for PgUserTagRow {
  fn from(t: &UserTag<Uuid7>) -> Self {
    Self {
      id: uuid7_to_uuid(*t.id_ref()),
      name: t.name().to_owned(),
      color_rgba: t.color().map(|c| i64::from(c.bits())),
      created_at_ms: timestamp_to_millis(*t.created_at_ref()),
    }
  }
}

impl TryFrom<PgUserTagRow> for UserTag<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgUserTagRow) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
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

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSceneAnnotationRow {
  pub id: Uuid,
  pub scene: Uuid,
  pub favorite: bool,
  pub rating: Option<i16>,
  pub note: String,
  pub updated_at_ms: i64,
}

/// One `scene_annotation_user_tag` join row: a single (annotation, tag)
/// edge with the tag's `ordinal` position in `SceneAnnotation::user_tags`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSceneAnnotationUserTagRow {
  pub scene_annotation: Uuid,
  pub user_tag: Uuid,
  pub ordinal: i32,
}

impl From<&SceneAnnotation<Uuid7>>
  for (
    PgSceneAnnotationRow,
    std::vec::Vec<PgSceneAnnotationUserTagRow>,
  )
{
  fn from(a: &SceneAnnotation<Uuid7>) -> Self {
    let annotation = uuid7_to_uuid(*a.id_ref());
    let joins = a
      .user_tags_slice()
      .iter()
      .enumerate()
      .map(|(i, tag)| PgSceneAnnotationUserTagRow {
        scene_annotation: annotation,
        user_tag: uuid7_to_uuid(*tag),
        ordinal: i as i32,
      })
      .collect();
    let row = PgSceneAnnotationRow {
      id: annotation,
      scene: uuid7_to_uuid(*a.scene_ref()),
      favorite: a.is_favorite(),
      rating: a.rating().map(i16::from),
      note: a.note().to_owned(),
      updated_at_ms: timestamp_to_millis(*a.updated_at_ref()),
    };
    (row, joins)
  }
}

impl
  TryFrom<(
    PgSceneAnnotationRow,
    std::vec::Vec<PgSceneAnnotationUserTagRow>,
  )> for SceneAnnotation<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, mut joins): (
      PgSceneAnnotationRow,
      std::vec::Vec<PgSceneAnnotationUserTagRow>,
    ),
  ) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let scene = uuid_to_uuid7(r.scene)?;
    let updated_at = millis_to_timestamp(r.updated_at_ms)?;
    joins.sort_by_key(|j| j.ordinal);
    let mut tags = std::vec::Vec::with_capacity(joins.len());
    for j in joins {
      tags.push(uuid_to_uuid7(j.user_tag)?);
    }
    let rating = match r.rating {
      None => None,
      Some(n) => Some(
        u8::try_from(n)
          .map_err(|e| SqlxError::UnknownDiscriminant(format!("SceneAnnotation.rating: {e}")))?,
      ),
    };
    Ok(
      SceneAnnotation::try_new(id, scene, updated_at)
        .map_err(|e: SceneAnnotationError| SqlxError::DomainConstructorRejected(e.to_string()))?
        .with_favorite(r.favorite)
        .with_user_tags(tags)
        .with_rating(rating)
        .with_note(r.note),
    )
  }
}

// ---------------------------------------------------------------------------
// WatchedLocationRow
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgWatchedLocationRow {
  pub id: Uuid,
  pub volume: Uuid,
  pub recursive: bool,
  pub enabled: bool,
  pub is_ejectable: bool,
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

impl From<&WatchedLocation<Uuid7>> for PgWatchedLocationRow {
  fn from(w: &WatchedLocation<Uuid7>) -> Self {
    let last_error = w.last_error_ref();
    Self {
      id: uuid7_to_uuid(*w.id_ref()),
      volume: uuid7_to_uuid(*w.volume_ref()),
      recursive: w.is_recursive(),
      enabled: w.is_enabled(),
      is_ejectable: w.is_ejectable(),
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

impl TryFrom<PgWatchedLocationRow> for WatchedLocation<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgWatchedLocationRow) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let volume = uuid_to_uuid7(r.volume)?;
    let added_at = millis_to_timestamp(r.added_at_ms)?;
    let mut w = WatchedLocation::try_new(id, volume, added_at)
      .map_err(|e: WatchedLocationError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_recursive(r.recursive)
      .with_enabled(r.enabled)
      .with_ejectable(r.is_ejectable);
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
    let s = Speaker::try_new(Uuid7::new(), Uuid7::new(), 1, "x").unwrap();
    let row: PgSpeakerRow = (&s).into();
    let s2: Speaker<Uuid7> = row.try_into().unwrap();
    assert_eq!(s.id_ref(), s2.id_ref());
    assert_eq!(s.parent_ref(), s2.parent_ref());
  }

  #[test]
  fn user_tag_roundtrip() {
    let t = UserTag::try_new(Uuid7::new(), "n", ts()).unwrap();
    let row: PgUserTagRow = (&t).into();
    let t2: UserTag<Uuid7> = row.try_into().unwrap();
    assert_eq!(t.id_ref(), t2.id_ref());
  }

  #[test]
  fn scene_annotation_roundtrip() {
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let a = SceneAnnotation::try_new(Uuid7::new(), Uuid7::new(), ts())
      .unwrap()
      .with_favorite(true)
      .with_user_tags(std::vec![t1, t2]);
    let tuple: (
      PgSceneAnnotationRow,
      std::vec::Vec<PgSceneAnnotationUserTagRow>,
    ) = (&a).into();
    assert_eq!(tuple.1.len(), 2);
    let a2: SceneAnnotation<Uuid7> = tuple.try_into().unwrap();
    assert_eq!(a.id_ref(), a2.id_ref());
    assert!(a2.is_favorite());
    assert_eq!(a2.user_tags_slice(), &[t1, t2]);
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
      PgSceneAnnotationRow,
      std::vec::Vec<PgSceneAnnotationUserTagRow>,
    ) = (&a).into();
    // Shuffle the join rows — TryFrom must sort by ordinal.
    joins.reverse();
    let a2: SceneAnnotation<Uuid7> = (row, joins).try_into().unwrap();
    assert_eq!(a2.user_tags_slice(), &[t1, t2, t3]);
  }

  #[test]
  fn watched_location_roundtrip() {
    let w = WatchedLocation::try_new(Uuid7::new(), Uuid7::new(), ts())
      .unwrap()
      .with_last_error(Some(ErrorInfo::new(ErrorCode::PathNotFound, "")));
    let row: PgWatchedLocationRow = (&w).into();
    let w2: WatchedLocation<Uuid7> = row.try_into().unwrap();
    assert_eq!(w.id_ref(), w2.id_ref());
    assert_eq!(
      w2.last_error_ref().map(|e| e.code()),
      Some(ErrorCode::PathNotFound)
    );
  }

  #[test]
  fn speaker_row_with_nil_uuid_rejected() {
    let row = PgSpeakerRow {
      id: uuid::Uuid::nil(),
      parent: uuid::Uuid::nil(),
      cluster_id: 0,
      name: String::new(),
      speech_duration_ms: None,
    };
    assert!(Speaker::<Uuid7>::try_from(row)
      .unwrap_err()
      .is_invalid_uuid());
  }
}
