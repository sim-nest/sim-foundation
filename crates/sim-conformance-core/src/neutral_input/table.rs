// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use sim_kernel::{ContentId, Datum, Symbol};

use super::MS_PER_HOUR;
use super::decode::*;
use super::markdown;
use super::types::*;

/// Exact-byte identity of a source: tagged `source-deck/bytes-v1` datum over the
/// bytes, the form `sim-source-deck`'s byte identity uses.
///
/// # Errors
/// Returns an identity gap if the datum cannot be canonicalized.
pub fn source_byte_identity(bytes: &[u8]) -> Result<ContentId> {
    Datum::Node {
        tag: Symbol::qualified("source-deck", "bytes-v1"),
        fields: vec![(Symbol::new("bytes"), Datum::Bytes(bytes.to_vec()))],
    }
    .content_id()
    .map_err(|e| NeutralInputError::new(NeutralInputGap::Identity, e.to_string()))
}

const TABLE_HEADER: &str = "| Retained SUP20 measure | Observed value |";

fn number(text: &str) -> Option<u64> {
    let digits: String = text.chars().filter(|c| *c != ',').collect();
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| digits.parse().ok())?
}

/// The facts and table region derived from the selected source, by this owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestatedMeasurements {
    /// Facts the table and the two contrast sentences state.
    pub facts: NeutralFacts,
    /// The exact bytes of the table region (header through last row).
    pub table_region: Vec<u8>,
    /// Precision of the duration conversion, derived from the source spelling.
    pub precision: MeasurementPrecision,
    /// Identity of the conversion procedure record, derived from the source.
    pub procedure: ContentId,
}

