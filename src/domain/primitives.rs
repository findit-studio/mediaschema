//! Foundational newtypes for the domain layer.
//!
//! All locked-schema aggregates are generic over an `Id` type parameter,
//! defaulting to [`Uuid7`] (a time-ordered UUIDv7 newtype). [`FileChecksum`]
//! is the 256-bit content hash, **distinct** from `Id` (content ≠ identity).
//! [`Location`] is the structured oneof copied from
//! `findit-proto::common::location` (volume-aware).

use core::{fmt, str::FromStr};

use smol_str::SmolStr;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Uuid7 — canonical concrete identity (UUIDv7, time-ordered)
// ---------------------------------------------------------------------------

/// Canonical concrete identity for mediaschema aggregates: a time-ordered
/// UUIDv7 (RFC 9562 §5.7), 16 bytes.
///
/// Locked identity model: aggregates are **generic over `Id`** (see
/// `schema/primitives.md` r5); `Uuid7` is the default concrete the schema
/// targets. Projection adapters may plug in backend-native types representing
/// the *same* 16 bytes (Postgres `uuid`, SQLite `BLOB(16)`, MySQL
/// `BINARY(16)`, MongoDB `_id` = `BinData` — never `ObjectId`).
///
/// Locked invariant: `Uuid7` is **distinct** from [`FileChecksum`] (content
/// hash is not identity).
///
/// ## Construction
///
/// Every *public* construction path is **validating**: [`Uuid7::new`]
/// (fresh v7), [`Uuid7::try_from_bytes`], `TryFrom<Uuid>`, and `FromStr` —
/// all of which reject nil and non-v7 values. There is no `Default` impl.
/// The nil sentinel (`Uuid7::nil`) and the unchecked byte ctor
/// (`Uuid7::from_bytes_unchecked`) are `pub(crate)`, reserved for the
/// generated proto adapter's wire round-trip; the locked schema expresses
/// "unassigned" via `Option<Uuid7>` at field boundaries, so external code
/// has no need for either escape. This makes the locked invariant —
/// non-nil + v7 — unreachable through the public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Uuid7(Uuid);

/// Error returned when a value fails the [`Uuid7`] invariants
/// (non-nil + UUIDv7 layout).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Uuid7Error {
  /// The input parsed as a UUID but its byte layout is the all-zero
  /// nil UUID — not a valid identity.
  Nil,
  /// The input parsed as a UUID but its version field is not `7`.
  NotV7(usize),
  /// The input could not be parsed as a UUID at all (string form).
  InvalidUuid(uuid::Error),
}

impl fmt::Display for Uuid7Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Nil => f.write_str("nil UUID is not a valid Uuid7 identity"),
      Self::NotV7(v) => write!(f, "expected UUIDv7, got UUIDv{v}"),
      Self::InvalidUuid(e) => write!(f, "invalid UUID: {e}"),
    }
  }
}

#[cfg(feature = "std")]
impl std::error::Error for Uuid7Error {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Self::InvalidUuid(e) => Some(e),
      _ => None,
    }
  }
}

impl From<uuid::Error> for Uuid7Error {
  fn from(e: uuid::Error) -> Self {
    Self::InvalidUuid(e)
  }
}

/// Validate that `u` is a non-nil UUIDv7. Single source of truth for the
/// invariant — every validating constructor funnels through here.
fn validate_v7(u: Uuid) -> Result<Uuid7, Uuid7Error> {
  if u.is_nil() {
    return Err(Uuid7Error::Nil);
  }
  // `Uuid::get_version_num()` returns the 4-bit version field directly,
  // which is what RFC 9562 §4.2 calls out as the version discriminant.
  let v = u.get_version_num();
  if v != 7 {
    return Err(Uuid7Error::NotV7(v));
  }
  Ok(Uuid7(u))
}

impl Uuid7 {
  /// Generate a new time-ordered UUIDv7 from the current wall-clock time.
  #[cfg(feature = "std")]
  #[inline]
  pub fn new() -> Self {
    Self(Uuid::now_v7())
  }

  /// The nil UUID (all-zero bits) — `pub(crate)` sentinel reserved for
  /// **wire-codec internals** (e.g. the generated proto adapter signalling
  /// an unset oneof during round-trip). The locked schema represents
  /// absence as `Option<Uuid7>` at every domain field boundary, so this is
  /// deliberately unreachable from outside this crate.
  ///
  /// Currently only the internal test suite exercises this; the
  /// `dead_code` allow is held for the upcoming proto adapter and the
  /// `Location::try_local_uuid7` nil-rejection regression test.
  #[inline]
  #[allow(dead_code)]
  pub(crate) const fn nil() -> Self {
    Self(Uuid::nil())
  }

