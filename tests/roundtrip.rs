use buffa::Message;
use core::num::NonZeroU32;
use mediaschema::{
    ActionDetection, Aesthetics, AnimalAnalysis, AppPathBuf, Audio, AudioAnalysis,
    AudioChannelLayout, AudioChannelOrderKind, AudioChannelSpec, AudioClipKind, AudioCodec,
    AudioContainerFormat, AudioCoverArt, AudioEvent, AudioFileRecord, AudioFormat, AudioMeta,
    AudioPrefilterClass, AudioSampleFormat, AudioStreamMeta, AudioSummary, AudioTrack, AudioTrackMeta,
    AudioTrackRole,
    AudioTranscriptSegment, BarcodeDetection, BodyPose3DDetection, BodyPose3DHeightEstimation,
    BodyPose3DJoint, BodyPoseDetection, BodyPoseJoint, BoundingBox, Ced, CedDetection,
    ChannelLayoutKind, Chromaprint, Clap, ClassificationDetection, CodecId, ColorDetection,
    DbMediaKind, Detection, Dimensions, DocumentSegment, Ebur128, EmotionDetection, ErrorInfo,
    FaceDetection, FaceLandmarkPoint, FaceLandmarkRegion, FaceLandmarksDetection, FailedFile,
    FeaturePrint, FileChecksum, HandChirality, HandPoseDetection, HorizonInfo, HumanAnalysis, Id,
    Keyframe, LightingDetection, Local, Location, LocationKind, LocationTarget, LocationTargetKind,
    Media, MediaKind, MediaKindKind, MediaMeta, MoodDetection, ObjectDetection,
    PersonInstanceMaskDetection, PersonSegmentationMask, Point2D, SaliencyRegion, Scene, SceneMeta,
    SceneVlmResult, SoundSource, Sp2CodegenSmoke, SpeakerSegment, SubjectDetection, Subtitle,
    SubtitleCue, SubtitleMeta, SubtitleTrack, SubtitleTrackFormat, SubtitleTrackMeta,
    SubtitleTrackOrigin, SubtitleTrackOriginSource, SubtitleTrackRole, Tag, TagConfidence,
    TextDetection, Timecode, TimedDetection, TrackClassificationType, TrackRecord, TrackTag,
    TrackTime, Video, VideoFormat, VideoMeta, VideoStreamMeta, VideoTrack, VideoTrackMeta,
    VolumeMeta, WatchedLocation,
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
                    FaceLandmarkPoint { x: 0.5, y: 0.55, ..Default::default() },
                    FaceLandmarkPoint { x: 0.52, y: 0.57, ..Default::default() },
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
            data: vec![0xAAu8; 32 * 32],
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
                    FaceLandmarkPoint { x: 0.25, y: 0.35, ..Default::default() },
                    FaceLandmarkPoint { x: 0.30, y: 0.36, ..Default::default() },
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
            data: vec![0xBBu8; 16 * 16],
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
        id: vec![0x01, 0x02, 0x03, 0x04],
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
        id: vec![0xAA, 0xBB],
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
    assert_eq!(SubtitleTrackFormat::SUBTITLE_TRACK_FORMAT_UNSPECIFIED as i32, 0);
    assert_eq!(SubtitleTrackFormat::SUBTITLE_TRACK_FORMAT_WHISPER as i32, 9);
    assert_eq!(SubtitleTrackRole::SUBTITLE_TRACK_ROLE_UNSPECIFIED as i32, 0);
    assert_eq!(SubtitleTrackRole::SUBTITLE_TRACK_ROLE_COMMENTARY as i32, 6);
    assert_eq!(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_UNSPECIFIED as i32, 0);
    assert_eq!(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_CH22_2 as i32, 38);
    assert_eq!(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_CUBE as i32, 9);
    assert_eq!(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_CH5_1 as i32, 19);
    assert_eq!(AudioChannelOrderKind::AUDIO_CHANNEL_ORDER_KIND_UNSPECIFIED as i32, 0);
    assert_eq!(AudioChannelOrderKind::AUDIO_CHANNEL_ORDER_KIND_AMBISONIC as i32, 3);
    assert_eq!(AudioClipKind::AUDIO_CLIP_KIND_UNSPECIFIED as i32, 0);
    assert_eq!(AudioClipKind::AUDIO_CLIP_KIND_EVENT_SPAN as i32, 4);
    assert_eq!(AudioPrefilterClass::AUDIO_PREFILTER_CLASS_UNSPECIFIED as i32, 0);
    assert_eq!(AudioPrefilterClass::AUDIO_PREFILTER_CLASS_NOISE as i32, 3);
    assert_eq!(AudioTrackRole::AUDIO_TRACK_ROLE_UNSPECIFIED as i32, 0);
    assert_eq!(AudioTrackRole::AUDIO_TRACK_ROLE_LYRICS as i32, 6);
    assert_eq!(AudioContainerFormat::AUDIO_CONTAINER_FORMAT_UNSPECIFIED as i32, 0);
    assert_eq!(AudioContainerFormat::AUDIO_CONTAINER_FORMAT_AAC as i32, 10);
    assert_eq!(AudioCodec::AUDIO_CODEC_UNSPECIFIED as i32, 0);
    assert_eq!(AudioCodec::AUDIO_CODEC_VORBIS as i32, 7);
    assert_eq!(AudioSampleFormat::AUDIO_SAMPLE_FORMAT_UNSPECIFIED as i32, 0);
    assert_eq!(AudioSampleFormat::AUDIO_SAMPLE_FORMAT_F32 as i32, 4);
    assert_eq!(TrackClassificationType::TRACK_CLASSIFICATION_TYPE_UNSPECIFIED as i32, 0);
    assert_eq!(TrackClassificationType::TRACK_CLASSIFICATION_TYPE_MIXED as i32, 7);
}

