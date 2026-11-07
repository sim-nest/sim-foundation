// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use sim_kernel::{ContentId, Datum, Symbol};

use super::encode::*;
use super::types::*;
use super::{MAX_NEUTRAL_INPUT_BYTES, NEUTRAL_INPUT_VERSION, NS};

pub(super) struct Fields<'a> {
    map: Vec<(&'a str, &'a Datum)>,
}

pub(super) fn open<'a>(datum: &'a Datum, tag: &str, allowed: &[&str]) -> Result<Fields<'a>> {
    let Datum::Node { tag: found, fields } = datum else {
        return Err(NeutralInputError::new(
            NeutralInputGap::Malformed,
            format!("expected node {tag}"),
        ));
    };
    if *found != Symbol::qualified(NS, tag) {
        return Err(NeutralInputError::new(
            NeutralInputGap::UnrecognizedTag,
            format!("expected tag {tag}"),
        ));
    }
    let mut map: Vec<(&str, &Datum)> = Vec::new();
    for (name, value) in fields {
        if name.namespace.is_some() || !allowed.contains(&&*name.name) {
            return Err(NeutralInputError::new(
                NeutralInputGap::ExtraField,
                format!("{tag}.{}", name.name),
            ));
        }
        if map.iter().any(|(n, _)| *n == &*name.name) {
            return Err(NeutralInputError::new(
                NeutralInputGap::ExtraField,
                format!("{tag}.{} repeated", name.name),
            ));
        }
        map.push((&*name.name, value));
    }
    for required in allowed {
        if !map.iter().any(|(n, _)| n == required) {
            return Err(NeutralInputError::new(
                NeutralInputGap::MissingField,
                format!("{tag}.{required}"),
            ));
        }
    }
    Ok(Fields { map })
}

impl<'a> Fields<'a> {
    pub(super) fn get(&self, name: &str) -> &'a Datum {
        self.map
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| *v)
            .expect("checked by open")
    }
    pub(super) fn u64(&self, name: &str) -> Result<u64> {
        match self.get(name) {
            Datum::Number(n) if n.domain == Symbol::qualified(NS, "u64") => {
                if n.canonical.is_empty()
                    || (n.canonical.len() > 1 && n.canonical.starts_with('0'))
                    || !n.canonical.bytes().all(|b| b.is_ascii_digit())
                {
                    return Err(NeutralInputError::new(
                        NeutralInputGap::Malformed,
                        format!("{name} not canonical u64"),
                    ));
                }
                n.canonical.parse::<u64>().map_err(|_| {
                    NeutralInputError::new(
                        NeutralInputGap::Arithmetic,
                        format!("{name} overflows u64"),
                    )
                })
            }
            _ => Err(NeutralInputError::new(
                NeutralInputGap::Malformed,
                format!("{name} is not a u64"),
            )),
        }
    }
    pub(super) fn sym(&self, name: &str) -> Result<&'a Symbol> {
        match self.get(name) {
            Datum::Symbol(s) => Ok(s),
            _ => Err(NeutralInputError::new(
                NeutralInputGap::Malformed,
                format!("{name} is not a symbol"),
            )),
        }
    }
    pub(super) fn node(&self, name: &str) -> &'a Datum {
        self.get(name)
    }
}

pub(super) fn read_content_id(datum: &Datum) -> Result<ContentId> {
    let f = open(datum, "content-id-v1", &["algorithm", "digest"])?;
    let algorithm = f.sym("algorithm")?.clone();
    if algorithm != Symbol::qualified("core", "sha256-datum-v1") {
        return Err(NeutralInputError::new(
            NeutralInputGap::UnrecognizedTag,
            "content id algorithm is not core/sha256-datum-v1",
        ));
    }
    let Datum::Bytes(bytes) = f.get("digest") else {
        return Err(NeutralInputError::new(
            NeutralInputGap::Malformed,
            "digest is not bytes",
        ));
    };
    let bytes: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
        NeutralInputError::new(NeutralInputGap::Malformed, "digest is not 32 bytes")
    })?;
    Ok(ContentId { algorithm, bytes })
}