  /// Is this the nil sentinel? Useful for the same wire-codec paths that
  /// produce the sentinel; domain code should not test for it.
  #[inline]
  #[allow(dead_code)]
  pub(crate) fn is_nil(&self) -> bool {
    self.0.is_nil()
  }

  /// Underlying `uuid::Uuid` (read-only; conversion back is `TryFrom`).
  #[inline]
  pub fn as_uuid(&self) -> Uuid {
    self.0
  }

  /// Raw 16-byte representation.
  #[inline]
  pub fn as_bytes(&self) -> &[u8; 16] {
    self.0.as_bytes()
  }

  /// Validating constructor from raw 16 bytes — rejects nil and any
  /// non-v7 layout.
  #[inline]
  pub fn try_from_bytes(bytes: [u8; 16]) -> Result<Self, Uuid7Error> {
    validate_v7(Uuid::from_bytes(bytes))
  }

  /// **Unchecked** construction from raw 16 bytes — `pub(crate)` and
  /// reserved for the proto round-trip path, where the producing side
  /// already validated the layout. External callers must use
  /// [`Uuid7::try_from_bytes`] (validating) or `TryFrom<Uuid>` /
  /// `FromStr` — making the unchecked path crate-private is what closes
  /// off "construct a nil/v4 Uuid7 from arbitrary bytes" from the public
  /// API.
  ///
  /// This is **safe** Rust (no memory-safety hazard), but it can violate
  /// the locked invariant — internal callers carry that obligation.
  ///
  /// Held with `#[allow(dead_code)]` for the upcoming proto adapter; the
  /// in-crate test suite exercises the round-trip semantics.
  #[inline]
  #[allow(dead_code)]
  pub(crate) const fn from_bytes_unchecked(bytes: [u8; 16]) -> Self {
    Self(Uuid::from_bytes(bytes))
  }
}

impl fmt::Display for Uuid7 {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Display::fmt(&self.0, f)
  }
}

impl FromStr for Uuid7 {
  type Err = Uuid7Error;

  /// Parse a UUID string and validate it as a non-nil UUIDv7.
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let u = Uuid::parse_str(s)?;
    validate_v7(u)
  }
}

impl TryFrom<Uuid> for Uuid7 {
  type Error = Uuid7Error;

  /// Convert a `uuid::Uuid` — rejects nil and non-v7 layouts.
  fn try_from(u: Uuid) -> Result<Self, Self::Error> {
    validate_v7(u)
  }
}

impl From<Uuid7> for Uuid {
  #[inline]
  fn from(id: Uuid7) -> Self {
    id.0
  }
}

// ---------------------------------------------------------------------------
// FileChecksum — 32-byte content hash, distinct newtype from Id
// ---------------------------------------------------------------------------

/// 256-bit content hash identifying a file by its contents.
///
/// Locked rule (`schema/primitives.md` r5): content hash **is not identity**.
/// `FileChecksum` is the **unique index** across `Media`, *never* the primary
/// key, and a **distinct** newtype — never interchangeable with [`Uuid7`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct FileChecksum(pub [u8; 32]);

impl FileChecksum {
  /// Wrap a 32-byte hash.
  #[inline]
  pub const fn new(bytes: [u8; 32]) -> Self {
    Self(bytes)
  }

  /// All-zero sentinel for "not yet computed".
  #[inline]
  pub const fn zero() -> Self {
    Self([0; 32])
  }

  /// Raw bytes.
  #[inline]
  pub const fn as_bytes(&self) -> &[u8; 32] {
    &self.0
  }

  /// Is this the all-zero sentinel?
  #[inline]
  pub fn is_zero(&self) -> bool {
    self.0 == [0; 32]
  }
}

impl fmt::Display for FileChecksum {
  /// Lower-case hex (64 chars).
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for b in &self.0 {
      write!(f, "{:02x}", b)?;
    }
    Ok(())
  }
}

impl From<[u8; 32]> for FileChecksum {
  #[inline]
  fn from(bytes: [u8; 32]) -> Self {
    Self(bytes)
  }
}

// ---------------------------------------------------------------------------
// Rgba — packed RGBA colour
// ---------------------------------------------------------------------------

/// Packed 32-bit RGBA colour (`0xRRGGBBAA`).
///
/// Used by curation `UserTag.color` (smart-folder layer) and by colorthief
/// `DominantColor.rgb` on `Keyframe`. Cheap-unambiguous redesign of the
/// proto `Tag.color: u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgba(pub u32);

impl Rgba {
  /// Pack from RGBA components.
  #[inline]
  pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
    Self(((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32))
  }

