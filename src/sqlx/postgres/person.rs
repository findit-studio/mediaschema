//! PostgreSQL row shape for the `Person` aggregate.
//!
//! `id` rides as a native `uuid`. The optional inner
//! [`VoiceFingerprint`] VO is flattened
//! into nine sibling columns; `voiceprint_vector_id IS NOT NULL` is the
//! presence discriminator (when present, every other `voiceprint_*` column
//! carries a value — except the inner `Option<f32>` `confidence`, which is
//! independently nullable). `confidence` itself rides as a `SMALLINT`
//! discriminator (`0 = AutoMatched, 1 = UserConfirmed`). Wall-clock
//! timestamps are `BIGINT` ms-since-epoch.
//!
//! Reconstruction uses [`Person::from_parts`] (infallible — the data was
//! already validated by `try_new` when first written) and
//! [`VoiceFingerprint::from_parts`] for the embedded VO.

use uuid::Uuid;

use crate::{
  domain::{
    vo::{Provenance, VoiceFingerprint},
    Person, PersonConfidence, Uuid7,
  },
  sqlx::{
    dto::{millis_to_timestamp, timestamp_to_millis, uuid7_to_uuid, uuid_to_uuid7},
    SqlxError,
  },
};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PgPersonRow {
  pub id: Uuid,
  pub name: String,
  /// `0 = AutoMatched`, `1 = UserConfirmed`.
  pub confidence: i16,
  /// Discriminates presence of the flattened `VoiceFingerprint` VO.
  pub voiceprint_vector_id: Option<Uuid>,
  pub voiceprint_dimensions: Option<i32>,
  pub voiceprint_extracted_at_ms: Option<i64>,
  /// Inner `Option<f32>` of the present `VoiceFingerprint` — independently
  /// nullable even when the outer VO is present (the embedding model may
  /// not expose a confidence score).
  pub voiceprint_confidence: Option<f32>,
  pub voiceprint_provenance_model_name: Option<String>,
  pub voiceprint_provenance_model_version: Option<String>,
  pub voiceprint_provenance_prompt_version: Option<String>,
  pub voiceprint_provenance_indexer_version: Option<String>,
  pub created_at_ms: i64,
  pub updated_at_ms: i64,
}

fn person_confidence_to_i16(c: PersonConfidence) -> i16 {
  match c {
    PersonConfidence::AutoMatched => 0,
    PersonConfidence::UserConfirmed => 1,
  }
}

fn person_confidence_from_i16(n: i16) -> Result<PersonConfidence, SqlxError> {
  match n {
    0 => Ok(PersonConfidence::AutoMatched),
    1 => Ok(PersonConfidence::UserConfirmed),
    other => Err(SqlxError::UnknownDiscriminant(format!(
      "Person.confidence: {other}"
    ))),
  }
}

impl From<&Person<Uuid7>> for PgPersonRow {
  fn from(p: &Person<Uuid7>) -> Self {
    let vfp = p.voiceprint_ref();
    let prov = vfp.map(|v| v.provenance_ref());
    Self {
      id: uuid7_to_uuid(*p.id_ref()),
      name: p.name().to_owned(),
      confidence: person_confidence_to_i16(p.confidence()),
      voiceprint_vector_id: vfp.map(|v| uuid7_to_uuid(*v.vector_id_ref())),
      voiceprint_dimensions: vfp.map(|v| v.dimensions() as i32),
      voiceprint_extracted_at_ms: vfp.map(|v| timestamp_to_millis(v.extracted_at())),
      voiceprint_confidence: vfp.and_then(|v| v.confidence()),
      voiceprint_provenance_model_name: prov.map(|p| p.model_name().to_owned()),
      voiceprint_provenance_model_version: prov.map(|p| p.model_version().to_owned()),
      voiceprint_provenance_prompt_version: prov.map(|p| p.prompt_version().to_owned()),
      voiceprint_provenance_indexer_version: prov.map(|p| p.indexer_version().to_owned()),
      created_at_ms: timestamp_to_millis(p.created_at()),
      updated_at_ms: timestamp_to_millis(p.updated_at()),
    }
  }
}

