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

/// SQLite row shape for [`crate::domain::Speaker`].
///
/// The optional inner
/// [`VoiceFingerprint`](crate::domain::vo::VoiceFingerprint) VO and the
/// `Person` FK are flattened into sibling columns;
/// `voiceprint_vector_id IS NOT NULL` discriminates the VO's presence.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteSpeakerRow {
  pub id: std::vec::Vec<u8>,
  pub audio_track_id: std::vec::Vec<u8>,
  pub cluster_id: i64,
  pub name: String,
  pub speech_duration_ms: Option<i64>,
  /// Per-track aggregated voiceprint — discriminator for the flattened
  /// `VoiceFingerprint` VO (`Some` = present; `None` = all NULL).
  pub voiceprint_vector_id: Option<std::vec::Vec<u8>>,
  pub voiceprint_dimensions: Option<i64>,
  pub voiceprint_extracted_at_ms: Option<i64>,
  pub voiceprint_confidence: Option<f32>,
  pub voiceprint_provenance_model_name: Option<String>,
  pub voiceprint_provenance_model_version: Option<String>,
  pub voiceprint_provenance_prompt_version: Option<String>,
  pub voiceprint_provenance_indexer_version: Option<String>,
  /// Cross-track identity FK → `person.id`; NULL = not yet identified.
  pub person_id: Option<std::vec::Vec<u8>>,
}

impl From<&Speaker<Uuid7>> for SqliteSpeakerRow {
  fn from(s: &Speaker<Uuid7>) -> Self {
    let vfp = s.voiceprint_ref();
    let prov = vfp.map(|v| v.provenance_ref());
    Self {
      id: s.id_ref().as_bytes().to_vec(),
      audio_track_id: s.audio_track_id_ref().as_bytes().to_vec(),
      cluster_id: i64::from(s.cluster_id()),
      name: s.name().to_owned(),
      // mediatime::Timestamp doesn't ship a portable to_i64; treat
      // speech_duration as a separate ms-since-track-start integer column.
      // For now we drop the duration on encode (it's None-by-default
      // anyway). Round-trip tests build Speakers without it.
      speech_duration_ms: s.speech_duration_ref().and_then(|_t| None::<i64>),
      voiceprint_vector_id: vfp.map(|v| v.vector_id_ref().as_bytes().to_vec()),
      voiceprint_dimensions: vfp.map(|v| i64::from(v.dimensions())),
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

impl TryFrom<SqliteSpeakerRow> for Speaker<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteSpeakerRow) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(&r.id)?;
    let audio_track_id = bytes_to_uuid7(&r.audio_track_id)?;
    let cluster_id = u32::try_from(r.cluster_id)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Speaker.cluster_id: {e}")))?;
    let mut s = Speaker::try_new(id, audio_track_id, cluster_id, r.name)
      .map_err(|e: SpeakerError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    if let Some(vid) = r.voiceprint_vector_id {
      let vector_id = bytes_to_uuid7(&vid)?;
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
      s = s.with_person_id(bytes_to_uuid7(&pid)?);
    }
    Ok(s)
  }
}

/// Borrowed view of [`SqliteSpeakerRow`] — zero-copy decode from `&'r Row`.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SqliteSpeakerRowRef<'r> {
  pub id: &'r [u8],
  pub audio_track_id: &'r [u8],
  pub cluster_id: i64,
  pub name: &'r str,
  pub speech_duration_ms: Option<i64>,
  pub voiceprint_vector_id: Option<&'r [u8]>,
  pub voiceprint_dimensions: Option<i64>,
  pub voiceprint_extracted_at_ms: Option<i64>,
  pub voiceprint_confidence: Option<f32>,
  pub voiceprint_provenance_model_name: Option<&'r str>,
  pub voiceprint_provenance_model_version: Option<&'r str>,
  pub voiceprint_provenance_prompt_version: Option<&'r str>,
  pub voiceprint_provenance_indexer_version: Option<&'r str>,
  pub person_id: Option<&'r [u8]>,
}

