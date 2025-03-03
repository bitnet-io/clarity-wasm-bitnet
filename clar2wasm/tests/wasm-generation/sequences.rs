use clar2wasm::tools::crosscheck;
use clarity::vm::types::{ListData, ListTypeData, SequenceData, TypeSignature};
use clarity::vm::Value;
use proptest::prelude::*;

use crate::{bool, int, prop_signature, PropValue};

proptest! {
    #![proptest_config(super::runtime_config())]

    #[test]
    fn append_value_to_list(mut values in (prop_signature(), 1usize..16).prop_flat_map(|(ty, size)| PropValue::many_from_type(ty, size))) {
        let expected = Value::cons_list_unsanitized(values.iter().cloned().map(Value::from).collect()).unwrap();

        let elem = values.pop().unwrap();
        let values = PropValue::try_from(values).unwrap();

        crosscheck(&format!("(append {values} {elem})"), Ok(Some(expected)))
    }
}

proptest! {
    #![proptest_config(super::runtime_config())]

    #[test]
    fn as_max_len_equal_max_len_is_some((max_len, value) in (0usize..=16).prop_ind_flat_map2(PropValue::any_sequence)) {
        crosscheck(
            &format!("(as-max-len? {value} u{max_len})"),
            Ok(Some(Value::some(value.into()).unwrap()))
        )
    }

    #[test]
    fn as_max_len_smaller_than_len_is_none((max_len, value) in (1usize..=16).prop_ind_flat_map2(PropValue::any_sequence)) {
        crosscheck(
            &format!("(as-max-len? {value} u{})", max_len-1),
            Ok(Some(Value::none()))
        )
    }
}

proptest! {
    #![proptest_config(super::runtime_config())]

    #[test]
    fn concat_crosscheck((seq1, seq2) in (0usize..=16).prop_flat_map(PropValue::any_sequence).prop_ind_flat_map2(|seq1| PropValue::from_type(TypeSignature::type_of(&seq1.into()).expect("Could not get type signature")))) {
        let snippet = format!("(concat {seq1} {seq2})");

        let expected = {
            let Value::Sequence(mut seq_data1) = seq1.into() else { unreachable!() };
            let Value::Sequence(seq_data2) = seq2.into() else { unreachable!() };
            seq_data1.concat(&clarity::types::StacksEpochId::latest(), seq_data2).expect("Unable to concat sequences");
            Value::Sequence(seq_data1)
        };

        crosscheck(&snippet, Ok(Some(expected)));
    }
}

proptest! {
    #![proptest_config(super::runtime_config())]

    #[test]
    fn element_at_crosscheck((seq, idx) in (1usize..=16).prop_flat_map(|max_len| (PropValue::any_sequence(max_len), (0..max_len)))) {
        let snippet = format!("(element-at? {seq} u{idx})");

        let expected = {
            let Value::Sequence(seq_data) = seq.into() else { unreachable!() };
            seq_data.element_at(idx).expect("element_at failed").map_or_else(Value::none, |v| Value::some(v).unwrap())
        };

        crosscheck(&snippet, Ok(Some(expected)));
    }
}

proptest! {
    #![proptest_config(super::runtime_config())]

    #[test]
    fn len_crosscheck(seq in (1usize..=16).prop_flat_map(PropValue::any_sequence)) {
        let snippet = format!("(len {seq})");

        let expected = {
            let Value::Sequence(seq_data) = seq.into() else { unreachable!() };
            Value::UInt(seq_data.len() as u128)
        };

        crosscheck(&snippet, Ok(Some(expected)));
    }
}

proptest! {
    #![proptest_config(super::runtime_config())]

    #[test]
    fn slice_crosscheck_valid_range(
        (seq, lo, hi) in (1usize..=16)
        .prop_flat_map(PropValue::any_sequence)
        .prop_ind_flat_map2(|seq| 0..extract_sequence(seq).len())
        .prop_ind_flat_map2(|(seq, lo)| lo..extract_sequence(seq).len())
        .prop_map(|((seq, lo), hi)| (seq, lo, hi))
    )
    {
        let snippet = format!("(slice? {seq} u{lo} u{hi})");

        let expected =
            Value::some(
                extract_sequence(seq)
                .slice(&clarity::types::StacksEpochId::latest(), lo, hi)
                .expect("Could not take a slice from sequence")
            ).unwrap();

        crosscheck(&snippet, Ok(Some(expected)));
    }
}

