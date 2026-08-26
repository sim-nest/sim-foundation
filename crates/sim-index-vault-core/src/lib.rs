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

use std::collections::BTreeSet;

mod claim;
mod placement;

pub use claim::{ClaimCertificate, ClaimSite, DerivedClaim};

pub use sim_index_core::IndexRow;
use sim_index_core::Visibility;

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
    /// Invoke/read/write/view surface detail.
    Surface,
    /// Runnable specimen detail.
    Specimen,
    /// Authored feature draft detail.
    Draft,
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
    /// Whether this fact was present in inventory or derived as incoming navigation.
    pub origin: RelationOrigin,
}

impl Relation {
    /// Builds a canonical forward relation.
    pub fn canonical(
        from: impl Into<String>,
        rel: impl Into<String>,
        to: impl Into<String>,
    ) -> Self {
        Self {
            from: from.into(),
            rel: rel.into(),
            to: to.into(),
            origin: RelationOrigin::Canonical,
        }
    }
}

/// Validates that supplied incoming navigation is exactly derived from forward facts.
pub fn validate_incoming_relations(
    forward: &[Relation],
    incoming: &[Relation],
) -> Result<(), ProjectionError> {
    let expected = forward
        .iter()
        .map(|relation| Relation {
            from: relation.to.clone(),
            rel: relation.rel.clone(),
            to: relation.from.clone(),
            origin: RelationOrigin::Derived,
        })
        .collect::<BTreeSet<_>>();
    let supplied = incoming.iter().cloned().collect::<BTreeSet<_>>();
    if let Some(relation) = supplied.difference(&expected).next() {
        return Err(ProjectionError::ReverseWithoutForward(relation.clone()));
    }
    if let Some(relation) = expected.difference(&supplied).next() {
        return Err(ProjectionError::MissingDerivedRelation(relation.clone()));
    }
    Ok(())
}

/// Provenance of a projected relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum RelationOrigin {
    /// Forward fact from the canonical inventory.
    Canonical,
    /// Incoming relation mechanically derived from a canonical forward fact.
    Derived,
}

/// Identity retained by complete and fragment certificates.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct MetadataIdentity {
    /// Index schema.
    pub schema: String,
    /// Generator identity.
    pub generated_by: String,
    /// Source visibility.
    pub visibility: Visibility,
}

/// A typed relation endpoint intentionally deferred to another repository.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct BoundaryWitness {
    /// Local source id.
    pub from: String,
    /// Canonical relationship label.
    pub rel: String,
    /// External target id.
    pub external_to: String,
}

/// Exact local-coverage proof for one repository fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FragmentCertificate {
    metadata: MetadataIdentity,
    local: ClaimCertificate,
    deferred: Vec<BoundaryWitness>,
}

impl FragmentCertificate {
    /// Fragment metadata identity.
    pub fn metadata(&self) -> &MetadataIdentity {
        &self.metadata
    }
    /// Exact certificate for every local primary row.
    pub fn local_claims(&self) -> &ClaimCertificate {
        &self.local
    }
    /// Sorted external endpoints retained as visible evidence.
    pub fn deferred_external_endpoints(&self) -> &[BoundaryWitness] {
        &self.deferred
    }
    /// Fragments deliberately cannot claim whole-graph completeness.
    pub const fn is_whole_graph_complete(&self) -> bool {
        false
    }
}

/// A repository-local projection with explicit graph-boundary evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FragmentProjection {
    notes: Vec<VaultNotePlan>,
    certificate: FragmentCertificate,
}

impl FragmentProjection {
    /// Deterministic local note plans.
    pub fn notes(&self) -> &[VaultNotePlan] {
        &self.notes
    }
    /// Local and boundary certificate.
    pub fn certificate(&self) -> &FragmentCertificate {
        &self.certificate
    }
}

mod projection;
pub use projection::ProjectionError;
