//! GraphQL exposure of [`Media`] + its file-level VOs
//! ([`MediaDevice`], [`MediaGeoLocation`]) + the shared VOs that
//! resolvers ultimately reach ([`Provenance`], [`LocalizedText`],
//! [`ErrorInfo`], [`Rgba`], [`Location`] / [`LocalLocation`]).
//!
//! Every `#[Object]` impl is on a `Gql*` newtype wrapper because the
//! domain types already define inherent methods with the same names as
//! the GraphQL fields; the macro extends the impl block of the type it
//! decorates, so those names would collide.

use async_graphql::{Object, Union, ID};

use mediaframe::capture::{Device as MediaDevice, GeoLocation as MediaGeoLocation};

use crate::domain::{
  primitives::{ErrorCode, LocalLocation, Location, UnknownErrorCode},
  ErrorInfo, FileChecksum, LocalizedText, Media, Provenance, Rgba, Uuid7,
};

use super::{
  bitflags::GqlMediaErrorFlags,
  enums::GqlMediaKind,
  scalars::{empty_as_none, GqlJiffTimestamp, GqlMediaTimestamp},
};

// ---------------------------------------------------------------------------
// Provenance
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`Provenance`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GqlProvenance(pub Provenance);

impl From<Provenance> for GqlProvenance {
  #[inline]
  fn from(v: Provenance) -> Self {
    Self(v)
  }
}
impl From<GqlProvenance> for Provenance {
  #[inline]
  fn from(v: GqlProvenance) -> Self {
    v.0
  }
}

#[Object(name = "Provenance")]
impl GqlProvenance {
  async fn model_name(&self) -> Option<String> {
    empty_as_none(self.0.model_name())
  }
  async fn model_version(&self) -> Option<String> {
    empty_as_none(self.0.model_version())
  }
  async fn prompt_version(&self) -> Option<String> {
    empty_as_none(self.0.prompt_version())
  }
  async fn indexer_version(&self) -> Option<String> {
    empty_as_none(self.0.indexer_version())
  }
  async fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
}

// ---------------------------------------------------------------------------
// LocalizedText
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`LocalizedText`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GqlLocalizedText(pub LocalizedText);

impl From<LocalizedText> for GqlLocalizedText {
  #[inline]
  fn from(v: LocalizedText) -> Self {
    Self(v)
  }
}
impl From<GqlLocalizedText> for LocalizedText {
  #[inline]
  fn from(v: GqlLocalizedText) -> Self {
    v.0
  }
}

#[Object(name = "LocalizedText")]
impl GqlLocalizedText {
  async fn src(&self) -> Option<String> {
    empty_as_none(self.0.src())
  }
  async fn translated(&self) -> Option<String> {
    empty_as_none(self.0.translated())
  }
  async fn display(&self) -> String {
    self.0.display().to_string()
  }
}

// ---------------------------------------------------------------------------
// ErrorInfo
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`ErrorInfo`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GqlErrorInfo(pub ErrorInfo);

impl From<ErrorInfo> for GqlErrorInfo {
  #[inline]
  fn from(v: ErrorInfo) -> Self {
    Self(v)
  }
}
impl From<GqlErrorInfo> for ErrorInfo {
  #[inline]
  fn from(v: GqlErrorInfo) -> Self {
    v.0
  }
}

#[Object(name = "ErrorInfo")]
impl GqlErrorInfo {
  async fn code(&self) -> u32 {
    self.0.code().as_u32()
  }
  async fn code_name(&self) -> String {
    error_code_name(self.0.code())
  }
  async fn message(&self) -> Option<String> {
    empty_as_none(self.0.message())
  }
}