pub(super) fn reject_text(datum: &Datum) -> Result<()> {
    match datum {
        Datum::String(_) => Err(NeutralInputError::new(
            NeutralInputGap::ForbiddenText,
            "string value",
        )),
        Datum::List(items) | Datum::Vector(items) | Datum::Set(items) => {
            items.iter().try_for_each(reject_text)
        }
        Datum::Map(entries) => entries
            .iter()
            .try_for_each(|(k, v)| reject_text(k).and_then(|()| reject_text(v))),
        Datum::Node { fields, .. } => fields.iter().try_for_each(|(_, v)| reject_text(v)),
        Datum::Nil | Datum::Bool(_) | Datum::Number(_) | Datum::Symbol(_) | Datum::Bytes(_) => {
            Ok(())
        }
    }
}

pub(super) fn bound(ok: bool, what: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(NeutralInputError::new(NeutralInputGap::Arithmetic, what))
    }
}

impl NeutralQualificationInput {
    /// Strict decode of one input datum, with every Shape-level identity,
    /// arithmetic and canonical-size check. It does not consult the selected
    /// source or the trusted commission: it is NOT admission. Use
    /// [`Self::admit`], the only admission entry point.
    ///
    /// # Errors
    /// Returns the first named gap.
    pub fn from_datum(datum: &Datum) -> Result<Self> {
        reject_text(datum)?;
        let encoded = datum
            .canonical_bytes()
            .map_err(|e| NeutralInputError::new(NeutralInputGap::Malformed, e.to_string()))?;
        Self::check_encoded_size(encoded.len())?;
        let top = open(
            datum,
            "neutral-input-v1",
            &["derivation", "facts", "selected-source", "version"],
        )?;
        if top.u64("version")? != NEUTRAL_INPUT_VERSION {
            return Err(NeutralInputError::new(
                NeutralInputGap::UnsupportedVersion,
                "version is not 1",
            ));
        }
        let selected_source = read_content_id(top.node("selected-source"))?;
        let facts = read_facts(top.node("facts"))?;
        let derivation = read_receipt(top.node("derivation"), &facts, &selected_source)?;
        Ok(Self {
            selected_source,
            facts,
            derivation,
        })
    }

    /// Refuses an encoded form larger than the Shape allows.
    ///
    /// # Errors
    /// Returns an oversize gap.
    pub(super) fn check_encoded_size(encoded_len: usize) -> Result<()> {
        if encoded_len > MAX_NEUTRAL_INPUT_BYTES {
            return Err(NeutralInputError::new(
                NeutralInputGap::Oversize,
                "encoded input exceeds 64 KiB",
            ));
        }
        Ok(())
    }
}

pub(super) fn read_facts(datum: &Datum) -> Result<NeutralFacts> {
    let f = open(
        datum,
        "neutral-facts-v1",
        &[
            "contrast-reuse",
            "epochs",
            "id",
            "last-frontier",
            "lifecycle-rows",
            "materialized",
            "overlapping-span-milliseconds",
            "proof-classes",
            "repeated-ready-rows",
            "repeated-receipt-rows",
            "revisions",
            "scheduled",
            "spans",
        ],
    )?;
    let e = open(
        f.node("epochs"),
        "epoch-counts-v1",
        &["failed", "interrupted-running", "invalidated", "total"],
    )?;
    let epochs = EpochCounts {
        failed: e.u64("failed")?,
        invalidated: e.u64("invalidated")?,
        interrupted_running: e.u64("interrupted-running")?,
        total: e.u64("total")?,
    };
    let p = open(
        f.node("proof-classes"),
        "proof-class-counts-v1",
        &["generate", "package", "standalone", "total", "workspace"],
    )?;
    let proof_classes = ProofClassCounts {
        generate: p.u64("generate")?,
        standalone: p.u64("standalone")?,
        workspace: p.u64("workspace")?,
        package: p.u64("package")?,
        total: p.u64("total")?,
    };
    let s = open(f.node("spans"), "span-counts-v1", &["finished", "total"])?;
    let spans = SpanCounts {
        finished: s.u64("finished")?,
        total: s.u64("total")?,
    };
    let fr = open(
        f.node("last-frontier"),
        "frontier-counts-v1",
        &["green", "interrupted", "pending", "total"],
    )?;
    let last_frontier = FrontierCounts {
        green: fr.u64("green")?,
        interrupted: fr.u64("interrupted")?,
        pending: fr.u64("pending")?,
        total: fr.u64("total")?,
    };
    let Datum::Vector(items) = f.node("contrast-reuse") else {
        return Err(NeutralInputError::new(
            NeutralInputGap::Malformed,
            "contrast-reuse is not a vector",
        ));
    };
    let mut contrast_reuse = Vec::new();
    for item in items {
        let r = open(
            item,
            "reuse-count-v1",
            &["contrast", "denominator", "retained"],
        )?;
        contrast_reuse.push(ReuseCount {
            contrast: ReuseContrast::from_symbol(r.sym("contrast")?)?,
            retained: r.u64("retained")?,
            denominator: r.u64("denominator")?,
        });
    }
    let facts = NeutralFacts {
        scheduled: f.u64("scheduled")?,
        materialized: f.u64("materialized")?,
        revisions: f.u64("revisions")?,
        epochs,
        proof_classes,
        lifecycle_rows: f.u64("lifecycle-rows")?,
        spans,
        overlapping_span_milliseconds: f.u64("overlapping-span-milliseconds")?,
        repeated_ready_rows: f.u64("repeated-ready-rows")?,
        repeated_receipt_rows: f.u64("repeated-receipt-rows")?,
        last_frontier,
        contrast_reuse,
    };
    check_facts(&facts)?;
    if read_content_id(f.node("id"))? != facts.id()? {
        return Err(NeutralInputError::new(
            NeutralInputGap::Identity,
            "facts id is not the facts-only identity",
        ));
    }
    Ok(facts)
}

