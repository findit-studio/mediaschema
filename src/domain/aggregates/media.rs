//! `Media<Id>` — shared content row (locked `schema/media.md` r9).
//!
//! The indexed media **content** — one row per content hash. Many physical
//! copies of the same bytes (the same file duplicated across
//! folders/volumes) collapse to **one** `Media`; the copy-specific
//! metadata (file name, path, filesystem creation time, discovering watch)
//! lives on the per-copy [`MediaFile`](super::MediaFile) aggregate, and
//! `files: Vec<Id>` is the reverse lookup to those copies. All
//! **content-intrinsic** scalar metadata lives here and nowhere else; the
//! kind facets are thin aggregates and stream/codec data is per-track.
//! `Media` is the architectural root of the domain — every other aggregate
//! transitively descends from it via the three optional facet FKs
//! (`video`/`audio`/`subtitle`).
//!
//! ## Cross-cutting (locked)
//!
//! - Generic over `Id` (default [`Uuid7`]); facet FKs are the same UUIDv7.
//! - [`FileChecksum`] is the 32-byte content hash — a **distinct newtype
//!   from `Id`** (content ≠ identity).
//! - Wall-clock: [`jiff::Timestamp`] (ms-resolution).
//! - Media-time (overall duration): [`mediatime::Timestamp`]. (The locked
//!   doc names it `mediatime::TrackTime`, but `mediatime 0.1.6` only
//!   exports `Timestamp`/`TimeRange`/`Timebase`; same doc/code drift as
//!   `Speaker.speech_duration` — read the doc's `TrackTime` as
//!   `mediatime::Timestamp` pending a doc-name fix.)
//! - `error_flags` is a maintained **rollup** of per-kind track failures
//!   ([`MediaErrorFlags`] — `VIDEO_ERROR` / `AUDIO_ERROR` /
//!   `SUBTITLE_ERROR` + reserved bits). Drill-down details live on the
//!   tracks (`*Track.index_errors`).
//! - `probe_error` is the **one** non-track error case — file unprobeable
//!   ⇒ no tracks were created.
//! - `device` / `gps` are the published `mediaframe` capture-metadata
//!   types ([`mediaframe::capture::Device`] /
//!   [`mediaframe::capture::GeoLocation`]; EXIF / capture-metadata
//!   charter).
//!
//! ## Encapsulation
//!
//! No public fields. Access via getters (`const fn` where possible);
//! mutation via `with_*` builders and `set_*` setters (returning `()`).
//! `try_new` is the validating constructor; nil `id` and nil `checksum`
//! are rejected.

use derive_more::IsVariant;
use jiff::Timestamp as JiffTimestamp;
use mediaframe::{
  capture::{Device, GeoLocation},
  container::Format,
};
use mediatime::Timestamp as MediaTimestamp;

use crate::domain::{ErrorInfo, FileChecksum, MediaErrorFlags, MediaKind, Uuid7};

/// The indexed media content — the architectural root of the domain.
///
/// **The content row, one per content hash.** Per-copy metadata (name,
/// path, filesystem creation time, discovering watch) lives on
/// [`MediaFile`](super::MediaFile); `files` is the reverse lookup to this
/// content's copies.
///
/// **No `Default` impl** — defaulting to `{ id: nil, checksum: zero, … }`
/// would represent an orphan content row with no real identity. Construct
/// via [`Media::try_new`] (validating: rejects nil `id` and zero
/// `checksum`).
///
/// **Fields are private**; access via getters and `with_*` / `set_*`
/// mutators per the encapsulation rule.
#[derive(Debug, Clone, PartialEq)]
pub struct Media<Id = Uuid7> {
  id: Id,
  checksum: FileChecksum,
  /// **Container** format (MP4/MKV/MKA/…). Codec is per-track.
  format: Format,
  size: u64,
  duration: Option<MediaTimestamp>,
  kind: MediaKind,
  /// Reverse lookup → this content's [`MediaFile`](super::MediaFile)
  /// copies (the reverse side of `MediaFile.media_id`).
  files: std::vec::Vec<Id>,
  /// FK → `Video` facet (`None` = no video stream on this file).
  video: Option<Id>,
  /// FK → `Audio` facet (`None` = no audio stream).
  audio: Option<Id>,
  /// FK → `Subtitle` facet (`None` = no subtitle stream).
  subtitle: Option<Id>,
  /// Maintained rollup: a bit is set iff that kind's `track_progress.failed
  /// > 0`. Drill-down details live on `*Track.index_errors`.
  error_flags: MediaErrorFlags,
  /// **File-level** probe failure only — the one non-track error case
  /// (file unprobeable ⇒ no tracks were created).
  probe_error: Option<ErrorInfo>,
  /// EXIF capture date (wall-clock; `None` = not recorded; wire encodes
  /// `0` as `None`).
  capture_date: Option<JiffTimestamp>,
  /// EXIF device info (camera / phone make+model).
  device: Option<Device>,
  /// EXIF GPS reading.
  gps: Option<GeoLocation>,
}

