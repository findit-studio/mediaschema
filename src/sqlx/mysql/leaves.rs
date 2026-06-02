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
    vo::{Provenance, VoiceFingerprint},
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

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct MySqlSpeakerRow {
  pub id: std::vec::Vec<u8>,
  pub audio_track_id: std::vec::Vec<u8>,
  pub cluster_id: u32,
  pub name: String,
  pub speech_duration_ms: Option<i64>,
  /// Per-track aggregated voiceprint — discriminator for the flattened
  /// `VoiceFingerprint` VO (`Some` = present; `None` = all NULL).
  pub voiceprint_vector_id: Option<std::vec::Vec<u8>>,
  pub voiceprint_dimensions: Option<u32>,
  pub voiceprint_extracted_at_ms: Option<i64>,
  pub voiceprint_confidence: Option<f32>,
  pub voiceprint_provenance_model_name: Option<String>,
  pub voiceprint_provenance_model_version: Option<String>,
  pub voiceprint_provenance_prompt_version: Option<String>,
  pub voiceprint_provenance_indexer_version: Option<String>,
  /// Cross-track identity FK → `person.id`; NULL = not yet identified.
  pub person_id: Option<std::vec::Vec<u8>>,
}

impl From<&Speaker<Uuid7>> for MySqlSpeakerRow {
  fn from(s: &Speaker<Uuid7>) -> Self {
    let vfp = s.voiceprint_ref();
    let prov = vfp.map(|v| v.provenance_ref());
    Self {
      id: s.id_ref().as_bytes().to_vec(),
      audio_track_id: s.audio_track_id_ref().as_bytes().to_vec(),
      cluster_id: s.cluster_id(),
      name: s.name().to_owned(),
      speech_duration_ms: None,
      voiceprint_vector_id: vfp.map(|v| v.vector_id_ref().as_bytes().to_vec()),
      voiceprint_dimensions: vfp.map(|v| v.dimensions()),
      voiceprint_extracted_at_ms: vfp.map(|v| timestamp_to_millis(v.extracted_at())),
      voiceprint_confidence: vfp.and_then(|v| v.confidence()),
      voiceprint_provenance_model_name: prov.map(|p| p.model_name().to_owned()),
      voiceprint_provenance_model_version: prov.map(|p| p.model_version().to_owned()),
      voiceprint_provenance_prompt_version: prov.map(|p| p.prompt_version().to_owned()),
      voiceprint_provenance_indexer_version: prov.map(|p| p.indexer_version().to_owned()),
      person_id: s.person_id_ref().map(|p| p.as_bytes().to_vec()),
    }
  }
}

impl TryFrom<MySqlSpeakerRow> for Speaker<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: MySqlSpeakerRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let audio_track_id = bytes_to_uuid7(&r.audio_track_id)?;
    let mut s = Speaker::try_new(id, audio_track_id, r.cluster_id, r.name)
      .map_err(|e: SpeakerError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    if let Some(vid) = r.voiceprint_vector_id {
      let vector_id = bytes_to_uuid7(&vid)?;
      let dimensions = r.voiceprint_dimensions.unwrap_or(0);
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
      s = s.with_person_id(bytes_to_uuid7(&pid)?);
    }
    Ok(s)
  }
}

/// Borrowed view of [`MySqlSpeakerRow`] — zero-copy decode from `&'r Row`.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct MySqlSpeakerRowRef<'r> {
  pub id: &'r [u8],
  pub audio_track_id: &'r [u8],
  pub cluster_id: u32,
  pub name: &'r str,
  pub speech_duration_ms: Option<i64>,
  pub voiceprint_vector_id: Option<&'r [u8]>,
  pub voiceprint_dimensions: Option<u32>,
  pub voiceprint_extracted_at_ms: Option<i64>,
  pub voiceprint_confidence: Option<f32>,
  pub voiceprint_provenance_model_name: Option<&'r str>,
  pub voiceprint_provenance_model_version: Option<&'r str>,
  pub voiceprint_provenance_prompt_version: Option<&'r str>,
  pub voiceprint_provenance_indexer_version: Option<&'r str>,
  pub person_id: Option<&'r [u8]>,
}

impl MySqlSpeakerRow {
  /// Cheap borrow — produces a [`MySqlSpeakerRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlSpeakerRowRef<'_> {
    MySqlSpeakerRowRef {
      id: &self.id,
      audio_track_id: &self.audio_track_id,
      cluster_id: self.cluster_id,
      name: &self.name,
      speech_duration_ms: self.speech_duration_ms,
      voiceprint_vector_id: self.voiceprint_vector_id.as_deref(),
      voiceprint_dimensions: self.voiceprint_dimensions,
      voiceprint_extracted_at_ms: self.voiceprint_extracted_at_ms,
      voiceprint_confidence: self.voiceprint_confidence,
      voiceprint_provenance_model_name: self.voiceprint_provenance_model_name.as_deref(),
      voiceprint_provenance_model_version: self.voiceprint_provenance_model_version.as_deref(),
      voiceprint_provenance_prompt_version: self.voiceprint_provenance_prompt_version.as_deref(),
      voiceprint_provenance_indexer_version: self.voiceprint_provenance_indexer_version.as_deref(),
      person_id: self.person_id.as_deref(),
    }
  }
}