/// Stable variant-name projection — used by `ErrorInfo.code_name`.
pub(crate) fn error_code_name(code: ErrorCode) -> String {
  match code {
    ErrorCode::BadRequest => "BAD_REQUEST",
    ErrorCode::PermissionDenied => "PERMISSION_DENIED",
    ErrorCode::NotFound => "NOT_FOUND",
    ErrorCode::AlreadyExists => "ALREADY_EXISTS",
    ErrorCode::UnprocessableEntity => "UNPROCESSABLE_ENTITY",
    ErrorCode::InternalError => "INTERNAL_ERROR",
    ErrorCode::ServiceUnavailable => "SERVICE_UNAVAILABLE",
    ErrorCode::Timeout => "TIMEOUT",
    ErrorCode::ProbeCorrupt => "PROBE_CORRUPT",
    ErrorCode::ProbeUnsupportedFormat => "PROBE_UNSUPPORTED_FORMAT",
    ErrorCode::ProbeNoVideoStream => "PROBE_NO_VIDEO_STREAM",
    ErrorCode::ProbeNoAudioStream => "PROBE_NO_AUDIO_STREAM",
    ErrorCode::SceneDetectionFailed => "SCENE_DETECTION_FAILED",
    ErrorCode::SceneDetectionModelError => "SCENE_DETECTION_MODEL_ERROR",
    ErrorCode::TranscriptionFailed => "TRANSCRIPTION_FAILED",
    ErrorCode::TranscriptionModelError => "TRANSCRIPTION_MODEL_ERROR",
    ErrorCode::VlmFailed => "VLM_FAILED",
    ErrorCode::VlmModelError => "VLM_MODEL_ERROR",
    ErrorCode::AppleVisionFailed => "APPLE_VISION_FAILED",
    ErrorCode::AppleVisionRequestFailed => "APPLE_VISION_REQUEST_FAILED",
    ErrorCode::EmbeddingFailed => "EMBEDDING_FAILED",
    ErrorCode::EmbeddingModelError => "EMBEDDING_MODEL_ERROR",
    ErrorCode::EmbeddingModelLoadFailed => "EMBEDDING_MODEL_LOAD_FAILED",
    ErrorCode::EmbeddingPreprocessFailed => "EMBEDDING_PREPROCESS_FAILED",
    ErrorCode::EmbeddingInferenceFailed => "EMBEDDING_INFERENCE_FAILED",
    ErrorCode::EmbeddingOutputInvalid => "EMBEDDING_OUTPUT_INVALID",
    ErrorCode::PathNotFound => "PATH_NOT_FOUND",
    ErrorCode::VolumeNotAvailable => "VOLUME_NOT_AVAILABLE",
    ErrorCode::MissingVolumeId => "MISSING_VOLUME_ID",
    ErrorCode::MalformedVolumeId => "MALFORMED_VOLUME_ID",
    ErrorCode::VolumeIdMismatch => "VOLUME_ID_MISMATCH",
    ErrorCode::LocalDatabaseError => "LOCAL_DATABASE_ERROR",
    ErrorCode::FolderNotAvailable => "FOLDER_NOT_AVAILABLE",
    ErrorCode::LocalPermissionDenied => "LOCAL_PERMISSION_DENIED",
    ErrorCode::EndpointUnreachable => "ENDPOINT_UNREACHABLE",
    ErrorCode::AuthenticationFailed => "AUTHENTICATION_FAILED",
    ErrorCode::BucketNotFound => "BUCKET_NOT_FOUND",
    ErrorCode::QuotaExceeded => "QUOTA_EXCEEDED",
    ErrorCode::RemoteDatabaseError => "REMOTE_DATABASE_ERROR",
    ErrorCode::RemoteTimeout => "REMOTE_TIMEOUT",
    ErrorCode::Cancelled => "CANCELLED",
    ErrorCode::OutOfMemory => "OUT_OF_MEMORY",
    ErrorCode::CedFailed => "CED_FAILED",
    ErrorCode::CedRequestFailed => "CED_REQUEST_FAILED",
    ErrorCode::CedModelError => "CED_MODEL_ERROR",
    ErrorCode::Unknown(u) => return format!("UNKNOWN_{}", UnknownErrorCode::get(u)),
    // `ErrorCode` is `#[non_exhaustive]` — keep the wildcard for
    // forward-compat. Currently unreachable.
    #[allow(unreachable_patterns)]
    _ => "UNKNOWN",
  }
  .into()
}

// ---------------------------------------------------------------------------
// Rgba
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`Rgba`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GqlRgba(pub Rgba);

