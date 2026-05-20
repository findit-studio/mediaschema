// The roundtrip suite exercises the buffa-generated wire layer
// (`mediaschema::Detection`, `mediaschema::AudioFormat`, …), which is
// only compiled when `feature = "std"` is on (see `src/lib.rs`). Gate
// the whole test file on `feature = "std"` so `--no-default-features`
// and `--features alloc` builds skip it entirely.
#![cfg(feature = "std")]
// The roundtrip suite has multiple `#[cfg(feature = "json")]`-gated test
// functions, plus nested `use mediaschema::{...}` blocks inside many
// batch functions. As a result, the *set* of top-level imports actually
// referenced is feature-combo-dependent: under `--no-default-features`,
// names like `FailedFile`/`MediaMeta`/`SubtitleTrackOriginSource` (used
// only inside json-gated batches or via nested re-imports) read as
// unused, while under `--all-features` they are needed. Sprinkling
// per-name `#[cfg(...)]` on the import block would mirror every
// downstream gate; allowing unused-imports is the small, correct fix.
#![allow(unused_imports)]

use buffa::Message;
use core::num::NonZeroU32;
use mediaschema::{
  ActionDetection, Aesthetics, AnimalAnalysis, AppPathBuf, Audio, AudioAnalysis,
  AudioChannelLayout, AudioChannelOrderKind, AudioChannelSpec, AudioClipKind, AudioCodec,
  AudioContainerFormat, AudioCoverArt, AudioEvent, AudioFileRecord, AudioFormat, AudioMeta,
  AudioPrefilterClass, AudioSampleFormat, AudioStreamMeta, AudioSummary, AudioTrack,
  AudioTrackMeta, AudioTrackRole, AudioTranscriptSegment, BarcodeDetection, BodyPose3DDetection,
  BodyPose3DHeightEstimation, BodyPose3DJoint, BodyPoseDetection, BodyPoseJoint, BoundingBox,
  BrowseItem, BrowseRequest, BrowseResponse, Ced, CedDetection, ChannelLayoutKind, Chromaprint,
  Clap, ClassificationDetection, CodecId, ColorDetection, DbMediaKind, Detection, Dimensions,
  DocumentSegment, Ebur128, EjectVolumeRequest, EjectVolumeResponse, EmotionDetection, ErrorInfo,
  Event, EventKind, FaceDetection, FaceLandmarkPoint, FaceLandmarkRegion, FaceLandmarksDetection,
  FailedFile, FailedFilesResponse, FeaturePrint, FileChecksum, FolderUpdatedEvent,
  GetDaemonInfoRequest, GetDaemonInfoResponse, GetFileIndexingStatsRequest,
  GetFileIndexingStatsResponse, GetIndexedFileRequest, GetIndexedFileResponse,
  GetLocationStatsRequest, GetLocationStatsResponse, GetModelStatusRequest, GetModelStatusResponse,
  HandChirality, HandPoseDetection, HeartbeatRequest, HeartbeatResponse, HorizonInfo,
  HumanAnalysis, Id, IndexLocationRequest, IndexLocationResponse, IndexingFile,
  IndexingProgressResponse, Keyframe, LightingDetection, ListLocationsRequest,
  ListLocationsResponse, Local, Location, LocationKind, LocationTarget, LocationTargetKind, Media,
  MediaKind, MediaKindKind, MediaMeta, ModelDownloadProgress, ModelDownloadProgressEvent,
  ModelDownloadProgressResponse, ModelInfo, MoodDetection, NetFailedFile, ObjectDetection,
  Pagination, PersonInstanceMaskDetection, PersonSegmentationMask, Point2D, RemoveLocationRequest,
  RemoveLocationResponse, Request, RequestEnvelope, RequestKind, Response, ResponseEnvelope,
  ResponseKind, RetryFailedRequest, RetryFailedResponse, SaliencyRegion, Scene, SceneMeta,
  SceneVlmResult, SearchFilter, SearchHit, SearchRequest, SearchResponse, SoundSource,
  Sp2CodegenSmoke, Sp3CodegenSmoke, SpeakerSegment, SubjectDetection, Subtitle, SubtitleCue,
  SubtitleMeta, SubtitleTrack, SubtitleTrackFormat, SubtitleTrackMeta, SubtitleTrackOrigin,
  SubtitleTrackOriginSource, SubtitleTrackRole, Tag, TagConfidence, TextDetection, Timecode,
  TimedDetection, TrackClassificationType, TrackRecord, TrackTag, TrackTime,
  UpdateAnnotationRequest, UpdateAnnotationResponse, Video, VideoFormat, VideoMeta,
  VideoStreamMeta, VideoTrack, VideoTrackMeta, Volume, VolumeMeta, VolumeStateChangedEvent,
  WatchedLocation,
};
use mediatime::{TimeRange, Timebase};

fn rt<M: Message + PartialEq + std::fmt::Debug>(m: &M) {
  let bytes = m.encode_to_vec();
  let back = M::decode_from_slice(&bytes).expect("decode");
  assert_eq!(*m, back, "wire round-trip mismatch");
}

#[test]
fn detection_roundtrip() {
  let d = Detection {
    label: "beach".into(),
    confidence: 0.93,
    ..Default::default()
  };
  rt(&d);
  rt(&Detection::default());
}

#[test]
fn bounding_box_roundtrip() {
  let b = BoundingBox {
    x: 0.1,
    y: 0.2,
    width: 0.5,
    height: 0.4,
    ..Default::default()
  };
  rt(&b);
}

#[test]
fn timed_detection_extern_roundtrip() {
  let mut td = TimedDetection::default();
  td.detection = buffa::MessageField::some(Detection {
    label: "car".into(),
    confidence: 0.7,
    ..Default::default()
  });
  td.range = buffa::MessageField::some(TimeRange::new(
    10,
    20,
    Timebase::new(30000, NonZeroU32::new(1001).unwrap()),
  ));
  td.timebase = buffa::MessageField::some(Timebase::new(1, NonZeroU32::new(48000).unwrap()));
  rt(&td);

  // num == 0 timebase: independently guards mediatime's unconditional-encode
  // contract end-to-end through the mediaschema extern wire path.
  let mut td0 = TimedDetection::default();
  td0.timebase = buffa::MessageField::some(Timebase::new(0, NonZeroU32::new(1).unwrap()));
  rt(&td0);
}

#[test]
#[cfg(feature = "json")]
fn json_roundtrip() {
  let d = Detection {
    label: "indoor".into(),
    confidence: 0.5,
    ..Default::default()
  };
  let json = serde_json::to_string(&d).expect("to_json");
  let back: Detection = serde_json::from_str(&json).expect("from_json");
  assert_eq!(d, back);
}

#[test]
#[cfg(feature = "quickcheck")]
fn detection_quickcheck_roundtrip() {
  fn prop(label: String, confidence: f32) -> quickcheck::TestResult {
    // `Detection` derives PartialEq over the raw f32; NaN != NaN makes an
    // equality property ill-defined for non-finite confidence (the codec
    // itself round-trips the bits faithfully). Scope to the finite domain.
    if !confidence.is_finite() {
      return quickcheck::TestResult::discard();
    }
    let d = Detection {
      label,
      confidence,
      ..Default::default()
    };
    let bytes = d.encode_to_vec();
    let ok = Detection::decode_from_slice(&bytes)
      .map(|b| b == d)
      .unwrap_or(false);
    quickcheck::TestResult::from_bool(ok)
  }
  quickcheck::quickcheck(prop as fn(String, f32) -> quickcheck::TestResult);
}

// ── SP1 Batch 1 ──────────────────────────────────────────────────────────────

#[test]
fn batch1_roundtrip() {
  // Point2D
  let p = Point2D {
    x: 1.5,
    y: 2.5,
    ..Default::default()
  };
  rt(&p);
  rt(&Point2D::default());

  // Dimensions
  let d = Dimensions {
    width: 1920,
    height: 1080,
    ..Default::default()
  };
  rt(&d);
  rt(&Dimensions::default());

  // Aesthetics
  let a = Aesthetics {
    overall_score: 0.85,
    is_utility: true,
    ..Default::default()
  };
  rt(&a);
  rt(&Aesthetics::default());

  // HorizonInfo
  let h = HorizonInfo {
    angle: 3.14,
    confidence: 0.9,
    ..Default::default()
  };
  rt(&h);
  rt(&HorizonInfo::default());

  // CodecId
  let c = CodecId {
    value: 42,
    ..Default::default()
  };
  rt(&c);
  rt(&CodecId::default());

  // FeaturePrint
  let f = FeaturePrint {
    data: vec![0xDE, 0xAD, 0xBE, 0xEF].into(),
    element_type: 1,
    ..Default::default()
  };
  rt(&f);
  rt(&FeaturePrint::default());

  // MediaKind — video arm
  let mk_video = MediaKind {
    kind: Some(MediaKindKind::Video(buffa::EnumValue::from(
      VideoFormat::VIDEO_FORMAT_MP4,
    ))),
    ..Default::default()
  };
  rt(&mk_video);

  // MediaKind — audio arm
  let mk_audio = MediaKind {
    kind: Some(MediaKindKind::Audio(buffa::EnumValue::from(
      AudioFormat::AUDIO_FORMAT_AAC,
    ))),
    ..Default::default()
  };
  rt(&mk_audio);

  // MediaKind — default (no arm set)
  rt(&MediaKind::default());

  // DocumentSegment
  let make_pt = |x, y| {
    buffa::MessageField::some(Point2D {
      x,
      y,
      ..Default::default()
    })
  };
  let seg = DocumentSegment {
    top_left: make_pt(0.0, 0.0),
    top_right: make_pt(1.0, 0.0),
    bottom_left: make_pt(0.0, 1.0),
    bottom_right: make_pt(1.0, 1.0),
    confidence: 0.98,
    ..Default::default()
  };
  rt(&seg);
  rt(&DocumentSegment::default());
}

#[test]
#[cfg(feature = "json")]
fn document_segment_json_roundtrip() {
  use buffa::MessageField;
  let make_pt = |x, y| {
    MessageField::some(Point2D {
      x,
      y,
      ..Default::default()
    })
  };
  let seg = DocumentSegment {
    top_left: make_pt(0.1, 0.2),
    top_right: make_pt(0.9, 0.2),
    bottom_left: make_pt(0.1, 0.8),
    bottom_right: make_pt(0.9, 0.8),
    confidence: 0.75,
    ..Default::default()
  };
  let json = serde_json::to_string(&seg).expect("to_json");
  let back: DocumentSegment = serde_json::from_str(&json).expect("from_json");
  assert_eq!(seg, back);
}

#[test]
#[cfg(feature = "quickcheck")]
fn dimensions_quickcheck_roundtrip() {
  fn prop(width: u32, height: u32) -> quickcheck::TestResult {
    // Dimensions uses u32 scalars; all values are valid (no non-finite
    // domain to discard). Mirror SP0's style with discard as a safety
    // valve — use it to filter any pathological zero-zero case if needed;
    // here we simply admit all values.
    let d = Dimensions {
      width,
      height,
      ..Default::default()
    };
    let bytes = d.encode_to_vec();
    let ok = Dimensions::decode_from_slice(&bytes)
      .map(|b| b == d)
      .unwrap_or(false);
    quickcheck::TestResult::from_bool(ok)
  }
  quickcheck::quickcheck(prop as fn(u32, u32) -> quickcheck::TestResult);
}

// ── SP1 Batch 2 ──────────────────────────────────────────────────────────────

#[test]
fn batch2_roundtrip() {
  // Id — 16 non-zero bytes + default (empty)
  let id = Id {
    value: (1u8..=16).collect(),
    ..Default::default()
  };
  rt(&id);
  rt(&Id::default());

  // FileChecksum — 32 non-zero bytes + default (empty)
  let cksum = FileChecksum {
    value: (1u8..=32).collect(),
    ..Default::default()
  };
  rt(&cksum);
  rt(&FileChecksum::default());

  // Local — nested Id + components
  let local_populated = Local {
    volume: buffa::MessageField::some(Id {
      value: (1u8..=16).collect(),
      ..Default::default()
    }),
    components: vec!["a".into(), "b".into()],
    ..Default::default()
  };
  rt(&local_populated);
  rt(&Local::default());

  // Location — Local arm set
  let loc_local = Location {
    kind: Some(LocationKind::Local(Box::new(Local {
      volume: buffa::MessageField::some(Id {
        value: (1u8..=16).collect(),
        ..Default::default()
      }),
      components: vec!["a".into(), "b".into()],
      ..Default::default()
    }))),
    ..Default::default()
  };
  rt(&loc_local);
  // Location — no arm set (default)
  rt(&Location::default());

  // LocationTarget — local (String) arm set
  let lt_local = LocationTarget {
    kind: Some(LocationTargetKind::Local("/tmp/media".into())),
    ..Default::default()
  };
  rt(&lt_local);
  // LocationTarget — no arm set (default)
  rt(&LocationTarget::default());

  // AppPathBuf — nesting FileChecksum + Location
  let apb = AppPathBuf {
    checksum: buffa::MessageField::some(FileChecksum {
      value: (1u8..=32).collect(),
      ..Default::default()
    }),
    location: buffa::MessageField::some(Location {
      kind: Some(LocationKind::Local(Box::new(Local {
        volume: buffa::MessageField::some(Id {
          value: (1u8..=16).collect(),
          ..Default::default()
        }),
        components: vec!["a".into(), "b".into()],
        ..Default::default()
      }))),
      ..Default::default()
    }),
    ..Default::default()
  };
  rt(&apb);
  rt(&AppPathBuf::default());

  // Tag — populated + default
  let tag = Tag {
    name: "favorite".into(),
    color: 0xFF_AA_00_FF,
    ..Default::default()
  };
  rt(&tag);
  rt(&Tag::default());

  // ErrorInfo — populated + default
  let err = ErrorInfo {
    code: 404,
    message: "not found".into(),
    ..Default::default()
  };
  rt(&err);
  rt(&ErrorInfo::default());
}

#[test]
#[cfg(feature = "json")]
fn app_path_buf_json_roundtrip() {
  let apb = AppPathBuf {
    checksum: buffa::MessageField::some(FileChecksum {
      value: (1u8..=32).collect(),
      ..Default::default()
    }),
    location: buffa::MessageField::some(Location {
      kind: Some(LocationKind::Local(Box::new(Local {
        volume: buffa::MessageField::some(Id {
          value: (1u8..=16).collect(),
          ..Default::default()
        }),
        components: vec!["docs".into(), "video.mp4".into()],
        ..Default::default()
      }))),
      ..Default::default()
    }),
    ..Default::default()
  };
  let json = serde_json::to_string(&apb).expect("to_json");
  let back: AppPathBuf = serde_json::from_str(&json).expect("from_json");
  assert_eq!(apb, back);
}

// ── SP1 Batch 3 ──────────────────────────────────────────────────────────────

fn make_local_location() -> Location {
  Location {
    kind: Some(LocationKind::Local(Box::new(Local {
      volume: buffa::MessageField::some(Id {
        value: (1u8..=16).collect(),
        ..Default::default()
      }),
      components: vec!["media".into(), "videos".into()],
      ..Default::default()
    }))),
    ..Default::default()
  }
}

#[test]
fn batch3_roundtrip() {
  // ── WatchedLocation — fully populated (deleted_at: Some) ─────────────────
  let wl_full = WatchedLocation {
    id: buffa::MessageField::some(Id {
      value: (1u8..=16).collect(),
      ..Default::default()
    }),
    location: buffa::MessageField::some(make_local_location()),
    name: "My Videos".into(),
    status: 3,
    created_at: 1_700_000_000,
    deleted_at: Some(1_800_000_000),
    total_files: 1000,
    indexed_files: 950,
    total_videos: 800,
    indexed_videos: 780,
    total_scenes: 5000,
    total_audios: 200,
    indexed_audios: 195,
    total_failed_files: 50,
    failed_videos: 20,
    failed_audios: 5,
    ..Default::default()
  };
  rt(&wl_full);

  // ── WatchedLocation — deleted_at: None ───────────────────────────────────
  let wl_no_deleted = WatchedLocation {
    id: buffa::MessageField::some(Id {
      value: (1u8..=16).collect(),
      ..Default::default()
    }),
    location: buffa::MessageField::some(make_local_location()),
    name: "Active Location".into(),
    status: 1,
    created_at: 1_700_000_000,
    deleted_at: None,
    total_files: 42,
    indexed_files: 42,
    total_videos: 10,
    indexed_videos: 10,
    total_scenes: 100,
    total_audios: 32,
    indexed_audios: 32,
    total_failed_files: 3,
    failed_videos: 2,
    failed_audios: 1,
    ..Default::default()
  };
  rt(&wl_no_deleted);

  // ── WatchedLocation — default ─────────────────────────────────────────────
  rt(&WatchedLocation::default());

  // ── VolumeMeta — fully populated ──────────────────────────────────────────
  let vm_full = VolumeMeta {
    id: buffa::MessageField::some(Id {
      value: (1u8..=16).collect(),
      ..Default::default()
    }),
    location: buffa::MessageField::some(make_local_location()),
    name: "Seagate 4TB".into(),
    total_size: 4_000_000_000_000,
    used_size: 2_500_000_000_000,
    status: 3,
    ..Default::default()
  };
  rt(&vm_full);

  // ── VolumeMeta — default ──────────────────────────────────────────────────
  rt(&VolumeMeta::default());
}

#[test]
#[cfg(feature = "json")]
fn watched_location_json_roundtrip() {
  let wl = WatchedLocation {
    id: buffa::MessageField::some(Id {
      value: (1u8..=16).collect(),
      ..Default::default()
    }),
    location: buffa::MessageField::some(make_local_location()),
    name: "JSON Test Location".into(),
    status: 3,
    created_at: 1_700_000_000,
    deleted_at: Some(1_800_000_000),
    total_files: 1000,
    indexed_files: 950,
    total_videos: 800,
    indexed_videos: 780,
    total_scenes: 5000,
    total_audios: 200,
    indexed_audios: 195,
    total_failed_files: 50,
    failed_videos: 20,
    failed_audios: 5,
    ..Default::default()
  };
  let json = serde_json::to_string(&wl).expect("to_json");
  let back: WatchedLocation = serde_json::from_str(&json).expect("from_json");
  assert_eq!(wl, back);
}

// ── SP1 Batch 4 ──────────────────────────────────────────────────────────────

fn make_detection(label: &str, confidence: f32) -> buffa::MessageField<Detection> {
  buffa::MessageField::some(Detection {
    label: label.into(),
    confidence,
    ..Default::default()
  })
}

fn make_bbox(x: f32, y: f32, w: f32, h: f32) -> buffa::MessageField<BoundingBox> {
  buffa::MessageField::some(BoundingBox {
    x,
    y,
    width: w,
    height: h,
    ..Default::default()
  })
}

