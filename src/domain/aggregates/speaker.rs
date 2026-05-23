//! `Speaker` — a per-track diarized voice (locked `schema/speaker.md` r1).
//!
//! From `dia` diarization (per-track clustering). The 256-d voice embedding
//! is **not** a field — it lives in **LanceDB keyed by `Speaker.id`** (locked
//! embeddings rule). Cross-media identity (canonical `Person` layer over
//! voice-embedding similarity) is a future enhancement, not modelled here.
//!
//! Per-track scope: the same physical person in another track is a
//! **separate** `Speaker` row; linking them is a LanceDB voice-similarity
//! query, not a stored FK.

// NOTE: `schema/speaker.md` r1 names `speech_duration: Option<mediatime::Timestamp>`,
// but `mediatime` 0.1.6 publicly exports only `Timebase`/`Timestamp`/`TimeRange`
// — no `Timestamp`. The verify-against-source discipline caught the doc name
// imprecision. We use `mediatime::Timestamp` (as a track-relative offset/duration
// — same timebase as the track's `TimeRange.start`), which is the closest fit;
// the schema doc's `Timestamp` reference should be read as `mediatime::Timestamp`
// pending a doc-name fix.
use derive_more::IsVariant;
use mediatime::Timestamp;
use smol_str::SmolStr;

use crate::domain::{vo::VoiceFingerprint, Uuid7};

// ---------------------------------------------------------------------------
// Duration validation — shared by `speech_duration`'s validating mutators
// ---------------------------------------------------------------------------

/// `speech_duration` is semantically a non-negative duration. A
/// `mediatime::Timestamp` is negative iff its `pts()` is negative — the
/// timebase numerator/denominator are always positive (`u32` / `NonZeroU32`),
/// so the sign is carried entirely by the PTS value. `None` (absent) is not
/// negative.
#[inline]
const fn is_negative_duration(d: Option<Timestamp>) -> bool {
  match d {
    None => false,
    Some(ts) => ts.pts() < 0,
  }
}

/// One distinct voice in an `AudioTrack`. The track's speaker set is
/// `AudioTrack.speakers: Vec<Id> → Speaker`; each `AudioSegment.speaker:
/// Option<Id> → Speaker`.
///
/// **No `Default`**: a `Speaker` with nil `id` and nil `parent` would be
/// an orphan voice clustered as `SPEAKER_NN=0` — a real invalid state.
/// Construct explicitly via [`Speaker::try_new`].
///
/// Fields are private per the encapsulation rule; access via `id_ref()` /
/// `parent_ref()` / `cluster_id()` / `name()` / `speech_duration_ref()` /
/// `voiceprint_ref()` / `person_ref()` getters and `with_*` / `set_*`
/// builders/mutators.
///
/// `Eq` / `Hash` are intentionally **not** derived — the per-track
/// `voiceprint` carries an `Option<f32>` confidence (via
/// [`VoiceFingerprint`]), which precludes total equality and hashing.
/// Speakers are keyed by `id`; callers should hash / equate them by id
/// rather than by the whole value.
#[derive(Debug, Clone, PartialEq)]
pub struct Speaker<Id = Uuid7> {
  id: Id,
  parent: Id,
  cluster_id: u32,
  name: SmolStr,
  /// **Semantically a non-negative duration in the track's timebase**,
  /// even though the type is `mediatime::Timestamp` (`mediatime` 0.1.6
  /// has no dedicated `Duration`). A proper `TrackDuration` newtype is
  /// a tracked follow-up in `mediatime`.
  speech_duration: Option<Timestamp>,
  /// Per-track aggregated voiceprint — the centroid across this track's
  /// `AudioSegment.voice_fingerprint`s. `None` until aggregation runs.
  voiceprint: Option<VoiceFingerprint<Id>>,
  /// Cross-track identity FK → [`Person`](crate::domain::Person). `None`
  /// = not yet identified.
  person: Option<Id>,
}

impl Speaker<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (LanceDB voiceprint key collision) and nil
  /// `parent` (orphaned voice with no `AudioTrack`).
  pub fn try_new(
    id: Uuid7,
    parent: Uuid7,
    cluster_id: u32,
    name: impl Into<SmolStr>,
  ) -> Result<Self, SpeakerError> {
    if id.is_nil() {
      return Err(SpeakerError::NilId);
    }
    if parent.is_nil() {
      return Err(SpeakerError::NilParent);
    }
    Ok(Self {
      id,
      parent,
      cluster_id,
      name: name.into(),
      speech_duration: None,
      voiceprint: None,
      person: None,
    })
  }
}

