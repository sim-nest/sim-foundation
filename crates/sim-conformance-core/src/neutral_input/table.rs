// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use sim_kernel::{ContentId, Datum, Symbol};

use super::MS_PER_HOUR;
use super::decode::*;
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
        let start = unique(text, TABLE_HEADER)?;
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
        let (control, heads) = contrast_sentences(text)?;
        let last_frontier_total = green + interrupted + pending;
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
                    denominator: proof_total,
                },
            ],
        };
        check_facts(&facts)?;
        Ok(Self {
            facts,
            table_region: table.as_bytes().to_vec(),
        })
    }
}

fn unique(text: &str, needle: &str) -> Result<usize> {
    let mut found = text.match_indices(needle).map(|(i, _)| i);
    match (found.next(), found.next()) {
        (Some(i), None) => Ok(i),
        _ => Err(NeutralInputError::new(
            NeutralInputGap::SourceMismatch,
            "retained-measurement table not found exactly once",
        )),
    }
}

fn contrast_sentences(text: &str) -> Result<(u64, u64)> {
    let sentence = |lead: &str| -> Result<u64> {
        let i = unique(text, lead)?;
        let tail = &text[i + lead.len()..];
        let n: String = tail.chars().take_while(char::is_ascii_digit).collect();
        number(&n).ok_or_else(|| {
            NeutralInputError::new(NeutralInputGap::FactMismatch, "contrast sentence malformed")
        })
    };
    let control = sentence("A control-only repair retained ")?;
    if !text.contains(&format!(
        "A control-only repair retained {control} of 280 proof receipts."
    )) {
        return Err(NeutralInputError::new(
            NeutralInputGap::FactMismatch,
            "control-only denominator is not 280 proof receipts",
        ));
    }
    if !text.contains("retained only one.") {
        return Err(NeutralInputError::new(
            NeutralInputGap::FactMismatch,
            "public-head-and-pin-change sentence absent",
        ));
    }
    Ok((control, 1))
}

impl NeutralQualificationInput {
    /// Verifies an input against the exact selected source bytes: the source
    /// identity, the derivation's source and table-region digests, and that every
    /// fact equals the one this owner derives from the table. The ids and bounds
    /// were already checked by [`Self::from_datum`].
    ///
    /// # Errors
    /// Returns the first named gap.
    pub fn verify_restated_measurements(&self, source: &[u8]) -> Result<()> {
        use sha2::{Digest, Sha256};
        if self.derivation.basis != DerivationBasis::RestatedMeasurements {
            return Err(NeutralInputError::new(
                NeutralInputGap::FactMismatch,
                "basis is not restated measurements",
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
    pub fn author_restated(
        source: &[u8],
        procedure: ContentId,
        producer: Symbol,
        authority: Symbol,
        review: Symbol,
    ) -> Result<Self> {
        use sha2::{Digest, Sha256};
        let derived = RestatedMeasurements::from_source(source)?;
        let source_digest: [u8; 32] = Sha256::digest(source).into();
        let table_digest: [u8; 32] = Sha256::digest(&derived.table_region).into();
        Ok(Self {
            selected_source: source_byte_identity(source)?,
            facts: derived.facts,
            derivation: NeutralFactDerivationReceipt {
                basis: DerivationBasis::RestatedMeasurements,
                procedure,
                producer,
                authority,
                review,
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
                precision: MeasurementPrecision {
                    source_decimal_places: 2,
                },
                no_runtime_consumption: true,
            },
        })
    }
}
