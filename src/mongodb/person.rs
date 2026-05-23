//! `Person` ↔ bson `Document` mapping.
//!
//! `Person` is the cross-track / cross-modality identity anchor. One
//! `Person` ↔ many [`Speaker`](crate::domain::Speaker)s. See
//! `schema/person.md` for the locked spec.
//!
//! MongoDB is a document database, so the optional `voiceprint`
//! sub-VO is stored as a natural **embedded sub-document** (queryable
//! / indexable on `voiceprint.provenance.model_name` etc.) rather than
//! flattened — mirroring how `Media.gps` / `Media.device` /
//! `Audio.track_progress` are kept as embedded docs. Absence is encoded
//! as a `Null` field.

use ::bson::{Bson, Document};

use crate::domain::{
  aggregates::person::{Person, PersonConfidence},
  Uuid7,
};

use super::{error::MongoError, util::*};

// ---------------------------------------------------------------------------
// `PersonConfidence` ↔ Int32 (0/1)
// ---------------------------------------------------------------------------

fn person_confidence_to_i32(c: PersonConfidence) -> i32 {
  match c {
    PersonConfidence::AutoMatched => 0,
    PersonConfidence::UserConfirmed => 1,
  }
}

fn person_confidence_from_i64(v: i64, field: &'static str) -> Result<PersonConfidence, MongoError> {
  match v {
    0 => Ok(PersonConfidence::AutoMatched),
    1 => Ok(PersonConfidence::UserConfirmed),
    _ => Err(MongoError::IntOutOfRange {
      field: smol_str::SmolStr::from(field),
      value: v,
    }),
  }
}

// ---------------------------------------------------------------------------
// Person
// ---------------------------------------------------------------------------

impl From<&Person<Uuid7>> for Document {
  fn from(p: &Person<Uuid7>) -> Self {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(*p.id_ref()));
    d.insert("name", Bson::String(p.name().to_owned()));
    d.insert(
      "confidence",
      Bson::Int32(person_confidence_to_i32(p.confidence())),
    );
    d.insert(
      "voiceprint",
      p.voiceprint_ref()
        .map(voice_fingerprint_to_bson)
        .unwrap_or(Bson::Null),
    );
    d.insert("created_at", jiff_to_bson(p.created_at()));
    d.insert("updated_at", jiff_to_bson(p.updated_at()));
    d
  }
}

impl TryFrom<Document> for Person<Uuid7> {
  type Error = MongoError;

  fn try_from(mut d: Document) -> Result<Self, Self::Error> {
    let id = uuid7_from_bson(take(&mut d, "_id")?, "_id")?;
    let name = as_smol(take(&mut d, "name")?, "name")?;
    let created_at = jiff_from_bson(take(&mut d, "created_at")?, "created_at")?;
    let updated_at = jiff_from_bson(take(&mut d, "updated_at")?, "updated_at")?;
    let confidence = match take_opt(&mut d, "confidence") {
      Some(b) => person_confidence_from_i64(as_i64(b, "confidence")?, "confidence")?,
      None => PersonConfidence::default(),
    };
    let voiceprint = opt(take_opt(&mut d, "voiceprint"), |b| {
      voice_fingerprint_from_bson(b, "voiceprint")
    })?;
    // `from_parts` is the raw storage-reconstruction constructor: the
    // document was validated by `try_new` when first written, so the
    // nil-id check is not repeated here.
    Ok(Person::from_parts(
      id, name, voiceprint, confidence, created_at, updated_at,
    ))
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use crate::domain::vo::{Provenance, VoiceFingerprint};
  use jiff::Timestamp as JiffTimestamp;

  fn ts() -> JiffTimestamp {
    JiffTimestamp::from_millisecond(1_700_000_000_000).expect("valid timestamp")
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
  fn person_minimal_roundtrip() {
    let p = Person::try_new(Uuid7::new(), "", ts(), ts()).unwrap();
    let doc: Document = (&p).into();
    let p2: Person<Uuid7> = doc.try_into().unwrap();
    assert_eq!(p, p2);
    assert!(p2.voiceprint_ref().is_none());
    assert_eq!(p2.confidence(), PersonConfidence::AutoMatched);
  }

  #[test]
  fn person_full_roundtrip_with_voiceprint() {
    let p = Person::try_new(Uuid7::new(), "Jane Doe", ts(), ts())
      .unwrap()
      .with_voiceprint(vfp())
      .with_confidence(PersonConfidence::UserConfirmed);
    let doc: Document = (&p).into();
    let p2: Person<Uuid7> = doc.try_into().unwrap();
    assert_eq!(p, p2);
  }

  #[test]
  fn person_voiceprint_embedded_as_subdocument() {
    let p = Person::try_new(Uuid7::new(), "", ts(), ts())
      .unwrap()
      .with_voiceprint(vfp());
    let doc: Document = (&p).into();
    let v = doc.get("voiceprint").expect("voiceprint present");
    let sub = match v {
      Bson::Document(d) => d,
      other => panic!("voiceprint should be a sub-document, got {other:?}"),
    };
    assert!(sub.contains_key("vector_id"));
    assert!(sub.contains_key("dimensions"));
    assert!(sub.contains_key("extracted_at"));
    assert!(sub.contains_key("confidence"));
    let prov = sub.get_document("provenance").expect("provenance sub-doc");
    assert_eq!(prov.get_str("model_name").unwrap(), "ecapa-tdnn");
  }

  #[test]
  fn person_voiceprint_absence_round_trips_as_null() {
    let p = Person::try_new(Uuid7::new(), "", ts(), ts()).unwrap();
    let doc: Document = (&p).into();
    assert_eq!(doc.get("voiceprint"), Some(&Bson::Null));
    let p2: Person<Uuid7> = doc.try_into().unwrap();
    assert!(p2.voiceprint_ref().is_none());
  }

  #[test]
  fn person_missing_id_errors() {
    let mut d = Document::new();
    d.insert("name", Bson::String(String::new()));
    d.insert("created_at", jiff_to_bson(ts()));
    d.insert("updated_at", jiff_to_bson(ts()));
    let err = Person::<Uuid7>::try_from(d).unwrap_err();
    assert!(err.is_missing_field());
  }

  #[test]
  fn person_nil_id_rejected() {
    // Encode a fake nil-id document by hand (the From impl can't
    // produce one because `Person::try_new` would have rejected it).
    let mut d = Document::new();
    d.insert(
      "_id",
      Bson::Binary(::bson::Binary {
        subtype: ::bson::spec::BinarySubtype::Uuid,
        bytes: vec![0u8; 16],
      }),
    );
    d.insert("name", Bson::String(String::new()));
    d.insert("confidence", Bson::Int32(0));
    d.insert("created_at", jiff_to_bson(ts()));
    d.insert("updated_at", jiff_to_bson(ts()));
    let err = Person::<Uuid7>::try_from(d).unwrap_err();
    assert!(err.is_uuid_7());
  }

  #[test]
  fn person_unknown_confidence_int_rejected() {
    let mut d = Document::new();
    d.insert("_id", uuid7_to_bson(Uuid7::new()));
    d.insert("name", Bson::String(String::new()));
    d.insert("confidence", Bson::Int32(7));
    d.insert("created_at", jiff_to_bson(ts()));
    d.insert("updated_at", jiff_to_bson(ts()));
    let err = Person::<Uuid7>::try_from(d).unwrap_err();
    assert!(matches!(err, MongoError::IntOutOfRange { .. }));
  }
}
