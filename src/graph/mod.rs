//! Standalone object-graph types — the "whole record" programming shape.
//!
//! Every domain aggregate in the media tree has a graph counterpart that
//! **owns its fields outright**. Graph types embed no `domain::*`
//! aggregates and carry none of the relational plumbing: parent FK fields
//! (`media_id`, `video_track_id`, …) are gone — the parent is the node
//! you are inside — and the id-vec fields (`files: Vec<Id>`,
//! `scenes: Vec<Id>`, `video_id: Option<Id>`, …) are replaced by the
//! children themselves (`files: Vec<MediaFile>`, `scenes: Vec<Scene>`,
//! `video: Option<Video>`).
//!
//! What stays: each node's **own `id`** (blob keys, vector-store keys,
//! SQL rows, and targeted commands address by id) and **cross-tree
//! association ids** (`Speaker::person_id`, `AudioSegment::speaker_id`,
//! `MediaFile::watched_location_id`) — nesting replaces the joins, not
//! the identity.
//!
//! Rules (locked in `docs/superpowers/specs/2026-06-10-graph-module-design.md`):
//!
//! - **Totality** — a graph is assembled fully or not at all:
//!   "everything that currently exists in storage." Empty child vecs are
//!   unambiguous because `index_status` says which stages have run.
//! - **Role** — writes and incremental stage work use the flat
//!   aggregates (their builder API is the write surface); full-record
//!   reads and transfers use graphs. Graph types are immutable:
//!   lift via [`try_from_flat`](Media::try_from_flat), then read.
//! - **Coherence** — every lift validates the flat child's parent FK
//!   against the parent it is being nested under
//!   ([`GraphError::ParentMismatch`]), and [`Media`] checks the stored
//!   facet links against the embedded facets
//!   ([`GraphError::FacetLinkMismatch`]). The FKs are *consumed* by that
//!   validation; they do not exist in the graph shape.
//!
//! Drift guard (compile-time): every lift destructures the flat
//! aggregate's `*Parts` struct **exhaustively** (no `..`), and each
//! `into_parts()` destructures its aggregate the same way — so adding a
//! field to a domain aggregate is a compile error in `into_parts`, in
//! `*Parts`, and in the lift here, until the graph mirrors it. Lifts
//! move every field; nothing is cloned.
//!
//! The module requires `std` plus all three medium features — a graph is
//! a complete record; partial-medium consumers use the flat aggregates.

mod audio;
mod subtitle;
mod video;

pub use audio::{Audio, AudioSegment, AudioTrack, Speaker};
pub use subtitle::{Subtitle, SubtitleCue, SubtitleTrack};
pub use video::{Keyframe, Scene, Video, VideoTrack};

use derive_more::{Display, IsVariant};
use indexmap::IndexMap;
use jiff::Timestamp as JiffTimestamp;
use mediaframe::{
  capture::{Device, GeoLocation},
  container::Format,
};
use mediatime::{TimeRange, Timestamp as MediaTimestamp};
use smol_str::SmolStr;

use crate::domain::{
  self,
  aggregates::{chapter::ChapterParts, media::MediaParts, media_file::MediaFileParts},
  ErrorInfo, FileChecksum, Location, MediaErrorFlags, MediaKind, Uuid7,
};

/// Which parent-child relation a [`GraphError`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IsVariant, Display)]
#[display("{}", self.as_str())]
#[non_exhaustive]
pub enum NodeKind {
  /// [`MediaFile`] under [`Media`].
  MediaFile,
  /// [`Chapter`] under [`Media`].
  Chapter,
  /// [`Video`] facet under [`Media`].
  VideoFacet,
  /// [`VideoTrack`] under [`Video`].
  VideoTrack,
  /// [`Scene`] under [`VideoTrack`].
  Scene,
  /// [`Keyframe`] under [`Scene`].
  Keyframe,
  /// [`Audio`] facet under [`Media`].
  AudioFacet,
  /// [`AudioTrack`] under [`Audio`].
  AudioTrack,
  /// [`AudioSegment`] under [`AudioTrack`].
  AudioSegment,
  /// [`Speaker`] under [`AudioTrack`].
  Speaker,
  /// [`Subtitle`] facet under [`Media`].
  SubtitleFacet,
  /// [`SubtitleTrack`] under [`Subtitle`].
  SubtitleTrack,
  /// [`SubtitleCue`] under [`SubtitleTrack`].
  SubtitleCue,
}