  #[inline]
  pub const fn r(self) -> u8 {
    (self.0 >> 24) as u8
  }
  #[inline]
  pub const fn g(self) -> u8 {
    (self.0 >> 16) as u8
  }
  #[inline]
  pub const fn b(self) -> u8 {
    (self.0 >> 8) as u8
  }
  #[inline]
  pub const fn a(self) -> u8 {
    self.0 as u8
  }
}

// ---------------------------------------------------------------------------
// Location — structured oneof (copied from findit-proto::common::location)
// ---------------------------------------------------------------------------

/// Error returned when a [`Location`] cannot be constructed because the
/// payload violates a real-file invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LocationError {
  /// The path-components slice was empty (a volume root with no path
  /// segments is not a file location).
  EmptyPath,
  /// The supplied volume id was the [`Uuid7`] nil sentinel — only valid
  /// for the wire-codec unset path, not for a real local location.
  NilVolume,
}

impl fmt::Display for LocationError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::EmptyPath => f.write_str("Location::Local requires a non-empty path"),
      Self::NilVolume => f.write_str("Location::Local requires a non-nil volume id"),
    }
  }
}

#[cfg(feature = "std")]
impl std::error::Error for LocationError {}

/// File location — a structured oneof, **not** an opaque path string.
///
/// Copied verbatim from `findit-proto::common::location` per locked
/// `schema/primitives.md` r5: a local path is volume-id-relative so removable
/// drives keep one stable identity across eject/replug + mount-point changes.
/// Object-storage support (`Object { store, bucket, key }` + a
/// `StorageProvider` aggregate) is deferred until a real S3/R2 consumer
/// appears (the `mediaframe::Language`-style boundary applies —
/// `#[non_exhaustive]` keeps it forward-compatible).
///
/// ## Construction
///
/// The `Local` variant is itself `#[non_exhaustive]`, so external callers
/// cannot use the `Location::Local { volume, components }` literal syntax.
/// The only public path is [`Location::try_local`], which validates the
/// payload and returns `Result<Self, LocationError>` — empty path is
/// always rejected, and for the [`Location<Uuid7>`] specialization, a nil
/// volume id is also rejected. Combined with `Uuid7::nil()` being
/// `pub(crate)`, this makes "construct a fake local location"
/// unreachable from outside this crate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Location<Id = Uuid7> {
  /// A local volume path: stable `volume` identity + platform-agnostic
  /// path components. Identity is the volume's stable UUID (the old
  /// indexer writes it once to `<mount>/.findit_index/.id`), **not** the
  /// OS mount path (which is volatile across remounts).
  #[non_exhaustive]
  Local {
    volume: Id,
    components: Vec<SmolStr>,
  },
  // Future: `RemoteUrl(SmolStr)`, `Object { store, bucket, key }` —
  // surfaced when a real consumer needs object-storage roots; reopens
  // this `Location` enum at that time.
}

impl<Id> Location<Id> {
  /// **Internal** infallible builder — bypasses validation. Used by the
  /// validating `try_local` paths after they've checked invariants, and
  /// (eventually) by the wire-codec adapter when round-tripping a
  /// pre-validated oneof. External callers must go through
  /// [`Location::try_local`].
  ///
  /// Held with `#[allow(dead_code)]` for the upcoming proto adapter.
  #[inline]
  #[allow(dead_code)]
  pub(crate) fn local_unchecked<I, S>(volume: Id, components: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<SmolStr>,
  {
    Self::Local {
      volume,
      components: components.into_iter().map(Into::into).collect(),
    }
  }
}

impl<Id> Location<Id> {
  /// Generic validating builder: requires a non-empty path.
  ///
  /// (Volume-nil validation is type-dependent; see the
  /// [`Location<Uuid7>`] specialization for the `Uuid7` case.)
  pub fn try_local<I, S>(volume: Id, components: I) -> Result<Self, LocationError>
  where
    I: IntoIterator<Item = S>,
    S: Into<SmolStr>,
  {
    let components: Vec<SmolStr> = components.into_iter().map(Into::into).collect();
    if components.is_empty() {
      return Err(LocationError::EmptyPath);
    }
    Ok(Self::Local { volume, components })
  }
}

impl Location<Uuid7> {
  /// `Uuid7`-specialized validating builder: also rejects a nil volume id
  /// (the wire-codec sentinel is `pub(crate)`, so external callers can't
  /// produce one anyway — this is belt-and-braces).
  pub fn try_local_uuid7<I, S>(volume: Uuid7, components: I) -> Result<Self, LocationError>
  where
    I: IntoIterator<Item = S>,
    S: Into<SmolStr>,
  {
    if volume.is_nil() {
      return Err(LocationError::NilVolume);
    }
    let components: Vec<SmolStr> = components.into_iter().map(Into::into).collect();
    if components.is_empty() {
      return Err(LocationError::EmptyPath);
    }
    Ok(Self::Local { volume, components })
  }
}

// Note: `Location` deliberately does **not** implement `Default`. The wire
// shape is a `oneof` whose absence is "no arm at all", and the only existing
// arm (`Local`) cannot be defaulted without fabricating a nil volume id and
// an empty path — neither of which represents a real file. Domain fields
// that may be absent express that with `Option<Location>`; wire conversion
// returns `None` for a no-arm value.

// ---------------------------------------------------------------------------
// ErrorCode — structured error vocabulary (verified vs findit-proto)
// ---------------------------------------------------------------------------

/// Opaque payload of [`ErrorCode::Unknown`]. The inner `u32` is
/// `pub(crate)` so external callers cannot construct
/// `ErrorCode::Unknown(404)` directly — the *only* way to land in
/// `Unknown` is [`ErrorCode::from_u32`] with a value outside the named
/// catalog. This keeps `is_unknown()` consistent with `Unknown ⇔ not
/// canonicalisable`, and prevents
/// `from_u32(c.as_u32()) != c` ambiguities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnknownErrorCode(pub(crate) u32);

impl UnknownErrorCode {
  /// The wire `u32` this `Unknown` carries.
  #[inline]
  pub const fn get(self) -> u32 {
    self.0
  }
}

impl fmt::Display for UnknownErrorCode {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Unknown({})", self.0)
  }
}

