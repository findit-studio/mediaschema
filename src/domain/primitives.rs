//! Foundational newtypes for the domain layer.
//!
//! All locked-schema aggregates are generic over an `Id` type parameter,
//! defaulting to [`Uuid7`] (a time-ordered UUIDv7 newtype). [`FileChecksum`]
//! is the 256-bit content hash, **distinct** from `Id` (content ≠ identity).
//! [`Location`] is the structured oneof copied from
//! `findit-proto::common::location` (volume-aware).
//!
//! ## Encapsulation rules (apply across `domain/*`)
//!
//! - **No public fields.** Access goes through `field()` getters and
//!   `with_field(...)` / `set_field(...)` builders / mutators. Builders are
//!   `const fn` where possible.
//! - **No structure variants in enums.** Extracted to a named struct and
//!   wrapped in a newtype variant (`Local(LocalLocation<Id>)` not
//!   `Local { volume, components }`).
//! - **Enum derives.** Unit-only enums get [`derive_more::IsVariant`].
//!   Enums with a newtype variant additionally get
//!   `derive_more::{Unwrap, TryUnwrap}` with `#[unwrap(ref, ref_mut)]
//!   #[try_unwrap(ref, ref_mut)]` so accessors flow as `unwrap_*` /
//!   `unwrap_*_ref` / `unwrap_*_ref_mut` / `try_unwrap_*` variants.
//! - **`core::error::Error`**, not `std::error::Error` — stable since
//!   1.81, MSRV is 1.85.

use core::{fmt, str::FromStr};

