//! Validation for the SIM Index graph.

use std::collections::BTreeSet;

use crate::{AnchorId, DiscoveredSpecimen, FeatureDraft, IndexDoc, RouteStep, SpecimenId};

pub use crate::check_error::{IndexError, IndexReport};

/// Checks a complete index document.
pub fn check_index_doc(doc: &IndexDoc) -> Result<IndexReport, IndexError> {
    reject_non_ascii(doc)?;
    reject_invalid_ids(doc)?;
    reject_duplicate_ids(doc)?;
    reject_authored_literals(doc)?;
    reject_unresolved_claims(doc)?;
    reject_duplicate_claims(doc)?;
    reject_canonical_key_collisions(doc)?;
    reject_invalid_grammar_contracts(doc)?;
    reject_unrunnable_specimen_claims(doc)?;
    reject_dead_route_steps(doc)?;
    reject_dangling_doc_anchors(doc)?;
    Ok(IndexReport::from_doc(doc))
}

fn reject_non_ascii(doc: &IndexDoc) -> Result<(), IndexError> {
    check_ascii("schema", &doc.schema)?;
    check_ascii("generated_by", &doc.generated_by)?;
    for subject in &doc.subjects {
        check_ascii("subject.id", subject.id.as_str())?;
        check_ascii("subject.kind", &subject.kind)?;
        check_ascii("subject.title", &subject.title)?;
    }
    for anchor in &doc.anchors {
        check_ascii("anchor.id", anchor.id.as_str())?;
        check_ascii("anchor.kind", &anchor.kind)?;
    }
    for surface in &doc.surfaces {
        check_ascii("surface.id", surface.id.as_str())?;
        check_ascii("surface.kind", &surface.kind)?;
    }
    for specimen in &doc.specimens {
        check_ascii("specimen.id", specimen.id.as_str())?;
        check_ascii("specimen.kind", &specimen.kind)?;
        check_ascii("specimen.path", &specimen.path)?;
        if let Some(language) = &specimen.language {
            check_ascii("specimen.language", language)?;
        }
        if let Some(checked_by) = &specimen.checked_by {
            check_ascii("specimen.checked_by", checked_by)?;
        }
    }
    for draft in &doc.drafts {
        check_feature_text("draft", draft.id.as_str(), &draft.title, &draft.summary)?;
        for value in draft
            .literal_anchors
            .iter()
            .chain(&draft.literal_surfaces)
            .chain(&draft.literal_specimens)
        {
            check_ascii("draft.literal", value)?;
        }
    }
    for feature in &doc.features {
        check_feature_text(
            "feature",
            feature.id.as_str(),
            &feature.title,
            &feature.summary,
        )?;
        check_ascii("feature.key", feature.key.as_str())?;
    }
    for route in &doc.routes {
        check_ascii("route.id", route.id.as_str())?;
        check_ascii("route.title", &route.title)?;
    }
    for edge in &doc.edges {
        check_ascii("edge.from", &edge.from)?;
        check_ascii("edge.rel", &edge.rel)?;
        check_ascii("edge.to", &edge.to)?;
    }
    Ok(())
}

fn check_feature_text(
    prefix: &'static str,
    id: &str,
    title: &str,
    summary: &str,
) -> Result<(), IndexError> {
    check_ascii(prefix, id)?;
    check_ascii("title", title)?;
    check_ascii("summary", summary)
}

fn check_ascii(field: &'static str, value: &str) -> Result<(), IndexError> {
    if value.is_ascii() {
        Ok(())
    } else {
        Err(IndexError::NonAscii {
            field,
            value: value.to_owned(),
        })
    }
}

fn reject_invalid_ids(doc: &IndexDoc) -> Result<(), IndexError> {
    for subject in &doc.subjects {
        check_id("subject", subject.id.as_str())?;
    }
    for anchor in &doc.anchors {
        check_id("anchor", anchor.id.as_str())?;
        check_id("subject", anchor.subject.as_str())?;
    }
    for surface in &doc.surfaces {
        check_id("surface", surface.id.as_str())?;
        check_id("subject", surface.subject.as_str())?;
    }
    for specimen in &doc.specimens {
        check_id("specimen", specimen.id.as_str())?;
        check_id("subject", specimen.subject.as_str())?;
    }
    for draft in &doc.drafts {
        check_id("feature", draft.id.as_str())?;
        check_id("subject", draft.subject.as_str())?;
    }
    for feature in &doc.features {
        check_id("feature", feature.id.as_str())?;
        check_id("subject", feature.subject.as_str())?;
        if !feature.key.is_valid() {
            return Err(IndexError::InvalidId {
                kind: "canonical-key",
                id: feature.key.as_str().to_owned(),
            });
        }
    }
    for route in &doc.routes {
        check_id("route", route.id.as_str())?;
    }
    for edge in &doc.edges {
        check_id("edge", &edge.from)?;
        check_id("edge", &edge.to)?;
    }
    Ok(())
}