impl From<Rgba> for GqlRgba {
  #[inline]
  fn from(v: Rgba) -> Self {
    Self(v)
  }
}
impl From<GqlRgba> for Rgba {
  #[inline]
  fn from(v: GqlRgba) -> Self {
    v.0
  }
}

#[Object(name = "Rgba")]
impl GqlRgba {
  async fn r(&self) -> u8 {
    self.0.r()
  }
  async fn g(&self) -> u8 {
    self.0.g()
  }
  async fn b(&self) -> u8 {
    self.0.b()
  }
  async fn a(&self) -> u8 {
    self.0.a()
  }
  async fn bits(&self) -> u32 {
    self.0.bits()
  }
}

// ---------------------------------------------------------------------------
// Location — Union over the newtype variants of `Location<Uuid7>`.
// ---------------------------------------------------------------------------

/// GraphQL Object for [`LocalLocation<Uuid7>`].
#[derive(Debug, Clone)]
pub struct GqlLocalLocation(LocalLocation<Uuid7>);

impl GqlLocalLocation {
  /// Borrow the wrapped domain value.
  pub fn inner(&self) -> &LocalLocation<Uuid7> {
    &self.0
  }
}

#[Object(name = "LocalLocation")]
impl GqlLocalLocation {
  async fn volume(&self) -> Uuid7 {
    *self.0.volume_ref()
  }
  async fn components(&self) -> std::vec::Vec<String> {
    self.0.components_slice().iter().map(|s| s.to_string()).collect()
  }
}

/// GraphQL Union mirror of [`Location<Uuid7>`].
#[derive(Debug, Clone, Union)]
#[graphql(name = "Location")]
pub enum GqlLocation {
  Local(GqlLocalLocation),
}

impl From<Location<Uuid7>> for GqlLocation {
  fn from(v: Location<Uuid7>) -> Self {
    match v {
      Location::Local(inner) => Self::Local(GqlLocalLocation(inner)),
      // `Location` is `#[non_exhaustive]`; a new variant would need a
      // new GraphQL union arm too. Currently unreachable.
      #[allow(unreachable_patterns)]
      _ => unreachable!("Location is non_exhaustive but has only Local today"),
    }
  }
}

impl GqlLocation {
  /// Helper: borrow into the inner `LocalLocation` if this is a
  /// `Local` arm.
  pub fn as_local(&self) -> Option<&LocalLocation<Uuid7>> {
    match self {
      Self::Local(l) => Some(&l.0),
    }
  }
}

// ---------------------------------------------------------------------------
// MediaDevice
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`MediaDevice`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GqlMediaDevice(pub MediaDevice);

impl From<MediaDevice> for GqlMediaDevice {
  #[inline]
  fn from(v: MediaDevice) -> Self {
    Self(v)
  }
}
impl From<GqlMediaDevice> for MediaDevice {
  #[inline]
  fn from(v: GqlMediaDevice) -> Self {
    v.0
  }
}

#[Object(name = "MediaDevice")]
impl GqlMediaDevice {
  async fn make(&self) -> Option<String> {
    empty_as_none(self.0.make())
  }
  async fn model(&self) -> Option<String> {
    empty_as_none(self.0.model())
  }
  async fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
}

// ---------------------------------------------------------------------------
// MediaGeoLocation
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`MediaGeoLocation`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GqlMediaGeoLocation(pub MediaGeoLocation);

impl From<MediaGeoLocation> for GqlMediaGeoLocation {
  #[inline]
  fn from(v: MediaGeoLocation) -> Self {
    Self(v)
  }
}
impl From<GqlMediaGeoLocation> for MediaGeoLocation {
  #[inline]
  fn from(v: GqlMediaGeoLocation) -> Self {
    v.0
  }
}

#[Object(name = "MediaGeoLocation")]
impl GqlMediaGeoLocation {
  async fn lat(&self) -> f64 {
    self.0.lat()
  }
  async fn lon(&self) -> f64 {
    self.0.lon()
  }
  async fn altitude(&self) -> Option<f64> {
    self.0.altitude().map(f64::from)
  }
}

