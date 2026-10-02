// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Oracles for the classes a hand audit finds after the mechanical sweeps:
//! region digests bound to their own bytes, differentiated (unequal-valued)
//! fixtures that expose wrong semantic slots, golden identities, registered
//! symbols, and the gap kind of every contract refusal.

use sha2::{Digest, Sha256};
use sim_kernel::{Datum, Symbol};

use super::*;

fn sha(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[test]
fn region_roles_starts_lengths_and_digests_are_bound_to_their_own_bytes() {
    let bytes = source();
    let text = String::from_utf8(bytes.clone()).unwrap();
    // Independent slicing: located here by their visible first and last lines.
    let table_start = text
        .find("| Retained SUP20 measure | Observed value |")
        .unwrap();
    let table_end = text
        .find("| Reused receipts in that last epoch | 2 |\n")
        .unwrap()
        + "| Reused receipts in that last epoch | 2 |\n".len();
    let para_start = text.find("A control-only repair retained").unwrap();
    // The contrast region ends just before its last line ending; the table region includes its own.
    let para_end = text.find("invalidation.\n").unwrap() + "invalidation.".len();
    let regions = RestatedMeasurements::from_source(&bytes).unwrap().regions;
    let expected = [
        (
            DerivationSourceRole::SelectedRoundSource,
            0,
            bytes.len(),
            sha(&bytes),
        ),
        (
            DerivationSourceRole::MeasurementsTableRegion,
            table_start,
            table_end - table_start,
            sha(&bytes[table_start..table_end]),
        ),
        (
            DerivationSourceRole::ContrastStatementRegion,
            para_start,
            para_end - para_start,
            sha(&bytes[para_start..para_end]),
        ),
    ];
    assert_eq!(regions.len(), 3);
    for (region, (role, start, length, digest)) in regions.iter().zip(expected) {
        assert_eq!(region.role, role);
        assert_eq!(region.start, start as u64);
        assert_eq!(region.length, length as u64);
        assert_eq!(region.digest, digest);
    }
    // The three digests are pairwise different (a whole-source digest everywhere is wrong).
    assert!(regions[0].digest != regions[1].digest && regions[1].digest != regions[2].digest);
}

fn differentiated() -> String {
    // Pairwise-unequal epoch-interrupted (2), frontier-interrupted (3), public-head (1), reused (5)
    // and decimal-places (2 is the only repeat, against epoch-interrupted: checked separately)
    // values, arithmetic still conforming (43 = 37 + 4 + 2; 280 = 26 + 3 + 251).
    String::from_utf8(source())
        .unwrap()
        .replace(
            "42: 37 failed, 4 invalidated, 1 interrupted",
            "43: 37 failed, 4 invalidated, 2 interrupted",
        )
        .replace(
            "26 green, 1 interrupted, 253 pending",
            "26 green, 2 interrupted, 252 pending",
        )
        .replace(
            "| Reused receipts in that last epoch | 2 |",
            "| Reused receipts in that last epoch | 5 |",
        )
}

#[test]
fn a_differentiated_fixture_exposes_every_wrong_semantic_slot_and_round_trips() {
    let text = differentiated();
    let input = NeutralQualificationInput::author_restated(text.as_bytes(), &commission()).unwrap();
    let f = &input.facts;
    assert_eq!(
        (
            f.epochs.failed,
            f.epochs.invalidated,
            f.epochs.interrupted_running,
            f.epochs.total
        ),
        (37, 4, 2, 43)
    );
    let fr = &f.last_frontier;
    assert_eq!(
        (fr.green, fr.interrupted, fr.pending, fr.total),
        (26, 2, 252, 280)
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
            (ReuseContrast::LastRawEpoch, 5, 280),
            (ReuseContrast::PreviousFixedPoint, 280, 280),
        ]
    );
    assert_eq!(f.proof_classes.total, 280);
    let measured = RestatedMeasurements::from_source(text.as_bytes()).unwrap();
    assert_eq!(measured.precision.source_decimal_places, 2);
    let datum = input.to_datum().unwrap();
    assert_eq!(
        NeutralQualificationInput::from_datum(&datum).unwrap(),
        input
    );
    input
        .verify_restated_measurements(text.as_bytes(), &commission())
        .unwrap();
}

