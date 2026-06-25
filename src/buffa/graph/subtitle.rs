//! Wire ⇄ graph conversions for the subtitle subtree:
//! `media.v2::Subtitle` ⇄ [`graph::Subtitle`], `media.v2::SubtitleTrack`
//! ⇄ [`graph::SubtitleTrack`] and `media.v2::SubtitleCue` ⇄
//! [`graph::SubtitleCue`].
//!
//! Cue payloads reuse the `media.v1` per-format data messages
//! (`SrtData` … `EbuStlData`) and their existing
//! [`subtitle`](crate::buffa::subtitle) bridges; the v2 cue differs from
//! v1 in being self-contained (own `mediatime.v1.TimeRange` span — no
//! parent-timebase dance), kind-less (the oneof arm is the
//! discriminant) and FK-less (nesting implies the track).
//!
//! ## Field correspondence — `Subtitle`
//!
//! | wire field                        | graph field       | notes                |
//! | --------------------------------- | ----------------- | -------------------- |
//! | `id` (bytes, 16)                  | `id`              | validating           |
//! | `track_progress: IndexProgress`   | `track_progress`  | unset ⇒ empty rollup |
//! | `tracks: repeated SubtitleTrack`  | `tracks: Vec<_>`  | children embedded    |
//!
//! ## Field correspondence — `SubtitleTrack`
//!
//! | wire field                              | graph field                    | notes                                      |
//! | --------------------------------------- | ------------------------------ | ------------------------------------------ |
//! | `id` (bytes, 16)                        | `id`                           | validating                                 |
//! | `stream_index` / `container_track_id`   | same                           |                                            |
//! | `codec: string`                         | `codec: SubtitleCodec`         | slug; total `FromStr`                      |
//! | `format: Format` / `origin: TrackOrigin`| same                           | mediaframe externs; unset ⇒ domain default |
//! | `language: Language`                    | `language`                     | extern; non-optional in the domain         |
//! | `title: string`                         | `title`                        | `""` = absent                              |
//! | `disposition: TrackDisposition`         | `disposition`                  | extern; unset ⇒ empty flags                |
//! | `is_primary` / `auto_selected`          | same                           |                                            |
//! | `duration: Timestamp`                   | `Option<Timestamp>`            | presence = `Some`                          |
//! | `cue_count: uint32`                     | `cue_count`                    | denormalized rollup                        |
//! | `cues: repeated SubtitleCue`            | `cues: Vec<_>`                 | children embedded                          |
//! | `provenance: Provenance`                | `provenance`                   | unset ⇒ empty                              |
//! | `source_checksum: optional bytes`       | `Option<FileChecksum>`         | 32 bytes, validating                       |
//! | `character_encoding: string`            | `character_encoding`           | `""` = unknown                             |
//! | `bom_present` / `is_sdh` / `is_closed_caption` / `is_translation` | same |                                          |
//! | `kind: string`                          | `kind: SubtitleKind`           | slug; unknown rejected                     |
//! | `coverage_ratio: optional float`        | same                           |                                            |
//! | `is_empty: bool`                        | `is_empty`                     |                                            |
//! | `first_cue` / `last_cue: Timestamp`     | `Option<Timestamp>`            | presence = `Some`                          |
//! | `metadata: repeated KeyValue`           | `metadata: IndexMap`           | insertion order preserved                  |
//! | `index_status: uint32`                  | `index_status: SubtitleIndexStatus` | raw bits / `from_bits_retain`         |
//! | `index_errors: repeated ErrorInfo`      | `index_errors: Vec<_>`         |                                            |
//!
//! ## Field correspondence — `SubtitleCue`
//!
//! | wire field                  | graph field                | notes                                            |
//! | --------------------------- | -------------------------- | ------------------------------------------------ |
//! | `id` (bytes, 16)            | `id`                       | validating                                       |
//! | `ordinal: uint32`           | `ordinal`                  |                                                  |
//! | `span: TimeRange`           | `span`                     | mediatime extern; required                       |
//! | `text: LocalizedText`       | `text`                     | unset ⇒ empty VO                                 |
//! | `data` oneof (13 arms)      | `data: SubtitleCueDetails` | unset ⇒ [`BuffaError::MissingRequiredField`]     |

use buffa::MessageField;
use mediaframe::codec::SubtitleCodec;
use smol_str::SmolStr;

