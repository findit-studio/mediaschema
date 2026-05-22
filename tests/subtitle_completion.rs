//! External-facing regression test for the OCR-bypass bug (Codex
//! rounds 2 & 3).
//!
//! This file is an *integration test* — it sees `mediaschema` exactly as
//! a downstream consumer does and may only touch the **public** API.
//! That is the point: round 3 found that the bool-gated helpers
//! `SubtitleIndexStatus::is_fully_indexed(bool)` and
//! `SubtitleIndexStage::from_status(.., bool, ..)` were still `pub`, so a
//! caller could pass `requires_ocr = false` on an unknown/image track and
//! mark it complete without `OCR_DONE`.
//!
//! Round 4 found one more: `SubtitleIndexStatus::fully_indexed_mask(bool)`
//! was still `pub`, letting a caller write
//! `track.index_status().contains(fully_indexed_mask(false))` — the same
//! bypass in a different shape. It is now `pub(crate)` too.
//!
//! After the fix those helpers are `pub(crate)` and the ONLY public
//! completion/stage path is `SubtitleTrack::is_fully_indexed()` /
//! `SubtitleTrack::index_stage()`, which bind `requires_ocr` internally
//! from the track's codec/format. The asserts below prove an
//! unknown-codec + unknown-format track with every *text*-pipeline stage
//! bit set still cannot be reported complete / `Done` through any
//! exported path.
//!
//! `SubtitleTrack` and its friends are heap-tier domain types, so the
//! whole file is gated on a heap capability feature.
#![cfg(any(feature = "std", feature = "alloc"))]

use mediaframe::{codec::SubtitleCodec, subtitle::Format};
use mediaschema::domain::{SubtitleIndexStage, SubtitleIndexStatus, SubtitleTrack, Uuid7};
use smol_str::SmolStr;

/// Build an unknown-codec + unknown-format subtitle track whose
/// `index_status` carries every *text*-pipeline stage bit
/// (`TRACKS_DISCOVERED | CUES_EXTRACTED | SEARCH_INDEXED`) but NOT
/// `OCR_DONE`.
fn unknown_track_text_complete() -> SubtitleTrack<Uuid7> {
  use SubtitleIndexStatus as S;
  SubtitleTrack::try_new(Uuid7::new(), Uuid7::new())
    .expect("non-nil ids")
    .with_codec(SubtitleCodec::Other(SmolStr::new("mystery")))
    .with_format(Format::Other(SmolStr::new("mystery")))
    .with_index_status(S::TRACKS_DISCOVERED | S::CUES_EXTRACTED | S::SEARCH_INDEXED)
}

#[test]
fn unknown_track_without_ocr_done_is_not_fully_indexed() {
  let track = unknown_track_text_complete();

  // An unclassified track conservatively requires OCR.
  assert!(
    track.requires_ocr(),
    "unknown codec + unknown format must conservatively require OCR"
  );

  // The ONLY public completion path must reject it: `OCR_DONE` absent.
  assert!(
    !track.is_fully_indexed(),
    "unknown track without OCR_DONE must NOT report fully-indexed"
  );
}

#[test]
fn unknown_track_without_ocr_done_does_not_reach_done_stage() {
  let track = unknown_track_text_complete();

  // The ONLY public stage path must not derive `Done`.
  assert_ne!(
    track.index_stage(),
    SubtitleIndexStage::Done,
    "unknown track without OCR_DONE must NOT reach the Done stage"
  );
}

#[test]
fn raw_status_from_index_status_accessor_offers_no_bypass() {
  // `index_status()` exposes the raw `SubtitleIndexStatus`. Round 3's
  // bypass was `track.index_status().is_fully_indexed(false)`. With the
  // helper restricted to `pub(crate)` that call no longer compiles from
  // an external crate, so the only thing a consumer can do with the raw
  // status is inspect bits — never bind `requires_ocr` themselves.
  let track = unknown_track_text_complete();
  let raw: SubtitleIndexStatus = track.index_status();

  // Bit inspection is still fine and proves the dangerous shape: every
  // text bit set, `OCR_DONE` absent.
  assert!(raw.contains(SubtitleIndexStatus::TRACKS_DISCOVERED));
  assert!(raw.contains(SubtitleIndexStatus::CUES_EXTRACTED));
  assert!(raw.contains(SubtitleIndexStatus::SEARCH_INDEXED));
  assert!(!raw.contains(SubtitleIndexStatus::OCR_DONE));

  // The aggregate — the only public completion authority — still says
  // "not complete", regardless of what the raw bits look like.
  assert!(!track.is_fully_indexed());
  assert_ne!(track.index_stage(), SubtitleIndexStage::Done);
}

#[test]
fn ocr_done_completes_the_unknown_track() {
  use SubtitleIndexStatus as S;
  // Positive control: once OCR genuinely ran (`OCR_DONE` set), the same
  // track does reach completion through the public path.
  //
  // Note: the full mask is composed here from individual public bit
  // constants on purpose. `SubtitleIndexStatus::fully_indexed_mask(bool)`
  // is `pub(crate)` (Codex round 4) and is therefore unreachable from
  // this integration test — exactly the property under test. There is no
  // public, caller-supplied-`requires_ocr` completion helper to call.
  let track = unknown_track_text_complete()
    .with_index_status(S::TRACKS_DISCOVERED | S::CUES_EXTRACTED | S::OCR_DONE | S::SEARCH_INDEXED);

  assert!(
    track.is_fully_indexed(),
    "with OCR_DONE the unknown track must report fully-indexed"
  );
  assert_eq!(
    track.index_stage(),
    SubtitleIndexStage::Done,
    "with OCR_DONE the unknown track must reach the Done stage"
  );
}

/// Round-4 regression: the mask-helper bypass shape.
///
/// Before the round-4 fix a consumer could write
/// `track.index_status().contains(SubtitleIndexStatus::fully_indexed_mask(false))`
/// to mark an unknown/image track "complete" without `OCR_DONE`. With
/// `fully_indexed_mask` now `pub(crate)` that call does not compile from
/// this external crate, so the bypass is structurally unreachable.
///
/// This test reconstructs the *value* the bypass mask would have had
/// (the text-only bit set) using public bit constants and confirms the
/// only public completion authority — `SubtitleTrack` — still refuses to
/// report the track complete.
#[test]
fn mask_helper_bypass_shape_is_unreachable() {
  use SubtitleIndexStatus as S;

  // The bits a `fully_indexed_mask(false)` would have yielded.
  let text_only_mask = S::TRACKS_DISCOVERED | S::CUES_EXTRACTED | S::SEARCH_INDEXED;
  assert!(!text_only_mask.contains(S::OCR_DONE));

  let track = unknown_track_text_complete();

  // The dangerous shape: raw bits *do* contain every text-pipeline bit.
  assert!(track.index_status().contains(text_only_mask));

  // But the unknown track conservatively requires OCR, and the only
  // public completion/stage authority rejects it regardless.
  assert!(track.requires_ocr());
  assert!(
    !track.is_fully_indexed(),
    "text-only mask must not complete an OCR-requiring track via any public path"
  );
  assert_ne!(track.index_stage(), SubtitleIndexStage::Done);
}
