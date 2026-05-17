use buffa::Message;
use core::num::NonZeroU32;
use mediaschema::{
    Aesthetics, AudioFormat, BoundingBox, CodecId, Detection, Dimensions, DocumentSegment,
    FeaturePrint, HorizonInfo, MediaKind, Point2D, TimedDetection, VideoFormat,
};
use mediaschema::media_kind::Kind as MediaKindKind;
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
        let _ = (width, height); // prevent unused-variable lint
        let d = Dimensions { width, height, ..Default::default() };
        let bytes = d.encode_to_vec();
        let ok = Dimensions::decode_from_slice(&bytes).map(|b| b == d).unwrap_or(false);
        quickcheck::TestResult::from_bool(ok)
    }
    quickcheck::quickcheck(prop as fn(u32, u32) -> quickcheck::TestResult);
}