use super::{
  checksum_from_wire, checksum_to_wire, errors_from_wire, errors_to_wire, graph_err, id_from_wire,
  id_to_wire, index_progress_from_wire, index_progress_to_wire, metadata_from_wire,
  metadata_to_wire, opt_msg, provenance_from_wire, provenance_to_wire, rejected, unknown_slug,
};
use crate::{
  buffa::{
    error::BuffaError,
    vo::{localized_text_from_wire, localized_text_to_wire},
  },
  domain::{
    self,
    aggregates::subtitle::{
      AssData, Cea608Data, EbuStlData, LrcData, MicroDvdData, PgsData, SamiData, SbvData, SrtData,
      SubViewerData, SubtitleCueDetails, TtmlData, VobSubData, VttData,
    },
    SubtitleIndexStatus, SubtitleKind, Uuid7,
  },
  generated::media::{
    v1 as wire1,
    v2::{self as wire, __buffa::oneof::subtitle_cue::Data as WireCueData},
  },
  graph,
};

// ---------------------------------------------------------------------------
// graph::Subtitle ⇄ wire::Subtitle
// ---------------------------------------------------------------------------

impl From<&graph::Subtitle<Uuid7>> for wire::Subtitle {
  fn from(g: &graph::Subtitle<Uuid7>) -> Self {
    wire::Subtitle {
      id: id_to_wire(g.id_ref()),
      track_progress: index_progress_to_wire(g.track_progress_ref()),
      tracks: g
        .tracks_slice()
        .iter()
        .map(wire::SubtitleTrack::from)
        .collect(),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Decode the facet and its track subtrees. The flat facet's `media_id`
/// is synthesized from the facet's own id (consumed by the lift).
impl TryFrom<&wire::Subtitle> for graph::Subtitle<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::Subtitle) -> Result<Self, Self::Error> {
    let id = id_from_wire(&w.id, "Subtitle.id")?;
    let tracks = w
      .tracks
      .iter()
      .map(|t| subtitle_track_from_wire(t, id))
      .collect::<Result<Vec<_>, _>>()?;
    let flat = domain::Subtitle::try_new(id, id)
      .map_err(rejected)?
      .with_track_progress(index_progress_from_wire(&w.track_progress)?);
    graph::Subtitle::try_from_flat(&id, flat, tracks).map_err(graph_err)
  }
}

// ---------------------------------------------------------------------------
// graph::SubtitleTrack ⇄ wire::SubtitleTrack
// ---------------------------------------------------------------------------

impl From<&graph::SubtitleTrack<Uuid7>> for wire::SubtitleTrack {
  fn from(g: &graph::SubtitleTrack<Uuid7>) -> Self {
    wire::SubtitleTrack {
      id: id_to_wire(g.id_ref()),
      stream_index: g.stream_index(),
      container_track_id: g.container_track_id(),
      codec: g.codec_ref().as_str().into(),
      format: MessageField::some(g.format_ref().clone()),
      origin: MessageField::some(*g.origin_ref()),
      language: MessageField::some(*g.language_ref()),
      title: SmolStr::from(g.title()),
      disposition: MessageField::some(g.disposition()),
      is_primary: g.is_primary(),
      auto_selected: g.auto_selected(),
      duration: opt_msg(g.duration_ref().copied()),
      cue_count: g.cue_count(),
      cues: g.cues_slice().iter().map(wire::SubtitleCue::from).collect(),
      provenance: provenance_to_wire(g.provenance_ref()),
      source_checksum: g.source_checksum_ref().map(checksum_to_wire),
      character_encoding: SmolStr::from(g.character_encoding()),
      bom_present: g.bom_present(),
      is_sdh: g.is_sdh(),
      is_closed_caption: g.is_closed_caption(),
      is_translation: g.is_translation(),
      kind: g.kind().as_str().into(),
      coverage_ratio: g.coverage_ratio(),
      is_empty: g.is_empty(),
      first_cue: opt_msg(g.first_cue_ref().copied()),
      last_cue: opt_msg(g.last_cue_ref().copied()),
      metadata: metadata_to_wire(g.metadata_ref()),
      index_status: g.index_status().bits(),
      index_errors: errors_to_wire(g.index_errors_slice()),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Decode one track under the given parent facet id, then lift it with
/// its decoded cues.
fn subtitle_track_from_wire(
  w: &wire::SubtitleTrack,
  subtitle_id: Uuid7,
) -> Result<graph::SubtitleTrack<Uuid7>, BuffaError> {
  let id = id_from_wire(&w.id, "SubtitleTrack.id")?;
  let Ok(codec) = w.codec.as_str().parse::<SubtitleCodec>();
  let kind = SubtitleKind::from_str(w.kind.as_str())
    .ok_or_else(|| unknown_slug("SubtitleTrack.kind", w.kind.as_str()))?;
  let source_checksum = w
    .source_checksum
    .as_ref()
    .map(checksum_from_wire)
    .transpose()?;

  let mut t = domain::SubtitleTrack::try_new(id, subtitle_id)
    .map_err(rejected)?
    .with_stream_index(w.stream_index)
    .with_container_track_id(w.container_track_id)
    .with_codec(codec)
    .with_title(w.title.as_str())
    .with_primary(w.is_primary)
    .with_auto_selected(w.auto_selected)
    .with_duration(w.duration.as_option().copied())
    .with_cue_count(w.cue_count)
    .with_provenance(provenance_from_wire(&w.provenance))
    .with_source_checksum(source_checksum)
    .with_character_encoding(w.character_encoding.as_str())
    .with_bom_present(w.bom_present)
    .with_sdh(w.is_sdh)
    .with_closed_caption(w.is_closed_caption)
    .with_translation(w.is_translation)
    .with_kind(kind)
    .with_coverage_ratio(w.coverage_ratio)
    .with_empty(w.is_empty)
    .with_first_cue(w.first_cue.as_option().copied())
    .with_last_cue(w.last_cue.as_option().copied())
    .with_metadata(metadata_from_wire(&w.metadata))
    .with_index_status(SubtitleIndexStatus::from_bits_retain(w.index_status))
    .with_index_errors(errors_from_wire(&w.index_errors));
  if let Some(v) = w.format.as_option() {
    t = t.with_format(v.clone());
  }
  if let Some(v) = w.origin.as_option() {
    t = t.with_origin(*v);
  }
  if let Some(v) = w.language.as_option() {
    t = t.with_language(*v);
  }
  if let Some(v) = w.disposition.as_option() {
    t = t.with_disposition(*v);
  }

  let cues = w
    .cues
    .iter()
    .map(|c| flat_subtitle_cue(c, id))
    .collect::<Result<Vec<_>, _>>()?;
  graph::SubtitleTrack::try_from_flat(&subtitle_id, t, cues).map_err(graph_err)
}

/// Standalone decode — the parent FK is synthesized from the track's
/// own id and consumed by the lift.
impl TryFrom<&wire::SubtitleTrack> for graph::SubtitleTrack<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::SubtitleTrack) -> Result<Self, Self::Error> {
    let synthetic_parent = id_from_wire(&w.id, "SubtitleTrack.id")?;
    subtitle_track_from_wire(w, synthetic_parent)
  }
}

// ---------------------------------------------------------------------------
// graph::SubtitleCue ⇄ wire::SubtitleCue
// ---------------------------------------------------------------------------

impl From<&graph::SubtitleCue<Uuid7>> for wire::SubtitleCue {
  fn from(g: &graph::SubtitleCue<Uuid7>) -> Self {
    let data = match g.data_ref() {
      SubtitleCueDetails::Srt(d) => WireCueData::from(wire1::SrtData::from(d)),
      SubtitleCueDetails::Vtt(d) => WireCueData::from(wire1::VttData::from(d)),
      SubtitleCueDetails::Ass(d) => WireCueData::from(wire1::AssData::from(d)),
      SubtitleCueDetails::Lrc(d) => WireCueData::from(wire1::LrcData::from(d)),
      SubtitleCueDetails::MicroDvd(d) => WireCueData::from(wire1::MicroDvdData::from(d)),
      SubtitleCueDetails::SubViewer(d) => WireCueData::from(wire1::SubViewerData::from(d)),
      SubtitleCueDetails::Sbv(d) => WireCueData::from(wire1::SbvData::from(d)),
      SubtitleCueDetails::Ttml(d) => WireCueData::from(wire1::TtmlData::from(d)),
      SubtitleCueDetails::Sami(d) => WireCueData::from(wire1::SamiData::from(d)),
      SubtitleCueDetails::VobSub(d) => WireCueData::from(wire1::VobSubData::from(d)),
      SubtitleCueDetails::Pgs(d) => WireCueData::from(wire1::PgsData::from(d)),
      SubtitleCueDetails::Cea608(d) => WireCueData::from(wire1::Cea608Data::from(d)),
      SubtitleCueDetails::EbuStl(d) => WireCueData::from(wire1::EbuStlData::from(d)),
    };
    wire::SubtitleCue {
      id: id_to_wire(g.id_ref()),
      ordinal: g.ordinal(),
      span: MessageField::some(*g.span_ref()),
      // Empty-as-absent: an empty `LocalizedText` encodes to `none`
      // (decode maps unset ⇒ empty). See `localized_text_to_wire`.
      text: localized_text_to_wire(g.text_ref()),
      data: Some(data),
      __buffa_unknown_fields: Default::default(),
    }
  }
}

/// Reconstruct the flat cue under the given `subtitle_track_id`. The
/// oneof arm is the payload discriminant; an unset oneof has no domain
/// representation (`SubtitleCueDetails` is total over the 13 formats).
fn flat_subtitle_cue(
  w: &wire::SubtitleCue,
  subtitle_track_id: Uuid7,
) -> Result<domain::SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>>, BuffaError> {
  let id = id_from_wire(&w.id, "SubtitleCue.id")?;
  let span = *w
    .span
    .as_option()
    .ok_or(BuffaError::MissingRequiredField("SubtitleCue.span"))?;
  let text = localized_text_from_wire(&w.text);
  let data = match w
    .data
    .as_ref()
    .ok_or(BuffaError::MissingRequiredField("SubtitleCue.data"))?
  {
    WireCueData::Srt(d) => SubtitleCueDetails::Srt(SrtData::from(d.as_ref())),
    WireCueData::Vtt(d) => SubtitleCueDetails::Vtt(VttData::try_from(d.as_ref())?),
    WireCueData::Ass(d) => SubtitleCueDetails::Ass(AssData::try_from(d.as_ref())?),
    WireCueData::Lrc(d) => SubtitleCueDetails::Lrc(LrcData::from(d.as_ref())),
    WireCueData::MicroDvd(d) => SubtitleCueDetails::MicroDvd(MicroDvdData::from(d.as_ref())),
    WireCueData::SubViewer(d) => SubtitleCueDetails::SubViewer(SubViewerData::from(d.as_ref())),
    WireCueData::Sbv(d) => SubtitleCueDetails::Sbv(SbvData::from(d.as_ref())),
    WireCueData::Ttml(d) => SubtitleCueDetails::Ttml(TtmlData::try_from(d.as_ref())?),
    WireCueData::Sami(d) => SubtitleCueDetails::Sami(SamiData::from(d.as_ref())),
    WireCueData::VobSub(d) => SubtitleCueDetails::VobSub(VobSubData::try_from(d.as_ref())?),
    WireCueData::Pgs(d) => SubtitleCueDetails::Pgs(PgsData::try_from(d.as_ref())?),
    WireCueData::Cea608(d) => SubtitleCueDetails::Cea608(Cea608Data::try_from(d.as_ref())?),
    WireCueData::EbuStl(d) => SubtitleCueDetails::EbuStl(EbuStlData::try_from(d.as_ref())?),
  };
  domain::SubtitleCue::try_new(id, subtitle_track_id, w.ordinal, span, text, data).map_err(rejected)
}

/// Standalone decode — the parent FK is synthesized from the cue's own
/// id and consumed by the lift.
impl TryFrom<&wire::SubtitleCue> for graph::SubtitleCue<Uuid7> {
  type Error = BuffaError;