impl Media<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects nil `id` (every content row must have a real identity) and
  /// the zero `checksum` sentinel ("not yet computed" — a probed file
  /// always has its content hash before reaching the domain). Other
  /// content-intrinsic fields are caller-supplied; `files` starts empty
  /// (filled via `push_file` / `with_files` as copies are discovered) and
  /// facet FKs default to `None`, filled in via `with_video` /
  /// `with_audio` / `with_subtitle` after the corresponding facet
  /// aggregates land.
  pub fn try_new(
    id: Uuid7,
    checksum: FileChecksum,
    format: Format,
    size: u64,
    kind: MediaKind,
  ) -> Result<Self, MediaError> {
    if id.is_nil() {
      return Err(MediaError::NilId);
    }
    if checksum.is_zero() {
      return Err(MediaError::ZeroChecksum);
    }
    Ok(Self {
      id,
      checksum,
      format,
      size,
      duration: None,
      kind,
      files: std::vec::Vec::new(),
      video: None,
      audio: None,
      subtitle: None,
      error_flags: MediaErrorFlags::new(),
      probe_error: None,
      capture_date: None,
      device: None,
      gps: None,
    })
  }
}

impl<Id> Media<Id> {
  /// Canonical identity.
  #[inline]
  pub const fn id(&self) -> &Id {
    &self.id
  }

  /// Content hash (the unique-across-`Media` index).
  #[inline]
  pub const fn checksum(&self) -> &FileChecksum {
    &self.checksum
  }

  /// Container format (MP4/MKV/MKA/…).
  #[inline]
  pub const fn format(&self) -> &Format {
    &self.format
  }

  /// File size in bytes.
  #[inline]
  pub const fn size(&self) -> u64 {
    self.size
  }

  /// **Overall** media length (per-track duration is on the track).
  #[inline]
  pub const fn duration(&self) -> Option<&MediaTimestamp> {
    self.duration.as_ref()
  }

  /// Top-level media kind.
  #[inline]
  pub const fn kind(&self) -> MediaKind {
    self.kind
  }

  /// Reverse lookup → this content's [`MediaFile`](super::MediaFile)
  /// copies (projects the slice, never `&Vec`).
  #[inline]
  pub fn files(&self) -> &[Id] {
    self.files.as_slice()
  }

  /// FK → `Video` facet.
  #[inline]
  pub const fn video(&self) -> Option<&Id> {
    self.video.as_ref()
  }

  /// FK → `Audio` facet.
  #[inline]
  pub const fn audio(&self) -> Option<&Id> {
    self.audio.as_ref()
  }

  /// FK → `Subtitle` facet.
  #[inline]
  pub const fn subtitle(&self) -> Option<&Id> {
    self.subtitle.as_ref()
  }

  /// Per-kind error rollup.
  #[inline]
  pub const fn error_flags(&self) -> MediaErrorFlags {
    self.error_flags
  }

  /// File-level probe error (the non-track case).
  #[inline]
  pub const fn probe_error(&self) -> Option<&ErrorInfo> {
    self.probe_error.as_ref()
  }

  /// EXIF capture timestamp.
  #[inline]
  pub const fn capture_date(&self) -> Option<&JiffTimestamp> {
    self.capture_date.as_ref()
  }