impl<'r> TryFrom<MySqlSpeakerRowRef<'r>> for Speaker<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: MySqlSpeakerRowRef<'r>) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
    let audio_track_id = bytes_to_uuid7(r.audio_track_id)?;
    let mut s = Speaker::try_new(id, audio_track_id, r.cluster_id, r.name)
      .map_err(|e: SpeakerError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    if let Some(vid) = r.voiceprint_vector_id {
      let vector_id = bytes_to_uuid7(vid)?;
      let dimensions = r.voiceprint_dimensions.unwrap_or(0);
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
      s = s.with_person_id(bytes_to_uuid7(pid)?);
    }
    Ok(s)
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

/// Borrowed view of [`MySqlUserTagRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlUserTagRowRef<'r> {
  pub id: &'r [u8],
  pub name: &'r str,
  pub color_rgba: Option<u32>,
  pub created_at_ms: i64,
}

impl MySqlUserTagRow {
  /// Cheap borrow — produces a [`MySqlUserTagRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlUserTagRowRef<'_> {
    MySqlUserTagRowRef {
      id: &self.id,
      name: &self.name,
      color_rgba: self.color_rgba,
      created_at_ms: self.created_at_ms,
    }
  }
}

impl<'r> TryFrom<MySqlUserTagRowRef<'r>> for UserTag<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: MySqlUserTagRowRef<'r>) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
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
  pub scene_id: std::vec::Vec<u8>,
  pub favorite: i8,
  pub rating: Option<u8>,
  pub note: String,
  pub updated_at_ms: i64,
}

/// One `scene_annotation_user_tag` join row: a single (annotation, tag)
/// edge with the tag's `ordinal` position in `SceneAnnotation::user_tags`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSceneAnnotationUserTagRow {
  pub scene_annotation_id: std::vec::Vec<u8>,
  pub user_tag_id: std::vec::Vec<u8>,
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
        scene_annotation_id: annotation.clone(),
        user_tag_id: tag.as_bytes().to_vec(),
        ordinal: i as i32,
      })
      .collect();
    let row = MySqlSceneAnnotationRow {
      id: annotation,
      scene_id: a.scene_id_ref().as_bytes().to_vec(),
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
    let scene_id = bytes_to_uuid7(&r.scene_id)?;
    let updated_at = millis_to_timestamp(r.updated_at_ms)?;
    joins.sort_by_key(|j| j.ordinal);
    let mut tags = std::vec::Vec::with_capacity(joins.len());
    for j in joins {
      tags.push(bytes_to_uuid7(&j.user_tag_id)?);
    }
    let ann = SceneAnnotation::try_new(id, scene_id, updated_at)
      .map_err(|e: SceneAnnotationError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_favorite(r.favorite != 0)
      .with_user_tags(tags)
      .with_rating(r.rating)
      .with_note(r.note);
    Ok(ann)
  }
}

/// Borrowed view of [`MySqlSceneAnnotationRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSceneAnnotationRowRef<'r> {
  pub id: &'r [u8],
  pub scene_id: &'r [u8],
  pub favorite: i8,
  pub rating: Option<u8>,
  pub note: &'r str,
  pub updated_at_ms: i64,
}

/// Borrowed view of [`MySqlSceneAnnotationUserTagRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlSceneAnnotationUserTagRowRef<'r> {
  pub scene_annotation_id: &'r [u8],
  pub user_tag_id: &'r [u8],
  pub ordinal: i32,
}

impl MySqlSceneAnnotationRow {
  /// Cheap borrow — produces a [`MySqlSceneAnnotationRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlSceneAnnotationRowRef<'_> {
    MySqlSceneAnnotationRowRef {
      id: &self.id,
      scene_id: &self.scene_id,
      favorite: self.favorite,
      rating: self.rating,
      note: &self.note,
      updated_at_ms: self.updated_at_ms,
    }
  }
}

impl MySqlSceneAnnotationUserTagRow {
  /// Cheap borrow — produces a [`MySqlSceneAnnotationUserTagRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlSceneAnnotationUserTagRowRef<'_> {
    MySqlSceneAnnotationUserTagRowRef {
      scene_annotation_id: &self.scene_annotation_id,
      user_tag_id: &self.user_tag_id,
      ordinal: self.ordinal,
    }
  }
}