// ── SP2 Batch 2: audio scalar leaves ────────────────────────────────────────

#[test]
fn batch2_sp2_roundtrip() {
    use mediaschema::{
        AudioChannelSpec, AudioEvent, AudioTranscriptSegment, Ced, CedDetection,
        Chromaprint, Ebur128, SoundSource, SpeakerSegment, TagConfidence, Timecode,
    };
    rt(&TagConfidence { label: "speech".into(), confidence: 0.91, ..Default::default() });
    rt(&TagConfidence::default());
    rt(&SoundSource { name: "rain".into(), prominence: "background".into(), description: "steady rain".into(), ..Default::default() });
    rt(&SoundSource::default());
    rt(&AudioEvent { event_type: "applause".into(), start_ms: 1000, end_ms: 4000, avg_confidence: 0.8, ..Default::default() });
    rt(&AudioEvent::default());
    rt(&SpeakerSegment { start_ms: 0, end_ms: 2500, speaker_id: 3, ..Default::default() });
    rt(&SpeakerSegment::default());
    rt(&AudioTranscriptSegment { start_ms: 100, end_ms: 900, text: "hello".into(), language: "en".into(), confidence: 0.97, ..Default::default() });
    rt(&AudioTranscriptSegment::default());
    rt(&AudioChannelSpec { index: 2, raw_id: 0x10, label: "FL".into(), ..Default::default() });
    rt(&AudioChannelSpec::default());
    rt(&Chromaprint { fingerprint: vec![0x01, 0x02, 0x03, 0x04], fingerprint_duration: 120.5, ..Default::default() });
    rt(&Chromaprint::default());
    rt(&Ebur128 { loudness_lufs: -14.0, loudness_range_lu: 7.5, true_peak_dbtp: -1.2, ..Default::default() });
    rt(&Ebur128::default());
    rt(&Timecode { start: "00:00:00:00".into(), end: "01:23:45:12".into(), fps: 25.0, drop_frame: true, ..Default::default() });
    rt(&Timecode::default());
    rt(&CedDetection { tag: 0xDEAD_BEEF_0000_1234, confidence: 0.66, ..Default::default() });
    rt(&CedDetection::default());
    rt(&Ced {
        tags: vec![
            CedDetection { tag: 1, confidence: 0.5, ..Default::default() },
            CedDetection { tag: 0xFFFF_FFFF_FFFF_FFFF, confidence: 0.9, ..Default::default() },
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
            CedDetection { tag: 0xABCD_0000_0000_0001, confidence: 0.42, ..Default::default() },
            CedDetection { tag: 7, confidence: 0.99, ..Default::default() },
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
        AudioChannelLayout, AudioChannelOrderKind, AudioChannelSpec, ChannelLayoutKind,
        Clap, Detection, TrackTag,
    };
    let det = |l: &str, c: f32| buffa::MessageField::some(Detection { label: l.into(), confidence: c, ..Default::default() });

    rt(&AudioChannelLayout {
        order: buffa::EnumValue::from(AudioChannelOrderKind::AUDIO_CHANNEL_ORDER_KIND_NATIVE),
        channels: 6,
        known_kind: buffa::EnumValue::from(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_CH5_1),
        native_mask: Some(0x3F),
        custom_channels: vec![AudioChannelSpec { index: 0, raw_id: 1, label: "FL".into(), ..Default::default() }],
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
            Detection { label: "applause".into(), confidence: 0.5, ..Default::default() },
            Detection { label: "cheer".into(), confidence: 0.55, ..Default::default() },
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
        detections: vec![Detection { label: "wind".into(), confidence: 0.6, ..Default::default() }],
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
        audio_detection: buffa::MessageField::some(Detection { label: "speech".into(), confidence: 0.88, ..Default::default() }),
        scene: buffa::MessageField::none(),
        mood: buffa::MessageField::some(Detection { label: "calm".into(), confidence: 0.7, ..Default::default() }),
        voice: buffa::MessageField::none(),
        sound_events: vec![Detection { label: "door".into(), confidence: 0.4, ..Default::default() }],
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
            0, 1000, mediatime::Timebase::new(30000, core::num::NonZeroU32::new(1001).unwrap()),
        )),
        packet_observed: buffa::MessageField::none(),
        decoded_observed: buffa::MessageField::none(),
        ..Default::default()
    })
}