#[test]
fn batch4_roundtrip() {
  // ── 6 single-Detection envelopes ─────────────────────────────────────────
  let populated_det = make_detection("x", 0.9);

  let cd = ClassificationDetection {
    detection: populated_det.clone(),
    ..Default::default()
  };
  rt(&cd);
  rt(&ClassificationDetection::default());

  let ad = ActionDetection {
    detection: populated_det.clone(),
    ..Default::default()
  };
  rt(&ad);
  rt(&ActionDetection::default());

  let ed = EmotionDetection {
    detection: populated_det.clone(),
    ..Default::default()
  };
  rt(&ed);
  rt(&EmotionDetection::default());

  let md = MoodDetection {
    detection: populated_det.clone(),
    ..Default::default()
  };
  rt(&md);
  rt(&MoodDetection::default());

  let ld = LightingDetection {
    detection: populated_det.clone(),
    ..Default::default()
  };
  rt(&ld);
  rt(&LightingDetection::default());

  let col = ColorDetection {
    detection: populated_det.clone(),
    ..Default::default()
  };
  rt(&col);
  rt(&ColorDetection::default());

  // ── ObjectDetection: optional BoundingBox — SET and UNSET ────────────────
  // Both arms use MessageField<BoundingBox>; presence distinguished by .is_set().
  let obj_with_bbox = ObjectDetection {
    detection: make_detection("dog", 0.85),
    bbox: make_bbox(0.1, 0.2, 0.3, 0.4), // MessageField::some(…)
    ..Default::default()
  };
  rt(&obj_with_bbox);

  let obj_no_bbox = ObjectDetection {
    detection: make_detection("sky", 0.7),
    bbox: buffa::MessageField::none(), // optional: explicitly absent
    ..Default::default()
  };
  rt(&obj_no_bbox);

  rt(&ObjectDetection::default());

  // ── SubjectDetection ─────────────────────────────────────────────────────
  let sub = SubjectDetection {
    detection: make_detection("Human", 0.95),
    bbox: make_bbox(0.05, 0.1, 0.4, 0.8),
    ..Default::default()
  };
  rt(&sub);
  rt(&SubjectDetection::default());

  // ── TextDetection ─────────────────────────────────────────────────────────
  let txt = TextDetection {
    text: "Hello World".into(),
    confidence: 0.99,
    bbox: make_bbox(0.0, 0.0, 0.5, 0.1),
    ..Default::default()
  };
  rt(&txt);
  rt(&TextDetection::default());

  // ── BarcodeDetection ──────────────────────────────────────────────────────
  let barcode = BarcodeDetection {
    payload: "https://example.com".into(),
    symbology: "QR_CODE".into(),
    confidence: 0.98,
    bbox: make_bbox(0.2, 0.3, 0.15, 0.15),
    ..Default::default()
  };
  rt(&barcode);
  rt(&BarcodeDetection::default());

  // ── FaceDetection: all 6 floats non-zero incl. a negative angle ───────────
  let face = FaceDetection {
    bbox: make_bbox(0.3, 0.1, 0.2, 0.3),
    confidence: 0.88,
    capture_quality: 0.75,
    roll: -0.5, // negative angle
    yaw: 0.1,
    pitch: 0.2,
    ..Default::default()
  };
  rt(&face);
  rt(&FaceDetection::default());

  // ── SaliencyRegion ────────────────────────────────────────────────────────
  let sal = SaliencyRegion {
    bbox: make_bbox(0.1, 0.1, 0.8, 0.8),
    confidence: 0.6,
    ..Default::default()
  };
  rt(&sal);
  rt(&SaliencyRegion::default());
}

#[test]
#[cfg(feature = "json")]
fn barcode_detection_json_roundtrip() {
  let barcode = BarcodeDetection {
    payload: "https://example.com/scan?q=42".into(),
    symbology: "QR_CODE".into(),
    confidence: 0.98,
    bbox: make_bbox(0.2, 0.3, 0.15, 0.15),
    ..Default::default()
  };
  let json = serde_json::to_string(&barcode).expect("to_json");
  let back: BarcodeDetection = serde_json::from_str(&json).expect("from_json");
  assert_eq!(barcode, back);
}

// ── SP1 Batch 5 ──────────────────────────────────────────────────────────────

#[test]
fn batch5_roundtrip() {
  // ── FaceLandmarkPoint ─────────────────────────────────────────────────────
  let flp = FaceLandmarkPoint {
    x: 0.3,
    y: 0.7,
    ..Default::default()
  };
  rt(&flp);
  rt(&FaceLandmarkPoint::default());

  // ── FaceLandmarkRegion — name + ≥2 points ────────────────────────────────
  let flr = FaceLandmarkRegion {
    name: "left_eye".into(),
    points: vec![
      FaceLandmarkPoint {
        x: 0.25,
        y: 0.35,
        ..Default::default()
      },
      FaceLandmarkPoint {
        x: 0.30,
        y: 0.36,
        ..Default::default()
      },
      FaceLandmarkPoint {
        x: 0.35,
        y: 0.35,
        ..Default::default()
      },
    ],
    ..Default::default()
  };
  rt(&flr);
  rt(&FaceLandmarkRegion::default());

  // ── FaceLandmarksDetection — bbox + confidence + ≥1 non-empty region ─────
  let fld = FaceLandmarksDetection {
    bbox: make_bbox(0.1, 0.1, 0.4, 0.5),
    confidence: 0.92,
    regions: vec![FaceLandmarkRegion {
      name: "nose_tip".into(),
      points: vec![
        FaceLandmarkPoint {
          x: 0.5,
          y: 0.55,
          ..Default::default()
        },
        FaceLandmarkPoint {
          x: 0.52,
          y: 0.57,
          ..Default::default()
        },
      ],
      ..Default::default()
    }],
    ..Default::default()
  };
  rt(&fld);
  rt(&FaceLandmarksDetection::default());

  // ── BodyPoseJoint — name + 3 floats, ≥1 negative coord ───────────────────
  let bpj = BodyPoseJoint {
    name: "left_shoulder".into(),
    x: -0.15,
    y: 0.45,
    confidence: 0.88,
    ..Default::default()
  };
  rt(&bpj);
  rt(&BodyPoseJoint::default());

  // ── BodyPoseDetection — bbox + ≥2 joints ─────────────────────────────────
  let bpd = BodyPoseDetection {
    bbox: make_bbox(0.05, 0.1, 0.5, 0.8),
    confidence: 0.87,
    joints: vec![
      BodyPoseJoint {
        name: "left_hip".into(),
        x: 0.3,
        y: 0.6,
        confidence: 0.9,
        ..Default::default()
      },
      BodyPoseJoint {
        name: "right_hip".into(),
        x: -0.1,
        y: 0.61,
        confidence: 0.85,
        ..Default::default()
      },
    ],
    ..Default::default()
  };
  rt(&bpd);
  rt(&BodyPoseDetection::default());

  // ── BodyPose3DJoint — name + 4 floats incl. z, ≥1 negative ──────────────
  let bp3j = BodyPose3DJoint {
    name: "left_knee".into(),
    x: 0.2,
    y: 0.7,
    z: -0.05,
    confidence: 0.83,
    ..Default::default()
  };
  rt(&bp3j);
  rt(&BodyPose3DJoint::default());

  // ── BodyPose3DDetection — REFERENCE, MEASURED, UNSPECIFIED variants ──────
  let bp3d_reference = BodyPose3DDetection {
    confidence: 0.91,
    body_height: 1.75,
    height_estimation: buffa::EnumValue::from(
      BodyPose3DHeightEstimation::BODY_POSE_3D_HEIGHT_ESTIMATION_REFERENCE,
    ),
    joints: vec![
      BodyPose3DJoint {
        name: "spine".into(),
        x: 0.0,
        y: 0.5,
        z: 0.02,
        confidence: 0.95,
        ..Default::default()
      },
      BodyPose3DJoint {
        name: "neck".into(),
        x: 0.0,
        y: 0.8,
        z: -0.01,
        confidence: 0.93,
        ..Default::default()
      },
    ],
    ..Default::default()
  };
  rt(&bp3d_reference);

  let bp3d_measured = BodyPose3DDetection {
    confidence: 0.88,
    body_height: 1.80,
    height_estimation: buffa::EnumValue::from(
      BodyPose3DHeightEstimation::BODY_POSE_3D_HEIGHT_ESTIMATION_MEASURED,
    ),
    joints: vec![BodyPose3DJoint {
      name: "left_ankle".into(),
      x: -0.2,
      y: 0.05,
      z: 0.0,
      confidence: 0.79,
      ..Default::default()
    }],
    ..Default::default()
  };
  rt(&bp3d_measured);

  let bp3d_unspecified = BodyPose3DDetection {
    confidence: 0.75,
    body_height: 1.70,
    height_estimation: buffa::EnumValue::from(
      BodyPose3DHeightEstimation::BODY_POSE_3D_HEIGHT_ESTIMATION_UNSPECIFIED,
    ),
    joints: vec![BodyPose3DJoint {
      name: "right_wrist".into(),
      x: 0.4,
      y: 0.55,
      z: -0.03,
      confidence: 0.80,
      ..Default::default()
    }],
    ..Default::default()
  };
  rt(&bp3d_unspecified);
  rt(&BodyPose3DDetection::default());

  // ── HandPoseDetection — LEFT and RIGHT chirality variants ─────────────────
  let hpd_left = HandPoseDetection {
    bbox: make_bbox(0.1, 0.2, 0.2, 0.3),
    confidence: 0.94,
    chirality: buffa::EnumValue::from(HandChirality::HAND_CHIRALITY_LEFT),
    joints: vec![
      BodyPoseJoint {
        name: "thumb_tip".into(),
        x: 0.15,
        y: 0.22,
        confidence: 0.91,
        ..Default::default()
      },
      BodyPoseJoint {
        name: "index_tip".into(),
        x: 0.18,
        y: 0.25,
        confidence: 0.89,
        ..Default::default()
      },
    ],
    ..Default::default()
  };
  rt(&hpd_left);

  let hpd_right = HandPoseDetection {
    bbox: make_bbox(0.6, 0.2, 0.2, 0.3),
    confidence: 0.90,
    chirality: buffa::EnumValue::from(HandChirality::HAND_CHIRALITY_RIGHT),
    joints: vec![BodyPoseJoint {
      name: "thumb_tip".into(),
      x: 0.65,
      y: 0.22,
      confidence: 0.88,
      ..Default::default()
    }],
    ..Default::default()
  };
  rt(&hpd_right);
  rt(&HandPoseDetection::default());

  // ── PersonSegmentationMask — bbox + Dimensions + non-empty data ───────────
  let psm = PersonSegmentationMask {
    bbox: make_bbox(0.0, 0.0, 1.0, 1.0),
    confidence: 0.97,
    dimensions: buffa::MessageField::some(Dimensions {
      width: 64,
      height: 64,
      ..Default::default()
    }),
    data: vec![0xAAu8; 64 * 64].into(),
    ..Default::default()
  };
  rt(&psm);
  rt(&PersonSegmentationMask::default());

  // ── PersonInstanceMaskDetection — bbox + Dimensions + instance_index + data
  let pimd = PersonInstanceMaskDetection {
    bbox: make_bbox(0.1, 0.05, 0.35, 0.7),
    confidence: 0.89,
    instance_index: 2,
    dimensions: buffa::MessageField::some(Dimensions {
      width: 32,
      height: 32,
      ..Default::default()
    }),
    data: vec![0xBBu8; 32 * 32].into(),
    ..Default::default()
  };
  rt(&pimd);
  rt(&PersonInstanceMaskDetection::default());
}

#[test]
#[cfg(feature = "json")]
fn body_pose_3d_detection_json_roundtrip() {
  let bp3d = BodyPose3DDetection {
    confidence: 0.93,
    body_height: 1.78,
    height_estimation: buffa::EnumValue::from(
      BodyPose3DHeightEstimation::BODY_POSE_3D_HEIGHT_ESTIMATION_MEASURED,
    ),
    joints: vec![
      BodyPose3DJoint {
        name: "left_shoulder".into(),
        x: -0.1,
        y: 0.75,
        z: 0.02,
        confidence: 0.94,
        ..Default::default()
      },
      BodyPose3DJoint {
        name: "right_shoulder".into(),
        x: 0.1,
        y: 0.75,
        z: 0.02,
        confidence: 0.92,
        ..Default::default()
      },
      BodyPose3DJoint {
        name: "left_hip".into(),
        x: -0.08,
        y: 0.45,
        z: 0.01,
        confidence: 0.90,
        ..Default::default()
      },
    ],
    ..Default::default()
  };
  let json = serde_json::to_string(&bp3d).expect("to_json");
  let back: BodyPose3DDetection = serde_json::from_str(&json).expect("from_json");
  assert_eq!(bp3d, back);
}

// ── SP1 Batch 6 ──────────────────────────────────────────────────────────────

fn make_subject(label: &str, confidence: f32) -> SubjectDetection {
  SubjectDetection {
    detection: make_detection(label, confidence),
    bbox: make_bbox(0.05, 0.1, 0.4, 0.8),
    ..Default::default()
  }
}

fn make_body_pose() -> BodyPoseDetection {
  BodyPoseDetection {
    bbox: make_bbox(0.05, 0.1, 0.5, 0.8),
    confidence: 0.87,
    joints: vec![
      BodyPoseJoint {
        name: "left_hip".into(),
        x: 0.3,
        y: 0.6,
        confidence: 0.9,
        ..Default::default()
      },
      BodyPoseJoint {
        name: "right_hip".into(),
        x: -0.1,
        y: 0.61,
        confidence: 0.85,
        ..Default::default()
      },
    ],
    ..Default::default()
  }
}

fn make_time_range(start: i64, end: i64) -> ::buffa::MessageField<::mediatime::TimeRange> {
  buffa::MessageField::some(TimeRange::new(
    start,
    end,
    Timebase::new(30000, NonZeroU32::new(1001).unwrap()),
  ))
}

#[test]
fn batch6_roundtrip() {
  // ── TrackTime: one field set (declared only) ──────────────────────────────
  let tt_declared_only = TrackTime {
    declared: make_time_range(10, 20),
    packet_observed: buffa::MessageField::none(),
    decoded_observed: buffa::MessageField::none(),
    ..Default::default()
  };
  rt(&tt_declared_only);

  // ── TrackTime: all three fields set with distinct ranges ──────────────────
  let tt_all_set = TrackTime {
    declared: make_time_range(0, 100),
    packet_observed: make_time_range(1, 99),
    decoded_observed: make_time_range(2, 98),
    ..Default::default()
  };
  rt(&tt_all_set);

  // ── TrackTime: default (all unset) ────────────────────────────────────────
  rt(&TrackTime::default());

  // ── AnimalAnalysis: ≥1 subject + ≥1 body pose ─────────────────────────────
  let animal = AnimalAnalysis {
    subjects: vec![make_subject("dog", 0.91)],
    body_poses: vec![make_body_pose()],
    ..Default::default()
  };
  rt(&animal);
  rt(&AnimalAnalysis::default());

  // ── HumanAnalysis: at least subjects, faces, body_poses, body_poses_3d, ──
  // ── face_landmarks, segmentation_masks populated ──────────────────────────
  let human = HumanAnalysis {
    subjects: vec![make_subject("Human", 0.95)],
    faces: vec![FaceDetection {
      bbox: make_bbox(0.3, 0.1, 0.2, 0.3),
      confidence: 0.88,
      capture_quality: 0.75,
      roll: -0.5,
      yaw: 0.1,
      pitch: 0.2,
      ..Default::default()
    }],
    body_poses: vec![make_body_pose()],
    hand_poses: vec![],
    body_poses_3d: vec![BodyPose3DDetection {
      confidence: 0.91,
      body_height: 1.75,
      height_estimation: buffa::EnumValue::from(
        BodyPose3DHeightEstimation::BODY_POSE_3D_HEIGHT_ESTIMATION_REFERENCE,
      ),
      joints: vec![BodyPose3DJoint {
        name: "spine".into(),
        x: 0.0,
        y: 0.5,
        z: 0.02,
        confidence: 0.95,
        ..Default::default()
      }],
      ..Default::default()
    }],
    instance_masks: vec![],
    face_rectangles: vec![],
    face_landmarks: vec![FaceLandmarksDetection {
      bbox: make_bbox(0.1, 0.1, 0.4, 0.5),
      confidence: 0.92,
      regions: vec![FaceLandmarkRegion {
        name: "nose_tip".into(),
        points: vec![
          FaceLandmarkPoint {
            x: 0.5,
            y: 0.55,
            ..Default::default()
          },
          FaceLandmarkPoint {
            x: 0.52,
            y: 0.57,
            ..Default::default()
          },
        ],
        ..Default::default()
      }],
      ..Default::default()
    }],
    segmentation_masks: vec![PersonSegmentationMask {
      bbox: make_bbox(0.0, 0.0, 1.0, 1.0),
      confidence: 0.97,
      dimensions: buffa::MessageField::some(Dimensions {
        width: 32,
        height: 32,
        ..Default::default()
      }),
      data: vec![0xAAu8; 32 * 32].into(),
      ..Default::default()
    }],
    ..Default::default()
  };
  rt(&human);
  rt(&HumanAnalysis::default());
}

#[test]
#[cfg(feature = "json")]
fn track_time_json_roundtrip() {
  // TrackTime with declared set — exercises extern mediatime under serde.
  let tt = TrackTime {
    declared: make_time_range(10, 900),
    packet_observed: make_time_range(11, 898),
    decoded_observed: buffa::MessageField::none(),
    ..Default::default()
  };
  let json = serde_json::to_string(&tt).expect("to_json");
  let back: TrackTime = serde_json::from_str(&json).expect("from_json");
  assert_eq!(tt, back);
}

#[test]
#[cfg(feature = "json")]
fn human_analysis_json_roundtrip() {
  let human = HumanAnalysis {
    subjects: vec![make_subject("Human", 0.95)],
    faces: vec![FaceDetection {
      bbox: make_bbox(0.3, 0.1, 0.2, 0.3),
      confidence: 0.88,
      capture_quality: 0.75,
      roll: -0.5,
      yaw: 0.1,
      pitch: 0.2,
      ..Default::default()
    }],
    body_poses: vec![make_body_pose()],
    hand_poses: vec![],
    body_poses_3d: vec![BodyPose3DDetection {
      confidence: 0.88,
      body_height: 1.80,
      height_estimation: buffa::EnumValue::from(
        BodyPose3DHeightEstimation::BODY_POSE_3D_HEIGHT_ESTIMATION_MEASURED,
      ),
      joints: vec![BodyPose3DJoint {
        name: "left_ankle".into(),
        x: -0.2,
        y: 0.05,
        z: 0.0,
        confidence: 0.79,
        ..Default::default()
      }],
      ..Default::default()
    }],
    instance_masks: vec![],
    face_rectangles: vec![],
    face_landmarks: vec![FaceLandmarksDetection {
      bbox: make_bbox(0.15, 0.15, 0.35, 0.45),
      confidence: 0.89,
      regions: vec![FaceLandmarkRegion {
        name: "left_eye".into(),
        points: vec![
          FaceLandmarkPoint {
            x: 0.25,
            y: 0.35,
            ..Default::default()
          },
          FaceLandmarkPoint {
            x: 0.30,
            y: 0.36,
            ..Default::default()
          },
        ],
        ..Default::default()
      }],
      ..Default::default()
    }],
    segmentation_masks: vec![PersonSegmentationMask {
      bbox: make_bbox(0.0, 0.0, 1.0, 1.0),
      confidence: 0.96,
      dimensions: buffa::MessageField::some(Dimensions {
        width: 16,
        height: 16,
        ..Default::default()
      }),
      data: vec![0xBBu8; 16 * 16].into(),
      ..Default::default()
    }],
    ..Default::default()
  };
  let json = serde_json::to_string(&human).expect("to_json");
  let back: HumanAnalysis = serde_json::from_str(&json).expect("from_json");
  assert_eq!(human, back);
}

// ── SP2 Task 0: cross-package + extern codegen smoke ────────────────────────

#[test]
fn sp2_codegen_smoke_roundtrip() {
  use mediaschema::{ErrorInfo, VideoFormat};
  let s = Sp2CodegenSmoke {
    id: vec![0x01, 0x02, 0x03, 0x04].into(),
    error: buffa::MessageField::some(ErrorInfo {
      code: 7,
      message: "smoke".into(),
      ..Default::default()
    }),
    format: buffa::EnumValue::from(VideoFormat::VIDEO_FORMAT_MP4),
    range: buffa::MessageField::some(mediatime::TimeRange::new(
      10,
      20,
      mediatime::Timebase::new(30000, core::num::NonZeroU32::new(1001).unwrap()),
    )),
    ..Default::default()
  };
  rt(&s);
  rt(&Sp2CodegenSmoke::default());
}

#[test]
#[cfg(feature = "json")]
fn sp2_codegen_smoke_json_roundtrip() {
  use mediaschema::{ErrorInfo, VideoFormat};
  let s = Sp2CodegenSmoke {
    id: vec![0xAA, 0xBB].into(),
    error: buffa::MessageField::some(ErrorInfo {
      code: 1,
      message: "json".into(),
      ..Default::default()
    }),
    format: buffa::EnumValue::from(VideoFormat::VIDEO_FORMAT_MKV),
    range: buffa::MessageField::some(mediatime::TimeRange::new(
      0,
      5,
      mediatime::Timebase::new(1, core::num::NonZeroU32::new(48000).unwrap()),
    )),
    ..Default::default()
  };
  let json = serde_json::to_string(&s).expect("to_json");
  let back: Sp2CodegenSmoke = serde_json::from_str(&json).expect("from_json");
  assert_eq!(s, back);
}