/// Stable error-code vocabulary used inside [`ErrorInfo`].
///
/// HTTP-style codes (400–599) for general protocol errors; domain-specific
/// codes (1000+) for indexing-pipeline errors. **Verified** against
/// `findit-proto::common::error_info::ErrorCode` (the values are
/// wire-stable). `#[non_exhaustive]` — new codes may be added without
/// breaking domain consumers.
///
/// `Unknown(UnknownErrorCode)` preserves any wire code not yet enumerated
/// here so a future producer's stage signal survives an older
/// mediaschema's wire→domain→wire round-trip lossless. The payload type
/// has a `pub(crate)` field, so callers cannot fabricate
/// `Unknown(404)` — the *only* path to construct `Unknown` is
/// [`ErrorCode::from_u32`], which routes recognised codes to their named
/// variant first. That guarantees: `from_u32(c.as_u32()) == c` for every
/// publicly constructible `ErrorCode`, and `is_unknown()` is a stable
/// statement about the code, not an artifact of how it was built.
///
/// The domain error model uses `code` as the *stage-coded* signal in
/// `index_errors: Vec<ErrorInfo>`; this replaces the dropped per-track
/// `error_status` bitflags (error-state is now **derived** from the
/// stage codes + `index_status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCode {
  // --- HTTP-style protocol errors ---
  BadRequest,
  PermissionDenied,
  NotFound,
  AlreadyExists,
  UnprocessableEntity,
  InternalError,
  ServiceUnavailable,
  Timeout,

  // --- Probe (1000s) ---
  ProbeCorrupt,
  ProbeUnsupportedFormat,
  ProbeNoVideoStream,
  ProbeNoAudioStream,

  // --- Scene detection (2000s) ---
  SceneDetectionFailed,
  SceneDetectionModelError,

  // --- Transcription (3000s) ---
  TranscriptionFailed,
  TranscriptionModelError,

  // --- VLM (4000s) ---
  VlmFailed,
  VlmModelError,

  // --- Apple Vision (5000s) ---
  AppleVisionFailed,
  AppleVisionRequestFailed,

  // --- Embedding (6000s) ---
  EmbeddingFailed,
  EmbeddingModelError,
  EmbeddingModelLoadFailed,
  EmbeddingPreprocessFailed,
  EmbeddingInferenceFailed,
  EmbeddingOutputInvalid,

  // --- Path / volume (7000s) ---
  PathNotFound,
  VolumeNotAvailable,
  MissingVolumeId,
  MalformedVolumeId,
  VolumeIdMismatch,
  LocalDatabaseError,
  FolderNotAvailable,
  LocalPermissionDenied,

  // --- Remote (8000s) ---
  EndpointUnreachable,
  AuthenticationFailed,
  BucketNotFound,
  QuotaExceeded,
  RemoteDatabaseError,
  RemoteTimeout,

  // --- Cancelled / resource (9000s) ---
  Cancelled,
  OutOfMemory,

  // --- CED sound-event detection (10000s) ---
  CedFailed,
  CedRequestFailed,
  CedModelError,

  /// A wire code not enumerated above — preserved verbatim so a wire
  /// → domain → wire round-trip retains the producer's exact stage
  /// signal across version skew. The inner payload is opaque; the only
  /// way to land here is [`ErrorCode::from_u32`] with an unrecognised
  /// value.
  Unknown(UnknownErrorCode),
}