  fn try_from(w: &wire::SubtitleCue) -> Result<Self, Self::Error> {
    let synthetic_parent = id_from_wire(&w.id, "SubtitleCue.id")?;
    let flat = flat_subtitle_cue(w, synthetic_parent)?;
    graph::SubtitleCue::try_from_flat(&synthetic_parent, flat).map_err(graph_err)
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use core::num::NonZeroU32;

  use mediaframe::lang::Language;
  use mediatime::{TimeRange, Timebase, Timestamp};

  use super::*;
  use crate::domain::{FileChecksum, IndexProgress, LocalizedText, Provenance};

  fn tb() -> Timebase {
    Timebase::new(1, NonZeroU32::new(1000).expect("nonzero"))
  }

  fn span() -> TimeRange {
    TimeRange::new(0, 1000, tb())
  }

  fn flat_cue(track_id: Uuid7) -> domain::SubtitleCue<Uuid7, SubtitleCueDetails<Uuid7>> {
    domain::SubtitleCue::try_new(
      Uuid7::new(),
      track_id,
      0,
      span(),
      LocalizedText::from_src_translated("hello", "hola"),
      SubtitleCueDetails::Srt(SrtData::new()),
    )
    .expect("valid cue")
  }

  fn rich_track(subtitle_id: Uuid7) -> domain::SubtitleTrack<Uuid7> {
    domain::SubtitleTrack::try_new(Uuid7::new(), subtitle_id)
      .expect("valid track")
      .with_stream_index(Some(2))
      .with_container_track_id(Some(3))
      .with_codec("subrip".parse().expect("total"))
      .with_language(Language::from_bcp47("en").expect("valid tag"))
      .with_title("English (SDH)")
      .with_primary(true)
      .with_auto_selected(true)
      .with_duration(Some(Timestamp::new(90_000, tb())))
      .with_cue_count(1)
      .with_provenance(Provenance::from_parts("ffmpeg", "7.0", "", "indexer-0.1"))
      .with_source_checksum(Some(FileChecksum::from_bytes([9u8; 32])))
      .with_character_encoding("utf-8")
      .with_bom_present(true)
      .with_sdh(true)
      .with_kind(SubtitleKind::FullDialogue)
      .with_coverage_ratio(Some(0.97))
      .with_first_cue(Some(Timestamp::new(0, tb())))
      .with_last_cue(Some(Timestamp::new(1_000, tb())))
      .with_index_status(SubtitleIndexStatus::TRACKS_DISCOVERED)
  }

  #[test]
  fn subtitle_cue_round_trips_srt() {
    let track_id = Uuid7::new();
    let g = graph::SubtitleCue::try_from_flat(&track_id, flat_cue(track_id)).expect("coherent");
    let w = wire::SubtitleCue::from(&g);
    assert!(matches!(w.data, Some(WireCueData::Srt(_))));
    let g2 = graph::SubtitleCue::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }

  #[test]
  fn subtitle_cue_missing_data_errors() {
    let track_id = Uuid7::new();
    let g = graph::SubtitleCue::try_from_flat(&track_id, flat_cue(track_id)).expect("coherent");
    let mut w = wire::SubtitleCue::from(&g);
    w.data = None;
    let err = graph::SubtitleCue::try_from(&w).unwrap_err();
    assert!(err.is_missing_required_field());
  }

  #[test]
  fn subtitle_track_round_trips_with_cue() {
    let subtitle_id = Uuid7::new();
    let track = rich_track(subtitle_id);
    let track_id = *track.id_ref();
    let g = graph::SubtitleTrack::try_from_flat(&subtitle_id, track, vec![flat_cue(track_id)])
      .expect("coherent");
    let w = wire::SubtitleTrack::from(&g);
    let g2 = graph::SubtitleTrack::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }

  #[test]
  fn subtitle_track_unknown_kind_slug_errors() {
    let subtitle_id = Uuid7::new();
    let g = graph::SubtitleTrack::try_from_flat(&subtitle_id, rich_track(subtitle_id), vec![])
      .expect("coherent");
    let mut w = wire::SubtitleTrack::from(&g);
    w.kind = SmolStr::from("karaoke");
    let err = graph::SubtitleTrack::try_from(&w).unwrap_err();
    assert!(err.is_domain_constructor_rejected());
  }

  #[test]
  fn subtitle_facet_round_trips() {
    let media_id = Uuid7::new();
    let facet = domain::Subtitle::try_new(Uuid7::new(), media_id)
      .expect("valid facet")
      .with_track_progress(IndexProgress::try_new(1, 0, 1).expect("valid rollup"));
    let facet_id = *facet.id_ref();
    let track = graph::SubtitleTrack::try_from_flat(&facet_id, rich_track(facet_id), vec![])
      .expect("coherent");
    let g = graph::Subtitle::try_from_flat(&media_id, facet, vec![track]).expect("coherent");
    let w = wire::Subtitle::from(&g);
    let g2 = graph::Subtitle::try_from(&w).expect("decodes");
    assert_eq!(g2, g);
  }
}