impl NodeKind {
  /// Stable snake_case slug — the canonical string form of every variant.
  #[inline(always)]
  pub const fn as_str(&self) -> &'static str {
    match self {
      Self::MediaFile => "media_file",
      Self::Chapter => "chapter",
      Self::VideoFacet => "video_facet",
      Self::VideoTrack => "video_track",
      Self::Scene => "scene",
      Self::Keyframe => "keyframe",
      Self::AudioFacet => "audio_facet",
      Self::AudioTrack => "audio_track",
      Self::AudioSegment => "audio_segment",
      Self::Speaker => "speaker",
      Self::SubtitleFacet => "subtitle_facet",
      Self::SubtitleTrack => "subtitle_track",
      Self::SubtitleCue => "subtitle_cue",
    }
  }
}

/// Lift failure: the flat children handed to a `try_from_flat` do not
/// form a coherent tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GraphError {
  /// A flat child's parent FK names a different parent than the one it
  /// is being nested under.
  #[error("{kind} `{child}` references parent `{referenced}`, expected `{expected}`")]
  ParentMismatch {
    /// Which relation failed.
    kind: NodeKind,
    /// The child's own id.
    child: Uuid7,
    /// The parent id the child's FK carries.
    referenced: Uuid7,
    /// The parent id it is being nested under.
    expected: Uuid7,
  },
  /// `Media`'s stored facet link (`video_id` / `audio_id` /
  /// `subtitle_id`) disagrees with the facet embedded in the graph
  /// (present vs absent, or a different id).
  #[error("{kind} link on media `{media}` disagrees with the embedded facet")]
  FacetLinkMismatch {
    /// Which facet link failed.
    kind: NodeKind,
    /// The media row's id.
    media: Uuid7,
  },
}

/// Shared lift check: the flat child's parent FK must name the parent it
/// is nested under.
pub(crate) fn parent_check(
  kind: NodeKind,
  child: Uuid7,
  referenced: &Uuid7,
  expected: &Uuid7,
) -> Result<(), GraphError> {
  if referenced == expected {
    Ok(())
  } else {
    Err(GraphError::ParentMismatch {
      kind,
      child,
      referenced: *referenced,
      expected: *expected,
    })
  }
}

/// The stored facet link and the embedded facet must agree: both absent,
/// or present with the same id.
fn facet_link_check(
  kind: NodeKind,
  media: Uuid7,
  stored: Option<&Uuid7>,
  embedded: Option<&Uuid7>,
) -> Result<(), GraphError> {
  if stored == embedded {
    Ok(())
  } else {
    Err(GraphError::FacetLinkMismatch { kind, media })
  }
}

/// The complete record for one content row — every field of the flat
/// `Media` except the relational plumbing, plus the children themselves.
#[derive(Debug, Clone, PartialEq)]
pub struct Media<Id = Uuid7> {
  id: Id,
  checksum: FileChecksum,
  format: Format,
  size: u64,
  duration: Option<MediaTimestamp>,
  kind: MediaKind,
  nb_streams: u32,
  nb_chapters: u32,
  files: Vec<MediaFile<Id>>,
  chapters: Vec<Chapter<Id>>,
  video: Option<Video<Id>>,
  audio: Option<Audio<Id>>,
  subtitle: Option<Subtitle<Id>>,
  error_flags: MediaErrorFlags,
  probe_error: Option<ErrorInfo>,
  capture_date: Option<JiffTimestamp>,
  device: Option<Device>,
  gps: Option<GeoLocation>,
}

