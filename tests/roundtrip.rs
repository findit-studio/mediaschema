use buffa::Message;
use core::num::NonZeroU32;
use mediaschema::{
    ActionDetection, Aesthetics, AppPathBuf, AudioFormat, BarcodeDetection,
    BodyPose3DDetection, BodyPose3DHeightEstimation, BodyPose3DJoint, BodyPoseDetection,
    BodyPoseJoint, BoundingBox, ClassificationDetection, CodecId, ColorDetection, Detection,
    Dimensions, DocumentSegment, EmotionDetection, ErrorInfo, FaceDetection, FaceLandmarkPoint,
    FaceLandmarkRegion, FaceLandmarksDetection, FeaturePrint, FileChecksum, HandChirality,
    HandPoseDetection, HorizonInfo, Id, LightingDetection, Local, Location, LocationKind,
    LocationTarget, LocationTargetKind, MediaKind, MediaKindKind, MoodDetection, ObjectDetection,
    PersonInstanceMaskDetection, PersonSegmentationMask, Point2D, SaliencyRegion, SubjectDetection,
    Tag, TextDetection, TimedDetection, VideoFormat, VolumeMeta, WatchedLocation,
};
use mediatime::{Timebase, TimeRange};

fn rt<M: Message + PartialEq + std::fmt::Debug>(m: &M) {
    let bytes = m.encode_to_vec();
    let back = M::decode_from_slice(&bytes).expect("decode");
    assert_eq!(*m, back, "wire round-trip mismatch");
}

#[test]
fn detection_roundtrip() {
    let d = Detection { label: "beach".into(), confidence: 0.93, ..Default::default() };
    rt(&d);
    rt(&Detection::default());
}

#[test]
fn bounding_box_roundtrip() {
    let b = BoundingBox { x: 0.1, y: 0.2, width: 0.5, height: 0.4, ..Default::default() };
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
    td.timebase =
        buffa::MessageField::some(Timebase::new(1, NonZeroU32::new(48000).unwrap()));
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
    let d = Detection { label: "indoor".into(), confidence: 0.5, ..Default::default() };
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
        let d = Detection { label, confidence, ..Default::default() };
        let bytes = d.encode_to_vec();
        let ok = Detection::decode_from_slice(&bytes).map(|b| b == d).unwrap_or(false);
        quickcheck::TestResult::from_bool(ok)
    }
    quickcheck::quickcheck(prop as fn(String, f32) -> quickcheck::TestResult);
}

// ── SP1 Batch 1 ──────────────────────────────────────────────────────────────

#[test]
fn batch1_roundtrip() {
    // Point2D
    let p = Point2D { x: 1.5, y: 2.5, ..Default::default() };
    rt(&p);
    rt(&Point2D::default());

    // Dimensions
    let d = Dimensions { width: 1920, height: 1080, ..Default::default() };
    rt(&d);
    rt(&Dimensions::default());

    // Aesthetics
    let a = Aesthetics { overall_score: 0.85, is_utility: true, ..Default::default() };
    rt(&a);
    rt(&Aesthetics::default());

    // HorizonInfo
    let h = HorizonInfo { angle: 3.14, confidence: 0.9, ..Default::default() };
    rt(&h);
    rt(&HorizonInfo::default());

    // CodecId
    let c = CodecId { value: 42, ..Default::default() };
    rt(&c);
    rt(&CodecId::default());

    // FeaturePrint
    let f = FeaturePrint { data: vec![0xDE, 0xAD, 0xBE, 0xEF], element_type: 1, ..Default::default() };
    rt(&f);
    rt(&FeaturePrint::default());

    // MediaKind — video arm
    let mk_video = MediaKind {
        kind: Some(MediaKindKind::Video(buffa::EnumValue::from(VideoFormat::VIDEO_FORMAT_MP4))),
        ..Default::default()
    };
    rt(&mk_video);

    // MediaKind — audio arm
    let mk_audio = MediaKind {
        kind: Some(MediaKindKind::Audio(buffa::EnumValue::from(AudioFormat::AUDIO_FORMAT_AAC))),
        ..Default::default()
    };
    rt(&mk_audio);

    // MediaKind — default (no arm set)
    rt(&MediaKind::default());

    // DocumentSegment
    let make_pt = |x, y| buffa::MessageField::some(Point2D { x, y, ..Default::default() });
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
    let make_pt = |x, y| MessageField::some(Point2D { x, y, ..Default::default() });
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
        let d = Dimensions { width, height, ..Default::default() };
        let bytes = d.encode_to_vec();
        let ok = Dimensions::decode_from_slice(&bytes).map(|b| b == d).unwrap_or(false);
        quickcheck::TestResult::from_bool(ok)
    }
    quickcheck::quickcheck(prop as fn(u32, u32) -> quickcheck::TestResult);
}