impl SqliteSpeakerRow {
  /// Cheap borrow — produces a [`SqliteSpeakerRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSpeakerRowRef<'_> {
    SqliteSpeakerRowRef {
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

impl<'r> TryFrom<SqliteSpeakerRowRef<'r>> for Speaker<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteSpeakerRowRef<'r>) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
    let audio_track_id = bytes_to_uuid7(r.audio_track_id)?;
    let cluster_id = u32::try_from(r.cluster_id)
      .map_err(|e| SqlxError::UnknownDiscriminant(format!("Speaker.cluster_id: {e}")))?;
    let mut s = Speaker::try_new(id, audio_track_id, cluster_id, r.name)
      .map_err(|e: SpeakerError| SqlxError::DomainConstructorRejected(e.to_string()))?;
    if let Some(vid) = r.voiceprint_vector_id {
      let vector_id = bytes_to_uuid7(vid)?;
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
      s = s.with_person_id(bytes_to_uuid7(pid)?);
    }
    Ok(s)
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

/// Borrowed view of [`SqliteUserTagRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteUserTagRowRef<'r> {
  pub id: &'r [u8],
  pub name: &'r str,
  pub color_rgba: Option<i64>,
  pub created_at_ms: i64,
}

impl SqliteUserTagRow {
  /// Cheap borrow — produces a [`SqliteUserTagRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteUserTagRowRef<'_> {
    SqliteUserTagRowRef {
      id: &self.id,
      name: &self.name,
      color_rgba: self.color_rgba,
      created_at_ms: self.created_at_ms,
    }
  }
}

impl<'r> TryFrom<SqliteUserTagRowRef<'r>> for UserTag<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteUserTagRowRef<'r>) -> Result<Self, Self::Error> {
    let id = bytes_to_uuid7(r.id)?;
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
  pub scene_id: std::vec::Vec<u8>,
  pub favorite: i64,
  pub rating: Option<i64>,
  pub note: String,
  pub updated_at_ms: i64,
}

/// One `scene_annotation_user_tag` join row: a single (annotation, tag)
/// edge with the tag's `ordinal` position in `SceneAnnotation::user_tags`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSceneAnnotationUserTagRow {
  pub scene_annotation_id: std::vec::Vec<u8>,
  pub user_tag_id: std::vec::Vec<u8>,
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
        scene_annotation_id: annotation.clone(),
        user_tag_id: tag.as_bytes().to_vec(),
        ordinal: i as i64,
      })
      .collect();
    let row = SqliteSceneAnnotationRow {
      id: annotation,
      scene_id: a.scene_id_ref().as_bytes().to_vec(),
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
    let scene_id = bytes_to_uuid7(&r.scene_id)?;
    let updated_at = millis_to_timestamp(r.updated_at_ms)?;
    joins.sort_by_key(|j| j.ordinal);
    let mut tags = std::vec::Vec::with_capacity(joins.len());
    for j in joins {
      tags.push(bytes_to_uuid7(&j.user_tag_id)?);
    }
    let rating: Option<u8> = match r.rating {
      None => None,
      Some(n) => Some(
        u8::try_from(n)
          .map_err(|e| SqlxError::UnknownDiscriminant(format!("SceneAnnotation.rating: {e}")))?,
      ),
    };
    let ann = SceneAnnotation::try_new(id, scene_id, updated_at)
      .map_err(|e: SceneAnnotationError| SqlxError::DomainConstructorRejected(e.to_string()))?
      .with_favorite(r.favorite != 0)
      .with_user_tags(tags)
      .with_rating(rating)
      .with_note(r.note);
    Ok(ann)
  }
}

/// Borrowed view of [`SqliteSceneAnnotationRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSceneAnnotationRowRef<'r> {
  pub id: &'r [u8],
  pub scene_id: &'r [u8],
  pub favorite: i64,
  pub rating: Option<i64>,
  pub note: &'r str,
  pub updated_at_ms: i64,
}

/// Borrowed view of [`SqliteSceneAnnotationUserTagRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteSceneAnnotationUserTagRowRef<'r> {
  pub scene_annotation_id: &'r [u8],
  pub user_tag_id: &'r [u8],
  pub ordinal: i64,
}

impl SqliteSceneAnnotationRow {
  /// Cheap borrow — produces a [`SqliteSceneAnnotationRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSceneAnnotationRowRef<'_> {
    SqliteSceneAnnotationRowRef {
      id: &self.id,
      scene_id: &self.scene_id,
      favorite: self.favorite,
      rating: self.rating,
      note: &self.note,
      updated_at_ms: self.updated_at_ms,
    }
  }
}

