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
    vo::{Provenance, VoiceFingerprint},
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

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PgSpeakerRow {
  pub id: Uuid,
  pub audio_track_id: Uuid,
  pub cluster_id: i32,
  pub name: String,
  pub speech_duration_ms: Option<i64>,
  /// Per-track aggregated voiceprint — discriminator for the flattened
  /// `VoiceFingerprint` VO (`Some` = present, every other `voiceprint_*`
  /// column carries a value; `None` = absent, all NULL).
  pub voiceprint_vector_id: Option<Uuid>,
  pub voiceprint_dimensions: Option<i32>,
  pub voiceprint_extracted_at_ms: Option<i64>,
  pub voiceprint_confidence: Option<f32>,
  pub voiceprint_provenance_model_name: Option<String>,
  pub voiceprint_provenance_model_version: Option<String>,
  pub voiceprint_provenance_prompt_version: Option<String>,
  pub voiceprint_provenance_indexer_version: Option<String>,
  /// Cross-track identity FK → `person.id`; NULL = not yet identified.
  pub person_id: Option<Uuid>,
}

impl From<&Speaker<Uuid7>> for PgSpeakerRow {
  fn from(s: &Speaker<Uuid7>) -> Self {
    let vfp = s.voiceprint_ref();
    let prov = vfp.map(|v| v.provenance_ref());
    Self {
      id: uuid7_to_uuid(*s.id_ref()),
      audio_track_id: uuid7_to_uuid(*s.audio_track_id_ref()),
      cluster_id: s.cluster_id() as i32,
      name: s.name().to_owned(),
      speech_duration_ms: s.speech_duration_ref().and_then(|_| None::<i64>),
      voiceprint_vector_id: vfp.map(|v| uuid7_to_uuid(*v.vector_id_ref())),
      voiceprint_dimensions: vfp.map(|v| v.dimensions() as i32),
      voiceprint_extracted_at_ms: vfp.map(|v| timestamp_to_millis(v.extracted_at())),
      voiceprint_confidence: vfp.and_then(|v| v.confidence()),
      voiceprint_provenance_model_name: prov.map(|p| p.model_name().to_owned()),
      voiceprint_provenance_model_version: prov.map(|p| p.model_version().to_owned()),
      voiceprint_provenance_prompt_version: prov.map(|p| p.prompt_version().to_owned()),
      voiceprint_provenance_indexer_version: prov.map(|p| p.indexer_version().to_owned()),
      person_id: s.person_id_ref().map(|p| uuid7_to_uuid(*p)),
    }
  }
}

impl TryFrom<PgSpeakerRow> for Speaker<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgSpeakerRow) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let audio_track_id = uuid_to_uuid7(r.audio_track_id)?;
    let cluster_id = r.cluster_id as u32;
    let mut s = Speaker::try_new(id, audio_track_id, cluster_id, r.name)
      .map_err(|e: SpeakerError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    if let Some(vid) = r.voiceprint_vector_id {
      let vector_id = uuid_to_uuid7(vid)?;
      let dimensions = u32::try_from(r.voiceprint_dimensions.unwrap_or(0)).map_err(|e| {
        SqlxError::UnknownDiscriminant(format!("Speaker.voiceprint_dimensions: {e}"))
      })?;
      let extracted_at = millis_to_timestamp(r.voiceprint_extracted_at_ms.unwrap_or(0))?;
      let provenance = Provenance::from_parts(
        r.voiceprint_provenance_model_name.unwrap_or_default(),
        r.voiceprint_provenance_model_version.unwrap_or_default(),
        r.voiceprint_provenance_prompt_version.unwrap_or_default(),
        r.voiceprint_provenance_indexer_version.unwrap_or_default(),
      );
      s = s.with_voiceprint(VoiceFingerprint::from_parts(
        vector_id,
        dimensions,
        extracted_at,
        r.voiceprint_confidence,
        provenance,
      ));
    }
    if let Some(pid) = r.person_id {
      s = s.with_person_id(uuid_to_uuid7(pid)?);
    }
    Ok(s)
  }
}

