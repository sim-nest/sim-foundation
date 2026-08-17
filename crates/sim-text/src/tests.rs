use super::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[test]
fn length_index_slice_and_code_unit_iteration_are_utf16() {
    let string = CodeUnitString::from_scalar("A\u{1f600}B");
    assert_eq!(string.len(), 4);
    assert_eq!(string.code_unit_at(CodeUnitOffset::new(1)), Some(0xd83d));
    assert_eq!(string.as_code_units(), [0x0041, 0xd83d, 0xde00, 0x0042]);
    assert_eq!(
        string
            .slice(CodeUnitRange::new(
                CodeUnitOffset::new(1),
                CodeUnitOffset::new(3)
            ))
            .as_code_units(),
        [0xd83d, 0xde00]
    );
    assert_eq!(string.code_units().count(), 4);
}

#[test]
fn scalar_conversion_and_paired_iteration_are_exact() {
    let string = CodeUnitString::from_code_units(vec![0xd83d, 0xde00]);
    assert_eq!(string.to_scalar().unwrap(), "\u{1f600}");
    assert_eq!(
        string.iter_code_points().next().unwrap().as_code_units(),
        [0xd83d, 0xde00]
    );
}

#[test]
fn lone_high_surrogate_is_preserved_and_rejected_by_scalar_face() {
    let string = CodeUnitString::from_code_units(vec![0xd800]);
    assert_eq!(string.code_unit_at(CodeUnitOffset::new(0)), Some(0xd800));
    assert_eq!(
        string.iter_code_points().next().unwrap().as_code_units(),
        [0xd800]
    );
    assert_eq!(
        string.to_scalar(),
        Err(CodeUnitStringError::LoneSurrogate(InvalidSurrogate {
            offset: CodeUnitOffset::new(0),
            unit: 0xd800
        }))
    );
}

#[test]
fn lone_low_surrogate_is_preserved_and_rejected_by_scalar_face() {
    let string = CodeUnitString::from_code_units(vec![0xdc00]);
    assert_eq!(
        string.slice(CodeUnitRange::new(
            CodeUnitOffset::new(0),
            CodeUnitOffset::new(1)
        )),
        string
    );
    assert_eq!(
        string.to_scalar(),
        Err(CodeUnitStringError::LoneSurrogate(InvalidSurrogate {
            offset: CodeUnitOffset::new(0),
            unit: 0xdc00
        }))
    );
}

#[test]
fn pair_iteration_preserves_lone_units_and_nul() {
    let string =
        CodeUnitString::from_code_units(vec![0xd800, 0x0061, 0xd83d, 0xde00, 0xdc00, 0x0000]);
    let chunks: Vec<Vec<u16>> = string
        .iter_code_points()
        .map(|chunk| chunk.code_units().collect())
        .collect();
    assert_eq!(
        chunks,
        vec![
            vec![0xd800],
            vec![0x0061],
            vec![0xd83d, 0xde00],
            vec![0xdc00],
            vec![0x0000]
        ]
    );
}

#[test]
fn offset_conversion_is_bounded_and_never_splits_a_pair() {
    let string = CodeUnitString::from_scalar("A\u{1f600}B");
    assert_eq!(
        string.code_unit_offset(ScalarOffset::new(2)),
        Ok(CodeUnitOffset::new(3))
    );
    assert_eq!(
        string.scalar_offset(CodeUnitOffset::new(3)),
        Ok(ScalarOffset::new(2))
    );
    assert_eq!(
        string.scalar_offset(CodeUnitOffset::new(2)),
        Err(OffsetConversionError::NotScalarBoundary(
            CodeUnitOffset::new(2)
        ))
    );
    assert_eq!(
        string.code_unit_offset(ScalarOffset::new(4)),
        Err(OffsetConversionError::ScalarOutOfBounds {
            offset: ScalarOffset::new(4),
            len: 3
        })
    );
}

#[test]
fn scalar_text_round_trips_for_generated_unicode_sequences() {
    let alphabet = ['\0', 'a', '\u{df}', '\u{4e2d}', '\u{1f600}', '\u{10ffff}'];
    for seed in 0usize..512 {
        let text: String = (0..seed % 17)
            .map(|at| alphabet[(seed.wrapping_mul(17) ^ at.wrapping_mul(31)) % alphabet.len()])
            .collect();
        assert_eq!(
            CodeUnitString::from_scalar(&text).to_scalar().unwrap(),
            text
        );
    }
}

#[test]
fn arbitrary_raw_units_are_preserved_by_value_equality_and_hash() {
    let mut state = 0x5eed_cafe_dead_beefu64;
    for len in 0..128 {
        let units: Vec<u16> = (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u16
            })
            .collect();
        let left = CodeUnitString::from_code_units(units.clone());
        let right = CodeUnitString::from_code_units(units.clone());
        assert_eq!(left.as_code_units(), units);
        assert_eq!(left, right);
        let mut left_hash = DefaultHasher::new();
        let mut right_hash = DefaultHasher::new();
        left.hash(&mut left_hash);
        right.hash(&mut right_hash);
        assert_eq!(left_hash.finish(), right_hash.finish());
    }
}
