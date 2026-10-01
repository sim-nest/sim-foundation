// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use sim_kernel::{ContentId, Datum, Symbol};

use super::MS_PER_HOUR;
use super::decode::*;
use super::grammar::*;
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

/// The facts and table region derived from the selected source, by this owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestatedMeasurements {
    /// Facts the table and the two contrast sentences state.
    pub facts: NeutralFacts,
    /// The three source regions (whole source, table, contrast paragraph) with
    /// their offsets, lengths and digests, as a receipt records them.
    pub regions: Vec<DerivationSource>,
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
        let text = utf8(source)?;
        let (table, paragraph) = locate(text)?;
        Self::build(source, text, table, paragraph)
    }

    /// Derives the facts from the byte regions a receipt names. It slices and
    /// parses exactly those regions and never searches the document: which
    /// regions are designated is an authoring act pinned by offset, length and
    /// digest, and judged by the independent reviewer of the slot. The only
    /// structural anchor checked is that the contrast paragraph directly
    /// follows the E1 heading.
    ///
    /// # Errors
    /// Returns a gap if the regions are not the three expected, are out of
    /// bounds or misaligned, or do not parse.
    pub fn from_regions(source: &[u8], regions: &[DerivationSource]) -> Result<Self> {
        let text = utf8(source)?;
        let bad =
            |what: &str| NeutralInputError::new(NeutralInputGap::SourceMismatch, what.to_owned());
        let [whole, table, paragraph] = regions else {
            return Err(bad("derivation must name exactly three source regions"));
        };
        if whole.role != DerivationSourceRole::SelectedRoundSource
            || table.role != DerivationSourceRole::MeasurementsTableRegion
            || paragraph.role != DerivationSourceRole::ContrastStatementRegion
        {
            return Err(bad("source regions are not in role order"));
        }
        if whole.start != 0 || whole.length != source.len() as u64 {
            return Err(bad("selected-round-source region is not the whole source"));
        }
        let range = |r: &DerivationSource| -> Result<core::ops::Range<usize>> {
            let start = usize::try_from(r.start).map_err(|_| bad("region offset overflows"))?;
            let end = start
                .checked_add(usize::try_from(r.length).map_err(|_| bad("region length overflows"))?)
                .ok_or_else(|| bad("region end overflows"))?;
            let aligned =
                |i: usize| i == 0 || i == text.len() || text.as_bytes().get(i - 1) == Some(&b'\n');
            if end > text.len()
                || start >= end
                || !text.is_char_boundary(start)
                || !text.is_char_boundary(end)
                || !aligned(start)
            {
                return Err(bad("region is out of bounds or not line aligned"));
            }
            // The end must be the end of the source, just after a line ending, or
            // just before one: ordinary source bytes on both sides are a mid-line cut.
            let bytes = text.as_bytes();
            let end_aligned = end == text.len()
                || bytes[end - 1] == b'\n'
                || (bytes[end] == b'\n' && bytes[end - 1] != b'\r')
                || bytes[end..].starts_with(b"\r\n");
            if !end_aligned {
                return Err(bad("region does not end at a line boundary"));
            }
            Ok(start..end)
        };
        let table_range = range(table)?;
        let paragraph_range = range(paragraph)?;
        if table_range.start < paragraph_range.end && paragraph_range.start < table_range.end {
            return Err(bad("table and contrast regions overlap"));
        }
        let anchor = text[..paragraph_range.start]
            .trim_end()
            .lines()
            .last()
            .unwrap_or("");
        if !markdown::atx_heading(anchor)
            .is_some_and(|(level, content)| level == 3 && content == CONTRAST_HEADING)
        {
            return Err(bad(
                "contrast paragraph does not directly follow the E1 heading",
            ));
        }
        Self::build(source, text, table_range, paragraph_range)
    }

    fn build(
        source: &[u8],
        text: &str,
        table_range: core::ops::Range<usize>,
        paragraph_range: core::ops::Range<usize>,
    ) -> Result<Self> {
        use sha2::{Digest, Sha256};
        let table = &text[table_range.clone()];
        check_table_grammar(table)?;
        let row = |label: &str| -> Result<&str> {
            let mut found = None;
            for line in table.lines() {
                let cells = super::grammar::cells(line).unwrap_or_default();
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
        let epoch_parts = components(
            parts,
            &[" failed", " invalidated", " interrupted/running raw state"],
        )?;
        let proof_row = row("Proof leaves in the terminal graph")?;
        let (ptotal, pparts) = proof_row.split_once(": ").ok_or_else(|| bad("proof"))?;
        let proof_parts = components(
            pparts,
            &[" generate", " standalone", " workspace", " package"],
        )?;
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
        let [green, interrupted, pending] =
            components(frontier_row, &[" green", " interrupted", " pending"])?[..]
        else {
            return Err(bad("frontier"));
        };
        let previous = number(
            row("Previous fixed point before retirement")?
                .strip_suffix(" green leaves")
                .ok_or_else(|| bad("fixed point"))?,
        )
        .ok_or_else(|| bad("fixed point"))?;
        let reused =
            number(row("Reused receipts in that last epoch")?).ok_or_else(|| bad("reused"))?;
        let proof_total = number(ptotal).ok_or_else(|| bad("proof total"))?;
        let (control, control_denominator, heads) = parse_contrast(&text[paragraph_range.clone()])?;
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
                failed: epoch_parts[0],
                invalidated: epoch_parts[1],
                interrupted_running: epoch_parts[2],
                total: number(total).ok_or_else(|| bad("epochs total"))?,
            },
            proof_classes: ProofClassCounts {
                generate: proof_parts[0],
                standalone: proof_parts[1],
                workspace: proof_parts[2],
                package: proof_parts[3],
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
        let region = |role, range: &core::ops::Range<usize>, bytes: &[u8]| DerivationSource {
            role,
            start: range.start as u64,
            length: (range.end - range.start) as u64,
            digest: Sha256::digest(bytes).into(),
        };
        let whole = 0..source.len();
        let regions = vec![
            region(DerivationSourceRole::SelectedRoundSource, &whole, source),
            region(
                DerivationSourceRole::MeasurementsTableRegion,
                &table_range,
                table.as_bytes(),
            ),
            region(
                DerivationSourceRole::ContrastStatementRegion,
                &paragraph_range,
                text[paragraph_range.clone()].as_bytes(),
            ),
        ];
        Ok(Self {
            facts,
            regions,
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
        let derived = RestatedMeasurements::from_regions(source, &self.derivation.source_digests)?;
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
        // The regions were rebuilt from the recorded offsets and lengths: their
        // digests must equal the recorded ones.
        if self.derivation.source_digests != derived.regions {
            return Err(NeutralInputError::new(
                NeutralInputGap::SourceMismatch,
                "a recorded region digest is not the digest of its bytes",
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
        let derived = RestatedMeasurements::from_source(source)?;
        Ok(Self {
            selected_source: source_byte_identity(source)?,
            facts: derived.facts,
            derivation: NeutralFactDerivationReceipt {
                basis: DerivationBasis::RestatedMeasurements,
                procedure: derived.procedure,
                producer: commission.producer.clone(),
                authority: commission.authority.clone(),
                review: commission.review.clone(),
                source_digests: derived.regions,
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