/// Borrowed view of [`PgSpeakerRow`] — zero-copy decode from `&'r Row`.
///
/// Variable-length text/byte columns borrow from the underlying row;
/// promotion to the domain [`Speaker`] only allocates strings IF the
/// caller asks for it via `TryFrom`. See [`PgSpeakerRow::as_ref`] for
/// the cheap-borrow path from an already-owned row.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PgSpeakerRowRef<'r> {
  pub id: Uuid,
  pub audio_track_id: Uuid,
  pub cluster_id: i32,
  pub name: &'r str,
  pub speech_duration_ms: Option<i64>,
  pub voiceprint_vector_id: Option<Uuid>,
  pub voiceprint_dimensions: Option<i32>,
  pub voiceprint_extracted_at_ms: Option<i64>,
  pub voiceprint_confidence: Option<f32>,
  pub voiceprint_provenance_model_name: Option<&'r str>,
  pub voiceprint_provenance_model_version: Option<&'r str>,
  pub voiceprint_provenance_prompt_version: Option<&'r str>,
  pub voiceprint_provenance_indexer_version: Option<&'r str>,
  pub person_id: Option<Uuid>,
}

impl PgSpeakerRow {
  /// Cheap borrow — produces a [`PgSpeakerRowRef`] that references `self`.
  pub fn as_ref(&self) -> PgSpeakerRowRef<'_> {
    PgSpeakerRowRef {
      id: self.id,
      audio_track_id: self.audio_track_id,
      cluster_id: self.cluster_id,
      name: &self.name,
      speech_duration_ms: self.speech_duration_ms,
      voiceprint_vector_id: self.voiceprint_vector_id,
      voiceprint_dimensions: self.voiceprint_dimensions,
      voiceprint_extracted_at_ms: self.voiceprint_extracted_at_ms,
      voiceprint_confidence: self.voiceprint_confidence,
      voiceprint_provenance_model_name: self.voiceprint_provenance_model_name.as_deref(),
      voiceprint_provenance_model_version: self.voiceprint_provenance_model_version.as_deref(),
      voiceprint_provenance_prompt_version: self.voiceprint_provenance_prompt_version.as_deref(),
      voiceprint_provenance_indexer_version: self.voiceprint_provenance_indexer_version.as_deref(),
      person_id: self.person_id,
    }
  }
}

impl<'r> TryFrom<PgSpeakerRowRef<'r>> for Speaker<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgSpeakerRowRef<'r>) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let audio_track_id = uuid_to_uuid7(r.audio_track_id)?;
    let cluster_id = r.cluster_id as u32;
    let mut s = Speaker::try_new(id, audio_track_id, cluster_id, r.name)
      .map_err(|e: SpeakerError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    if let Some(vid) = r.voiceprint_vector_id {
      let vector_id = uuid_to_uuid7(vid)?;
      let dimensions = u32::try_from(r.voiceprint_dimensions.unwrap_or(0)).map_err(|e| {
        SqlxError::UnknownDiscriminant(format!("Speaker.voiceprint_dimensions: {e}"))
      })?;
      let extracted_at = millis_to_timestamp(r.voiceprint_extracted_at_ms.unwrap_or(0))?;
      let provenance = Provenance::from_parts(
        r.voiceprint_provenance_model_name.unwrap_or_default(),
        r.voiceprint_provenance_model_version.unwrap_or_default(),
        r.voiceprint_provenance_prompt_version.unwrap_or_default(),
        r.voiceprint_provenance_indexer_version.unwrap_or_default(),
      );
      s = s.with_voiceprint(VoiceFingerprint::from_parts(
        vector_id,
        dimensions,
        extracted_at,
        r.voiceprint_confidence,
        provenance,
      ));
    }
    if let Some(pid) = r.person_id {
      s = s.with_person_id(uuid_to_uuid7(pid)?);
    }
    Ok(s)
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

/// Borrowed view of [`PgUserTagRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgUserTagRowRef<'r> {
  pub id: Uuid,
  pub name: &'r str,
  pub color_rgba: Option<i64>,
  pub created_at_ms: i64,
}

impl PgUserTagRow {
  /// Cheap borrow — produces a [`PgUserTagRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgUserTagRowRef<'_> {
    PgUserTagRowRef {
      id: self.id,
      name: &self.name,
      color_rgba: self.color_rgba,
      created_at_ms: self.created_at_ms,
    }
  }
}

