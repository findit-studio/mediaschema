//! `Person` — cross-track / cross-modality identity anchor.
//!
//! One `Person` ↔ many [`Speaker`](crate::domain::Speaker)s (one per track
//! they appear in). Designed **modality-neutral**: a future
//! `FaceDetection.person: Option<Id>` link should hang off this aggregate
//! without re-shaping it. The per-track diarized voice stays on `Speaker`;
//! `Person` is the canonical "this is the same human across tracks /
//! modalities" layer that sits above it.
//!
//! The canonical aggregated voiceprint (the centroid across all linked
//! `Speaker`s' per-track voiceprints) lives on
//! [`Person::voiceprint_ref`]; it is meaningful only when the contributing
//! samples share one `(model, version)` pair — see
//! [`crate::domain::VoiceFingerprint`]'s `provenance`.
//!
//! Source doc: `schema/person.md` (added in a later stacked PR).

use derive_more::{Display, IsVariant};
use jiff::Timestamp as JiffTimestamp;
use smol_str::SmolStr;

use crate::domain::{vo::VoiceFingerprint, Uuid7};

/// Cross-track / cross-modality identity anchor. One `Person` ↔ many
/// [`Speaker`](crate::domain::Speaker)s.
///
/// **No `Default`** — defaulting to `{ id: nil, … }` would be an
/// identity-less anchor. Construct via [`Person::try_new`] (validating:
/// rejects nil `id`).
///
/// `Eq` / `Hash` are intentionally **not** derived — the
/// [`VoiceFingerprint`] inside `voiceprint` carries an `Option<f32>`
/// confidence, which precludes total equality and hashing. Aggregates are
/// keyed by `id` anyway, so callers should hash / equate `Person` rows by
/// their id, not by the whole value.
#[derive(Debug, Clone, PartialEq)]
pub struct Person<Id = Uuid7> {
  id: Id,
  /// Human name. `""` = unnamed (the schema convention: empty string is
  /// the canonical absent value for string fields).
  name: SmolStr,
  /// Aggregated canonical voiceprint — the centroid across all linked
  /// `Speaker`s' per-track voiceprints. `None` until aggregation runs.
  /// Only meaningful when contributing samples share one `(model,
  /// version)`; see [`VoiceFingerprint`]'s `provenance`.
  voiceprint: Option<VoiceFingerprint<Id>>,
  /// Whether identification was user-confirmed or auto-matched.
  confidence: PersonConfidence,
  created_at: JiffTimestamp,
  updated_at: JiffTimestamp,
}

impl Person<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (an identity anchor with no id is not a real
  /// `Person`). All other fields start in their neutral state:
  /// `name = ""`, `voiceprint = None`, `confidence =
  /// PersonConfidence::AutoMatched`.
  pub fn try_new(
    id: Uuid7,
    name: impl Into<SmolStr>,
    created_at: JiffTimestamp,
    updated_at: JiffTimestamp,
  ) -> Result<Self, PersonError> {
    if id.is_nil() {
      return Err(PersonError::NilId);
    }
    Ok(Self {
      id,
      name: name.into(),
      voiceprint: None,
      confidence: PersonConfidence::default(),
      created_at,
      updated_at,
    })
  }
}

impl<Id> Person<Id> {
  /// Raw constructor for **storage / wire reconstruction** — assembles a
  /// `Person` directly from its persisted fields, bypassing the nil-id
  /// check that [`Person::try_new`] performs. Intended ONLY for backends
  /// rebuilding a `Person` from a trusted persisted row/document (the data
  /// was validated by `try_new` when first written). Application code
  /// building a fresh `Person` must use [`Person::try_new`].
  ///
  /// Not `const` — `Option<VoiceFingerprint<Id>>` is not const-constructible
  /// across a generic `Id`.
  #[inline(always)]
  #[must_use]
  pub fn from_parts(
    id: Id,
    name: SmolStr,
    voiceprint: Option<VoiceFingerprint<Id>>,
    confidence: PersonConfidence,
    created_at: JiffTimestamp,
    updated_at: JiffTimestamp,
  ) -> Self {
    Self {
      id,
      name,
      voiceprint,
      confidence,
      created_at,
      updated_at,
    }
  }

  /// Canonical identity (the `Person`'s key).
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// Human name (`""` = unnamed).
  #[inline(always)]
  pub fn name(&self) -> &str {
    self.name.as_str()
  }

  /// Aggregated canonical voiceprint (`None` until aggregation runs).
  #[inline(always)]
  pub const fn voiceprint_ref(&self) -> Option<&VoiceFingerprint<Id>> {
    self.voiceprint.as_ref()
  }

  /// User-confirmed vs auto-matched identification.
  #[inline(always)]
  pub const fn confidence(&self) -> PersonConfidence {
    self.confidence
  }

  /// Record creation timestamp. `jiff::Timestamp` is `Copy`, so by-value.
  #[inline(always)]
  pub const fn created_at(&self) -> JiffTimestamp {
    self.created_at
  }

