use buffa::Message;
use core::num::NonZeroU32;
use mediaschema::{BoundingBox, Detection, TimedDetection};
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