impl<Id> Speaker<Id> {
  /// Canonical identity (also the LanceDB voiceprint key).
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  /// FK → `AudioTrack.id`.
  #[inline(always)]
  pub const fn parent_ref(&self) -> &Id {
    &self.parent
  }

  /// `dia` cluster label within this track.
  #[inline(always)]
  pub const fn cluster_id(&self) -> u32 {
    self.cluster_id
  }

  /// Human-assigned identity label (`""` = unassigned).
  #[inline(always)]
  pub fn name(&self) -> &str {
    self.name.as_str()
  }

  /// Total time this speaker spoke (`None` = not yet rolled up).
  #[inline(always)]
  pub const fn speech_duration_ref(&self) -> Option<&Timestamp> {
    self.speech_duration.as_ref()
  }

  /// Per-track aggregated voiceprint (`None` until aggregation runs).
  #[inline(always)]
  pub const fn voiceprint_ref(&self) -> Option<&VoiceFingerprint<Id>> {
    self.voiceprint.as_ref()
  }

  /// Cross-track identity FK → [`Person`](crate::domain::Person) (`None`
  /// = not yet identified).
  #[inline(always)]
  pub const fn person_ref(&self) -> Option<&Id> {
    self.person.as_ref()
  }

  /// Builder: replace `name` and return `self`.
  #[inline(always)]
  #[must_use]
  pub fn with_name(mut self, name: impl Into<SmolStr>) -> Self {
    self.name = name.into();
    self
  }

  /// Validating builder: replace `speech_duration` and return `self`.
  ///
  /// `speech_duration` is semantically a non-negative duration, but
  /// `mediatime::Timestamp` is an offset type that admits negative PTS — a
  /// negative speaking time is meaningless and is rejected with
  /// [`SpeakerError::NegativeSpeechDuration`]. `None` (not yet rolled up)
  /// and a zero / positive `Timestamp` are accepted. On rejection `self` is
  /// returned unchanged inside the `Err`.
  #[inline]
  pub fn try_with_speech_duration(mut self, d: Option<Timestamp>) -> Result<Self, SpeakerError> {
    if is_negative_duration(d) {
      return Err(SpeakerError::NegativeSpeechDuration);
    }
    self.speech_duration = d;
    Ok(self)
  }

  /// Builder: replace `cluster_id` and return `self`.
  #[inline(always)]
  #[must_use]
  pub const fn with_cluster_id(mut self, cluster_id: u32) -> Self {
    self.cluster_id = cluster_id;
    self
  }

  /// In-place mutator for `name`.
  #[inline(always)]
  pub fn set_name(&mut self, name: impl Into<SmolStr>) -> &mut Self {
    self.name = name.into();
    self
  }

  /// Validating in-place mutator for `speech_duration`. Rejects a negative
  /// `Timestamp` ([`SpeakerError::NegativeSpeechDuration`]); `None` and a
  /// zero / positive `Timestamp` are accepted. On rejection `self` is left
  /// unchanged.
  #[inline]
  pub const fn try_set_speech_duration(
    &mut self,
    d: Option<Timestamp>,
  ) -> Result<&mut Self, SpeakerError> {
    if is_negative_duration(d) {
      return Err(SpeakerError::NegativeSpeechDuration);
    }
    self.speech_duration = d;
    Ok(self)
  }

  /// In-place mutator for `cluster_id`.
  #[inline(always)]
  pub const fn set_cluster_id(&mut self, cluster_id: u32) -> &mut Self {
    self.cluster_id = cluster_id;
    self
  }

  // ----- `voiceprint` — full `Option<T>` mutator vocabulary ----------------

  /// Builder: put `voiceprint` into the *present* state.
  #[inline(always)]
  #[must_use]
  pub fn with_voiceprint(mut self, voiceprint: VoiceFingerprint<Id>) -> Self {
    self.voiceprint = Some(voiceprint);
    self
  }

  /// Builder: assign the *raw* `voiceprint` wrapper.
  #[inline(always)]
  #[must_use]
  pub fn maybe_voiceprint(mut self, voiceprint: Option<VoiceFingerprint<Id>>) -> Self {
    self.voiceprint = voiceprint;
    self
  }

  /// In-place mutator: put `voiceprint` into the *present* state.
  #[inline(always)]
  pub fn set_voiceprint(&mut self, voiceprint: VoiceFingerprint<Id>) -> &mut Self {
    self.voiceprint = Some(voiceprint);
    self
  }

