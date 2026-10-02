// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use sha2::Digest;

use super::admission::HEADING;
use super::*;

fn regions_of(source: &[u8]) -> Vec<DerivationSource> {
    RestatedMeasurements::from_source(source).unwrap().regions
}

#[test]
fn verification_slices_the_pinned_regions_and_never_searches_the_document() {
    let real = source();
    let regions = regions_of(&real);
    let expected = RestatedMeasurements::from_source(&real).unwrap().facts;
    // A decoy duplicate of the heading makes LOCATING ambiguous (authoring refuses) ...
    let decoy = format!(
        "{}\n{HEADING}\n\nx\n",
        String::from_utf8(real.clone()).unwrap()
    );
    assert!(RestatedMeasurements::from_source(decoy.as_bytes()).is_err());
    // ... but the pinned regions still verify: the verifier reads exactly them.
    let mut pinned = regions.clone();
    pinned[0].length = decoy.len() as u64;
    pinned[0].digest = sha2::Sha256::digest(decoy.as_bytes()).into();
    assert_eq!(
        RestatedMeasurements::from_regions(decoy.as_bytes(), &pinned)
            .unwrap()
            .facts,
        expected
    );
}

#[test]
fn region_tampering_is_refused_for_its_own_reason() {
    let input = authored();
    let verify = |input: &NeutralQualificationInput| {
        input.verify_restated_measurements(&source(), &commission())
    };
    let mutate = |f: &dyn Fn(&mut Vec<DerivationSource>)| {
        let mut i = input.clone();
        f(&mut i.derivation.source_digests);
        i
    };
    assert_eq!(
        verify(&mutate(&|r| r[1].digest = [1; 32])).unwrap_err().gap,
        NeutralInputGap::SourceMismatch
    );
    assert_eq!(
        verify(&mutate(&|r| r[2].digest = [1; 32])).unwrap_err().gap,
        NeutralInputGap::SourceMismatch
    );
    assert_eq!(
        verify(&mutate(&|r| r[0].digest = [1; 32])).unwrap_err().gap,
        NeutralInputGap::SourceMismatch
    );
    assert_eq!(
        verify(&mutate(&|r| r[1].start += 1)).unwrap_err().gap,
        NeutralInputGap::SourceMismatch
    );
    assert_eq!(
        verify(&mutate(&|r| r[2].length += 100_000))
            .unwrap_err()
            .gap,
        NeutralInputGap::SourceMismatch
    );
    assert_eq!(
        verify(&mutate(&|r| r[0].length -= 1)).unwrap_err().gap,
        NeutralInputGap::SourceMismatch
    );
    assert_eq!(
        verify(&mutate(&|r| r.swap(1, 2))).unwrap_err().gap,
        NeutralInputGap::SourceMismatch
    );
    assert_eq!(
        verify(&mutate(&|r| {
            r.pop();
        }))
        .unwrap_err()
        .gap,
        NeutralInputGap::SourceMismatch
    );
    // The contrast region moved to another line range (here: the table's) overlaps/fails its anchor.
    assert_eq!(
        verify(&mutate(&|r| r[2] = r[1])).unwrap_err().gap,
        NeutralInputGap::SourceMismatch
    );
    // A contrast region that does not directly follow the E1 heading is refused.
    let mut moved = regions_of(&source());
    let text = String::from_utf8(source()).unwrap();
    let intro = text.find("The corpus is valuable").unwrap();
    moved[2] = DerivationSource {
        start: intro as u64,
        length: 40,
        digest: [0; 32],
        ..moved[2]
    };
    assert_eq!(
        RestatedMeasurements::from_regions(&source(), &moved)
            .unwrap_err()
            .gap,
        NeutralInputGap::SourceMismatch
    );
}

fn repinned(
    source: &[u8],
    mut regions: Vec<DerivationSource>,
    f: &dyn Fn(&mut Vec<DerivationSource>),
) -> Vec<DerivationSource> {
    f(&mut regions);
    for r in &mut regions {
        let end = (r.start + r.length) as usize;
        let start = (r.start as usize).min(source.len());
        r.digest = sha2::Sha256::digest(&source[start..end.min(source.len()).max(start)]).into();
    }
    regions
}