impl ErrorCode {
  /// Numeric wire value (matches `findit-proto::common::error_info::ErrorCode`).
  /// Always lossless: known variants emit their stable HTTP-style /
  /// domain-bucket code; `Unknown(n)` round-trips `n` verbatim.
  pub const fn as_u32(self) -> u32 {
    match self {
      // --- HTTP-style protocol errors ---
      Self::BadRequest => 400,
      Self::PermissionDenied => 403,
      Self::NotFound => 404,
      Self::AlreadyExists => 409,
      Self::UnprocessableEntity => 422,
      Self::InternalError => 500,
      Self::ServiceUnavailable => 503,
      Self::Timeout => 504,
      // --- Probe (1000s) ---
      Self::ProbeCorrupt => 1000,
      Self::ProbeUnsupportedFormat => 1001,
      Self::ProbeNoVideoStream => 1002,
      Self::ProbeNoAudioStream => 1003,
      // --- Scene detection (2000s) ---
      Self::SceneDetectionFailed => 2000,
      Self::SceneDetectionModelError => 2001,
      // --- Transcription (3000s) ---
      Self::TranscriptionFailed => 3000,
      Self::TranscriptionModelError => 3001,
      // --- VLM (4000s) ---
      Self::VlmFailed => 4000,
      Self::VlmModelError => 4001,
      // --- Apple Vision (5000s) ---
      Self::AppleVisionFailed => 5000,
      Self::AppleVisionRequestFailed => 5001,
      // --- Embedding (6000s) ---
      Self::EmbeddingFailed => 6000,
      Self::EmbeddingModelError => 6001,
      Self::EmbeddingModelLoadFailed => 6002,
      Self::EmbeddingPreprocessFailed => 6003,
      Self::EmbeddingInferenceFailed => 6004,
      Self::EmbeddingOutputInvalid => 6005,
      // --- Path / volume (7000s) ---
      Self::PathNotFound => 7000,
      Self::VolumeNotAvailable => 7001,
      Self::MissingVolumeId => 7002,
      Self::MalformedVolumeId => 7003,
      Self::VolumeIdMismatch => 7004,
      Self::LocalDatabaseError => 7005,
      Self::FolderNotAvailable => 7006,
      Self::LocalPermissionDenied => 7007,
      // --- Remote (8000s) ---
      Self::EndpointUnreachable => 8000,
      Self::AuthenticationFailed => 8001,
      Self::BucketNotFound => 8002,
      Self::QuotaExceeded => 8003,
      Self::RemoteDatabaseError => 8004,
      Self::RemoteTimeout => 8005,
      // --- Cancelled / resource (9000s) ---
      Self::Cancelled => 9000,
      Self::OutOfMemory => 9001,
      // --- CED (10000s) ---
      Self::CedFailed => 10000,
      Self::CedRequestFailed => 10001,
      Self::CedModelError => 10002,
      // --- Wire-preserved escape ---
      Self::Unknown(u) => u.get(),
    }
  }