  /// In-place mutator: assign the *raw* `voiceprint` wrapper.
  #[inline(always)]
  pub fn update_voiceprint(&mut self, voiceprint: Option<VoiceFingerprint<Id>>) -> &mut Self {
    self.voiceprint = voiceprint;
    self
  }

  /// In-place mutator: clear `voiceprint`.
  #[inline(always)]
  pub fn clear_voiceprint(&mut self) -> &mut Self {
    self.voiceprint = None;
    self
  }

  // ----- `person` — full `Option<T>` mutator vocabulary --------------------

  /// Builder: put `person` into the *present* state.
  #[inline(always)]
  #[must_use]
  pub fn with_person(mut self, person: Id) -> Self {
    self.person = Some(person);
    self
  }

  /// Builder: assign the *raw* `person` wrapper.
  #[inline(always)]
  #[must_use]
  pub fn maybe_person(mut self, person: Option<Id>) -> Self {
    self.person = person;
    self
  }

  /// In-place mutator: put `person` into the *present* state.
  #[inline(always)]
  pub fn set_person(&mut self, person: Id) -> &mut Self {
    self.person = Some(person);
    self
  }

  /// In-place mutator: assign the *raw* `person` wrapper.
  #[inline(always)]
  pub fn update_person(&mut self, person: Option<Id>) -> &mut Self {
    self.person = person;
    self
  }

  /// In-place mutator: clear `person`.
  #[inline(always)]
  pub fn clear_person(&mut self) -> &mut Self {
    self.person = None;
    self
  }
}

