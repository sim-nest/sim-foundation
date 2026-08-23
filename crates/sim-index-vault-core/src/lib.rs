//! Pure, complete vault projection for the canonical SIM Index inventory.
//!
//! The projection retains [`IndexRow`] values verbatim. Consumers may render
//! its note plan in any syntax, while this crate proves that every canonical
//! row has exactly one primary home and every derived navigation claim has an
//! explicit origin.
//!
//! ```
//! use sim_index_core::IndexDoc;
//! use sim_index_vault_core::{VaultGranularity, VaultProjection};
//!
//! let doc = IndexDoc::public("example");
//! let projection = VaultProjection::from_complete(&doc, VaultGranularity::Full)?;
//! assert!(projection.certificate().is_closed());
//! # Ok::<(), sim_index_vault_core::ProjectionError>(())
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::{BTreeMap, BTreeSet};

mod claim;
mod placement;

pub use claim::{ClaimCertificate, ClaimSite, DerivedClaim};

pub use sim_index_core::IndexRow;
use sim_index_core::{IndexDoc, IndexRowRef, ProtocolResolution, Visibility, check_index_doc};

/// Requested density of a future renderer. Both modes preserve every row.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum VaultGranularity {
    /// One compact primary placement per row.
    Compact,
    /// Primary placement plus all derived navigation claims.
    Full,
}

/// Stable, syntax-independent identity of a planned note.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct VaultNoteId(String);

impl VaultNoteId {
    /// Creates a note identity from a canonical Index id.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    /// Borrows the identity.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Semantic role of a note, independent of any application profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum VaultNoteKind {
    /// Graph-wide index material.
    Index,
    /// Subject-owned inventory.
    Subject,
    /// Source-anchor detail.
    Anchor,
    /// Authored or materialized feature detail.
    Feature,
    /// Ordered route detail.
    Route,
}

/// One deterministic note and the canonical rows placed in it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultNotePlan {
    /// Note identity.
    pub id: VaultNoteId,
    /// Semantic note kind.
    pub kind: VaultNoteKind,
    /// Canonical rows, in structural identity order.
    pub rows: Vec<IndexRow>,
}

/// Pure projection result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultProjection {
    granularity: VaultGranularity,
    notes: Vec<VaultNotePlan>,
    certificate: ClaimCertificate,
    relations: Vec<Relation>,
    reverse_relations: Vec<Relation>,
}

/// A canonical-id relation and its deterministic reverse mirror.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Relation {
    /// Canonical source id.
    pub from: String,
    /// Relationship label.
    pub rel: String,
    /// Canonical target id.
    pub to: String,
}

impl VaultProjection {
    /// Projects only a checked, complete public document through `inventory()`.
    pub fn from_complete(
        doc: &IndexDoc,
        granularity: VaultGranularity,
    ) -> Result<Self, ProjectionError> {
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
            let (note, kind, section) = placement::primary_site(*borrowed);
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
            })
            .collect::<Vec<_>>();
        reverse_relations.sort();
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