  /// EXIF device info.
  #[inline]
  pub const fn device(&self) -> Option<&Device> {
    self.device.as_ref()
  }

  /// EXIF GPS reading.
  #[inline]
  pub const fn gps(&self) -> Option<&GeoLocation> {
    self.gps.as_ref()
  }

  // --- builders -----------------------------------------------------------

  /// Builder: replace overall `duration`.
  #[inline]
  pub fn with_duration(mut self, d: Option<MediaTimestamp>) -> Self {
    self.duration = d;
    self
  }

  /// Builder: replace the whole `files` reverse-lookup list.
  #[inline]
  pub fn with_files(mut self, files: std::vec::Vec<Id>) -> Self {
    self.files = files;
    self
  }

  /// Builder: append one `MediaFile` id to the reverse-lookup list.
  #[inline]
  pub fn push_file(mut self, file: Id) -> Self {
    self.files.push(file);
    self
  }

  /// Builder: set the `Video` facet FK.
  #[inline]
  pub fn with_video(mut self, video: Option<Id>) -> Self {
    self.video = video;
    self
  }

  /// Builder: set the `Audio` facet FK.
  #[inline]
  pub fn with_audio(mut self, audio: Option<Id>) -> Self {
    self.audio = audio;
    self
  }

  /// Builder: set the `Subtitle` facet FK.
  #[inline]
  pub fn with_subtitle(mut self, subtitle: Option<Id>) -> Self {
    self.subtitle = subtitle;
    self
  }

  /// Builder: replace the per-kind error-rollup.
  #[inline]
  pub const fn with_error_flags(mut self, flags: MediaErrorFlags) -> Self {
    self.error_flags = flags;
    self
  }

  /// Builder: replace `probe_error`.
  #[inline]
  pub fn with_probe_error(mut self, e: Option<ErrorInfo>) -> Self {
    self.probe_error = e;
    self
  }

  /// Builder: replace `capture_date`, collapsing the 0-ms wire sentinel
  /// to `None`.
  ///
  /// The wire encodes `capture_date == 0` (Unix epoch, ms) as absent, so
  /// storing `Some(<0-ms timestamp>)` would round-trip back to `None` and
  /// lose data. To keep the domain incapable of holding the sentinel as a
  /// real date, a `Some(t)` whose `t.as_millisecond() == 0` is collapsed
  /// to `None` here. (`Timestamp::as_millisecond` is not `const`, so this
  /// builder is not `const`.)
  #[inline]
  pub fn with_capture_date(mut self, t: Option<JiffTimestamp>) -> Self {
    self.capture_date = normalize_capture_date(t);
    self
  }

  /// Builder: replace `device`.
  #[inline]
  pub fn with_device(mut self, d: Option<Device>) -> Self {
    self.device = d;
    self
  }

  /// Builder: replace `gps`.
  #[inline]
  pub const fn with_gps(mut self, g: Option<GeoLocation>) -> Self {
    self.gps = g;
    self
  }

  // --- in-place setters ---------------------------------------------------

  /// In-place mutator for overall `duration`.
  #[inline]
  pub fn set_duration(&mut self, d: Option<MediaTimestamp>) {
    self.duration = d;
  }

  /// In-place mutator: replace the whole `files` reverse-lookup list.
  #[inline]
  pub fn set_files(&mut self, files: std::vec::Vec<Id>) {
    self.files = files;
  }

  /// In-place mutator: append one `MediaFile` id to the reverse-lookup
  /// list.
  #[inline]
  pub fn add_file(&mut self, file: Id) {
    self.files.push(file);
  }

  /// In-place mutator for the `Video` facet FK.
  #[inline]
  pub fn set_video(&mut self, video: Option<Id>) {
    self.video = video;
  }

  /// In-place mutator for the `Audio` facet FK.
  #[inline]
  pub fn set_audio(&mut self, audio: Option<Id>) {
    self.audio = audio;
  }

  /// In-place mutator for the `Subtitle` facet FK.
  #[inline]
  pub fn set_subtitle(&mut self, subtitle: Option<Id>) {
    self.subtitle = subtitle;
  }

