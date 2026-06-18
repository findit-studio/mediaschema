use super::*;
use core::num::NonZeroU32;
use mediatime::Timebase;

// Confirms the up-the-chain re-exports resolve: these types must be
// reachable at `crate::domain::*`, not only from the `audio` submodule.
use crate::domain::{SpeechSegment, SpeechSegmentError, SpeechSegmentParts};

fn tb() -> Timebase {
  // A standard 1/1000 timebase (millisecond ticks) is enough for the
  // domain-level invariant tests; the exact timebase isn't material.
  Timebase::new(1, NonZeroU32::new(1000).expect("nonzero"))
}

fn span(start_ticks: i64, end_ticks: i64) -> TimeRange {
  TimeRange::new(start_ticks, end_ticks, tb())
}

#[test]
fn try_new_happy_path() {
  let audio_track_id = Uuid7::new();
  let s = SpeechSegment::try_new(Uuid7::new(), audio_track_id, 0, span(0, 1500), 0.87)
    .expect("valid construction must succeed");
  assert_eq!(s.audio_track_id_ref(), &audio_track_id);
  assert_eq!(s.index(), 0);
  assert_eq!(s.span_ref(), &span(0, 1500));
  assert!((s.confidence() - 0.87).abs() < f32::EPSILON);
}

#[test]
fn try_new_rejects_nil_id() {
  let r = SpeechSegment::try_new(Uuid7::nil(), Uuid7::new(), 0, span(0, 1500), 0.5);
  assert_eq!(r.err(), Some(SpeechSegmentError::NilId));
  assert!(SpeechSegmentError::NilId.is_nil_id());
}

#[test]
fn try_new_rejects_nil_audio_track_id() {
  let r = SpeechSegment::try_new(Uuid7::new(), Uuid7::nil(), 0, span(0, 1500), 0.5);
  assert_eq!(r.err(), Some(SpeechSegmentError::NilAudioTrackId));
  assert!(SpeechSegmentError::NilAudioTrackId.is_nil_audio_track_id());
}

#[test]
fn try_new_rejects_non_finite_or_out_of_range_confidence() {
  for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 1.1] {
    let r = SpeechSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 100), bad);
    assert_eq!(r.err(), Some(SpeechSegmentError::ConfidenceOutOfRange));
  }
  // boundary values are accepted
  assert!(SpeechSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 100), 0.0,).is_ok());
  assert!(SpeechSegment::try_new(Uuid7::new(), Uuid7::new(), 0, span(0, 100), 1.0,).is_ok());
  assert!(SpeechSegmentError::ConfidenceOutOfRange.is_confidence_out_of_range());
}

#[test]
fn try_new_accepts_zero_length_span() {
  // `start == end` is allowed (a degenerate instant range).
  SpeechSegment::try_new(Uuid7::new(), Uuid7::new(), 3, span(500, 500), 1.0)
    .expect("zero-length span ok");
}

#[test]
fn accessors_round_trip_a_valid_segment() {
  let id = Uuid7::new();
  let track = Uuid7::new();
  let s = SpeechSegment::try_new(id, track, 2, span(100, 900), 0.42).expect("valid");
  assert_eq!(s.id_ref(), &id);
  assert_eq!(s.audio_track_id_ref(), &track);
  assert_eq!(s.index(), 2);
  assert_eq!(s.span_ref(), &span(100, 900));
  assert!((s.confidence() - 0.42).abs() < f32::EPSILON);
}

#[test]
fn try_new_rejects_inverted_span() {
  // `TimeRange::try_new` rejects an inverted span at construction...
  assert!(TimeRange::try_new(2000, 1000, tb()).is_none());
  // ...but its public `with_*` mutators can invert a *valid* range after
  // the fact — `SpeechSegment::try_new` must re-validate.
  let inverted = TimeRange::new(1_000, 5_000, tb()).with_end(0);
  assert!(inverted.start_pts() > inverted.end_pts());
  let r = SpeechSegment::try_new(Uuid7::new(), Uuid7::new(), 0, inverted, 0.5);
  assert_eq!(r.err(), Some(SpeechSegmentError::InvertedSpan));
  assert!(SpeechSegmentError::InvertedSpan.is_inverted_span());
}

#[test]
fn into_parts_exposes_every_field() {
  let track = Uuid7::new();
  let s = SpeechSegment::try_new(Uuid7::new(), track, 2, span(100, 900), 0.42).expect("valid");
  let parts = s.into_parts();
  assert_eq!(parts.audio_track_id, track);
  assert_eq!(parts.index, 2);
  assert_eq!(parts.span, span(100, 900));
  assert!((parts.confidence - 0.42).abs() < f32::EPSILON);
}

#[test]
fn into_parts_round_trips_through_try_new() {
  let s =
    SpeechSegment::try_new(Uuid7::new(), Uuid7::new(), 2, span(100, 900), 0.42).expect("valid");
  let SpeechSegmentParts {
    id,
    audio_track_id,
    index,
    span,
    confidence,
  } = s.clone().into_parts();
  let back = SpeechSegment::try_new(id, audio_track_id, index, span, confidence)
    .expect("re-validating an already-valid decomposition must succeed");
  assert_eq!(back, s);
}