impl Media<Uuid7> {
  /// Lift the flat aggregate into the graph shape.
  ///
  /// Validates that every flat file/chapter's `media_id` equals
  /// `media.id` and that each stored facet link agrees with the embedded
  /// facet (present with the same id, or both absent). The facets arrive
  /// pre-lifted; their `media_id` was consumed by their own lift.
  pub fn try_from_flat(
    media: domain::Media<Uuid7>,
    files: Vec<domain::MediaFile<Uuid7>>,
    chapters: Vec<domain::Chapter<Uuid7>>,
    video: Option<Video<Uuid7>>,
    audio: Option<Audio<Uuid7>>,
    subtitle: Option<Subtitle<Uuid7>>,
  ) -> Result<Self, GraphError> {
    // Exhaustive destructure: a new `Media` field is a compile error
    // here until the graph mirrors it. The reverse-lookup id vecs are
    // deliberately discarded — the children carry that information.
    let MediaParts {
      id,
      checksum,
      format,
      size,
      duration,
      kind,
      nb_streams,
      nb_chapters,
      files: _,
      chapters: _,
      video_id,
      audio_id,
      subtitle_id,
      error_flags,
      probe_error,
      capture_date,
      device,
      gps,
    } = media.into_parts();
    facet_link_check(
      NodeKind::VideoFacet,
      id,
      video_id.as_ref(),
      video.as_ref().map(|v| v.id_ref()),
    )?;
    facet_link_check(
      NodeKind::AudioFacet,
      id,
      audio_id.as_ref(),
      audio.as_ref().map(|a| a.id_ref()),
    )?;
    facet_link_check(
      NodeKind::SubtitleFacet,
      id,
      subtitle_id.as_ref(),
      subtitle.as_ref().map(|s| s.id_ref()),
    )?;
    let files = files
      .into_iter()
      .map(|f| MediaFile::try_from_flat(&id, f))
      .collect::<Result<Vec<_>, _>>()?;
    let chapters = chapters
      .into_iter()
      .map(|c| Chapter::try_from_flat(&id, c))
      .collect::<Result<Vec<_>, _>>()?;
    Ok(Self {
      id,
      checksum,
      format,
      size,
      duration,
      kind,
      nb_streams,
      nb_chapters,
      files,
      chapters,
      video,
      audio,
      subtitle,
      error_flags,
      probe_error,
      capture_date,
      device,
      gps,
    })
  }
}

impl<Id> Media<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  #[inline(always)]
  pub const fn checksum_ref(&self) -> &FileChecksum {
    &self.checksum
  }

  #[inline(always)]
  pub const fn format_ref(&self) -> &Format {
    &self.format
  }

  #[inline(always)]
  pub const fn size(&self) -> u64 {
    self.size
  }

  #[inline(always)]
  pub const fn duration_ref(&self) -> Option<&MediaTimestamp> {
    self.duration.as_ref()
  }

  #[inline(always)]
  pub const fn kind(&self) -> MediaKind {
    self.kind
  }

  #[inline(always)]
  pub const fn nb_streams(&self) -> u32 {
    self.nb_streams
  }

  #[inline(always)]
  pub const fn nb_chapters(&self) -> u32 {
    self.nb_chapters
  }

  /// This content's physical copies.
  #[inline(always)]
  pub const fn files_slice(&self) -> &[MediaFile<Id>] {
    self.files.as_slice()
  }

  /// This content's container chapters.
  #[inline(always)]
  pub const fn chapters_slice(&self) -> &[Chapter<Id>] {
    self.chapters.as_slice()
  }

  /// The video subtree, when the file has video streams.
  #[inline(always)]
  pub const fn video_ref(&self) -> Option<&Video<Id>> {
    self.video.as_ref()
  }

  /// The audio subtree, when the file has audio streams.
  #[inline(always)]
  pub const fn audio_ref(&self) -> Option<&Audio<Id>> {
    self.audio.as_ref()
  }

  /// The subtitle subtree, when the file has subtitle streams.
  #[inline(always)]
  pub const fn subtitle_ref(&self) -> Option<&Subtitle<Id>> {
    self.subtitle.as_ref()
  }

  #[inline(always)]
  pub const fn error_flags(&self) -> MediaErrorFlags {
    self.error_flags
  }

  #[inline(always)]
  pub const fn probe_error_ref(&self) -> Option<&ErrorInfo> {
    self.probe_error.as_ref()
  }

  #[inline(always)]
  pub const fn capture_date_ref(&self) -> Option<&JiffTimestamp> {
    self.capture_date.as_ref()
  }

  #[inline(always)]
  pub const fn device_ref(&self) -> Option<&Device> {
    self.device.as_ref()
  }

  #[inline(always)]
  pub const fn gps_ref(&self) -> Option<&GeoLocation> {
    self.gps.as_ref()
  }
}

