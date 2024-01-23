use clar2wasm::tools::crosscheck_compare_only;
use proptest::proptest;

use crate::{int, uint};

const ONE_VALUE_OPS: [&str; 2] = ["sqrti", "log2"];
const TWO_VALUE_OPS: [&str; 2] = ["pow", "mod"];
const MULTI_VALUE_OPS: [&str; 4] = ["+", "-", "*", "/"];

proptest! {
  #[test]
  fn one_value_int(v1 in int()) {
    for op in &ONE_VALUE_OPS {
        crosscheck_compare_only(
            &format!("({op} {v1})")
        )
    }
}
}

proptest! {
  #[test]
  fn one_value_uint(v1 in uint()) {
    for op in &ONE_VALUE_OPS {
        crosscheck_compare_only(
            &format!("({op} {v1})")
        )
    }
}
}

proptest! {
  #[test]
  fn two_value_int(v1 in int()) {
    for op in &TWO_VALUE_OPS {
        crosscheck_compare_only(
            &format!("({op} {v1})")
        )
    }
}
}

proptest! {
  #[test]
  fn two_value_uint(v1 in int()) {
    for op in &TWO_VALUE_OPS {
        crosscheck_compare_only(
            &format!("({op} {v1})")
        )
    }
}
}

proptest! {
  // TODO: Renable this test once issue #281 is fixed
  #[test]
  #[ignore = "This must be re-enabled once issue #281 is fixed"]
  fn crossprop_multi_value_int(values in proptest::collection::vec(int(), 1..=10)) {
    for op in &MULTI_VALUE_OPS {
      let values_str = values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" ");
      crosscheck_compare_only(
          &format!("({op} {values_str})")
      )
    }
}
}

proptest! {
  #[test]
  fn crossprop_multi_value_uint(v1 in uint(), v2 in uint()) {
    for op in &MULTI_VALUE_OPS {
      crosscheck_compare_only(
          &format!("({op} {v1} {v2})")
      )
    }
  }
}
