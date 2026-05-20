use mediaschema_derive::QuickcheckArbitrary;

#[derive(Clone, PartialEq, Debug, Default, arbitrary::Arbitrary, QuickcheckArbitrary)]
struct Sample {
  a: u32,
  b: String,
  c: Vec<i64>,
}

#[test]
fn derives_quickcheck_arbitrary() {
  fn assert_arbitrary<T: quickcheck::Arbitrary>() {}
  assert_arbitrary::<Sample>();

  let mut g = quickcheck::Gen::new(64);
  let _ = <Sample as quickcheck::Arbitrary>::arbitrary(&mut g);
}

#[derive(Clone, PartialEq, Debug, arbitrary::Arbitrary, QuickcheckArbitrary)]
enum Color {
  Red,
  Green,
  Blue,
}

#[test]
fn derives_quickcheck_arbitrary_enum() {
  fn assert_arbitrary<T: quickcheck::Arbitrary>() {}
  assert_arbitrary::<Color>();

  let mut g = quickcheck::Gen::new(64);
  let _ = <Color as quickcheck::Arbitrary>::arbitrary(&mut g);
}