// ── SP2 Batch 1: DB enums ───────────────────────────────────────────────────

#[test]
fn batch1_sp2_enum_discriminants() {
  assert_eq!(DbMediaKind::MEDIA_KIND_UNSPECIFIED as i32, 0);
  assert_eq!(DbMediaKind::MEDIA_KIND_AUDIO as i32, 2);
  assert_eq!(
    SubtitleTrackFormat::SUBTITLE_TRACK_FORMAT_UNSPECIFIED as i32,
    0
  );
  assert_eq!(SubtitleTrackFormat::SUBTITLE_TRACK_FORMAT_WHISPER as i32, 9);
  assert_eq!(SubtitleTrackRole::SUBTITLE_TRACK_ROLE_UNSPECIFIED as i32, 0);
  assert_eq!(SubtitleTrackRole::SUBTITLE_TRACK_ROLE_COMMENTARY as i32, 6);
  assert_eq!(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_UNSPECIFIED as i32, 0);
  assert_eq!(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_CH22_2 as i32, 38);
  assert_eq!(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_CUBE as i32, 9);
  assert_eq!(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_CH5_1 as i32, 19);
  assert_eq!(
    AudioChannelOrderKind::AUDIO_CHANNEL_ORDER_KIND_UNSPECIFIED as i32,
    0
  );
  assert_eq!(
    AudioChannelOrderKind::AUDIO_CHANNEL_ORDER_KIND_AMBISONIC as i32,
    3
  );
  assert_eq!(AudioClipKind::AUDIO_CLIP_KIND_UNSPECIFIED as i32, 0);
  assert_eq!(AudioClipKind::AUDIO_CLIP_KIND_EVENT_SPAN as i32, 4);
  assert_eq!(
    AudioPrefilterClass::AUDIO_PREFILTER_CLASS_UNSPECIFIED as i32,
    0
  );
  assert_eq!(AudioPrefilterClass::AUDIO_PREFILTER_CLASS_NOISE as i32, 3);
  assert_eq!(AudioTrackRole::AUDIO_TRACK_ROLE_UNSPECIFIED as i32, 0);
  assert_eq!(AudioTrackRole::AUDIO_TRACK_ROLE_LYRICS as i32, 6);
  assert_eq!(
    AudioContainerFormat::AUDIO_CONTAINER_FORMAT_UNSPECIFIED as i32,
    0
  );
  assert_eq!(AudioContainerFormat::AUDIO_CONTAINER_FORMAT_AAC as i32, 10);
  assert_eq!(AudioCodec::AUDIO_CODEC_UNSPECIFIED as i32, 0);
  assert_eq!(AudioCodec::AUDIO_CODEC_VORBIS as i32, 7);
  assert_eq!(AudioSampleFormat::AUDIO_SAMPLE_FORMAT_UNSPECIFIED as i32, 0);
  assert_eq!(AudioSampleFormat::AUDIO_SAMPLE_FORMAT_F32 as i32, 4);
  assert_eq!(
    TrackClassificationType::TRACK_CLASSIFICATION_TYPE_UNSPECIFIED as i32,
    0
  );
  assert_eq!(
    TrackClassificationType::TRACK_CLASSIFICATION_TYPE_MIXED as i32,
    7
  );
}

// ── SP2 Batch 2: audio scalar leaves ────────────────────────────────────────

#[test]
fn batch2_sp2_roundtrip() {
  use mediaschema::{
    AudioChannelSpec, AudioEvent, AudioTranscriptSegment, Ced, CedDetection, Chromaprint, Ebur128,
    SoundSource, SpeakerSegment, TagConfidence, Timecode,
  };
  rt(&TagConfidence {
    label: "speech".into(),
    confidence: 0.91,
    ..Default::default()
  });
  rt(&TagConfidence::default());
  rt(&SoundSource {
    name: "rain".into(),
    prominence: "background".into(),
    description: "steady rain".into(),
    ..Default::default()
  });
  rt(&SoundSource::default());
  rt(&AudioEvent {
    event_type: "applause".into(),
    start_ms: 1000,
    end_ms: 4000,
    avg_confidence: 0.8,
    ..Default::default()
  });
  rt(&AudioEvent::default());
  rt(&SpeakerSegment {
    start_ms: 0,
    end_ms: 2500,
    speaker_id: 3,
    ..Default::default()
  });
  rt(&SpeakerSegment::default());
  rt(&AudioTranscriptSegment {
    start_ms: 100,
    end_ms: 900,
    text: "hello".into(),
    language: "en".into(),
    confidence: 0.97,
    ..Default::default()
  });
  rt(&AudioTranscriptSegment::default());
  rt(&AudioChannelSpec {
    index: 2,
    raw_id: 0x10,
    label: "FL".into(),
    ..Default::default()
  });
  rt(&AudioChannelSpec::default());
  rt(&Chromaprint {
    fingerprint: vec![0x01, 0x02, 0x03, 0x04].into(),
    fingerprint_duration: 120.5,
    ..Default::default()
  });
  rt(&Chromaprint::default());
  rt(&Ebur128 {
    loudness_lufs: -14.0,
    loudness_range_lu: 7.5,
    true_peak_dbtp: -1.2,
    ..Default::default()
  });
  rt(&Ebur128::default());
  rt(&Timecode {
    start: "00:00:00:00".into(),
    end: "01:23:45:12".into(),
    fps: 25.0,
    drop_frame: true,
    ..Default::default()
  });
  rt(&Timecode::default());
  rt(&CedDetection {
    tag: 0xDEAD_BEEF_0000_1234,
    confidence: 0.66,
    ..Default::default()
  });
  rt(&CedDetection::default());
  rt(&Ced {
    tags: vec![
      CedDetection {
        tag: 1,
        confidence: 0.5,
        ..Default::default()
      },
      CedDetection {
        tag: 0xFFFF_FFFF_FFFF_FFFF,
        confidence: 0.9,
        ..Default::default()
      },
    ],
    ..Default::default()
  });
  rt(&Ced::default());
}

#[test]
#[cfg(feature = "json")]
fn batch2_sp2_json_roundtrip() {
  use mediaschema::{Ced, CedDetection};
  let c = Ced {
    tags: vec![
      CedDetection {
        tag: 0xABCD_0000_0000_0001,
        confidence: 0.42,
        ..Default::default()
      },
      CedDetection {
        tag: 7,
        confidence: 0.99,
        ..Default::default()
      },
    ],
    ..Default::default()
  };
  let json = serde_json::to_string(&c).expect("to_json");
  let back: Ced = serde_json::from_str(&json).expect("from_json");
  assert_eq!(c, back);
}

// ── SP2 Batch 3: composite audio leaves + reuse-only wrappers ────────────────

#[test]
fn batch3_sp2_roundtrip() {
  use mediaschema::{
    AudioChannelLayout, AudioChannelOrderKind, AudioChannelSpec, ChannelLayoutKind, Clap,
    Detection, TrackTag,
  };
  let det = |l: &str, c: f32| {
    buffa::MessageField::some(Detection {
      label: l.into(),
      confidence: c,
      ..Default::default()
    })
  };

  rt(&AudioChannelLayout {
    order: buffa::EnumValue::from(AudioChannelOrderKind::AUDIO_CHANNEL_ORDER_KIND_NATIVE),
    channels: 6,
    known_kind: buffa::EnumValue::from(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_CH5_1),
    native_mask: Some(0x3F),
    custom_channels: vec![AudioChannelSpec {
      index: 0,
      raw_id: 1,
      label: "FL".into(),
      ..Default::default()
    }],
    description: "5.1".into(),
    ..Default::default()
  });
  rt(&AudioChannelLayout {
    order: buffa::EnumValue::from(AudioChannelOrderKind::AUDIO_CHANNEL_ORDER_KIND_UNSPECIFIED),
    channels: 2,
    known_kind: buffa::EnumValue::from(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_UNSPECIFIED),
    native_mask: None,
    custom_channels: vec![],
    description: String::new(),
    ..Default::default()
  });
  rt(&AudioChannelLayout::default());

  rt(&Clap {
    audio_detection: det("music", 0.9),
    scene: det("concert", 0.8),
    mood: det("energetic", 0.7),
    voice: det("singing", 0.6),
    sound_events: vec![
      Detection {
        label: "applause".into(),
        confidence: 0.5,
        ..Default::default()
      },
      Detection {
        label: "cheer".into(),
        confidence: 0.55,
        ..Default::default()
      },
    ],
    ..Default::default()
  });
  rt(&Clap {
    audio_detection: buffa::MessageField::none(),
    scene: buffa::MessageField::none(),
    mood: buffa::MessageField::none(),
    voice: buffa::MessageField::none(),
    sound_events: vec![],
    ..Default::default()
  });
  rt(&Clap::default());

  rt(&TrackTag {
    category: "ambience".into(),
    detections: vec![Detection {
      label: "wind".into(),
      confidence: 0.6,
      ..Default::default()
    }],
    source: "panns".into(),
    ..Default::default()
  });
  rt(&TrackTag::default());
}

#[test]
#[cfg(feature = "json")]
fn batch3_sp2_json_roundtrip() {
  use mediaschema::{Clap, Detection};
  let c = Clap {
    audio_detection: buffa::MessageField::some(Detection {
      label: "speech".into(),
      confidence: 0.88,
      ..Default::default()
    }),
    scene: buffa::MessageField::none(),
    mood: buffa::MessageField::some(Detection {
      label: "calm".into(),
      confidence: 0.7,
      ..Default::default()
    }),
    voice: buffa::MessageField::none(),
    sound_events: vec![Detection {
      label: "door".into(),
      confidence: 0.4,
      ..Default::default()
    }],
    ..Default::default()
  };
  let json = serde_json::to_string(&c).expect("to_json");
  let back: Clap = serde_json::from_str(&json).expect("from_json");
  assert_eq!(c, back);
}

// ── SP2 Batch 4: non-audio meta blocks ──────────────────────────────────────

fn sp2_track_time_one() -> buffa::MessageField<mediaschema::TrackTime> {
  use mediaschema::TrackTime;
  buffa::MessageField::some(TrackTime {
    declared: buffa::MessageField::some(mediatime::TimeRange::new(
      0,
      1000,
      mediatime::Timebase::new(30000, core::num::NonZeroU32::new(1001).unwrap()),
    )),
    packet_observed: buffa::MessageField::none(),
    decoded_observed: buffa::MessageField::none(),
    ..Default::default()
  })
}

#[test]
fn batch4_sp2_roundtrip() {
  use mediaschema::{
    CodecId, Dimensions, FailedFile, MediaMeta, SceneMeta, SubtitleMeta, SubtitleTrackMeta,
    SubtitleTrackOrigin, SubtitleTrackOriginSource, VideoFormat, VideoMeta, VideoStreamMeta,
    VideoTrackMeta,
  };
  rt(&VideoMeta {
    id: vec![1, 2, 3, 4].into(),
    name: "clip.mp4".into(),
    format: buffa::EnumValue::from(VideoFormat::VIDEO_FORMAT_MP4),
    dimensions: buffa::MessageField::some(Dimensions {
      width: 1920,
      height: 1080,
      ..Default::default()
    }),
    size: 123456,
    time: sp2_track_time_one(),
    frame_rate: 23.976,
    bit_rate: 8_000_000,
    created_at: 1_700_000_000,
    ..Default::default()
  });
  rt(&VideoMeta::default());

  rt(&VideoTrackMeta {
    id: vec![9].into(),
    ordinal: 0,
    stream_index: 1,
    container_track_id: Some(42),
    time: sp2_track_time_one(),
    ..Default::default()
  });
  rt(&VideoTrackMeta {
    id: vec![9].into(),
    ordinal: 1,
    stream_index: 2,
    container_track_id: None,
    time: buffa::MessageField::none(),
    ..Default::default()
  });
  rt(&VideoTrackMeta::default());

  rt(&VideoStreamMeta {
    codec_id: buffa::MessageField::some(CodecId {
      value: 27,
      ..Default::default()
    }),
    dimensions: buffa::MessageField::some(Dimensions {
      width: 3840,
      height: 2160,
      ..Default::default()
    }),
    total_pts: 90_000,
    frame_rate: 60.0,
    bit_rate: 20_000_000,
    time_base: buffa::MessageField::some(mediatime::Timebase::new(
      1,
      core::num::NonZeroU32::new(30000).unwrap(),
    )),
    ..Default::default()
  });
  rt(&VideoStreamMeta::default());

  rt(&MediaMeta {
    id: vec![1].into(),
    checksum: (1u8..=32).collect(),
    name: "a".into(),
    size: 10,
    time: sp2_track_time_one(),
    created_at: 1,
    ..Default::default()
  });
  rt(&MediaMeta::default());

  rt(&SceneMeta {
    id: vec![1].into(),
    video_id: vec![2].into(),
    range: buffa::MessageField::some(mediatime::TimeRange::new(
      100,
      500,
      mediatime::Timebase::new(30000, core::num::NonZeroU32::new(1001).unwrap()),
    )),
    created_at: 5,
    video_track_id: vec![3].into(),
    ..Default::default()
  });
  rt(&SceneMeta::default());

  rt(&SubtitleMeta {
    id: vec![7].into(),
    created_at: 9,
    ..Default::default()
  });
  rt(&SubtitleMeta::default());

  rt(&SubtitleTrackMeta {
    id: vec![1].into(),
    ordinal: 0,
    stream_index: Some(3),
    container_track_id: Some(8),
    time: sp2_track_time_one(),
    ..Default::default()
  });
  rt(&SubtitleTrackMeta {
    id: vec![1].into(),
    ordinal: 2,
    stream_index: None,
    container_track_id: None,
    time: buffa::MessageField::none(),
    ..Default::default()
  });
  rt(&SubtitleTrackMeta::default());

  rt(&SubtitleTrackOrigin {
    kind: 1,
    source: None,
    ..Default::default()
  });
  rt(&SubtitleTrackOrigin {
    kind: 3,
    source: Some(SubtitleTrackOriginSource::SourceAudioTrackId(
      vec![0xAA, 0xBB].into(),
    )),
    ..Default::default()
  });
  rt(&SubtitleTrackOrigin {
    kind: 4,
    source: Some(SubtitleTrackOriginSource::SourceSubtitleTrackId(
      vec![0xCC, 0xDD].into(),
    )),
    ..Default::default()
  });
  rt(&SubtitleTrackOrigin::default());

  rt(&FailedFile {
    id: vec![1].into(),
    media_id: vec![2].into(),
    location_id: vec![3].into(),
    failed_at: 42,
    ..Default::default()
  });
  rt(&FailedFile::default());
}

#[test]
#[cfg(feature = "json")]
fn batch4_sp2_json_roundtrip() {
  use mediaschema::{
    Dimensions, SubtitleTrackOrigin, SubtitleTrackOriginSource, VideoFormat, VideoMeta,
  };
  let vm = VideoMeta {
    id: vec![1, 2].into(),
    name: "j.mp4".into(),
    format: buffa::EnumValue::from(VideoFormat::VIDEO_FORMAT_MKV),
    dimensions: buffa::MessageField::some(Dimensions {
      width: 1280,
      height: 720,
      ..Default::default()
    }),
    size: 99,
    time: sp2_track_time_one(),
    frame_rate: 25.0,
    bit_rate: 5_000_000,
    created_at: 7,
    ..Default::default()
  };
  let json = serde_json::to_string(&vm).expect("to_json");
  let back: VideoMeta = serde_json::from_str(&json).expect("from_json");
  assert_eq!(vm, back);

  let o = SubtitleTrackOrigin {
    kind: 4,
    source: Some(SubtitleTrackOriginSource::SourceSubtitleTrackId(
      vec![1, 2, 3].into(),
    )),
    ..Default::default()
  };
  let oj = serde_json::to_string(&o).expect("to_json");
  let ob: SubtitleTrackOrigin = serde_json::from_str(&oj).expect("from_json");
  assert_eq!(o, ob);
}

// ── SP2 Batch 5: non-audio track/record wrappers ─────────────────────────────

fn make_video_meta() -> buffa::MessageField<VideoMeta> {
  buffa::MessageField::some(VideoMeta {
    id: vec![1, 2, 3, 4].into(),
    name: "clip.mp4".into(),
    format: buffa::EnumValue::from(mediaschema::VideoFormat::VIDEO_FORMAT_MP4),
    dimensions: buffa::MessageField::some(Dimensions {
      width: 1920,
      height: 1080,
      ..Default::default()
    }),
    size: 123456,
    time: sp2_track_time_one(),
    frame_rate: 23.976,
    bit_rate: 8_000_000,
    created_at: 1_700_000_000,
    ..Default::default()
  })
}

fn make_video_track_meta() -> buffa::MessageField<VideoTrackMeta> {
  buffa::MessageField::some(VideoTrackMeta {
    id: vec![9].into(),
    ordinal: 0,
    stream_index: 1,
    container_track_id: Some(42),
    time: sp2_track_time_one(),
    ..Default::default()
  })
}

fn make_video_stream_meta() -> buffa::MessageField<VideoStreamMeta> {
  buffa::MessageField::some(VideoStreamMeta {
    codec_id: buffa::MessageField::some(CodecId {
      value: 27,
      ..Default::default()
    }),
    dimensions: buffa::MessageField::some(Dimensions {
      width: 1920,
      height: 1080,
      ..Default::default()
    }),
    total_pts: 90_000,
    frame_rate: 30.0,
    bit_rate: 8_000_000,
    time_base: buffa::MessageField::some(mediatime::Timebase::new(
      1,
      NonZeroU32::new(30000).unwrap(),
    )),
    ..Default::default()
  })
}

fn make_subtitle_track_meta() -> buffa::MessageField<SubtitleTrackMeta> {
  buffa::MessageField::some(SubtitleTrackMeta {
    id: vec![1].into(),
    ordinal: 0,
    stream_index: Some(3),
    container_track_id: Some(8),
    time: sp2_track_time_one(),
    ..Default::default()
  })
}

fn make_media_meta() -> buffa::MessageField<mediaschema::MediaMeta> {
  buffa::MessageField::some(mediaschema::MediaMeta {
    id: vec![1].into(),
    checksum: (1u8..=32).collect(),
    name: "movie.mp4".into(),
    size: 99_000_000,
    time: sp2_track_time_one(),
    created_at: 1_700_000_000,
    ..Default::default()
  })
}

fn make_subtitle_meta() -> buffa::MessageField<mediaschema::SubtitleMeta> {
  buffa::MessageField::some(mediaschema::SubtitleMeta {
    id: vec![7].into(),
    created_at: 9,
    ..Default::default()
  })
}

fn make_error_info() -> buffa::MessageField<ErrorInfo> {
  buffa::MessageField::some(ErrorInfo {
    code: 5,
    message: "x".into(),
    ..Default::default()
  })
}

