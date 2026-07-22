//! Report and error types for index validation.

use std::{error::Error, fmt};

use crate::IndexDoc;

/// Successful validation summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexReport {
    /// Number of subjects.
    pub subjects: usize,
    /// Number of features.
    pub features: usize,
    /// Number of specimens.
    pub specimens: usize,
    /// Number of routes.
    pub routes: usize,
}

impl IndexReport {
    /// Builds a report from a checked document.
    pub fn from_doc(doc: &IndexDoc) -> Self {
        Self {
            subjects: doc.subjects.len(),
            features: doc.features.len(),
            specimens: doc.specimens.len(),
            routes: doc.routes.len(),
        }
    }
}

/// Validation failure for an index graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IndexError {
    /// A string field contains non-ASCII text.
    NonAscii {
        /// Field that failed the ASCII check.
        field: &'static str,
        /// Offending field value.
        value: String,
    },
    /// An id or key does not satisfy the index grammar.
    InvalidId {
        /// Kind of id being checked.
        kind: &'static str,
        /// Offending id text.
        id: String,
    },
    /// The same id appears twice in one collection.
    DuplicateId {
        /// Collection kind that contains the duplicate.
        kind: &'static str,
        /// Duplicate id text.
        id: String,
    },
    /// A feature or route points at a missing discovered fact.
    UnresolvedClaim {
        /// Feature, edge, or discovered row that made the claim.
        owner: String,
        /// Referenced fact kind.
        kind: &'static str,
        /// Missing id text.
        id: String,
    },
    /// Authored overlay attempted to include a literal fact instead of a discovered id.
    LiteralClaim {
        /// Draft that included the literal.
        owner: String,
        /// Literal fact kind.
        kind: &'static str,
    },
    /// The same discovered fact is claimed more than once by one owner.
    DuplicateClaim {
        /// Feature or draft that repeated the claim.
        owner: String,
        /// Repeated fact kind.
        kind: &'static str,
        /// Repeated id text.
        id: String,
    },
    /// Two features share one canonical key.
    DuplicateCanonicalKey {
        /// Duplicate canonical key text.
        key: String,
    },
    /// A grammar contract is not closed enough to index.
    InvalidGrammarContract {
        /// Feature or draft that owns the contract.
        owner: String,
        /// Grammar contract id.
        id: String,
    },
    /// A claimed specimen is not runnable and checked.
    NonRunnableSpecimen {
        /// Feature or draft that claimed the specimen.
        owner: String,
        /// Specimen id.
        id: String,
    },
    /// A route has no live feature or specimen step.
    DeadRouteStep {
        /// Route id.
        route: String,
        /// Missing step id, or `<empty>` for an empty route.
        step: String,
    },
    /// A documentation anchor reference has no discovered anchor.
    DanglingDocAnchor {
        /// Row that references the documentation anchor.
        owner: String,
        /// Missing documentation anchor id.
        id: String,
    },
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonAscii { field, value } => {
                write!(f, "{field} contains non-ASCII text: {value}")
            }
            Self::InvalidId { kind, id } => write!(f, "invalid {kind} id: {id}"),
            Self::DuplicateId { kind, id } => write!(f, "duplicate {kind} id: {id}"),
            Self::UnresolvedClaim { owner, kind, id } => {
                write!(f, "{owner} claims missing {kind}: {id}")
            }
            Self::LiteralClaim { owner, kind } => write!(f, "{owner} has literal {kind} claim"),
            Self::DuplicateClaim { owner, kind, id } => {
                write!(f, "{owner} claims duplicate {kind}: {id}")
            }
            Self::DuplicateCanonicalKey { key } => write!(f, "duplicate canonical key: {key}"),
            Self::InvalidGrammarContract { owner, id } => {
                write!(f, "{owner} has invalid grammar contract: {id}")
            }
            Self::NonRunnableSpecimen { owner, id } => {
                write!(f, "{owner} claims non-runnable specimen: {id}")
            }
            Self::DeadRouteStep { route, step } => {
                write!(f, "{route} has dead route step: {step}")
            }
            Self::DanglingDocAnchor { owner, id } => {
                write!(f, "{owner} references missing doc anchor: {id}")
            }
        }
    }
}

impl Error for IndexError {}