// ── SP1 Batch 2 ──────────────────────────────────────────────────────────────

#[test]
fn batch2_roundtrip() {
    // Id — 16 non-zero bytes + default (empty)
    let id = Id { value: (1u8..=16).collect(), ..Default::default() };
    rt(&id);
    rt(&Id::default());

    // FileChecksum — 32 non-zero bytes + default (empty)
    let cksum = FileChecksum { value: (1u8..=32).collect(), ..Default::default() };
    rt(&cksum);
    rt(&FileChecksum::default());

    // Local — nested Id + components
    let local_populated = Local {
        volume: buffa::MessageField::some(Id { value: (1u8..=16).collect(), ..Default::default() }),
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
    let tag = Tag { name: "favorite".into(), color: 0xFF_AA_00_FF, ..Default::default() };
    rt(&tag);
    rt(&Tag::default());

    // ErrorInfo — populated + default
    let err = ErrorInfo { code: 404, message: "not found".into(), ..Default::default() };
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
        id: buffa::MessageField::some(Id { value: (1u8..=16).collect(), ..Default::default() }),
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
        id: buffa::MessageField::some(Id { value: (1u8..=16).collect(), ..Default::default() }),
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
        id: buffa::MessageField::some(Id { value: (1u8..=16).collect(), ..Default::default() }),
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
        id: buffa::MessageField::some(Id { value: (1u8..=16).collect(), ..Default::default() }),
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
    buffa::MessageField::some(Detection { label: label.into(), confidence, ..Default::default() })
}

fn make_bbox(x: f32, y: f32, w: f32, h: f32) -> buffa::MessageField<BoundingBox> {
    buffa::MessageField::some(BoundingBox { x, y, width: w, height: h, ..Default::default() })
}

#[test]
fn batch4_roundtrip() {
    // ── 6 single-Detection envelopes ─────────────────────────────────────────
    let populated_det = make_detection("x", 0.9);

    let cd = ClassificationDetection { detection: populated_det.clone(), ..Default::default() };
    rt(&cd);
    rt(&ClassificationDetection::default());

    let ad = ActionDetection { detection: populated_det.clone(), ..Default::default() };
    rt(&ad);
    rt(&ActionDetection::default());

    let ed = EmotionDetection { detection: populated_det.clone(), ..Default::default() };
    rt(&ed);
    rt(&EmotionDetection::default());

    let md = MoodDetection { detection: populated_det.clone(), ..Default::default() };
    rt(&md);
    rt(&MoodDetection::default());

    let ld = LightingDetection { detection: populated_det.clone(), ..Default::default() };
    rt(&ld);
    rt(&LightingDetection::default());

    let col = ColorDetection { detection: populated_det.clone(), ..Default::default() };
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
        roll: -0.5,  // negative angle
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
    let flp = FaceLandmarkPoint { x: 0.3, y: 0.7, ..Default::default() };
    rt(&flp);
    rt(&FaceLandmarkPoint::default());

    // ── FaceLandmarkRegion — name + ≥2 points ────────────────────────────────
    let flr = FaceLandmarkRegion {
        name: "left_eye".into(),
        points: vec![
            FaceLandmarkPoint { x: 0.25, y: 0.35, ..Default::default() },
            FaceLandmarkPoint { x: 0.30, y: 0.36, ..Default::default() },
            FaceLandmarkPoint { x: 0.35, y: 0.35, ..Default::default() },
        ],
        ..Default::default()
    };
    rt(&flr);
    rt(&FaceLandmarkRegion::default());

    // ── FaceLandmarksDetection — bbox + confidence + ≥1 non-empty region ─────
    let fld = FaceLandmarksDetection {
        bbox: make_bbox(0.1, 0.1, 0.4, 0.5),
        confidence: 0.92,
        regions: vec![
            FaceLandmarkRegion {
                name: "nose_tip".into(),
                points: vec![
                    FaceLandmarkPoint { x: 0.5, y: 0.55, ..Default::default() },
                    FaceLandmarkPoint { x: 0.52, y: 0.57, ..Default::default() },
                ],
                ..Default::default()
            },
        ],
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
            BodyPoseJoint { name: "left_hip".into(), x: 0.3, y: 0.6, confidence: 0.9, ..Default::default() },
            BodyPoseJoint { name: "right_hip".into(), x: -0.1, y: 0.61, confidence: 0.85, ..Default::default() },
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
        height_estimation: buffa::EnumValue::from(BodyPose3DHeightEstimation::BODY_POSE_3D_HEIGHT_ESTIMATION_REFERENCE),
        joints: vec![
            BodyPose3DJoint { name: "spine".into(), x: 0.0, y: 0.5, z: 0.02, confidence: 0.95, ..Default::default() },
            BodyPose3DJoint { name: "neck".into(), x: 0.0, y: 0.8, z: -0.01, confidence: 0.93, ..Default::default() },
        ],
        ..Default::default()
    };
    rt(&bp3d_reference);

    let bp3d_measured = BodyPose3DDetection {
        confidence: 0.88,
        body_height: 1.80,
        height_estimation: buffa::EnumValue::from(BodyPose3DHeightEstimation::BODY_POSE_3D_HEIGHT_ESTIMATION_MEASURED),
        joints: vec![
            BodyPose3DJoint { name: "left_ankle".into(), x: -0.2, y: 0.05, z: 0.0, confidence: 0.79, ..Default::default() },
        ],
        ..Default::default()
    };
    rt(&bp3d_measured);

    let bp3d_unspecified = BodyPose3DDetection {
        confidence: 0.75,
        body_height: 1.70,
        height_estimation: buffa::EnumValue::from(BodyPose3DHeightEstimation::BODY_POSE_3D_HEIGHT_ESTIMATION_UNSPECIFIED),
        joints: vec![
            BodyPose3DJoint { name: "right_wrist".into(), x: 0.4, y: 0.55, z: -0.03, confidence: 0.80, ..Default::default() },
        ],
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
            BodyPoseJoint { name: "thumb_tip".into(), x: 0.15, y: 0.22, confidence: 0.91, ..Default::default() },
            BodyPoseJoint { name: "index_tip".into(), x: 0.18, y: 0.25, confidence: 0.89, ..Default::default() },
        ],
        ..Default::default()
    };
    rt(&hpd_left);

    let hpd_right = HandPoseDetection {
        bbox: make_bbox(0.6, 0.2, 0.2, 0.3),
        confidence: 0.90,
        chirality: buffa::EnumValue::from(HandChirality::HAND_CHIRALITY_RIGHT),
        joints: vec![
            BodyPoseJoint { name: "thumb_tip".into(), x: 0.65, y: 0.22, confidence: 0.88, ..Default::default() },
        ],
        ..Default::default()
    };
    rt(&hpd_right);
    rt(&HandPoseDetection::default());

    // ── PersonSegmentationMask — bbox + Dimensions + non-empty data ───────────
    let psm = PersonSegmentationMask {
        bbox: make_bbox(0.0, 0.0, 1.0, 1.0),
        confidence: 0.97,
        dimensions: buffa::MessageField::some(Dimensions { width: 64, height: 64, ..Default::default() }),
        data: vec![0xAAu8; 64 * 64],
        ..Default::default()
    };
    rt(&psm);
    rt(&PersonSegmentationMask::default());

    // ── PersonInstanceMaskDetection — bbox + Dimensions + instance_index + data
    let pimd = PersonInstanceMaskDetection {
        bbox: make_bbox(0.1, 0.05, 0.35, 0.7),
        confidence: 0.89,
        instance_index: 2,
        dimensions: buffa::MessageField::some(Dimensions { width: 32, height: 32, ..Default::default() }),
        data: vec![0xBBu8; 32 * 32],
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
        height_estimation: buffa::EnumValue::from(BodyPose3DHeightEstimation::BODY_POSE_3D_HEIGHT_ESTIMATION_MEASURED),
        joints: vec![
            BodyPose3DJoint { name: "left_shoulder".into(), x: -0.1, y: 0.75, z: 0.02, confidence: 0.94, ..Default::default() },
            BodyPose3DJoint { name: "right_shoulder".into(), x: 0.1, y: 0.75, z: 0.02, confidence: 0.92, ..Default::default() },
            BodyPose3DJoint { name: "left_hip".into(), x: -0.08, y: 0.45, z: 0.01, confidence: 0.90, ..Default::default() },
        ],
        ..Default::default()
    };
    let json = serde_json::to_string(&bp3d).expect("to_json");
    let back: BodyPose3DDetection = serde_json::from_str(&json).expect("from_json");
    assert_eq!(bp3d, back);
}