#[test]
fn batch5_sp2_roundtrip() {
  // ── Video: populated (non-trivial) ───────────────────────────────────────
  rt(&Video {
    meta: make_video_meta(),
    scenes: vec![vec![1].into(), vec![2].into()],
    index_status: 0x01 | 0x02 | 0x80,
    index_error: make_error_info(),
    error_status: 1,
    ..Default::default()
  });
  // Video: no error, no scenes
  rt(&Video {
    meta: make_video_meta(),
    scenes: vec![],
    index_status: 0x01,
    index_error: buffa::MessageField::none(),
    error_status: 0,
    ..Default::default()
  });
  rt(&Video::default());

  // ── VideoTrack: populated ────────────────────────────────────────────────
  rt(&VideoTrack {
    meta: make_video_track_meta(),
    stream: make_video_stream_meta(),
    disposition: 0x1 | 0x40,
    is_primary: true,
    auto_selected: false,
    selection_reason: "auto".into(),
    video_id: vec![7].into(),
    index_error: make_error_info(),
    ..Default::default()
  });
  rt(&VideoTrack::default());

  // ── Media: populated (video) ─────────────────────────────────────────────
  rt(&Media {
    meta: make_media_meta(),
    kind: buffa::EnumValue::from(DbMediaKind::MEDIA_KIND_VIDEO),
    index_status: 0x01 | 0x02,
    index_error: make_error_info(),
    video_id: Some(vec![1].into()),
    audio_id: None,
    subtitle_id: None,
    error_status: 0,
    capture_date: 1700,
    device_make: "Apple".into(),
    device_model: "iPhone 15".into(),
    gps_location: "37.33,-122.03".into(),
    ..Default::default()
  });
  // Media: unspecified kind, all ids None
  rt(&Media {
    meta: make_media_meta(),
    kind: buffa::EnumValue::from(DbMediaKind::MEDIA_KIND_UNSPECIFIED),
    index_status: 0,
    index_error: buffa::MessageField::none(),
    video_id: None,
    audio_id: None,
    subtitle_id: None,
    error_status: 0,
    capture_date: 0,
    device_make: String::new(),
    device_model: String::new(),
    gps_location: String::new(),
    ..Default::default()
  });
  rt(&Media::default());

  // ── Subtitle: populated ──────────────────────────────────────────────────
  rt(&Subtitle {
    meta: make_subtitle_meta(),
    index_status: 0x01 | 0x04,
    index_error: make_error_info(),
    ..Default::default()
  });
  rt(&Subtitle::default());

  // ── SubtitleTrack: populated ─────────────────────────────────────────────
  rt(&SubtitleTrack {
    meta: make_subtitle_track_meta(),
    subtitle_id: vec![0xAB, 0xCD].into(),
    origin: buffa::MessageField::some(SubtitleTrackOrigin {
      kind: 2,
      source: None,
      ..Default::default()
    }),
    format: buffa::EnumValue::from(SubtitleTrackFormat::SUBTITLE_TRACK_FORMAT_SRT),
    role: buffa::EnumValue::from(SubtitleTrackRole::SUBTITLE_TRACK_ROLE_CAPTION),
    language: "en".into(),
    title: "English".into(),
    codec_id: buffa::MessageField::some(CodecId {
      value: 86,
      ..Default::default()
    }),
    disposition: 0x1,
    is_primary: true,
    auto_selected: false,
    selection_reason: "user".into(),
    index_error: make_error_info(),
    ..Default::default()
  });
  rt(&SubtitleTrack::default());

  // ── SubtitleCue: populated ───────────────────────────────────────────────
  rt(&SubtitleCue {
    id: vec![0x01, 0x02].into(),
    subtitle_track_id: vec![0x03, 0x04].into(),
    range: buffa::MessageField::some(mediatime::TimeRange::new(
      10,
      99,
      mediatime::Timebase::new(30000, NonZeroU32::new(1001).unwrap()),
    )),
    text: "Hello world".into(),
    language: "en".into(),
    confidence: Some(0.9),
    raw_payload: "raw".into(),
    ..Default::default()
  });
  // SubtitleCue: confidence None, range none
  rt(&SubtitleCue {
    id: vec![0x05].into(),
    subtitle_track_id: vec![0x06].into(),
    range: buffa::MessageField::none(),
    text: "bye".into(),
    language: "fr".into(),
    confidence: None,
    raw_payload: String::new(),
    ..Default::default()
  });
  rt(&SubtitleCue::default());
}

#[test]
#[cfg(feature = "json")]
fn batch5_sp2_json_roundtrip() {
  let st = SubtitleTrack {
    meta: make_subtitle_track_meta(),
    subtitle_id: vec![0xAB, 0xCD].into(),
    origin: buffa::MessageField::some(SubtitleTrackOrigin {
      kind: 3,
      source: Some(SubtitleTrackOriginSource::SourceAudioTrackId(
        vec![0xAA, 0xBB].into(),
      )),
      ..Default::default()
    }),
    format: buffa::EnumValue::from(SubtitleTrackFormat::SUBTITLE_TRACK_FORMAT_SRT),
    role: buffa::EnumValue::from(SubtitleTrackRole::SUBTITLE_TRACK_ROLE_CAPTION),
    language: "en".into(),
    title: "English Captions".into(),
    codec_id: buffa::MessageField::some(CodecId {
      value: 86,
      ..Default::default()
    }),
    disposition: 0x1,
    is_primary: true,
    auto_selected: false,
    selection_reason: "user".into(),
    index_error: make_error_info(),
    ..Default::default()
  };
  let json = serde_json::to_string(&st).expect("to_json");
  let back: SubtitleTrack = serde_json::from_str(&json).expect("from_json");
  assert_eq!(st, back);
}

// ── SP2 Batch 6: Scene + SceneVlmResult ─────────────────────────────────────

fn make_scene_meta() -> buffa::MessageField<SceneMeta> {
  buffa::MessageField::some(SceneMeta {
    id: vec![1].into(),
    video_id: vec![2].into(),
    range: buffa::MessageField::some(mediatime::TimeRange::new(
      100,
      500,
      mediatime::Timebase::new(30000, core::num::NonZeroU32::new(1001).unwrap()),
    )),
    created_at: 5,
    video_track_id: vec![3].into(),
    ..Default::default()
  })
}

#[test]
fn batch6_sp2_roundtrip() {
  // ── Scene: populated ──────────────────────────────────────────────────────
  rt(&Scene {
    meta: make_scene_meta(),
    keyframes: vec![vec![1].into(), vec![2].into()],
    description: "ocean coast".into(),
    shot_type: "wide".into(),
    camera_motion: "pan".into(),
    tags: "beach,sunset".into(),
    people_count: 3,
    tag_ids: vec![vec![9].into()],
    vision_provider: vec!["apple".into()],
    smart_folders: vec!["fav".into()],
    ..Default::default()
  });
  // Scene: default
  rt(&Scene::default());

  // ── SceneVlmResult: fully populated ───────────────────────────────────────
  rt(&SceneVlmResult {
    scene: Some("beach".into()),
    description: Some("sunny".into()),
    subjects: vec![SubjectDetection {
      detection: make_detection("person", 0.92),
      bbox: make_bbox(0.1, 0.1, 0.3, 0.7),
      ..Default::default()
    }],
    objects: vec![ObjectDetection {
      detection: make_detection("umbrella", 0.75),
      bbox: make_bbox(0.4, 0.2, 0.2, 0.3),
      ..Default::default()
    }],
    actions: vec![ActionDetection {
      detection: make_detection("swimming", 0.80),
      ..Default::default()
    }],
    mood: vec![MoodDetection {
      detection: make_detection("joyful", 0.85),
      ..Default::default()
    }],
    shot_type: Some("wide".into()),
    lighting: vec![LightingDetection {
      detection: make_detection("natural", 0.90),
      ..Default::default()
    }],
    colors: vec![ColorDetection {
      detection: make_detection("blue", 0.88),
      ..Default::default()
    }],
    tags: vec!["sunset".into()],
    classifications: vec![ClassificationDetection {
      detection: make_detection("landscape", 0.95),
      ..Default::default()
    }],
    ..Default::default()
  });
  // SceneVlmResult: all optionals None, all repeated empty
  rt(&SceneVlmResult {
    scene: None,
    description: None,
    subjects: vec![],
    objects: vec![],
    actions: vec![],
    mood: vec![],
    shot_type: None,
    lighting: vec![],
    colors: vec![],
    tags: vec![],
    classifications: vec![],
    ..Default::default()
  });
  // SceneVlmResult: default
  rt(&SceneVlmResult::default());
}

#[test]
#[cfg(feature = "json")]
fn batch6_sp2_json_roundtrip() {
  let svr = SceneVlmResult {
    scene: Some("beach".into()),
    description: Some("sunny afternoon on the coast".into()),
    subjects: vec![SubjectDetection {
      detection: make_detection("person", 0.92),
      bbox: make_bbox(0.1, 0.1, 0.3, 0.7),
      ..Default::default()
    }],
    objects: vec![ObjectDetection {
      detection: make_detection("umbrella", 0.75),
      bbox: make_bbox(0.4, 0.2, 0.2, 0.3),
      ..Default::default()
    }],
    actions: vec![ActionDetection {
      detection: make_detection("swimming", 0.80),
      ..Default::default()
    }],
    mood: vec![MoodDetection {
      detection: make_detection("joyful", 0.85),
      ..Default::default()
    }],
    shot_type: Some("wide".into()),
    lighting: vec![LightingDetection {
      detection: make_detection("natural", 0.90),
      ..Default::default()
    }],
    colors: vec![ColorDetection {
      detection: make_detection("blue", 0.88),
      ..Default::default()
    }],
    tags: vec!["sunset".into()],
    classifications: vec![ClassificationDetection {
      detection: make_detection("landscape", 0.95),
      ..Default::default()
    }],
    ..Default::default()
  };
  let json = serde_json::to_string(&svr).expect("to_json");
  let back: SceneVlmResult = serde_json::from_str(&json).expect("from_json");
  assert_eq!(svr, back);
}

// ── SP2 Batch 7: Keyframe ────────────────────────────────────────────────────

fn make_keyframe() -> Keyframe {
  Keyframe {
    id: vec![1, 2].into(),
    scene_id: vec![3, 4].into(),
    pts: 12345,
    dimensions: buffa::MessageField::some(Dimensions {
      width: 1920,
      height: 1080,
      ..Default::default()
    }),
    data: vec![0xABu8; 64].into(),
    classifications: vec![ClassificationDetection {
      detection: make_detection("landscape", 0.95),
      ..Default::default()
    }],
    humans: buffa::MessageField::some(HumanAnalysis {
      subjects: vec![make_subject("Human", 0.95)],
      faces: vec![FaceDetection {
        bbox: make_bbox(0.3, 0.1, 0.2, 0.3),
        confidence: 0.88,
        capture_quality: 0.75,
        roll: -0.5,
        yaw: 0.1,
        pitch: 0.2,
        ..Default::default()
      }],
      body_poses: vec![make_body_pose()],
      hand_poses: vec![],
      body_poses_3d: vec![BodyPose3DDetection {
        confidence: 0.91,
        body_height: 1.75,
        height_estimation: buffa::EnumValue::from(
          BodyPose3DHeightEstimation::BODY_POSE_3D_HEIGHT_ESTIMATION_REFERENCE,
        ),
        joints: vec![BodyPose3DJoint {
          name: "spine".into(),
          x: 0.0,
          y: 0.5,
          z: 0.02,
          confidence: 0.95,
          ..Default::default()
        }],
        ..Default::default()
      }],
      instance_masks: vec![],
      face_rectangles: vec![],
      face_landmarks: vec![FaceLandmarksDetection {
        bbox: make_bbox(0.1, 0.1, 0.4, 0.5),
        confidence: 0.92,
        regions: vec![FaceLandmarkRegion {
          name: "nose_tip".into(),
          points: vec![
            FaceLandmarkPoint {
              x: 0.5,
              y: 0.55,
              ..Default::default()
            },
            FaceLandmarkPoint {
              x: 0.52,
              y: 0.57,
              ..Default::default()
            },
          ],
          ..Default::default()
        }],
        ..Default::default()
      }],
      segmentation_masks: vec![PersonSegmentationMask {
        bbox: make_bbox(0.0, 0.0, 1.0, 1.0),
        confidence: 0.97,
        dimensions: buffa::MessageField::some(Dimensions {
          width: 32,
          height: 32,
          ..Default::default()
        }),
        data: vec![0xAAu8; 32 * 32].into(),
        ..Default::default()
      }],
      ..Default::default()
    }),
    animals: buffa::MessageField::some(AnimalAnalysis {
      subjects: vec![make_subject("dog", 0.91)],
      body_poses: vec![make_body_pose()],
      ..Default::default()
    }),
    objects: vec![ObjectDetection {
      detection: make_detection("umbrella", 0.75),
      bbox: make_bbox(0.4, 0.2, 0.2, 0.3),
      ..Default::default()
    }],
    actions: vec![ActionDetection {
      detection: make_detection("swimming", 0.80),
      ..Default::default()
    }],
    mood: vec![MoodDetection {
      detection: make_detection("joyful", 0.85),
      ..Default::default()
    }],
    emotion: vec![EmotionDetection {
      detection: make_detection("happy", 0.88),
      ..Default::default()
    }],
    lighting: vec![LightingDetection {
      detection: make_detection("natural", 0.90),
      ..Default::default()
    }],
    colors: vec![ColorDetection {
      detection: make_detection("blue", 0.88),
      ..Default::default()
    }],
    text_detections: vec![TextDetection {
      text: "Hello World".into(),
      confidence: 0.99,
      bbox: make_bbox(0.0, 0.0, 0.5, 0.1),
      ..Default::default()
    }],
    barcodes: vec![BarcodeDetection {
      payload: "https://example.com".into(),
      symbology: "QR_CODE".into(),
      confidence: 0.98,
      bbox: make_bbox(0.2, 0.3, 0.15, 0.15),
      ..Default::default()
    }],
    attention_saliency: vec![SaliencyRegion {
      bbox: make_bbox(0.1, 0.1, 0.5, 0.5),
      confidence: 0.82,
      ..Default::default()
    }],
    objectness_saliency: vec![SaliencyRegion {
      bbox: make_bbox(0.3, 0.3, 0.4, 0.4),
      confidence: 0.77,
      ..Default::default()
    }],
    horizon: buffa::MessageField::some(HorizonInfo {
      angle: 1.57,
      confidence: 0.91,
      ..Default::default()
    }),
    document_segments: vec![DocumentSegment {
      top_left: buffa::MessageField::some(Point2D {
        x: 0.0,
        y: 0.0,
        ..Default::default()
      }),
      top_right: buffa::MessageField::some(Point2D {
        x: 1.0,
        y: 0.0,
        ..Default::default()
      }),
      bottom_left: buffa::MessageField::some(Point2D {
        x: 0.0,
        y: 1.0,
        ..Default::default()
      }),
      bottom_right: buffa::MessageField::some(Point2D {
        x: 1.0,
        y: 1.0,
        ..Default::default()
      }),
      confidence: 0.96,
      ..Default::default()
    }],
    feature_print: buffa::MessageField::some(FeaturePrint {
      data: vec![0xFFu8; 64].into(),
      element_type: 1,
      ..Default::default()
    }),
    aesthetics: buffa::MessageField::some(Aesthetics {
      overall_score: 0.87,
      is_utility: false,
      ..Default::default()
    }),
    ..Default::default()
  }
}

#[test]
fn batch7_sp2_roundtrip() {
  let kf = make_keyframe();
  rt(&kf);
  rt(&Keyframe::default());
}

#[test]
#[cfg(feature = "json")]
fn batch7_sp2_json_roundtrip() {
  let kf = make_keyframe();
  let json = serde_json::to_string(&kf).expect("to_json");
  let back: Keyframe = serde_json::from_str(&json).expect("from_json");
  assert_eq!(kf, back);
}

// ── SP2 Batch 8: audio meta + summary blocks ─────────────────────────────────

#[test]
fn batch8_sp2_roundtrip() {
  // ── AudioMeta: populated (exercises reserved 2 transparently) ────────────
  rt(&AudioMeta {
    id: vec![1, 2].into(),
    name: "mka".into(),
    container: "mka".into(),
    size: 999,
    time: sp2_track_time_one(),
    created_at: 1700,
    ..Default::default()
  });
  rt(&AudioMeta::default());

  // ── AudioStreamMeta: populated ────────────────────────────────────────────
  rt(&AudioStreamMeta {
    codec_id: buffa::MessageField::some(CodecId {
      value: 86,
      ..Default::default()
    }),
    sample_rate: 48000,
    layout: buffa::MessageField::some(AudioChannelLayout {
      order: buffa::EnumValue::from(AudioChannelOrderKind::AUDIO_CHANNEL_ORDER_KIND_NATIVE),
      channels: 6,
      known_kind: buffa::EnumValue::from(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_CH5_1),
      native_mask: Some(0x3F),
      custom_channels: vec![AudioChannelSpec {
        index: 0,
        raw_id: 1,
        label: "FL".into(),
        ..Default::default()
      }],
      description: "5.1".into(),
      ..Default::default()
    }),
    bit_rate: 256000,
    language: "en".into(),
    stream_title: "Main Audio".into(),
    album: "OST".into(),
    artist: "Composer".into(),
    title: "Track 1".into(),
    genre: "Soundtrack".into(),
    track_number: 3,
    sample_format: "fltp".into(),
    bits_per_sample: 24,
    ..Default::default()
  });
  rt(&AudioStreamMeta::default());

  // ── AudioTrackMeta: container_track_id Some ───────────────────────────────
  rt(&AudioTrackMeta {
    id: vec![1].into(),
    ordinal: 0,
    stream_index: 1,
    container_track_id: Some(7),
    time: sp2_track_time_one(),
    ..Default::default()
  });
  // AudioTrackMeta: container_track_id None, time none
  rt(&AudioTrackMeta {
    id: vec![2].into(),
    ordinal: 1,
    stream_index: 2,
    container_track_id: None,
    time: buffa::MessageField::none(),
    ..Default::default()
  });
  rt(&AudioTrackMeta::default());

  // ── AudioSummary: fully populated ─────────────────────────────────────────
  rt(&AudioSummary {
    prefilter_class: buffa::EnumValue::from(AudioPrefilterClass::AUDIO_PREFILTER_CLASS_CONTENT),
    audio_type: buffa::MessageField::some(TagConfidence {
      label: "speech".into(),
      confidence: 0.91,
      ..Default::default()
    }),
    scene: buffa::MessageField::some(TagConfidence {
      label: "concert".into(),
      confidence: 0.82,
      ..Default::default()
    }),
    mood: buffa::MessageField::some(TagConfidence {
      label: "energetic".into(),
      confidence: 0.75,
      ..Default::default()
    }),
    voice: buffa::MessageField::some(TagConfidence {
      label: "singing".into(),
      confidence: 0.68,
      ..Default::default()
    }),
    has_speech: true,
    has_music: true,
    dominant_language: "en".into(),
    speech_ratio: 0.6,
    speaker_count: 2,
    loudness_lufs: -14.0,
    rms_db: -18.5,
    transcript_preview: "Hello world...".into(),
    clip_count: 4,
    fingerprint: vec![1, 2, 3],
    gemini_enhanced: true,
    ..Default::default()
  });
  // AudioSummary: UNSPECIFIED, all 4 optionals none, empty fingerprint
  rt(&AudioSummary {
    prefilter_class: buffa::EnumValue::from(AudioPrefilterClass::AUDIO_PREFILTER_CLASS_UNSPECIFIED),
    audio_type: buffa::MessageField::none(),
    scene: buffa::MessageField::none(),
    mood: buffa::MessageField::none(),
    voice: buffa::MessageField::none(),
    has_speech: false,
    has_music: false,
    dominant_language: String::new(),
    speech_ratio: 0.0,
    speaker_count: 0,
    loudness_lufs: 0.0,
    rms_db: 0.0,
    transcript_preview: String::new(),
    clip_count: 0,
    fingerprint: vec![],
    gemini_enhanced: false,
    ..Default::default()
  });
  rt(&AudioSummary::default());
}

#[test]
#[cfg(feature = "json")]
fn batch8_sp2_json_roundtrip() {
  let s = AudioSummary {
    prefilter_class: buffa::EnumValue::from(AudioPrefilterClass::AUDIO_PREFILTER_CLASS_CONTENT),
    audio_type: buffa::MessageField::some(TagConfidence {
      label: "speech".into(),
      confidence: 0.91,
      ..Default::default()
    }),
    scene: buffa::MessageField::some(TagConfidence {
      label: "concert".into(),
      confidence: 0.82,
      ..Default::default()
    }),
    mood: buffa::MessageField::some(TagConfidence {
      label: "energetic".into(),
      confidence: 0.75,
      ..Default::default()
    }),
    voice: buffa::MessageField::some(TagConfidence {
      label: "singing".into(),
      confidence: 0.68,
      ..Default::default()
    }),
    has_speech: true,
    has_music: false,
    dominant_language: "en".into(),
    speech_ratio: 0.5,
    speaker_count: 1,
    loudness_lufs: -16.0,
    rms_db: -20.0,
    transcript_preview: "Hello...".into(),
    clip_count: 2,
    fingerprint: vec![1, 2, 3],
    gemini_enhanced: true,
    ..Default::default()
  };
  let json = serde_json::to_string(&s).expect("to_json");
  let back: AudioSummary = serde_json::from_str(&json).expect("from_json");
  assert_eq!(s, back);
}

