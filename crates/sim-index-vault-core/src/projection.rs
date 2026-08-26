use crate::*;
use sim_index_core::{
    IndexDoc, IndexRowRef, ProtocolResolution, Visibility, check_index_doc, check_index_fragment,
};
use std::collections::{BTreeMap, BTreeSet};

impl VaultProjection {
    /// Projects only a checked, complete public document through `inventory()`.
    pub fn from_complete(
        doc: &IndexDoc,
        granularity: VaultGranularity,
    ) -> Result<Self, ProjectionError> {
        validate_capacity(doc)?;
        validate_source_paths(doc)?;
        let (metadata, inventory) = doc.inventory();
        if metadata.visibility != Visibility::Public {
            return Err(ProjectionError::NonPublicDocument);
        }
        check_index_doc(doc)
            .map_err(|error| ProjectionError::IncompleteDocument(error.to_string()))?;
        let anchors = inventory
            .iter()
            .filter_map(|row| match row {
                IndexRowRef::Anchor(a) => Some(a.id.to_string()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let anchor_subjects = anchor_subjects(doc);
        for row in &inventory {
            match row {
                IndexRowRef::Declaration(d) if !anchors.contains(d.anchor.as_str()) => {
                    return Err(ProjectionError::MissingAnchor(d.anchor.to_string()));
                }
                IndexRowRef::ProtocolRelation(p) if !anchors.contains(p.anchor.as_str()) => {
                    return Err(ProjectionError::MissingAnchor(p.anchor.to_string()));
                }
                _ => {}
            }
        }
        let rows = inventory
            .iter()
            .map(|row| (*row).to_owned())
            .collect::<Vec<_>>();
        let mut primary = Vec::with_capacity(rows.len());
        let mut by_note: BTreeMap<(VaultNoteId, VaultNoteKind), Vec<IndexRow>> = BTreeMap::new();
        let mut derived = Vec::new();
        let mut relations = Vec::new();
        for (borrowed, row) in inventory.iter().zip(rows.iter()) {
            let (note, kind, section) =
                resolved_primary_site(&anchor_subjects, *borrowed, granularity);
            let site = ClaimSite {
                note: note.clone(),
                section: section.to_owned(),
            };
            primary.push((row.clone(), site));
            by_note
                .entry((note.clone(), kind))
                .or_default()
                .push(row.clone());
            placement::derive(*borrowed, row, &note, &mut derived, &mut relations);
        }
        derived.sort();
        relations.sort();
        if relations.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ProjectionError::DuplicateRelation(
                relations.windows(2).find(|p| p[0] == p[1]).expect("found")[0].clone(),
            ));
        }
        let mut reverse_relations = relations
            .iter()
            .map(|r| Relation {
                from: r.to.clone(),
                rel: r.rel.clone(),
                to: r.from.clone(),
                origin: RelationOrigin::Derived,
            })
            .collect::<Vec<_>>();
        reverse_relations.sort();
        validate_incoming_relations(&relations, &reverse_relations)?;
        let certificate = ClaimCertificate::close(rows, primary, derived)?;
        let notes = by_note
            .into_iter()
            .map(|((id, kind), mut rows)| {
                rows.sort();
                VaultNotePlan { id, kind, rows }
            })
            .collect();
        Ok(Self {
            granularity,
            notes,
            certificate,
            relations,
            reverse_relations,
        })
    }

    /// Projects a checked repository fragment without asserting global closure.
    pub fn project_fragment(doc: &IndexDoc) -> Result<FragmentProjection, ProjectionError> {
        validate_capacity(doc)?;
        validate_source_paths(doc)?;
        check_index_fragment(doc)
            .map_err(|error| ProjectionError::InvalidFragment(error.to_string()))?;
        let (metadata, inventory) = doc.inventory();
        if metadata.visibility != Visibility::Public {
            return Err(ProjectionError::NonPublicDocument);
        }
        let known = local_ids(doc);
        let anchor_subjects = anchor_subjects(doc);
        let rows = inventory
            .iter()
            .map(|row| (*row).to_owned())
            .collect::<Vec<_>>();
        let mut primary = Vec::with_capacity(rows.len());
        let mut by_note = BTreeMap::new();
        for (borrowed, row) in inventory.iter().zip(&rows) {
            let (note, kind, section) =
                resolved_primary_site(&anchor_subjects, *borrowed, VaultGranularity::Compact);
            primary.push((
                row.clone(),
                ClaimSite {
                    note: note.clone(),
                    section: section.into(),
                },
            ));
            by_note
                .entry((note, kind))
                .or_insert_with(Vec::new)
                .push(row.clone());
        }
        let local = ClaimCertificate::close(rows, primary, Vec::new())?;
        let mut deferred = doc
            .edges
            .iter()
            .filter(|edge| !known.contains(edge.to.as_str()))
            .map(|edge| BoundaryWitness {
                from: edge.from.clone(),
                rel: edge.rel.clone(),
                external_to: edge.to.clone(),
            })
            .collect::<Vec<_>>();
        deferred.sort();
        let notes = by_note
            .into_iter()
            .map(|((id, kind), mut rows)| {
                rows.sort();
                VaultNotePlan { id, kind, rows }
            })
            .collect();
        Ok(FragmentProjection {
            notes,
            certificate: FragmentCertificate {
                metadata: MetadataIdentity {
                    schema: metadata.schema.into(),
                    generated_by: metadata.generated_by.into(),
                    visibility: metadata.visibility,
                },
                local,
                deferred,
            },
        })
    }
    /// Requested granularity.
    pub fn granularity(&self) -> VaultGranularity {
        self.granularity
    }
    /// Deterministic note plans.
    pub fn notes(&self) -> &[VaultNotePlan] {
        &self.notes
    }
    /// Exact claim certificate.
    pub fn certificate(&self) -> &ClaimCertificate {
        &self.certificate
    }
    /// Forward relations.
    pub fn relations(&self) -> &[Relation] {
        &self.relations
    }
    /// Reverse mirrors.
    pub fn reverse_relations(&self) -> &[Relation] {
        &self.reverse_relations
    }
}

/// Typed, bounded projection diagnostics; rows remain canonical values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectionError {
    /// Input crossed the public visibility boundary.
    NonPublicDocument,
    /// The canonical Index checker rejected the supposedly complete input.
    IncompleteDocument(String),
    /// A declaration or protocol row names no canonical anchor.
    MissingAnchor(String),
    /// The canonical inventory contains an exact duplicate row.
    DuplicateCanonicalRow(Box<IndexRow>),
    /// A primary claim names a row outside the canonical inventory.
    UnknownClaimedRow(Box<IndexRow>),
    /// A canonical row has no primary claim.
    UnclaimedRow(Box<IndexRow>),
    /// A canonical row has more than one primary claim.
    MultiplyClaimedRow(Box<IndexRow>),
    /// A derived claim has no corresponding primary claim.
    DerivedWithoutPrimary(Box<IndexRow>),
    /// Two identical canonical relations were supplied.
    DuplicateRelation(Relation),
    /// A repository fragment violated a local invariant.
    InvalidFragment(String),
    /// A source path is not a normalized repository-relative slash path.
    InvalidSourcePath(String),
    /// Source inventory arithmetic overflowed before allocation.
    InventorySizeOverflow,
    /// Incoming relation has no canonical forward origin.
    ReverseWithoutForward(Relation),
    /// A canonical forward relation lacks its derived incoming navigation.
    MissingDerivedRelation(Relation),
}

