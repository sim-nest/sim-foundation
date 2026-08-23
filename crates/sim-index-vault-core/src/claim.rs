use std::collections::{BTreeMap, BTreeSet};

use crate::{IndexRow, ProjectionError, VaultNoteId};

/// The single primary placement of a canonical row.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ClaimSite {
    /// Owning note.
    pub note: VaultNoteId,
    /// Stable semantic section within the note.
    pub section: String,
}

/// Navigation material derived from a canonically placed row.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct DerivedClaim {
    /// Canonical row being referenced.
    pub row: IndexRow,
    /// Derived placement.
    pub site: ClaimSite,
    /// Explicit derivation origin.
    pub origin: String,
}

/// Exact proof that canonical rows and primary claim keys are the same set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimCertificate {
    canonical: BTreeSet<IndexRow>,
    primary: BTreeMap<IndexRow, ClaimSite>,
    derived: Vec<DerivedClaim>,
}

impl ClaimCertificate {
    /// Closes an independently supplied claim set, returning precise failures.
    pub fn close(
        canonical: impl IntoIterator<Item = IndexRow>,
        primary: impl IntoIterator<Item = (IndexRow, ClaimSite)>,
        derived: Vec<DerivedClaim>,
    ) -> Result<Self, ProjectionError> {
        let mut rows = BTreeSet::new();
        for row in canonical {
            if !rows.insert(row.clone()) {
                return Err(ProjectionError::DuplicateCanonicalRow(Box::new(row)));
            }
        }
        let mut claims = BTreeMap::new();
        for (row, site) in primary {
            if claims.insert(row.clone(), site).is_some() {
                return Err(ProjectionError::MultiplyClaimedRow(Box::new(row)));
            }
        }
        if let Some(row) = claims.keys().find(|row| !rows.contains(*row)) {
            return Err(ProjectionError::UnknownClaimedRow(Box::new(row.clone())));
        }
        if let Some(row) = rows.iter().find(|row| !claims.contains_key(*row)) {
            return Err(ProjectionError::UnclaimedRow(Box::new(row.clone())));
        }
        if let Some(claim) = derived
            .iter()
            .find(|claim| !claims.contains_key(&claim.row))
        {
            return Err(ProjectionError::DerivedWithoutPrimary(Box::new(
                claim.row.clone(),
            )));
        }
        Ok(Self {
            canonical: rows,
            primary: claims,
            derived,
        })
    }
    /// Whether exact set equality was proven.
    pub fn is_closed(&self) -> bool {
        self.canonical.len() == self.primary.len()
    }
    /// Primary claim map.
    pub fn primary(&self) -> &BTreeMap<IndexRow, ClaimSite> {
        &self.primary
    }
    /// Explicit derived claims.
    pub fn derived(&self) -> &[DerivedClaim] {
        &self.derived
    }
}