// ── SP2 Batch 9: audio aggregates ────────────────────────────────────────────

fn make_audio_meta_mf() -> buffa::MessageField<AudioMeta> {
  buffa::MessageField::some(AudioMeta {
    id: vec![0xA1, 0xA2].into(),
    name: "track.mka".into(),
    container: "mka".into(),
    size: 88_200,
    time: sp2_track_time_one(),
    created_at: 1_700_000_001,
    ..Default::default()
  })
}

fn make_audio_summary_mf() -> buffa::MessageField<AudioSummary> {
  buffa::MessageField::some(AudioSummary {
    prefilter_class: buffa::EnumValue::from(AudioPrefilterClass::AUDIO_PREFILTER_CLASS_CONTENT),
    audio_type: buffa::MessageField::some(TagConfidence {
      label: "speech".into(),
      confidence: 0.91,
      ..Default::default()
    }),
    scene: buffa::MessageField::some(TagConfidence {
      label: "concert".into(),
      confidence: 0.82,
      ..Default::default()
    }),
    mood: buffa::MessageField::some(TagConfidence {
      label: "energetic".into(),
      confidence: 0.75,
      ..Default::default()
    }),
    voice: buffa::MessageField::some(TagConfidence {
      label: "singing".into(),
      confidence: 0.68,
      ..Default::default()
    }),
    has_speech: true,
    has_music: true,
    dominant_language: "en".into(),
    speech_ratio: 0.6,
    speaker_count: 2,
    loudness_lufs: -14.0,
    rms_db: -18.5,
    transcript_preview: "Hello world...".into(),
    clip_count: 4,
    fingerprint: vec![1, 2, 3],
    gemini_enhanced: true,
    ..Default::default()
  })
}

fn make_audio_track_meta_mf() -> buffa::MessageField<AudioTrackMeta> {
  buffa::MessageField::some(AudioTrackMeta {
    id: vec![0xB1].into(),
    ordinal: 0,
    stream_index: 1,
    container_track_id: Some(7),
    time: sp2_track_time_one(),
    ..Default::default()
  })
}

fn make_audio_stream_meta_mf() -> buffa::MessageField<AudioStreamMeta> {
  buffa::MessageField::some(AudioStreamMeta {
    codec_id: buffa::MessageField::some(CodecId {
      value: 86,
      ..Default::default()
    }),
    sample_rate: 48000,
    layout: buffa::MessageField::some(AudioChannelLayout {
      order: buffa::EnumValue::from(AudioChannelOrderKind::AUDIO_CHANNEL_ORDER_KIND_NATIVE),
      channels: 2,
      known_kind: buffa::EnumValue::from(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_STEREO),
      native_mask: Some(0x3),
      custom_channels: vec![],
      description: "stereo".into(),
      ..Default::default()
    }),
    bit_rate: 192_000,
    language: "en".into(),
    stream_title: "Main".into(),
    album: String::new(),
    artist: String::new(),
    title: String::new(),
    genre: String::new(),
    track_number: 0,
    sample_format: "fltp".into(),
    bits_per_sample: 32,
    ..Default::default()
  })
}

#[test]
fn batch9_sp2_roundtrip() {
  // ── AudioAnalysis: fully populated (all ~44 fields non-trivial) ───────────
  let aa_full = AudioAnalysis {
    id: vec![0x01, 0x02, 0x03, 0x04].into(),
    audio_id: vec![0x05, 0x06, 0x07, 0x08].into(),
    scene_id: Some(vec![5].into()),
    kind: buffa::EnumValue::from(AudioClipKind::AUDIO_CLIP_KIND_FIXED_WINDOW),
    start_ms: 1000,
    end_ms: 5000,
    track_id: vec![0x09, 0x0A].into(),
    ced_tags: vec![CedDetection {
      tag: 0xDEAD_BEEF_0000_0001,
      confidence: 0.7,
      ..Default::default()
    }],
    zs_audio_type: buffa::MessageField::some(TagConfidence {
      label: "speech".into(),
      confidence: 0.88,
      ..Default::default()
    }),
    zs_scene: buffa::MessageField::some(TagConfidence {
      label: "concert".into(),
      confidence: 0.77,
      ..Default::default()
    }),
    zs_mood: buffa::MessageField::some(TagConfidence {
      label: "energetic".into(),
      confidence: 0.65,
      ..Default::default()
    }),
    zs_sound_events: vec![TagConfidence {
      label: "applause".into(),
      confidence: 0.5,
      ..Default::default()
    }],
    zs_voice: buffa::MessageField::some(TagConfidence {
      label: "male".into(),
      confidence: 0.9,
      ..Default::default()
    }),
    description_en: "Crowd cheering at a concert venue.".into(),
    description_zh: "音乐会场地人群欢呼".into(),
    gemini_scene: "live concert".into(),
    gemini_mood: "excited".into(),
    gemini_sound_sources: vec![SoundSource {
      name: "crowd".into(),
      prominence: "foreground".into(),
      description: "cheering crowd".into(),
      ..Default::default()
    }],
    gemini_foreground: "applause and cheers".into(),
    gemini_background: "ambient noise".into(),
    gemini_enhanced: true,
    event_timeline: vec![AudioEvent {
      event_type: "applause".into(),
      start_ms: 1000,
      end_ms: 4000,
      avg_confidence: 0.8,
      ..Default::default()
    }],
    foreground_layer: "vocals".into(),
    background_layer: "music".into(),
    speech_ratio: 0.6,
    speaker_count: 2,
    speaker_segments: vec![SpeakerSegment {
      start_ms: 0,
      end_ms: 2500,
      speaker_id: 1,
      ..Default::default()
    }],
    voice_gender: buffa::MessageField::some(TagConfidence {
      label: "male".into(),
      confidence: 0.85,
      ..Default::default()
    }),
    voice_emotion: buffa::MessageField::some(TagConfidence {
      label: "excited".into(),
      confidence: 0.72,
      ..Default::default()
    }),
    transcript: "Hello everyone welcome to the show.".into(),
    transcript_segments: vec![AudioTranscriptSegment {
      start_ms: 100,
      end_ms: 900,
      text: "hello".into(),
      language: "en".into(),
      confidence: 0.97,
      ..Default::default()
    }],
    language: "eng".into(),
    music_genre: buffa::MessageField::some(TagConfidence {
      label: "rock".into(),
      confidence: 0.82,
      ..Default::default()
    }),
    music_bpm: Some(120.0),
    music_instruments: vec![TagConfidence {
      label: "guitar".into(),
      confidence: 0.75,
      ..Default::default()
    }],
    loudness_lufs: -14.0,
    rms_db: -18.5,
    snr_db: Some(15.0),
    has_sudden_onset: true,
    energy_profile: "rising".into(),
    spectral_flatness: 0.4,
    fingerprint: vec![0xDEAD_BEEF, 0x0000_1234],
    prefilter_class: buffa::EnumValue::from(AudioPrefilterClass::AUDIO_PREFILTER_CLASS_CONTENT),
    has_speech: true,
    has_music: true,
    ..Default::default()
  };
  rt(&aa_full);

  // ── AudioAnalysis: minimal (sparse-band guard — all optional→None, repeated→empty) ──
  let aa_min = AudioAnalysis {
    id: vec![0x10].into(),
    audio_id: vec![0x11].into(),
    scene_id: None,
    kind: buffa::EnumValue::from(AudioClipKind::AUDIO_CLIP_KIND_UNSPECIFIED),
    start_ms: 0,
    end_ms: 0,
    track_id: vec![].into(),
    ced_tags: vec![],
    zs_audio_type: buffa::MessageField::none(),
    zs_scene: buffa::MessageField::none(),
    zs_mood: buffa::MessageField::none(),
    zs_sound_events: vec![],
    zs_voice: buffa::MessageField::none(),
    description_en: String::new(),
    description_zh: String::new(),
    gemini_scene: String::new(),
    gemini_mood: String::new(),
    gemini_sound_sources: vec![],
    gemini_foreground: String::new(),
    gemini_background: String::new(),
    gemini_enhanced: false,
    event_timeline: vec![],
    foreground_layer: String::new(),
    background_layer: String::new(),
    speech_ratio: 0.0,
    speaker_count: 0,
    speaker_segments: vec![],
    voice_gender: buffa::MessageField::none(),
    voice_emotion: buffa::MessageField::none(),
    transcript: String::new(),
    transcript_segments: vec![],
    language: String::new(),
    music_genre: buffa::MessageField::none(),
    music_bpm: None,
    music_instruments: vec![],
    loudness_lufs: 0.0,
    rms_db: 0.0,
    snr_db: None,
    has_sudden_onset: false,
    energy_profile: String::new(),
    spectral_flatness: 0.0,
    fingerprint: vec![],
    prefilter_class: buffa::EnumValue::from(AudioPrefilterClass::AUDIO_PREFILTER_CLASS_UNSPECIFIED),
    has_speech: false,
    has_music: false,
    ..Default::default()
  };
  rt(&aa_min);

  rt(&AudioAnalysis::default());

  // ── TrackRecord: fully populated ─────────────────────────────────────────
  let tr_full = TrackRecord {
    id: vec![0x20, 0x21].into(),
    audio_id: vec![0x22, 0x23].into(),
    track_index: 1,
    codec: buffa::EnumValue::from(AudioCodec::AUDIO_CODEC_AAC),
    sample_format: buffa::EnumValue::from(AudioSampleFormat::AUDIO_SAMPLE_FORMAT_FLTP),
    sample_rate: 48000,
    channels: 2,
    channel_layout: buffa::EnumValue::from(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_STEREO),
    bit_rate: 192_000,
    bit_depth: 16,
    total_pts: 2_304_000,
    time_base: buffa::MessageField::some(mediatime::Timebase::new(
      1,
      NonZeroU32::new(48000).unwrap(),
    )),
    language: 0x00_65_6E_67, // "eng" ISO 639-2B
    timecode: buffa::MessageField::some(Timecode {
      start: "00:00:00:00".into(),
      end: "01:23:45:12".into(),
      fps: 25.0,
      drop_frame: false,
      ..Default::default()
    }),
    classification: buffa::EnumValue::from(
      TrackClassificationType::TRACK_CLASSIFICATION_TYPE_VOICE,
    ),
    stop_reason: 0x02,
    tags: vec![TrackTag {
      category: "ambience".into(),
      detections: vec![Detection {
        label: "wind".into(),
        confidence: 0.6,
        ..Default::default()
      }],
      source: "panns".into(),
      ..Default::default()
    }],
    ebur_128: buffa::MessageField::some(Ebur128 {
      loudness_lufs: -14.0,
      loudness_range_lu: 7.5,
      true_peak_dbtp: -1.2,
      ..Default::default()
    }),
    clap: buffa::MessageField::some(Clap {
      audio_detection: buffa::MessageField::some(Detection {
        label: "music".into(),
        confidence: 0.9,
        ..Default::default()
      }),
      scene: buffa::MessageField::some(Detection {
        label: "concert".into(),
        confidence: 0.8,
        ..Default::default()
      }),
      mood: buffa::MessageField::some(Detection {
        label: "energetic".into(),
        confidence: 0.7,
        ..Default::default()
      }),
      voice: buffa::MessageField::some(Detection {
        label: "singing".into(),
        confidence: 0.6,
        ..Default::default()
      }),
      sound_events: vec![Detection {
        label: "applause".into(),
        confidence: 0.5,
        ..Default::default()
      }],
      ..Default::default()
    }),
    ced: buffa::MessageField::some(Ced {
      tags: vec![
        CedDetection {
          tag: 1,
          confidence: 0.5,
          ..Default::default()
        },
        CedDetection {
          tag: 0xFFFF_FFFF_FFFF_FFFF,
          confidence: 0.9,
          ..Default::default()
        },
      ],
      ..Default::default()
    }),
    chromaprint: buffa::MessageField::some(Chromaprint {
      fingerprint: vec![0x01, 0x02, 0x03, 0x04].into(),
      fingerprint_duration: 120.5,
      ..Default::default()
    }),
    index_status: 0x01 | 0x100 | 0x400,
    index_error: buffa::MessageField::some(ErrorInfo {
      code: 7,
      message: "partial".into(),
      ..Default::default()
    }),
    error_status: 1,
    ..Default::default()
  };
  rt(&tr_full);

  // ── TrackRecord: 1000s band all none(), timecode none(), index_error none() ──
  // (proves reserved 17 + 1000s/2000s bands round-trip)
  let tr_min = TrackRecord {
    id: vec![0x30].into(),
    audio_id: vec![0x31].into(),
    track_index: 0,
    codec: buffa::EnumValue::from(AudioCodec::AUDIO_CODEC_UNSPECIFIED),
    sample_format: buffa::EnumValue::from(AudioSampleFormat::AUDIO_SAMPLE_FORMAT_UNSPECIFIED),
    sample_rate: 0,
    channels: 0,
    channel_layout: buffa::EnumValue::from(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_UNSPECIFIED),
    bit_rate: 0,
    bit_depth: 0,
    total_pts: 0,
    time_base: buffa::MessageField::none(),
    language: 0,
    timecode: buffa::MessageField::none(),
    classification: buffa::EnumValue::from(
      TrackClassificationType::TRACK_CLASSIFICATION_TYPE_UNSPECIFIED,
    ),
    stop_reason: 0,
    tags: vec![],
    ebur_128: buffa::MessageField::none(),
    clap: buffa::MessageField::none(),
    ced: buffa::MessageField::none(),
    chromaprint: buffa::MessageField::none(),
    index_status: 0,
    index_error: buffa::MessageField::none(),
    error_status: 0,
    ..Default::default()
  };
  rt(&tr_min);

  rt(&TrackRecord::default());

  // ── Audio: populated (proves reserved 3, 4) ───────────────────────────────
  let audio_full = Audio {
    meta: make_audio_meta_mf(),
    analyses: vec![vec![1].into(), vec![2].into()],
    summary: make_audio_summary_mf(),
    index_status: 0x01 | 0x20,
    index_error: buffa::MessageField::some(ErrorInfo {
      code: 5,
      message: "x".into(),
      ..Default::default()
    }),
    ..Default::default()
  };
  rt(&audio_full);

  // ── Audio: index_error none(), empty analyses ─────────────────────────────
  let audio_min = Audio {
    meta: make_audio_meta_mf(),
    analyses: vec![],
    summary: make_audio_summary_mf(),
    index_status: 0,
    index_error: buffa::MessageField::none(),
    ..Default::default()
  };
  rt(&audio_min);

  rt(&Audio::default());

  // ── AudioTrack: populated (proves reserved 2) ─────────────────────────────
  let at_full = AudioTrack {
    meta: make_audio_track_meta_mf(),
    stream: make_audio_stream_meta_mf(),
    disposition: 0x1,
    role: buffa::EnumValue::from(AudioTrackRole::AUDIO_TRACK_ROLE_MAIN_PROGRAM),
    is_primary: true,
    auto_selected: false,
    selection_reason: "auto".into(),
    audio_id: vec![0xC1, 0xC2].into(),
    index_error: buffa::MessageField::some(ErrorInfo {
      code: 3,
      message: "err".into(),
      ..Default::default()
    }),
    ..Default::default()
  };
  rt(&at_full);

  rt(&AudioTrack::default());
}

#[test]
#[cfg(feature = "json")]
fn batch9_sp2_json_roundtrip() {
  // ── AudioAnalysis: fully populated (widest sparse-field serde case) ───────
  let aa = AudioAnalysis {
    id: vec![0x01, 0x02, 0x03, 0x04].into(),
    audio_id: vec![0x05, 0x06, 0x07, 0x08].into(),
    scene_id: Some(vec![5].into()),
    kind: buffa::EnumValue::from(AudioClipKind::AUDIO_CLIP_KIND_FIXED_WINDOW),
    start_ms: 1000,
    end_ms: 5000,
    track_id: vec![0x09, 0x0A].into(),
    ced_tags: vec![CedDetection {
      tag: 0xDEAD_BEEF_0000_0001,
      confidence: 0.7,
      ..Default::default()
    }],
    zs_audio_type: buffa::MessageField::some(TagConfidence {
      label: "speech".into(),
      confidence: 0.88,
      ..Default::default()
    }),
    zs_scene: buffa::MessageField::some(TagConfidence {
      label: "concert".into(),
      confidence: 0.77,
      ..Default::default()
    }),
    zs_mood: buffa::MessageField::some(TagConfidence {
      label: "energetic".into(),
      confidence: 0.65,
      ..Default::default()
    }),
    zs_sound_events: vec![TagConfidence {
      label: "applause".into(),
      confidence: 0.5,
      ..Default::default()
    }],
    zs_voice: buffa::MessageField::some(TagConfidence {
      label: "male".into(),
      confidence: 0.9,
      ..Default::default()
    }),
    description_en: "Crowd cheering at a concert venue.".into(),
    description_zh: "音乐会场地人群欢呼".into(),
    gemini_scene: "live concert".into(),
    gemini_mood: "excited".into(),
    gemini_sound_sources: vec![SoundSource {
      name: "crowd".into(),
      prominence: "foreground".into(),
      description: "cheering crowd".into(),
      ..Default::default()
    }],
    gemini_foreground: "applause and cheers".into(),
    gemini_background: "ambient noise".into(),
    gemini_enhanced: true,
    event_timeline: vec![AudioEvent {
      event_type: "applause".into(),
      start_ms: 1000,
      end_ms: 4000,
      avg_confidence: 0.8,
      ..Default::default()
    }],
    foreground_layer: "vocals".into(),
    background_layer: "music".into(),
    speech_ratio: 0.6,
    speaker_count: 2,
    speaker_segments: vec![SpeakerSegment {
      start_ms: 0,
      end_ms: 2500,
      speaker_id: 1,
      ..Default::default()
    }],
    voice_gender: buffa::MessageField::some(TagConfidence {
      label: "male".into(),
      confidence: 0.85,
      ..Default::default()
    }),
    voice_emotion: buffa::MessageField::some(TagConfidence {
      label: "excited".into(),
      confidence: 0.72,
      ..Default::default()
    }),
    transcript: "Hello everyone welcome to the show.".into(),
    transcript_segments: vec![AudioTranscriptSegment {
      start_ms: 100,
      end_ms: 900,
      text: "hello".into(),
      language: "en".into(),
      confidence: 0.97,
      ..Default::default()
    }],
    language: "eng".into(),
    music_genre: buffa::MessageField::some(TagConfidence {
      label: "rock".into(),
      confidence: 0.82,
      ..Default::default()
    }),
    music_bpm: Some(120.0),
    music_instruments: vec![TagConfidence {
      label: "guitar".into(),
      confidence: 0.75,
      ..Default::default()
    }],
    loudness_lufs: -14.0,
    rms_db: -18.5,
    snr_db: Some(15.0),
    has_sudden_onset: true,
    energy_profile: "rising".into(),
    spectral_flatness: 0.4,
    fingerprint: vec![0xDEAD_BEEF, 0x0000_1234],
    prefilter_class: buffa::EnumValue::from(AudioPrefilterClass::AUDIO_PREFILTER_CLASS_CONTENT),
    has_speech: true,
    has_music: true,
    ..Default::default()
  };
  let json = serde_json::to_string(&aa).expect("to_json");
  let back: AudioAnalysis = serde_json::from_str(&json).expect("from_json");
  assert_eq!(aa, back);

  // ── TrackRecord: banded tags + extern Timebase + nested Clap/Ced ─────────
  let tr = TrackRecord {
    id: vec![0x20, 0x21].into(),
    audio_id: vec![0x22, 0x23].into(),
    track_index: 1,
    codec: buffa::EnumValue::from(AudioCodec::AUDIO_CODEC_AAC),
    sample_format: buffa::EnumValue::from(AudioSampleFormat::AUDIO_SAMPLE_FORMAT_FLTP),
    sample_rate: 48000,
    channels: 2,
    channel_layout: buffa::EnumValue::from(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_STEREO),
    bit_rate: 192_000,
    bit_depth: 16,
    total_pts: 2_304_000,
    time_base: buffa::MessageField::some(mediatime::Timebase::new(
      1,
      NonZeroU32::new(48000).unwrap(),
    )),
    language: 0x00_65_6E_67,
    timecode: buffa::MessageField::some(Timecode {
      start: "00:00:00:00".into(),
      end: "01:23:45:12".into(),
      fps: 25.0,
      drop_frame: false,
      ..Default::default()
    }),
    classification: buffa::EnumValue::from(
      TrackClassificationType::TRACK_CLASSIFICATION_TYPE_VOICE,
    ),
    stop_reason: 0x02,
    tags: vec![TrackTag {
      category: "ambience".into(),
      detections: vec![Detection {
        label: "wind".into(),
        confidence: 0.6,
        ..Default::default()
      }],
      source: "panns".into(),
      ..Default::default()
    }],
    ebur_128: buffa::MessageField::some(Ebur128 {
      loudness_lufs: -14.0,
      loudness_range_lu: 7.5,
      true_peak_dbtp: -1.2,
      ..Default::default()
    }),
    clap: buffa::MessageField::some(Clap {
      audio_detection: buffa::MessageField::some(Detection {
        label: "music".into(),
        confidence: 0.9,
        ..Default::default()
      }),
      scene: buffa::MessageField::some(Detection {
        label: "concert".into(),
        confidence: 0.8,
        ..Default::default()
      }),
      mood: buffa::MessageField::some(Detection {
        label: "energetic".into(),
        confidence: 0.7,
        ..Default::default()
      }),
      voice: buffa::MessageField::some(Detection {
        label: "singing".into(),
        confidence: 0.6,
        ..Default::default()
      }),
      sound_events: vec![Detection {
        label: "applause".into(),
        confidence: 0.5,
        ..Default::default()
      }],
      ..Default::default()
    }),
    ced: buffa::MessageField::some(Ced {
      tags: vec![
        CedDetection {
          tag: 1,
          confidence: 0.5,
          ..Default::default()
        },
        CedDetection {
          tag: 0xFFFF_FFFF_FFFF_FFFF,
          confidence: 0.9,
          ..Default::default()
        },
      ],
      ..Default::default()
    }),
    chromaprint: buffa::MessageField::some(Chromaprint {
      fingerprint: vec![0x01, 0x02, 0x03, 0x04].into(),
      fingerprint_duration: 120.5,
      ..Default::default()
    }),
    index_status: 0x01 | 0x100 | 0x400,
    index_error: buffa::MessageField::some(ErrorInfo {
      code: 7,
      message: "partial".into(),
      ..Default::default()
    }),
    error_status: 1,
    ..Default::default()
  };
  let json = serde_json::to_string(&tr).expect("to_json");
  let back: TrackRecord = serde_json::from_str(&json).expect("from_json");
  assert_eq!(tr, back);
}