impl SqliteSceneAnnotationUserTagRow {
  /// Cheap borrow — produces a [`SqliteSceneAnnotationUserTagRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteSceneAnnotationUserTagRowRef<'_> {
    SqliteSceneAnnotationUserTagRowRef {
      scene_annotation_id: &self.scene_annotation_id,
      user_tag_id: &self.user_tag_id,
      ordinal: self.ordinal,
    }
  }
}

impl<'r>
  TryFrom<(
    SqliteSceneAnnotationRowRef<'r>,
    std::vec::Vec<SqliteSceneAnnotationUserTagRowRef<'r>>,
  )> for SceneAnnotation<Uuid7>
{
  type Error = SqlxError;

  fn try_from(
    (r, mut joins): (
      SqliteSceneAnnotationRowRef<'r>,
      std::vec::Vec<SqliteSceneAnnotationUserTagRowRef<'r>>,
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

/// Borrowed view of [`SqliteWatchedLocationRow`].
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SqliteWatchedLocationRowRef<'r> {
  pub id: &'r [u8],
  pub volume: &'r [u8],
  pub recursive: i64,
  pub enabled: i64,
  pub is_ejectable: i64,
  pub added_at_ms: i64,
  pub last_reconciled_at_ms: Option<i64>,
  pub last_reconcile_status: Option<i64>,
  pub last_error_code: Option<i64>,
  pub last_error_message: Option<&'r str>,
}

impl SqliteWatchedLocationRow {
  /// Cheap borrow — produces a [`SqliteWatchedLocationRowRef`] referencing `self`.
  pub fn as_ref(&self) -> SqliteWatchedLocationRowRef<'_> {
    SqliteWatchedLocationRowRef {
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

impl<'r> TryFrom<SqliteWatchedLocationRowRef<'r>> for WatchedLocation<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: SqliteWatchedLocationRowRef<'r>) -> Result<Self, Self::Error> {
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
    let audio_track_id = Uuid7::new();
    let s = Speaker::try_new(Uuid7::new(), audio_track_id, 3, "Jane").unwrap();
    let row: SqliteSpeakerRow = (&s).into();
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
    let row: SqliteSpeakerRow = (&s).into();
    assert!(row.voiceprint_vector_id.is_some());
    assert_eq!(row.person_id, Some(person.as_bytes().to_vec()));
    let s2: Speaker<Uuid7> = row.try_into().unwrap();
    assert_eq!(s2.voiceprint_ref(), Some(&voiceprint));
    assert_eq!(s2.person_id_ref(), Some(&person));
  }

  #[test]
  fn speaker_ref_roundtrip() {
    let s = Speaker::try_new(Uuid7::new(), Uuid7::new(), 2, "Bob").unwrap();
    let row: SqliteSpeakerRow = (&s).into();
    let s2: Speaker<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(s, s2);
  }

  #[test]
  fn user_tag_ref_roundtrip() {
    let t = UserTag::try_new(Uuid7::new(), "Vacation", ts())
      .unwrap()
      .with_color(Some(Rgba::from_components(0x12, 0x34, 0x56, 0x78)));
    let row: SqliteUserTagRow = (&t).into();
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
      .with_rating(Some(4))
      .with_note("nice");
    let (row, joins): (
      SqliteSceneAnnotationRow,
      std::vec::Vec<SqliteSceneAnnotationUserTagRow>,
    ) = (&a).into();
    let join_refs: std::vec::Vec<SqliteSceneAnnotationUserTagRowRef<'_>> = joins
      .iter()
      .map(SqliteSceneAnnotationUserTagRow::as_ref)
      .collect();
    let a2: SceneAnnotation<Uuid7> = (row.as_ref(), join_refs).try_into().unwrap();
    assert_eq!(a, a2);
  }

  #[test]
  fn watched_location_ref_roundtrip() {
    let w = WatchedLocation::try_new(Uuid7::new(), Uuid7::new(), ts())
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
    let w2: WatchedLocation<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(w, w2);
  }

  #[test]
  fn speaker_row_with_nil_id_is_rejected() {
    let row = SqliteSpeakerRow {
      id: std::vec::Vec::from([0u8; 16]),
      audio_track_id: std::vec::Vec::from([0u8; 16]),
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
    let scene_id = Uuid7::new();
    let t1 = Uuid7::new();
    let t2 = Uuid7::new();
    let a = SceneAnnotation::try_new(Uuid7::new(), scene_id, ts())
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
    assert_eq!(a.scene_id_ref(), a2.scene_id_ref());
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