  /// Convert a wire `u32` into an [`ErrorCode`]. Unknown values land in
  /// `Self::Unknown(n)` — never lossy, infallible.
  pub const fn from_u32(n: u32) -> Self {
    match n {
      // --- HTTP-style protocol errors ---
      400 => Self::BadRequest,
      403 => Self::PermissionDenied,
      404 => Self::NotFound,
      409 => Self::AlreadyExists,
      422 => Self::UnprocessableEntity,
      500 => Self::InternalError,
      503 => Self::ServiceUnavailable,
      504 => Self::Timeout,
      // --- Probe (1000s) ---
      1000 => Self::ProbeCorrupt,
      1001 => Self::ProbeUnsupportedFormat,
      1002 => Self::ProbeNoVideoStream,
      1003 => Self::ProbeNoAudioStream,
      // --- Scene detection (2000s) ---
      2000 => Self::SceneDetectionFailed,
      2001 => Self::SceneDetectionModelError,
      // --- Transcription (3000s) ---
      3000 => Self::TranscriptionFailed,
      3001 => Self::TranscriptionModelError,
      // --- VLM (4000s) ---
      4000 => Self::VlmFailed,
      4001 => Self::VlmModelError,
      // --- Apple Vision (5000s) ---
      5000 => Self::AppleVisionFailed,
      5001 => Self::AppleVisionRequestFailed,
      // --- Embedding (6000s) ---
      6000 => Self::EmbeddingFailed,
      6001 => Self::EmbeddingModelError,
      6002 => Self::EmbeddingModelLoadFailed,
      6003 => Self::EmbeddingPreprocessFailed,
      6004 => Self::EmbeddingInferenceFailed,
      6005 => Self::EmbeddingOutputInvalid,
      // --- Path / volume (7000s) ---
      7000 => Self::PathNotFound,
      7001 => Self::VolumeNotAvailable,
      7002 => Self::MissingVolumeId,
      7003 => Self::MalformedVolumeId,
      7004 => Self::VolumeIdMismatch,
      7005 => Self::LocalDatabaseError,
      7006 => Self::FolderNotAvailable,
      7007 => Self::LocalPermissionDenied,
      // --- Remote (8000s) ---
      8000 => Self::EndpointUnreachable,
      8001 => Self::AuthenticationFailed,
      8002 => Self::BucketNotFound,
      8003 => Self::QuotaExceeded,
      8004 => Self::RemoteDatabaseError,
      8005 => Self::RemoteTimeout,
      // --- Cancelled / resource (9000s) ---
      9000 => Self::Cancelled,
      9001 => Self::OutOfMemory,
      // --- CED (10000s) ---
      10000 => Self::CedFailed,
      10001 => Self::CedRequestFailed,
      10002 => Self::CedModelError,
      // Wire value not enumerated — preserve it. `UnknownErrorCode` has
      // a `pub(crate)` payload, so this is the only path that can land
      // in `Unknown`.
      other => Self::Unknown(UnknownErrorCode(other)),
    }
  }

  /// Is this an `Unknown(_)` wire-preserved code (i.e. a producer used a
  /// code newer than this `ErrorCode` enum knows about)?
  pub const fn is_unknown(self) -> bool {
    matches!(self, Self::Unknown(_))
  }
}

// Deliberately **no** `Default for ErrorCode`. There is no meaningful
// default error — every error has a specific stage signal. `ErrorInfo`
// must be constructed with an explicit code via `ErrorInfo::new` /
// `ErrorInfo::code_only`.

// ---------------------------------------------------------------------------
// ErrorInfo — verified vs findit-proto::common::error_info
// ---------------------------------------------------------------------------

/// Reusable error detail used across domain records.
///
/// Verified shape (`findit-proto::common::error_info`): `{ code: ErrorCode,
/// message: SmolStr }`. The `code` is the stage-coded id; `message` is the
/// human-readable description (`""`=absent — no `Option` for strings, per
/// the locked rule).
///
/// No `Default` impl — there is no meaningful "default error". Construct
/// explicitly via [`ErrorInfo::new`] or [`ErrorInfo::code_only`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ErrorInfo {
  pub code: ErrorCode,
  pub message: SmolStr,
}

impl ErrorInfo {
  /// Construct an `ErrorInfo` with the given code and message.
  #[inline]
  pub fn new(code: ErrorCode, message: impl Into<SmolStr>) -> Self {
    Self {
      code,
      message: message.into(),
    }
  }

  /// Construct with just a code (empty message).
  #[inline]
  pub fn code_only(code: ErrorCode) -> Self {
    Self {
      code,
      message: SmolStr::default(),
    }
  }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn uuid7_new_is_non_nil_and_unique() {
    let a = Uuid7::new();
    let b = Uuid7::new();
    assert!(!a.is_nil());
    assert!(!b.is_nil());
    assert_ne!(a, b, "two fresh UUIDv7s must differ");
  }

  #[test]
  fn uuid7_nil_sentinel_must_be_explicit() {
    // `Default` is intentionally not derived, so this is the only way
    // to spell the nil sentinel — and it requires a deliberate call.
    let n = Uuid7::nil();
    assert!(n.is_nil());
    assert_eq!(n.as_bytes(), &[0; 16]);
  }

  #[test]
  fn uuid7_string_roundtrip() {
    let a = Uuid7::new();
    let s = a.to_string();
    let b: Uuid7 = s.parse().expect("parse roundtrip");
    assert_eq!(a, b);
  }

  #[test]
  fn uuid7_rejects_nil_via_validating_constructors() {
    // try_from_bytes
    assert_eq!(Uuid7::try_from_bytes([0; 16]), Err(Uuid7Error::Nil));
    // TryFrom<Uuid>
    assert_eq!(Uuid7::try_from(uuid::Uuid::nil()), Err(Uuid7Error::Nil));
    // FromStr
    assert_eq!(
      "00000000-0000-0000-0000-000000000000".parse::<Uuid7>(),
      Err(Uuid7Error::Nil)
    );
  }

