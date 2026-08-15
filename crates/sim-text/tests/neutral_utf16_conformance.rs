use sim_text::{
    CodeUnitOffset, CodeUnitRange, CodeUnitString, code_unit_string_from_expr,
    code_unit_string_to_expr,
};

const SPECIMEN_SOURCE: &str = include_str!("neutral_utf16_conformance.rs");
const FEATURE_INDEX: &str = include_str!("../../../features.toml");

#[test]
fn exact_utf16_is_a_language_neutral_value() {
    let exact =
        CodeUnitString::from_code_units(vec![0x0041, 0xd83d, 0xde00, 0xd800, 0x0062, 0xdc00]);

    assert_eq!(exact.len(), 6);
    assert_eq!(exact.code_unit_at(CodeUnitOffset::new(1)), Some(0xd83d));
    assert_eq!(
        exact
            .slice(CodeUnitRange::new(
                CodeUnitOffset::new(1),
                CodeUnitOffset::new(4),
            ))
            .as_code_units(),
        [0xd83d, 0xde00, 0xd800],
    );
    assert_eq!(
        exact.code_units().collect::<Vec<_>>(),
        [0x0041, 0xd83d, 0xde00, 0xd800, 0x0062, 0xdc00],
    );
    assert_eq!(
        exact
            .iter_code_points()
            .map(|part| part.as_code_units().to_vec())
            .collect::<Vec<_>>(),
        [
            vec![0x0041],
            vec![0xd83d, 0xde00],
            vec![0xd800],
            vec![0x0062],
            vec![0xdc00],
        ],
    );

    // Browse the exact face directly, without asking for scalar projection.
    assert_eq!(
        exact.as_code_units(),
        [0x0041, 0xd83d, 0xde00, 0xd800, 0x0062, 0xdc00],
    );

    let encoded = code_unit_string_to_expr(&exact);
    assert_eq!(code_unit_string_from_expr(&encoded).unwrap(), exact);
    let rendered = format!("{encoded:?}");
    for unit in [0xd83d_u16, 0xde00, 0xd800, 0xdc00] {
        let bytes = unit.to_be_bytes();
        assert!(rendered.contains(&bytes[0].to_string()));
        assert!(rendered.contains(&bytes[1].to_string()));
    }
}

#[test]
fn specimen_and_index_query_prove_neutral_ownership() {
    let imports = SPECIMEN_SOURCE
        .lines()
        .filter(|line| line.trim_start().starts_with("use "))
        .collect::<Vec<_>>();
    assert_eq!(imports.len(), 1);
    assert!(imports[0].starts_with("use sim_text::"));
    let forbidden = [
        ["java", "script"].concat(),
        ["j", "vm"].concat(),
        ["pat", "tern"].concat(),
    ];
    for guest_owner in forbidden {
        assert!(!SPECIMEN_SOURCE.to_ascii_lowercase().contains(&guest_owner));
    }

    let utf16_hits = FEATURE_INDEX
        .split("[[feature]]")
        .chain(FEATURE_INDEX.split("[[route]]"))
        .filter(|record| record.to_ascii_lowercase().contains("utf-16"))
        .collect::<Vec<_>>();
    assert!(
        !utf16_hits.is_empty(),
        "Index find utf-16 must return a row"
    );
    assert!(utf16_hits.iter().any(|record| {
        record.contains("feature/sim-foundation/exact-code-unit-text")
            && record.contains("crate/sim-text")
    }));
}