#[test]
fn consistently_repinned_but_malformed_regions_are_refused_structurally() {
    let real = source();
    let base = regions_of(&real);
    // The table region grown to swallow the contrast paragraph (digests re-pinned consistently).
    let grown = repinned(&real, base.clone(), &|r| {
        r[1].length = r[2].start + r[2].length - r[1].start
    });
    assert_eq!(
        RestatedMeasurements::from_regions(&real, &grown)
            .unwrap_err()
            .gap,
        NeutralInputGap::SourceMismatch
    );
    // A region that starts mid-line.
    let misaligned = repinned(&real, base.clone(), &|r| {
        r[1].start += 1;
        r[1].length -= 1;
    });
    assert_eq!(
        RestatedMeasurements::from_regions(&real, &misaligned)
            .unwrap_err()
            .gap,
        NeutralInputGap::SourceMismatch
    );
    // An empty region.
    let empty = repinned(&real, base, &|r| r[2].length = 0);
    assert_eq!(
        RestatedMeasurements::from_regions(&real, &empty)
            .unwrap_err()
            .gap,
        NeutralInputGap::SourceMismatch
    );
}

fn admit_with_regions(
    source: &[u8],
    mutate: &dyn Fn(&mut Vec<DerivationSource>),
) -> core::result::Result<NeutralQualificationInput, NeutralInputError> {
    let regions = repinned(source, regions_of(source), mutate);
    let mut input = NeutralQualificationInput::author_restated(source, &commission()).unwrap();
    input.derivation.source_digests = regions;
    let datum = input.to_datum().unwrap();
    let parsed = NeutralQualificationInput::from_datum(&datum)?;
    parsed.verify_restated_measurements(source, &commission())?;
    Ok(parsed)
}

#[test]
fn region_ends_must_be_line_aligned_and_char_aligned() {
    let real = source();
    // Ending two bytes early cuts the last row mid-line (the final closing pipe is lost).
    let cut = admit_with_regions(&real, &|r| r[1].length -= 3);
    assert_eq!(cut.unwrap_err().gap, NeutralInputGap::SourceMismatch);
    // A region ending inside a multi-byte scalar is a named gap, never a panic.
    let text = String::from_utf8(real.clone()).unwrap();
    let with_scalar = text.replacen(
        "| Reused receipts in that last epoch | 2 |\n",
        "| Reused receipts in that last epoch | 2 |\n\u{e9}\n",
        1,
    );
    let base = regions_of(with_scalar.as_bytes());
    let inside = repinned(with_scalar.as_bytes(), base, &|r| r[1].length += 1);
    let result = std::panic::catch_unwind(|| {
        RestatedMeasurements::from_regions(with_scalar.as_bytes(), &inside)
    });
    assert_eq!(
        result.expect("must not panic").unwrap_err().gap,
        NeutralInputGap::SourceMismatch
    );
}

#[test]
fn the_table_grammar_is_exhaustive() {
    let real = String::from_utf8(source()).unwrap();
    for (from, to) in [
        (
            "| Closeout epochs | 42: 37 failed, 4 invalidated, 1 interrupted/running raw state |",
            "| Closeout epochs | 42: 37 failed, 999 failed, 4 invalidated, 1 interrupted/running raw state |",
        ),
        (
            "| Closeout epochs | 42: 37 failed, 4 invalidated, 1 interrupted/running raw state |",
            "| Closeout epochs | 42: 37 failed, 4 invalidated, 1 interrupted/running raw state, 5 mystery |",
        ),
        (
            "| Reused receipts in that last epoch | 2 |",
            "| Reused receipts in that last epoch | 2 |\n| Unrepresented retained measure | 999 |",
        ),
        (
            "| Plan revisions | 230 |",
            "| Plan revisions | 230 |\n| Plan revisions | 230 |",
        ),
        ("| --- | ---: |", "| --- | ---: | extra |"),
        (
            "| Last raw epoch after revision | 26 green, 1 interrupted, 253 pending |",
            "| Last raw epoch after revision | 26 green, 1 interrupted, 253 pending, 1 green |",
        ),
        (
            "| Proof leaves in the terminal graph | 280: 60 generate, 183 standalone, 10 workspace, 27 package |",
            "| Proof leaves in the terminal graph | 280: 60 generate, 183 standalone, 10 workspace, 27 package, 1 other |",
        ),
        ("| Plan revisions | 230 |", "| Plan revisions | 230"),
    ] {
        let mutated = real.replacen(from, to, 1);
        assert_ne!(mutated, real, "{from:?}");
        let source = mutated.as_bytes();
        assert!(RestatedMeasurements::from_source(source).is_err(), "{to:?}");
        // Re-authoring and admission never succeed for any of them either.
        assert!(
            NeutralQualificationInput::author_restated(source, &commission()).is_err(),
            "{to:?}"
        );
    }
}

#[test]
fn a_region_may_end_right_after_a_line_ending_followed_by_text() {
    // The table directly followed by prose (no blank line): its region ends just
    // after the final row's line ending with an ordinary byte next.
    let text = String::from_utf8(source()).unwrap().replacen(
        "| 2 |\n\nThe corpus",
        "| 2 |\nThe corpus",
        1,
    );
    let regions = RestatedMeasurements::from_source(text.as_bytes())
        .unwrap()
        .regions;
    let end = (regions[1].start + regions[1].length) as usize;
    assert_eq!(text.as_bytes()[end - 1], b'\n');
    assert_ne!(text.as_bytes()[end], b'\n');
    assert!(RestatedMeasurements::from_regions(text.as_bytes(), &regions).is_ok());
}