  #[test]
  fn uuid7_rejects_non_v7_via_validating_constructors() {
    // Hand-craft a non-nil v4-layout UUID: byte 6 high nibble = 4
    // (RFC 9562 §4.2 version field), byte 8 high bits = 10 (variant).
    let mut bytes = [0u8; 16];
    bytes[0] = 0x01; // make sure it isn't nil
    bytes[6] = 0x4a; // version nibble = 4
    bytes[8] = 0x80; // variant = RFC 4122
    let v4 = uuid::Uuid::from_bytes(bytes);
    assert_eq!(v4.get_version_num(), 4);
    let v4_str = v4.to_string();
    match Uuid7::try_from(v4) {
      Err(Uuid7Error::NotV7(4)) => {}
      other => panic!("expected NotV7(4), got {other:?}"),
    }
    match v4_str.parse::<Uuid7>() {
      Err(Uuid7Error::NotV7(4)) => {}
      other => panic!("expected NotV7(4), got {other:?}"),
    }
  }

  #[test]
  fn uuid7_invalid_string_returns_uuid_error() {
    match "not-a-uuid".parse::<Uuid7>() {
      Err(Uuid7Error::InvalidUuid(_)) => {}
      other => panic!("expected InvalidUuid, got {other:?}"),
    }
  }

  #[test]
  fn uuid7_from_bytes_unchecked_still_works_for_wire_roundtrip() {
    // The escape hatch must remain available for wire codecs — the
    // caller asserts the layout. Confirm it produces a usable v7.
    let a = Uuid7::new();
    let raw = *a.as_bytes();
    let b = Uuid7::from_bytes_unchecked(raw);
    assert_eq!(a, b);
  }

  #[test]
  fn uuid7_distinct_from_filechecksum_type() {
    // Compile-time guard: distinct newtypes (a poor stand-in is to check
    // sizes / no implicit conversion exists; we rely on the type system).
    let _u: Uuid7 = Uuid7::nil();
    let _c: FileChecksum = FileChecksum::zero();
    // (no `From<Uuid7> for FileChecksum` exists — that's the invariant)
  }

