//! Validation specific to additive source facts.

use std::collections::{BTreeMap, BTreeSet};

use crate::{IndexDoc, IndexError, ProtocolResolution, UnresolvedReason};

pub(crate) fn reject_invalid_source_facts(doc: &IndexDoc) -> Result<(), IndexError> {
    let anchors = doc
        .anchors
        .iter()
        .map(|record| record.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut declarations = BTreeSet::new();
    for fact in &doc.declarations {
        require_anchor(&anchors, "declaration", fact.anchor.as_str())?;
        if !declarations.insert((fact.anchor.as_str(), fact.role, fact.module_path.as_str())) {
            return Err(IndexError::DuplicateSourceFact {
                anchor: fact.anchor.to_string(),
                role: fact.role.as_str(),
            });
        }
        let size = fact.generics.len() + fact.members.iter().map(String::len).sum::<usize>();
        if fact.syntax_bound.max_bytes == 0
            || (!fact.syntax_bound.truncated && size > fact.syntax_bound.max_bytes)
            || (fact.syntax_bound.truncated
                && (!fact.generics.is_empty() || !fact.members.is_empty()))
        {
            return invalid_bound(fact.anchor.to_string(), fact.syntax_bound.max_bytes);
        }
    }
    reject_invalid_protocol_relations(doc, &anchors)
}

fn reject_invalid_protocol_relations(
    doc: &IndexDoc,
    anchors: &BTreeSet<&str>,
) -> Result<(), IndexError> {
    let mut relations = BTreeSet::new();
    let mut resolutions = BTreeMap::new();
    for relation in &doc.protocol_relations {
        require_anchor(anchors, "protocol", relation.anchor.as_str())?;
        let key = (
            relation.anchor.as_str(),
            relation.implementor.as_str(),
            relation.source_spelling.as_str(),
        );
        if !relations.insert((key, &relation.resolution)) {
            return Err(IndexError::DuplicateProtocolRelation {
                anchor: relation.anchor.to_string(),
                implementor: relation.implementor.clone(),
            });
        }
        if let Some(previous) = resolutions.insert(key, &relation.resolution)
            && previous != &relation.resolution
        {
            return Err(IndexError::ConflictingProtocolResolution {
                anchor: relation.anchor.to_string(),
                implementor: relation.implementor.clone(),
            });
        }
        if relation.body_bound.max_bytes == 0
            || (!relation.body_bound.truncated
                && relation.body_fingerprint.len() > relation.body_bound.max_bytes)
            || (relation.body_bound.truncated && !relation.body_fingerprint.is_empty())
        {
            return invalid_bound(relation.anchor.to_string(), relation.body_bound.max_bytes);
        }
        validate_resolution(relation)?;
    }
    Ok(())
}

fn validate_resolution(relation: &crate::ProtocolRelation) -> Result<(), IndexError> {
    match &relation.resolution {
        ProtocolResolution::Resolved { protocol } if protocol.is_empty() => {}
        ProtocolResolution::Unresolved { reason, candidates }
            if !strictly_sorted(candidates)
                || (matches!(reason, UnresolvedReason::AmbiguousName)
                    != (candidates.len() > 1)) => {}
        _ => return Ok(()),
    }
    Err(IndexError::InvalidProtocolResolution {
        anchor: relation.anchor.to_string(),
    })
}

fn require_anchor(
    anchors: &BTreeSet<&str>,
    owner_kind: &'static str,
    anchor: &str,
) -> Result<(), IndexError> {
    if anchors.contains(anchor) {
        Ok(())
    } else {
        Err(IndexError::UnresolvedClaim {
            owner: format!("{owner_kind}:{anchor}"),
            kind: "anchor",
            id: anchor.to_owned(),
        })
    }
}

fn invalid_bound<T>(anchor: String, max_bytes: usize) -> Result<T, IndexError> {
    Err(IndexError::InvalidSourceBound { anchor, max_bytes })
}

pub(crate) fn reject_unstable_source_fact_order(doc: &IndexDoc) -> Result<(), IndexError> {
    if !strictly_sorted(&doc.declarations) {
        return Err(IndexError::UnstableOrdering {
            kind: "declaration",
        });
    }
    if !strictly_sorted(&doc.protocol_relations) {
        return Err(IndexError::UnstableOrdering {
            kind: "protocol-relation",
        });
    }
    Ok(())
}

fn strictly_sorted<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