#[test]
fn a_declared_proof_total_that_contradicts_its_components_is_refused() {
    let real = String::from_utf8(source()).unwrap();
    let hostile = real
        .replacen("280: 60 generate", "999: 60 generate", 1)
        .replacen(
            "retained 213 of 280 proof receipts",
            "retained 213 of 999 proof receipts",
            1,
        );
    assert_ne!(hostile, real);
    let error = RestatedMeasurements::from_source(hostile.as_bytes()).unwrap_err();
    assert_eq!(error.gap, NeutralInputGap::Arithmetic);
    assert!(
        error
            .to_string()
            .contains("proof classes do not sum to total"),
        "{error}"
    );
}

#[test]
fn registered_contrast_symbols_are_the_four_exact_spellings() {
    let datum = authored().to_datum().unwrap();
    let mut seen = Vec::new();
    fn walk(d: &Datum, out: &mut Vec<String>) {
        match d {
            Datum::Symbol(s) if s.namespace.as_deref() == Some("qualification") => {
                out.push(s.name.to_string())
            }
            Datum::Node { fields, .. } => fields.iter().for_each(|(_, v)| walk(v, out)),
            Datum::Vector(items) | Datum::List(items) => items.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }
    walk(&datum, &mut seen);
    for name in [
        "control-only-repair",
        "public-head-and-pin-change",
        "last-raw-epoch",
        "previous-fixed-point",
    ] {
        assert_eq!(
            seen.iter().filter(|s| s.as_str() == name).count(),
            1,
            "{name}"
        );
    }
    let _ = Symbol::new("x");
}

#[test]
fn the_facts_identity_is_pinned_to_its_domain_separated_golden_value() {
    let id = authored().facts.id().unwrap();
    let hex: String = id.bytes.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(id.algorithm, Symbol::qualified("core", "sha256-datum-v1"));
    // Regression anchor recorded from the implementation (not an independent derivation) ...
    assert_eq!(
        hex,
        "71d47919c14eb66ce17589533ef562d25a8f89e013157662395e24d678a94776"
    );
    // ... plus an independently spelled domain-separation tag over the same field set.
    let Datum::Node { fields, .. } = authored().facts.datum_without_id() else {
        panic!("node")
    };
    let expected = Datum::Node {
        tag: Symbol::qualified("qualification", "neutral-facts-content-v1"),
        fields,
    }
    .content_id()
    .unwrap();
    assert_eq!(id, expected);
}

#[test]
fn contract_refusals_carry_their_documented_gap_kind() {
    // Invalid UTF-8 source.
    assert_eq!(
        RestatedMeasurements::from_source(&[0xff, 0xfe])
            .unwrap_err()
            .gap,
        NeutralInputGap::SourceMismatch
    );
    // A top datum that is not a node.
    assert_eq!(
        NeutralQualificationInput::from_datum(&Datum::Bool(true))
            .unwrap_err()
            .gap,
        NeutralInputGap::Malformed
    );
    // A stored u64 beyond u64::MAX is an arithmetic gap, not a malformed one.
    let base = authored().to_datum().unwrap();
    let huge = edit(&base, &["facts", "revisions"], &|_| {
        Datum::Number(sim_kernel::NumberLiteral {
            domain: Symbol::qualified("qualification", "u64"),
            canonical: "18446744073709551616".to_owned(),
        })
    });
    assert_eq!(
        NeutralQualificationInput::from_datum(&huge)
            .unwrap_err()
            .gap,
        NeutralInputGap::Arithmetic
    );
    // A codec that decodes but cannot re-encode canonically is malformed.
    struct NoEncode(Vec<u8>, Datum);
    impl SlotCodec for NoEncode {
        fn decode(&self, bytes: &[u8]) -> core::result::Result<Datum, String> {
            (bytes == self.0.as_slice())
                .then(|| self.1.clone())
                .ok_or_else(|| "no".to_owned())
        }
        fn encode(&self, _: &Datum) -> core::result::Result<Vec<u8>, String> {
            Err("refuses".to_owned())
        }
    }
    let stored = base.canonical_bytes().unwrap();
    let codec = NoEncode(stored.clone(), base);
    assert_eq!(
        NeutralQualificationInput::admit(&stored, &codec, &source(), &commission())
            .unwrap_err()
            .gap,
        NeutralInputGap::Malformed
    );
}