  #[test]
  fn filechecksum_hex_display() {
    let bytes = [0xde, 0xad, 0xbe, 0xef]
      .into_iter()
      .cycle()
      .take(32)
      .collect::<Vec<u8>>()
      .try_into()
      .unwrap();
    let cs = FileChecksum::new(bytes);
    let s = cs.to_string();
    assert_eq!(s.len(), 64);
    assert!(s.starts_with("deadbeef"));
    assert!(s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
  }

  #[test]
  fn filechecksum_zero_sentinel() {
    let z = FileChecksum::zero();
    assert!(z.is_zero());
    assert_eq!(z.to_string(), "0".repeat(64));
  }

  #[test]
  fn rgba_pack_unpack() {
    let c = Rgba::new(0x12, 0x34, 0x56, 0x78);
    assert_eq!(c.0, 0x12_34_56_78);
    assert_eq!(c.r(), 0x12);
    assert_eq!(c.g(), 0x34);
    assert_eq!(c.b(), 0x56);
    assert_eq!(c.a(), 0x78);
  }

  // (Previously `location_local_default` — removed alongside the
  // `impl Default for Location`. Absence is now expressed by
  // `Option<Location>` at field boundaries; this is exercised by the
  // `location_local_builder` test below and at every consumer site.)

  #[test]
  fn location_try_local_uuid7_happy_path() {
    let vol = Uuid7::new();
    let l = Location::try_local_uuid7(vol, ["Movies", "Holiday"]).unwrap();
    match l {
      Location::Local { volume, components } => {
        assert_eq!(volume, vol);
        assert_eq!(components, vec!["Movies", "Holiday"]);
      }
    }
  }

  #[test]
  fn location_try_local_uuid7_rejects_nil_volume() {
    // `Uuid7::nil()` is `pub(crate)`; this case can only be reached
    // inside the crate, which is exactly what the validation guards.
    let r = Location::try_local_uuid7(Uuid7::nil(), ["Movies"]);
    assert_eq!(r, Err(LocationError::NilVolume));
  }

  #[test]
  fn location_try_local_uuid7_rejects_empty_path() {
    let vol = Uuid7::new();
    let r = Location::try_local_uuid7::<core::iter::Empty<&str>, &str>(vol, core::iter::empty());
    assert_eq!(r, Err(LocationError::EmptyPath));
  }

  #[test]
  fn location_generic_try_local_rejects_empty_path() {
    // For non-Uuid7 Id types the generic try_local still checks the
    // path; volume-nil is the caller's responsibility.
    let r = Location::<u32>::try_local::<core::iter::Empty<&str>, &str>(7, core::iter::empty());
    assert_eq!(r, Err(LocationError::EmptyPath));
  }

  #[test]
  fn error_code_unknown_round_trips_wire_value() {
    // The Unknown(_) variant preserves any code we don't enumerate,
    // so wire→domain→wire is lossless across version skew.
    for wire in [42_u32, 1234, 99_999, u32::MAX] {
      let c = ErrorCode::from_u32(wire);
      assert!(c.is_unknown(), "{wire} should land in Unknown");
      assert_eq!(c.as_u32(), wire, "wire->domain->wire must be identity");
    }
  }

  #[test]
  fn error_code_from_u32_normalises_known_codes() {
    // Regression for Codex round-2 ambiguity worry: a known wire value
    // (e.g. 404) must canonicalise to its named variant, never to
    // Unknown(404). Combined with `UnknownErrorCode` having a
    // `pub(crate)` payload, this means there is no public path that
    // produces `Unknown(404)` — the round-trip is identity for every
    // publicly constructible `ErrorCode`.
    let canonical = ErrorCode::from_u32(404);
    assert_eq!(canonical, ErrorCode::NotFound);
    assert!(!canonical.is_unknown());

    // And the identity round-trip:
    for c in [
      ErrorCode::BadRequest,
      ErrorCode::NotFound,
      ErrorCode::ProbeCorrupt,
      ErrorCode::from_u32(99_999), // genuine Unknown
    ] {
      assert_eq!(ErrorCode::from_u32(c.as_u32()), c);
    }
  }

  #[test]
  fn error_code_known_round_trips_canonical_value() {
    // Sample one code per bucket — every named variant must survive
    // an as_u32() ↔ from_u32() round-trip.
    for c in [
      ErrorCode::BadRequest,
      ErrorCode::NotFound,
      ErrorCode::InternalError,
      ErrorCode::ProbeCorrupt,
      ErrorCode::SceneDetectionFailed,
      ErrorCode::TranscriptionFailed,
      ErrorCode::EmbeddingOutputInvalid,
      ErrorCode::LocalPermissionDenied,
      ErrorCode::EndpointUnreachable,
      ErrorCode::Cancelled,
      ErrorCode::CedModelError,
    ] {
      let n = c.as_u32();
      let back = ErrorCode::from_u32(n);
      assert_eq!(c, back, "{c:?} did not survive u32 round-trip");
      assert!(!back.is_unknown());
    }
  }

  #[test]
  fn error_code_discriminants_match_findit_proto() {
    // Spot-check the verified-vs-findit-proto values across each group.
    assert_eq!(ErrorCode::BadRequest.as_u32(), 400);
    assert_eq!(ErrorCode::NotFound.as_u32(), 404);
    assert_eq!(ErrorCode::Timeout.as_u32(), 504);
    assert_eq!(ErrorCode::ProbeCorrupt.as_u32(), 1000);
    assert_eq!(ErrorCode::ProbeUnsupportedFormat.as_u32(), 1001);
    assert_eq!(ErrorCode::SceneDetectionFailed.as_u32(), 2000);
    assert_eq!(ErrorCode::TranscriptionFailed.as_u32(), 3000);
    assert_eq!(ErrorCode::VlmFailed.as_u32(), 4000);
    assert_eq!(ErrorCode::AppleVisionFailed.as_u32(), 5000);
    assert_eq!(ErrorCode::EmbeddingFailed.as_u32(), 6000);
    assert_eq!(ErrorCode::EmbeddingOutputInvalid.as_u32(), 6005);
    assert_eq!(ErrorCode::PathNotFound.as_u32(), 7000);
    assert_eq!(ErrorCode::LocalPermissionDenied.as_u32(), 7007);
    assert_eq!(ErrorCode::EndpointUnreachable.as_u32(), 8000);
    assert_eq!(ErrorCode::Cancelled.as_u32(), 9000);
    assert_eq!(ErrorCode::OutOfMemory.as_u32(), 9001);
    assert_eq!(ErrorCode::CedFailed.as_u32(), 10000);
    assert_eq!(ErrorCode::CedModelError.as_u32(), 10002);
  }

  #[test]
  fn error_info_construction() {
    let e = ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad container header");
    assert_eq!(e.code, ErrorCode::ProbeCorrupt);
    assert_eq!(e.message.as_str(), "bad container header");

    let e2 = ErrorInfo::code_only(ErrorCode::Cancelled);
    assert_eq!(e2.code, ErrorCode::Cancelled);
    assert!(e2.message.is_empty());
  }
}