use derive_more::{IsVariant, TryUnwrap, Unwrap};
#[cfg(any(feature = "std", feature = "alloc"))]
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
///
/// Unit + two newtype variants, so derives both [`IsVariant`] and the
/// `Unwrap` / `TryUnwrap` accessor families with shared-ref + mut-ref
/// flavours per the encapsulation rules.
#[derive(Debug, Clone, PartialEq, Eq, IsVariant, Unwrap, TryUnwrap, thiserror::Error)]
#[unwrap(ref, ref_mut)]
#[try_unwrap(ref, ref_mut)]
#[non_exhaustive]
pub enum Uuid7Error {
  /// The input parsed as a UUID but its byte layout is the all-zero
  /// nil UUID — not a valid identity.
  #[error("nil UUID is not a valid Uuid7 identity")]
  Nil,
  /// The input parsed as a UUID but its version field is not `7`. The
  /// payload is the actual version nibble.
  ///
  /// (Variant intentionally not named `NotV7(usize)` — `derive_more`
  /// snake-cases it through a digit boundary as `not_v_7`, which the
  /// `Unwrap`/`TryUnwrap`/`IsVariant` accessors then carry verbatim
  /// (`is_not_v_7`, `try_unwrap_not_v_7_ref`, …). `WrongVersion` gives
  /// the cleaner accessor names.)
  #[error("expected UUIDv7, got UUIDv{0}")]
  WrongVersion(usize),
  /// The input could not be parsed as a UUID at all (string form).
  #[error("invalid UUID: {0}")]
  InvalidUuid(#[from] uuid::Error),
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
    return Err(Uuid7Error::WrongVersion(v));
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
  pub const fn as_uuid(&self) -> Uuid {
    self.0
  }

  /// Raw 16-byte representation.
  #[inline]
  pub const fn as_bytes(&self) -> &[u8; 16] {
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
/// Inner bytes are private; access via [`FileChecksum::as_bytes`].
///
/// **Default convention**: `Default::default()` calls
/// [`FileChecksum::new`], which returns the all-zero sentinel
/// ("not yet computed"). Use [`FileChecksum::from_bytes`] (or the
/// `From<[u8; 32]>` impl) for an actual hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileChecksum([u8; 32]);

impl FileChecksum {
  /// All-zero sentinel for "not yet computed". The canonical no-arg
  /// constructor — [`Default::default`] is `Self::new()`.
  #[inline]
  pub const fn new() -> Self {
    Self([0; 32])
  }

  /// Wrap a 32-byte hash.
  #[inline]
  pub const fn from_bytes(bytes: [u8; 32]) -> Self {
    Self(bytes)
  }

  /// Raw bytes.
  #[inline]
  pub const fn as_bytes(&self) -> &[u8; 32] {
    &self.0
  }

  /// Is this the all-zero "not yet computed" sentinel?
  #[inline]
  pub fn is_zero(&self) -> bool {
    self.0 == [0; 32]
  }
}

impl Default for FileChecksum {
  #[inline]
  fn default() -> Self {
    Self::new()
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
/// proto `Tag.color: u32`. Inner `u32` is private; access component bytes
/// via [`Rgba::r`] / [`Rgba::g`] / [`Rgba::b`] / [`Rgba::a`] or the raw
/// pack via [`Rgba::bits`].
///
/// **Default convention**: `Default::default()` calls [`Rgba::new`],
/// which returns transparent black (`0x00000000`). Use
/// [`Rgba::from_components`] (or [`Rgba::from_bits`]) to construct a
/// specific colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba(u32);

impl Rgba {
  /// Transparent black (`0x00000000`). The canonical no-arg constructor —
  /// [`Default::default`] is `Self::new()`.
  #[inline]
  pub const fn new() -> Self {
    Self(0)
  }

  /// Pack from RGBA components.
  #[inline]
  pub const fn from_components(r: u8, g: u8, b: u8, a: u8) -> Self {
    Self(((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32))
  }

  /// Construct from a pre-packed `0xRRGGBBAA` value.
  #[inline]
  pub const fn from_bits(bits: u32) -> Self {
    Self(bits)
  }

  /// Raw packed `0xRRGGBBAA` value.
  #[inline]
  pub const fn bits(self) -> u32 {
    self.0
  }

  /// Red component.
  #[inline]
  pub const fn r(self) -> u8 {
    (self.0 >> 24) as u8
  }
  /// Green component.
  #[inline]
  pub const fn g(self) -> u8 {
    (self.0 >> 16) as u8
  }
  /// Blue component.
  #[inline]
  pub const fn b(self) -> u8 {
    (self.0 >> 8) as u8
  }
  /// Alpha component.
  #[inline]
  pub const fn a(self) -> u8 {
    self.0 as u8
  }
}

impl Default for Rgba {
  #[inline]
  fn default() -> Self {
    Self::new()
  }
}

// ---------------------------------------------------------------------------
// Location — structured oneof (copied from findit-proto::common::location)
// ---------------------------------------------------------------------------

/// Error returned when a [`Location`] cannot be constructed because the
/// payload violates a real-file invariant.
///
/// Unit-only enum → derives [`IsVariant`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum LocationError {
  /// The path-components slice was empty (a volume root with no path
  /// segments is not a file location).
  #[error("Location::Local requires a non-empty path")]
  EmptyPath,
  /// The supplied volume id was the [`Uuid7`] nil sentinel — only valid
  /// for the wire-codec unset path, not for a real local location.
  #[error("Location::Local requires a non-nil volume id")]
  NilVolume,
}

/// Payload of [`Location::Local`] — extracted to a named struct per the
/// no-structure-variants rule. Fields are private; the only public
/// construction paths are [`Location::try_local`] /
/// [`Location::try_local_uuid7`], which validate the payload first.
///
/// A local volume path: stable `volume` identity + platform-agnostic path
/// components. Identity is the volume's stable UUID (the old indexer
/// writes it once to `<mount>/.findit_index/.id`), **not** the OS mount
/// path (which is volatile across remounts).
///
/// **Requires `feature = "alloc"`** — the `components` field is a
/// `Vec<SmolStr>` (heap).
#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocalLocation<Id = Uuid7> {
  volume: Id,
  components: std::vec::Vec<SmolStr>,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<Id> LocalLocation<Id> {
  /// **Internal** infallible builder — `pub(crate)` to keep
  /// [`Location::try_local`] the single public construction gate.
  /// The validating ctors call this after checking invariants;
  /// the proto wire adapter will call it when round-tripping a
  /// pre-validated oneof.
  #[inline]
  pub(crate) fn new<I, S>(volume: Id, components: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<SmolStr>,
  {
    Self {
      volume,
      components: components.into_iter().map(Into::into).collect(),
    }
  }

  /// Stable volume identity (the UUID written to
  /// `<mount>/.findit_index/.id`).
  #[inline]
  pub fn volume(&self) -> &Id {
    &self.volume
  }

  /// Platform-agnostic path components, relative to the volume root.
  #[inline]
  pub fn components(&self) -> &[SmolStr] {
    &self.components
  }
}

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
/// ## Variant shape
///
/// Newtype variants only — `Local(LocalLocation<Id>)` follows the
/// no-structure-variants rule. The enum derives `IsVariant` + `Unwrap` +
/// `TryUnwrap` with shared/mut ref flavours, so consumers go through
/// `loc.is_local()` / `loc.unwrap_local_ref()` / `loc.try_unwrap_local()`
/// rather than ad-hoc `match`.
///
/// ## Construction
///
/// `LocalLocation::new` is `pub(crate)`, so [`Location::try_local`] /
/// [`Location::try_local_uuid7`] are the only **public** construction
/// gates and both validate the payload (empty path always rejected, nil
/// volume rejected for the `Uuid7` specialisation). Combined with
/// `Uuid7::nil()` being `pub(crate)`, "construct a fake local location"
/// is unreachable from outside this crate.
///
/// **Requires `feature = "alloc"`** — wraps [`LocalLocation`] whose
/// payload is a `Vec<SmolStr>`.
#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, IsVariant, Unwrap, TryUnwrap)]
#[unwrap(ref, ref_mut)]
#[try_unwrap(ref, ref_mut)]
#[non_exhaustive]
pub enum Location<Id = Uuid7> {
  /// A local volume path. See [`LocalLocation`].
  Local(LocalLocation<Id>),
  // Future: `RemoteUrl(SmolStr)`, `Object(ObjectLocation)` — surfaced
  // when a real consumer needs object-storage roots; reopens this
  // `Location` enum at that time.
}

#[cfg(any(feature = "std", feature = "alloc"))]
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
    let components: std::vec::Vec<SmolStr> = components.into_iter().map(Into::into).collect();
    if components.is_empty() {
      return Err(LocationError::EmptyPath);
    }
    Ok(Self::Local(LocalLocation::new(volume, components)))
  }
}

#[cfg(any(feature = "std", feature = "alloc"))]
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
    let components: std::vec::Vec<SmolStr> = components.into_iter().map(Into::into).collect();
    if components.is_empty() {
      return Err(LocationError::EmptyPath);
    }
    Ok(Self::Local(LocalLocation::new(volume, components)))
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
/// canonicalisable`, and prevents `from_u32(c.as_u32()) != c`
/// ambiguities. Read the wire value via [`UnknownErrorCode::get`].
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
/// Mixed enum (unit + one newtype variant) → derives `IsVariant` plus
/// `Unwrap` / `TryUnwrap` accessor families with ref + ref_mut flavours.
///
/// The domain error model uses `code` as the *stage-coded* signal in
/// `index_errors: Vec<ErrorInfo>`; this replaces the dropped per-track
/// `error_status` bitflags (error-state is now **derived** from the
/// stage codes + `index_status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IsVariant, Unwrap, TryUnwrap)]
#[unwrap(ref, ref_mut)]
#[try_unwrap(ref, ref_mut)]
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
  /// `Self::Unknown(_)` — never lossy, infallible.
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
/// explicitly via [`ErrorInfo::new`] or [`ErrorInfo::code_only`]. Fields
/// are private; access via [`ErrorInfo::code`] / [`ErrorInfo::message`].
///
/// **Requires `feature = "alloc"`** — the `message` field is a
/// `SmolStr` (which needs an allocator for non-inline-sized strings).
#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ErrorInfo {
  code: ErrorCode,
  message: SmolStr,
}

#[cfg(any(feature = "std", feature = "alloc"))]
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

  /// Stage-coded error id (the wire-stable signal).
  #[inline]
  pub const fn code(&self) -> ErrorCode {
    self.code
  }

  /// Human-readable description (`""` = absent).
  #[inline]
  pub fn message(&self) -> &str {
    self.message.as_str()
  }

  /// Builder: replace the message and return `self`.
  #[inline]
  pub fn with_message(mut self, message: impl Into<SmolStr>) -> Self {
    self.message = message.into();
    self
  }

  /// Builder: replace the code and return `self`.
  #[inline]
  pub fn with_code(mut self, code: ErrorCode) -> Self {
    self.code = code;
    self
  }

  /// In-place setter for the message.
  #[inline]
  pub fn set_message(&mut self, message: impl Into<SmolStr>) {
    self.message = message.into();
  }

  /// In-place setter for the code.
  #[inline]
  pub fn set_code(&mut self, code: ErrorCode) {
    self.code = code;
  }
}

// ===========================================================================
// Tests
// ===========================================================================
//
// Gated on `feature = "std"` because many of these exercise
// [`Uuid7::new`] (which depends on `Uuid::now_v7()` ⇒ `SystemTime` ⇒
// std). With `--no-default-features` the `Uuid7::new` ctor disappears
// and these tests would fail to compile; meaningful coverage of the
// validating ctors all flows through the `Uuid7::new` happy path, so
// gating the whole module keeps the matrix green without sprinkling
// per-test `#[cfg(feature = "std")]`s.

#[cfg(all(test, feature = "std"))]
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
    assert_eq!(Uuid7::try_from_bytes([0; 16]), Err(Uuid7Error::Nil));
    assert_eq!(Uuid7::try_from(uuid::Uuid::nil()), Err(Uuid7Error::Nil));
    assert_eq!(
      "00000000-0000-0000-0000-000000000000".parse::<Uuid7>(),
      Err(Uuid7Error::Nil)
    );
    // IsVariant derive: `.is_nil()` reads on the error too.
    assert!(Uuid7Error::Nil.is_nil());
    assert!(!Uuid7Error::WrongVersion(4).is_nil());
  }

