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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
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
    pub speech_duration: Option<Timestamp>,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_unassigned_anonymous() {
        let s: Speaker = Speaker::default();
        assert!(s.id.is_nil());
        assert!(s.parent.is_nil());
        assert_eq!(s.cluster_id, 0);
        assert!(s.name.is_empty(), "diarization is anonymous by default");
        assert!(s.speech_duration.is_none());
    }

    #[test]
    fn construct_a_named_speaker() {
        let parent = Uuid7::new();
        let s: Speaker = Speaker {
            id: Uuid7::new(),
            parent,
            cluster_id: 2,
            name: "Jane".into(),
            speech_duration: None,
        };
        assert_eq!(s.parent, parent);
        assert_eq!(s.cluster_id, 2);
        assert_eq!(s.name.as_str(), "Jane");
    }

    #[test]
    fn empty_name_is_absent_no_option() {
        // The locked rule: SmolStr "" = absent, not Option<SmolStr>.
        let s: Speaker = Speaker {
            name: "".into(),
            ..Speaker::default()
        };
        assert!(s.name.is_empty());
    }
}