impl TryFrom<PgPersonRow> for Person<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgPersonRow) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let confidence = person_confidence_from_i16(r.confidence)?;
    let created_at = millis_to_timestamp(r.created_at_ms)?;
    let updated_at = millis_to_timestamp(r.updated_at_ms)?;
    let voiceprint = match r.voiceprint_vector_id {
      None => None,
      Some(vid) => {
        let vector_id = uuid_to_uuid7(vid)?;
        let dimensions = u32::try_from(r.voiceprint_dimensions.unwrap_or(0)).map_err(|e| {
          SqlxError::UnknownDiscriminant(format!("Person.voiceprint_dimensions: {e}"))
        })?;
        let extracted_at = millis_to_timestamp(r.voiceprint_extracted_at_ms.unwrap_or(0))?;
        let provenance = Provenance::from_parts(
          r.voiceprint_provenance_model_name.unwrap_or_default(),
          r.voiceprint_provenance_model_version.unwrap_or_default(),
          r.voiceprint_provenance_prompt_version.unwrap_or_default(),
          r.voiceprint_provenance_indexer_version.unwrap_or_default(),
        );
        Some(VoiceFingerprint::from_parts(
          vector_id,
          dimensions,
          extracted_at,
          r.voiceprint_confidence,
          provenance,
        ))
      }
    };
    Ok(Person::from_parts(
      id,
      r.name.into(),
      voiceprint,
      confidence,
      created_at,
      updated_at,
    ))
  }
}

/// Borrowed view of [`PgPersonRow`] — zero-copy decode from `&'r Row`.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PgPersonRowRef<'r> {
  pub id: Uuid,
  pub name: &'r str,
  pub confidence: i16,
  pub voiceprint_vector_id: Option<Uuid>,
  pub voiceprint_dimensions: Option<i32>,
  pub voiceprint_extracted_at_ms: Option<i64>,
  pub voiceprint_confidence: Option<f32>,
  pub voiceprint_provenance_model_name: Option<&'r str>,
  pub voiceprint_provenance_model_version: Option<&'r str>,
  pub voiceprint_provenance_prompt_version: Option<&'r str>,
  pub voiceprint_provenance_indexer_version: Option<&'r str>,
  pub created_at_ms: i64,
  pub updated_at_ms: i64,
}

impl PgPersonRow {
  /// Cheap borrow — produces a [`PgPersonRowRef`] referencing `self`.
  pub fn as_ref(&self) -> PgPersonRowRef<'_> {
    PgPersonRowRef {
      id: self.id,
      name: &self.name,
      confidence: self.confidence,
      voiceprint_vector_id: self.voiceprint_vector_id,
      voiceprint_dimensions: self.voiceprint_dimensions,
      voiceprint_extracted_at_ms: self.voiceprint_extracted_at_ms,
      voiceprint_confidence: self.voiceprint_confidence,
      voiceprint_provenance_model_name: self.voiceprint_provenance_model_name.as_deref(),
      voiceprint_provenance_model_version: self.voiceprint_provenance_model_version.as_deref(),
      voiceprint_provenance_prompt_version: self.voiceprint_provenance_prompt_version.as_deref(),
      voiceprint_provenance_indexer_version: self.voiceprint_provenance_indexer_version.as_deref(),
      created_at_ms: self.created_at_ms,
      updated_at_ms: self.updated_at_ms,
    }
  }
}

impl<'r> TryFrom<PgPersonRowRef<'r>> for Person<Uuid7> {
  type Error = SqlxError;