  #[test]
  fn uuid7_rejects_non_v7_via_validating_constructors() {
    // Hand-craft a non-nil v4-layout UUID: byte 6 high nibble = 4
    // (RFC 9562 §4.2 version field), byte 8 high bits = 10 (variant).
    let mut bytes = [0u8; 16];
    bytes[0] = 0x01;
    bytes[6] = 0x4a;
    bytes[8] = 0x80;
    let v4 = uuid::Uuid::from_bytes(bytes);
    assert_eq!(v4.get_version_num(), 4);
    let err = Uuid7::try_from(v4).unwrap_err();
    // TryUnwrap derive: shared-ref accessor returns Ok(&inner) for the
    // matching variant; the `IsVariant` derive is the boolean flavour.
    assert!(err.is_wrong_version());
    assert_eq!(err.try_unwrap_wrong_version_ref(), Ok(&4_usize));
    let parse_err = v4.to_string().parse::<Uuid7>().unwrap_err();
    assert_eq!(parse_err.try_unwrap_wrong_version_ref(), Ok(&4_usize));
  }

  #[test]
  fn uuid7_invalid_string_returns_uuid_error() {
    let err = "not-a-uuid".parse::<Uuid7>().unwrap_err();
    assert!(err.is_invalid_uuid());
    assert!(err.try_unwrap_invalid_uuid_ref().is_ok());
  }