// ── SP2 Batch 10 ─────────────────────────────────────────────────────────────

#[test]
fn batch10_sp2_roundtrip() {
  // ── AudioCoverArt: populated (path set, dimensions set) ───────────────────
  let cover_populated = AudioCoverArt {
    path: buffa::MessageField::some(make_local_location()),
    mime: "image/jpeg".into(),
    dimensions: buffa::MessageField::some(Dimensions {
      width: 600,
      height: 600,
      ..Default::default()
    }),
    size: 40_000,
    ..Default::default()
  };
  rt(&cover_populated);

  // ── AudioCoverArt: path/dimensions absent ─────────────────────────────────
  let cover_minimal = AudioCoverArt {
    path: buffa::MessageField::none(),
    dimensions: buffa::MessageField::none(),
    mime: String::new(),
    size: 0,
    ..Default::default()
  };
  rt(&cover_minimal);
  rt(&AudioCoverArt::default());

  // ── AudioFileRecord: fully populated ──────────────────────────────────────
  let afr_full = AudioFileRecord {
    id: vec![1, 2].into(),
    checksum: Some((1u8..=32).collect()),
    name: "track.flac".into(),
    format: buffa::EnumValue::from(AudioFormat::AUDIO_FORMAT_FLAC),
    size: 52_428_800,
    total_pts: 2_646_000,
    frame_rate: 23.976,
    bit_rate: 1_411_200,
    time_base: buffa::MessageField::some(mediatime::Timebase::new(
      1,
      NonZeroU32::new(44100).unwrap(),
    )),
    container_format: buffa::EnumValue::from(AudioContainerFormat::AUDIO_CONTAINER_FORMAT_FLAC),
    stream_count: 1,
    title: "Song Title".into(),
    artist: "Artist Name".into(),
    album_artist: "Album Artist".into(),
    album: "Album Name".into(),
    genre: "Rock".into(),
    composer: "Composer Name".into(),
    performer: "Performer Name".into(),
    date: "2023-01-01".into(),
    track_number: 3,
    total_tracks: 12,
    disc_number: 1,
    total_discs: 2,
    comment: "A great track".into(),
    lyrics: "La la la...".into(),
    tag_types: vec!["ID3v2".into(), "Vorbis".into()],
    cover_art: buffa::MessageField::some(AudioCoverArt {
      path: buffa::MessageField::some(make_local_location()),
      mime: "image/jpeg".into(),
      dimensions: buffa::MessageField::some(Dimensions {
        width: 600,
        height: 600,
        ..Default::default()
      }),
      size: 40_000,
      ..Default::default()
    }),
    created_at: 1700,
    ..Default::default()
  };
  rt(&afr_full);

  // ── AudioFileRecord: checksum/cover_art/tag_types absent ──────────────────
  // (proves reserved 28..30 transparent + optionals default to absent)
  let afr_sparse = AudioFileRecord {
    id: vec![3, 4].into(),
    checksum: None,
    name: "sparse.flac".into(),
    format: buffa::EnumValue::from(AudioFormat::AUDIO_FORMAT_FLAC),
    size: 1024,
    total_pts: 100,
    frame_rate: 44100.0,
    bit_rate: 320_000,
    time_base: buffa::MessageField::some(mediatime::Timebase::new(
      1,
      NonZeroU32::new(48000).unwrap(),
    )),
    container_format: buffa::EnumValue::from(AudioContainerFormat::AUDIO_CONTAINER_FORMAT_FLAC),
    stream_count: 1,
    title: "Sparse".into(),
    artist: "Nobody".into(),
    album_artist: String::new(),
    album: String::new(),
    genre: String::new(),
    composer: String::new(),
    performer: String::new(),
    date: String::new(),
    track_number: 1,
    total_tracks: 1,
    disc_number: 1,
    total_discs: 1,
    comment: String::new(),
    lyrics: String::new(),
    tag_types: vec![],
    cover_art: buffa::MessageField::none(),
    created_at: 0,
    ..Default::default()
  };
  rt(&afr_sparse);
  rt(&AudioFileRecord::default());
}

#[test]
#[cfg(feature = "json")]
fn batch10_sp2_json_roundtrip() {
  let afr = AudioFileRecord {
    id: vec![1, 2].into(),
    checksum: Some((1u8..=32).collect()),
    name: "track.flac".into(),
    format: buffa::EnumValue::from(AudioFormat::AUDIO_FORMAT_FLAC),
    size: 52_428_800,
    total_pts: 2_646_000,
    frame_rate: 23.976,
    bit_rate: 1_411_200,
    time_base: buffa::MessageField::some(mediatime::Timebase::new(
      1,
      NonZeroU32::new(44100).unwrap(),
    )),
    container_format: buffa::EnumValue::from(AudioContainerFormat::AUDIO_CONTAINER_FORMAT_FLAC),
    stream_count: 1,
    title: "Song Title".into(),
    artist: "Artist Name".into(),
    album_artist: "Album Artist".into(),
    album: "Album Name".into(),
    genre: "Rock".into(),
    composer: "Composer Name".into(),
    performer: "Performer Name".into(),
    date: "2023-01-01".into(),
    track_number: 3,
    total_tracks: 12,
    disc_number: 1,
    total_discs: 2,
    comment: "A great track".into(),
    lyrics: "La la la...".into(),
    tag_types: vec!["ID3v2".into(), "Vorbis".into()],
    cover_art: buffa::MessageField::some(AudioCoverArt {
      path: buffa::MessageField::some(make_local_location()),
      mime: "image/jpeg".into(),
      dimensions: buffa::MessageField::some(Dimensions {
        width: 600,
        height: 600,
        ..Default::default()
      }),
      size: 40_000,
      ..Default::default()
    }),
    created_at: 1700,
    ..Default::default()
  };
  let json = serde_json::to_string(&afr).expect("to_json");
  let back: AudioFileRecord = serde_json::from_str(&json).expect("from_json");
  assert_eq!(afr, back);
}

// ── SP3 Task 0: three-way cross-package + extern codegen smoke ──────────────

#[test]
fn sp3_codegen_smoke_roundtrip() {
  let s = Sp3CodegenSmoke {
    id: vec![0x01, 0x02, 0x03, 0x04].into(),
    error: make_error_info(),
    video_meta: make_video_meta(),
    range: buffa::MessageField::some(mediatime::TimeRange::new(
      10,
      20,
      mediatime::Timebase::new(30000, core::num::NonZeroU32::new(1001).unwrap()),
    )),
    ..Default::default()
  };
  rt(&s);
  rt(&Sp3CodegenSmoke::default());
}

#[test]
#[cfg(feature = "json")]
fn sp3_codegen_smoke_json_roundtrip() {
  let s = Sp3CodegenSmoke {
    id: vec![0xAA, 0xBB].into(),
    error: make_error_info(),
    video_meta: make_video_meta(),
    range: buffa::MessageField::some(mediatime::TimeRange::new(
      0,
      5,
      mediatime::Timebase::new(1, core::num::NonZeroU32::new(48000).unwrap()),
    )),
    ..Default::default()
  };
  let json = serde_json::to_string(&s).expect("to_json");
  let back: Sp3CodegenSmoke = serde_json::from_str(&json).expect("from_json");
  assert_eq!(s, back);
}

// ── SP3 Batch 1: scalar leaves ──────────────────────────────────────────────

#[test]
fn batch1_sp3_roundtrip() {
  rt(&Pagination {
    limit: 25,
    offset: 100,
    ..Default::default()
  });
  rt(&Pagination::default()); // proto Default = limit/offset 0 (zero-values omitted on wire); client treats absent limit as 50 (§7.8 #9)
  rt(&SearchFilter {
    key: "scene".into(),
    value: "beach".into(),
    weight: 0.5,
    ..Default::default()
  });
  rt(&SearchFilter::default()); // proto Default = weight 0.0 (zero-value omitted on wire); client treats absent weight as 1.0 (§7.8 #9)
  rt(&HeartbeatRequest {
    timestamp: 1_700_000_000_123,
    ..Default::default()
  });
  rt(&HeartbeatRequest::default());
  rt(&HeartbeatResponse {
    timestamp: 1_700_000_000_456,
    ..Default::default()
  });
  rt(&HeartbeatResponse::default());
}

#[test]
#[cfg(feature = "json")]
fn batch1_sp3_json_roundtrip() {
  let f = SearchFilter {
    key: "tag".into(),
    value: "sunset".into(),
    weight: 2.5,
    ..Default::default()
  };
  let json = serde_json::to_string(&f).expect("to_json");
  let back: SearchFilter = serde_json::from_str(&json).expect("from_json");
  assert_eq!(f, back);
}

// ── SP3 Batch 2: reuse-only leaves ──────────────────────────────────────────

#[test]
fn batch2_sp3_roundtrip() {
  // ── SearchHit: fully populated ───────────────────────────────────────────
  rt(&SearchHit {
    scene_id: vec![0x01, 0x02, 0x03].into(),
    video_id: vec![0x04, 0x05, 0x06].into(),
    video_name: "clip.mp4".into(),
    location: buffa::MessageField::some(make_local_location()),
    description: "sunset on the beach".into(),
    score: 0.87,
    range: buffa::MessageField::some(mediatime::TimeRange::new(
      10,
      20,
      mediatime::Timebase::new(30000, core::num::NonZeroU32::new(1001).unwrap()),
    )),
    thumbnail: vec![0xFF, 0xD8, 0xFF].into(),
    dimensions: buffa::MessageField::some(Dimensions {
      width: 1920,
      height: 1080,
      ..Default::default()
    }),
    ..Default::default()
  });
  // SearchHit: optional fields absent, empty bytes/strings
  rt(&SearchHit {
    scene_id: vec![].into(),
    video_id: vec![].into(),
    video_name: "".into(),
    location: buffa::MessageField::none(),
    description: "".into(),
    score: 0.0,
    range: buffa::MessageField::none(),
    thumbnail: vec![].into(),
    dimensions: buffa::MessageField::none(),
    ..Default::default()
  });
  rt(&SearchHit::default());

  // ── BrowseItem: fully populated ──────────────────────────────────────────
  rt(&BrowseItem {
    meta: make_video_meta(),
    location: buffa::MessageField::some(make_local_location()),
    scene_count: 42,
    thumbnail: vec![0xAB, 0xCD].into(),
    ..Default::default()
  });
  // BrowseItem: meta/location absent, empty thumbnail
  rt(&BrowseItem {
    meta: buffa::MessageField::none(),
    location: buffa::MessageField::none(),
    scene_count: 0,
    thumbnail: vec![].into(),
    ..Default::default()
  });
  rt(&BrowseItem::default());

  // ── ModelInfo: known status values ───────────────────────────────────────
  rt(&ModelInfo {
    name: "qwen".into(),
    status: 2,
    size_bytes: 1_000_000,
    ..Default::default()
  });
  // ModelInfo: status=99 — out-of-table value proving plain-uint32 §7.3 round-trips exactly
  rt(&ModelInfo {
    name: "qwen".into(),
    status: 99,
    size_bytes: 0,
    ..Default::default()
  });
  rt(&ModelInfo::default());

  // ── ModelDownloadProgress: known status ──────────────────────────────────
  rt(&ModelDownloadProgress {
    name: "siglip".into(),
    progress: 0.73,
    downloaded_bytes: 500,
    total_bytes: 1000,
    status: 4,
    error_msg: "".into(),
    ..Default::default()
  });
  // ModelDownloadProgress: status=250 — out-of-table value proving §7.3
  rt(&ModelDownloadProgress {
    name: "siglip".into(),
    progress: 0.0,
    downloaded_bytes: 0,
    total_bytes: 0,
    status: 250,
    error_msg: "".into(),
    ..Default::default()
  });
  rt(&ModelDownloadProgress::default());

  // ── NetFailedFile: fully populated ───────────────────────────────────────
  rt(&NetFailedFile {
    kind: 1,
    location: buffa::MessageField::some(make_local_location()),
    error: make_error_info(),
    error_status: 0x01 | 0x02,
    index_status: 0x01 | 0x80,
    ..Default::default()
  });
  // NetFailedFile: kind=7 — out-of-table value proving §7.3; location/error absent
  rt(&NetFailedFile {
    kind: 7,
    location: buffa::MessageField::none(),
    error: buffa::MessageField::none(),
    error_status: 0,
    index_status: 0,
    ..Default::default()
  });
  rt(&NetFailedFile::default());

  // ── IndexingFile: fully populated ────────────────────────────────────────
  rt(&IndexingFile {
    location: buffa::MessageField::some(make_local_location()),
    name: "a.mp4".into(),
    completed_phases: 0x01 | 0x04 | 0x40,
    ..Default::default()
  });
  // IndexingFile: location absent, empty name, zero phases
  rt(&IndexingFile {
    location: buffa::MessageField::none(),
    name: "".into(),
    completed_phases: 0,
    ..Default::default()
  });
  rt(&IndexingFile::default());
}

#[test]
#[cfg(feature = "json")]
fn batch2_sp3_json_roundtrip() {
  // SearchHit: covers media.v1.Location + media.v1.Dimensions + mediatime.v1.TimeRange extern
  // + inline bytes under serde
  let sh = SearchHit {
    scene_id: vec![0x01, 0x02].into(),
    video_id: vec![0x03, 0x04].into(),
    video_name: "beach.mp4".into(),
    location: buffa::MessageField::some(make_local_location()),
    description: "golden hour".into(),
    score: 0.95,
    range: buffa::MessageField::some(mediatime::TimeRange::new(
      5,
      15,
      mediatime::Timebase::new(30000, core::num::NonZeroU32::new(1001).unwrap()),
    )),
    thumbnail: vec![0xAA, 0xBB].into(),
    dimensions: buffa::MessageField::some(Dimensions {
      width: 1280,
      height: 720,
      ..Default::default()
    }),
    ..Default::default()
  };
  let json = serde_json::to_string(&sh).expect("to_json");
  let back: SearchHit = serde_json::from_str(&json).expect("from_json");
  assert_eq!(sh, back);

  // BrowseItem: covers VideoMeta (same-package since mono-consolidation) under serde
  let bi = BrowseItem {
    meta: make_video_meta(),
    location: buffa::MessageField::some(make_local_location()),
    scene_count: 7,
    thumbnail: vec![0x11, 0x22].into(),
    ..Default::default()
  };
  let json = serde_json::to_string(&bi).expect("to_json");
  let back: BrowseItem = serde_json::from_str(&json).expect("from_json");
  assert_eq!(bi, back);
}

// ── SP3 Batch 3: empty + simple request/response leaves ──────────────────────

#[test]
fn batch3_sp3_roundtrip() {
  // empty message: populated == default (zero fields)
  rt(&ListLocationsRequest::default());
  // empty message: populated == default (zero fields)
  rt(&RemoveLocationResponse::default());
  // empty message: populated == default (zero fields)
  rt(&RetryFailedResponse::default());
  // empty message: populated == default (zero fields)
  rt(&EjectVolumeResponse::default());
  // empty message: populated == default (zero fields)
  rt(&GetModelStatusRequest::default());
  // empty message: populated == default (zero fields)
  rt(&GetDaemonInfoRequest::default());
  // empty message: populated == default (zero fields)
  rt(&UpdateAnnotationResponse::default());

  // EjectVolumeRequest: non-empty bytes + default
  rt(&EjectVolumeRequest {
    volume_id: vec![1, 2, 3].into(),
    ..Default::default()
  });
  rt(&EjectVolumeRequest::default());

  // GetIndexedFileRequest: non-empty bytes + default
  rt(&GetIndexedFileRequest {
    checksum: vec![1, 2, 3].into(),
    ..Default::default()
  });
  rt(&GetIndexedFileRequest::default());

  // GetFileIndexingStatsRequest: non-empty bytes + default
  rt(&GetFileIndexingStatsRequest {
    video_id: vec![1, 2, 3].into(),
    ..Default::default()
  });
  rt(&GetFileIndexingStatsRequest::default());

  // GetLocationStatsRequest: location present + none + default
  rt(&GetLocationStatsRequest {
    location: buffa::MessageField::some(make_local_location()),
    ..Default::default()
  });
  rt(&GetLocationStatsRequest {
    location: buffa::MessageField::none(),
    ..Default::default()
  });
  rt(&GetLocationStatsRequest::default());

  // RemoveLocationRequest: location present + none + default
  rt(&RemoveLocationRequest {
    location: buffa::MessageField::some(make_local_location()),
    ..Default::default()
  });
  rt(&RemoveLocationRequest {
    location: buffa::MessageField::none(),
    ..Default::default()
  });
  rt(&RemoveLocationRequest::default());

  // RetryFailedRequest: location present + none + default
  rt(&RetryFailedRequest {
    location: buffa::MessageField::some(make_local_location()),
    ..Default::default()
  });
  rt(&RetryFailedRequest {
    location: buffa::MessageField::none(),
    ..Default::default()
  });
  rt(&RetryFailedRequest::default());

  // BrowseRequest: both present + both none + default
  rt(&BrowseRequest {
    location: buffa::MessageField::some(make_local_location()),
    pagination: buffa::MessageField::some(Pagination {
      limit: 10,
      offset: 0,
      ..Default::default()
    }),
    ..Default::default()
  });
  rt(&BrowseRequest {
    location: buffa::MessageField::none(),
    pagination: buffa::MessageField::none(),
    ..Default::default()
  });
  rt(&BrowseRequest::default());

  // IndexLocationRequest: target with Local arm set + none + default
  rt(&IndexLocationRequest {
    target: buffa::MessageField::some(LocationTarget {
      kind: Some(LocationTargetKind::Local("/tmp/media".into())),
      ..Default::default()
    }),
    ..Default::default()
  });
  rt(&IndexLocationRequest {
    target: buffa::MessageField::none(),
    ..Default::default()
  });
  rt(&IndexLocationRequest::default());

  // IndexLocationResponse: folder with full WatchedLocation + none + default
  rt(&IndexLocationResponse {
    folder: buffa::MessageField::some(WatchedLocation {
      id: buffa::MessageField::some(Id {
        value: (1u8..=16).collect(),
        ..Default::default()
      }),
      location: buffa::MessageField::some(make_local_location()),
      name: "My Videos".into(),
      status: 3,
      created_at: 1_700_000_000,
      deleted_at: Some(1_800_000_000),
      total_files: 1000,
      indexed_files: 950,
      total_videos: 800,
      indexed_videos: 780,
      total_scenes: 5000,
      total_audios: 200,
      indexed_audios: 195,
      total_failed_files: 50,
      failed_videos: 20,
      failed_audios: 5,
      ..Default::default()
    }),
    ..Default::default()
  });
  rt(&IndexLocationResponse {
    folder: buffa::MessageField::none(),
    ..Default::default()
  });
  rt(&IndexLocationResponse::default());

  // UpdateAnnotationRequest: scene_ids + user_tags populated; empty-both case; default
  rt(&UpdateAnnotationRequest {
    scene_ids: vec![vec![1].into(), vec![2].into()],
    user_tags: vec![Tag {
      name: "favorite".into(),
      color: 0xFF_AA_00_FF,
      ..Default::default()
    }],
    ..Default::default()
  });
  rt(&UpdateAnnotationRequest {
    scene_ids: vec![],
    user_tags: vec![],
    ..Default::default()
  });
  rt(&UpdateAnnotationRequest::default());

  // SearchResponse: non-trivial + default
  rt(&SearchResponse {
    search_id: vec![9, 9].into(),
    total_count: 123,
    ..Default::default()
  });
  rt(&SearchResponse::default());

  // GetDaemonInfoResponse: all 5 fields non-trivial + default
  rt(&GetDaemonInfoResponse {
    version: "1.2.3".into(),
    started_at: 1_700_000_000,
    total_videos: 42_000,
    total_scenes: 500_000,
    active_tasks: 7,
    ..Default::default()
  });
  rt(&GetDaemonInfoResponse::default());
}