impl<'r> TryFrom<PgUserTagRowRef<'r>> for UserTag<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgUserTagRowRef<'r>) -> Result<Self, Self::Error> {
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
  pub scene_id: Uuid,
  pub favorite: bool,
  pub rating: Option<i16>,
  pub note: String,
  pub updated_at_ms: i64,
}

/// One `scene_annotation_user_tag` join row: a single (annotation, tag)
/// edge with the tag's `ordinal` position in `SceneAnnotation::user_tags`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSceneAnnotationUserTagRow {
  pub scene_annotation_id: Uuid,
  pub user_tag_id: Uuid,
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
        scene_annotation_id: annotation,
        user_tag_id: uuid7_to_uuid(*tag),
        ordinal: i as i32,
      })
      .collect();
    let row = PgSceneAnnotationRow {
      id: annotation,
      scene_id: uuid7_to_uuid(*a.scene_id_ref()),
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
    let scene_id = uuid_to_uuid7(r.scene_id)?;
    let updated_at = millis_to_timestamp(r.updated_at_ms)?;
    joins.sort_by_key(|j| j.ordinal);
    let mut tags = std::vec::Vec::with_capacity(joins.len());
    for j in joins {
      tags.push(uuid_to_uuid7(j.user_tag_id)?);
    }
    let rating = match r.rating {
      None => None,
      Some(n) => Some(
        u8::try_from(n)
          .map_err(|e| SqlxError::UnknownDiscriminant(format!("SceneAnnotation.rating: {e}")))?,
      ),
    };
    Ok(
      SceneAnnotation::try_new(id, scene_id, updated_at)
        .map_err(|e: SceneAnnotationError| SqlxError::DomainConstructorRejected(e.to_string()))?
        .with_favorite(r.favorite)
        .with_user_tags(tags)
        .with_rating(rating)
        .with_note(r.note),
    )
  }
}

/// Borrowed view of [`PgSceneAnnotationRow`].
///
/// Note: `PgSceneAnnotationUserTagRow` is all-`Copy` (two `Uuid`s + `i32`)
/// so no `Ref` sibling is needed for the join row — the tuple form keeps
/// the owned join row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgSceneAnnotationRowRef<'r> {
  pub id: Uuid,
  pub scene_id: Uuid,
  pub favorite: bool,
  pub rating: Option<i16>,
  pub note: &'r str,
  pub updated_at_ms: i64,
}

impl PgSceneAnnotationRow {
  /// Cheap borrow — produces a [`PgSceneAnnotationRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgSceneAnnotationRowRef<'_> {
    PgSceneAnnotationRowRef {
      id: self.id,
      scene_id: self.scene_id,
      favorite: self.favorite,
      rating: self.rating,
      note: &self.note,
      updated_at_ms: self.updated_at_ms,
    }
  }
}

impl<'r>
  TryFrom<(
    PgSceneAnnotationRowRef<'r>,
    std::vec::Vec<PgSceneAnnotationUserTagRow>,
  )> for SceneAnnotation<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, mut joins): (
      PgSceneAnnotationRowRef<'r>,
      std::vec::Vec<PgSceneAnnotationUserTagRow>,
    ),
  ) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let scene = uuid_to_uuid7(r.scene_id)?;
    let updated_at = millis_to_timestamp(r.updated_at_ms)?;
    joins.sort_by_key(|j| j.ordinal);
    let mut tags = std::vec::Vec::with_capacity(joins.len());
    for j in joins {
      tags.push(uuid_to_uuid7(j.user_tag_id)?);
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

/// Borrowed view of [`PgWatchedLocationRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PgWatchedLocationRowRef<'r> {
  pub id: Uuid,
  pub volume: Uuid,
  pub recursive: bool,
  pub enabled: bool,
  pub is_ejectable: bool,
  pub added_at_ms: i64,
  pub last_reconciled_at_ms: Option<i64>,
  pub last_reconcile_status: Option<i16>,
  pub last_error_code: Option<i32>,
  pub last_error_message: Option<&'r str>,
}

