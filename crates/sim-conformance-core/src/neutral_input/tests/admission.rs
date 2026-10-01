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

fn with_section(section: &str) -> String {
    String::from_utf8(source()).unwrap().replace(
        "A control-only repair retained 213 of 280 proof receipts. A change to three\npublic heads and their pins retained only one. Cache identity was too broad and\nproof invalidation had no single accountable projection. Every conclusion must\nname the exact semantic facts on which it depends and explain every reuse and\ninvalidation.\n",
        section,
    )
}

#[test]
fn markdown_contexts_are_not_assertions() {
    let control = "A control-only repair retained 213 of 280 proof receipts.";
    let heads = "A change to three public heads and their pins retained only one.";
    for section in [
        format!("False claims:\n1. {control}\n2. {heads}\n"),
        format!("```text\nExample.\n{control}\n{heads}\n```\n"),
        format!("> False claims.\n{control}\n{heads}\n"),
        format!("- {control} {heads}\n"),
        format!("> {control} {heads}\n"),
        format!("    {control} {heads}\n"),
        format!("| {control} | {heads} |\n"),
        format!("# {control} {heads}\n"),
        format!("It is false that {control} {heads}\n"),
        format!("{heads} {control}\n"),
    ] {
        let source = with_section(&section);
        assert_eq!(
            RestatedMeasurements::from_source(source.as_bytes())
                .unwrap_err()
                .gap,
            NeutralInputGap::FactMismatch,
            "{section:?}"
        );
    }
    // The sentences placed outside the E1 section are not read either.
    let moved = String::from_utf8(source()).unwrap().replace(
        "### E1 - semantic identity must be declared, and must be narrow",
        "### E2 - another section",
    );
    assert_eq!(
        RestatedMeasurements::from_source(moved.as_bytes())
            .unwrap_err()
            .gap,
        NeutralInputGap::FactMismatch
    );
    // A duplicated heading is refused.
    let twice = format!(
        "{}\n### E1 - semantic identity must be declared, and must be narrow\n\nx\n",
        String::from_utf8(source()).unwrap()
    );
    assert_eq!(
        RestatedMeasurements::from_source(twice.as_bytes())
            .unwrap_err()
            .gap,
        NeutralInputGap::FactMismatch
    );
}

#[test]
fn the_real_section_structure_wraps_lines_and_may_continue_after_the_two_sentences() {
    let derived = RestatedMeasurements::from_source(&source()).unwrap();
    let control = derived
        .facts
        .contrast_reuse
        .iter()
        .find(|r| r.contrast == ReuseContrast::ControlOnlyRepair)
        .unwrap();
    assert_eq!((control.retained, control.denominator), (213, 280));
    // CRLF line endings parse identically.
    let crlf = String::from_utf8(source()).unwrap().replace('\n', "\r\n");
    assert_eq!(
        RestatedMeasurements::from_source(crlf.as_bytes())
            .unwrap()
            .facts,
        derived.facts
    );
}

const HEADING: &str = "### E1 - semantic identity must be declared, and must be narrow";
const REAL: &str = "A control-only repair retained 213 of 280 proof receipts. A change to three\npublic heads and their pins retained only one. Cache identity was too broad and\nproof invalidation had no single accountable projection. Every conclusion must\nname the exact semantic facts on which it depends and explain every reuse and\ninvalidation.\n";
const CLAIMS: &str = "A control-only repair retained 213 of 280 proof receipts. A change to three public heads and their pins retained only one.";

fn without_real_section() -> String {
    String::from_utf8(source())
        .unwrap()
        .replace(&format!("{HEADING}\n\n{REAL}"), "")
}