/// One physical file copy — every field of the flat `MediaFile` except
/// `media_id` (implied by nesting). `watched_location_id` stays: it is a
/// cross-tree association, not a containment edge.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaFile<Id = Uuid7> {
  id: Id,
  created_at: Option<JiffTimestamp>,
  location: Location<Id>,
  watched_location_id: Id,
  watch_volume: Id,
}

impl MediaFile<Uuid7> {
  /// Lift the flat aggregate; validates `media_id == expected_media`.
  pub fn try_from_flat(
    expected_media: &Uuid7,
    file: domain::MediaFile<Uuid7>,
  ) -> Result<Self, GraphError> {
    let MediaFileParts {
      id,
      media_id,
      created_at,
      location,
      watched_location_id,
      watch_volume,
    } = file.into_parts();
    parent_check(NodeKind::MediaFile, id, &media_id, expected_media)?;
    Ok(Self {
      id,
      created_at,
      location,
      watched_location_id,
      watch_volume,
    })
  }
}

impl<Id> MediaFile<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  #[inline(always)]
  pub const fn created_at_ref(&self) -> Option<&JiffTimestamp> {
    self.created_at.as_ref()
  }

  #[inline(always)]
  pub const fn location_ref(&self) -> &Location<Id> {
    &self.location
  }

  /// Cross-tree association → the discovering `WatchedLocation`.
  #[inline(always)]
  pub const fn watched_location_id_ref(&self) -> &Id {
    &self.watched_location_id
  }

  #[inline(always)]
  pub const fn watch_volume_ref(&self) -> &Id {
    &self.watch_volume
  }
}

/// One container chapter — every field of the flat `Chapter` except
/// `media_id` (implied by nesting).
#[derive(Debug, Clone, PartialEq)]
pub struct Chapter<Id = Uuid7> {
  id: Id,
  index: u32,
  source_id: i64,
  time_range: TimeRange,
  title: SmolStr,
  metadata: IndexMap<SmolStr, SmolStr>,
}

impl Chapter<Uuid7> {
  /// Lift the flat aggregate; validates `media_id == expected_media`.
  pub fn try_from_flat(
    expected_media: &Uuid7,
    chapter: domain::Chapter<Uuid7>,
  ) -> Result<Self, GraphError> {
    let ChapterParts {
      id,
      media_id,
      index,
      source_id,
      time_range,
      title,
      metadata,
    } = chapter.into_parts();
    parent_check(NodeKind::Chapter, id, &media_id, expected_media)?;
    Ok(Self {
      id,
      index,
      source_id,
      time_range,
      title,
      metadata,
    })
  }
}

impl<Id> Chapter<Id> {
  #[inline(always)]
  pub const fn id_ref(&self) -> &Id {
    &self.id
  }

  #[inline(always)]
  pub const fn index(&self) -> u32 {
    self.index
  }

  #[inline(always)]
  pub const fn source_id(&self) -> i64 {
    self.source_id
  }

  #[inline(always)]
  pub const fn time_range_ref(&self) -> &TimeRange {
    &self.time_range
  }

  /// Chapter title (`""` = absent).
  #[inline(always)]
  pub fn title(&self) -> &str {
    self.title.as_str()
  }

  #[inline(always)]
  pub const fn title_ref(&self) -> &SmolStr {
    &self.title
  }

  #[inline(always)]
  pub const fn metadata_ref(&self) -> &IndexMap<SmolStr, SmolStr> {
    &self.metadata
  }
}

#[cfg(test)]
mod tests {
  use core::num::NonZeroU32;

  use mediatime::Timebase;

  use super::*;

  fn flat_media(id: Uuid7) -> domain::Media<Uuid7> {
    domain::Media::try_new(
      id,
      FileChecksum::from_bytes([7u8; 32]),
      Format::Mp4,
      1024,
      MediaKind::Video,
    )
    .expect("valid media")
  }

  fn span() -> TimeRange {
    TimeRange::new(0, 1000, Timebase::new(1, NonZeroU32::new(1000).unwrap()))
  }