#[test]
fn an_identical_duplicate_component_is_refused() {
    let real = String::from_utf8(source()).unwrap();
    let dup = real.replacen(
        "26 green, 1 interrupted",
        "26 green, 26 green, 1 interrupted",
        1,
    );
    assert_eq!(
        RestatedMeasurements::from_source(dup.as_bytes())
            .unwrap_err()
            .gap,
        NeutralInputGap::FactMismatch
    );
}

#[test]
fn crlf_regions_never_end_between_cr_and_lf() {
    let crlf = String::from_utf8(source()).unwrap().replace('\n', "\r\n");
    let regions = RestatedMeasurements::from_source(crlf.as_bytes())
        .unwrap()
        .regions;
    assert!(RestatedMeasurements::from_regions(crlf.as_bytes(), &regions).is_ok());
    // Cutting the contrast paragraph between its last CR and LF (digest re-pinned).
    let cut = repinned(crlf.as_bytes(), regions, &|r| r[2].length += 1);
    let end = (cut[2].start + cut[2].length) as usize;
    assert_eq!(&crlf.as_bytes()[end - 1..end + 1], b"\r\n");
    assert_eq!(
        RestatedMeasurements::from_regions(crlf.as_bytes(), &cut)
            .unwrap_err()
            .gap,
        NeutralInputGap::SourceMismatch
    );
}

#[test]
fn malformed_numerals_and_bare_components_are_refused() {
    let real = String::from_utf8(source()).unwrap();
    for (from, to) in [
        ("| Plan revisions | 230 |", "| Plan revisions | 2,,30 |"),
        ("| Plan revisions | 230 |", "| Plan revisions | 2,30 |"),
        ("| Plan revisions | 230 |", "| Plan revisions | ,230 |"),
        ("| Plan revisions | 230 |", "| Plan revisions | 0230 |"),
        (
            "| Lifecycle event rows | 458,386 |",
            "| Lifecycle event rows | 4,58,386 |",
        ),
        (
            "| Lifecycle event rows | 458,386 |",
            "| Lifecycle event rows | 458386, |",
        ),
        (
            "26 green, 1 interrupted, 253 pending",
            "26 green, 1 interrupted, 253 pending, 7",
        ),
        (
            "26 green, 1 interrupted, 253 pending",
            "26 green, 7, 1 interrupted, 253 pending",
        ),
        ("| Plan revisions | 230 |", "| Plan revisions | +230 |"),
        (
            "| Plan revisions | 230 |",
            "| Plan revisions | 18446744073709551616 |",
        ),
    ] {
        let mutated = real.replacen(from, to, 1);
        assert_ne!(mutated, real, "{to:?}");
        assert!(
            RestatedMeasurements::from_source(mutated.as_bytes()).is_err(),
            "{to:?}"
        );
    }
    // Grouped thousands remain valid where the roadmap writes them.
    assert!(RestatedMeasurements::from_source(real.as_bytes()).is_ok());
}

/// The numeral language is the canonical-spelling bijection: independently
/// formulated reference, exhaustive over a bounded alphabet (every string up to
/// seven characters), so no single-token spelling can be accepted by accident.
#[test]
fn numeral_language_is_exactly_the_canonical_spellings() {
    use super::super::grammar::{grouped, number};
    // Reference written as the regular expression `0|[1-9][0-9]{0,2}(,[0-9]{3})*`
    // (a lone zero; a leading zero never starts a longer numeral).
    fn reference(s: &str) -> bool {
        if s == "0" {
            return true;
        }
        let mut groups = s.split(',');
        let first = groups.next().unwrap_or("");
        let head = match first.len() {
            1..=3 => first.bytes().all(|b| b.is_ascii_digit()) && !first.starts_with('0'),
            _ => false,
        };
        head && groups.all(|g| g.len() == 3 && g.bytes().all(|b| b.is_ascii_digit()))
    }
    let alphabet = ['0', '1', '9', ',', '+', ' ', 'x'];
    let mut stack: Vec<String> = vec![String::new()];
    while let Some(s) = stack.pop() {
        assert_eq!(number(&s).is_some(), reference(&s), "{s:?}");
        if let Some(v) = number(&s) {
            assert_eq!(grouped(v), s);
        }
        if s.len() < 7 {
            for c in alphabet {
                stack.push(format!("{s}{c}"));
            }
        }
    }
    for v in (0..=5000u64).chain([999, 1_000, 999_999, 1_000_000, u64::MAX]) {
        assert_eq!(number(&grouped(v)), Some(v));
    }
    for bad in [
        "1000",
        "18446744073709551615",
        "18,446,744,073,709,551,616",
        "+0,230",
        "458386",
    ] {
        assert_eq!(number(bad), None, "{bad:?}");
    }
}