proptest! {
    #![proptest_config(super::runtime_config())]

    #[test]
    fn crosscheck_map_add(
        seq in proptest::collection::vec(proptest::collection::vec(1u128..=1000, 1..=100), 1..=50)
    ) {

        let result: Vec<_> = seq.iter()
        .skip(1).fold(seq[0].clone(), |acc, vecint| {
            acc.into_iter()
            .zip(vecint.iter())
            .map(|(x, y)| x + y)
            .collect()
        })
        .iter().map(|el| Value::UInt(*el)).collect();

        let expected = Value::Sequence(
            SequenceData::List(
                ListData {
                    data: result.clone(),
                    type_signature: ListTypeData::new_list(TypeSignature::UIntType, result.len() as u32).unwrap()
                }
            )
        );

        let lists: Vec<_> = seq.iter().map(|v| {
            v.iter().map(|&el| {
                Value::UInt(el)
            }).collect::<Vec<_>>()
        })
        .map(|v| {
            Value::Sequence(
                SequenceData::List(
                    ListData {
                        data: v.clone(),
                        type_signature: ListTypeData::new_list(TypeSignature::UIntType, v.len() as u32).unwrap()
                    }
                )
            )
        })
        .map(PropValue::from).collect();

        let lists_str: String = lists.iter().map(|el| el.to_string() + " ").collect();
        let snippet = format!("(map + {})", lists_str);

        crosscheck(
            &snippet,
            Ok(Some(expected))
        )
    }
}

proptest! {
    #![proptest_config(super::runtime_config())]

    #[test]
    fn crosscheck_map_not(
        seq in proptest::collection::vec(bool(), 1..=100)
        .prop_map(|v| {
            Value::Sequence(
                SequenceData::List(
                    ListData {
                        data: v.clone(),
                        type_signature: ListTypeData::new_list(TypeSignature::BoolType, v.len() as u32).unwrap()
                    }
                )
            )
        }).prop_map(PropValue::from)
    ) {
        let expected = extract_sequence(seq.clone());
        let snippet = format!("(map not (map not {seq}))");

        crosscheck(
            &snippet,
            Ok(Some(Value::Sequence(expected)))
        )
    }
}

proptest! {
    #![proptest_config(super::runtime_config())]

    #[test]
    fn crosscheck_map_concat_int(
        seq_1 in proptest::collection::vec(int(), 1..=100)
            .prop_map(|v| {
                Value::Sequence(
                    SequenceData::List(
                        ListData {
                            data: v.clone(),
                            type_signature: ListTypeData::new_list(TypeSignature::IntType, v.len() as u32).unwrap()
                        }
                    )
                )
            }).prop_map(PropValue::from),
        seq_2 in proptest::collection::vec(int(), 1..=100)
            .prop_map(|v| {
                Value::Sequence(
                    SequenceData::List(
                        ListData {
                            data: v.clone(),
                            type_signature: ListTypeData::new_list(TypeSignature::IntType, v.len() as u32).unwrap()
                        }
                    )
                )
            }).prop_map(PropValue::from)
    ) {
        let mut expected = extract_sequence(seq_1.clone());
        expected.concat(
            &clarity::types::StacksEpochId::latest(),
            extract_sequence(seq_2.clone())
        ).expect("Could not concat sequences");

        crosscheck(
            &format!(r#"(define-private (fun (a (list 100 int)) (b (list 100 int))) (concat a b)) (try! (element-at (map fun (list {seq_1}) (list {seq_2})) u0))"#),
            Ok(Some(Value::Sequence(expected)))
        )
    }
}

fn extract_sequence(sequence: PropValue) -> SequenceData {
    match Value::from(sequence) {
        Value::Sequence(seq_data) => seq_data,
        _ => panic!("Should only call this function on the result of PropValue::any_sequence"),
    }
}