  /// In-place mutator for the per-kind error rollup.
  #[inline]
  pub const fn set_error_flags(&mut self, flags: MediaErrorFlags) {
    self.error_flags = flags;
  }

  /// In-place mutator for `probe_error`.
  #[inline]
  pub fn set_probe_error(&mut self, e: Option<ErrorInfo>) {
    self.probe_error = e;
  }

  /// In-place mutator for `capture_date`, collapsing the 0-ms wire
  /// sentinel to `None` (see [`Media::with_capture_date`]).
  #[inline]
  pub fn set_capture_date(&mut self, t: Option<JiffTimestamp>) {
    self.capture_date = normalize_capture_date(t);
  }

  /// In-place mutator for `device`.
  #[inline]
  pub fn set_device(&mut self, d: Option<Device>) {
    self.device = d;
  }

  /// In-place mutator for `gps`.
  #[inline]
  pub const fn set_gps(&mut self, g: Option<GeoLocation>) {
    self.gps = g;
  }
}

/// Collapse the 0-ms `capture_date` wire sentinel to `None`.
///
/// The wire codec encodes `0` (Unix epoch, ms) as "absent", so a real
/// `Some(<0-ms>)` would be indistinguishable from `None` after a
/// round-trip. Normalising on the way in keeps the domain incapable of
/// holding the sentinel as a genuine date.
#[inline]
fn normalize_capture_date(t: Option<JiffTimestamp>) -> Option<JiffTimestamp> {
  match t {
    Some(ts) if ts.as_millisecond() == 0 => None,
    other => other,
  }
}