impl PgWatchedLocationRow {
  /// Cheap borrow — produces a [`PgWatchedLocationRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgWatchedLocationRowRef<'_> {
    PgWatchedLocationRowRef {
      id: self.id,
      volume: self.volume,
      recursive: self.recursive,
      enabled: self.enabled,
      is_ejectable: self.is_ejectable,
      added_at_ms: self.added_at_ms,
      last_reconciled_at_ms: self.last_reconciled_at_ms,
      last_reconcile_status: self.last_reconcile_status,
      last_error_code: self.last_error_code,
      last_error_message: self.last_error_message.as_deref(),
    }
  }
}

impl<'r> TryFrom<PgWatchedLocationRowRef<'r>> for WatchedLocation<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgWatchedLocationRowRef<'r>) -> Result<Self, Self::Error> {
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
    assert_eq!(s.audio_track_id_ref(), s2.audio_track_id_ref());
    assert!(s2.voiceprint_ref().is_none());
    assert!(s2.person_id_ref().is_none());
  }

  #[test]
  fn speaker_roundtrip_with_voiceprint_and_person() {
    let voiceprint = VoiceFingerprint::try_new(
      Uuid7::new(),
      192,
      ts(),
      Some(0.83),
      Provenance::from_parts("ecapa-tdnn", "v1.0.0", "", "findit-indexer-0.1.0"),
    )
    .unwrap();
    let person = Uuid7::new();
    let s = Speaker::try_new(Uuid7::new(), Uuid7::new(), 1, "Jane")
      .unwrap()
      .with_voiceprint(voiceprint.clone())
      .with_person_id(person);
    let row: PgSpeakerRow = (&s).into();
    assert!(row.voiceprint_vector_id.is_some());
    assert_eq!(row.person_id, Some(uuid7_to_uuid(person)));
    let s2: Speaker<Uuid7> = row.try_into().unwrap();
    assert_eq!(s2.voiceprint_ref(), Some(&voiceprint));
    assert_eq!(s2.person_id_ref(), Some(&person));
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
  fn speaker_ref_roundtrip() {
    let s = Speaker::try_new(Uuid7::new(), Uuid7::new(), 2, "Bob").unwrap();
    let row: PgSpeakerRow = (&s).into();
    let s2: Speaker<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn user_tag_ref_roundtrip() {
    let t = UserTag::try_new(Uuid7::new(), "n", ts())
      .unwrap()
      .with_color(Some(Rgba::from_bits(0xdeadbeef)));
    let row: PgUserTagRow = (&t).into();
    let t2: UserTag<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(t, t2);
  }

  #[test]
  fn scene_annotation_ref_roundtrip() {
    let t1 = Uuid7::new();
    let a = SceneAnnotation::try_new(Uuid7::new(), Uuid7::new(), ts())
      .unwrap()
      .with_favorite(true)
      .with_user_tags(std::vec![t1])
      .with_note("hi");
    let (row, joins): (
      PgSceneAnnotationRow,
      std::vec::Vec<PgSceneAnnotationUserTagRow>,
    ) = (&a).into();
    let a2: SceneAnnotation<Uuid7> = (row.as_ref(), joins).try_into().unwrap();
    assert_eq!(a, a2);
  }

  #[test]
  fn watched_location_ref_roundtrip() {
    let w = WatchedLocation::try_new(Uuid7::new(), Uuid7::new(), ts())
      .unwrap()
      .with_last_error(Some(ErrorInfo::new(ErrorCode::PathNotFound, "gone")));
    let row: PgWatchedLocationRow = (&w).into();
    let w2: WatchedLocation<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(w, w2);
  }

  #[test]
  fn speaker_row_with_nil_uuid_rejected() {
    let row = PgSpeakerRow {
      id: uuid::Uuid::nil(),
      audio_track_id: uuid::Uuid::nil(),
      cluster_id: 0,
      name: String::new(),
      speech_duration_ms: None,
      voiceprint_vector_id: None,
      voiceprint_dimensions: None,
      voiceprint_extracted_at_ms: None,
      voiceprint_confidence: None,
      voiceprint_provenance_model_name: None,
      voiceprint_provenance_model_version: None,
      voiceprint_provenance_prompt_version: None,
      voiceprint_provenance_indexer_version: None,
      person_id: None,
    };
    assert!(Speaker::<Uuid7>::try_from(row)
      .unwrap_err()
      .is_invalid_uuid());
  }
}
