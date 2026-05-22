//! Integration test for the optional `async-graphql` feature.
//!
//! Builds the GraphQL schema, asserts every domain aggregate name
//! appears in the emitted SDL, and writes the SDL to
//! `src/graphql/schema.graphql` for human inspection. Re-run the test
//! to refresh that file after schema changes:
//!
//! ```sh
//! cargo test --features async-graphql --test graphql_sdl
//! ```

#![cfg(feature = "async-graphql")]

use std::path::PathBuf;

use async_graphql::{EmptyMutation, EmptySubscription, Schema};

use mediaschema::graphql::Query;

fn build() -> Schema<Query, EmptyMutation, EmptySubscription> {
  Schema::build(Query, EmptyMutation, EmptySubscription).finish()
}

/// Every aggregate / VO / scalar / enum / bitflag type that must be
/// reachable from the schema.
const EXPECTED_TYPES: &[&str] = &[
  // Aggregates (16-aggregate full surface).
  "Media",
  "Video",
  "VideoTrack",
  "Scene",
  "Keyframe",
  "Audio",
  "AudioTrack",
  "AudioSegment",
  "Subtitle",
  "SubtitleTrack",
  "SubtitleCue",
  "Speaker",
  "WatchedLocation",
  "UserTag",
  "SceneAnnotation",
  // Shared VOs.
  "Provenance",
  "LocalizedText",
  "ErrorInfo",
  "MediaDevice",
  "MediaGeoLocation",
  "AudioTags",
  "AudioCoverArt",
  "Loudness",
  "AudioFingerprint",
  "Word",
  // Subtitle progress + Video progress (distinct types).
  "SubtitleIndexProgress",
  "VideoIndexProgress",
  // Frame / pixel / colour placeholder VOs.
  "Rect",
  "ColorInfo",
  "HdrStaticMetadata",
  "DolbyVisionConfig",
  "FrameRate",
  "Rational",
  "Dimensions",
  // Detection VOs.
  "Detection",
  "BoundingBox",
  "ObjectDetection",
  "ActionDetection",
  "TextDetection",
  "BarcodeDetection",
  "SaliencyRegion",
  "HorizonInfo",
  "DocumentSegment",
  "Point2D",
  "BodyPoseJoint",
  "BodyPose3DJoint",
  "BodyPoseDetection",
  "HandPoseDetection",
  "BodyPose3DDetection",
  "SubjectDetection",
  "FaceDetection",
  "FaceLandmarkRegion",
  "FaceLandmarksDetection",
  "PersonInstanceMaskDetection",
  "PersonSegmentationMask",
  "HumanAnalysis",
  "AnimalAnalysis",
  "Aesthetics",
  "DominantColor",
  "VlmAnalysis",
  // VideoCodec mixed enum (Object wrapper, not GraphQL Enum).
  "VideoCodec",
  // Enums (GraphQL Enum derives).
  "MediaKind",
  "AudioContentKind",
  "ScanStatus",
  "SceneDetector",
  "KeyframeExtractor",
  "SubtitleKind",
  "VideoIndexStage",
  "AudioIndexStage",
  "SubtitleIndexStage",
  // Bitflags.
  "MediaErrorFlags",
  "VideoIndexStatus",
  "AudioIndexStatus",
  "SubtitleIndexStatus",
  // Scalars.
  "Uuid7",
  "FileChecksum",
  "JiffTimestamp",
  "MediaTimestamp",
  "MediaTimeRange",
  // Rgba (Object) + Location Union.
  "Rgba",
  "Location",
  "LocalLocation",
];

#[test]
fn schema_builds_and_contains_every_expected_type() {
  let schema = build();
  let sdl = schema.sdl();
  let mut missing = std::vec::Vec::new();
  for ty in EXPECTED_TYPES {
    if !sdl.contains(ty) {
      missing.push(*ty);
    }
  }
  assert!(
    missing.is_empty(),
    "SDL is missing expected types: {missing:?}\n--- SDL ---\n{sdl}"
  );
}

/// Refresh `src/graphql/schema.graphql`. The committed file is the
/// canonical artifact for review; this test rewrites it so a CI run
/// catches schema drift.
#[test]
fn schema_sdl_matches_committed_file() {
  let schema = build();
  let mut sdl = schema.sdl();
  if !sdl.ends_with('\n') {
    sdl.push('\n');
  }

  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/graphql/schema.graphql");
  let on_disk = std::fs::read_to_string(&path).unwrap_or_default();

  if std::env::var_os("REGEN_SCHEMA").is_some() || on_disk != sdl {
    std::fs::write(&path, &sdl).expect("write schema.graphql");
  }

  // After the (possible) regen, the file must match.
  let on_disk = std::fs::read_to_string(&path).expect("read schema.graphql");
  assert_eq!(
    on_disk, sdl,
    "schema.graphql is out of date — re-run the test (or set REGEN_SCHEMA=1) and commit."
  );
}

/// Tiny synchronous `block_on` — saves pulling in a runtime dep just
/// for the SDL test. The async-graphql query executor is poll-friendly
/// (it never parks the thread), so a noop waker is sufficient.
fn block_on<F: core::future::Future>(mut fut: F) -> F::Output {
  use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

  const VTABLE: RawWakerVTable = RawWakerVTable::new(
    |_| RawWaker::new(core::ptr::null(), &VTABLE),
    |_| {},
    |_| {},
    |_| {},
  );
  let raw = RawWaker::new(core::ptr::null(), &VTABLE);
  // SAFETY: the vtable above implements every required op as a no-op or
  // clone-of-self; the waker is therefore safe to share even though we
  // never wake it (we busy-loop on Pending instead).
  let waker = unsafe { Waker::from_raw(raw) };
  let mut ctx = Context::from_waker(&waker);
  // SAFETY: `fut` is consumed by this scope and never moved; we treat
  // its stack slot as the pinned location for the duration of the
  // call.
  let mut fut = unsafe { core::pin::Pin::new_unchecked(&mut fut) };
  loop {
    match fut.as_mut().poll(&mut ctx) {
      Poll::Ready(v) => return v,
      Poll::Pending => std::thread::yield_now(),
    }
  }
}

#[test]
fn introspection_query_succeeds() {
  let schema = build();
  let res = block_on(schema.execute(
    r#"{
      __schema {
        queryType { name }
        types { name kind }
      }
    }"#,
  ));
  assert!(
    res.errors.is_empty(),
    "introspection errors: {:?}",
    res.errors
  );
}

#[test]
fn stub_resolvers_return_null() {
  let schema = build();
  let id = mediaschema::domain::Uuid7::new().to_string();
  let q = format!(
    r#"{{
      media(id: "{id}") {{ id }}
      audio(id: "{id}") {{ id }}
      video(id: "{id}") {{ id }}
      subtitle(id: "{id}") {{ id }}
    }}"#
  );
  let res = block_on(schema.execute(&q));
  assert!(res.errors.is_empty(), "errors: {:?}", res.errors);
  let data = res.data.into_json().unwrap();
  assert!(data["media"].is_null());
  assert!(data["audio"].is_null());
  assert!(data["video"].is_null());
  assert!(data["subtitle"].is_null());
}