// ---------------------------------------------------------------------------
// Media
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`Media`].
#[derive(Debug, Clone)]
pub struct GqlMedia(pub Media<Uuid7>);

impl From<Media<Uuid7>> for GqlMedia {
  #[inline]
  fn from(v: Media<Uuid7>) -> Self {
    Self(v)
  }
}
impl From<GqlMedia> for Media<Uuid7> {
  #[inline]
  fn from(v: GqlMedia) -> Self {
    v.0
  }
}

#[Object(name = "Media")]
impl GqlMedia {
  async fn id(&self) -> ID {
    ID(self.0.id_ref().to_string())
  }
  async fn checksum(&self) -> FileChecksum {
    *self.0.checksum_ref()
  }
  /// Container format short name (`as_str()`); `null` when the
  /// `Other("")` absent sentinel.
  async fn format(&self) -> Option<String> {
    empty_as_none(self.0.format_ref().as_str())
  }
  async fn size(&self) -> String {
    self.0.size().to_string()
  }
  async fn duration(&self) -> Option<GqlMediaTimestamp> {
    self.0.duration_ref().copied().map(GqlMediaTimestamp)
  }
  async fn kind(&self) -> GqlMediaKind {
    self.0.kind().into()
  }
  async fn video(&self) -> Option<ID> {
    self.0.video_ref().map(|id| ID(id.to_string()))
  }
  async fn audio(&self) -> Option<ID> {
    self.0.audio_ref().map(|id| ID(id.to_string()))
  }
  async fn subtitle(&self) -> Option<ID> {
    self.0.subtitle_ref().map(|id| ID(id.to_string()))
  }
  async fn error_flags(&self) -> GqlMediaErrorFlags {
    self.0.error_flags().into()
  }
  async fn probe_error(&self) -> Option<GqlErrorInfo> {
    self.0.probe_error_ref().cloned().map(GqlErrorInfo)
  }
  async fn capture_date(&self) -> Option<GqlJiffTimestamp> {
    self.0.capture_date_ref().copied().map(GqlJiffTimestamp)
  }
  async fn device(&self) -> Option<GqlMediaDevice> {
    self.0.device_ref().cloned().map(GqlMediaDevice)
  }
  async fn gps(&self) -> Option<GqlMediaGeoLocation> {
    self.0.gps_ref().copied().map(GqlMediaGeoLocation)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::{MediaKind, Uuid7};
  use mediaframe::container::Format;

  #[test]
  fn provenance_wrapper_roundtrips() {
    let p = Provenance::from_parts("m", "v", "p", "i");
    let g: GqlProvenance = p.clone().into();
    let back: Provenance = g.into();
    assert_eq!(back, p);
  }

  #[test]
  fn location_union_roundtrips_local_variant() {
    let vol = Uuid7::new();
    let l = Location::try_local_uuid7(vol, ["Movies", "Holiday"]).unwrap();
    let g: GqlLocation = l.into();
    let local = g.as_local().expect("local variant");
    assert_eq!(local.volume_ref(), &vol);
    assert_eq!(local.components_slice(), &["Movies", "Holiday"]);
  }

  #[test]
  fn media_wrapper_round_trips_through_try_new() {
    let id = Uuid7::new();
    let cs = FileChecksum::from_bytes([0x11; 32]);
    let m = Media::try_new(id, cs, Format::Mp4, 1_234, MediaKind::Video).unwrap();
    let g: GqlMedia = m.clone().into();
    let back: Media<Uuid7> = g.into();
    assert_eq!(back.checksum_ref(), &cs);
  }

  #[test]
  fn error_code_name_renders_known_and_unknown() {
    assert_eq!(error_code_name(ErrorCode::ProbeCorrupt), "PROBE_CORRUPT");
    assert_eq!(
      error_code_name(ErrorCode::from_u32(99_999)),
      "UNKNOWN_99999"
    );
  }

  #[test]
  fn rgba_wrapper_roundtrips() {
    let c = Rgba::from_components(0x12, 0x34, 0x56, 0x78);
    let g: GqlRgba = c.into();
    let back: Rgba = g.into();
    assert_eq!(back, c);
  }
}