#[test]
#[cfg(feature = "json")]
fn batch3_sp3_json_roundtrip() {
  // IndexLocationResponse: covers media.v1.WatchedLocation cross-package +
  // its nested media.v1.Location oneof under serde
  let ilr = IndexLocationResponse {
    folder: buffa::MessageField::some(WatchedLocation {
      id: buffa::MessageField::some(Id {
        value: (1u8..=16).collect(),
        ..Default::default()
      }),
      location: buffa::MessageField::some(make_local_location()),
      name: "JSON Test Location".into(),
      status: 3,
      created_at: 1_700_000_000,
      deleted_at: Some(1_800_000_000),
      total_files: 1000,
      indexed_files: 950,
      total_videos: 800,
      indexed_videos: 780,
      total_scenes: 5000,
      total_audios: 200,
      indexed_audios: 195,
      total_failed_files: 50,
      failed_videos: 20,
      failed_audios: 5,
      ..Default::default()
    }),
    ..Default::default()
  };
  let json = serde_json::to_string(&ilr).expect("to_json");
  let back: IndexLocationResponse = serde_json::from_str(&json).expect("from_json");
  assert_eq!(ilr, back);

  // UpdateAnnotationRequest: covers repeated bytes + repeated media.v1.Tag under serde
  let uar = UpdateAnnotationRequest {
    scene_ids: vec![vec![1].into(), vec![2].into()],
    user_tags: vec![Tag {
      name: "favorite".into(),
      color: 0xFF_AA_00_FF,
      ..Default::default()
    }],
    ..Default::default()
  };
  let json = serde_json::to_string(&uar).expect("to_json");
  let back: UpdateAnnotationRequest = serde_json::from_str(&json).expect("from_json");
  assert_eq!(uar, back);
}

// ── SP3 Batch 4: composite responses ────────────────────────────────────────

#[test]
fn batch4_sp3_roundtrip() {
  // ── Volume ───────────────────────────────────────────────────────────────
  let vol_populated = Volume {
    meta: buffa::MessageField::some(VolumeMeta {
      id: buffa::MessageField::some(Id {
        value: (1u8..=16).collect(),
        ..Default::default()
      }),
      location: buffa::MessageField::some(make_local_location()),
      name: "Seagate 4TB".into(),
      total_size: 4_000_000_000_000,
      used_size: 2_500_000_000_000,
      status: 3,
      ..Default::default()
    }),
    folders: vec![WatchedLocation {
      id: buffa::MessageField::some(Id {
        value: (1u8..=16).collect(),
        ..Default::default()
      }),
      location: buffa::MessageField::some(make_local_location()),
      name: "My Videos".into(),
      status: 3,
      created_at: 1_700_000_000,
      deleted_at: Some(1_800_000_000),
      total_files: 1000,
      indexed_files: 950,
      total_videos: 800,
      indexed_videos: 780,
      total_scenes: 5000,
      total_audios: 200,
      indexed_audios: 195,
      total_failed_files: 50,
      failed_videos: 20,
      failed_audios: 5,
      ..Default::default()
    }],
    ..Default::default()
  };
  rt(&vol_populated);
  rt(&Volume {
    meta: buffa::MessageField::none(),
    folders: vec![],
    ..Default::default()
  });
  rt(&Volume::default());

  // ── ListLocationsResponse ────────────────────────────────────────────────
  rt(&ListLocationsResponse {
    groups: vec![vol_populated.clone()],
    ..Default::default()
  });
  rt(&ListLocationsResponse {
    groups: vec![],
    ..Default::default()
  });
  rt(&ListLocationsResponse::default());

  // ── GetLocationStatsResponse ─────────────────────────────────────────────
  rt(&GetLocationStatsResponse {
    total_files: 1000,
    indexed_files: 950,
    total_videos: 800,
    total_scenes: 5000,
    total_audios: 200,
    failed_files: vec![NetFailedFile {
      kind: 0,
      location: buffa::MessageField::some(make_local_location()),
      error: make_error_info(),
      error_status: 0x01,
      index_status: 0x02,
      ..Default::default()
    }],
    ..Default::default()
  });
  rt(&GetLocationStatsResponse {
    total_files: 0,
    indexed_files: 0,
    total_videos: 0,
    total_scenes: 0,
    total_audios: 0,
    failed_files: vec![],
    ..Default::default()
  });
  rt(&GetLocationStatsResponse::default());

  // ── FailedFilesResponse ───────────────────────────────────────────────────
  rt(&FailedFilesResponse {
    location_id: vec![7, 7].into(),
    failed_files: vec![NetFailedFile {
      kind: 0,
      location: buffa::MessageField::some(make_local_location()),
      error: make_error_info(),
      error_status: 0x01,
      index_status: 0x02,
      ..Default::default()
    }],
    ..Default::default()
  });
  rt(&FailedFilesResponse {
    location_id: vec![].into(),
    failed_files: vec![],
    ..Default::default()
  });
  rt(&FailedFilesResponse::default());

  // ── SearchRequest ─────────────────────────────────────────────────────────
  rt(&SearchRequest {
    query: "beach".into(),
    pagination: buffa::MessageField::some(Pagination {
      limit: 20,
      offset: 40,
      ..Default::default()
    }),
    filters: vec![SearchFilter {
      key: "k".into(),
      value: "v".into(),
      weight: 0.8,
      ..Default::default()
    }],
    ..Default::default()
  });
  rt(&SearchRequest {
    query: "".into(),
    pagination: buffa::MessageField::none(),
    filters: vec![],
    ..Default::default()
  });
  rt(&SearchRequest::default());

  // ── BrowseResponse ────────────────────────────────────────────────────────
  rt(&BrowseResponse {
    items: vec![BrowseItem {
      meta: make_video_meta(),
      location: buffa::MessageField::some(make_local_location()),
      scene_count: 3,
      thumbnail: vec![1].into(),
      ..Default::default()
    }],
    total_count: 1,
    pagination: buffa::MessageField::some(Pagination {
      limit: 50,
      offset: 0,
      ..Default::default()
    }),
    ..Default::default()
  });
  rt(&BrowseResponse {
    items: vec![],
    total_count: 0,
    pagination: buffa::MessageField::none(),
    ..Default::default()
  });
  rt(&BrowseResponse::default());

  // ── GetIndexedFileResponse ────────────────────────────────────────────────
  rt(&GetIndexedFileResponse {
    video: buffa::MessageField::some(Video {
      meta: make_video_meta(),
      scenes: vec![vec![1].into()],
      index_status: 0x01 | 0x80,
      index_error: make_error_info(),
      error_status: 1,
      ..Default::default()
    }),
    scenes: vec![Scene {
      meta: make_scene_meta(),
      keyframes: vec![vec![1].into()],
      description: "ocean coast".into(),
      shot_type: "wide".into(),
      camera_motion: "pan".into(),
      tags: "beach,sunset".into(),
      people_count: 2,
      tag_ids: vec![vec![9].into()],
      vision_provider: vec!["apple".into()],
      smart_folders: vec!["fav".into()],
      ..Default::default()
    }],
    ..Default::default()
  });
  rt(&GetIndexedFileResponse {
    video: buffa::MessageField::none(),
    scenes: vec![],
    ..Default::default()
  });
  rt(&GetIndexedFileResponse::default());

  // ── GetFileIndexingStatsResponse ──────────────────────────────────────────
  rt(&GetFileIndexingStatsResponse {
    video_id: vec![1].into(),
    index_status: 0x01 | 0x04,
    error: make_error_info(),
    ..Default::default()
  });
  rt(&GetFileIndexingStatsResponse {
    video_id: vec![2].into(),
    index_status: 0,
    error: buffa::MessageField::none(),
    ..Default::default()
  });
  rt(&GetFileIndexingStatsResponse::default());

  // ── GetModelStatusResponse ────────────────────────────────────────────────
  rt(&GetModelStatusResponse {
    models: vec![ModelInfo {
      name: "m".into(),
      status: 2,
      size_bytes: 1024,
      ..Default::default()
    }],
    ..Default::default()
  });
  rt(&GetModelStatusResponse {
    models: vec![],
    ..Default::default()
  });
  rt(&GetModelStatusResponse::default());

  // ── ModelDownloadProgressResponse ─────────────────────────────────────────
  rt(&ModelDownloadProgressResponse {
    model: buffa::MessageField::some(ModelDownloadProgress {
      name: "m".into(),
      progress: 0.5,
      downloaded_bytes: 1,
      total_bytes: 2,
      status: 1,
      error_msg: "".into(),
      ..Default::default()
    }),
    ..Default::default()
  });
  rt(&ModelDownloadProgressResponse {
    model: buffa::MessageField::none(),
    ..Default::default()
  });
  rt(&ModelDownloadProgressResponse::default());

  // ── IndexingProgressResponse ──────────────────────────────────────────────
  rt(&IndexingProgressResponse {
    location: buffa::MessageField::some(make_local_location()),
    total_files: 100,
    indexed_files: 50,
    active_files: vec![IndexingFile {
      location: buffa::MessageField::some(make_local_location()),
      name: "a".into(),
      completed_phases: 0x01,
      ..Default::default()
    }],
    ..Default::default()
  });
  rt(&IndexingProgressResponse {
    location: buffa::MessageField::none(),
    total_files: 0,
    indexed_files: 0,
    active_files: vec![],
    ..Default::default()
  });
  rt(&IndexingProgressResponse::default());
}

#[test]
#[cfg(feature = "json")]
fn batch4_sp3_json_roundtrip() {
  // GetIndexedFileResponse: heaviest SP3 serde — Video + repeated Scene (same-package since mono-consolidation)
  let gifr = GetIndexedFileResponse {
    video: buffa::MessageField::some(Video {
      meta: make_video_meta(),
      scenes: vec![vec![1].into(), vec![2].into()],
      index_status: 0x01 | 0x80,
      index_error: make_error_info(),
      error_status: 1,
      ..Default::default()
    }),
    scenes: vec![Scene {
      meta: make_scene_meta(),
      keyframes: vec![vec![1].into()],
      description: "ocean coast".into(),
      shot_type: "wide".into(),
      camera_motion: "pan".into(),
      tags: "beach,sunset".into(),
      people_count: 3,
      tag_ids: vec![vec![9].into()],
      vision_provider: vec!["apple".into()],
      smart_folders: vec!["fav".into()],
      ..Default::default()
    }],
    ..Default::default()
  };
  let json = serde_json::to_string(&gifr).expect("to_json");
  let back: GetIndexedFileResponse = serde_json::from_str(&json).expect("from_json");
  assert_eq!(gifr, back);

  // ListLocationsResponse: nested Volume -> media.v1.VolumeMeta / media.v1.WatchedLocation
  let llr = ListLocationsResponse {
    groups: vec![Volume {
      meta: buffa::MessageField::some(VolumeMeta {
        id: buffa::MessageField::some(Id {
          value: (1u8..=16).collect(),
          ..Default::default()
        }),
        location: buffa::MessageField::some(make_local_location()),
        name: "Seagate 4TB".into(),
        total_size: 4_000_000_000_000,
        used_size: 2_500_000_000_000,
        status: 3,
        ..Default::default()
      }),
      folders: vec![WatchedLocation {
        id: buffa::MessageField::some(Id {
          value: (1u8..=16).collect(),
          ..Default::default()
        }),
        location: buffa::MessageField::some(make_local_location()),
        name: "My Videos".into(),
        status: 3,
        created_at: 1_700_000_000,
        deleted_at: Some(1_800_000_000),
        total_files: 1000,
        indexed_files: 950,
        total_videos: 800,
        indexed_videos: 780,
        total_scenes: 5000,
        total_audios: 200,
        indexed_audios: 195,
        total_failed_files: 50,
        failed_videos: 20,
        failed_audios: 5,
        ..Default::default()
      }],
      ..Default::default()
    }],
    ..Default::default()
  };
  let json = serde_json::to_string(&llr).expect("to_json");
  let back: ListLocationsResponse = serde_json::from_str(&json).expect("from_json");
  assert_eq!(llr, back);

  // SearchRequest: nested Pagination + SearchFilter
  let sr = SearchRequest {
    query: "beach".into(),
    pagination: buffa::MessageField::some(Pagination {
      limit: 20,
      offset: 40,
      ..Default::default()
    }),
    filters: vec![SearchFilter {
      key: "k".into(),
      value: "v".into(),
      weight: 0.8,
      ..Default::default()
    }],
    ..Default::default()
  };
  let json = serde_json::to_string(&sr).expect("to_json");
  let back: SearchRequest = serde_json::from_str(&json).expect("from_json");
  assert_eq!(sr, back);
}

#[test]
fn batch5_sp3_roundtrip() {
  // ── VolumeStateChangedEvent ───────────────────────────────────────────────
  rt(&VolumeStateChangedEvent {
    volume: buffa::MessageField::some(VolumeMeta {
      id: buffa::MessageField::some(Id {
        value: (1u8..=16).collect(),
        ..Default::default()
      }),
      location: buffa::MessageField::some(make_local_location()),
      name: "Seagate 4TB".into(),
      total_size: 4_000_000_000_000,
      used_size: 2_500_000_000_000,
      status: 3,
      ..Default::default()
    }),
    event: 2,
    ..Default::default()
  });
  // OUT-OF-TABLE event: 99 — proves §7.3 bare-uint32 round-trips undocumented value
  rt(&VolumeStateChangedEvent {
    volume: buffa::MessageField::none(),
    event: 99,
    ..Default::default()
  });
  rt(&VolumeStateChangedEvent::default());

  // ── FolderUpdatedEvent ────────────────────────────────────────────────────
  rt(&FolderUpdatedEvent {
    folder_location: buffa::MessageField::some(make_local_location()),
    path: "/a/b.mp4".into(),
    event: 1,
    ..Default::default()
  });
  // OUT-OF-TABLE event: 200 — proves §7.3 bare-uint32 round-trips undocumented value
  rt(&FolderUpdatedEvent {
    folder_location: buffa::MessageField::none(),
    path: "".into(),
    event: 200,
    ..Default::default()
  });
  rt(&FolderUpdatedEvent::default());

  // ── ModelDownloadProgressEvent ────────────────────────────────────────────
  rt(&ModelDownloadProgressEvent {
    model: buffa::MessageField::some(ModelDownloadProgress {
      name: "m".into(),
      progress: 0.9,
      downloaded_bytes: 9,
      total_bytes: 10,
      status: 1,
      error_msg: "".into(),
      ..Default::default()
    }),
    ..Default::default()
  });
  rt(&ModelDownloadProgressEvent {
    model: buffa::MessageField::none(),
    ..Default::default()
  });
  rt(&ModelDownloadProgressEvent::default());
}

#[test]
#[cfg(feature = "json")]
fn batch5_sp3_json_roundtrip() {
  // VolumeStateChangedEvent: covers media.v1.VolumeMeta cross-package + bare-uint32 event under serde
  let vse = VolumeStateChangedEvent {
    volume: buffa::MessageField::some(VolumeMeta {
      id: buffa::MessageField::some(Id {
        value: (1u8..=16).collect(),
        ..Default::default()
      }),
      location: buffa::MessageField::some(make_local_location()),
      name: "Seagate 4TB".into(),
      total_size: 4_000_000_000_000,
      used_size: 2_500_000_000_000,
      status: 3,
      ..Default::default()
    }),
    event: 2,
    ..Default::default()
  };
  let json = serde_json::to_string(&vse).expect("to_json");
  let back: VolumeStateChangedEvent = serde_json::from_str(&json).expect("from_json");
  assert_eq!(vse, back);

  // FolderUpdatedEvent: covers media.v1.Location oneof under serde
  let fue = FolderUpdatedEvent {
    folder_location: buffa::MessageField::some(make_local_location()),
    path: "/a/b.mp4".into(),
    event: 1,
    ..Default::default()
  };
  let json = serde_json::to_string(&fue).expect("to_json");
  let back: FolderUpdatedEvent = serde_json::from_str(&json).expect("from_json");
  assert_eq!(fue, back);
}

// ── SP3 Batch 6: Request oneof envelope ─────────────────────────────────────

