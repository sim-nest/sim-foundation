// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! One focused negative per row shape, contrast sentence and arithmetic guard
//! of the retained-measurement table, each bound to the guard's own refusal.

use super::*;

fn refused_with(from: &str, to: &str, message: &str) {
    let real = String::from_utf8(source()).unwrap();
    let mutated = real.replacen(from, to, 1);
    assert_ne!(mutated, real, "{to:?}");
    let error = RestatedMeasurements::from_source(mutated.as_bytes()).expect_err(to);
    assert!(error.to_string().contains(message), "{to:?}: {error}");
}

#[test]
fn every_row_shape_refuses_each_malformed_spelling_for_its_own_row() {
    for (from, to, row) in [
        (
            "| Materialized schedule truth | 269 of 270 |",
            "| Materialized schedule truth | nonsense |",
            "Materialized schedule truth",
        ),
        (
            "| Materialized schedule truth | 269 of 270 |",
            "| Materialized schedule truth | 0 of x |",
            "Materialized schedule truth",
        ),
        (
            "| Materialized schedule truth | 269 of 270 |",
            "| Materialized schedule truth | x of 270 |",
            "Materialized schedule truth",
        ),
        (
            "42: 37 failed, 4 invalidated, 1 interrupted/running raw state",
            "0 failed, 0 invalidated, 0 interrupted/running raw state",
            "Closeout epochs",
        ),
        (
            "42: 37 failed, 4 invalidated, 1 interrupted/running raw state",
            "x: 0 failed, 0 invalidated, 0 interrupted/running raw state",
            "Closeout epochs",
        ),
        (
            "103,261, of which 103,213 finished",
            "103,261",
            "Telemetry spans",
        ),
        (
            "103,261, of which 103,213 finished",
            "x, of which 0 finished",
            "Telemetry spans",
        ),
        (
            "103,261, of which 103,213 finished",
            "103,261, of which 103,213",
            "Telemetry spans",
        ),
        (
            "563.33 hours",
            "563.33",
            "Sum of overlapping span durations",
        ),
        (
            "563.33 hours",
            "563 hours",
            "Sum of overlapping span durations",
        ),
        (
            "563.33 hours",
            "x3.33 hours",
            "Sum of overlapping span durations",
        ),
        (
            "280 green leaves",
            "garbage",
            "Previous fixed point before retirement",
        ),
        (
            "280 green leaves",
            "x green leaves",
            "Previous fixed point before retirement",
        ),
        (
            "| Reused receipts in that last epoch | 2 |",
            "| Reused receipts in that last epoch | x |",
            "Reused receipts in that last epoch",
        ),
    ] {
        refused_with(from, to, &format!("row malformed: {row}"));
    }
}

#[test]
fn hours_arithmetic_is_checked_at_every_step() {
    // whole * 100 overflows; the fractional addition overflows; hours * ms-per-hour overflows;
    // and a value that wraps to a divisible result is still refused.
    for hours in [
        "4,611,686,018,427,387,904.33 hours",
        "184,467,440,737,095,516.99 hours",
        "1,281,023,894,007.61 hours",
    ] {
        let real = String::from_utf8(source()).unwrap();
        let mutated = real.replacen("563.33 hours", hours, 1);
        assert!(
            RestatedMeasurements::from_source(mutated.as_bytes()).is_err(),
            "{hours}"
        );
    }
}

#[test]
fn contrast_sentences_are_exact_and_each_refuses_for_its_own_reason() {
    let control = "A control-only repair retained 213 of 280 proof receipts.";
    let heads = "A change to three\npublic heads and their pins retained only one.";
    for (from, to, message) in [
        (heads, "", "public-head sentence is not the second sentence"),
        (
            control,
            "213 of 280 proof receipts.",
            "control-only sentence is not the first sentence",
        ),
        (
            control,
            "A control-only repair retained x of 280 proof receipts.",
            "control-only sentence is not the first sentence",
        ),
    ] {
        let real = String::from_utf8(source()).unwrap();
        let mutated = real.replacen(from, to, 1);
        assert_ne!(mutated, real, "{to:?}");
        let error = RestatedMeasurements::from_source(mutated.as_bytes()).expect_err(to);
        assert!(error.to_string().contains(message), "{to:?}: {error}");
    }
}

#[test]
fn the_template_matcher_is_total_exact_and_never_defaults() {
    use super::super::grammar::template;
    assert_eq!(template("1,234 of 5", "{} of {}"), Some(vec![1234, 5]));
    assert_eq!(
        template("103,261, of which 7 finished", "{}, of which {} finished"),
        Some(vec![103_261, 7])
    );
    assert_eq!(
        template("563.33 hours", "{}.{f2} hours"),
        Some(vec![563, 33])
    );
    for bad in [
        "",
        "1 of",
        "of 1",
        "1 of 2 ",
        " 1 of 2",
        "1  of 2",
        "1 of x",
        "01 of 2",
        "1,0 of 2",
        "1 of 2,",
        "458386 of 2",
    ] {
        assert_eq!(template(bad, "{} of {}"), None, "{bad:?}");
    }
    for bad in [
        "563.3 hours",
        "563.333 hours",
        "563.x3 hours",
        "563,33 hours",
        "563.33hours",
    ] {
        assert_eq!(template(bad, "{}.{f2} hours"), None, "{bad:?}");
    }
    assert_eq!(template("1", "{}{}"), None);
    assert_eq!(template("1", "{x}"), None);
    // The first slot is the longest canonical prefix: a following literal may open with a comma.
    assert_eq!(template("1,2", "{},{}"), Some(vec![1, 2]));
}
