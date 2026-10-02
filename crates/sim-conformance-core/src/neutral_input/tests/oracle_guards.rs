// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Independent oracles: expected facts, identities and bounds are spelled out
//! here separately from the production derivations, so a self-consistent wrong
//! literal, slot permutation or off-by-one cannot hide behind shared code.

use sim_kernel::{Datum, Symbol};

use super::*;

fn n(domain_value: u64) -> Datum {
    Datum::Number(sim_kernel::NumberLiteral {
        domain: Symbol::qualified("qualification", "u64"),
        canonical: domain_value.to_string(),
    })
}

#[test]
fn the_exact_fixture_states_every_fact_and_the_precision_independently() {
    let measured = RestatedMeasurements::from_source(&source()).unwrap();
    let f = &measured.facts;
    assert_eq!((f.scheduled, f.materialized, f.revisions), (270, 269, 230));
    assert_eq!(
        (
            f.epochs.failed,
            f.epochs.invalidated,
            f.epochs.interrupted_running,
            f.epochs.total
        ),
        (37, 4, 1, 42)
    );
    let p = &f.proof_classes;
    assert_eq!(
        (p.generate, p.standalone, p.workspace, p.package, p.total),
        (60, 183, 10, 27, 280)
    );
    assert_eq!(f.lifecycle_rows, 458_386);
    assert_eq!((f.spans.total, f.spans.finished), (103_261, 103_213));
    assert_eq!(f.overlapping_span_milliseconds, 2_027_988_000);
    assert_eq!(
        (f.repeated_ready_rows, f.repeated_receipt_rows),
        (188_393, 134_616)
    );
    let fr = &f.last_frontier;
    assert_eq!(
        (fr.green, fr.interrupted, fr.pending, fr.total),
        (26, 1, 253, 280)
    );
    let reuse: Vec<(ReuseContrast, u64, u64)> = f
        .contrast_reuse
        .iter()
        .map(|r| (r.contrast, r.retained, r.denominator))
        .collect();
    assert_eq!(
        reuse,
        vec![
            (ReuseContrast::ControlOnlyRepair, 213, 280),
            (ReuseContrast::PublicHeadAndPinChange, 1, 280),
            (ReuseContrast::LastRawEpoch, 2, 280),
            (ReuseContrast::PreviousFixedPoint, 280, 280),
        ]
    );
    assert_eq!(measured.precision.source_decimal_places, 2);
}

#[test]
fn source_identity_and_conversion_recipe_match_independently_spelled_data() {
    let bytes = source();
    let expected_source = Datum::Node {
        tag: Symbol::qualified("source-deck", "bytes-v1"),
        fields: vec![(Symbol::new("bytes"), Datum::Bytes(bytes.clone()))],
    }
    .content_id()
    .unwrap();
    assert_eq!(source_byte_identity(&bytes).unwrap(), expected_source);
    let sym = |v: &str| Datum::Symbol(Symbol::qualified("qualification", v));
    let expected_procedure = Datum::Node {
        tag: Symbol::qualified("qualification", "restated-measurements-procedure-v1"),
        fields: vec![
            (Symbol::new("basis"), sym("restated-measurements")),
            (
                Symbol::new("calculation"),
                sym("exact-rational-hours-to-milliseconds"),
            ),
            (Symbol::new("milliseconds-per-hour"), n(3_600_000)),
            (Symbol::new("result-milliseconds"), n(2_027_988_000)),
            (Symbol::new("source-hour-scale"), n(100)),
            (Symbol::new("source-hours-hundredths"), n(56_333)),
            (Symbol::new("source-precision"), sym("rounded-to-0.01-hour")),
        ],
    }
    .content_id()
    .unwrap();
    assert_eq!(
        RestatedMeasurements::from_source(&bytes).unwrap().procedure,
        expected_procedure
    );
}

#[test]
fn same_width_signed_fractions_and_wrapping_sums_are_refused() {
    use super::super::grammar::template;
    assert_eq!(template("563.+3 hours", "{}.{f2} hours"), None);
    assert_eq!(template("563.-3 hours", "{}.{f2} hours"), None);
    let real = String::from_utf8(source()).unwrap();
    for (from, to, message) in [
        (
            "563.33 hours",
            "563.+3 hours",
            "row malformed: Sum of overlapping span durations",
        ),
        (
            "42: 37 failed, 4 invalidated,",
            "42: 18,446,744,073,709,551,615 failed, 43 invalidated,",
            "epoch counts do not sum to total",
        ),
        (
            "280: 60 generate, 183 standalone,",
            "280: 18,446,744,073,709,551,615 generate, 281 standalone,",
            "proof classes do not sum to total",
        ),
    ] {
        let m = real.replacen(from, to, 1);
        let m = m.replacen(
            "1 interrupted/running raw state",
            "0 interrupted/running raw state",
            usize::from(to.contains("epoch") || to.contains("failed")),
        );
        let m = if to.contains("generate") {
            m.replacen("10 workspace, 27 package", "0 workspace, 0 package", 1)
        } else {
            m
        };
        assert_ne!(m, real, "{to:?}");
        let error = RestatedMeasurements::from_source(m.as_bytes()).unwrap_err();
        assert!(error.to_string().contains(message), "{to:?}: {error}");
    }
}

#[test]
fn authoring_names_of_exactly_ninety_six_characters_are_accepted() {
    let long = |p: &str| Symbol::qualified("authoring", format!("{p}{}", "a".repeat(96 - p.len())));
    let commission = TrustedCommission {
        producer: long("p"),
        authority: long("u"),
        review: long("r"),
    };
    let input = NeutralQualificationInput::author_restated(&source(), &commission).unwrap();
    input
        .verify_restated_measurements(&source(), &commission)
        .unwrap();
    let too_long = TrustedCommission {
        producer: Symbol::qualified("authoring", "a".repeat(97)),
        ..commission
    };
    assert!(NeutralQualificationInput::author_restated(&source(), &too_long).is_err());
}

#[test]
fn registered_shape_constants_are_pinned_to_independent_literals() {
    assert_eq!(NEUTRAL_INPUT_VERSION, 1);
    assert_eq!(MAX_NEUTRAL_INPUT_BYTES, 65_536);
    let datum = authored().to_datum().unwrap();
    let Datum::Node { tag, fields } = &datum else {
        panic!("node")
    };
    // Independently spelled tag, namespace and stored version literal.
    assert_eq!(*tag, Symbol::qualified("qualification", "neutral-input-v1"));
    let version = fields.iter().find(|(n, _)| &*n.name == "version").unwrap();
    assert_eq!(version.1, n(1));
    // The stored version is exactly 1: 0 and 2 are both unsupported.
    for stored in [0u64, 2] {
        let mutated = edit(&datum, &["version"], &|_| n(stored));
        assert_eq!(
            NeutralQualificationInput::from_datum(&mutated)
                .unwrap_err()
                .gap,
            NeutralInputGap::UnsupportedVersion,
            "{stored}"
        );
    }
}
