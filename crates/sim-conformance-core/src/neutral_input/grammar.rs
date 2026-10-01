// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Authoring-side location and exhaustive grammar of the retained-measurement
//! table and the contrast paragraph (verification slices pinned regions and
//! reuses only the pure parsers here).

use super::markdown;
use super::types::*;

pub(super) const TABLE_HEADER: &str = "| Retained SUP20 measure | Observed value |";
/// The roadmap section that states the two contrast sentences.
pub(super) const CONTRAST_HEADING: &str =
    "E1 - semantic identity must be declared, and must be narrow";

/// The canonical spelling of `value`: decimal digits grouped in threes from the
/// right by commas, no sign, no leading zero (`458,386`, `230`, `0`).
pub(super) fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Reads a source numeral. The accepted language is exactly the set of
/// canonical spellings: a numeral is admitted only when it equals
/// [`grouped`] of its own value, so no other spelling (ungrouped long runs,
/// leading zeros or signs, misplaced or doubled commas, overflow) can be
/// accepted regardless of which defect it exercises (the parse may be lenient: a
/// sign or stray character cannot survive the equality).
pub(super) fn number(text: &str) -> Option<u64> {
    let value: u64 = text.replace(',', "").parse().ok()?;
    (grouped(value) == text).then_some(value)
}

/// The registered data-row labels of the retained-measurement table, each of
/// which must appear exactly once.
pub(super) const TABLE_LABELS: [&str; 12] = [
    "Materialized schedule truth",
    "Plan revisions",
    "Closeout epochs",
    "Proof leaves in the terminal graph",
    "Lifecycle event rows",
    "Telemetry spans",
    "Sum of overlapping span durations",
    "`unit.ready.carried` events",
    "`focused.check.receipt.carried` events",
    "Previous fixed point before retirement",
    "Last raw epoch after revision",
    "Reused receipts in that last epoch",
];

/// The table grammar is exhaustive: the exact header, a separator row, and
/// exactly the registered data rows once each, nothing else. A genuinely new
/// measure fails with a named gap until the representation is extended.
pub(super) fn check_table_grammar(table: &str) -> Result<()> {
    let bad = |what: &str| NeutralInputError::new(NeutralInputGap::FactMismatch, what.to_owned());
    let mut lines = table.lines();
    if lines.next().map(str::trim_end) != Some(TABLE_HEADER) {
        return Err(bad("table does not start with the exact header"));
    }
    let separator = lines
        .next()
        .ok_or_else(|| bad("table has no separator row"))?;
    let cells: Vec<&str> = separator
        .trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect();
    let dashes = |c: &str| {
        let core = c.trim_start_matches(':').trim_end_matches(':');
        core.len() >= 3 && core.chars().all(|ch| ch == '-')
    };
    if cells.len() != 2 || !cells.iter().all(|c| dashes(c)) {
        return Err(bad("table separator row is malformed"));
    }
    let mut seen: Vec<&str> = Vec::new();
    for line in lines {
        let trimmed = line.trim_end();
        if !(trimmed.starts_with('|') && trimmed.ends_with('|')) {
            return Err(bad("table row is not a closed pipe row"));
        }
        let cells: Vec<&str> = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() != 2 || !TABLE_LABELS.contains(&cells[0]) || seen.contains(&cells[0]) {
            return Err(bad("table has an unregistered, malformed or repeated row"));
        }
        seen.push(cells[0]);
    }
    if seen.len() != TABLE_LABELS.len() {
        return Err(bad("table is missing a registered row"));
    }
    Ok(())
}

/// Parses a `", "`-separated list of `N<suffix>` components: every registered
/// suffix exactly once, nothing unknown or repeated. Values come back in the
/// order of `suffixes`.
pub(super) fn components(text: &str, suffixes: &[&str]) -> Result<Vec<u64>> {
    let bad = |what: &str| NeutralInputError::new(NeutralInputGap::FactMismatch, what.to_owned());
    let mut values: Vec<Option<u64>> = vec![None; suffixes.len()];
    for part in text.split(", ") {
        let (index, number_text) = suffixes
            .iter()
            .enumerate()
            .find_map(|(i, suffix)| part.strip_suffix(suffix).map(|n| (i, n)))
            .ok_or_else(|| bad("row has an unregistered component"))?;
        if values[index].is_some() {
            return Err(bad("row repeats a component"));
        }
        values[index] =
            Some(number(number_text.trim()).ok_or_else(|| bad("row component malformed"))?);
    }
    values
        .into_iter()
        .collect::<Option<Vec<u64>>>()
        .ok_or_else(|| bad("row is missing a component"))
}