  /// Record last-updated timestamp. `jiff::Timestamp` is `Copy`, so
  /// by-value.
  #[inline(always)]
  pub const fn updated_at(&self) -> JiffTimestamp {
    self.updated_at
  }

  // ----- Builders ---------------------------------------------------------

  /// Builder: replace `name`.
  #[inline(always)]
  #[must_use]
  pub fn with_name(mut self, name: impl Into<SmolStr>) -> Self {
    self.name = name.into();
    self
  }

  /// Builder: put `voiceprint` into the *present* state. Mirrors the
  /// `set_/with_` "present value" arm of the `Option<T>` mutator
  /// vocabulary (golden-rules §3).
  #[inline(always)]
  #[must_use]
  pub fn with_voiceprint(mut self, voiceprint: VoiceFingerprint<Id>) -> Self {
    self.voiceprint = Some(voiceprint);
    self
  }

  /// Builder: assign the *raw* `voiceprint` wrapper (the `maybe_*` arm of
  /// the `Option<T>` mutator vocabulary).
  #[inline(always)]
  #[must_use]
  pub fn maybe_voiceprint(mut self, voiceprint: Option<VoiceFingerprint<Id>>) -> Self {
    self.voiceprint = voiceprint;
    self
  }

  /// Builder: replace `confidence`.
  #[inline(always)]
  #[must_use]
  pub const fn with_confidence(mut self, confidence: PersonConfidence) -> Self {
    self.confidence = confidence;
    self
  }

  /// Builder: replace `created_at`.
  #[inline(always)]
  #[must_use]
  pub const fn with_created_at(mut self, t: JiffTimestamp) -> Self {
    self.created_at = t;
    self
  }

  /// Builder: replace `updated_at`.
  #[inline(always)]
  #[must_use]
  pub const fn with_updated_at(mut self, t: JiffTimestamp) -> Self {
    self.updated_at = t;
    self
  }

  // ----- In-place setters -------------------------------------------------

  /// In-place mutator for `name`.
  #[inline(always)]
  pub fn set_name(&mut self, name: impl Into<SmolStr>) -> &mut Self {
    self.name = name.into();
    self
  }

  /// In-place mutator: put `voiceprint` into the *present* state.
  #[inline(always)]
  pub fn set_voiceprint(&mut self, voiceprint: VoiceFingerprint<Id>) -> &mut Self {
    self.voiceprint = Some(voiceprint);
    self
  }

  /// In-place mutator: assign the *raw* `voiceprint` wrapper (the
  /// `update_*` arm of the `Option<T>` mutator vocabulary).
  #[inline(always)]
  pub fn update_voiceprint(&mut self, voiceprint: Option<VoiceFingerprint<Id>>) -> &mut Self {
    self.voiceprint = voiceprint;
    self
  }

  /// In-place mutator: clear `voiceprint` (the `clear_*` arm).
  #[inline(always)]
  pub fn clear_voiceprint(&mut self) -> &mut Self {
    self.voiceprint = None;
    self
  }

  /// In-place mutator for `confidence`.
  #[inline(always)]
  pub const fn set_confidence(&mut self, confidence: PersonConfidence) -> &mut Self {
    self.confidence = confidence;
    self
  }

  /// In-place mutator for `created_at`.
  #[inline(always)]
  pub const fn set_created_at(&mut self, t: JiffTimestamp) -> &mut Self {
    self.created_at = t;
    self
  }

  /// In-place mutator for `updated_at`.
  #[inline(always)]
  pub const fn set_updated_at(&mut self, t: JiffTimestamp) -> &mut Self {
    self.updated_at = t;
    self
  }
}

/// Whether a [`Person`]'s identification is user-confirmed or
/// auto-matched.
///
/// Defaults to [`PersonConfidence::AutoMatched`] — a freshly clustered
/// identity hasn't been reviewed yet.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, IsVariant, Display)]
#[display("{}", self.as_str())]
pub enum PersonConfidence {
  /// Auto-clustered by similarity, not yet reviewed.
  #[default]
  AutoMatched,
  /// User confirmed (or manually created / edited).
  UserConfirmed,
}

impl PersonConfidence {
  /// Stable snake_case slug — the canonical string form of every variant.
  #[inline(always)]
  pub const fn as_str(&self) -> &'static str {
    match self {
      Self::AutoMatched => "auto_matched",
      Self::UserConfirmed => "user_confirmed",
    }
  }
  /// Inverse of [`as_str`](Self::as_str). Returns `None` for any input
  /// that isn't an exact match of one of the slugs.
  #[inline]
  pub fn from_str(s: &str) -> Option<Self> {
    Some(match s {
      "auto_matched" => Self::AutoMatched,
      "user_confirmed" => Self::UserConfirmed,
      _ => return None,
    })
  }
}

