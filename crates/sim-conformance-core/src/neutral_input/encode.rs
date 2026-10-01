// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use sim_kernel::{ContentId, Datum, NumberLiteral, Symbol};

use super::types::*;
use super::{NEUTRAL_INPUT_VERSION, NS};

pub(super) fn node(name: &str, mut fields: Vec<(&str, Datum)>) -> Datum {
    fields.sort_by(|a, b| a.0.cmp(b.0));
    Datum::Node {
        tag: Symbol::qualified(NS, name),
        fields: fields
            .into_iter()
            .map(|(field, value)| (Symbol::new(field), value))
            .collect(),
    }
}

pub(super) fn u64_datum(value: u64) -> Datum {
    Datum::Number(NumberLiteral {
        domain: Symbol::qualified(NS, "u64"),
        canonical: value.to_string(),
    })
}

pub(super) fn symbol(name: &str) -> Datum {
    Datum::Symbol(Symbol::qualified(NS, name))
}

pub(super) fn content_id_datum(id: &ContentId) -> Datum {
    node(
        "content-id-v1",
        vec![
            ("algorithm", Datum::Symbol(id.algorithm.clone())),
            ("digest", Datum::Bytes(id.bytes.to_vec())),
        ],
    )
}

pub(super) fn domain_id(datum: &Datum, domain: &str) -> Result<ContentId> {
    let Datum::Node { fields, .. } = datum else {
        return Err(NeutralInputError::new(
            NeutralInputGap::Malformed,
            "id of non-node",
        ));
    };
    Datum::Node {
        tag: Symbol::qualified(NS, domain),
        fields: fields.clone(),
    }
    .content_id()
    .map_err(|e| NeutralInputError::new(NeutralInputGap::Identity, e.to_string()))
}

impl NeutralFacts {
    pub(super) fn datum_without_id(&self) -> Datum {
        let epochs = node(
            "epoch-counts-v1",
            vec![
                ("failed", u64_datum(self.epochs.failed)),
                (
                    "interrupted-running",
                    u64_datum(self.epochs.interrupted_running),
                ),
                ("invalidated", u64_datum(self.epochs.invalidated)),
                ("total", u64_datum(self.epochs.total)),
            ],
        );
        let proof = node(
            "proof-class-counts-v1",
            vec![
                ("generate", u64_datum(self.proof_classes.generate)),
                ("package", u64_datum(self.proof_classes.package)),
                ("standalone", u64_datum(self.proof_classes.standalone)),
                ("total", u64_datum(self.proof_classes.total)),
                ("workspace", u64_datum(self.proof_classes.workspace)),
            ],
        );
        let spans = node(
            "span-counts-v1",
            vec![
                ("finished", u64_datum(self.spans.finished)),
                ("total", u64_datum(self.spans.total)),
            ],
        );
        let frontier = node(
            "frontier-counts-v1",
            vec![
                ("green", u64_datum(self.last_frontier.green)),
                ("interrupted", u64_datum(self.last_frontier.interrupted)),
                ("pending", u64_datum(self.last_frontier.pending)),
                ("total", u64_datum(self.last_frontier.total)),
            ],
        );
        let reuse = Datum::Vector(
            self.contrast_reuse
                .iter()
                .map(|r| {
                    node(
                        "reuse-count-v1",
                        vec![
                            ("contrast", symbol(r.contrast.name())),
                            ("denominator", u64_datum(r.denominator)),
                            ("retained", u64_datum(r.retained)),
                        ],
                    )
                })
                .collect(),
        );
        node(
            "neutral-facts-v1",
            vec![
                ("contrast-reuse", reuse),
                ("epochs", epochs),
                ("last-frontier", frontier),
                ("lifecycle-rows", u64_datum(self.lifecycle_rows)),
                ("materialized", u64_datum(self.materialized)),
                (
                    "overlapping-span-milliseconds",
                    u64_datum(self.overlapping_span_milliseconds),
                ),
                ("proof-classes", proof),
                ("repeated-ready-rows", u64_datum(self.repeated_ready_rows)),
                (
                    "repeated-receipt-rows",
                    u64_datum(self.repeated_receipt_rows),
                ),
                ("revisions", u64_datum(self.revisions)),
                ("scheduled", u64_datum(self.scheduled)),
                ("spans", spans),
            ],
        )
    }