fn check_id(kind: &'static str, id: &str) -> Result<(), IndexError> {
    if crate::shape::is_index_id(id) {
        Ok(())
    } else {
        Err(IndexError::InvalidId {
            kind,
            id: id.to_owned(),
        })
    }
}

fn reject_duplicate_ids(doc: &IndexDoc) -> Result<(), IndexError> {
    duplicates(
        "subject",
        doc.subjects.iter().map(|record| record.id.as_str()),
    )?;
    duplicates(
        "anchor",
        doc.anchors.iter().map(|record| record.id.as_str()),
    )?;
    duplicates(
        "surface",
        doc.surfaces.iter().map(|record| record.id.as_str()),
    )?;
    duplicates(
        "specimen",
        doc.specimens.iter().map(|record| record.id.as_str()),
    )?;
    duplicates("draft", doc.drafts.iter().map(|record| record.id.as_str()))?;
    duplicates(
        "feature",
        doc.features.iter().map(|record| record.id.as_str()),
    )?;
    duplicates("route", doc.routes.iter().map(|record| record.id.as_str()))
}

fn duplicates<'a>(
    kind: &'static str,
    ids: impl Iterator<Item = &'a str>,
) -> Result<(), IndexError> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(IndexError::DuplicateId {
                kind,
                id: id.to_owned(),
            });
        }
    }
    Ok(())
}

fn reject_authored_literals(doc: &IndexDoc) -> Result<(), IndexError> {
    for draft in &doc.drafts {
        if !draft.literal_anchors.is_empty() {
            return literal(draft, "anchor");
        }
        if !draft.literal_surfaces.is_empty() {
            return literal(draft, "surface");
        }
        if !draft.literal_specimens.is_empty() {
            return literal(draft, "specimen");
        }
    }
    Ok(())
}

fn literal<T>(draft: &FeatureDraft, kind: &'static str) -> Result<T, IndexError> {
    Err(IndexError::LiteralClaim {
        owner: draft.id.to_string(),
        kind,
    })
}

fn reject_unresolved_claims(doc: &IndexDoc) -> Result<(), IndexError> {
    let subjects = ids(doc.subjects.iter().map(|record| record.id.as_str()));
    let anchors = ids(doc.anchors.iter().map(|record| record.id.as_str()));
    let surfaces = ids(doc.surfaces.iter().map(|record| record.id.as_str()));
    let specimens = ids(doc.specimens.iter().map(|record| record.id.as_str()));
    let features = ids(doc.features.iter().map(|record| record.id.as_str()));

    for anchor in &doc.anchors {
        require(
            &subjects,
            "anchor",
            anchor.id.as_str(),
            "subject",
            anchor.subject.as_str(),
        )?;
    }
    for surface in &doc.surfaces {
        require(
            &subjects,
            "surface",
            surface.id.as_str(),
            "subject",
            surface.subject.as_str(),
        )?;
    }
    for specimen in &doc.specimens {
        require(
            &subjects,
            "specimen",
            specimen.id.as_str(),
            "subject",
            specimen.subject.as_str(),
        )?;
    }
    for draft in &doc.drafts {
        require(
            &subjects,
            "draft",
            draft.id.as_str(),
            "subject",
            draft.subject.as_str(),
        )?;
        require_all(
            &anchors,
            draft.id.as_str(),
            "anchor",
            draft.claims_anchors.iter(),
        )?;
        require_all(
            &surfaces,
            draft.id.as_str(),
            "surface",
            draft.claims_surfaces.iter(),
        )?;
        require_all(
            &specimens,
            draft.id.as_str(),
            "specimen",
            draft.claims_specimens.iter(),
        )?;
    }
    for feature in &doc.features {
        require(
            &subjects,
            "feature",
            feature.id.as_str(),
            "subject",
            feature.subject.as_str(),
        )?;
        require_all(
            &anchors,
            feature.id.as_str(),
            "anchor",
            feature.anchors.iter(),
        )?;
        require_all(
            &surfaces,
            feature.id.as_str(),
            "surface",
            feature.surfaces.iter(),
        )?;
        require_all(
            &specimens,
            feature.id.as_str(),
            "specimen",
            feature.specimens.iter(),
        )?;
    }
    let routes = ids(doc.routes.iter().map(|record| record.id.as_str()));
    let known = subjects
        .iter()
        .chain(&anchors)
        .chain(&surfaces)
        .chain(&specimens)
        .chain(&features)
        .chain(&routes)
        .copied()
        .collect::<BTreeSet<_>>();

    for edge in &doc.edges {
        match edge.rel.as_str() {
            "contains" => {
                require(&subjects, "edge", &edge.rel, "subject", &edge.from)?;
                require(&subjects, "edge", &edge.rel, "subject", &edge.to)?;
            }
            "anchors" => {
                require(&features, "edge", &edge.rel, "feature", &edge.from)?;
                require(&anchors, "edge", &edge.rel, "anchor", &edge.to)?;
            }
            "surfaces" => {
                require(&features, "edge", &edge.rel, "feature", &edge.from)?;
                require(&surfaces, "edge", &edge.rel, "surface", &edge.to)?;
            }
            "demonstrates" => {
                require(&features, "edge", &edge.rel, "feature", &edge.from)?;
                require(&specimens, "edge", &edge.rel, "specimen", &edge.to)?;
            }
            "supports" | "presents" | "replaces" => {
                require(&features, "edge", &edge.rel, "feature", &edge.from)?;
                require(&features, "edge", &edge.rel, "feature", &edge.to)?;
            }
            "documents" => {
                require(&known, "edge", &edge.rel, "index row", &edge.from)?;
                require(&anchors, "edge", &edge.rel, "anchor", &edge.to)?;
            }
            "routes" => {
                require(&routes, "edge", &edge.rel, "route", &edge.from)?;
                require(&known, "edge", &edge.rel, "index row", &edge.to)?;
            }
            _ => {
                require(&known, "edge", &edge.rel, "index row", &edge.from)?;
                require(&known, "edge", &edge.rel, "index row", &edge.to)?;
            }
        }
    }
    Ok(())
}