/// Error returned by [`Person::try_new`] when an invariant cannot be
/// upheld. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum PersonError {
  /// Supplied `id` was the [`Uuid7`] nil sentinel — not a real identity.
  #[error("Person id must not be the nil UUID")]
  NilId,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;
  use crate::domain::vo::Provenance;

  #[test]
  fn person_confidence_slug_roundtrip() {
    for v in [
      PersonConfidence::AutoMatched,
      PersonConfidence::UserConfirmed,
    ] {
      assert_eq!(PersonConfidence::from_str(v.as_str()), Some(v), "{v:?}");
    }
    assert_eq!(PersonConfidence::from_str("not_a_slug"), None);
  }

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
  fn try_new_happy_path() {
    let id = Uuid7::new();
    let p = Person::try_new(id, "", ts(), ts()).expect("valid construction must succeed");
    assert_eq!(p.id_ref(), &id);
    assert!(p.name().is_empty());
    assert!(p.voiceprint_ref().is_none());
    assert_eq!(p.confidence(), PersonConfidence::AutoMatched);
    assert_eq!(p.created_at(), ts());
    assert_eq!(p.updated_at(), ts());
  }

  #[test]
  fn try_new_accepts_named_person() {
    let p = Person::try_new(Uuid7::new(), "Jane Doe", ts(), ts()).unwrap();
    assert_eq!(p.name(), "Jane Doe");
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = Person::try_new(Uuid7::nil(), "", ts(), ts());
    assert_eq!(r.err(), Some(PersonError::NilId));
    assert!(PersonError::NilId.is_nil_id());
  }

  #[test]
  fn person_confidence_default_is_auto_matched() {
    assert_eq!(PersonConfidence::default(), PersonConfidence::AutoMatched);
    assert!(PersonConfidence::AutoMatched.is_auto_matched());
    assert!(PersonConfidence::UserConfirmed.is_user_confirmed());
  }

  #[test]
  fn from_parts_round_trips_a_validated_instance() {
    let v = vfp();
    let original = Person::try_new(Uuid7::new(), "Jane", ts(), ts())
      .unwrap()
      .with_voiceprint(v.clone())
      .with_confidence(PersonConfidence::UserConfirmed);
    let rebuilt: Person<Uuid7> = Person::from_parts(
      *original.id_ref(),
      SmolStr::new(original.name()),
      original.voiceprint_ref().cloned(),
      original.confidence(),
      original.created_at(),
      original.updated_at(),
    );
    assert_eq!(rebuilt, original);
  }

  #[test]
  fn voiceprint_full_option_mutator_vocabulary() {
    let v = vfp();
    // with_/set_ = present
    let p = Person::try_new(Uuid7::new(), "", ts(), ts())
      .unwrap()
      .with_voiceprint(v.clone());
    assert!(p.voiceprint_ref().is_some());
    // maybe_ = raw wrapper (consuming)
    let p = p.maybe_voiceprint(None);
    assert!(p.voiceprint_ref().is_none());
    let p = p.maybe_voiceprint(Some(v.clone()));
    assert_eq!(p.voiceprint_ref(), Some(&v));
    // in-place setters
    let mut p = p;
    p.clear_voiceprint();
    assert!(p.voiceprint_ref().is_none());
    p.set_voiceprint(v.clone());
    assert_eq!(p.voiceprint_ref(), Some(&v));
    p.update_voiceprint(None);
    assert!(p.voiceprint_ref().is_none());
    p.update_voiceprint(Some(v.clone()));
    assert_eq!(p.voiceprint_ref(), Some(&v));
  }

  #[test]
  fn confidence_builder_and_setter() {
    let p = Person::try_new(Uuid7::new(), "", ts(), ts())
      .unwrap()
      .with_confidence(PersonConfidence::UserConfirmed);
    assert_eq!(p.confidence(), PersonConfidence::UserConfirmed);
    let mut p = p;
    p.set_confidence(PersonConfidence::AutoMatched);
    assert_eq!(p.confidence(), PersonConfidence::AutoMatched);
  }

  #[test]
  fn timestamp_builders_and_setters() {
    let later = JiffTimestamp::from_millisecond(1_800_000_000_000).expect("valid timestamp");
    let p = Person::try_new(Uuid7::new(), "", ts(), ts())
      .unwrap()
      .with_created_at(later)
      .with_updated_at(later);
    assert_eq!(p.created_at(), later);
    assert_eq!(p.updated_at(), later);
    let mut p = p;
    p.set_created_at(ts());
    p.set_updated_at(ts());
    assert_eq!(p.created_at(), ts());
    assert_eq!(p.updated_at(), ts());
  }

  #[test]
  fn name_builder_and_setter() {
    let p = Person::try_new(Uuid7::new(), "old", ts(), ts())
      .unwrap()
      .with_name("new");
    assert_eq!(p.name(), "new");
    let mut p = p;
    p.set_name("");
    assert!(p.name().is_empty());
  }
}