/// Bounds and identities every fact set must satisfy.
pub(super) fn check_facts(f: &NeutralFacts) -> Result<()> {
    bound(
        f.materialized <= f.scheduled,
        "materialized exceeds scheduled",
    )?;
    bound(
        f.epochs
            .failed
            .checked_add(f.epochs.invalidated)
            .and_then(|v| v.checked_add(f.epochs.interrupted_running))
            == Some(f.epochs.total),
        "epoch counts do not sum to total",
    )?;
    let p = &f.proof_classes;
    bound(
        p.generate
            .checked_add(p.standalone)
            .and_then(|v| v.checked_add(p.workspace))
            .and_then(|v| v.checked_add(p.package))
            == Some(p.total),
        "proof classes do not sum to total",
    )?;
    bound(
        f.spans.finished <= f.spans.total,
        "finished spans exceed all spans",
    )?;
    let fr = &f.last_frontier;
    bound(
        fr.green
            .checked_add(fr.interrupted)
            .and_then(|v| v.checked_add(fr.pending))
            == Some(fr.total),
        "frontier counts do not sum to total",
    )?;
    bound(
        fr.total == p.total,
        "frontier total differs from proof-leaf total",
    )?;
    let mut seen: Vec<ReuseContrast> = Vec::new();
    for r in &f.contrast_reuse {
        bound(r.retained <= r.denominator, "retained exceeds denominator")?;
        bound(
            r.contrast != ReuseContrast::PreviousFixedPoint || r.retained == r.denominator,
            "previous fixed point is not all green",
        )?;
        bound(!seen.contains(&r.contrast), "contrast named twice")?;
        seen.push(r.contrast);
    }
    bound(
        f.contrast_reuse
            .windows(2)
            .all(|w| w[0].contrast < w[1].contrast),
        "contrast-reuse is not ordered",
    )?;
    Ok(())
}