impl std::fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ProjectionError {}

// Keep the preservation law visible to rustdoc and the compiler: protocol
// resolution is never converted to a string or interpreted as an Index id.
const _: fn(&ProtocolResolution) = |_| {};

fn validate_capacity(doc: &IndexDoc) -> Result<usize, ProjectionError> {
    checked_inventory_size([
        doc.subjects.len(),
        doc.anchors.len(),
        doc.source_units.len(),
        doc.declarations.len(),
        doc.protocol_relations.len(),
        doc.surfaces.len(),
        doc.specimens.len(),
        doc.drafts.len(),
        doc.features.len(),
        doc.routes.len(),
        doc.edges.len(),
    ])
}

fn resolved_primary_site<'a>(
    anchor_subjects: &BTreeMap<&str, &str>,
    row: IndexRowRef<'a>,
    granularity: VaultGranularity,
) -> (VaultNoteId, VaultNoteKind, &'static str) {
    let (mut note, mut kind, section) = placement::primary_site(row, granularity);
    if granularity == VaultGranularity::Compact {
        let anchor = match row {
            IndexRowRef::Declaration(record) => Some(record.anchor.as_str()),
            IndexRowRef::ProtocolRelation(record) => Some(record.anchor.as_str()),
            _ => None,
        };
        if let Some(subject) = anchor.and_then(|id| anchor_subjects.get(id)) {
            note = VaultNoteId::new(*subject);
            kind = VaultNoteKind::Subject;
        }
    }
    (note, kind, section)
}

fn anchor_subjects(doc: &IndexDoc) -> BTreeMap<&str, &str> {
    doc.anchors
        .iter()
        .map(|record| (record.id.as_str(), record.subject.as_str()))
        .collect()
}

fn checked_inventory_size(
    lengths: impl IntoIterator<Item = usize>,
) -> Result<usize, ProjectionError> {
    lengths.into_iter().try_fold(0usize, |sum, len| {
        sum.checked_add(len)
            .ok_or(ProjectionError::InventorySizeOverflow)
    })
}

fn validate_source_paths(doc: &IndexDoc) -> Result<(), ProjectionError> {
    for path in doc
        .source_units
        .iter()
        .map(|unit| unit.path.as_str())
        .chain(
            doc.declarations
                .iter()
                .map(|fact| fact.location.file.as_str()),
        )
        .chain(doc.specimens.iter().map(|specimen| specimen.path.as_str()))
    {
        let unsafe_path = path.is_empty()
            || path.starts_with('/')
            || path.starts_with('\\')
            || path.contains('\\')
            || path.contains('\0')
            || path.bytes().any(|b| b.is_ascii_control())
            || path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || (path.len() >= 2
                && path.as_bytes()[0].is_ascii_alphabetic()
                && path.as_bytes()[1] == b':');
        if unsafe_path {
            return Err(ProjectionError::InvalidSourcePath(path.to_owned()));
        }
    }
    Ok(())
}

fn local_ids(doc: &IndexDoc) -> BTreeSet<&str> {
    doc.subjects
        .iter()
        .map(|r| r.id.as_str())
        .chain(doc.anchors.iter().map(|r| r.id.as_str()))
        .chain(doc.surfaces.iter().map(|r| r.id.as_str()))
        .chain(doc.specimens.iter().map(|r| r.id.as_str()))
        .chain(doc.features.iter().map(|r| r.id.as_str()))
        .chain(doc.routes.iter().map(|r| r.id.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{ProjectionError, checked_inventory_size};

    #[test]
    fn inventory_size_overflow_is_rejected_before_allocation() {
        assert_eq!(
            checked_inventory_size([usize::MAX, 1]),
            Err(ProjectionError::InventorySizeOverflow)
        );
    }
}