pub(super) fn utf8(source: &[u8]) -> Result<&str> {
    core::str::from_utf8(source)
        .map_err(|_| NeutralInputError::new(NeutralInputGap::SourceMismatch, "source is not UTF-8"))
}

/// AUTHORING AID, not a trust boundary: finds the table block and the E1
/// section's first plain-prose paragraph with the narrow block scanner, so an
/// author can designate the two regions a receipt then pins by offset, length
/// and digest. Verification (`from_regions`) never calls this.
pub(super) fn locate(text: &str) -> Result<(core::ops::Range<usize>, core::ops::Range<usize>)> {
    let bad = |what: &str| NeutralInputError::new(NeutralInputGap::FactMismatch, what.to_owned());
    let lines = markdown::scan(text);
    let headers: Vec<&markdown::Line<'_>> = lines
        .iter()
        .filter(|l| l.prose && l.text.trim_end() == TABLE_HEADER)
        .collect();
    let [header] = headers.as_slice() else {
        return Err(NeutralInputError::new(
            NeutralInputGap::SourceMismatch,
            "retained-measurement table not found exactly once",
        ));
    };
    let rest = &text[header.start..];
    let mut end = rest.len();
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if !line.starts_with('|') {
            end = offset;
            break;
        }
        offset += line.len();
    }
    let table = header.start..header.start + end;
    let mut headings = lines.iter().enumerate().filter(|(_, l)| {
        l.prose
            && markdown::atx_heading(l.text)
                .is_some_and(|(level, content)| level == 3 && content == CONTRAST_HEADING)
    });
    let (Some((at, _)), None) = (headings.next(), headings.next()) else {
        return Err(bad("contrast section heading absent or repeated"));
    };
    let mut rest = lines[at + 1..]
        .iter()
        .skip_while(|l| l.text.trim().is_empty());
    let first = rest
        .next()
        .ok_or_else(|| bad("contrast section is empty"))?;
    if !first.prose || markdown::starts_other_block(first.text) {
        return Err(bad("contrast paragraph is not plain prose"));
    }
    let mut last = first;
    for l in rest {
        if l.text.trim().is_empty() {
            break;
        }
        if !l.prose || markdown::starts_other_block(l.text) {
            return Err(bad("contrast paragraph is interrupted by another block"));
        }
        last = l;
    }
    Ok((table, first.start..last.start + last.text.len()))
}

/// Parses the two contrast statements from the designated paragraph text.
/// Returns the control-only repair numerator and its stated denominator, and
/// the public-head retention.
pub(super) fn parse_contrast(paragraph: &str) -> Result<(u64, u64, u64)> {
    let bad = |what: &str| NeutralInputError::new(NeutralInputGap::FactMismatch, what.to_owned());
    if paragraph.contains('|') {
        return Err(bad("contrast paragraph contains table syntax"));
    }
    let flat = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
    let sentences: Vec<String> = flat
        .split(". ")
        .map(|s| {
            if s.ends_with('.') {
                s.to_owned()
            } else {
                format!("{s}.")
            }
        })
        .collect();
    let control_lead = "A control-only repair retained ";
    let control_tail = " proof receipts.";
    let heads_sentence = "A change to three public heads and their pins retained only one.";
    let [control_sentence, heads, ..] = sentences.as_slice() else {
        return Err(bad("contrast paragraph lacks the two sentences"));
    };
    if heads != heads_sentence {
        return Err(bad("public-head sentence is not the second sentence"));
    }
    let statement = control_sentence
        .strip_prefix(control_lead)
        .and_then(|rest| rest.strip_suffix(control_tail))
        .ok_or_else(|| bad("control-only sentence is not the first sentence"))?;
    let (retained, denominator) = statement
        .split_once(" of ")
        .ok_or_else(|| bad("control-only sentence malformed"))?;
    let control = number(retained).ok_or_else(|| bad("control-only numerator malformed"))?;
    let control_denominator =
        number(denominator).ok_or_else(|| bad("control-only denominator malformed"))?;
    Ok((control, control_denominator, 1))
}