impl RestatedMeasurements {
    /// Derives the facts from the selected roadmap source bytes.
    ///
    /// # Errors
    /// Returns a fact-mismatch gap if the table or a contrast sentence is
    /// absent, repeated or malformed.
    pub fn from_source(source: &[u8]) -> Result<Self> {
        let text = core::str::from_utf8(source).map_err(|_| {
            NeutralInputError::new(NeutralInputGap::SourceMismatch, "source is not UTF-8")
        })?;
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
        let start = header.start;
        let rest = &text[start..];
        let mut end = rest.len();
        let mut offset = 0;
        for line in rest.split_inclusive('\n') {
            if !line.starts_with('|') {
                end = offset;
                break;
            }
            offset += line.len();
        }
        let table = &rest[..end];
        let row = |label: &str| -> Result<&str> {
            let mut found = None;
            for line in table.lines() {
                let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
                if cells.len() == 2 && cells[0] == label {
                    if found.is_some() {
                        return Err(NeutralInputError::new(
                            NeutralInputGap::FactMismatch,
                            format!("row repeated: {label}"),
                        ));
                    }
                    found = Some(cells[1]);
                }
            }
            found.ok_or_else(|| {
                NeutralInputError::new(
                    NeutralInputGap::FactMismatch,
                    format!("row missing: {label}"),
                )
            })
        };
        let bad = |what: &str| {
            NeutralInputError::new(
                NeutralInputGap::FactMismatch,
                format!("row malformed: {what}"),
            )
        };
        let (mat, sch) = row("Materialized schedule truth")?
            .split_once(" of ")
            .ok_or_else(|| bad("schedule"))?;
        let revisions = number(row("Plan revisions")?).ok_or_else(|| bad("revisions"))?;
        let epochs_row = row("Closeout epochs")?;
        let (total, parts) = epochs_row.split_once(": ").ok_or_else(|| bad("epochs"))?;
        let pick = |haystack: &str, word: &str| -> Option<u64> {
            haystack
                .split(", ")
                .find_map(|p| p.strip_suffix(word).and_then(|n| number(n.trim())))
        };
        let proof_row = row("Proof leaves in the terminal graph")?;
        let (ptotal, pparts) = proof_row.split_once(": ").ok_or_else(|| bad("proof"))?;
        let spans_row = row("Telemetry spans")?;
        let (stotal, sfin) = spans_row
            .split_once(", of which ")
            .ok_or_else(|| bad("spans"))?;
        let hours = row("Sum of overlapping span durations")?
            .strip_suffix(" hours")
            .ok_or_else(|| bad("hours"))?;
        let (whole, frac) = hours.split_once('.').ok_or_else(|| bad("hours"))?;
        if frac.len() != 2 {
            return Err(bad("hours precision"));
        }
        let decimal_places = frac.len() as u64;
        let hundredths = number(whole)
            .and_then(|w| w.checked_mul(100))
            .and_then(|w| w.checked_add(number(frac)?))
            .ok_or_else(|| bad("hours"))?;
        let ms_numer = hundredths
            .checked_mul(MS_PER_HOUR)
            .ok_or_else(|| bad("hours overflow"))?;
        if ms_numer % 100 != 0 {
            return Err(NeutralInputError::new(
                NeutralInputGap::Arithmetic,
                "hours do not convert to whole milliseconds",
            ));
        }
        let frontier_row = row("Last raw epoch after revision")?;
        let green = pick(frontier_row, " green").ok_or_else(|| bad("frontier"))?;
        let interrupted = pick(frontier_row, " interrupted").ok_or_else(|| bad("frontier"))?;
        let pending = pick(frontier_row, " pending").ok_or_else(|| bad("frontier"))?;
        let previous = number(
            row("Previous fixed point before retirement")?
                .strip_suffix(" green leaves")
                .ok_or_else(|| bad("fixed point"))?,
        )
        .ok_or_else(|| bad("fixed point"))?;
        let reused =
            number(row("Reused receipts in that last epoch")?).ok_or_else(|| bad("reused"))?;
        let proof_total = number(ptotal).ok_or_else(|| bad("proof total"))?;
        let (control, control_denominator, heads) = contrast_sentences(text)?;
        if control_denominator != proof_total {
            return Err(NeutralInputError::new(
                NeutralInputGap::FactMismatch,
                "control-only repair denominator is not the proof-leaf total",
            ));
        }
        let last_frontier_total = green
            .checked_add(interrupted)
            .and_then(|v| v.checked_add(pending))
            .ok_or_else(|| {
                NeutralInputError::new(NeutralInputGap::Arithmetic, "frontier total overflows")
            })?;
        let facts = NeutralFacts {
            scheduled: number(sch).ok_or_else(|| bad("scheduled"))?,
            materialized: number(mat).ok_or_else(|| bad("materialized"))?,
            revisions,
            epochs: EpochCounts {
                failed: pick(parts, " failed").ok_or_else(|| bad("epochs"))?,
                invalidated: pick(parts, " invalidated").ok_or_else(|| bad("epochs"))?,
                interrupted_running: parts
                    .split(", ")
                    .find_map(|p| {
                        p.strip_suffix(" interrupted/running raw state")
                            .and_then(|n| number(n.trim()))
                    })
                    .ok_or_else(|| bad("epochs"))?,
                total: number(total).ok_or_else(|| bad("epochs total"))?,
            },
            proof_classes: ProofClassCounts {
                generate: pick(pparts, " generate").ok_or_else(|| bad("proof"))?,
                standalone: pick(pparts, " standalone").ok_or_else(|| bad("proof"))?,
                workspace: pick(pparts, " workspace").ok_or_else(|| bad("proof"))?,
                package: pick(pparts, " package").ok_or_else(|| bad("proof"))?,
                total: proof_total,
            },
            lifecycle_rows: number(row("Lifecycle event rows")?).ok_or_else(|| bad("lifecycle"))?,
            spans: SpanCounts {
                total: number(stotal).ok_or_else(|| bad("spans"))?,
                finished: number(sfin.strip_suffix(" finished").ok_or_else(|| bad("spans"))?)
                    .ok_or_else(|| bad("spans"))?,
            },
            overlapping_span_milliseconds: ms_numer / 100,
            repeated_ready_rows: number(row("`unit.ready.carried` events")?)
                .ok_or_else(|| bad("ready"))?,
            repeated_receipt_rows: number(row("`focused.check.receipt.carried` events")?)
                .ok_or_else(|| bad("receipt"))?,
            last_frontier: FrontierCounts {
                green,
                interrupted,
                pending,
                total: last_frontier_total,
            },
            contrast_reuse: vec![
                ReuseCount {
                    contrast: ReuseContrast::ControlOnlyRepair,
                    retained: control,
                    denominator: proof_total,
                },
                ReuseCount {
                    contrast: ReuseContrast::PublicHeadAndPinChange,
                    retained: heads,
                    denominator: proof_total,
                },
                ReuseCount {
                    contrast: ReuseContrast::LastRawEpoch,
                    retained: reused,
                    denominator: last_frontier_total,
                },
                ReuseCount {
                    contrast: ReuseContrast::PreviousFixedPoint,
                    retained: previous,
                    denominator: previous,
                },
            ],
        };
        check_facts(&facts)?;
        let procedure = restated_procedure_id(hundredths, facts.overlapping_span_milliseconds)?;
        Ok(Self {
            facts,
            table_region: table.as_bytes().to_vec(),
            precision: MeasurementPrecision {
                source_decimal_places: decimal_places,
            },
            procedure,
        })
    }
}