  fn try_from(r: PgPersonRowRef<'r>) -> Result<Self, Self::Error> {
    let id = uuid_to_uuid7(r.id)?;
    let confidence = person_confidence_from_i16(r.confidence)?;
    let created_at = millis_to_timestamp(r.created_at_ms)?;
    let updated_at = millis_to_timestamp(r.updated_at_ms)?;
    let voiceprint = match r.voiceprint_vector_id {
      None => None,
      Some(vid) => {
        let vector_id = uuid_to_uuid7(vid)?;
        let dimensions = u32::try_from(r.voiceprint_dimensions.unwrap_or(0)).map_err(|e| {
          SqlxError::UnknownDiscriminant(format!("Person.voiceprint_dimensions: {e}"))
        })?;
        let extracted_at = millis_to_timestamp(r.voiceprint_extracted_at_ms.unwrap_or(0))?;
        let provenance = Provenance::from_parts(
          r.voiceprint_provenance_model_name.unwrap_or_default(),
          r.voiceprint_provenance_model_version.unwrap_or_default(),
          r.voiceprint_provenance_prompt_version.unwrap_or_default(),
          r.voiceprint_provenance_indexer_version.unwrap_or_default(),
        );
        Some(VoiceFingerprint::from_parts(
          vector_id,
          dimensions,
          extracted_at,
          r.voiceprint_confidence,
          provenance,
        ))
      }
    };
    Ok(Person::from_parts(
      id,
      r.name.into(),
      voiceprint,
      confidence,
      created_at,
      updated_at,
    ))
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

  fn vfp() -> VoiceFingerprint<Uuid7> {
    VoiceFingerprint::try_new(
      Uuid7::new(),
      192,
      ts(),
      Some(0.83),
      Provenance::from_parts("ecapa-tdnn", "v1.0.0", "", "findit-indexer-0.1.0"),
    )
    .expect("valid voiceprint")
  }

  #[test]
  fn person_roundtrip_with_voiceprint() {
    let p = Person::try_new(Uuid7::new(), "Jane Doe", ts(), ts())
      .unwrap()
      .with_voiceprint(vfp())
      .with_confidence(PersonConfidence::UserConfirmed);
    let row: PgPersonRow = (&p).into();
    assert!(row.voiceprint_vector_id.is_some());
    let p2: Person<Uuid7> = row.try_into().unwrap();
    assert_eq!(p, p2);
  }

  #[test]
  fn person_roundtrip_without_voiceprint() {
    let p = Person::try_new(Uuid7::new(), "", ts(), ts()).unwrap();
    let row: PgPersonRow = (&p).into();
    assert!(row.voiceprint_vector_id.is_none());
    assert!(row.voiceprint_provenance_model_name.is_none());
    let p2: Person<Uuid7> = row.try_into().unwrap();
    assert_eq!(p, p2);
    assert_eq!(p2.confidence(), PersonConfidence::AutoMatched);
  }

  #[test]
  fn person_confidence_discriminator_round_trips() {
    let auto = Person::try_new(Uuid7::new(), "", ts(), ts()).unwrap();
    let user = auto
      .clone()
      .with_confidence(PersonConfidence::UserConfirmed);
    let row_a: PgPersonRow = (&auto).into();
    let row_u: PgPersonRow = (&user).into();
    assert_eq!(row_a.confidence, 0);
    assert_eq!(row_u.confidence, 1);
    let a2: Person<Uuid7> = row_a.try_into().unwrap();
    let u2: Person<Uuid7> = row_u.try_into().unwrap();
    assert_eq!(a2.confidence(), PersonConfidence::AutoMatched);
    assert_eq!(u2.confidence(), PersonConfidence::UserConfirmed);
  }

  #[test]
  fn person_ref_roundtrip() {
    let p = Person::try_new(Uuid7::new(), "Jane Doe", ts(), ts())
      .unwrap()
      .with_voiceprint(vfp())
      .with_confidence(PersonConfidence::UserConfirmed);
    let row: PgPersonRow = (&p).into();
    let p2: Person<Uuid7> = row.as_ref().try_into().unwrap();
    assert_eq!(p, p2);
  }

  #[test]
  fn person_unknown_confidence_discriminant_rejected() {
    let row = PgPersonRow {
      id: uuid7_to_uuid(Uuid7::new()),
      name: String::new(),
      confidence: 7,
      voiceprint_vector_id: None,
      voiceprint_dimensions: None,
      voiceprint_extracted_at_ms: None,
      voiceprint_confidence: None,
      voiceprint_provenance_model_name: None,
      voiceprint_provenance_model_version: None,
      voiceprint_provenance_prompt_version: None,
      voiceprint_provenance_indexer_version: None,
      created_at_ms: timestamp_to_millis(ts()),
      updated_at_ms: timestamp_to_millis(ts()),
    };
    let err = Person::<Uuid7>::try_from(row).unwrap_err();
    assert!(err.is_unknown_discriminant());
  }
}
