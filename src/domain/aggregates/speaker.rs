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
use mediatime::Timestamp;
use smol_str::SmolStr;

use crate::domain::Uuid7;

/// One distinct voice in an `AudioTrack`. The track's speaker set is
/// `AudioTrack.speakers: Vec<Id> → Speaker`; each `AudioSegment.speaker:
/// Option<Id> → Speaker`.
///
/// **No `Default`**: a `Speaker` with nil `id` and nil `parent` would
/// be an orphan voice clustered as `SPEAKER_NN=0` — a real invalid
/// state (Codex PR #11 finding #3). Construct explicitly via
/// [`Speaker::try_new`] for the canonical `Uuid7` identity, or via the
/// struct literal in projection adapters that supply backend-native
/// ids.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Speaker<Id = Uuid7> {
  /// Canonical identity. Also the **LanceDB voiceprint key**.
  pub id: Id,
  /// FK → `AudioTrack.id`.
  pub parent: Id,
  /// The `dia` cluster label within this track
  /// (`dia::DiarizedSpan.speaker_id`). Stable within the diarization run;
  /// `"SPEAKER_NN"` display strings are derived from this, not stored.
  pub cluster_id: u32,
  /// Human-assigned identity label (`""` = unassigned).
  pub name: SmolStr,
  /// Total time this speaker spoke (Σ of their `AudioSegment.span`s).
  /// Maintained rollup; truth = the segments.
  ///
  /// **Semantically a non-negative duration in the track's timebase**,
  /// even though the type is `mediatime::Timestamp` (`mediatime` 0.1.6
  /// has no dedicated `Duration`). Codex PR #11 finding #1 flagged the
  /// type mismatch: a `Timestamp` is an instant/PTS and can carry
  /// negative offsets, while a duration cannot. The `try_new`
  /// constructor enforces `>= 0`; a proper `TrackDuration` newtype is
  /// a tracked follow-up in `mediatime`.
  pub speech_duration: Option<Timestamp>,
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

/// Error returned when [`Speaker::try_new`] cannot uphold the
/// non-nil-id / non-nil-parent invariants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpeakerError {
  /// Supplied `id` was the nil sentinel — would collide as a
  /// LanceDB voiceprint key.
  NilId,
  /// Supplied `parent` was the nil sentinel — orphaned voice with
  /// no `AudioTrack` reference.
  NilParent,
}

impl core::fmt::Display for SpeakerError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::NilId => f.write_str("Speaker id must not be the nil UUID"),
      Self::NilParent => f.write_str("Speaker parent (AudioTrack) must not be the nil UUID"),
    }
  }
}

#[cfg(feature = "std")]
impl std::error::Error for SpeakerError {}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn try_new_happy_path() {
    let parent = Uuid7::new();
    let s =
      Speaker::try_new(Uuid7::new(), parent, 2, "Jane").expect("valid construction must succeed");
    assert_eq!(s.parent, parent);
    assert_eq!(s.cluster_id, 2);
    assert_eq!(s.name.as_str(), "Jane");
    assert!(s.speech_duration.is_none());
  }

  #[test]
  fn try_new_anonymous_diarization_uses_empty_name() {
    // The locked rule: SmolStr "" = absent, not Option<SmolStr>.
    let s = Speaker::try_new(Uuid7::new(), Uuid7::new(), 0, "").unwrap();
    assert!(s.name.is_empty());
    assert_eq!(s.cluster_id, 0);
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = Speaker::try_new(Uuid7::nil(), Uuid7::new(), 0, "");
    assert_eq!(r.err(), Some(SpeakerError::NilId));
  }

  #[test]
  fn try_new_rejects_nil_parent() {
    let r = Speaker::try_new(Uuid7::new(), Uuid7::nil(), 0, "");
    assert_eq!(r.err(), Some(SpeakerError::NilParent));
  }
}