/// Identity of the restated-measurements conversion recipe for the given
/// hundredths of hours and result milliseconds. The owner derives it; an
/// author never supplies it.
fn restated_procedure_id(hundredths: u64, milliseconds: u64) -> Result<ContentId> {
    let n = |name: &'static str, value: u64| (Symbol::new(name), number_datum(value));
    let sym = |value: &'static str| Datum::Symbol(Symbol::qualified("qualification", value));
    Datum::Node {
        tag: Symbol::qualified("qualification", "restated-measurements-procedure-v1"),
        fields: vec![
            (Symbol::new("basis"), sym("restated-measurements")),
            (
                Symbol::new("calculation"),
                sym("exact-rational-hours-to-milliseconds"),
            ),
            n("milliseconds-per-hour", MS_PER_HOUR),
            n("result-milliseconds", milliseconds),
            n("source-hour-scale", 100),
            n("source-hours-hundredths", hundredths),
            (Symbol::new("source-precision"), sym("rounded-to-0.01-hour")),
        ],
    }
    .content_id()
    .map_err(|e| NeutralInputError::new(NeutralInputGap::Identity, e.to_string()))
}

fn number_datum(value: u64) -> Datum {
    Datum::Number(sim_kernel::NumberLiteral {
        domain: Symbol::qualified("qualification", "u64"),
        canonical: value.to_string(),
    })
}

/// The roadmap section that states the two contrast sentences.
const CONTRAST_HEADING: &str = "E1 - semantic identity must be declared, and must be narrow";