#[test]
fn batch4_sp2_roundtrip() {
    use mediaschema::{
        CodecId, Dimensions, FailedFile, MediaMeta, SceneMeta, SubtitleMeta,
        SubtitleTrackMeta, SubtitleTrackOrigin, SubtitleTrackOriginSource, VideoFormat,
        VideoMeta, VideoStreamMeta, VideoTrackMeta,
    };
    rt(&VideoMeta {
        id: vec![1, 2, 3, 4],
        name: "clip.mp4".into(),
        format: buffa::EnumValue::from(VideoFormat::VIDEO_FORMAT_MP4),
        dimensions: buffa::MessageField::some(Dimensions { width: 1920, height: 1080, ..Default::default() }),
        size: 123456,
        time: sp2_track_time_one(),
        frame_rate: 23.976,
        bit_rate: 8_000_000,
        created_at: 1_700_000_000,
        ..Default::default()
    });
    rt(&VideoMeta::default());

    rt(&VideoTrackMeta { id: vec![9], ordinal: 0, stream_index: 1, container_track_id: Some(42), time: sp2_track_time_one(), ..Default::default() });
    rt(&VideoTrackMeta { id: vec![9], ordinal: 1, stream_index: 2, container_track_id: None, time: buffa::MessageField::none(), ..Default::default() });
    rt(&VideoTrackMeta::default());

    rt(&VideoStreamMeta {
        codec_id: buffa::MessageField::some(CodecId { value: 27, ..Default::default() }),
        dimensions: buffa::MessageField::some(Dimensions { width: 3840, height: 2160, ..Default::default() }),
        total_pts: 90_000,
        frame_rate: 60.0,
        bit_rate: 20_000_000,
        time_base: buffa::MessageField::some(mediatime::Timebase::new(1, core::num::NonZeroU32::new(30000).unwrap())),
        ..Default::default()
    });
    rt(&VideoStreamMeta::default());

    rt(&MediaMeta { id: vec![1], checksum: (1u8..=32).collect(), name: "a".into(), size: 10, time: sp2_track_time_one(), created_at: 1, ..Default::default() });
    rt(&MediaMeta::default());

    rt(&SceneMeta {
        id: vec![1], video_id: vec![2],
        range: buffa::MessageField::some(mediatime::TimeRange::new(
            100, 500, mediatime::Timebase::new(30000, core::num::NonZeroU32::new(1001).unwrap()),
        )),
        created_at: 5, video_track_id: vec![3],
        ..Default::default()
    });
    rt(&SceneMeta::default());

    rt(&SubtitleMeta { id: vec![7], created_at: 9, ..Default::default() });
    rt(&SubtitleMeta::default());

    rt(&SubtitleTrackMeta { id: vec![1], ordinal: 0, stream_index: Some(3), container_track_id: Some(8), time: sp2_track_time_one(), ..Default::default() });
    rt(&SubtitleTrackMeta { id: vec![1], ordinal: 2, stream_index: None, container_track_id: None, time: buffa::MessageField::none(), ..Default::default() });
    rt(&SubtitleTrackMeta::default());

    rt(&SubtitleTrackOrigin { kind: 1, source: None, ..Default::default() });
    rt(&SubtitleTrackOrigin { kind: 3, source: Some(SubtitleTrackOriginSource::SourceAudioTrackId(vec![0xAA, 0xBB])), ..Default::default() });
    rt(&SubtitleTrackOrigin { kind: 4, source: Some(SubtitleTrackOriginSource::SourceSubtitleTrackId(vec![0xCC, 0xDD])), ..Default::default() });
    rt(&SubtitleTrackOrigin::default());

    rt(&FailedFile { id: vec![1], media_id: vec![2], location_id: vec![3], failed_at: 42, ..Default::default() });
    rt(&FailedFile::default());
}