  #[test]
  fn uuid7_from_bytes_unchecked_still_works_for_wire_roundtrip() {
    let a = Uuid7::new();
    let raw = *a.as_bytes();
    let b = Uuid7::from_bytes_unchecked(raw);
    assert_eq!(a, b);
  }

  #[test]
  fn uuid7_distinct_from_filechecksum_type() {
    let _u: Uuid7 = Uuid7::nil();
    let _c: FileChecksum = FileChecksum::new();
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
    let cs = FileChecksum::from_bytes(bytes);
    let s = cs.to_string();
    assert_eq!(s.len(), 64);
    assert!(s.starts_with("deadbeef"));
    assert!(s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
  }

  #[test]
  fn filechecksum_new_is_zero_sentinel() {
    // `new()` is the canonical no-arg ctor — the all-zero "not yet
    // computed" sentinel. `Default::default()` delegates to `new()`.
    let z = FileChecksum::new();
    assert!(z.is_zero());
    assert_eq!(z.to_string(), "0".repeat(64));
    assert_eq!(FileChecksum::default(), z);
  }

  #[test]
  fn rgba_pack_unpack() {
    let c = Rgba::from_components(0x12, 0x34, 0x56, 0x78);
    assert_eq!(c.bits(), 0x12_34_56_78);
    assert_eq!(c.r(), 0x12);
    assert_eq!(c.g(), 0x34);
    assert_eq!(c.b(), 0x56);
    assert_eq!(c.a(), 0x78);
    // from_bits is the inverse of bits().
    assert_eq!(Rgba::from_bits(0x12_34_56_78), c);
  }

  #[test]
  fn rgba_new_is_transparent_black() {
    // `new()` is the canonical no-arg ctor — `Default::default()` is
    // `Self::new()` per the encapsulation rule.
    let n = Rgba::new();
    assert_eq!(n.bits(), 0);
    assert_eq!(Rgba::default(), n);
  }

  #[test]
  fn location_try_local_uuid7_happy_path() {
    let vol = Uuid7::new();
    let l = Location::try_local_uuid7(vol, ["Movies", "Holiday"]).unwrap();
    // IsVariant + Unwrap derives.
    assert!(l.is_local());
    let local = l.unwrap_local_ref();
    assert_eq!(local.volume(), &vol);
    assert_eq!(local.components(), &["Movies", "Holiday"]);
  }

  #[test]
  fn location_try_local_uuid7_rejects_nil_volume() {
    let r = Location::try_local_uuid7(Uuid7::nil(), ["Movies"]);
    assert_eq!(r, Err(LocationError::NilVolume));
    assert!(LocationError::NilVolume.is_nil_volume());
  }

  #[test]
  fn location_try_local_uuid7_rejects_empty_path() {
    let vol = Uuid7::new();
    let r = Location::try_local_uuid7::<core::iter::Empty<&str>, &str>(vol, core::iter::empty());
    assert_eq!(r, Err(LocationError::EmptyPath));
    assert!(LocationError::EmptyPath.is_empty_path());
  }

  #[test]
  fn location_generic_try_local_rejects_empty_path() {
    let r = Location::<u32>::try_local::<core::iter::Empty<&str>, &str>(7, core::iter::empty());
    assert_eq!(r, Err(LocationError::EmptyPath));
  }

  #[test]
  fn location_try_unwrap_local_returns_payload() {
    let vol = Uuid7::new();
    let l = Location::try_local_uuid7(vol, ["Movies"]).unwrap();
    let local: LocalLocation<Uuid7> = l.try_unwrap_local().unwrap();
    assert_eq!(local.volume(), &vol);
    assert_eq!(local.components(), &["Movies"]);
  }

  #[test]
  fn error_code_unknown_round_trips_wire_value() {
    for wire in [42_u32, 1234, 99_999, u32::MAX] {
      let c = ErrorCode::from_u32(wire);
      assert!(c.is_unknown(), "{wire} should land in Unknown");
      assert_eq!(c.as_u32(), wire, "wire->domain->wire must be identity");
      // Unwrap derive gives a direct accessor for the payload.
      let payload: UnknownErrorCode = c.unwrap_unknown();
      assert_eq!(payload.get(), wire);
    }
  }

  #[test]
  fn error_code_from_u32_normalises_known_codes() {
    // A known wire value (e.g. 404) must canonicalise to its named
    // variant, never to Unknown(404).
    let canonical = ErrorCode::from_u32(404);
    assert_eq!(canonical, ErrorCode::NotFound);
    assert!(!canonical.is_unknown());
    // try_unwrap_unknown_ref returns Err for non-unknown variants.
    assert!(canonical.try_unwrap_unknown_ref().is_err());

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
  fn error_info_construction_and_accessors() {
    let e = ErrorInfo::new(ErrorCode::ProbeCorrupt, "bad container header");
    assert_eq!(e.code(), ErrorCode::ProbeCorrupt);
    assert_eq!(e.message(), "bad container header");

    let e2 = ErrorInfo::code_only(ErrorCode::Cancelled);
    assert_eq!(e2.code(), ErrorCode::Cancelled);
    assert!(e2.message().is_empty());

    // Builders chain.
    let e3 = ErrorInfo::code_only(ErrorCode::ProbeCorrupt)
      .with_message("clip.mp4")
      .with_code(ErrorCode::ProbeUnsupportedFormat);
    assert_eq!(e3.code(), ErrorCode::ProbeUnsupportedFormat);
    assert_eq!(e3.message(), "clip.mp4");

    // Setters mutate in place.
    let mut e4 = e.clone();
    e4.set_message("");
    e4.set_code(ErrorCode::Cancelled);
    assert_eq!(e4.code(), ErrorCode::Cancelled);
    assert!(e4.message().is_empty());
  }
}