/// Error returned by [`Speaker`]'s validating constructor and
/// `speech_duration` mutators when an invariant cannot be upheld.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum SpeakerError {
  /// Supplied `id` was the nil sentinel — would collide as a
  /// LanceDB voiceprint key.
  #[error("Speaker id must not be the nil UUID")]
  NilId,
  /// Supplied `parent` was the nil sentinel — orphaned voice with
  /// no `AudioTrack` reference.
  #[error("Speaker parent (AudioTrack) must not be the nil UUID")]
  NilParent,
  /// A `Some(_)` `speech_duration` carried a negative `Timestamp` — a
  /// speaker's total speaking time cannot be negative.
  #[error("Speaker speech_duration must not be negative")]
  NegativeSpeechDuration,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  #[test]
  fn try_new_happy_path() {
    let parent = Uuid7::new();
    let s =
      Speaker::try_new(Uuid7::new(), parent, 2, "Jane").expect("valid construction must succeed");
    assert_eq!(s.parent_ref(), &parent);
    assert_eq!(s.cluster_id(), 2);
    assert_eq!(s.name(), "Jane");
    assert!(s.speech_duration_ref().is_none());
  }

  #[test]
  fn try_new_anonymous_diarization_uses_empty_name() {
    let s = Speaker::try_new(Uuid7::new(), Uuid7::new(), 0, "").unwrap();
    assert!(s.name().is_empty());
    assert_eq!(s.cluster_id(), 0);
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = Speaker::try_new(Uuid7::nil(), Uuid7::new(), 0, "");
    assert_eq!(r.err(), Some(SpeakerError::NilId));
    assert!(SpeakerError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_nil_parent() {
    let r = Speaker::try_new(Uuid7::new(), Uuid7::nil(), 0, "");
    assert_eq!(r.err(), Some(SpeakerError::NilParent));
    assert!(SpeakerError::NilParent.is_nil_parent());
  }

  #[test]
  fn builders_and_setters_chain() {
    let s = Speaker::try_new(Uuid7::new(), Uuid7::new(), 0, "")
      .unwrap()
      .with_name("Jane")
      .with_cluster_id(3);
    assert_eq!(s.name(), "Jane");
    assert_eq!(s.cluster_id(), 3);
    let mut s = s;
    s.set_name("");
    s.set_cluster_id(0);
    assert!(s.name().is_empty());
    assert_eq!(s.cluster_id(), 0);
  }

  // --- Finding 2: non-negative speech_duration ------------------------------

  /// A standard 1/1000 (millisecond) timebase for duration tests.
  fn tb() -> mediatime::Timebase {
    mediatime::Timebase::new(1, core::num::NonZeroU32::new(1000).expect("nonzero"))
  }

  #[test]
  fn try_with_speech_duration_rejects_negative() {
    let s = Speaker::try_new(Uuid7::new(), Uuid7::new(), 0, "Jane").unwrap();
    let neg = Timestamp::new(-1, tb());
    assert_eq!(
      s.clone().try_with_speech_duration(Some(neg)).err(),
      Some(SpeakerError::NegativeSpeechDuration)
    );
    assert!(SpeakerError::NegativeSpeechDuration.is_negative_speech_duration());
  }

  #[test]
  fn try_with_speech_duration_accepts_zero_positive_and_none() {
    let s = Speaker::try_new(Uuid7::new(), Uuid7::new(), 0, "Jane").unwrap();
    // zero
    let z = s
      .clone()
      .try_with_speech_duration(Some(Timestamp::new(0, tb())))
      .expect("zero duration accepted");
    assert_eq!(z.speech_duration_ref().unwrap().pts(), 0);
    // positive
    let p = s
      .clone()
      .try_with_speech_duration(Some(Timestamp::new(5000, tb())))
      .expect("positive duration accepted");
    assert_eq!(p.speech_duration_ref().unwrap().pts(), 5000);
    // absent
    let n = s.try_with_speech_duration(None).expect("None accepted");
    assert!(n.speech_duration_ref().is_none());
  }

  // --- voiceprint / person FK additive fields -------------------------------

  fn vfp() -> VoiceFingerprint<Uuid7> {
    use crate::domain::vo::Provenance;
    VoiceFingerprint::try_new(
      Uuid7::new(),
      192,
      jiff::Timestamp::from_millisecond(1_700_000_000_000).expect("valid ts"),
      Some(0.9),
      Provenance::from_parts("ecapa-tdnn", "v1.0.0", "", "findit-indexer-0.1.0"),
    )
    .expect("valid voiceprint")
  }

  #[test]
  fn voiceprint_and_person_default_to_none() {
    let s = Speaker::try_new(Uuid7::new(), Uuid7::new(), 0, "").unwrap();
    assert!(s.voiceprint_ref().is_none());
    assert!(s.person_ref().is_none());
  }

  #[test]
  fn voiceprint_full_option_mutator_vocabulary() {
    let v = vfp();
    let s = Speaker::try_new(Uuid7::new(), Uuid7::new(), 0, "")
      .unwrap()
      .with_voiceprint(v.clone());
    assert_eq!(s.voiceprint_ref(), Some(&v));
    let s = s.maybe_voiceprint(None);
    assert!(s.voiceprint_ref().is_none());
    let s = s.maybe_voiceprint(Some(v.clone()));
    assert_eq!(s.voiceprint_ref(), Some(&v));
    let mut s = s;
    s.clear_voiceprint();
    assert!(s.voiceprint_ref().is_none());
    s.set_voiceprint(v.clone());
    assert_eq!(s.voiceprint_ref(), Some(&v));
    s.update_voiceprint(None);
    assert!(s.voiceprint_ref().is_none());
    s.update_voiceprint(Some(v.clone()));
    assert_eq!(s.voiceprint_ref(), Some(&v));
  }

  #[test]
  fn person_full_option_mutator_vocabulary() {
    let pid = Uuid7::new();
    let s = Speaker::try_new(Uuid7::new(), Uuid7::new(), 0, "")
      .unwrap()
      .with_person(pid);
    assert_eq!(s.person_ref(), Some(&pid));
    let s = s.maybe_person(None);
    assert!(s.person_ref().is_none());
    let s = s.maybe_person(Some(pid));
    assert_eq!(s.person_ref(), Some(&pid));
    let mut s = s;
    s.clear_person();
    assert!(s.person_ref().is_none());
    s.set_person(pid);
    assert_eq!(s.person_ref(), Some(&pid));
    s.update_person(None);
    assert!(s.person_ref().is_none());
    s.update_person(Some(pid));
    assert_eq!(s.person_ref(), Some(&pid));
  }

  #[test]
  fn try_set_speech_duration_rejects_and_leaves_value_unchanged() {
    let mut s = Speaker::try_new(Uuid7::new(), Uuid7::new(), 0, "Jane").unwrap();
    s.try_set_speech_duration(Some(Timestamp::new(3000, tb())))
      .unwrap();
    // a negative update is rejected, leaving the prior value in place
    assert_eq!(
      s.try_set_speech_duration(Some(Timestamp::new(-10, tb())))
        .err(),
      Some(SpeakerError::NegativeSpeechDuration)
    );
    assert_eq!(s.speech_duration_ref().unwrap().pts(), 3000);
    // a valid update goes through
    s.try_set_speech_duration(Some(Timestamp::new(0, tb())))
      .unwrap();
    assert_eq!(s.speech_duration_ref().unwrap().pts(), 0);
  }
}