pub(super) fn read_receipt(
    datum: &Datum,
    facts: &NeutralFacts,
    selected: &ContentId,
) -> Result<NeutralFactDerivationReceipt> {
    let r = open(
        datum,
        "neutral-fact-derivation-receipt-v1",
        &[
            "authority",
            "basis",
            "facts",
            "id",
            "no-runtime-consumption",
            "precision",
            "procedure",
            "producer",
            "review",
            "selected-source",
            "source-digests",
        ],
    )?;
    let facts_id = facts.id()?;
    if read_content_id(r.node("facts"))? != facts_id {
        return Err(NeutralInputError::new(
            NeutralInputGap::Identity,
            "receipt facts reference is not the facts id",
        ));
    }
    if &read_content_id(r.node("selected-source"))? != selected {
        return Err(NeutralInputError::new(
            NeutralInputGap::Identity,
            "receipt and outer selected-source differ",
        ));
    }
    let basis = match r.sym("basis")? {
        s if *s == Symbol::qualified(NS, "restated-measurements") => {
            DerivationBasis::RestatedMeasurements
        }
        s if *s == Symbol::qualified(NS, "read-only-aggregates") => {
            DerivationBasis::ReadOnlyAggregates
        }
        _ => {
            return Err(NeutralInputError::new(
                NeutralInputGap::UnrecognizedTag,
                "unregistered basis",
            ));
        }
    };
    let pr = open(
        r.node("precision"),
        "measurement-precision-v1",
        &[
            "output-unit",
            "rounding",
            "source-decimal-places",
            "source-unit",
        ],
    )?;
    if *pr.sym("output-unit")? != Symbol::qualified("unit", "millisecond")
        || *pr.sym("source-unit")? != Symbol::qualified("unit", "hour")
        || *pr.sym("rounding")? != Symbol::qualified(NS, "nearest-millisecond")
    {
        return Err(NeutralInputError::new(
            NeutralInputGap::UnrecognizedTag,
            "precision units or rounding unregistered",
        ));
    }
    let Datum::Bool(no_runtime_consumption) = r.node("no-runtime-consumption") else {
        return Err(NeutralInputError::new(
            NeutralInputGap::Malformed,
            "no-runtime-consumption is not a bool",
        ));
    };
    if !no_runtime_consumption {
        return Err(NeutralInputError::new(
            NeutralInputGap::Arithmetic,
            "no-runtime-consumption must be true",
        ));
    }
    let Datum::Vector(sources) = r.node("source-digests") else {
        return Err(NeutralInputError::new(
            NeutralInputGap::Malformed,
            "source-digests is not a vector",
        ));
    };
    let mut source_digests = Vec::new();
    for s in sources {
        let sf = open(
            s,
            "derivation-source-v1",
            &["algorithm", "digest", "length", "role", "start"],
        )?;
        if *sf.sym("algorithm")? != Symbol::qualified("core", "sha256") {
            return Err(NeutralInputError::new(
                NeutralInputGap::UnrecognizedTag,
                "source digest algorithm",
            ));
        }
        let role = match sf.sym("role")? {
            x if *x == Symbol::qualified(NS, "selected-round-source") => {
                DerivationSourceRole::SelectedRoundSource
            }
            x if *x == Symbol::qualified(NS, "measurements-table-region") => {
                DerivationSourceRole::MeasurementsTableRegion
            }
            x if *x == Symbol::qualified(NS, "contrast-statement-region") => {
                DerivationSourceRole::ContrastStatementRegion
            }
            _ => {
                return Err(NeutralInputError::new(
                    NeutralInputGap::UnrecognizedTag,
                    "source role",
                ));
            }
        };
        let Datum::Bytes(b) = sf.get("digest") else {
            return Err(NeutralInputError::new(
                NeutralInputGap::Malformed,
                "source digest is not bytes",
            ));
        };
        let digest: [u8; 32] = b.as_slice().try_into().map_err(|_| {
            NeutralInputError::new(NeutralInputGap::Malformed, "source digest is not 32 bytes")
        })?;
        source_digests.push(DerivationSource {
            role,
            start: sf.u64("start")?,
            length: sf.u64("length")?,
            digest,
        });
    }
    let receipt = NeutralFactDerivationReceipt {
        basis,
        procedure: read_content_id(r.node("procedure"))?,
        producer: checked_authoring(r.sym("producer")?, "producer")?,
        authority: checked_authoring(r.sym("authority")?, "authority")?,
        review: checked_authoring(r.sym("review")?, "review")?,
        source_digests,
        precision: MeasurementPrecision {
            source_decimal_places: pr.u64("source-decimal-places")?,
        },
        no_runtime_consumption: *no_runtime_consumption,
    };
    let plain = receipt.datum_without_id(&facts_id, selected);
    if read_content_id(r.node("id"))? != domain_id(&plain, "neutral-fact-derivation-content-v1")? {
        return Err(NeutralInputError::new(
            NeutralInputGap::Identity,
            "receipt id is not its id-excluded identity",
        ));
    }
    Ok(receipt)
}

/// Bounded identifier grammar for the three authoring symbols.
pub(super) fn authoring_symbol(symbol: &Symbol, what: &str) -> Result<()> {
    let ok_namespace = symbol.namespace.as_deref() == Some("authoring");
    let name = &*symbol.name;
    let ok_name = (1..=96).contains(&name.len())
        && name
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if ok_namespace && ok_name {
        Ok(())
    } else {
        Err(NeutralInputError::new(
            NeutralInputGap::ForbiddenText,
            format!("{what} is not a bounded authoring identifier"),
        ))
    }
}

fn checked_authoring(symbol: &Symbol, what: &str) -> Result<Symbol> {
    authoring_symbol(symbol, what)?;
    Ok(symbol.clone())
}
