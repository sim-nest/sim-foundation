//! Shared records and checks for the SIM Index.
//!
//! `sim-index-core` is the source model for the derived SIM Index graph. It
//! keeps feature, package, surface, grammar, specimen, and route records in
//! plain Rust data; checks the graph before any codec or generated view consumes
//! it; and projects feature rows into ordinary kernel [`Card`] values.
//!
//! # Example
//!
//! ```
//! use sim_index_core::{
//!     FeatureId, SubjectId, canonical_feature_key,
//! };
//!
//! let feature = FeatureId::new("feature/sim-run/repl");
//! let subject = SubjectId::new("crate/sim-run");
//! let key = canonical_feature_key(&subject, feature.as_str());
//!
//! assert!(feature.is_valid());
//! assert_eq!(key.as_str(), "crate/sim-run/feature-sim-run-repl");
//! ```
//!
//! [`Card`]: sim_kernel::card::Card

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod card;
pub mod check;
mod check_error;
pub mod draft;
pub mod key;
pub mod model;
pub mod shape;
mod source_check;

pub use card::{declaration_card, feature_card, protocol_relation_card, route_card, specimen_card};
pub use check::{IndexError, IndexReport, check_index_doc, check_index_fragment};
pub use key::{CanonicalFeatureKey, canonical_feature_key};
pub use model::{
    AnchorId, DeclarationFact, DeclarationRole, DiscoveredAnchor, DiscoveredSpecimen,
    DiscoveredSurface, FeatureDraft, FeatureId, FeatureRecord, GrammarContract, IndexDoc,
    IndexEdge, ProtocolRelation, ProtocolResolution, RouteId, RouteRecord, RouteStep,
    SourceCompleteness, SourceLocation, SourceReachability, SourceUnit, SpecimenId, SubjectId,
    SubjectRecord, SurfaceId, SyntaxBound, UnresolvedReason, Visibility,
};

#[cfg(test)]
mod tests;