    /// Identity of the facts only.
    ///
    /// # Errors
    /// Returns an identity gap if the value cannot be canonicalized.
    pub fn id(&self) -> Result<ContentId> {
        domain_id(&self.datum_without_id(), "neutral-facts-content-v1")
    }
}

impl NeutralFactDerivationReceipt {
    pub(super) fn datum_without_id(&self, facts: &ContentId, selected: &ContentId) -> Datum {
        node(
            "neutral-fact-derivation-receipt-v1",
            vec![
                ("authority", Datum::Symbol(self.authority.clone())),
                (
                    "basis",
                    symbol(match self.basis {
                        DerivationBasis::RestatedMeasurements => "restated-measurements",
                        DerivationBasis::ReadOnlyAggregates => "read-only-aggregates",
                    }),
                ),
                ("facts", content_id_datum(facts)),
                (
                    "no-runtime-consumption",
                    Datum::Bool(self.no_runtime_consumption),
                ),
                (
                    "precision",
                    node(
                        "measurement-precision-v1",
                        vec![
                            (
                                "output-unit",
                                Datum::Symbol(Symbol::qualified("unit", "millisecond")),
                            ),
                            ("rounding", symbol("nearest-millisecond")),
                            (
                                "source-decimal-places",
                                u64_datum(self.precision.source_decimal_places),
                            ),
                            (
                                "source-unit",
                                Datum::Symbol(Symbol::qualified("unit", "hour")),
                            ),
                        ],
                    ),
                ),
                ("procedure", content_id_datum(&self.procedure)),
                ("producer", Datum::Symbol(self.producer.clone())),
                ("review", Datum::Symbol(self.review.clone())),
                ("selected-source", content_id_datum(selected)),
                (
                    "source-digests",
                    Datum::Vector(
                        self.source_digests
                            .iter()
                            .map(|s| {
                                node(
                                    "derivation-source-v1",
                                    vec![
                                        (
                                            "algorithm",
                                            Datum::Symbol(Symbol::qualified("core", "sha256")),
                                        ),
                                        ("digest", Datum::Bytes(s.digest.to_vec())),
                                        (
                                            "role",
                                            symbol(match s.role {
                                                DerivationSourceRole::SelectedRoundSource => {
                                                    "selected-round-source"
                                                }
                                                DerivationSourceRole::MeasurementsTableRegion => {
                                                    "measurements-table-region"
                                                }
                                            }),
                                        ),
                                    ],
                                )
                            })
                            .collect(),
                    ),
                ),
            ],
        )
    }
}

pub(super) fn with_id(datum: Datum, id: &ContentId) -> Datum {
    let Datum::Node { tag, mut fields } = datum else {
        return datum;
    };
    fields.push((Symbol::new("id"), content_id_datum(id)));
    fields.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    Datum::Node { tag, fields }
}

impl NeutralQualificationInput {
    /// The canonical datum form, with the facts id and the receipt id filled in.
    ///
    /// # Errors
    /// Returns an identity gap if an id cannot be computed.
    pub fn to_datum(&self) -> Result<Datum> {
        let facts_id = self.facts.id()?;
        let facts = with_id(self.facts.datum_without_id(), &facts_id);
        let receipt_plain = self
            .derivation
            .datum_without_id(&facts_id, &self.selected_source);
        let receipt_id = domain_id(&receipt_plain, "neutral-fact-derivation-content-v1")?;
        let receipt = with_id(receipt_plain, &receipt_id);
        Ok(node(
            "neutral-input-v1",
            vec![
                ("derivation", receipt),
                ("facts", facts),
                ("selected-source", content_id_datum(&self.selected_source)),
                ("version", u64_datum(NEUTRAL_INPUT_VERSION)),
            ],
        ))
    }
}