#[test]
fn batch6_sp3_roundtrip() {
  // ── all 14 arms individually ─────────────────────────────────────────────
  rt(&Request {
    kind: Some(RequestKind::Heartbeat(Box::new(HeartbeatRequest {
      timestamp: 1,
      ..Default::default()
    }))),
    ..Default::default()
  });
  rt(&Request {
    kind: Some(RequestKind::Search(Box::new(SearchRequest {
      query: "q".into(),
      pagination: buffa::MessageField::none(),
      filters: vec![],
      ..Default::default()
    }))),
    ..Default::default()
  });
  rt(&Request {
    kind: Some(RequestKind::Browse(Box::new(BrowseRequest {
      location: buffa::MessageField::none(),
      pagination: buffa::MessageField::none(),
      ..Default::default()
    }))),
    ..Default::default()
  });
  rt(&Request {
    kind: Some(RequestKind::GetLocationStats(Box::new(
      GetLocationStatsRequest {
        location: buffa::MessageField::none(),
        ..Default::default()
      },
    ))),
    ..Default::default()
  });
  rt(&Request {
    kind: Some(RequestKind::ListLocations(Box::new(
      ListLocationsRequest::default(),
    ))),
    ..Default::default()
  });
  rt(&Request {
    kind: Some(RequestKind::GetIndexedFile(Box::new(
      GetIndexedFileRequest {
        checksum: vec![1].into(),
        ..Default::default()
      },
    ))),
    ..Default::default()
  });
  rt(&Request {
    kind: Some(RequestKind::GetFileIndexingStats(Box::new(
      GetFileIndexingStatsRequest {
        video_id: vec![1].into(),
        ..Default::default()
      },
    ))),
    ..Default::default()
  });
  rt(&Request {
    kind: Some(RequestKind::GetModelStatus(Box::new(
      GetModelStatusRequest::default(),
    ))),
    ..Default::default()
  });
  rt(&Request {
    kind: Some(RequestKind::GetDaemonInfo(Box::new(
      GetDaemonInfoRequest::default(),
    ))),
    ..Default::default()
  });
  rt(&Request {
    kind: Some(RequestKind::IndexLocation(Box::new(IndexLocationRequest {
      target: buffa::MessageField::none(),
      ..Default::default()
    }))),
    ..Default::default()
  });
  rt(&Request {
    kind: Some(RequestKind::RemoveLocation(Box::new(
      RemoveLocationRequest {
        location: buffa::MessageField::none(),
        ..Default::default()
      },
    ))),
    ..Default::default()
  });
  rt(&Request {
    kind: Some(RequestKind::UpdateAnnotation(Box::new(
      UpdateAnnotationRequest {
        scene_ids: vec![vec![1].into()],
        user_tags: vec![],
        ..Default::default()
      },
    ))),
    ..Default::default()
  });
  rt(&Request {
    kind: Some(RequestKind::EjectVolume(Box::new(EjectVolumeRequest {
      volume_id: vec![1].into(),
      ..Default::default()
    }))),
    ..Default::default()
  });
  rt(&Request {
    kind: Some(RequestKind::RetryFailed(Box::new(RetryFailedRequest {
      location: buffa::MessageField::some(make_local_location()),
      ..Default::default()
    }))),
    ..Default::default()
  });

  // ── no-arm default (empty oneof round-trips) ─────────────────────────────
  rt(&Request::default());

  // ── fully-populated Search arm ────────────────────────────────────────────
  rt(&Request {
    kind: Some(RequestKind::Search(Box::new(SearchRequest {
      query: "beach sunset".into(),
      pagination: buffa::MessageField::some(Pagination {
        limit: 20,
        offset: 40,
        ..Default::default()
      }),
      filters: vec![SearchFilter {
        key: "type".into(),
        value: "video".into(),
        weight: 0.9,
        ..Default::default()
      }],
      ..Default::default()
    }))),
    ..Default::default()
  });

  // ── fully-populated UpdateAnnotation arm ──────────────────────────────────
  rt(&Request {
    kind: Some(RequestKind::UpdateAnnotation(Box::new(
      UpdateAnnotationRequest {
        scene_ids: vec![vec![1].into()],
        user_tags: vec![Tag {
          name: "favorite".into(),
          color: 0xFF_AA_00_FF,
          ..Default::default()
        }],
        ..Default::default()
      },
    ))),
    ..Default::default()
  });
}

#[test]
#[cfg(feature = "json")]
fn batch6_sp3_json_roundtrip() {
  // Search arm: covers nested Pagination + SearchFilter under serde
  let req_search = Request {
    kind: Some(RequestKind::Search(Box::new(SearchRequest {
      query: "beach sunset".into(),
      pagination: buffa::MessageField::some(Pagination {
        limit: 20,
        offset: 40,
        ..Default::default()
      }),
      filters: vec![SearchFilter {
        key: "type".into(),
        value: "video".into(),
        weight: 0.9,
        ..Default::default()
      }],
      ..Default::default()
    }))),
    ..Default::default()
  };
  let json = serde_json::to_string(&req_search).expect("to_json");
  let back: Request = serde_json::from_str(&json).expect("from_json");
  assert_eq!(req_search, back);

  // RetryFailed arm (tag 2005): proves the sparse high arm-tag survives serde
  let req_retry = Request {
    kind: Some(RequestKind::RetryFailed(Box::new(RetryFailedRequest {
      location: buffa::MessageField::some(make_local_location()),
      ..Default::default()
    }))),
    ..Default::default()
  };
  let json = serde_json::to_string(&req_retry).expect("to_json");
  let back: Request = serde_json::from_str(&json).expect("from_json");
  assert_eq!(req_retry, back);
}

#[test]
fn batch7_sp3_roundtrip() {
  // ── all 15 arms individually ─────────────────────────────────────────────
  rt(&Response {
    kind: Some(ResponseKind::Heartbeat(Box::new(HeartbeatResponse {
      timestamp: 1,
      ..Default::default()
    }))),
    ..Default::default()
  });
  rt(&Response {
    kind: Some(ResponseKind::Search(Box::new(SearchResponse {
      search_id: vec![1].into(),
      total_count: 1,
      ..Default::default()
    }))),
    ..Default::default()
  });
  rt(&Response {
    kind: Some(ResponseKind::Browse(Box::new(BrowseResponse {
      items: vec![],
      total_count: 0,
      pagination: buffa::MessageField::none(),
      ..Default::default()
    }))),
    ..Default::default()
  });
  rt(&Response {
    kind: Some(ResponseKind::GetIndexedFile(Box::new(
      GetIndexedFileResponse {
        video: buffa::MessageField::some(Video {
          meta: make_video_meta(),
          scenes: vec![vec![1].into()],
          index_status: 0x01 | 0x80,
          index_error: make_error_info(),
          error_status: 1,
          ..Default::default()
        }),
        scenes: vec![Scene {
          meta: make_scene_meta(),
          keyframes: vec![vec![1].into()],
          description: "ocean coast".into(),
          shot_type: "wide".into(),
          camera_motion: "pan".into(),
          tags: "beach,sunset".into(),
          people_count: 2,
          tag_ids: vec![vec![9].into()],
          vision_provider: vec!["apple".into()],
          smart_folders: vec!["fav".into()],
          ..Default::default()
        }],
        ..Default::default()
      },
    ))),
    ..Default::default()
  });
  rt(&Response {
    kind: Some(ResponseKind::ListLocations(Box::new(
      ListLocationsResponse::default(),
    ))),
    ..Default::default()
  });
  rt(&Response {
    kind: Some(ResponseKind::GetLocationStats(Box::new(
      GetLocationStatsResponse {
        total_files: 10,
        indexed_files: 8,
        total_videos: 5,
        total_scenes: 50,
        total_audios: 3,
        failed_files: vec![],
        ..Default::default()
      },
    ))),
    ..Default::default()
  });
  rt(&Response {
    kind: Some(ResponseKind::GetFileIndexingStats(Box::new(
      GetFileIndexingStatsResponse {
        video_id: vec![1].into(),
        index_status: 0x01,
        error: buffa::MessageField::none(),
        ..Default::default()
      },
    ))),
    ..Default::default()
  });
  rt(&Response {
    kind: Some(ResponseKind::GetModelStatus(Box::new(
      GetModelStatusResponse {
        models: vec![ModelInfo {
          name: "m".into(),
          status: 2,
          size_bytes: 1024,
          ..Default::default()
        }],
        ..Default::default()
      },
    ))),
    ..Default::default()
  });
  rt(&Response {
    kind: Some(ResponseKind::GetDaemonInfo(Box::new(
      GetDaemonInfoResponse {
        version: "1.0".into(),
        started_at: 1_700_000_000,
        total_videos: 100,
        total_scenes: 1000,
        active_tasks: 2,
        ..Default::default()
      },
    ))),
    ..Default::default()
  });
  rt(&Response {
    kind: Some(ResponseKind::IndexLocation(Box::new(
      IndexLocationResponse {
        folder: buffa::MessageField::some(WatchedLocation {
          id: buffa::MessageField::some(Id {
            value: (1u8..=16).collect(),
            ..Default::default()
          }),
          location: buffa::MessageField::some(make_local_location()),
          name: "Videos".into(),
          status: 1,
          created_at: 1_700_000_000,
          ..Default::default()
        }),
        ..Default::default()
      },
    ))),
    ..Default::default()
  });
  rt(&Response {
    kind: Some(ResponseKind::RemoveLocation(Box::new(
      RemoveLocationResponse::default(),
    ))),
    ..Default::default()
  });
  rt(&Response {
    kind: Some(ResponseKind::UpdateAnnotation(Box::new(
      UpdateAnnotationResponse::default(),
    ))),
    ..Default::default()
  });
  rt(&Response {
    kind: Some(ResponseKind::EjectVolume(Box::new(
      EjectVolumeResponse::default(),
    ))),
    ..Default::default()
  });
  rt(&Response {
    kind: Some(ResponseKind::RetryFailed(Box::new(
      RetryFailedResponse::default(),
    ))),
    ..Default::default()
  });
  rt(&Response {
    kind: Some(ResponseKind::Error(Box::new(ErrorInfo {
      code: 5,
      message: "e".into(),
      ..Default::default()
    }))),
    ..Default::default()
  });

  // ── no-arm default (empty oneof round-trips) ─────────────────────────────
  rt(&Response::default());

  // ── fully-populated GetIndexedFile arm (cross-package db.v1.Video + repeated db.v1.Scene) ──
  rt(&Response {
    kind: Some(ResponseKind::GetIndexedFile(Box::new(
      GetIndexedFileResponse {
        video: buffa::MessageField::some(Video {
          meta: make_video_meta(),
          scenes: vec![vec![1].into(), vec![2].into()],
          index_status: 0x01 | 0x02 | 0x80,
          index_error: make_error_info(),
          error_status: 1,
          ..Default::default()
        }),
        scenes: vec![Scene {
          meta: make_scene_meta(),
          keyframes: vec![vec![1].into()],
          description: "ocean coast".into(),
          shot_type: "wide".into(),
          camera_motion: "pan".into(),
          tags: "beach,sunset".into(),
          people_count: 3,
          tag_ids: vec![vec![9].into()],
          vision_provider: vec!["apple".into()],
          smart_folders: vec!["fav".into()],
          ..Default::default()
        }],
        ..Default::default()
      },
    ))),
    ..Default::default()
  });

  // ── fully-populated Error arm (cross-package media.v1.ErrorInfo collapse §7.8 #4) ──
  rt(&Response {
    kind: Some(ResponseKind::Error(Box::new(ErrorInfo {
      code: 404,
      message: "not found".into(),
      ..Default::default()
    }))),
    ..Default::default()
  });
}

#[test]
#[cfg(feature = "json")]
fn batch7_sp3_json_roundtrip() {
  // GetIndexedFile arm: heaviest SP3 serde — Video + repeated Scene (same-package since mono-consolidation)
  let resp_gif = Response {
    kind: Some(ResponseKind::GetIndexedFile(Box::new(
      GetIndexedFileResponse {
        video: buffa::MessageField::some(Video {
          meta: make_video_meta(),
          scenes: vec![vec![1].into(), vec![2].into()],
          index_status: 0x01 | 0x80,
          index_error: make_error_info(),
          error_status: 1,
          ..Default::default()
        }),
        scenes: vec![Scene {
          meta: make_scene_meta(),
          keyframes: vec![vec![1].into()],
          description: "ocean coast".into(),
          shot_type: "wide".into(),
          camera_motion: "pan".into(),
          tags: "beach,sunset".into(),
          people_count: 3,
          tag_ids: vec![vec![9].into()],
          vision_provider: vec!["apple".into()],
          smart_folders: vec!["fav".into()],
          ..Default::default()
        }],
        ..Default::default()
      },
    ))),
    ..Default::default()
  };
  let json = serde_json::to_string(&resp_gif).expect("to_json");
  let back: Response = serde_json::from_str(&json).expect("from_json");
  assert_eq!(resp_gif, back);

  // Error arm at tag 20000 (remapped from reserved 19999, §7.8): proves the
  // collapsed cross-package error arm + highest sparse arm-tag survive serde
  // (§7.8 #4 collapse).
  let resp_err = Response {
    kind: Some(ResponseKind::Error(Box::new(ErrorInfo {
      code: 7,
      message: "x".into(),
      ..Default::default()
    }))),
    ..Default::default()
  };
  let json = serde_json::to_string(&resp_err).expect("to_json");
  let back: Response = serde_json::from_str(&json).expect("from_json");
  assert_eq!(resp_err, back);
}

// ── SP3 Batch 8: Event oneof envelope ───────────────────────────────────────

#[test]
fn batch8_sp3_roundtrip() {
  // ── all 5 arms individually ──────────────────────────────────────────────
  rt(&Event {
    kind: Some(EventKind::FailedFiles(Box::new(FailedFilesResponse {
      location_id: vec![1].into(),
      failed_files: vec![NetFailedFile {
        kind: 0,
        location: buffa::MessageField::some(make_local_location()),
        error: make_error_info(),
        error_status: 0x01,
        index_status: 0x02,
        ..Default::default()
      }],
      ..Default::default()
    }))),
    ..Default::default()
  });
  rt(&Event {
    kind: Some(EventKind::IndexingProgress(Box::new(
      IndexingProgressResponse {
        location: buffa::MessageField::some(make_local_location()),
        total_files: 10,
        indexed_files: 5,
        active_files: vec![IndexingFile {
          location: buffa::MessageField::some(make_local_location()),
          name: "a".into(),
          completed_phases: 0x01,
          ..Default::default()
        }],
        ..Default::default()
      },
    ))),
    ..Default::default()
  });
  rt(&Event {
    kind: Some(EventKind::VolumeStateChanged(Box::new(
      VolumeStateChangedEvent {
        volume: buffa::MessageField::none(),
        event: 2,
        ..Default::default()
      },
    ))),
    ..Default::default()
  });
  rt(&Event {
    kind: Some(EventKind::FolderUpdated(Box::new(FolderUpdatedEvent {
      folder_location: buffa::MessageField::some(make_local_location()),
      path: "/x".into(),
      event: 1,
      ..Default::default()
    }))),
    ..Default::default()
  });
  rt(&Event {
    kind: Some(EventKind::ModelDownloadProgress(Box::new(
      ModelDownloadProgressEvent {
        model: buffa::MessageField::some(ModelDownloadProgress {
          name: "m".into(),
          progress: 0.5,
          downloaded_bytes: 1,
          total_bytes: 2,
          status: 1,
          error_msg: "".into(),
          ..Default::default()
        }),
        ..Default::default()
      },
    ))),
    ..Default::default()
  });

  // ── no-arm default (proves empty oneof round-trips) ──────────────────────
  rt(&Event::default());
}

#[test]
#[cfg(feature = "json")]
fn batch8_sp3_json_roundtrip() {
  // IndexingProgress arm: covers nested media.v1.Location + repeated IndexingFile under serde
  let ev_progress = Event {
    kind: Some(EventKind::IndexingProgress(Box::new(
      IndexingProgressResponse {
        location: buffa::MessageField::some(make_local_location()),
        total_files: 10,
        indexed_files: 5,
        active_files: vec![IndexingFile {
          location: buffa::MessageField::some(make_local_location()),
          name: "a".into(),
          completed_phases: 0x01,
          ..Default::default()
        }],
        ..Default::default()
      },
    ))),
    ..Default::default()
  };
  let json = serde_json::to_string(&ev_progress).expect("to_json");
  let back: Event = serde_json::from_str(&json).expect("from_json");
  assert_eq!(ev_progress, back);

  // ModelDownloadProgress arm at tag 20002: proves the high event arm-tag survives serde
  let ev_mdp = Event {
    kind: Some(EventKind::ModelDownloadProgress(Box::new(
      ModelDownloadProgressEvent {
        model: buffa::MessageField::some(ModelDownloadProgress {
          name: "m".into(),
          progress: 0.5,
          downloaded_bytes: 1,
          total_bytes: 2,
          status: 1,
          error_msg: "".into(),
          ..Default::default()
        }),
        ..Default::default()
      },
    ))),
    ..Default::default()
  };
  let json = serde_json::to_string(&ev_mdp).expect("to_json");
  let back: Event = serde_json::from_str(&json).expect("from_json");
  assert_eq!(ev_mdp, back);
}

// ── SP3 Batch 9: correlation envelopes ──────────────────────────────────────

#[test]
fn batch9_sp3_roundtrip() {
  // ── RequestEnvelope ──────────────────────────────────────────────────────

  // request_id=42, heartbeat arm
  rt(&RequestEnvelope {
    request_id: 42,
    request: buffa::MessageField::some(Request {
      kind: Some(RequestKind::Heartbeat(Box::new(HeartbeatRequest {
        timestamp: 1,
        ..Default::default()
      }))),
      ..Default::default()
    }),
    ..Default::default()
  });

  // request_id=0 (plain uint64 zero default) + RetryFailed arm:
  // proves the plain-uint64 (NOT optional) zero-default round-trips EXACTLY,
  // not dropped (the core §7.8 #3 risk).
  rt(&RequestEnvelope {
    request_id: 0,
    request: buffa::MessageField::some(Request {
      kind: Some(RequestKind::RetryFailed(Box::new(RetryFailedRequest {
        location: buffa::MessageField::some(make_local_location()),
        ..Default::default()
      }))),
      ..Default::default()
    }),
    ..Default::default()
  });

  // request_id=9, no inner Request (MessageField::none)
  rt(&RequestEnvelope {
    request_id: 9,
    request: buffa::MessageField::none(),
    ..Default::default()
  });

  // default (request_id=0, no request)
  rt(&RequestEnvelope::default());

  // ── ResponseEnvelope ─────────────────────────────────────────────────────

  // request_id=7, heartbeat arm
  rt(&ResponseEnvelope {
    request_id: 7,
    response: buffa::MessageField::some(Response {
      kind: Some(ResponseKind::Heartbeat(Box::new(HeartbeatResponse {
        timestamp: 1,
        ..Default::default()
      }))),
      ..Default::default()
    }),
    ..Default::default()
  });

  // request_id=0 + Error arm: proves the collapsed error arm survives inside
  // the envelope AND request_id=0 round-trips (§7.8 #3 + §7.8 #4).
  rt(&ResponseEnvelope {
    request_id: 0,
    response: buffa::MessageField::some(Response {
      kind: Some(ResponseKind::Error(Box::new(ErrorInfo {
        code: 5,
        message: "e".into(),
        ..Default::default()
      }))),
      ..Default::default()
    }),
    ..Default::default()
  });

  // no inner Response (MessageField::none)
  rt(&ResponseEnvelope {
    request_id: 0,
    response: buffa::MessageField::none(),
    ..Default::default()
  });

  // default
  rt(&ResponseEnvelope::default());
}

#[test]
#[cfg(feature = "json")]
fn batch9_sp3_json_roundtrip() {
  // Populated RequestEnvelope: nested Request oneof under serde; request_id non-zero
  let req_env = RequestEnvelope {
    request_id: 42,
    request: buffa::MessageField::some(Request {
      kind: Some(RequestKind::Heartbeat(Box::new(HeartbeatRequest {
        timestamp: 1,
        ..Default::default()
      }))),
      ..Default::default()
    }),
    ..Default::default()
  };
  let json = serde_json::to_string(&req_env).expect("to_json");
  let back: RequestEnvelope = serde_json::from_str(&json).expect("from_json");
  assert_eq!(req_env, back);

  // Populated ResponseEnvelope carrying ResponseKind::Error (collapsed error
  // oneof under serde); request_id non-zero
  let resp_env = ResponseEnvelope {
    request_id: 7,
    response: buffa::MessageField::some(Response {
      kind: Some(ResponseKind::Error(Box::new(ErrorInfo {
        code: 5,
        message: "e".into(),
        ..Default::default()
      }))),
      ..Default::default()
    }),
    ..Default::default()
  };
  let json = serde_json::to_string(&resp_env).expect("to_json");
  let back: ResponseEnvelope = serde_json::from_str(&json).expect("from_json");
  assert_eq!(resp_env, back);

  // request_id=0 JSON case: proves the zero default round-trips through serde
  // (plain uint64, NOT optional — §7.8 #3).
  let req_env_zero = RequestEnvelope {
    request_id: 0,
    request: buffa::MessageField::some(Request {
      kind: Some(RequestKind::RetryFailed(Box::new(RetryFailedRequest {
        location: buffa::MessageField::some(make_local_location()),
        ..Default::default()
      }))),
      ..Default::default()
    }),
    ..Default::default()
  };
  let json = serde_json::to_string(&req_env_zero).expect("to_json");
  let back: RequestEnvelope = serde_json::from_str(&json).expect("from_json");
  assert_eq!(req_env_zero, back);
}
