// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! One negative per decoder guard: each stored-form mutation keeps every other
//! field and id coherent, so only the guard under test can refuse it, and the
//! assertion binds to that guard's own message (a weakened guard either admits
//! the input or fails with a different message).

use sim_kernel::{Datum, Symbol};

use super::*;

fn number(domain: &str, canonical: &str) -> Datum {
    Datum::Number(sim_kernel::NumberLiteral {
        domain: Symbol::qualified("qualification", domain),
        canonical: canonical.to_owned(),
    })
}

fn source_entry(datum: &Datum, field: &str, value: Datum) -> Datum {
    edit(datum, &["derivation", "source-digests"], &|d| {
        let Datum::Vector(mut items) = d else {
            panic!("vector")
        };
        items[0] = edit(&items[0], &[field], &|_| value.clone());
        Datum::Vector(items)
    })
}

fn refused(datum: &Datum, message: &str) {
    let error = NeutralQualificationInput::from_datum(datum).expect_err(message);
    assert!(
        error.to_string().contains(message),
        "expected {message:?}, got {error}"
    );
}

#[test]
fn every_decoder_guard_refuses_its_own_spelling() {
    let base = authored().to_datum().unwrap();
    assert!(NeutralQualificationInput::from_datum(&base).is_ok());
    let sym = |ns: &str, name: &str| Datum::Symbol(Symbol::qualified(ns, name));
    let long = Datum::Bytes(vec![7; 33]);
    refused(
        &edit(&base, &["facts", "revisions"], &|_| {
            number("other-u64", "230")
        }),
        "revisions is not a u64",
    );
    refused(
        &edit(&base, &["facts", "revisions"], &|_| number("u64", "0230")),
        "revisions not canonical u64",
    );
    refused(
        &edit(&base, &["facts", "revisions"], &|_| number("u64", "+230")),
        "revisions not canonical u64",
    );
    refused(
        &edit(&base, &["selected-source", "digest"], &|_| long.clone()),
        "digest is not 32 bytes",
    );
    // The frontier total diverges from the proof-leaf total while its own sum stays coherent.
    let frontier = edit(&base, &["facts", "last-frontier", "total"], &|_| {
        number("u64", "281")
    });
    let frontier = edit(&frontier, &["facts", "last-frontier", "green"], &|_| {
        number("u64", "27")
    });
    refused(&frontier, "frontier total differs from proof-leaf total");
    refused(
        &edit(&base, &["derivation", "facts", "digest"], &|_| {
            Datum::Bytes(vec![9; 32])
        }),
        "receipt facts reference is not the facts id",
    );
    refused(
        &edit(&base, &["derivation", "selected-source", "digest"], &|_| {
            Datum::Bytes(vec![9; 32])
        }),
        "receipt and outer selected-source differ",
    );
    refused(
        &edit(&base, &["derivation", "basis"], &|_| {
            sym("qualification", "invented-basis")
        }),
        "unregistered basis",
    );
    refused(
        &edit(&base, &["derivation", "precision", "output-unit"], &|_| {
            sym("unit", "second")
        }),
        "precision units or rounding unregistered",
    );
    refused(
        &edit(&base, &["derivation", "no-runtime-consumption"], &|_| {
            sym("qualification", "true-ish")
        }),
        "no-runtime-consumption is not a bool",
    );
    refused(
        &source_entry(&base, "algorithm", sym("core", "blake3")),
        "source digest algorithm",
    );
    refused(
        &source_entry(&base, "role", sym("qualification", "invented-role")),
        "source role",
    );
    refused(
        &source_entry(&base, "digest", long.clone()),
        "source digest is not 32 bytes",
    );
}

#[test]
fn a_malformed_component_numeral_is_not_a_fallback_zero() {
    let real = String::from_utf8(source()).unwrap();
    let mutated = real.replacen("26 green,", "x green,", 1);
    assert_ne!(mutated, real);
    let error = RestatedMeasurements::from_source(mutated.as_bytes()).unwrap_err();
    assert!(error.to_string().contains("component malformed"), "{error}");
}

#[test]
fn proof_total_must_equal_frontier_total_in_the_source_too() {
    let real = String::from_utf8(source()).unwrap();
    let mutated = real.replacen(
        "26 green, 1 interrupted, 253 pending",
        "27 green, 1 interrupted, 253 pending",
        1,
    );
    assert_ne!(mutated, real);
    assert!(
        RestatedMeasurements::from_source(mutated.as_bytes())
            .and_then(|m| NeutralQualificationInput::author_restated(
                mutated.as_bytes(),
                &commission()
            )
            .map(|_| m))
            .is_err()
    );
}

#[test]
fn whitespace_and_edge_pipe_spellings_are_not_part_of_the_source_language() {
    let real = String::from_utf8(source()).unwrap();
    for (from, to) in [
        ("42: 37 failed,", "42:  37 failed,"),
        ("42: 37 failed,", "42: 37  failed,"),
        ("37 failed, 4 invalidated", "37 failed,  4 invalidated"),
        ("| --- | ---: |", "|| --- | ---: ||"),
        ("| --- | ---: |", "--- | ---:"),
        ("| Plan revisions | 230 |", "|| Plan revisions | 230 ||"),
        ("| Plan revisions | 230 |", "|Plan revisions | 230 |"),
        ("| Plan revisions | 230 |", "| Plan revisions  | 230 |"),
        ("| Plan revisions | 230 |", "| Plan revisions | 230  |"),
        (
            "| Retained SUP20 measure | Observed value |",
            "| Retained SUP20 measure | Observed value | ",
        ),
    ] {
        let mutated = real.replacen(from, to, 1);
        assert_ne!(mutated, real, "{to:?}");
        assert!(
            RestatedMeasurements::from_source(mutated.as_bytes()).is_err(),
            "authoring accepted {to:?}"
        );
        // And on the pinned verification path, with digests re-pinned consistently.
        let delta = to.len() as i64 - from.len() as i64;
        let regions = super::regions::shifted_regions(mutated.as_bytes(), delta);
        assert!(
            RestatedMeasurements::from_regions(mutated.as_bytes(), &regions).is_err(),
            "pinned verification accepted {to:?}"
        );
    }
}