#[test]
fn contextual_e1_look_alikes_are_never_the_section() {
    let base = without_real_section();
    assert_ne!(base, String::from_utf8(source()).unwrap());
    for wrapper in [
        format!("```text\n{HEADING}\n\n{CLAIMS}\n```\n"),
        format!("~~~\n{HEADING}\n\n{CLAIMS}\n~~~\n"),
        format!("<!--\n{HEADING}\n\n{CLAIMS}\n-->\n"),
        format!("<script type=\"text/plain\">\n{HEADING}\n\n{CLAIMS}\n</script>\n"),
        format!("    {HEADING}\n\n    {CLAIMS}\n"),
    ] {
        let source = format!("{base}\n{wrapper}");
        assert_eq!(
            RestatedMeasurements::from_source(source.as_bytes())
                .unwrap_err()
                .gap,
            NeutralInputGap::FactMismatch,
            "{wrapper:?}"
        );
    }
    let front = format!("---\n{HEADING}\n\n{CLAIMS}\n---\n{base}");
    assert_eq!(
        RestatedMeasurements::from_source(front.as_bytes())
            .unwrap_err()
            .gap,
        NeutralInputGap::FactMismatch
    );
}

#[test]
fn setext_and_table_interruptions_are_refused() {
    for tail in ["---\n", "===\n", " |\n:--- |\n"] {
        let section = format!(
            "{CLAIMS}{}\n{tail}",
            if tail.starts_with(' ') { " |" } else { "" }
        )
        .replace(" |\n |\n", " |\n");
        let source = with_section(&section);
        assert_eq!(
            RestatedMeasurements::from_source(source.as_bytes())
                .unwrap_err()
                .gap,
            NeutralInputGap::FactMismatch,
            "{tail:?}"
        );
    }
}

#[test]
fn unrelated_contexts_do_not_disturb_the_real_section() {
    let real = String::from_utf8(source()).unwrap();
    let expected = RestatedMeasurements::from_source(real.as_bytes())
        .unwrap()
        .facts;
    for extra in [
        format!("\n```text\n{HEADING}\n```\n"),
        format!("\n<!--\n{HEADING}\n-->\n"),
        format!("\n~~~\n{HEADING}\n~~~\n"),
        format!("\n    {HEADING}\n"),
    ] {
        let source = format!("{real}{extra}");
        assert_eq!(
            RestatedMeasurements::from_source(source.as_bytes())
                .unwrap()
                .facts,
            expected,
            "{extra:?}"
        );
    }
    let front = format!("---\ntitle: x\n{HEADING}\n---\n{real}");
    assert_eq!(
        RestatedMeasurements::from_source(front.as_bytes())
            .unwrap()
            .facts,
        expected
    );
    // Valid Markdown variants of the real section still parse: indentation up to
    // three spaces and a decorated ATX heading.
    for (from, to) in [
        (HEADING.to_owned(), format!("   {HEADING} ###")),
        ("A control-only".to_owned(), "   A control-only".to_owned()),
    ] {
        let variant = real.replacen(&from, &to, 1);
        assert_ne!(variant, real);
        assert_eq!(
            RestatedMeasurements::from_source(variant.as_bytes())
                .unwrap()
                .facts,
            expected,
            "{to:?}"
        );
    }
}

#[test]
fn a_table_header_inside_a_fence_is_not_the_table() {
    let real = String::from_utf8(source()).unwrap();
    let fenced = real.replace(
        "| Retained SUP20 measure | Observed value |",
        "```\n| Retained SUP20 measure | Observed value |",
    );
    assert_eq!(
        RestatedMeasurements::from_source(fenced.as_bytes())
            .unwrap_err()
            .gap,
        NeutralInputGap::SourceMismatch
    );
}

#[test]
fn raw_text_html_blocks_span_blank_lines_until_their_closing_tag() {
    let base = without_real_section();
    for tag in ["script", "pre", "style", "textarea", "SCRIPT"] {
        let section = format!("<{tag}>\n\n{HEADING}\n\n{CLAIMS}\n\n</{tag}>\n");
        let source = format!("{base}\n{section}");
        assert_eq!(
            RestatedMeasurements::from_source(source.as_bytes())
                .unwrap_err()
                .gap,
            NeutralInputGap::FactMismatch,
            "{tag}"
        );
    }
    // The whole retained table inside a raw-text block is not the table.
    let real = String::from_utf8(source()).unwrap();
    let start = real.find("| Retained SUP20 measure").unwrap();
    let end = real.find("\n\nThe corpus is valuable").unwrap();
    let table = &real[start..end];
    let wrapped = format!(
        "{}<script>\n\n{table}\n\n</script>{}",
        &real[..start],
        &real[end..]
    );
    assert_eq!(
        RestatedMeasurements::from_source(wrapped.as_bytes())
            .unwrap_err()
            .gap,
        NeutralInputGap::SourceMismatch
    );
}

