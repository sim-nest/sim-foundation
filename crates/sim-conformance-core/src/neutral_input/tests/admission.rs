// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use sim_kernel::{Datum, Symbol};

use super::*;

struct FakeCodec {
    entries: Vec<(Vec<u8>, Datum)>,
}

impl SlotCodec for FakeCodec {
    fn decode(&self, bytes: &[u8]) -> core::result::Result<Datum, String> {
        self.entries
            .iter()
            .find(|(b, _)| b == bytes)
            .map(|(_, d)| d.clone())
            .ok_or_else(|| "undecodable".to_owned())
    }
    fn encode(&self, datum: &Datum) -> core::result::Result<Vec<u8>, String> {
        datum.canonical_bytes().map_err(|e| e.to_string())
    }
}

fn stored_and_codec(variant: impl Fn(Vec<u8>) -> Vec<u8>) -> (Vec<u8>, FakeCodec) {
    let datum = authored().to_datum().unwrap();
    let stored = variant(datum.canonical_bytes().unwrap());
    (
        stored.clone(),
        FakeCodec {
            entries: vec![(stored, datum)],
        },
    )
}

#[test]
fn admission_binds_the_stored_bytes_size_and_canonical_form() {
    let (stored, codec) = stored_and_codec(|b| b);
    assert!(NeutralQualificationInput::admit(&stored, &codec, &source(), &commission()).is_ok());
    // A single leading byte makes the stored form non-canonical even though it decodes.
    let (spaced, codec) = stored_and_codec(|b| [b" ".to_vec(), b].concat());
    assert_eq!(
        NeutralQualificationInput::admit(&spaced, &codec, &source(), &commission())
            .unwrap_err()
            .gap,
        NeutralInputGap::Malformed
    );
    // Oversize stored bytes are refused before decoding.
    let (padded, codec) =
        stored_and_codec(|b| [vec![b' '; MAX_NEUTRAL_INPUT_BYTES + 1], b].concat());
    assert_eq!(
        NeutralQualificationInput::admit(&padded, &codec, &source(), &commission())
            .unwrap_err()
            .gap,
        NeutralInputGap::Oversize
    );
    // Undecodable bytes are refused.
    let none = FakeCodec { entries: vec![] };
    assert_eq!(
        NeutralQualificationInput::admit(b"x", &none, &source(), &commission())
            .unwrap_err()
            .gap,
        NeutralInputGap::Malformed
    );
    // Shape-level canonical datum size still bounds from_datum.
    let d = authored().to_datum().unwrap();
    let huge = edit(&d, &["derivation", "producer"], &|_| {
        sym_datum("authoring", &"a".repeat(70_000))
    });
    assert_eq!(
        gap(NeutralQualificationInput::from_datum(&huge)),
        NeutralInputGap::Oversize
    );
    assert!(NeutralQualificationInput::check_encoded_size(MAX_NEUTRAL_INPUT_BYTES).is_ok());
    assert_eq!(
        NeutralQualificationInput::check_encoded_size(MAX_NEUTRAL_INPUT_BYTES + 1)
            .unwrap_err()
            .gap,
        NeutralInputGap::Oversize
    );
}

#[test]
fn negated_or_embedded_contrast_clauses_are_not_sentences() {
    let s = String::from_utf8(source()).unwrap();
    for (from, to) in [
        (
            "A control-only repair retained 213 of 280 proof receipts.",
            "It is false that A control-only repair retained 213 of 280 proof receipts.",
        ),
        (
            "A change to three\npublic heads and their pins retained only one.",
            "It is false that A change to three public heads and their pins retained only one.",
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
fn an_invalid_trusted_commission_is_refused_when_authoring_and_verifying() {
    let good = authored();
    for bad in [
        Symbol::qualified("authoring", "PASS"),
        Symbol::qualified("foreign", "producer"),
        Symbol::qualified("authoring", "a".repeat(97).as_str()),
    ] {
        for which in 0..3 {
            let mut c = commission();
            match which {
                0 => c.producer = bad.clone(),
                1 => c.authority = bad.clone(),
                _ => c.review = bad.clone(),
            }
            assert_eq!(
                NeutralQualificationInput::author_restated(&source(), &c)
                    .unwrap_err()
                    .gap,
                NeutralInputGap::ForbiddenText
            );
            assert_eq!(
                good.verify_restated_measurements(&source(), &c)
                    .unwrap_err()
                    .gap,
                NeutralInputGap::ForbiddenText
            );
        }
    }
}

#[test]
fn previous_fixed_point_denominator_is_its_own_population_when_populations_diverge() {
    // 281 terminal proof/frontier leaves, but 280 green leaves at the previous fixed point.
    let s = String::from_utf8(source())
        .unwrap()
        .replace(
            "280: 60 generate, 183 standalone, 10 workspace, 27 package",
            "281: 60 generate, 183 standalone, 10 workspace, 28 package",
        )
        .replace(
            "26 green, 1 interrupted, 253 pending",
            "26 green, 1 interrupted, 254 pending",
        )
        .replace(
            "retained 213 of 280 proof receipts",
            "retained 213 of 281 proof receipts",
        );
    let derived = RestatedMeasurements::from_source(s.as_bytes()).unwrap();
    let previous = derived
        .facts
        .contrast_reuse
        .iter()
        .find(|r| r.contrast == ReuseContrast::PreviousFixedPoint)
        .unwrap();
    assert_eq!((previous.retained, previous.denominator), (280, 280));
    assert_eq!(derived.facts.proof_classes.total, 281);
}