fn ids<'a>(values: impl Iterator<Item = &'a str>) -> BTreeSet<&'a str> {
    values.collect()
}

fn require(
    known: &BTreeSet<&str>,
    owner_kind: &'static str,
    owner: &str,
    kind: &'static str,
    id: &str,
) -> Result<(), IndexError> {
    if known.contains(id) {
        Ok(())
    } else {
        Err(IndexError::UnresolvedClaim {
            owner: format!("{owner_kind}:{owner}"),
            kind,
            id: id.to_owned(),
        })
    }
}

fn require_all<'a, T>(
    known: &BTreeSet<&str>,
    owner: &str,
    kind: &'static str,
    values: impl Iterator<Item = &'a T>,
) -> Result<(), IndexError>
where
    T: AsRefId + 'a,
{
    for value in values {
        require(known, "feature", owner, kind, value.as_ref_id())?;
    }
    Ok(())
}

trait AsRefId {
    fn as_ref_id(&self) -> &str;
}

impl AsRefId for AnchorId {
    fn as_ref_id(&self) -> &str {
        self.as_str()
    }
}

impl AsRefId for crate::SurfaceId {
    fn as_ref_id(&self) -> &str {
        self.as_str()
    }
}

impl AsRefId for SpecimenId {
    fn as_ref_id(&self) -> &str {
        self.as_str()
    }
}

fn reject_duplicate_claims(doc: &IndexDoc) -> Result<(), IndexError> {
    for draft in &doc.drafts {
        duplicate_claims(draft.id.as_str(), "anchor", draft.claims_anchors.iter())?;
        duplicate_claims(draft.id.as_str(), "surface", draft.claims_surfaces.iter())?;
        duplicate_claims(draft.id.as_str(), "specimen", draft.claims_specimens.iter())?;
    }
    for feature in &doc.features {
        duplicate_claims(feature.id.as_str(), "anchor", feature.anchors.iter())?;
        duplicate_claims(feature.id.as_str(), "surface", feature.surfaces.iter())?;
        duplicate_claims(feature.id.as_str(), "specimen", feature.specimens.iter())?;
    }
    Ok(())
}

fn duplicate_claims<'a, T>(
    owner: &str,
    kind: &'static str,
    values: impl Iterator<Item = &'a T>,
) -> Result<(), IndexError>
where
    T: AsRefId + 'a,
{
    let mut seen = BTreeSet::new();
    for value in values {
        let id = value.as_ref_id();
        if !seen.insert(id) {
            return Err(IndexError::DuplicateClaim {
                owner: owner.to_owned(),
                kind,
                id: id.to_owned(),
            });
        }
    }
    Ok(())
}

fn reject_canonical_key_collisions(doc: &IndexDoc) -> Result<(), IndexError> {
    let mut seen = BTreeSet::new();
    for feature in &doc.features {
        if !seen.insert(feature.key.as_str()) {
            return Err(IndexError::DuplicateCanonicalKey {
                key: feature.key.as_str().to_owned(),
            });
        }
    }
    Ok(())
}