/// Error returned when [`Media::try_new`] cannot uphold the
/// non-nil-id / non-zero-checksum invariants. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum MediaError {
  /// Supplied `id` was the [`Uuid7`] nil sentinel — not a real identity.
  #[error("Media id must not be the nil UUID")]
  NilId,
  /// Supplied `checksum` was the all-zero "not yet computed" sentinel
  /// — a `Media` reaches the domain layer only after probing, so the
  /// content hash should already be filled in.
  #[error("Media checksum must not be the all-zero sentinel (file must be probed)")]
  ZeroChecksum,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::*;

  fn fake_checksum() -> FileChecksum {
    let mut bytes = [0u8; 32];
    bytes[0] = 0x01;
    FileChecksum::from_bytes(bytes)
  }

  /// A real (non-epoch) capture timestamp — `JiffTimestamp::default()`
  /// is 0 ms (Unix epoch), which `with_capture_date` collapses to `None`,
  /// so capture-date tests must use a non-zero instant.
  fn real_ts() -> JiffTimestamp {
    JiffTimestamp::from_millisecond(1_700_000_000_000).expect("valid timestamp")
  }

  #[test]
  fn try_new_happy_path() {
    let id = Uuid7::new();
    let cs = fake_checksum();
    let m = Media::try_new(id, cs, Format::Mp4, 12_345, MediaKind::Video)
      .expect("valid construction must succeed");
    assert_eq!(m.id(), &id);
    assert_eq!(m.checksum(), &cs);
    assert_eq!(m.format(), &Format::Mp4);
    assert_eq!(m.size(), 12_345);
    assert!(m.kind().is_video());
    assert!(m.files().is_empty(), "files start empty on construction");
    assert!(m.video().is_none());
    assert!(m.audio().is_none());
    assert!(m.subtitle().is_none());
    assert!(m.duration().is_none());
    assert_eq!(m.error_flags(), MediaErrorFlags::new());
    assert!(m.probe_error().is_none());
    assert!(m.capture_date().is_none());
    assert!(m.device().is_none());
    assert!(m.gps().is_none());
  }

  #[test]
  fn try_new_rejects_nil_id() {
    let r = Media::try_new(
      Uuid7::nil(),
      fake_checksum(),
      Format::Mp4,
      0,
      MediaKind::Video,
    );
    assert_eq!(r.err(), Some(MediaError::NilId));
    assert!(MediaError::NilId.is_nil_id());
  }

  #[test]
  fn try_new_rejects_zero_checksum() {
    let r = Media::try_new(
      Uuid7::new(),
      FileChecksum::new(),
      Format::Mp4,
      0,
      MediaKind::Video,
    );
    assert_eq!(r.err(), Some(MediaError::ZeroChecksum));
    assert!(MediaError::ZeroChecksum.is_zero_checksum());
  }

  #[test]
  fn files_round_trip() {
    let a = Uuid7::new();
    let b = Uuid7::new();
    let m = Media::try_new(
      Uuid7::new(),
      fake_checksum(),
      Format::Mp4,
      0,
      MediaKind::Video,
    )
    .unwrap()
    .push_file(a)
    .with_files(vec![a, b]);
    assert_eq!(m.files(), &[a, b]);

    // In-place vocabulary mirrors the builders.
    let mut m = m;
    let c = Uuid7::new();
    m.add_file(c);
    assert_eq!(m.files(), &[a, b, c]);
    m.set_files(vec![c]);
    assert_eq!(m.files(), &[c]);
  }

  #[test]
  fn capture_date_collapses_zero_ms_sentinel() {
    let m = Media::try_new(Uuid7::new(), fake_checksum(), Format::Mp4, 0, MediaKind::Video)
      .unwrap()
      // Epoch (0 ms) is the wire "absent" sentinel — must collapse to None.
      .with_capture_date(Some(JiffTimestamp::default()));
    assert!(
      m.capture_date().is_none(),
      "0-ms capture_date must collapse to None"
    );

    // A real instant survives.
    let m = m.with_capture_date(Some(real_ts()));
    assert_eq!(m.capture_date(), Some(&real_ts()));

    // The in-place setter collapses identically.
    let mut m = m;
    m.set_capture_date(Some(JiffTimestamp::default()));
    assert!(m.capture_date().is_none());
  }

  #[test]
  fn builders_chain() {
    let id = Uuid7::new();
    let video_id = Uuid7::new();
    let audio_id = Uuid7::new();
    let gps = GeoLocation::try_new(37.7749, -122.4194, Some(20.0)).expect("valid coordinates");
    let m = Media::try_new(id, fake_checksum(), Format::Mp4, 12_345, MediaKind::Video)
      .unwrap()
      .with_video(Some(video_id))
      .with_audio(Some(audio_id))
      .with_error_flags(MediaErrorFlags::VIDEO_ERROR)
      .with_capture_date(Some(real_ts()))
      .with_device(Some(
        Device::new().with_make("Apple").with_model("iPhone 15 Pro"),
      ))
      .with_gps(Some(gps));

    assert_eq!(m.video(), Some(&video_id));
    assert_eq!(m.audio(), Some(&audio_id));
    assert!(m.subtitle().is_none());
    assert_eq!(m.error_flags(), MediaErrorFlags::VIDEO_ERROR);
    assert!(m.capture_date().is_some());
    let dev = m.device().expect("device set");
    assert_eq!(dev.make(), "Apple");
    assert_eq!(dev.model(), "iPhone 15 Pro");
    let gps = m.gps().expect("gps set");
    assert_eq!(gps.lat(), 37.7749);
    assert_eq!(gps.lon(), -122.4194);
    assert_eq!(gps.altitude(), Some(20.0));
  }

  #[test]
  fn setters_mutate_in_place() {
    let mut m = Media::try_new(
      Uuid7::new(),
      fake_checksum(),
      Format::Mp4,
      0,
      MediaKind::Video,
    )
    .unwrap();
    m.set_video(Some(Uuid7::new()));
    m.set_error_flags(MediaErrorFlags::AUDIO_ERROR | MediaErrorFlags::SUBTITLE_ERROR);
    m.set_gps(Some(
      GeoLocation::try_new(0.0, 0.0, None).expect("valid coordinates"),
    ));
    assert!(m.video().is_some());
    assert!(m
      .error_flags()
      .contains(MediaErrorFlags::AUDIO_ERROR | MediaErrorFlags::SUBTITLE_ERROR));
    assert_eq!(m.gps().map(GeoLocation::altitude), Some(None));
  }
}