#[test]
fn a_nested_list_example_is_not_a_top_level_section() {
    let base = without_real_section();
    let nested = format!("- Non-asserting example:\n\n  {HEADING}\n\n  {CLAIMS}\n");
    let source = format!("{base}\n{nested}");
    assert_eq!(
        RestatedMeasurements::from_source(source.as_bytes())
            .unwrap_err()
            .gap,
        NeutralInputGap::FactMismatch
    );
    // A nested look-alike beside the real section neither satisfies nor disturbs it.
    let real = String::from_utf8(source_bytes_for_nested()).unwrap();
    let expected = RestatedMeasurements::from_source(&source_bytes_for_nested())
        .unwrap()
        .facts;
    let beside = format!("{real}\n{nested}");
    assert_eq!(
        RestatedMeasurements::from_source(beside.as_bytes())
            .unwrap()
            .facts,
        expected
    );
}

fn source_bytes_for_nested() -> Vec<u8> {
    source()
}

#[test]
fn setext_underlines_with_trailing_whitespace_are_headings_not_prose() {
    for tail in ["---   ", "===   ", "---\t"] {
        let source = with_section(&format!("{CLAIMS}\n{tail}\n"));
        assert_eq!(
            RestatedMeasurements::from_source(source.as_bytes())
                .unwrap_err()
                .gap,
            NeutralInputGap::FactMismatch,
            "{tail:?}"
        );
    }
}

#[test]
fn ordinary_wrapped_prose_is_still_prose() {
    let real = String::from_utf8(source()).unwrap();
    let expected = RestatedMeasurements::from_source(real.as_bytes())
        .unwrap()
        .facts;
    for section in [
        "A control-only repair retained\n213 of 280 proof receipts. A change to three public heads and their pins retained only one.\n".to_owned(),
        "A control-only repair retained 213 of 280 proof receipts. A change to three\n    public heads and their pins retained only one.\n".to_owned(),
        "A control-only repair retained 213 of 280 proof receipts. A change to three public heads and their pins retained only one. 2024 was a long year.\n".to_owned(),
    ] {
        let source = with_section(&section);
        assert_eq!(RestatedMeasurements::from_source(source.as_bytes()).unwrap().facts, expected, "{section:?}");
    }
    // An indented code block as the FIRST block after the heading is still refused.
    let source = with_section(&format!("    {CLAIMS}\n"));
    assert_eq!(
        RestatedMeasurements::from_source(source.as_bytes())
            .unwrap_err()
            .gap,
        NeutralInputGap::FactMismatch
    );
}

#[test]
fn list_paragraph_lines_and_ten_digit_numbers_do_not_confuse_the_scanner() {
    let real = String::from_utf8(source()).unwrap();
    let expected = RestatedMeasurements::from_source(real.as_bytes())
        .unwrap()
        .facts;
    // A heading may interrupt a list paragraph: the real section directly after a list line.
    let interrupted = real.replacen(
        &format!("\n\n{HEADING}"),
        &format!("\n- a list item\n{HEADING}"),
        1,
    );
    assert_ne!(interrupted, real);
    assert_eq!(
        RestatedMeasurements::from_source(interrupted.as_bytes())
            .unwrap()
            .facts,
        expected
    );
    // A ten-digit number is not an ordered-list marker.
    let numbered = with_section(&format!("{CLAIMS}\n1234567890. a continuation line\n"));
    assert_eq!(
        RestatedMeasurements::from_source(numbered.as_bytes())
            .unwrap()
            .facts,
        expected
    );
}