fn reject_invalid_grammar_contracts(doc: &IndexDoc) -> Result<(), IndexError> {
    let anchors = ids(doc.anchors.iter().map(|record| record.id.as_str()));
    let surfaces = ids(doc.surfaces.iter().map(|record| record.id.as_str()));
    for (owner, contracts) in doc
        .drafts
        .iter()
        .map(|draft| (draft.id.as_str(), &draft.grammar_contracts))
        .chain(
            doc.features
                .iter()
                .map(|feature| (feature.id.as_str(), &feature.grammar_contracts)),
        )
    {
        for contract in contracts {
            if !contract.is_valid() {
                return Err(IndexError::InvalidGrammarContract {
                    owner: owner.to_owned(),
                    id: contract.id.clone(),
                });
            }
            if let Some(decoder) = &contract.decoder {
                require(&anchors, "feature", owner, "anchor", decoder.as_str())?;
            }
            if let Some(encoder) = &contract.encoder {
                require(&anchors, "feature", owner, "anchor", encoder.as_str())?;
            }
            if let Some(surface) = &contract.surface {
                require(&surfaces, "feature", owner, "surface", surface.as_str())?;
            }
        }
    }
    Ok(())
}

fn reject_unrunnable_specimen_claims(doc: &IndexDoc) -> Result<(), IndexError> {
    let specimens: std::collections::BTreeMap<&str, &DiscoveredSpecimen> = doc
        .specimens
        .iter()
        .map(|specimen| (specimen.id.as_str(), specimen))
        .collect();
    for draft in &doc.drafts {
        for specimen in &draft.claims_specimens {
            ensure_runnable(&specimens, draft.id.as_str(), specimen)?;
        }
    }
    for feature in &doc.features {
        for specimen in &feature.specimens {
            ensure_runnable(&specimens, feature.id.as_str(), specimen)?;
        }
    }
    Ok(())
}

fn ensure_runnable(
    specimens: &std::collections::BTreeMap<&str, &DiscoveredSpecimen>,
    owner: &str,
    specimen: &SpecimenId,
) -> Result<(), IndexError> {
    if specimens
        .get(specimen.as_str())
        .is_some_and(|specimen| specimen.runnable && specimen.checked)
    {
        Ok(())
    } else {
        Err(IndexError::NonRunnableSpecimen {
            owner: owner.to_owned(),
            id: specimen.to_string(),
        })
    }
}

fn reject_dead_route_steps(doc: &IndexDoc) -> Result<(), IndexError> {
    let features = ids(doc.features.iter().map(|record| record.id.as_str()));
    let specimens = ids(doc.specimens.iter().map(|record| record.id.as_str()));
    for route in &doc.routes {
        if route.steps.is_empty() {
            return Err(IndexError::DeadRouteStep {
                route: route.id.to_string(),
                step: "<empty>".to_owned(),
            });
        }
        for step in &route.steps {
            match step {
                RouteStep::Feature { id, .. } if features.contains(id.as_str()) => {}
                RouteStep::Specimen { id, .. } if specimens.contains(id.as_str()) => {}
                RouteStep::Feature { id, .. } => {
                    return Err(IndexError::DeadRouteStep {
                        route: route.id.to_string(),
                        step: id.to_string(),
                    });
                }
                RouteStep::Specimen { id, .. } => {
                    return Err(IndexError::DeadRouteStep {
                        route: route.id.to_string(),
                        step: id.to_string(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn reject_dangling_doc_anchors(doc: &IndexDoc) -> Result<(), IndexError> {
    let anchors = ids(doc.anchors.iter().map(|record| record.id.as_str()));
    for specimen in &doc.specimens {
        check_doc_anchor(&anchors, specimen.id.as_str(), specimen.doc_anchor.as_ref())?;
    }
    for draft in &doc.drafts {
        check_doc_anchor(&anchors, draft.id.as_str(), draft.doc_anchor.as_ref())?;
    }
    for feature in &doc.features {
        check_doc_anchor(&anchors, feature.id.as_str(), feature.doc_anchor.as_ref())?;
    }
    for route in &doc.routes {
        check_doc_anchor(&anchors, route.id.as_str(), route.doc_anchor.as_ref())?;
    }
    Ok(())
}

fn check_doc_anchor(
    anchors: &BTreeSet<&str>,
    owner: &str,
    anchor: Option<&AnchorId>,
) -> Result<(), IndexError> {
    let Some(anchor) = anchor else {
        return Ok(());
    };
    if anchors.contains(anchor.as_str()) {
        Ok(())
    } else {
        Err(IndexError::DanglingDocAnchor {
            owner: owner.to_owned(),
            id: anchor.to_string(),
        })
    }
}
