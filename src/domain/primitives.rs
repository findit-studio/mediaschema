//! Foundational newtypes for the domain layer.
//!
//! All locked-schema aggregates are generic over an `Id` type parameter,
//! defaulting to [`Uuid7`] (a time-ordered UUIDv7 newtype). [`FileChecksum`]
//! is the 256-bit content hash, **distinct** from `Id` (content ≠ identity).
//! [`Location`] is the structured oneof copied from
//! `findit-proto::common::location` (volume-aware).

use core::fmt;
use core::str::FromStr;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Uuid7(pub Uuid);

impl Uuid7 {
    /// Generate a new time-ordered UUIDv7 from the current wall-clock time.
    #[cfg(feature = "std")]
    #[inline]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// The nil UUID (all-zero bits). Sentinel for "never assigned"; do **not**
    /// use as a real identity.
    #[inline]
    pub const fn nil() -> Self {
        Self(Uuid::nil())
    }

    /// Is this the nil sentinel?
    #[inline]
    pub fn is_nil(&self) -> bool {
        self.0.is_nil()
    }

    /// Raw 16-byte representation.
    #[inline]
    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    /// Construct from raw 16 bytes — caller asserts UUIDv7 layout; no
    /// validation is performed.
    #[inline]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }
}

impl fmt::Display for Uuid7 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl FromStr for Uuid7 {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

impl From<Uuid> for Uuid7 {
    #[inline]
    fn from(u: Uuid) -> Self {
        Self(u)
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

/// File location — a structured oneof, **not** an opaque path string.
///
/// Copied verbatim from `findit-proto::common::location` per locked
/// `schema/primitives.md` r5: a local path is volume-id-relative so removable
/// drives keep one stable identity across eject/replug + mount-point changes.
/// Object-storage support (`Object { store, bucket, key }` + a
/// `StorageProvider` aggregate) is deferred until a real S3/R2 consumer
/// appears (the `mediaframe::Language`-style boundary applies — `#[non_exhaustive]`
/// keeps it forward-compatible).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Location<Id = Uuid7> {
    /// A local volume path: stable `volume` identity + platform-agnostic
    /// path components. Identity is the volume's stable UUID (the old
    /// indexer writes it once to `<mount>/.findit_index/.id`), **not** the
    /// OS mount path (which is volatile across remounts).
    Local {
        volume: Id,
        components: Vec<SmolStr>,
    },
    // Future: `RemoteUrl(SmolStr)`, `Object { store, bucket, key }` —
    // surfaced when a real consumer needs object-storage roots; reopens
    // this `Location` enum at that time.
}

impl<Id: Default> Default for Location<Id> {
    fn default() -> Self {
        Self::Local {
            volume: Id::default(),
            components: Vec::new(),
        }
    }
}

impl<Id> Location<Id> {
    /// Construct a local volume path from components.
    #[inline]
    pub fn local<I, S>(volume: Id, components: I) -> Self
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

// ---------------------------------------------------------------------------
// ErrorCode — structured error vocabulary (verified vs findit-proto)
// ---------------------------------------------------------------------------

/// Stable error-code vocabulary used inside [`ErrorInfo`].
///
/// HTTP-style codes (400–599) for general protocol errors; domain-specific
/// codes (1000+) for indexing-pipeline errors. **Verified** against
/// `findit-proto::common::error_info::ErrorCode` (the values are
/// wire-stable). `#[non_exhaustive]` — new codes may be added without
/// breaking domain consumers.
///
/// The domain error model uses `code` as the *stage-coded* signal in
/// `index_errors: Vec<ErrorInfo>`; this replaces the dropped per-track
/// `error_status` bitflags (error-state is now **derived** from the
/// stage codes + `index_status`).
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCode {
    // --- HTTP-style protocol errors ---
    BadRequest = 400,
    PermissionDenied = 403,
    NotFound = 404,
    AlreadyExists = 409,
    UnprocessableEntity = 422,
    InternalError = 500,
    ServiceUnavailable = 503,
    Timeout = 504,

    // --- Probe (1000s) ---
    ProbeCorrupt = 1000,
    ProbeUnsupportedFormat = 1001,
    ProbeNoVideoStream = 1002,
    ProbeNoAudioStream = 1003,

    // --- Scene detection (2000s) ---
    SceneDetectionFailed = 2000,
    SceneDetectionModelError = 2001,

    // --- Transcription (3000s) ---
    TranscriptionFailed = 3000,
    TranscriptionModelError = 3001,

    // --- VLM (4000s) ---
    VlmFailed = 4000,
    VlmModelError = 4001,

    // --- Apple Vision (5000s) ---
    AppleVisionFailed = 5000,
    AppleVisionRequestFailed = 5001,

    // --- Embedding (6000s) ---
    EmbeddingFailed = 6000,
    EmbeddingModelError = 6001,
    EmbeddingModelLoadFailed = 6002,
    EmbeddingPreprocessFailed = 6003,
    EmbeddingInferenceFailed = 6004,
    EmbeddingOutputInvalid = 6005,

    // --- Path / volume (7000s) ---
    PathNotFound = 7000,
    VolumeNotAvailable = 7001,
    MissingVolumeId = 7002,
    MalformedVolumeId = 7003,
    VolumeIdMismatch = 7004,
    LocalDatabaseError = 7005,
    FolderNotAvailable = 7006,
    LocalPermissionDenied = 7007,

    // --- Remote (8000s) ---
    EndpointUnreachable = 8000,
    AuthenticationFailed = 8001,
    BucketNotFound = 8002,
    QuotaExceeded = 8003,
    RemoteDatabaseError = 8004,
    RemoteTimeout = 8005,

    // --- Cancelled / resource (9000s) ---
    Cancelled = 9000,
    OutOfMemory = 9001,

    // --- CED sound-event detection (10000s) ---
    CedFailed = 10000,
    CedRequestFailed = 10001,
    CedModelError = 10002,
}

impl ErrorCode {
    /// Numeric discriminant (wire-stable; matches
    /// `findit-proto::common::error_info::ErrorCode`).
    #[inline]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }
}

impl Default for ErrorCode {
    fn default() -> Self {
        Self::InternalError
    }
}

// ---------------------------------------------------------------------------
// ErrorInfo — verified vs findit-proto::common::error_info
// ---------------------------------------------------------------------------

/// Reusable error detail used across domain records.
///
/// Verified shape (`findit-proto::common::error_info`): `{ code: ErrorCode,
/// message: SmolStr }`. The `code` is the stage-coded id; `message` is the
/// human-readable description (`""`=absent — no `Option` for strings, per
/// the locked rule).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
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
    fn uuid7_nil_sentinel() {
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

    #[test]
    fn location_local_default() {
        let l: Location = Location::default();
        match l {
            Location::Local { volume, components } => {
                assert!(volume.is_nil());
                assert!(components.is_empty());
            }
        }
    }

    #[test]
    fn location_local_builder() {
        let vol = Uuid7::new();
        let l: Location = Location::local(vol, ["Movies", "Holiday"]);
        match l {
            Location::Local { volume, components } => {
                assert_eq!(volume, vol);
                assert_eq!(components, vec!["Movies", "Holiday"]);
            }
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