/// Parses the two contrast statements from the E1 section's first ordinary
/// prose paragraph. Block structure is preserved: the heading must appear
/// exactly once, the paragraph must follow it directly (after blank lines) and
/// must be plain prose, not a heading, list item, quotation, table row, code
/// fence or indented block, so examples and false claims in other blocks are
/// never read. Returns the control-only repair numerator and its stated
/// denominator, and the public-head retention.
fn contrast_sentences(text: &str) -> Result<(u64, u64, u64)> {
    let bad = |what: &str| NeutralInputError::new(NeutralInputGap::FactMismatch, what.to_owned());
    let lines = markdown::scan(text);
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
    let mut paragraph = vec![first.text];
    for l in rest {
        if l.text.trim().is_empty() {
            break;
        }
        if !l.prose || markdown::starts_other_block(l.text) {
            return Err(bad("contrast paragraph is interrupted by another block"));
        }
        paragraph.push(l.text);
    }
    if paragraph.iter().any(|l| l.contains('|')) {
        return Err(bad("contrast paragraph contains table syntax"));
    }
    let flat = paragraph
        .iter()
        .flat_map(|l| l.split_whitespace())
        .collect::<Vec<_>>()
        .join(" ");
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

impl NeutralQualificationInput {
    /// Verifies an input against the exact selected source bytes: the source
    /// identity, the derivation's source and table-region digests, and that every
    /// fact equals the one this owner derives from the table. The ids and bounds
    /// were already checked by [`Self::from_datum`].
    ///
    /// # Errors
    /// Returns the first named gap.
    pub fn verify_restated_measurements(
        &self,
        source: &[u8],
        commission: &TrustedCommission,
    ) -> Result<()> {
        use sha2::{Digest, Sha256};
        if self.derivation.basis != DerivationBasis::RestatedMeasurements {
            return Err(NeutralInputError::new(
                NeutralInputGap::FactMismatch,
                "basis is not restated measurements",
            ));
        }
        authoring_symbol(&commission.producer, "trusted producer")?;
        authoring_symbol(&commission.authority, "trusted authority")?;
        authoring_symbol(&commission.review, "trusted review")?;
        if self.derivation.producer != commission.producer
            || self.derivation.authority != commission.authority
            || self.derivation.review != commission.review
        {
            return Err(NeutralInputError::new(
                NeutralInputGap::Identity,
                "producer, authority or review is not the trusted commission",
            ));
        }
        if source_byte_identity(source)? != self.selected_source {
            return Err(NeutralInputError::new(
                NeutralInputGap::SourceMismatch,
                "selected-source is not the exact-byte identity of the source",
            ));
        }
        let derived = RestatedMeasurements::from_source(source)?;
        if derived.facts != self.facts {
            return Err(NeutralInputError::new(
                NeutralInputGap::FactMismatch,
                "facts differ from the retained table",
            ));
        }
        if self.derivation.procedure != derived.procedure {
            return Err(NeutralInputError::new(
                NeutralInputGap::Identity,
                "procedure is not the conversion recipe derived from the source",
            ));
        }
        if self.derivation.precision != derived.precision {
            return Err(NeutralInputError::new(
                NeutralInputGap::FactMismatch,
                "precision is not the precision of the source spelling",
            ));
        }
        let source_digest: [u8; 32] = Sha256::digest(source).into();
        let table_digest: [u8; 32] = Sha256::digest(&derived.table_region).into();
        let want = [
            DerivationSource {
                role: DerivationSourceRole::SelectedRoundSource,
                digest: source_digest,
            },
            DerivationSource {
                role: DerivationSourceRole::MeasurementsTableRegion,
                digest: table_digest,
            },
        ];
        if self.derivation.source_digests != want {
            return Err(NeutralInputError::new(
                NeutralInputGap::SourceMismatch,
                "source digests are not the selected source and exact table region",
            ));
        }
        Ok(())
    }

    /// Builds the input a commissioned author records for the selected source.
    /// The facts and table region are derived by this owner, never supplied.
    ///
    /// # Errors
    /// Returns a gap if the table cannot be derived or an identity fails.
    pub fn author_restated(source: &[u8], commission: &TrustedCommission) -> Result<Self> {
        authoring_symbol(&commission.producer, "producer")?;
        authoring_symbol(&commission.authority, "authority")?;
        authoring_symbol(&commission.review, "review")?;
        use sha2::{Digest, Sha256};
        let derived = RestatedMeasurements::from_source(source)?;
        let source_digest: [u8; 32] = Sha256::digest(source).into();
        let table_digest: [u8; 32] = Sha256::digest(&derived.table_region).into();
        Ok(Self {
            selected_source: source_byte_identity(source)?,
            facts: derived.facts,
            derivation: NeutralFactDerivationReceipt {
                basis: DerivationBasis::RestatedMeasurements,
                procedure: derived.procedure,
                producer: commission.producer.clone(),
                authority: commission.authority.clone(),
                review: commission.review.clone(),
                source_digests: vec![
                    DerivationSource {
                        role: DerivationSourceRole::SelectedRoundSource,
                        digest: source_digest,
                    },
                    DerivationSource {
                        role: DerivationSourceRole::MeasurementsTableRegion,
                        digest: table_digest,
                    },
                ],
                precision: derived.precision,
                no_runtime_consumption: true,
            },
        })
    }
}

/// The general expression codec, supplied by the caller. The caller must bind
/// this to the trusted `codec/lisp` adapter: an arbitrary producer-supplied
/// implementation is not a trust input.  This crate stays pure
/// and codec-free: admission needs only the two directions, and refuses any
/// stored form that is not exactly the codec's canonical encoding.
pub trait SlotCodec {
    /// Decodes stored bytes to a datum.
    ///
    /// # Errors
    /// Returns a diagnostic if the bytes are not a valid encoding.
    fn decode(&self, bytes: &[u8]) -> core::result::Result<Datum, String>;
    /// Encodes a datum to its canonical stored form.
    ///
    /// # Errors
    /// Returns a diagnostic if the datum cannot be encoded.
    fn encode(&self, datum: &Datum) -> core::result::Result<Vec<u8>, String>;
}

impl NeutralQualificationInput {
    /// The one admission entry point. It receives the exact stored bytes: it
    /// refuses a stored form over 64 KiB before decoding, decodes through the
    /// supplied codec, requires the bytes to equal the codec's canonical
    /// re-encoding of what they decode to, strictly decodes the Shape (which
    /// also bounds the canonical datum size), and verifies the input against the
    /// exact selected source and the trusted commission.
    ///
    /// # Errors
    /// Returns the first named gap.
    pub fn admit(
        stored: &[u8],
        codec: &dyn SlotCodec,
        source: &[u8],
        commission: &TrustedCommission,
    ) -> Result<Self> {
        Self::check_encoded_size(stored.len())?;
        let datum = codec
            .decode(stored)
            .map_err(|e| NeutralInputError::new(NeutralInputGap::Malformed, e))?;
        let canonical = codec
            .encode(&datum)
            .map_err(|e| NeutralInputError::new(NeutralInputGap::Malformed, e))?;
        if canonical != stored {
            return Err(NeutralInputError::new(
                NeutralInputGap::Malformed,
                "stored bytes are not the codec's canonical encoding",
            ));
        }
        let input = Self::from_datum(&datum)?;
        input.verify_restated_measurements(source, commission)?;
        Ok(input)
    }
}