fn mutated_source(from: &str, to: &str) -> String {
    let real = String::from_utf8(source()).unwrap();
    let out = real.replacen(from, to, 1);
    assert_ne!(out, real, "{to:?}");
    out
}

#[test]
fn same_value_ungrouped_spellings_of_every_real_numeral_are_refused() {
    for (from, to) in [
        (
            "| Lifecycle event rows | 458,386 |",
            "| Lifecycle event rows | 458386 |",
        ),
        ("103,261, of which", "103261, of which"),
        ("of which 103,213 finished", "of which 103213 finished"),
        (
            "| `unit.ready.carried` events | 188,393 |",
            "| `unit.ready.carried` events | 188393 |",
        ),
        (
            "| `focused.check.receipt.carried` events | 134,616 |",
            "| `focused.check.receipt.carried` events | 134616 |",
        ),
        ("| Plan revisions | 230 |", "| Plan revisions | +0,230 |"),
    ] {
        let mutated = mutated_source(from, to);
        assert!(
            RestatedMeasurements::from_source(mutated.as_bytes()).is_err(),
            "{to:?}"
        );
    }
}

#[test]
fn bare_numeric_components_are_a_named_refusal_not_a_fallback() {
    // `37` replaces the registered `failed` suffix: a fallback slot would absorb it.
    let mutated = mutated_source(
        "42: 37 failed, 4 invalidated, 1 interrupted/running raw state",
        "42: 37, 4 invalidated, 1 interrupted/running raw state",
    );
    let error = RestatedMeasurements::from_source(mutated.as_bytes()).unwrap_err();
    assert_eq!(error.gap, NeutralInputGap::FactMismatch);
    assert!(
        error.to_string().contains("row malformed: Closeout epochs"),
        "{error}"
    );
}

#[test]
fn a_missing_component_is_a_named_gap_never_an_unwind() {
    let mutated = mutated_source(
        "42: 37 failed, 4 invalidated, 1 interrupted/running raw state",
        "42: 37 failed, 4 invalidated",
    );
    let result = std::panic::catch_unwind(|| RestatedMeasurements::from_source(mutated.as_bytes()));
    let error = result.expect("no unwind").unwrap_err();
    assert!(
        error.to_string().contains("row malformed: Closeout epochs"),
        "{error}"
    );
}

/// Regions for `mutated` where the table region grew by `delta` bytes, so the
/// contrast region (which follows it) moved by the same amount; digests re-pinned
/// consistently so only the structural guards can refuse.
pub(super) fn shifted_regions(mutated: &[u8], delta: i64) -> Vec<DerivationSource> {
    let base = regions_of(&source());
    repinned(mutated, base, &|r| {
        r[1].length = (r[1].length as i64 + delta) as u64;
        r[2].start = (r[2].start as i64 + delta) as u64;
        r[0].length = mutated.len() as u64;
    })
}

#[test]
fn table_header_and_separator_shape_are_enforced_on_the_pinned_path() {
    for (from, to) in [
        (
            "| Retained SUP20 measure | Observed value |",
            "| Retained SUP20 measurX | Observed value |",
        ),
        ("| --- | ---: |", "| --- | ---: | --- |"),
        ("| --- | ---: |", "| -- | --: |"),
        ("| --- | ---: |", "| abc | def: |"),
    ] {
        let mutated = mutated_source(from, to);
        let delta = to.len() as i64 - from.len() as i64;
        let regions = shifted_regions(mutated.as_bytes(), delta);
        assert!(
            RestatedMeasurements::from_regions(mutated.as_bytes(), &regions).is_err(),
            "pinned path accepted {to:?}"
        );
        assert!(
            RestatedMeasurements::from_source(mutated.as_bytes()).is_err(),
            "{to:?}"
        );
    }
}

#[test]
fn a_contrast_paragraph_ending_at_end_of_source_is_accepted_without_unwinding() {
    let real = String::from_utf8(source()).unwrap();
    let unterminated = real.trim_end_matches('\n');
    assert_ne!(unterminated.len(), real.len());
    let result =
        std::panic::catch_unwind(|| RestatedMeasurements::from_source(unterminated.as_bytes()));
    let measured = result
        .expect("no unwind")
        .expect("end-of-source paragraph verifies");
    assert!(RestatedMeasurements::from_regions(unterminated.as_bytes(), &measured.regions).is_ok());
}
