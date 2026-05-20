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

use crate::domain::Uuid7;

/// One distinct voice in an `AudioTrack`. The track's speaker set is
/// `AudioTrack.speakers: Vec<Id> → Speaker`; each `AudioSegment.speaker:
/// Option<Id> → Speaker`.
///
/// **No `Default`**: a `Speaker` with nil `id` and nil `parent` would be
/// an orphan voice clustered as `SPEAKER_NN=0` — a real invalid state.
/// Construct explicitly via [`Speaker::try_new`].
///
/// Fields are private per the encapsulation rule; access via `id()` /
/// `parent()` / `cluster_id()` / `name()` / `speech_duration()` getters
/// and `with_*` / `set_*` builders/mutators.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    })
  }
}

impl<Id> Speaker<Id> {
  /// Canonical identity (also the LanceDB voiceprint key).
  #[inline]
  pub const fn id(&self) -> &Id {
    &self.id
  }

  /// FK → `AudioTrack.id`.
  #[inline]
  pub const fn parent(&self) -> &Id {
    &self.parent
  }

  /// `dia` cluster label within this track.
  #[inline]
  pub const fn cluster_id(&self) -> u32 {
    self.cluster_id
  }

  /// Human-assigned identity label (`""` = unassigned).
  #[inline]
  pub fn name(&self) -> &str {
    self.name.as_str()
  }

  /// Total time this speaker spoke (`None` = not yet rolled up).
  #[inline]
  pub const fn speech_duration(&self) -> Option<&Timestamp> {
    self.speech_duration.as_ref()
  }

  /// Builder: replace `name` and return `self`.
  #[inline]
  pub fn with_name(mut self, name: impl Into<SmolStr>) -> Self {
    self.name = name.into();
    self
  }

  /// Builder: replace `speech_duration` and return `self`.
  #[inline]
  pub fn with_speech_duration(mut self, d: Option<Timestamp>) -> Self {
    self.speech_duration = d;
    self
  }

  /// Builder: replace `cluster_id` and return `self`.
  #[inline]
  pub const fn with_cluster_id(mut self, cluster_id: u32) -> Self {
    self.cluster_id = cluster_id;
    self
  }

  /// In-place mutator for `name`.
  #[inline]
  pub fn set_name(&mut self, name: impl Into<SmolStr>) {
    self.name = name.into();
  }

  /// In-place mutator for `speech_duration`.
  #[inline]
  pub fn set_speech_duration(&mut self, d: Option<Timestamp>) {
    self.speech_duration = d;
  }

  /// In-place mutator for `cluster_id`.
  #[inline]
  pub const fn set_cluster_id(&mut self, cluster_id: u32) {
    self.cluster_id = cluster_id;
  }
}

/// Error returned when [`Speaker::try_new`] cannot uphold the
/// non-nil-id / non-nil-parent invariants. Unit-only enum.
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
    assert_eq!(s.parent(), &parent);
    assert_eq!(s.cluster_id(), 2);
    assert_eq!(s.name(), "Jane");
    assert!(s.speech_duration().is_none());
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
}