  #[test]
  fn empty_graph_lifts_and_mirrors_scalars() {
    let id = Uuid7::new();
    let g =
      Media::try_from_flat(flat_media(id), vec![], vec![], None, None, None).expect("coherent");
    assert_eq!(g.id_ref(), &id);
    assert_eq!(g.size(), 1024);
    assert_eq!(g.kind(), MediaKind::Video);
    assert!(g.files_slice().is_empty());
    assert!(g.video_ref().is_none());
  }

  #[test]
  fn chapter_with_wrong_parent_is_rejected() {
    let id = Uuid7::new();
    let stranger = Uuid7::new();
    let chapter =
      domain::Chapter::try_new(Uuid7::new(), stranger, 0, 1, span()).expect("valid chapter");
    let err = Media::try_from_flat(flat_media(id), vec![], vec![chapter], None, None, None)
      .expect_err("incoherent chapter");
    assert!(matches!(
      err,
      GraphError::ParentMismatch {
        kind: NodeKind::Chapter,
        ..
      }
    ));
  }

  #[test]
  fn lifted_chapter_drops_fk_and_keeps_fields() {
    let id = Uuid7::new();
    let flat = domain::Chapter::try_new(Uuid7::new(), id, 3, 42, span())
      .expect("valid chapter")
      .try_with_title("Intro")
      .expect("title fits");
    let chapter_id = *flat.id_ref();
    let g = Chapter::try_from_flat(&id, flat).expect("coherent");
    assert_eq!(g.id_ref(), &chapter_id);
    assert_eq!(g.index(), 3);
    assert_eq!(g.source_id(), 42);
    assert_eq!(g.title(), "Intro");
  }

  #[test]
  fn facet_link_must_agree_both_ways() {
    let id = Uuid7::new();
    // Stored link present, embedded facet absent.
    let mut m = flat_media(id);
    m.set_video_id(Some(Uuid7::new()));
    let err =
      Media::try_from_flat(m, vec![], vec![], None, None, None).expect_err("dangling facet link");
    assert!(matches!(
      err,
      GraphError::FacetLinkMismatch {
        kind: NodeKind::VideoFacet,
        ..
      }
    ));

    // Embedded facet present, stored link absent.
    let facet = domain::Video::try_new(Uuid7::new(), id).expect("valid facet");
    let video = Video::try_from_flat(&id, facet, vec![]).expect("coherent facet");
    let err = Media::try_from_flat(flat_media(id), vec![], vec![], Some(video), None, None)
      .expect_err("missing facet link");
    assert!(matches!(
      err,
      GraphError::FacetLinkMismatch {
        kind: NodeKind::VideoFacet,
        ..
      }
    ));
  }

  #[test]
  fn coherent_video_graph_lifts_end_to_end() {
    let id = Uuid7::new();
    let facet = domain::Video::try_new(Uuid7::new(), id).expect("valid facet");
    let facet_id = *facet.id_ref();
    let track = domain::VideoTrack::try_new(Uuid7::new(), facet_id).expect("valid track");
    let track_id = *track.id_ref();
    let lifted_track = VideoTrack::try_from_flat(&facet_id, track, vec![]).expect("coherent");
    assert_eq!(lifted_track.id_ref(), &track_id);
    let video = Video::try_from_flat(&id, facet, vec![lifted_track]).expect("coherent");
    let mut m = flat_media(id);
    m.set_video_id(Some(facet_id));
    let g = Media::try_from_flat(m, vec![], vec![], Some(video), None, None).expect("coherent");
    assert_eq!(g.video_ref().expect("video").tracks_slice().len(), 1);
  }

  #[test]
  fn parent_check_reports_all_ids() {
    let (child, referenced, expected) = (Uuid7::new(), Uuid7::new(), Uuid7::new());
    let err = parent_check(NodeKind::Scene, child, &referenced, &expected).expect_err("mismatch");
    let GraphError::ParentMismatch {
      kind,
      child: c,
      referenced: r,
      expected: e,
    } = err
    else {
      panic!("wrong variant");
    };
    assert_eq!(kind, NodeKind::Scene);
    assert_eq!((c, r, e), (child, referenced, expected));
    assert!(err.to_string().contains("scene"));
  }

  #[test]
  fn node_kind_slugs_are_snake_case() {
    assert_eq!(NodeKind::VideoFacet.as_str(), "video_facet");
    assert_eq!(NodeKind::SubtitleCue.to_string(), "subtitle_cue");
  }
}