impl<'r>
  TryFrom<(
    MySqlSceneAnnotationRowRef<'r>,
    std::vec::Vec<MySqlSceneAnnotationUserTagRowRef<'r>>,
  )> for SceneAnnotation<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, mut joins): (
      MySqlSceneAnnotationRowRef<'r>,
      std::vec::Vec<MySqlSceneAnnotationUserTagRowRef<'r>>,
    ),
  ) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
    let scene = bytes_to_uuid7(r.scene_id)?;
    let updated_at = millis_to_timestamp(r.updated_at_ms)?;
    joins.sort_by_key(|j| j.ordinal);
    let mut tags = std::vec::Vec::with_capacity(joins.len());
    for j in joins {
      tags.push(bytes_to_uuid7(j.user_tag_id)?);
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

/// Borrowed view of [`MySqlWatchedLocationRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MySqlWatchedLocationRowRef<'r> {
  pub id: &'r [u8],
  pub volume: &'r [u8],
  pub recursive: i8,
  pub enabled: i8,
  pub is_ejectable: i8,
  pub added_at_ms: i64,
  pub last_reconciled_at_ms: Option<i64>,
  pub last_reconcile_status: Option<i16>,
  pub last_error_code: Option<i32>,
  pub last_error_message: Option<&'r str>,
}

impl MySqlWatchedLocationRow {
  /// Cheap borrow — produces a [`MySqlWatchedLocationRowRef`] referencing `self`.
  pub fn as_ref(&self) -> MySqlWatchedLocationRowRef<'_> {
    MySqlWatchedLocationRowRef {
      id: &self.id,
      volume: &self.volume,
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

impl<'r> TryFrom<MySqlWatchedLocationRowRef<'r>> for WatchedLocation<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: MySqlWatchedLocationRowRef<'r>) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
    let volume = bytes_to_uuid7(r.volume)?;
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
    let audio_track_id = Uuid7::new();
    let s = Speaker::try_new(Uuid7::new(), audio_track_id, 3, "Jane").unwrap();
    let row: MySqlSpeakerRow = (&s).into();
    let s2: Speaker<Uuid7> = row.try_into().unwrap();
    assert_eq!(s2.id_ref(), s.id_ref());
    assert_eq!(s2.audio_track_id_ref(), s.audio_track_id_ref());
    assert_eq!(s2.cluster_id(), s.cluster_id());
    assert_eq!(s2.name(), s.name());
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
    let row: MySqlSpeakerRow = (&s).into();
    assert!(row.voiceprint_vector_id.is_some());
    assert_eq!(row.person_id, Some(person.as_bytes().to_vec()));
    let s2: Speaker<Uuid7> = row.try_into().unwrap();
    assert_eq!(s2.voiceprint_ref(), Some(&voiceprint));
    assert_eq!(s2.person_id_ref(), Some(&person));
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
  fn speaker_ref_roundtrip() {
    let s = Speaker::try_new(Uuid7::new(), Uuid7::new(), 2, "Bob").unwrap();
    let row: MySqlSpeakerRow = (&s).into();
    let s2: Speaker<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn user_tag_ref_roundtrip() {
    let t = UserTag::try_new(Uuid7::new(), "v", ts())
      .unwrap()
      .with_color(Some(Rgba::from_bits(0xdeadbeef)));
    let row: MySqlUserTagRow = (&t).into();
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
      .with_rating(Some(5))
      .with_note("ok");
    let (row, joins): (
      MySqlSceneAnnotationRow,
      std::vec::Vec<MySqlSceneAnnotationUserTagRow>,
    ) = (&a).into();
    let join_refs: std::vec::Vec<MySqlSceneAnnotationUserTagRowRef<'_>> = joins
      .iter()
      .map(MySqlSceneAnnotationUserTagRow::as_ref)
      .collect();
    let a2: SceneAnnotation<Uuid7> = (row.as_ref(), join_refs).try_into().unwrap();
    assert_eq!(a, a2);
  }

  #[test]
  fn watched_location_ref_roundtrip() {
    let w = WatchedLocation::try_new(Uuid7::new(), Uuid7::new(), ts())
      .unwrap()
      .with_enabled(true)
      .with_last_error(Some(ErrorInfo::new(ErrorCode::PathNotFound, "gone")));
    let row: MySqlWatchedLocationRow = (&w).into();
    let w2: WatchedLocation<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(w, w2);
  }

  #[test]
  fn speaker_row_with_nil_uuid_rejected() {
    let row = MySqlSpeakerRow {
      id: std::vec::Vec::from([0u8; 16]),
      audio_track_id: Uuid7::new().as_bytes().to_vec(),
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