#[test]
#[cfg(feature = "json")]
fn batch4_sp2_json_roundtrip() {
    use mediaschema::{Dimensions, SubtitleTrackOrigin, SubtitleTrackOriginSource, VideoFormat, VideoMeta};
    let vm = VideoMeta {
        id: vec![1, 2],
        name: "j.mp4".into(),
        format: buffa::EnumValue::from(VideoFormat::VIDEO_FORMAT_MKV),
        dimensions: buffa::MessageField::some(Dimensions { width: 1280, height: 720, ..Default::default() }),
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

    let o = SubtitleTrackOrigin { kind: 4, source: Some(SubtitleTrackOriginSource::SourceSubtitleTrackId(vec![1, 2, 3])), ..Default::default() };
    let oj = serde_json::to_string(&o).expect("to_json");
    let ob: SubtitleTrackOrigin = serde_json::from_str(&oj).expect("from_json");
    assert_eq!(o, ob);
}

// ── SP2 Batch 5: non-audio track/record wrappers ─────────────────────────────

fn make_video_meta() -> buffa::MessageField<VideoMeta> {
    buffa::MessageField::some(VideoMeta {
        id: vec![1, 2, 3, 4],
        name: "clip.mp4".into(),
        format: buffa::EnumValue::from(mediaschema::VideoFormat::VIDEO_FORMAT_MP4),
        dimensions: buffa::MessageField::some(Dimensions { width: 1920, height: 1080, ..Default::default() }),
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
        id: vec![9],
        ordinal: 0,
        stream_index: 1,
        container_track_id: Some(42),
        time: sp2_track_time_one(),
        ..Default::default()
    })
}

fn make_video_stream_meta() -> buffa::MessageField<VideoStreamMeta> {
    buffa::MessageField::some(VideoStreamMeta {
        codec_id: buffa::MessageField::some(CodecId { value: 27, ..Default::default() }),
        dimensions: buffa::MessageField::some(Dimensions { width: 1920, height: 1080, ..Default::default() }),
        total_pts: 90_000,
        frame_rate: 30.0,
        bit_rate: 8_000_000,
        time_base: buffa::MessageField::some(mediatime::Timebase::new(1, NonZeroU32::new(30000).unwrap())),
        ..Default::default()
    })
}

fn make_subtitle_track_meta() -> buffa::MessageField<SubtitleTrackMeta> {
    buffa::MessageField::some(SubtitleTrackMeta {
        id: vec![1],
        ordinal: 0,
        stream_index: Some(3),
        container_track_id: Some(8),
        time: sp2_track_time_one(),
        ..Default::default()
    })
}

fn make_media_meta() -> buffa::MessageField<mediaschema::MediaMeta> {
    buffa::MessageField::some(mediaschema::MediaMeta {
        id: vec![1],
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
        id: vec![7],
        created_at: 9,
        ..Default::default()
    })
}

fn make_error_info() -> buffa::MessageField<ErrorInfo> {
    buffa::MessageField::some(ErrorInfo { code: 5, message: "x".into(), ..Default::default() })
}

#[test]
fn batch5_sp2_roundtrip() {
    // ── Video: populated (non-trivial) ───────────────────────────────────────
    rt(&Video {
        meta: make_video_meta(),
        scenes: vec![vec![1], vec![2]],
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
        video_id: vec![7],
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
        video_id: Some(vec![1]),
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
        subtitle_id: vec![0xAB, 0xCD],
        origin: buffa::MessageField::some(SubtitleTrackOrigin { kind: 2, source: None, ..Default::default() }),
        format: buffa::EnumValue::from(SubtitleTrackFormat::SUBTITLE_TRACK_FORMAT_SRT),
        role: buffa::EnumValue::from(SubtitleTrackRole::SUBTITLE_TRACK_ROLE_CAPTION),
        language: "en".into(),
        title: "English".into(),
        codec_id: buffa::MessageField::some(CodecId { value: 86, ..Default::default() }),
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
        id: vec![0x01, 0x02],
        subtitle_track_id: vec![0x03, 0x04],
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
        id: vec![0x05],
        subtitle_track_id: vec![0x06],
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
        subtitle_id: vec![0xAB, 0xCD],
        origin: buffa::MessageField::some(SubtitleTrackOrigin {
            kind: 3,
            source: Some(SubtitleTrackOriginSource::SourceAudioTrackId(vec![0xAA, 0xBB])),
            ..Default::default()
        }),
        format: buffa::EnumValue::from(SubtitleTrackFormat::SUBTITLE_TRACK_FORMAT_SRT),
        role: buffa::EnumValue::from(SubtitleTrackRole::SUBTITLE_TRACK_ROLE_CAPTION),
        language: "en".into(),
        title: "English Captions".into(),
        codec_id: buffa::MessageField::some(CodecId { value: 86, ..Default::default() }),
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
        id: vec![1],
        video_id: vec![2],
        range: buffa::MessageField::some(mediatime::TimeRange::new(
            100,
            500,
            mediatime::Timebase::new(30000, core::num::NonZeroU32::new(1001).unwrap()),
        )),
        created_at: 5,
        video_track_id: vec![3],
        ..Default::default()
    })
}

#[test]
fn batch6_sp2_roundtrip() {
    // ── Scene: populated ──────────────────────────────────────────────────────
    rt(&Scene {
        meta: make_scene_meta(),
        keyframes: vec![vec![1], vec![2]],
        description: "ocean coast".into(),
        shot_type: "wide".into(),
        camera_motion: "pan".into(),
        tags: "beach,sunset".into(),
        people_count: 3,
        tag_ids: vec![vec![9]],
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
        id: vec![1, 2],
        scene_id: vec![3, 4],
        pts: 12345,
        dimensions: buffa::MessageField::some(Dimensions { width: 1920, height: 1080, ..Default::default() }),
        data: vec![0xABu8; 64],
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
                        FaceLandmarkPoint { x: 0.5, y: 0.55, ..Default::default() },
                        FaceLandmarkPoint { x: 0.52, y: 0.57, ..Default::default() },
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
                data: vec![0xAAu8; 32 * 32],
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
        horizon: buffa::MessageField::some(HorizonInfo { angle: 1.57, confidence: 0.91, ..Default::default() }),
        document_segments: vec![DocumentSegment {
            top_left: buffa::MessageField::some(Point2D { x: 0.0, y: 0.0, ..Default::default() }),
            top_right: buffa::MessageField::some(Point2D { x: 1.0, y: 0.0, ..Default::default() }),
            bottom_left: buffa::MessageField::some(Point2D { x: 0.0, y: 1.0, ..Default::default() }),
            bottom_right: buffa::MessageField::some(Point2D { x: 1.0, y: 1.0, ..Default::default() }),
            confidence: 0.96,
            ..Default::default()
        }],
        feature_print: buffa::MessageField::some(FeaturePrint {
            data: vec![0xFFu8; 64],
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
        id: vec![1, 2],
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
        codec_id: buffa::MessageField::some(CodecId { value: 86, ..Default::default() }),
        sample_rate: 48000,
        layout: buffa::MessageField::some(AudioChannelLayout {
            order: buffa::EnumValue::from(AudioChannelOrderKind::AUDIO_CHANNEL_ORDER_KIND_NATIVE),
            channels: 6,
            known_kind: buffa::EnumValue::from(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_CH5_1),
            native_mask: Some(0x3F),
            custom_channels: vec![AudioChannelSpec { index: 0, raw_id: 1, label: "FL".into(), ..Default::default() }],
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
        id: vec![1],
        ordinal: 0,
        stream_index: 1,
        container_track_id: Some(7),
        time: sp2_track_time_one(),
        ..Default::default()
    });
    // AudioTrackMeta: container_track_id None, time none
    rt(&AudioTrackMeta {
        id: vec![2],
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
        audio_type: buffa::MessageField::some(TagConfidence { label: "speech".into(), confidence: 0.91, ..Default::default() }),
        scene: buffa::MessageField::some(TagConfidence { label: "concert".into(), confidence: 0.82, ..Default::default() }),
        mood: buffa::MessageField::some(TagConfidence { label: "energetic".into(), confidence: 0.75, ..Default::default() }),
        voice: buffa::MessageField::some(TagConfidence { label: "singing".into(), confidence: 0.68, ..Default::default() }),
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
        audio_type: buffa::MessageField::some(TagConfidence { label: "speech".into(), confidence: 0.91, ..Default::default() }),
        scene: buffa::MessageField::some(TagConfidence { label: "concert".into(), confidence: 0.82, ..Default::default() }),
        mood: buffa::MessageField::some(TagConfidence { label: "energetic".into(), confidence: 0.75, ..Default::default() }),
        voice: buffa::MessageField::some(TagConfidence { label: "singing".into(), confidence: 0.68, ..Default::default() }),
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
        id: vec![0xA1, 0xA2],
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
        id: vec![0xB1],
        ordinal: 0,
        stream_index: 1,
        container_track_id: Some(7),
        time: sp2_track_time_one(),
        ..Default::default()
    })
}

fn make_audio_stream_meta_mf() -> buffa::MessageField<AudioStreamMeta> {
    buffa::MessageField::some(AudioStreamMeta {
        codec_id: buffa::MessageField::some(CodecId { value: 86, ..Default::default() }),
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
        id: vec![0x01, 0x02, 0x03, 0x04],
        audio_id: vec![0x05, 0x06, 0x07, 0x08],
        scene_id: Some(vec![5]),
        kind: buffa::EnumValue::from(AudioClipKind::AUDIO_CLIP_KIND_FIXED_WINDOW),
        start_ms: 1000,
        end_ms: 5000,
        track_id: vec![0x09, 0x0A],
        ced_tags: vec![CedDetection { tag: 0xDEAD_BEEF_0000_0001, confidence: 0.7, ..Default::default() }],
        zs_audio_type: buffa::MessageField::some(TagConfidence { label: "speech".into(), confidence: 0.88, ..Default::default() }),
        zs_scene: buffa::MessageField::some(TagConfidence { label: "concert".into(), confidence: 0.77, ..Default::default() }),
        zs_mood: buffa::MessageField::some(TagConfidence { label: "energetic".into(), confidence: 0.65, ..Default::default() }),
        zs_sound_events: vec![TagConfidence { label: "applause".into(), confidence: 0.5, ..Default::default() }],
        zs_voice: buffa::MessageField::some(TagConfidence { label: "male".into(), confidence: 0.9, ..Default::default() }),
        description_en: "Crowd cheering at a concert venue.".into(),
        description_zh: "音乐会场地人群欢呼".into(),
        gemini_scene: "live concert".into(),
        gemini_mood: "excited".into(),
        gemini_sound_sources: vec![SoundSource { name: "crowd".into(), prominence: "foreground".into(), description: "cheering crowd".into(), ..Default::default() }],
        gemini_foreground: "applause and cheers".into(),
        gemini_background: "ambient noise".into(),
        gemini_enhanced: true,
        event_timeline: vec![AudioEvent { event_type: "applause".into(), start_ms: 1000, end_ms: 4000, avg_confidence: 0.8, ..Default::default() }],
        foreground_layer: "vocals".into(),
        background_layer: "music".into(),
        speech_ratio: 0.6,
        speaker_count: 2,
        speaker_segments: vec![SpeakerSegment { start_ms: 0, end_ms: 2500, speaker_id: 1, ..Default::default() }],
        voice_gender: buffa::MessageField::some(TagConfidence { label: "male".into(), confidence: 0.85, ..Default::default() }),
        voice_emotion: buffa::MessageField::some(TagConfidence { label: "excited".into(), confidence: 0.72, ..Default::default() }),
        transcript: "Hello everyone welcome to the show.".into(),
        transcript_segments: vec![AudioTranscriptSegment { start_ms: 100, end_ms: 900, text: "hello".into(), language: "en".into(), confidence: 0.97, ..Default::default() }],
        language: "eng".into(),
        music_genre: buffa::MessageField::some(TagConfidence { label: "rock".into(), confidence: 0.82, ..Default::default() }),
        music_bpm: Some(120.0),
        music_instruments: vec![TagConfidence { label: "guitar".into(), confidence: 0.75, ..Default::default() }],
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
        id: vec![0x10],
        audio_id: vec![0x11],
        scene_id: None,
        kind: buffa::EnumValue::from(AudioClipKind::AUDIO_CLIP_KIND_UNSPECIFIED),
        start_ms: 0,
        end_ms: 0,
        track_id: vec![],
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
        id: vec![0x20, 0x21],
        audio_id: vec![0x22, 0x23],
        track_index: 1,
        codec: buffa::EnumValue::from(AudioCodec::AUDIO_CODEC_AAC),
        sample_format: buffa::EnumValue::from(AudioSampleFormat::AUDIO_SAMPLE_FORMAT_FLTP),
        sample_rate: 48000,
        channels: 2,
        channel_layout: buffa::EnumValue::from(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_STEREO),
        bit_rate: 192_000,
        bit_depth: 16,
        total_pts: 2_304_000,
        time_base: buffa::MessageField::some(
            mediatime::Timebase::new(1, NonZeroU32::new(48000).unwrap())
        ),
        language: 0x00_65_6E_67, // "eng" ISO 639-2B
        timecode: buffa::MessageField::some(Timecode {
            start: "00:00:00:00".into(),
            end: "01:23:45:12".into(),
            fps: 25.0,
            drop_frame: false,
            ..Default::default()
        }),
        classification: buffa::EnumValue::from(TrackClassificationType::TRACK_CLASSIFICATION_TYPE_VOICE),
        stop_reason: 0x02,
        tags: vec![TrackTag {
            category: "ambience".into(),
            detections: vec![Detection { label: "wind".into(), confidence: 0.6, ..Default::default() }],
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
            audio_detection: buffa::MessageField::some(Detection { label: "music".into(), confidence: 0.9, ..Default::default() }),
            scene: buffa::MessageField::some(Detection { label: "concert".into(), confidence: 0.8, ..Default::default() }),
            mood: buffa::MessageField::some(Detection { label: "energetic".into(), confidence: 0.7, ..Default::default() }),
            voice: buffa::MessageField::some(Detection { label: "singing".into(), confidence: 0.6, ..Default::default() }),
            sound_events: vec![Detection { label: "applause".into(), confidence: 0.5, ..Default::default() }],
            ..Default::default()
        }),
        ced: buffa::MessageField::some(Ced {
            tags: vec![
                CedDetection { tag: 1, confidence: 0.5, ..Default::default() },
                CedDetection { tag: 0xFFFF_FFFF_FFFF_FFFF, confidence: 0.9, ..Default::default() },
            ],
            ..Default::default()
        }),
        chromaprint: buffa::MessageField::some(Chromaprint {
            fingerprint: vec![0x01, 0x02, 0x03, 0x04],
            fingerprint_duration: 120.5,
            ..Default::default()
        }),
        index_status: 0x01 | 0x100 | 0x400,
        index_error: buffa::MessageField::some(ErrorInfo { code: 7, message: "partial".into(), ..Default::default() }),
        error_status: 1,
        ..Default::default()
    };
    rt(&tr_full);

    // ── TrackRecord: 1000s band all none(), timecode none(), index_error none() ──
    // (proves reserved 17 + 1000s/2000s bands round-trip)
    let tr_min = TrackRecord {
        id: vec![0x30],
        audio_id: vec![0x31],
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
        classification: buffa::EnumValue::from(TrackClassificationType::TRACK_CLASSIFICATION_TYPE_UNSPECIFIED),
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
        analyses: vec![vec![1], vec![2]],
        summary: make_audio_summary_mf(),
        index_status: 0x01 | 0x20,
        index_error: buffa::MessageField::some(ErrorInfo { code: 5, message: "x".into(), ..Default::default() }),
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
        audio_id: vec![0xC1, 0xC2],
        index_error: buffa::MessageField::some(ErrorInfo { code: 3, message: "err".into(), ..Default::default() }),
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
        id: vec![0x01, 0x02, 0x03, 0x04],
        audio_id: vec![0x05, 0x06, 0x07, 0x08],
        scene_id: Some(vec![5]),
        kind: buffa::EnumValue::from(AudioClipKind::AUDIO_CLIP_KIND_FIXED_WINDOW),
        start_ms: 1000,
        end_ms: 5000,
        track_id: vec![0x09, 0x0A],
        ced_tags: vec![CedDetection { tag: 0xDEAD_BEEF_0000_0001, confidence: 0.7, ..Default::default() }],
        zs_audio_type: buffa::MessageField::some(TagConfidence { label: "speech".into(), confidence: 0.88, ..Default::default() }),
        zs_scene: buffa::MessageField::some(TagConfidence { label: "concert".into(), confidence: 0.77, ..Default::default() }),
        zs_mood: buffa::MessageField::some(TagConfidence { label: "energetic".into(), confidence: 0.65, ..Default::default() }),
        zs_sound_events: vec![TagConfidence { label: "applause".into(), confidence: 0.5, ..Default::default() }],
        zs_voice: buffa::MessageField::some(TagConfidence { label: "male".into(), confidence: 0.9, ..Default::default() }),
        description_en: "Crowd cheering at a concert venue.".into(),
        description_zh: "音乐会场地人群欢呼".into(),
        gemini_scene: "live concert".into(),
        gemini_mood: "excited".into(),
        gemini_sound_sources: vec![SoundSource { name: "crowd".into(), prominence: "foreground".into(), description: "cheering crowd".into(), ..Default::default() }],
        gemini_foreground: "applause and cheers".into(),
        gemini_background: "ambient noise".into(),
        gemini_enhanced: true,
        event_timeline: vec![AudioEvent { event_type: "applause".into(), start_ms: 1000, end_ms: 4000, avg_confidence: 0.8, ..Default::default() }],
        foreground_layer: "vocals".into(),
        background_layer: "music".into(),
        speech_ratio: 0.6,
        speaker_count: 2,
        speaker_segments: vec![SpeakerSegment { start_ms: 0, end_ms: 2500, speaker_id: 1, ..Default::default() }],
        voice_gender: buffa::MessageField::some(TagConfidence { label: "male".into(), confidence: 0.85, ..Default::default() }),
        voice_emotion: buffa::MessageField::some(TagConfidence { label: "excited".into(), confidence: 0.72, ..Default::default() }),
        transcript: "Hello everyone welcome to the show.".into(),
        transcript_segments: vec![AudioTranscriptSegment { start_ms: 100, end_ms: 900, text: "hello".into(), language: "en".into(), confidence: 0.97, ..Default::default() }],
        language: "eng".into(),
        music_genre: buffa::MessageField::some(TagConfidence { label: "rock".into(), confidence: 0.82, ..Default::default() }),
        music_bpm: Some(120.0),
        music_instruments: vec![TagConfidence { label: "guitar".into(), confidence: 0.75, ..Default::default() }],
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
        id: vec![0x20, 0x21],
        audio_id: vec![0x22, 0x23],
        track_index: 1,
        codec: buffa::EnumValue::from(AudioCodec::AUDIO_CODEC_AAC),
        sample_format: buffa::EnumValue::from(AudioSampleFormat::AUDIO_SAMPLE_FORMAT_FLTP),
        sample_rate: 48000,
        channels: 2,
        channel_layout: buffa::EnumValue::from(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_STEREO),
        bit_rate: 192_000,
        bit_depth: 16,
        total_pts: 2_304_000,
        time_base: buffa::MessageField::some(
            mediatime::Timebase::new(1, NonZeroU32::new(48000).unwrap())
        ),
        language: 0x00_65_6E_67,
        timecode: buffa::MessageField::some(Timecode {
            start: "00:00:00:00".into(),
            end: "01:23:45:12".into(),
            fps: 25.0,
            drop_frame: false,
            ..Default::default()
        }),
        classification: buffa::EnumValue::from(TrackClassificationType::TRACK_CLASSIFICATION_TYPE_VOICE),
        stop_reason: 0x02,
        tags: vec![TrackTag {
            category: "ambience".into(),
            detections: vec![Detection { label: "wind".into(), confidence: 0.6, ..Default::default() }],
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
            audio_detection: buffa::MessageField::some(Detection { label: "music".into(), confidence: 0.9, ..Default::default() }),
            scene: buffa::MessageField::some(Detection { label: "concert".into(), confidence: 0.8, ..Default::default() }),
            mood: buffa::MessageField::some(Detection { label: "energetic".into(), confidence: 0.7, ..Default::default() }),
            voice: buffa::MessageField::some(Detection { label: "singing".into(), confidence: 0.6, ..Default::default() }),
            sound_events: vec![Detection { label: "applause".into(), confidence: 0.5, ..Default::default() }],
            ..Default::default()
        }),
        ced: buffa::MessageField::some(Ced {
            tags: vec![
                CedDetection { tag: 1, confidence: 0.5, ..Default::default() },
                CedDetection { tag: 0xFFFF_FFFF_FFFF_FFFF, confidence: 0.9, ..Default::default() },
            ],
            ..Default::default()
        }),
        chromaprint: buffa::MessageField::some(Chromaprint {
            fingerprint: vec![0x01, 0x02, 0x03, 0x04],
            fingerprint_duration: 120.5,
            ..Default::default()
        }),
        index_status: 0x01 | 0x100 | 0x400,
        index_error: buffa::MessageField::some(ErrorInfo { code: 7, message: "partial".into(), ..Default::default() }),
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
        dimensions: buffa::MessageField::some(Dimensions { width: 600, height: 600, ..Default::default() }),
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
        id: vec![1, 2],
        checksum: Some((1u8..=32).collect()),
        name: "track.flac".into(),
        format: buffa::EnumValue::from(AudioFormat::AUDIO_FORMAT_FLAC),
        size: 52_428_800,
        total_pts: 2_646_000,
        frame_rate: 23.976,
        bit_rate: 1_411_200,
        time_base: buffa::MessageField::some(mediatime::Timebase::new(1, NonZeroU32::new(44100).unwrap())),
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
            dimensions: buffa::MessageField::some(Dimensions { width: 600, height: 600, ..Default::default() }),
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
        id: vec![3, 4],
        checksum: None,
        name: "sparse.flac".into(),
        format: buffa::EnumValue::from(AudioFormat::AUDIO_FORMAT_FLAC),
        size: 1024,
        total_pts: 100,
        frame_rate: 44100.0,
        bit_rate: 320_000,
        time_base: buffa::MessageField::some(mediatime::Timebase::new(1, NonZeroU32::new(48000).unwrap())),
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
        id: vec![1, 2],
        checksum: Some((1u8..=32).collect()),
        name: "track.flac".into(),
        format: buffa::EnumValue::from(AudioFormat::AUDIO_FORMAT_FLAC),
        size: 52_428_800,
        total_pts: 2_646_000,
        frame_rate: 23.976,
        bit_rate: 1_411_200,
        time_base: buffa::MessageField::some(mediatime::Timebase::new(1, NonZeroU32::new(44100).unwrap())),
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
            dimensions: buffa::MessageField::some(Dimensions { width: 600, height: 600, ..Default::default() }),
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
